//! Combat (M3): damage, attack orders, animals, projectiles.
//!
//! Implements the rulebook's combat stack (§CMB/PRJ/CLS) inside the
//! deterministic sim:
//!
//! - Damage is table-driven (CMB-01): for each (class, value) pair in
//!   the attacker's row, if the target belongs to that class the pair
//!   contributes max(0, value − armor); the sum is scaled by the
//!   elevation factor (×1.25 attacker higher, ×0.75 attacker lower,
//!   ×1 level — MAP-06, binary: any height difference of ≥ 1 level
//!   gives the full factor) and the total is at least 1. Before M7
//!   the map was flat, so the factor was ×1 everywhere.
//! - The attack cycle (CMB-04/05/06): the first strike of an
//!   engagement lands after the unit's attack delay; later strikes
//!   land every reload ticks, counted from the previous landing.
//!   A melee strike lands only if the target is still in reach at the
//!   landing tick, else it whiffs (the reload still applies) and the
//!   attacker chases.
//! - Projectiles are simulated entities (PRJ-01): fixed trajectory
//!   from release point to impact point, no homing; the target dodges
//!   by moving (PRJ-06) and a near-miss can mishit another unit for
//!   50% (CMB-13) — never a friendly (archer-type flag, study §2.5).
//! - Accuracy is rolled ONCE per shot, at release, from the sim RNG
//!   (PRJ-03) — the sim's first RNG consumer. Rolls happen only here,
//!   in the combat phase, in ascending-attacker-id order, so every
//!   machine makes the same draws in the same order (SIM-04).
//! - Deaths are processed in one sweep at the end of the phase
//!   (CMB-02: HP < 1), ascending id; animals leave carcasses, and the
//!   villager credited with the kill starts harvesting (the standard
//!   hunt feel). A dead villager's carry is lost with it (CMB-14).
//!
//! Gaia behavior lives here too: sheep capture by proximity, the
//! boar's aggro → chase → give-up → return → heal loop (OPENING-SPEC
//! §2.2), and deer flight. Villagers never auto-attack and do not
//! flee (M5 AI polish); idle Militia/Archers auto-acquire boars in
//! LoS (CMB-08 — herdables and deer are never auto-targeted, and the
//! starting Scout's Stand Ground default means it does not acquire).

use fixed::types::I16F16;

use crate::build::BuildingKind;
use crate::classes as cls;
use crate::econ::{NodeKind, ResourceNode};
use crate::entity::{is_player_owner, EntityKind, Task, OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER};
use crate::units::{self, UnitSpec};
use crate::{steer, Sim};

/// Melee reach, center to center: 3.0 world units. Tuning constant:
/// separation holds two fighting units ~2.0–2.2 units apart, and the
/// reference lands melee hits at radius-sum distance — 3.0 gives the
/// slack CMB-06's landing check needs without letting units hit
/// across a crowd.
pub const MELEE_REACH: I16F16 = I16F16::from_bits(196_608); // 3.0
/// A herdable converts when a player unit stands this close
/// (4.0 wu = 1 tile). The wiki states the rule as proximity only —
/// "moving any of their units next to it" (Herdable animal) — and
/// gives no number; 1 tile is the tuned constant that reads as
/// "next to it" (rulebook AI-06).
pub const CAPTURE_RANGE: I16F16 = I16F16::from_bits(262_144); // 4.0
/// An un-locked boar gives up when it is this far from its target…
const GIVE_UP_CHASE: I16F16 = I16F16::from_bits(1_310_720); // 20.0
/// …or when its target is this far from the boar's post ("a short
/// distance" in the spec's lure rule).
const GIVE_UP_HOME: I16F16 = I16F16::from_bits(1_572_864); // 24.0
/// A returning boar that gets this close to its post has arrived:
/// it heals to full and resets.
const HOME_ARRIVE: I16F16 = I16F16::from_bits(98_304); // 1.5
/// A projectile hits a unit standing within its table radius plus
/// this slack of the impact point (PRJ-06 geometry).
const HIT_SLACK: I16F16 = I16F16::from_bits(49_152); // 0.75
/// A chasing attacker re-paths when its order's target has drifted
/// this far from the victim's current position.
const CHASE_REPATH: I16F16 = I16F16::from_bits(196_608); // 3.0
/// How long a deer keeps fleeing after a hit.
const DEER_FLEE_TICKS: u64 = 90;

/// One projectile in flight (PRJ-01): spawn point, impact point,
/// spawn/impact ticks, the intended target, and a snapshot of the
/// attacker's attack table — the arrow lands with full effect even if
/// the archer dies first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projectile {
    pub spawn_x: I16F16,
    pub spawn_y: I16F16,
    pub impact_x: I16F16,
    pub impact_y: I16F16,
    pub spawn_tick: u64,
    pub impact_tick: u64,
    /// The unit the shot was aimed at (release-time position is the
    /// aim point — no Ballistics tech in M3, PRJ-05).
    pub intended: Option<u32>,
    /// The placed building the shot was aimed at (M5), when the
    /// target was a building rather than a unit. Mutually exclusive
    /// with `intended`.
    pub intended_building: Option<u32>,
    /// Attacker entity id (kill credit).
    pub attacker: u32,
    /// Attacker owner (friendly-fire exclusion).
    pub owner: u8,
    /// The attacker's elevation level at release (M7, PRJ-10):
    /// height enters a shot's damage only through the elevation
    /// factor, comparing this against the target's level at impact.
    pub attacker_elev: u8,
    /// Snapshot of the attacker's (class, value) attack table.
    pub attacks: Vec<(u8, i32)>,
}

/// The pre-elevation damage sum of CMB-01: Σ over participating
/// (class, value) pairs of max(0, value − armor), and whether any
/// pair participated at all. The minimum-1 clamp is NOT applied
/// here — CMB-01 applies the elevation factor to this sum first
/// and clamps after (`compute_damage_elev`); on flat ground the
/// factor is 1 and the two readings coincide.
fn damage_sum(attacks: &[(u8, i32)], target: &UnitSpec) -> (i32, bool) {
    let mut total: i32 = 0;
    let mut participated = false;
    for &(class, value) in attacks {
        let armor = match class {
            cls::MELEE => {
                participated = true;
                target.armor_melee
            }
            cls::PIERCE => {
                participated = true;
                target.armor_pierce
            }
            c => {
                if target.classes & cls::bit(c) == 0 {
                    continue; // non-member classes contribute exactly zero (CMB-03)
                }
                participated = true;
                0 // no class armor beyond melee/pierce in the M3 tables
            }
        };
        total += (value - armor).max(0);
    }
    (total, participated)
}

/// CMB-01 damage of one landed attack against a target on LEVEL
/// ground, as a whole number (all table values are integers).
/// Pure — unit-tested directly. Returns 0 only when no attack pair
/// participates (an attacker with no applicable class at all);
/// any participating attack deals at least 1.
pub fn compute_damage(attacks: &[(u8, i32)], target: &UnitSpec) -> i32 {
    let (total, participated) = damage_sum(attacks, target);
    if participated {
        total.max(1)
    } else {
        0
    }
}

/// The elevation factor of MAP-06 / CMB-01: ×1.25 when the attacker
/// stands on higher ground than the target, ×0.75 when lower, ×1
/// when level — binary, so a difference of one level already gives
/// the full factor. Both values are exact in I16F16.
pub fn elevation_factor(attacker_elev: u8, target_elev: u8) -> I16F16 {
    match attacker_elev.cmp(&target_elev) {
        std::cmp::Ordering::Greater => I16F16::from_bits(81_920), // 1.25
        std::cmp::Ordering::Less => I16F16::from_bits(49_152),    // 0.75
        std::cmp::Ordering::Equal => I16F16::from_num(1),
    }
}

/// CMB-01 damage with the elevation factor (M7): max(1, k_elev × Σ)
/// over the armor-subtracted sum, as fixed-point (a ×1.25 of an odd
/// sum is fractional — HP is fractional state, so nothing rounds).
/// Returns 0 only when no attack pair participates.
pub fn compute_damage_elev(
    attacks: &[(u8, i32)],
    target: &UnitSpec,
    attacker_elev: u8,
    target_elev: u8,
) -> I16F16 {
    let (total, participated) = damage_sum(attacks, target);
    if !participated {
        return I16F16::ZERO;
    }
    let dmg = I16F16::from_num(total) * elevation_factor(attacker_elev, target_elev);
    if dmg < I16F16::from_num(1) {
        I16F16::from_num(1)
    } else {
        dmg
    }
}

/// The damage-table view of a placed building kind (M5): HP and
/// armor from the study's building table (`BuildingKind::max_hp` /
/// `armor_melee` / `armor_pierce`), classes All Buildings + Standard
/// Buildings (the same membership the Town Center entity carries in
/// the unit tables). Everything else is inert — a building never
/// moves, shoots, or trains through this view.
pub(crate) fn building_spec(kind: BuildingKind) -> UnitSpec {
    UnitSpec {
        max_hp: kind.max_hp(),
        attacks: &[],
        armor_melee: kind.armor_melee(),
        armor_pierce: kind.armor_pierce(),
        classes: cls::mask(&[cls::ALL_BUILDINGS, cls::STANDARD_BUILDINGS]),
        range_max: I16F16::ZERO,
        reload_ticks: 0,
        attack_delay_ticks: 0,
        accuracy: 0,
        projectile_speed: I16F16::ZERO,
        scatter: I16F16::ZERO,
        speed: I16F16::ZERO,
        los: I16F16::ZERO,
        radius: I16F16::ZERO,
        carcass_food: 0,
        carcass_decay_bits: 0,
        food_cost: 0,
        wood_cost: 0,
        gold_cost: 0,
        train_ticks: 0,
    }
}

impl Sim {
    /// The combat phase of a tick (SIM-07: after movement, before
    /// economy). Fixed internal order: capture, animal AI,
    /// acquisition, attack machines (ascending id), projectile
    /// impacts (spawn order), death sweep (ascending id).
    pub(crate) fn combat_phase(&mut self) {
        // MOV-04 flags are per-tick: engaged melee attackers re-set
        // theirs in their attack step below.
        for e in &mut self.entities {
            e.suppress_separation = false;
        }
        self.capture_sheep();
        self.animal_ai();
        self.acquire_targets();
        for idx in 0..self.entities.len() {
            self.attack_step(idx);
        }
        // M5: building attackers run their own step, after the
        // entity attackers (same ascending-id discipline).
        for idx in 0..self.entities.len() {
            self.attack_building_step(idx);
        }
        self.projectile_step();
        self.death_sweep();
    }

    /// Sheep conversion (ECO-13 / AI-06; wiki, Sheep + Herdable
    /// animal): a herdable belongs to the player whose units stand
    /// near it. A Gaia sheep converts to the player with the MOST
    /// units within capture range (an exact tie leaves it wild —
    /// the wiki names no tie-break, so the deterministic choice is
    /// to hold). An owned sheep stays its owner's while any
    /// friendly unit remains in range; left with no friendly unit
    /// nearby, it falls to an enemy whose unit passes by — sheep
    /// can be stolen this way. Only a player's real mobile units
    /// count: buildings (the Town Center included) never convert,
    /// and herdables never hold a herd for anyone.
    fn capture_sheep(&mut self) {
        for idx in 0..self.entities.len() {
            let e = &self.entities[idx];
            if e.kind != EntityKind::Sheep || e.dead {
                continue;
            }
            let (sx, sy) = (e.pos_x, e.pos_y);
            // Presence per owner tag (slot 1 is Gaia and stays 0).
            let mut near = [0u32; 3];
            for u in &self.entities {
                if u.dead
                    || u.kind == EntityKind::Sheep
                    || !u.kind.is_mobile()
                    || !is_player_owner(u.owner)
                {
                    continue;
                }
                if steer::vec_len(u.pos_x - sx, u.pos_y - sy) <= CAPTURE_RANGE {
                    near[u.owner as usize] += 1;
                }
            }
            let cur = self.entities[idx].owner;
            let next = if cur == OWNER_GAIA {
                if near[OWNER_PLAYER as usize] > near[OWNER_OPPONENT as usize] {
                    Some(OWNER_PLAYER)
                } else if near[OWNER_OPPONENT as usize] > near[OWNER_PLAYER as usize] {
                    Some(OWNER_OPPONENT)
                } else {
                    None
                }
            } else if near[cur as usize] == 0 {
                let foe = if cur == OWNER_PLAYER {
                    OWNER_OPPONENT
                } else {
                    OWNER_PLAYER
                };
                if near[foe as usize] > 0 {
                    Some(foe)
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(owner) = next {
                let e = &mut self.entities[idx];
                e.owner = owner;
                // A converted sheep stops and waits for its new
                // owner: a Gaia wander step or the old owner's
                // queued moves do not carry over the flip.
                e.order = None;
                e.order_queue.clear();
            }
        }
    }

    /// Gaia behavior: the boar's lure loop, deer flight, and sheep
    /// wander. Runs before the attack machines so a boar's aggro
    /// becomes its attack target on the same tick.
    fn animal_ai(&mut self) {
        for idx in 0..self.entities.len() {
            let (kind, dead) = (self.entities[idx].kind, self.entities[idx].dead);
            if dead {
                continue;
            }
            match kind {
                EntityKind::Boar => self.boar_ai(idx),
                EntityKind::Deer => self.deer_ai(idx),
                EntityKind::Sheep => self.sheep_wander(idx),
                _ => {}
            }
        }
    }

    /// Gaia sheep wander (AI-06; wiki, Gaia: "all predator,
    /// huntable, and herdable animals can wander in a specific
    /// area"): short slow steps around the spawn post with long
    /// graze pauses between them, every choice drawn from the
    /// seeded RNG in entity order. Owned sheep never wander —
    /// they hold position until their owner orders them.
    fn sheep_wander(&mut self, idx: usize) {
        {
            let e = &self.entities[idx];
            if e.owner != OWNER_GAIA || e.order.is_some() {
                return;
            }
            if e.wander_until == 0 {
                // First sight: schedule the first decision a few
                // seconds out, staggered by id so the herd never
                // steps in lockstep on the opening ticks.
                let first = self.tick + 240 + (e.id as u64 * 67) % 480;
                self.entities[idx].wander_until = first;
                return;
            }
            if self.tick < e.wander_until {
                return;
            }
        }
        // Decide this round: graze on, or take a short step. The
        // next decision lands 12–30 s out either way, so a walking
        // sheep (a ≤1-tile step takes ~1.5 s at sheep speed)
        // always arrives and grazes long before the following
        // one. The cadence is deliberately lazy — reference sheep
        // mostly stand and graze, shuffling a tile or so now and
        // then — and three of four rounds are pure grazing.
        let next = self.tick + 720 + self.rng.below(1080) as u64;
        self.entities[idx].wander_until = next;
        if self.rng.below(4) != 0 {
            return; // graze this round
        }
        let (px, py, hx, hy) = {
            let e = &self.entities[idx];
            (e.pos_x, e.pos_y, e.home_x, e.home_y)
        };
        let dx = self.rng.below(9) as i32 - 4; // ±4 wu = ±1 tile
        let dy = self.rng.below(9) as i32 - 4;
        if dx == 0 && dy == 0 {
            return;
        }
        // Leash: the step's target stays within ~2 tiles of the
        // post, so a herd drifts around its area instead of
        // marching off across the map.
        let mut tx = px + I16F16::from_num(dx);
        let mut ty = py + I16F16::from_num(dy);
        let (lx, ly) = (tx - hx, ty - hy);
        let len = steer::vec_len(lx, ly);
        let leash = I16F16::from_num(8);
        if len > leash {
            tx = hx + lx * leash / len;
            ty = hy + ly * leash / len;
        }
        let tx = crate::clamp_pos(tx);
        let ty = crate::clamp_pos(ty);
        self.set_move_order(idx, tx, ty);
    }

    /// The boar lure loop (OPENING-SPEC §2.2): aggro mirrors into the
    /// attack target; an un-locked boar gives up when the chase runs
    /// long, walks back to its post, and heals there; a locked boar
    /// (two hits taken, or any villager hit) pursues until someone
    /// dies.
    fn boar_ai(&mut self, idx: usize) {
        // Validate the aggro target.
        if let Some(a) = self.entities[idx].aggro_target {
            let valid = self
                .entity_index(a)
                .map(|ai| !self.entities[ai].dead)
                .unwrap_or(false);
            if !valid {
                let e = &mut self.entities[idx];
                e.aggro_target = None;
                e.clear_combat();
                e.returning_home = true;
            }
        }
        if let Some(a) = self.entities[idx].aggro_target {
            if !self.entities[idx].aggro_locked {
                let ai = self.entity_index(a).expect("validated above");
                let (ax, ay) = (self.entities[ai].pos_x, self.entities[ai].pos_y);
                let (hx, hy) = (self.entities[idx].home_x, self.entities[idx].home_y);
                let d_chase = self.distance_to(idx, ax, ay);
                let d_home = steer::vec_len(ax - hx, ay - hy);
                if d_chase > GIVE_UP_CHASE || d_home > GIVE_UP_HOME {
                    let e = &mut self.entities[idx];
                    e.aggro_target = None;
                    e.clear_combat();
                    e.returning_home = true;
                    return;
                }
            }
            let e = &mut self.entities[idx];
            e.attack_target = Some(a);
            e.returning_home = false;
            return;
        }
        // No aggro: head home if away, rest (and heal) at the post.
        if self.entities[idx].returning_home {
            let (hx, hy) = (self.entities[idx].home_x, self.entities[idx].home_y);
            if self.distance_to(idx, hx, hy) > HOME_ARRIVE {
                if self.entities[idx].order.is_none()
                    && !self.set_move_order(idx, hx, hy)
                {
                    // Home is unreachable (a building completed over
                    // the post): rest here instead of re-pathing
                    // every tick (M6 perf fix).
                    self.entities[idx].returning_home = false;
                }
                return;
            }
            // At the post it rests only once the fight is really over
            // — no living player unit within its LoS. (A boar that
            // kills its lure target mid-brawl must not reset while
            // the victim's friends are still swinging at it: the next
            // landed hit re-aggros it via `apply_hit` instead.)
            let (bx, by) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
            let los = units::spec(EntityKind::Boar).los;
            let threat_near = self.entities.iter().any(|u| {
                is_player_owner(u.owner)
                    && !u.dead
                    && u.kind.is_mobile()
                    && steer::vec_len(u.pos_x - bx, u.pos_y - by) <= los
            });
            if !threat_near {
                let max_hp = I16F16::from_num(units::spec(EntityKind::Boar).max_hp);
                let e = &mut self.entities[idx];
                e.returning_home = false;
                e.hp = max_hp;
                e.hits_taken = 0;
                e.aggro_locked = false;
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
        }
    }

    /// Deer flight: while the flee timer runs, keep a move order away
    /// from where the attack came from (re-issued when it completes).
    fn deer_ai(&mut self, idx: usize) {
        if self.tick >= self.entities[idx].flee_until || self.entities[idx].order.is_some() {
            return;
        }
        let e = &self.entities[idx];
        let (px, py) = (e.pos_x, e.pos_y);
        let (dx, dy) = (px - e.flee_from_x, py - e.flee_from_y);
        let len = steer::vec_len(dx, dy);
        let (ux, uy) = if len > I16F16::ZERO {
            (dx / len, dy / len)
        } else {
            (I16F16::from_num(1), I16F16::ZERO)
        };
        let dest_x = crate::clamp_pos(px + ux * I16F16::from_num(10));
        let dest_y = crate::clamp_pos(py + uy * I16F16::from_num(10));
        self.set_move_order(idx, dest_x, dest_y);
    }

    /// Auto-acquisition (CMB-08): idle Militia-line units and Archers of *any*
    /// player engage the closest enemy inside their LoS. In M3 the
    /// only enemies were boars; since M5 an opposing player's units
    /// qualify too (its Town Center never does — razing a Town
    /// Center is always an explicit order, never an accident of
    /// proximity). Herdables and deer are never auto-targeted
    /// (killing them is always an explicit order), villagers never
    /// auto-attack, and the Scout's Dark Age default stance is Stand
    /// Ground (CMB-09), so it does not acquire either.
    fn acquire_targets(&mut self) {
        for idx in 0..self.entities.len() {
            let e = &self.entities[idx];
            if e.dead
                || !is_player_owner(e.owner)
                || !matches!(
                    e.kind,
                    EntityKind::Militia | EntityKind::ManAtArms | EntityKind::Archer
                )
                || e.attack_target.is_some()
                || e.attack_building.is_some()
                || e.order.is_some()
            {
                continue;
            }
            let owner = e.owner;
            let spec = self.unit_stats(idx); // M6: the owner's effective LoS
            let (px, py) = (e.pos_x, e.pos_y);
            let mut best: Option<(i64, u32)> = None; // (distance² bits, id)
            for t in &self.entities {
                if t.dead || t.id == e.id {
                    continue;
                }
                let qualifies = t.kind == EntityKind::Boar
                    || (is_player_owner(t.owner)
                        && t.owner != owner
                        && t.kind != EntityKind::TownCenter);
                if !qualifies {
                    continue;
                }
                let d = steer::vec_len(t.pos_x - px, t.pos_y - py);
                if d > spec.los {
                    continue;
                }
                let db = d.to_bits() as i64;
                let key = (db * db, t.id);
                if best.map(|b| key < b).unwrap_or(true) {
                    best = Some(key);
                }
            }
            if let Some((_, tid)) = best {
                let e = &mut self.entities[idx];
                e.attack_target = Some(tid);
                e.attack_engaged = false;
                e.strike_tick = None;
            }
        }
    }

    /// One attacker's tick: resolve a due strike, then stand-and-fight
    /// when in range or chase when not. Melee attackers in range hold
    /// their ground (order cleared) and suppress separation (MOV-04).
    fn attack_step(&mut self, idx: usize) {
        let tick = self.tick;
        let tid = match self.entities[idx].attack_target {
            Some(t) if !self.entities[idx].dead => t,
            _ => return,
        };
        // M6: the attacker's effective row (owner's techs/age —
        // Fletching's range and attack enter here, via the lookups).
        let spec = self.unit_stats(idx);
        if spec.attacks.is_empty() {
            self.entities[idx].clear_combat();
            return;
        }
        let ti = match self.entity_index(tid) {
            Some(i) if !self.entities[i].dead => i,
            _ => {
                self.entities[idx].clear_combat();
                return;
            }
        };
        let (tx, ty) = (self.entities[ti].pos_x, self.entities[ti].pos_y);
        let dist = self.distance_to(idx, tx, ty);
        let in_range = if spec.range_max > I16F16::ZERO {
            dist <= spec.range_max
        } else {
            dist <= MELEE_REACH
        };

        // A pending strike lands (or whiffs) on its tick, wherever
        // the chase has taken us — CMB-06 checks range at landing.
        if let Some(st) = self.entities[idx].strike_tick {
            if tick >= st {
                self.resolve_strike(idx, ti);
            }
        }

        if in_range {
            // Stand and fight: drop any chase order and hold position.
            if self.entities[idx].order.is_some() {
                let e = &mut self.entities[idx];
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
            if spec.range_max == I16F16::ZERO {
                self.entities[idx].suppress_separation = true;
            }
            let ready = tick >= self.entities[idx].next_attack_tick;
            if self.entities[idx].strike_tick.is_none() && ready {
                if self.entities[idx].attack_engaged {
                    // Sustained attacks land the tick the reload
                    // completes — the delay applies to the first
                    // strike of an engagement only (CMB-04).
                    self.resolve_strike(idx, ti);
                } else {
                    let at = tick + spec.attack_delay_ticks;
                    self.entities[idx].strike_tick = Some(at);
                }
            }
        } else {
            // Chase: refresh the internal move order when the victim
            // has drifted from where we are headed (CMB-06: chasing is
            // mechanism, not a player order).
            let need = match &self.entities[idx].order {
                None => true,
                Some(o) => steer::vec_len(o.target_x - tx, o.target_y - ty) > CHASE_REPATH,
            };
            if need {
                if !self.set_move_order(idx, tx, ty) {
                    // The victim cannot be reached from here (it
                    // stands inside a footprint pocket, or this
                    // attacker is enclosed): the chase is over —
                    // retrying would re-run a failing full-grid A*
                    // every tick (M6 perf fix).
                    self.entities[idx].clear_combat();
                }
            }
        }
    }

    /// Land one strike from attacker `idx` on target `ti`: melee
    /// damage now (if still in reach), or a projectile release for a
    /// ranged attacker. Either way the reload restarts from this
    /// landing tick (CMB-05) and the engagement is underway.
    fn resolve_strike(&mut self, idx: usize, ti: usize) {
        // M6: release-time snapshot of the attacker's *effective*
        // attacks — a Fletching completing mid-flight does not
        // upgrade an arrow already in the air (TEC-06 discipline).
        let spec = self.unit_stats(idx);
        let attacker_id = self.entities[idx].id;
        let owner = self.entities[idx].owner;
        let (ax, ay) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let (tx, ty) = (self.entities[ti].pos_x, self.entities[ti].pos_y);
        if spec.range_max > I16F16::ZERO {
            // Ranged release: the accuracy roll is drawn NOW, at
            // release, in combat-phase id order (PRJ-03/SIM-04).
            let roll = self.rng.below(100);
            let mut impact = (tx, ty);
            if roll >= spec.accuracy {
                // Miss: scatter within the weapon's radius — two
                // further draws, same fixed order (PRJ-04).
                let sb = spec.scatter.to_bits();
                let span = (2 * sb + 1) as u32;
                let dx = self.rng.below(span) as i32 - sb;
                let dy = self.rng.below(span) as i32 - sb;
                impact = (tx + I16F16::from_bits(dx), ty + I16F16::from_bits(dy));
            }
            // Flight time = distance ÷ speed at spawn (PRJ-02),
            // integer ceiling, at least one tick.
            let dist_bits = steer::vec_len(tx - ax, ty - ay).to_bits() as i64;
            let speed_bits = spec.projectile_speed.to_bits() as i64;
            let flight = ((dist_bits + speed_bits - 1) / speed_bits).max(1) as u64;
            self.projectiles.push(Projectile {
                spawn_x: ax,
                spawn_y: ay,
                impact_x: impact.0,
                impact_y: impact.1,
                spawn_tick: self.tick,
                impact_tick: self.tick + flight,
                intended: Some(self.entities[ti].id),
                intended_building: None,
                attacker: attacker_id,
                owner,
                // PRJ-10 (M7): the shooter's elevation is captured
                // at release, like its attack table.
                attacker_elev: self.entity_height(idx),
                attacks: spec.attacks.to_vec(),
            });
        } else {
            // Melee: lands only if the target is still in reach at
            // the landing tick (CMB-06); otherwise it whiffs.
            let dist = steer::vec_len(tx - ax, ty - ay);
            if dist <= MELEE_REACH {
                let atk_elev = self.entity_height(idx);
                self.apply_hit(attacker_id, (ax, ay), spec.attacks, ti, false, atk_elev);
            }
        }
        let e = &mut self.entities[idx];
        e.strike_tick = None;
        e.next_attack_tick = self.tick + spec.reload_ticks;
        e.attack_engaged = true;
    }

    /// Apply one landed hit to entity `ti`: damage, kill credit, and
    /// the victim's reactions — a boar aggros its attacker (with the
    /// two-hit/villager lock), a deer flees, an idle military unit
    /// retaliates. `threat` is where the hit came from (the melee
    /// attacker's position, or a projectile's spawn point).
    fn apply_hit(
        &mut self,
        attacker_id: u32,
        threat: (I16F16, I16F16),
        attacks: &[(u8, i32)],
        ti: usize,
        mishit: bool,
        atk_elev: u8,
    ) {
        // M6: damage reads the target's *effective* armor — Loom's
        // +1/+2 and the Blacksmith armor lines land here (CMB-01
        // against the patched row, TEC-06). M7: the elevation
        // factor compares the attacker's level (`atk_elev` — the
        // shooter's at release for a projectile, PRJ-10) against
        // the target's level where it stands now.
        let tspec = self.unit_stats(ti);
        let tgt_elev = self.entity_height(ti);
        let full = compute_damage_elev(attacks, &tspec, atk_elev, tgt_elev);
        let dmg = if mishit {
            // CMB-13: a mishit deals 50% (fixed-point halving is exact).
            full / I16F16::from_num(2)
        } else {
            full
        };
        {
            let t = &mut self.entities[ti];
            t.hp -= dmg;
            t.last_hit_by = Some(attacker_id);
        }
        match self.entities[ti].kind {
            EntityKind::Boar => {
                let ai = self.entity_index(attacker_id);
                if let Some(ai) = ai {
                    let (aowner, adead, akind) = (
                        self.entities[ai].owner,
                        self.entities[ai].dead,
                        self.entities[ai].kind,
                    );
                    if aowner != OWNER_GAIA && !adead {
                        let b = &mut self.entities[ti];
                        b.aggro_target = Some(attacker_id);
                        b.returning_home = false;
                        b.hits_taken += 1;
                        // The lure lock: a villager's hit, or the
                        // second hit from anyone, commits the boar.
                        if akind == EntityKind::Villager || b.hits_taken >= 2 {
                            b.aggro_locked = true;
                        }
                    }
                }
            }
            EntityKind::Deer => {
                let t = &mut self.entities[ti];
                t.flee_until = self.tick + DEER_FLEE_TICKS;
                t.flee_from_x = threat.0;
                t.flee_from_y = threat.1;
            }
            EntityKind::Militia | EntityKind::Archer => {
                // Retaliation: an idle military unit of either
                // player answers its attacker (M5). (Villagers never
                // auto-attack — task rule.)
                let t = &self.entities[ti];
                if is_player_owner(t.owner) && t.attack_target.is_none() && t.order.is_none() {
                    self.entities[ti].attack_target = Some(attacker_id);
                }
            }
            _ => {}
        }
    }

    /// One building-attacker's tick (M5): the `attack_step`
    /// counterpart for placed buildings. Range is measured to the
    /// footprint's edge (the nearest point of its rectangle), chase
    /// targets a stand point just off the footprint's edge — its
    /// center is solid ground a unit can never stand on, and a ring
    /// cell's center can sit beyond melee reach of the edge (see
    /// `building_approach_point`) — and strikes follow
    /// the same delay/reload discipline as entity combat (CMB-04/05).
    fn attack_building_step(&mut self, idx: usize) {
        let tick = self.tick;
        let bid = match self.entities[idx].attack_building {
            Some(b) if !self.entities[idx].dead => b,
            _ => return,
        };
        let spec = self.unit_stats(idx); // M6: effective row (as attack_step)
        if spec.attacks.is_empty() {
            self.entities[idx].clear_combat();
            return;
        }
        let bi = match self.building_index(bid) {
            Some(bi) => bi,
            None => {
                // The building is gone (destroyed by someone else, or
                // a depleted farm retiring): the order is complete.
                self.entities[idx].clear_combat();
                return;
            }
        };
        let b = &self.buildings[bi];
        let (bx, by, size) = (b.pos_x, b.pos_y, b.kind.footprint_size());
        let (ax, ay) = (b.cell_x, b.cell_y);
        // Distance to the footprint rectangle (0 when beside it).
        let half = I16F16::from_num(size as i32 * 2);
        let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let qx = (px - bx).clamp(-half, half) + bx;
        let qy = (py - by).clamp(-half, half) + by;
        let dist = steer::vec_len(px - qx, py - qy);
        let in_range = if spec.range_max > I16F16::ZERO {
            dist <= spec.range_max
        } else {
            dist <= MELEE_REACH
        };

        if let Some(st) = self.entities[idx].strike_tick {
            if tick >= st {
                self.resolve_building_strike(idx, bi);
            }
        }
        // The strike may have destroyed the building (which clears
        // this attacker's target): nothing more to do this tick.
        if self.entities[idx].attack_building != Some(bid) {
            return;
        }

        if in_range {
            if self.entities[idx].order.is_some() {
                let e = &mut self.entities[idx];
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
            if spec.range_max == I16F16::ZERO {
                self.entities[idx].suppress_separation = true;
            }
            let ready = tick >= self.entities[idx].next_attack_tick;
            if self.entities[idx].strike_tick.is_none() && ready {
                if self.entities[idx].attack_engaged {
                    // The building record may have shifted since
                    // `bi` was read (another building destroyed
                    // earlier this phase): re-resolve by id.
                    if let Some(bi2) = self.building_index(bid) {
                        self.resolve_building_strike(idx, bi2);
                    }
                } else {
                    self.entities[idx].strike_tick = Some(tick + spec.attack_delay_ticks);
                }
            }
        } else {
            // Chase the nearest standable ring cell; re-path when the
            // current order no longer leads beside the footprint.
            let need = match &self.entities[idx].order {
                None => true,
                Some(o) => {
                    let (ox, oy) = (o.target_x, o.target_y);
                    let qx2 = (ox - bx).clamp(-half, half) + bx;
                    let qy2 = (oy - by).clamp(-half, half) + by;
                    steer::vec_len(ox - qx2, oy - qy2) > I16F16::from_num(6)
                }
            };
            if need {
                match self.building_approach_point(idx, ax, ay, size) {
                    Some((gx, gy)) => {
                        self.set_move_order(idx, gx, gy);
                    }
                    // No ring cell is reachable — the building is
                    // enclosed: the order cannot be carried out.
                    // (Probing again next tick would burn a
                    // find_path per ring cell per tick — M6 perf
                    // fix.)
                    None => self.entities[idx].clear_combat(),
                }
            }
        }
    }

    /// The standing spot for attacking the footprint anchored at
    /// `(ax, ay)`: a point STAND_OFF (1.0) outside the footprint's
    /// edge, on the edge-normal through a ring cell's center —
    /// never the ring cell's center itself. A ring *corner* cell's
    /// center sits ~2.83 from the edge on the diagonal, and
    /// steering's arrival shortfall (a waypoint counts as reached
    /// up to 1.0 early) stranded attackers up to ~3.8 from the
    /// edge — beyond MELEE_REACH — while combat re-issued the
    /// identical approach forever (probed: 40 000 ticks, zero
    /// damage). Aiming 1.0 off the edge absorbs the whole
    /// shortfall with margin to spare: arrival ends ≤ 2.0 from
    /// the edge from every direction, for every footprint size.
    ///
    /// The candidate nearest the *attacker* wins (ties break in
    /// ring order), so a unit no longer walks around the building
    /// to a fixed corner; a spot another living unit is already
    /// standing on is skipped while a free one paths, so a group
    /// spreads around the perimeter instead of piling onto one
    /// point — occupied spots remain as a fallback for a crowd
    /// larger than the perimeter. The first candidate (in that
    /// preference order) whose stand point is standable and
    /// reachable from the attacker wins; `None` when no ring spot
    /// is reachable at all (an enclosed building).
    fn building_approach_point(
        &self,
        idx: usize,
        ax: u32,
        ay: u32,
        size: u32,
    ) -> Option<(I16F16, I16F16)> {
        /// How far outside the footprint edge the attacker aims
        /// to stand (see the doc above).
        const STAND_OFF: I16F16 = I16F16::from_bits(65_536); // 1.0
        /// A stand point this close to another living unit's
        /// center counts as taken (units rest ~2.0 apart under
        /// separation): groups take the next spot along.
        const OCCUPIED: I16F16 = I16F16::from_bits(98_304); // 1.5

        let from = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let attacker_id = self.entities[idx].id;
        let (bx, by) = crate::build::footprint_center(ax, ay, size);
        let half = I16F16::from_num(size as i32 * 2);
        // (distance² to the attacker, ring order, stand point,
        // occupied) — sorted by the first two for a deterministic
        // preference order.
        let mut candidates: Vec<(i64, usize, I16F16, I16F16, bool)> = Vec::new();
        for (order, (cx, cy)) in crate::build::ring_cells(ax, ay, size).iter().enumerate() {
            let (cx, cy) = (*cx, *cy);
            if cx >= crate::GRID_DIM || cy >= crate::GRID_DIM {
                continue;
            }
            let center = self.grid.cell_center(cx, cy);
            // The edge point nearest this cell's center, and the
            // outward direction from it: axis-aligned for edge
            // cells, diagonal for corner cells.
            let ex = (center.0 - bx).clamp(-half, half) + bx;
            let ey = (center.1 - by).clamp(-half, half) + by;
            let (dx, dy) = (center.0 - ex, center.1 - ey);
            let len = steer::vec_len(dx, dy);
            if len == I16F16::ZERO {
                continue; // cannot happen: ring centers lie outside the rect
            }
            let stand = (ex + dx / len * STAND_OFF, ey + dy / len * STAND_OFF);
            let cell = self.grid.world_to_cell(stand.0, stand.1);
            if self.grid.is_blocked(cell.0, cell.1) {
                continue;
            }
            let occupied = self.entities.iter().any(|u| {
                u.id != attacker_id
                    && !u.dead
                    && u.kind.is_mobile()
                    && steer::vec_len(u.pos_x - stand.0, u.pos_y - stand.1) <= OCCUPIED
            });
            let d = steer::vec_len(stand.0 - from.0, stand.1 - from.1).to_bits() as i64;
            candidates.push((d * d, order, stand.0, stand.1, occupied));
        }
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        // Free spots first; occupied ones only when no free spot
        // paths (a crowd larger than the perimeter still closes
        // in behind the front rank).
        for want_occupied in [false, true] {
            for &(_, _, sx, sy, occupied) in &candidates {
                if occupied != want_occupied {
                    continue;
                }
                if self.grid.find_path(from, (sx, sy)).is_some() {
                    return Some((sx, sy));
                }
            }
        }
        None
    }

    /// Land one strike from attacker `idx` on building `bi` (M5):
    /// melee damage now (when still in reach of the footprint edge),
    /// or a projectile release aimed at the building's center for a
    /// ranged attacker — with the same release-time accuracy roll
    /// discipline (PRJ-03). Reload restarts from the landing tick.
    fn resolve_building_strike(&mut self, idx: usize, bi: usize) {
        let spec = self.unit_stats(idx); // M6: effective row (release snapshot)
        let attacker_id = self.entities[idx].id;
        let owner = self.entities[idx].owner;
        let (ax, ay) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let b = &self.buildings[bi];
        let (bx, by, bid, size) = (b.pos_x, b.pos_y, b.id, b.kind.footprint_size());
        if spec.range_max > I16F16::ZERO {
            let roll = self.rng.below(100);
            let mut impact = (bx, by);
            if roll >= spec.accuracy {
                let sb = spec.scatter.to_bits();
                let span = (2 * sb + 1) as u32;
                let dx = self.rng.below(span) as i32 - sb;
                let dy = self.rng.below(span) as i32 - sb;
                impact = (bx + I16F16::from_bits(dx), by + I16F16::from_bits(dy));
            }
            let dist_bits = steer::vec_len(bx - ax, by - ay).to_bits() as i64;
            let speed_bits = spec.projectile_speed.to_bits() as i64;
            let flight = ((dist_bits + speed_bits - 1) / speed_bits).max(1) as u64;
            self.projectiles.push(Projectile {
                spawn_x: ax,
                spawn_y: ay,
                impact_x: impact.0,
                impact_y: impact.1,
                spawn_tick: self.tick,
                impact_tick: self.tick + flight,
                intended: None,
                intended_building: Some(bid),
                attacker: attacker_id,
                owner,
                attacker_elev: self.entity_height(idx),
                attacks: spec.attacks.to_vec(),
            });
        } else {
            let half = I16F16::from_num(size as i32 * 2);
            let qx = (ax - bx).clamp(-half, half) + bx;
            let qy = (ay - by).clamp(-half, half) + by;
            if steer::vec_len(ax - qx, ay - qy) <= MELEE_REACH {
                let atk_elev = self.entity_height(idx);
                self.damage_building(bid, spec.attacks, atk_elev);
            }
        }
        let e = &mut self.entities[idx];
        e.strike_tick = None;
        e.next_attack_tick = self.tick + spec.reload_ticks;
        e.attack_engaged = true;
    }

    /// Apply one landed hit's damage to a placed building (M5) and
    /// destroy it when its HP falls below 1 (CMB-02's death rule,
    /// applied to buildings). Damage runs through the ordinary
    /// formula against a synthetic spec carrying the building table's
    /// armor and the All Buildings + Standard Buildings classes —
    /// so class bonuses (e.g. a future anti-building row) work
    /// against buildings with no special cases (CMB-01/03).
    pub(crate) fn damage_building(&mut self, bid: u32, attacks: &[(u8, i32)], atk_elev: u8) {
        let bi = match self.building_index(bid) {
            Some(bi) => bi,
            None => return,
        };
        let spec = building_spec(self.buildings[bi].kind);
        // M7: the elevation factor reads the building's own ground
        // (its footprint-center cell) against the attacker's level.
        let b = &self.buildings[bi];
        let tgt_elev = self.height_at_world(b.pos_x, b.pos_y);
        let dmg = compute_damage_elev(attacks, &spec, atk_elev, tgt_elev);
        self.buildings[bi].hp -= dmg;
        if self.buildings[bi].hp < I16F16::from_num(1) {
            self.destroy_building(bid);
        }
    }

    /// Remove a destroyed building (M5): free its footprint cells,
    /// retire a farm's node with it, put its builders and repairers
    /// back to idle
    /// (a *completed* building's loss leaves its queue and rally
    /// moot — the record is gone), and clear every attacker aimed at
    /// it. Cost is not refunded (no rule grants one).
    fn destroy_building(&mut self, bid: u32) {
        let bi = match self.building_index(bid) {
            Some(bi) => bi,
            None => return,
        };
        let b = self.buildings.remove(bi);
        if b.kind.blocks_footprint() {
            for (cx, cy) in b.footprint_cells() {
                self.grid.set_dynamic(cx, cy, false);
            }
        }
        if let Some(nid) = b.node_id {
            if let Some(ni) = self.node_index(nid) {
                self.nodes[ni].amount_left = I16F16::ZERO;
            }
        }
        for e in &mut self.entities {
            if e.attack_building == Some(bid) {
                e.clear_combat();
            }
            if e.task_building == Some(bid)
                && matches!(e.task, Task::ToBuild | Task::Building)
            {
                e.task = Task::Idle;
                e.task_building = None;
                e.task_dropoff = None;
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
            // Repairers of the fallen building (BLD-07) stand down
            // the same way: the repair target is gone.
            if e.task == (Task::Repair { building: bid }) {
                e.task = Task::Idle;
                e.task_dropoff = None;
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
        }
    }

    /// Projectile impacts, in spawn order (PRJ-01): the intended
    /// target takes the hit if it is still within its radius of the
    /// impact point; else the first non-friendly unit there takes a
    /// 50% mishit; else the arrow lands in the dirt.
    fn projectile_step(&mut self) {
        let mut i = 0;
        while i < self.projectiles.len() {
            if self.projectiles[i].impact_tick <= self.tick {
                let p = self.projectiles.remove(i);
                self.resolve_projectile(&p);
            } else {
                i += 1;
            }
        }
    }

    fn resolve_projectile(&mut self, p: &Projectile) {
        // A shot fired at a building (M5): it hits when the impact
        // point lands on (or within the slack of) the footprint;
        // buildings neither dodge nor move, so a surviving building
        // always takes its aimed shot. A destroyed building's shots
        // land in the dirt (no mishit scan — the aim was the site).
        if let Some(bid) = p.intended_building {
            if let Some(bi) = self.building_index(bid) {
                let b = &self.buildings[bi];
                let half =
                    I16F16::from_num(b.kind.footprint_size() as i32 * 2) + HIT_SLACK;
                if (p.impact_x - b.pos_x).abs() <= half
                    && (p.impact_y - b.pos_y).abs() <= half
                {
                    self.damage_building(bid, &p.attacks, p.attacker_elev);
                }
            }
            return;
        }
        if let Some(tid) = p.intended {
            if let Some(ti) = self.entity_index(tid) {
                if !self.entities[ti].dead {
                    let tspec = units::spec(self.entities[ti].kind);
                    let d = steer::vec_len(
                        self.entities[ti].pos_x - p.impact_x,
                        self.entities[ti].pos_y - p.impact_y,
                    );
                    if d <= tspec.radius + HIT_SLACK {
                        self.apply_hit(p.attacker, (p.spawn_x, p.spawn_y), &p.attacks, ti, false, p.attacker_elev);
                        return;
                    }
                }
            }
        }
        // Mishit scan, ascending id: no friendly fire (archer flag).
        for ti in 0..self.entities.len() {
            let e = &self.entities[ti];
            if e.dead || e.kind == EntityKind::TownCenter || e.owner == p.owner {
                continue;
            }
            let tspec = units::spec(e.kind);
            let d = steer::vec_len(e.pos_x - p.impact_x, e.pos_y - p.impact_y);
            if d <= tspec.radius + HIT_SLACK {
                self.apply_hit(p.attacker, (p.spawn_x, p.spawn_y), &p.attacks, ti, true, p.attacker_elev);
                return;
            }
        }
    }

    /// The death sweep (CMB-02/CMB-14): every entity below 1 HP dies,
    /// in ascending id order. Orders, tasks and carries vanish with
    /// the unit; animals leave a carcass node where they fell, and
    /// the villager credited with the kill starts harvesting it.
    fn death_sweep(&mut self) {
        for idx in 0..self.entities.len() {
            let e = &self.entities[idx];
            if e.dead || e.hp >= I16F16::from_num(1) {
                continue;
            }
            let (kind, px, py, killer) = (e.kind, e.pos_x, e.pos_y, e.last_hit_by);
            {
                let e = &mut self.entities[idx];
                e.dead = true;
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
                e.task = Task::Idle;
                e.task_node = None;
                e.task_building = None;
                e.task_dropoff = None;
                e.carry = I16F16::ZERO;
                e.carry_kind = None;
                e.clear_combat();
                e.aggro_target = None;
                e.returning_home = false;
            }
            let spec = units::spec(kind);
            if spec.carcass_food > 0 {
                let node_kind = if kind == EntityKind::Sheep {
                    NodeKind::CarcassHerd
                } else {
                    NodeKind::CarcassHunt
                };
                let nid = self.nodes.len() as u32;
                self.nodes.push(ResourceNode {
                    id: nid,
                    kind: node_kind,
                    pos_x: px,
                    pos_y: py,
                    amount_left: I16F16::from_num(spec.carcass_food),
                    decay_per_tick: I16F16::from_bits(spec.carcass_decay_bits),
                });
                // The killer starts harvesting its kill — the
                // standard hunt feel (sheep included, ECO-13).
                if let Some(kid) = killer {
                    if let Some(ki) = self.entity_index(kid) {
                        let k = &self.entities[ki];
                        if !k.dead && k.kind == EntityKind::Villager {
                            self.begin_gather(ki, nid);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes as cls;
    use crate::units::spec;

    #[test]
    fn damage_formula_matches_cmb_01() {
        // Militia (4 melee) vs Villager (0 melee armor) = 4.
        let militia = spec(EntityKind::Militia);
        let villager = spec(EntityKind::Villager);
        assert_eq!(compute_damage(militia.attacks, villager), 4);
        // Archer (4 pierce) vs Militia (1 pierce armor) = 3.
        let archer = spec(EntityKind::Archer);
        assert_eq!(compute_damage(archer.attacks, militia), 3);
        // Boar vs Scout: 7 melee + 3 cavalry bonus − 0 melee armor = 10.
        let boar = spec(EntityKind::Boar);
        let scout = spec(EntityKind::ScoutCavalry);
        assert_eq!(compute_damage(boar.attacks, scout), 10);
        // Boar vs Villager: no cavalry membership — bonus stays home.
        assert_eq!(compute_damage(boar.attacks, villager), 7);
        // Minimum damage 1: villager (3) vs a hypothetical 10-armor row.
        let mut armored = *villager;
        armored.armor_melee = 10;
        assert_eq!(compute_damage(villager.attacks, &armored), 1);
    }

    #[test]
    fn elevation_factor_matches_cmb_01() {
        // The factor itself: binary (MAP-06) — any difference of
        // ≥ 1 level gives the full ×1.25 / ×0.75.
        assert_eq!(elevation_factor(2, 2), I16F16::from_num(1));
        assert_eq!(elevation_factor(3, 2), I16F16::from_bits(81_920));
        assert_eq!(elevation_factor(4, 0), I16F16::from_bits(81_920));
        assert_eq!(elevation_factor(1, 2), I16F16::from_bits(49_152));
        assert_eq!(elevation_factor(0, 4), I16F16::from_bits(49_152));
        // Militia (4 melee) vs Villager (0 armor): Σ = 4.
        let militia = spec(EntityKind::Militia);
        let villager = spec(EntityKind::Villager);
        assert_eq!(
            compute_damage_elev(militia.attacks, villager, 2, 2),
            I16F16::from_num(4)
        );
        assert_eq!(
            compute_damage_elev(militia.attacks, villager, 3, 2),
            I16F16::from_num(5) // 4 × 1.25
        );
        assert_eq!(
            compute_damage_elev(militia.attacks, villager, 1, 2),
            I16F16::from_num(3) // 4 × 0.75
        );
        // Fractional results survive (HP is fractional): Archer
        // Σ = 4 − 1 = 3 vs Militia, uphill → 3.75.
        let archer = spec(EntityKind::Archer);
        assert_eq!(
            compute_damage_elev(archer.attacks, militia, 2, 1),
            I16F16::from_bits(245_760) // 3.75
        );
        // The minimum-1 clamp applies AFTER the factor (CMB-01):
        // villager 3 vs 2 armor → Σ = 1; downhill 0.75 → 1, uphill
        // 1.25 → 1.25 (not the pre-clamped 1 × 1.25 confusion — the
        // sum is what scales).
        let mut armored = *villager;
        armored.armor_melee = 2;
        assert_eq!(
            compute_damage_elev(villager.attacks, &armored, 0, 3),
            I16F16::from_num(1)
        );
        assert_eq!(
            compute_damage_elev(villager.attacks, &armored, 3, 0),
            I16F16::from_bits(81_920)
        );
        // No participating pair stays 0 at any elevation.
        let sheep = spec(EntityKind::Sheep);
        assert_eq!(
            compute_damage_elev(sheep.attacks, villager, 4, 0),
            I16F16::ZERO
        );
    }

    #[test]
    fn bonus_damage_requires_class_membership() {
        // The Archer's +3 applies only to Spearman-class members
        // (CMB-03): a synthetic spearman-masked row takes 4 + 3.
        let archer = spec(EntityKind::Archer);
        let mut spearman = *spec(EntityKind::Militia);
        spearman.classes = cls::mask(&[cls::INFANTRY, cls::SPEARMAN]);
        spearman.armor_pierce = 0;
        assert_eq!(compute_damage(archer.attacks, &spearman), 7);
        // …while the real Militia (no Spearman bit) takes only the 4,
        // minus its 1 pierce armor = 3 (checked above too).
        assert_eq!(compute_damage(archer.attacks, spec(EntityKind::Militia)), 3);
        // An attacker with no applicable pair at all deals 0 (sheep
        // have no attack row; the engine never calls this for them).
        let sheep = spec(EntityKind::Sheep);
        assert_eq!(compute_damage(sheep.attacks, spec(EntityKind::Villager)), 0);
    }
}
