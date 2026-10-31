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
//! §2.2), deer flight, and the wolf's hold → acquire → chase →
//! leash-return hunt (AI-08). Villagers never auto-attack and do
//! not flee (M5 AI polish); idle military units auto-acquire by
//! the CMB-08 chain (herdables and deer are never auto-targeted,
//! and wolves join that exclusion per AI-08's letter: a wolf is
//! Gaia, not an enemy, until combat starts), stance-gated since
//! CMB-09/10/11: No Attack never acquires, Stand Ground (the
//! starting Scout's Dark Age default) acquires only inside attack
//! range, Defensive chases only within its leash, and units on
//! attack-move or patrol acquire while walking.

use fixed::types::I16F16;

use crate::build::BuildingKind;
use crate::classes as cls;
use crate::econ::{NodeKind, ResourceNode};
use crate::entity::{
    is_player_owner, Entity, EntityKind, Task, OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER,
};
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
/// How far a Defensive unit (CMB-09) may stray from its post
/// while chasing an auto-acquired target before it breaks off
/// and returns: 12.0 world units = 3 tiles. TUNED (UNVERIFIED):
/// the wiki says only that a defensive unit "will follow for a
/// certain number of tiles before returning to their original
/// position" — no public source pins the number (the same
/// disclosure as `CAPTURE_RANGE` above). 3 tiles keeps the chase
/// inside the unit's own LoS (4 tiles) from its post.
pub const DEFENSIVE_LEASH: I16F16 = I16F16::from_bits(786_432); // 12.0
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
/// How close a non-Gaia unit must come to restart a deer's flee
/// window without touching it — AI-07's "or approaches" half, the
/// deer push (Study #3 §c.1). ~2 tiles. The wiki documents the
/// behaviour but gives no number (Hunt: "They flee when approached
/// by any non-Gaia unit and continue to do so if pursued"), so the
/// radius is a tuning constant (UNVERIFIED numerically).
const DEER_PUSH_RANGE: I16F16 = I16F16::from_bits(524_288); // 8.0
/// AI-08: a wolf breaks off the hunt when its prey is this far from
/// the wolf itself — half again its 12-tile line of sight, so prey
/// that has genuinely escaped its sight ends the chase (the AoM
/// predator rule the Predator article documents: predators stop
/// pursuing once the target is out of sight long enough and return
/// home). Adopted constant — the AoE II sources describe the lure
/// (a wolf follows a fleeing unit a long way) without a number.
const WOLF_GIVE_UP_CHASE: I16F16 = I16F16::from_bits(4_718_592); // 72.0 (18 tiles)
/// AI-08: …or when the prey is this far from the wolf's post —
/// the leash that keeps a hunt from crossing the map, twice its
/// line of sight. Adopted constant, same provenance as above.
const WOLF_LEASH: I16F16 = I16F16::from_bits(6_291_456); // 96.0 (24 tiles)

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
    /// The placed building that loosed this shot, when it came from
    /// a building weapon (BLD-18): `attacker` then reads the
    /// `BUILDING_ATTACKER_ID` sentinel (no entity loosed it) and
    /// this field names the shooter — used for nothing but
    /// provenance today; the kill credit rides `owner` into
    /// `Entity::last_hit_owner`.
    pub attacker_building: Option<u32>,
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
                // Castle P3c: the anti-building class reads the
                // target's building armor (Masonry's +3, the
                // Fortified Wall's 8 — DE's "building armor");
                // every other class still has no class armor.
                if c == cls::STANDARD_BUILDINGS {
                    target.armor_building
                } else {
                    0
                }
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
/// GAR-10 (Castle P4a): the attack table a Battering Ram
/// strikes a building with — its spec table with +10 per
/// passenger added to the buildings pair ONLY (the base
/// 2 melee and the +40 vs Siege never scale). Derived from
/// the passenger count at strike time (the GAR-05 lookup
/// pattern); every other attacker — and an empty Ram —
/// borrows its table unchanged.
pub fn ram_building_attacks(
    kind: EntityKind,
    passengers: usize,
    attacks: &[(u8, i32)],
) -> std::borrow::Cow<'_, [(u8, i32)]> {
    if kind != EntityKind::BatteringRam || passengers == 0 {
        return std::borrow::Cow::Borrowed(attacks);
    }
    let bonus = 10 * passengers as i32;
    std::borrow::Cow::Owned(
        attacks
            .iter()
            .map(|&(c, v)| {
                if c == cls::STANDARD_BUILDINGS {
                    (c, v + bonus)
                } else {
                    (c, v)
                }
            })
            .collect(),
    )
}

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
    compute_damage_hit(attacks, target, attacker_elev, target_elev, false)
}

/// CMB-01/CMB-13 in full: the documented DE composition composes
/// **all** multipliers — the elevation factor and the mishit ½
/// (k_miss) — and only then applies the minimum-damage rule:
/// `damage = max(1, k_elev × k_miss × Σ max(0, atk − armor))`
/// (wiki, *Attack*: the exact AoE2 formula; wiki, *Armor class*,
/// "Damage calculation" steps 5–6: "the minimum damage rule is
/// applied after all the attack multipliers" — both fetched
/// 2026-10-22). So a mishit whose pre-clamp value is 0.5 still
/// deals the minimum 1; the halving can never push a landed hit
/// below the minimum. A hit with no participating attack class
/// stays 0 — the minimum never *creates* damage.
pub fn compute_damage_hit(
    attacks: &[(u8, i32)],
    target: &UnitSpec,
    attacker_elev: u8,
    target_elev: u8,
    mishit: bool,
) -> I16F16 {
    let (total, participated) = damage_sum(attacks, target);
    if !participated {
        return I16F16::ZERO;
    }
    let mut dmg = I16F16::from_num(total) * elevation_factor(attacker_elev, target_elev);
    if mishit {
        dmg /= 2;
    }
    if dmg < I16F16::from_num(1) {
        I16F16::from_num(1)
    } else {
        dmg
    }
}

/// AI-08's prey test: the living player-owned mobile units a
/// wolf hunts on sight. The wiki's exception list (Wolf article):
/// Scout Cavalry, Monks, Trade Carts, and siege weapons are never
/// attacked on sight — of those, only the Scout exists here yet;
/// the rest join this test when their kinds land. Gaia's own are
/// never prey (wolves hunting wild animals was a cut feature, per
/// the same article's trivia), and herdables are not "units" for
/// this rule (a captured sheep is livestock — the CMB-08
/// exclusion's same reasoning). Being *attacked* overrides the
/// exceptions: retaliation is `apply_hit`'s Wolf arm, not this
/// test.
fn wolf_prey(t: &Entity) -> bool {
    is_player_owner(t.owner)
        && t.kind.is_mobile()
        && !matches!(t.kind, EntityKind::ScoutCavalry | EntityKind::Sheep)
}

/// The damage-table view of a placed building kind (M5): HP and
/// armor from the study's building table (`BuildingKind::max_hp` /
/// `armor_melee` / `armor_pierce`), classes All Buildings + Standard
/// Buildings (the same membership the Town Center entity carries in
/// the unit tables). Everything else is inert — a building never
/// moves, shoots, or trains through this view.
pub(crate) fn building_spec(kind: BuildingKind, owner_age: crate::tech::Age) -> UnitSpec {
    // BLD-13: wall segments additionally belong to the Walls and
    // Gates class (the reference's membership for the Palisade
    // Wall, on top of All/Standard Buildings).
    // Castle P3a: the Castle additionally belongs to the Castle
    // class — the membership its own anti-Castle bonus rows (the
    // Petard's +100) key on.
    let classes = if kind.is_wall() {
        cls::mask(&[
            cls::ALL_BUILDINGS,
            cls::STANDARD_BUILDINGS,
            cls::WALLS_AND_GATES,
        ])
    } else if kind == BuildingKind::Castle {
        cls::mask(&[cls::ALL_BUILDINGS, cls::STANDARD_BUILDINGS, cls::CASTLE])
    } else {
        cls::mask(&[cls::ALL_BUILDINGS, cls::STANDARD_BUILDINGS])
    };
    UnitSpec {
        max_hp: kind.max_hp_at(owner_age),
        attacks: &[],
        armor_melee: kind.armor_melee_at(owner_age),
        armor_pierce: kind.armor_pierce_at(owner_age),
        // Castle P3c: the BASE building armor (0 everywhere until
        // Masonry / Fortified Wall); `Sim::damage_building`
        // layers the owner's researched patch over this row via
        // `Sim::building_armor`, which also owns the patched
        // max HP — this pure row stays the unpatched table.
        armor_building: 0,
        classes,
        range_max: I16F16::ZERO,
        range_min: I16F16::ZERO,
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
        // BLD-18: buildings that shoot run after every entity
        // attacker, in building-id order (the buildings vec's
        // order) — RNG draws for accuracy happen here, after all
        // entity draws, keeping the phase's draw discipline total.
        self.building_weapon_step_all();
        // Castle P3b (GAR-06): Town Centers fire — only while
        // garrisoned — after the placed buildings, in TC order.
        self.tc_weapon_step_all();
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
                    || u.garrisoned_in.is_some()
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

    /// Gaia behavior: the boar's lure loop, deer flight, sheep
    /// wander, and the wolf's hunt. Runs before the attack machines
    /// so an animal's aggro becomes its attack target on the same
    /// tick.
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
                EntityKind::Wolf => self.wolf_ai(idx),
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

    /// The wolf's hunt (AI-08; wiki, Wolf + Predator): unlike the
    /// boar — which only retaliates — the wolf INITIATES. It holds
    /// its post until a prey unit enters its line of sight, then
    /// acquires the nearest one ("Wolves automatically attempt to
    /// attack the nearest unit"), chases, and attacks; when the
    /// prey dies it scans again from where it stands. A chase ends
    /// when the prey escapes the wolf's sight or drags it past its
    /// leash — then the wolf walks back to its post and holds
    /// again. It does not heal at the post (the wiki's regeneration
    /// notes belong to Scenario-Editor-only wolf variants).
    fn wolf_ai(&mut self, idx: usize) {
        // Validate the current prey.
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
            let ai = self.entity_index(a).expect("validated above");
            let (ax, ay) = (self.entities[ai].pos_x, self.entities[ai].pos_y);
            let (hx, hy) = (self.entities[idx].home_x, self.entities[idx].home_y);
            let d_chase = self.distance_to(idx, ax, ay);
            let d_home = steer::vec_len(ax - hx, ay - hy);
            if d_chase > WOLF_GIVE_UP_CHASE || d_home > WOLF_LEASH {
                let e = &mut self.entities[idx];
                e.aggro_target = None;
                e.clear_combat();
                e.returning_home = true;
                return;
            }
            let e = &mut self.entities[idx];
            e.attack_target = Some(a);
            e.returning_home = false;
            return;
        }
        // No prey: walk back to the post if a hunt dragged the
        // wolf away from it, then hold (and scan) there.
        if self.entities[idx].returning_home {
            let (hx, hy) = (self.entities[idx].home_x, self.entities[idx].home_y);
            if self.distance_to(idx, hx, hy) > HOME_ARRIVE {
                if self.entities[idx].order.is_none()
                    && !self.set_move_order(idx, hx, hy)
                {
                    // The post is unreachable (a building completed
                    // over it): hold here instead of re-pathing
                    // every tick (the boar's M6 perf fix, mirrored).
                    self.entities[idx].returning_home = false;
                }
                return;
            }
            let e = &mut self.entities[idx];
            e.returning_home = false;
            e.order = None;
            e.vel_x = I16F16::ZERO;
            e.vel_y = I16F16::ZERO;
        }
        // Acquire: the nearest prey inside the line of sight
        // (ties to the lowest id — the acquisition scan's rule).
        let (wx, wy) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let los = units::spec(EntityKind::Wolf).los;
        let mut best: Option<(i64, u32)> = None; // (distance² bits, id)
        for t in &self.entities {
            if t.dead || !wolf_prey(t) {
                continue;
            }
            let d = steer::vec_len(t.pos_x - wx, t.pos_y - wy);
            if d > los {
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
            e.aggro_target = Some(tid);
            e.attack_target = Some(tid);
            e.returning_home = false;
        }
    }

    /// Deer flight: while the flee timer runs, keep a move order away
    /// from where the attack came from (re-issued when it completes).
    fn deer_ai(&mut self, idx: usize) {
        self.deer_proximity_flee(idx);
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

    /// Proximity flight (AI-07, 2026-10-22 — Study #3's top finding):
    /// a deer also flees a unit that merely APPROACHES it, which is
    /// what makes pushing deer toward the Town Center with a scout
    /// work in DE (wiki, Hunt: regular huntables "flee when
    /// approached by any non-Gaia unit and continue to do so if
    /// pursued"; "units such as Scout Cavalry" herd them "in the
    /// direction of Town Centers and Mills"). The nearest mobile
    /// non-Gaia unit inside DEER_PUSH_RANGE restarts the same flee
    /// window the damage path sets, away from that unit's current
    /// position — no new state, no new pathing. Runs before the
    /// flee check so a pursuer staying on the deer's tail keeps
    /// refreshing the window (the push), while a unit that stops
    /// nearby lets the deer settle once it has stepped out of
    /// range. Buildings never scare deer (the Town Center is an
    /// entity, excluded by kind — deer are pushed TOWARD it), and
    /// Gaia animals never do (owner check), so boars and sheep are
    /// untouched by construction.
    fn deer_proximity_flee(&mut self, idx: usize) {
        let e = &self.entities[idx];
        let (px, py) = (e.pos_x, e.pos_y);
        let mut best: Option<(I16F16, I16F16, I16F16)> = None; // (dist, x, y)
        for u in &self.entities {
            if u.dead || u.owner == OWNER_GAIA || u.kind == EntityKind::TownCenter {
                continue;
            }
            let d = steer::vec_len(u.pos_x - px, u.pos_y - py);
            if d <= DEER_PUSH_RANGE && best.map_or(true, |(bd, _, _)| d < bd) {
                best = Some((d, u.pos_x, u.pos_y));
            }
        }
        if let Some((_, tx, ty)) = best {
            let e = &mut self.entities[idx];
            e.flee_until = self.tick + DEER_FLEE_TICKS;
            e.flee_from_x = tx;
            e.flee_from_y = ty;
        }
    }

    /// Auto-acquisition (CMB-08): idle military units of *any*
    /// player engage targets inside their sight by the rule's
    /// priority chain: (1) the closest attackable enemy *unit*;
    /// (2) if none, the enemy Town Center; (3) if none, the
    /// closest other enemy building (Farms are never auto-targeted;
    /// walls and gates do not exist yet and would be excluded the
    /// same way). In M3 the only enemies were boars; since M5 an
    /// opposing player's units qualify too. Steps (2)–(3) landed
    /// 2026-10-22 (before that, buildings were never auto-acquired
    /// and razing was always an explicit order). Herdables and deer
    /// are never auto-targeted (killing them is always an explicit
    /// order), and villagers never auto-attack.
    ///
    /// CMB-09/10/11 (2026-10-22): the chain is stance-gated. A
    /// No Attack unit never acquires; a Stand Ground unit acquires
    /// only inside its attack range (it will fight what reaches
    /// it, never ride out — the starting Scout's Dark Age default
    /// is Stand Ground, so it acquires only at point blank); other
    /// stances acquire inside LoS. A unit under an attack-move or
    /// patrol order acquires WHILE WALKING (that is the orders'
    /// whole point); a unit under any other move order does not.
    /// Every acquisition is marked `attack_auto` — the chase gates
    /// in the attack steps read the provenance.
    fn acquire_targets(&mut self) {
        for idx in 0..self.entities.len() {
            let e = &self.entities[idx];
            if e.dead
                || e.garrisoned_in.is_some()
                || !is_player_owner(e.owner)
                || !matches!(
                    e.kind,
                    EntityKind::Militia
                        | EntityKind::ManAtArms
                        | EntityKind::Archer
                        | EntityKind::ScoutCavalry
                )
                || e.attack_target.is_some()
                || e.attack_building.is_some()
                || e.stance == crate::Stance::NoAttack
                || (e.order.is_some() && e.attack_move.is_none() && e.patrol.is_empty())
            {
                continue;
            }
            let owner = e.owner;
            let spec = self.unit_stats(idx); // M6: the owner's effective LoS
            // CMB-09: Stand Ground's acquisition radius is its
            // attack range, not its LoS.
            let radius = if e.stance == crate::Stance::StandGround {
                if spec.range_max > I16F16::ZERO {
                    spec.range_max
                } else {
                    MELEE_REACH
                }
            } else {
                spec.los
            };
            let (px, py) = (e.pos_x, e.pos_y);
            let mut best: Option<(i64, u32)> = None; // (distance² bits, id)
            for t in &self.entities {
                if t.dead || t.id == e.id || t.garrisoned_in.is_some() {
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
                if d > radius {
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
                e.attack_auto = true;
                continue;
            }
            // CMB-08 step (2): no enemy unit in sight — the enemy
            // Town Center, if it stands inside this unit's LoS.
            // (The Town Center is an entity in our model; the unit
            // scan above deliberately passes it over so this step
            // can rank it below every unit.)
            let mut best_tc: Option<(i64, u32)> = None;
            for t in &self.entities {
                if t.dead
                    || t.kind != EntityKind::TownCenter
                    || !is_player_owner(t.owner)
                    || t.owner == owner
                {
                    continue;
                }
                let d = steer::vec_len(t.pos_x - px, t.pos_y - py);
                if d > radius {
                    continue;
                }
                let db = d.to_bits() as i64;
                let key = (db * db, t.id);
                if best_tc.map(|b| key < b).unwrap_or(true) {
                    best_tc = Some(key);
                }
            }
            if let Some((_, tid)) = best_tc {
                let e = &mut self.entities[idx];
                e.attack_target = Some(tid);
                e.attack_engaged = false;
                e.strike_tick = None;
                e.attack_auto = true;
                continue;
            }
            // CMB-08 step (3): other enemy buildings, closest first
            // (ties to the lowest building id). Farms are never
            // auto-targeted (the rule's exclusion); foundations
            // count — they are buildings, and the reference game
            // treats them as such.
            let mut best_b: Option<(i64, u32)> = None;
            for b in &self.buildings {
                if !is_player_owner(b.owner) || b.owner == owner {
                    continue;
                }
                if b.kind == BuildingKind::Farm {
                    continue;
                }
                let d = steer::vec_len(b.pos_x - px, b.pos_y - py);
                if d > radius {
                    continue;
                }
                let db = d.to_bits() as i64;
                let key = (db * db, b.id);
                if best_b.map(|x| key < x).unwrap_or(true) {
                    best_b = Some(key);
                }
            }
            if let Some((_, bid)) = best_b {
                let e = &mut self.entities[idx];
                e.attack_building = Some(bid);
                e.attack_engaged = false;
                e.strike_tick = None;
                e.attack_auto = true;
            }
        }
    }

    /// One attacker's tick: resolve a due strike, then stand-and-fight
    /// when in range or chase when not. Melee attackers in range hold
    /// their ground (order cleared) and suppress separation (MOV-04).
    fn attack_step(&mut self, idx: usize) {
        let tick = self.tick;
        if self.entities[idx].garrisoned_in.is_some() {
            return; // GAR-02: a garrisoned unit does not fight
        }
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
            Some(i) if !self.entities[i].dead && self.entities[i].garrisoned_in.is_none() => i,
            _ => {
                self.entities[idx].clear_combat();
                return;
            }
        };
        let (tx, ty) = (self.entities[ti].pos_x, self.entities[ti].pos_y);
        let dist = self.distance_to(idx, tx, ty);
        // PRJ-08: a ranged attacker's firing band is
        // [range_min, range_max] — inside range_min is the dead
        // zone, where it cannot fire at all (range_min is 0 for
        // every row but the Skirmisher's, so the band is the old
        // test for everything else).
        let in_range = if spec.range_max > I16F16::ZERO {
            dist <= spec.range_max && dist >= spec.range_min
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
        } else if spec.range_max > I16F16::ZERO && dist < spec.range_min {
            // PRJ-08: the target stands inside the dead zone — the
            // attacker backs away along the ray from the target
            // through itself, to a stand point near its maximum
            // range. (Near-max, not just past range_min: steering's
            // arrival slowdown inside SLOW_RADIUS would otherwise
            // leave the retreat creeping slower than a pursuer's
            // chase — the observable sequence is DE's either way:
            // no fire from inside the dead zone, fire from the
            // band, repeat.) The order refreshes only when it no
            // longer leads to a band point (the chase discipline
            // below). If no stand point paths, the engagement
            // cannot be fought from here — PRJ-08's "or not at all".
            let need = match &self.entities[idx].order {
                None => true,
                Some(o) => {
                    let d = steer::vec_len(o.target_x - tx, o.target_y - ty);
                    d < spec.range_min || d > spec.range_max
                }
            };
            if need {
                let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
                let (dx, dy) = (px - tx, py - ty);
                let len = steer::vec_len(dx, dy);
                let (ux, uy) = if len == I16F16::ZERO {
                    (I16F16::from_num(1), I16F16::ZERO)
                } else {
                    (dx / len, dy / len)
                };
                let near = spec.range_max - I16F16::from_bits(98_304); // max − 1.5
                let stand = if near > spec.range_min {
                    near
                } else {
                    spec.range_min + I16F16::from_bits(98_304)
                };
                if !self.set_move_order(idx, tx + ux * stand, ty + uy * stand) {
                    self.entities[idx].clear_combat();
                }
            }
        } else {
            // CMB-09: stance gates on AUTO-ACQUIRED targets only —
            // an explicit order (attack_auto false) is followed to
            // the death in every stance. Stand Ground never
            // chases: out of range ends the engagement. Defensive
            // breaks off at the leash and walks back to its post.
            if self.entities[idx].attack_auto && is_player_owner(self.entities[idx].owner) {
                match self.entities[idx].stance {
                    crate::Stance::StandGround => {
                        self.entities[idx].clear_combat();
                        return;
                    }
                    crate::Stance::Defensive => {
                        let e = &self.entities[idx];
                        let d = steer::vec_len(
                            e.pos_x - e.stance_anchor_x,
                            e.pos_y - e.stance_anchor_y,
                        );
                        if d >= DEFENSIVE_LEASH {
                            let (ax, ay) = (e.stance_anchor_x, e.stance_anchor_y);
                            self.entities[idx].clear_combat();
                            self.set_move_order(idx, ax, ay);
                            return;
                        }
                    }
                    _ => {}
                }
            }
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

    /// PRJ-05 (Castle P3c): the Ballistics aim point — the
    /// target's position led by its current velocity over the
    /// flight time. The flight time is the one the release
    /// already computes (distance from the shooter to the
    /// target's CURRENT position ÷ projectile speed, ceiling);
    /// the lead is one step of that flight at the target's
    /// standing velocity (`vel_x`/`vel_y`, world units per
    /// tick) — fixed-point throughout, no float. Without
    /// Ballistics this helper is never called and the aim stays
    /// the release-time position (PRJ-05's other half).
    pub(crate) fn ballistics_aim(
        sx: I16F16,
        sy: I16F16,
        tx: I16F16,
        ty: I16F16,
        vx: I16F16,
        vy: I16F16,
        projectile_speed: I16F16,
    ) -> (I16F16, I16F16) {
        let dist_bits = steer::vec_len(tx - sx, ty - sy).to_bits() as i64;
        let speed_bits = projectile_speed.to_bits() as i64;
        if speed_bits <= 0 {
            return (tx, ty);
        }
        let flight = ((dist_bits + speed_bits - 1) / speed_bits).max(1);
        (
            tx + I16F16::from_bits((vx.to_bits() as i64 * flight) as i32),
            ty + I16F16::from_bits((vy.to_bits() as i64 * flight) as i32),
        )
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
            // PRJ-08: a shot whose target has closed inside the
            // dead zone by the landing tick is never released —
            // the ranged mirror of the melee whiff below, with no
            // RNG draw (the release it would have fed never
            // happens); the reload still restarts and attack_step
            // repositions the shooter out of the dead zone.
            if steer::vec_len(tx - ax, ty - ay) >= spec.range_min {
                // PRJ-05 (Castle P3c): with Ballistics the aim
                // point is the target's position LED by its
                // current velocity over the flight time; without
                // it, the aim is where the target stands at
                // release — decided once, here, either way.
                let aim = if self.has_tech(owner, crate::tech::TechId::Ballistics) {
                    Self::ballistics_aim(
                        ax,
                        ay,
                        tx,
                        ty,
                        self.entities[ti].vel_x,
                        self.entities[ti].vel_y,
                        spec.projectile_speed,
                    )
                } else {
                    (tx, ty)
                };
                // Ranged release: the accuracy roll is drawn NOW, at
                // release, in combat-phase id order (PRJ-03/SIM-04).
                let roll = self.rng.below(100);
                let mut impact = aim;
                if roll >= spec.accuracy {
                    // Miss: scatter within the weapon's radius — two
                    // further draws, same fixed order (PRJ-04).
                    let sb = spec.scatter.to_bits();
                    let span = (2 * sb + 1) as u32;
                    let dx = self.rng.below(span) as i32 - sb;
                    let dy = self.rng.below(span) as i32 - sb;
                    impact = (aim.0 + I16F16::from_bits(dx), aim.1 + I16F16::from_bits(dy));
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
                    attacker_building: None,
                    owner,
                    // PRJ-10 (M7): the shooter's elevation is captured
                    // at release, like its attack table.
                    attacker_elev: self.entity_height(idx),
                    attacks: spec.attacks.to_vec(),
                });
            }
        } else {
            // Melee: lands only if the target is still in reach at
            // the landing tick (CMB-06); otherwise it whiffs.
            let dist = steer::vec_len(tx - ax, ty - ay);
            if dist <= MELEE_REACH {
                let atk_elev = self.entity_height(idx);
                // GAR-10 (Castle P4a): against the Town Center —
                // an entity in this architecture (BLD-16) but a
                // building for damage — the Ram's passenger bonus
                // applies exactly as against a placed building.
                let attacks = if self.entities[ti].kind == EntityKind::TownCenter {
                    ram_building_attacks(
                        self.entities[idx].kind,
                        self.entities[idx].garrison.len(),
                        spec.attacks,
                    )
                } else {
                    std::borrow::Cow::Borrowed(spec.attacks)
                };
                self.apply_hit(attacker_id, owner, (ax, ay), &attacks, ti, false, atk_elev);
                // CMB-17 (Castle P3a): a Petard's landed strike
                // detonates it at the target.
                if self.entities[idx].kind == EntityKind::Petard {
                    self.petard_detonate(idx, (tx, ty), spec.attacks, Some(ti), None);
                }
            }
        }
        let e = &mut self.entities[idx];
        e.strike_tick = None;
        e.next_attack_tick = self.tick + spec.reload_ticks;
        e.attack_engaged = true;
    }

    /// Apply one landed hit to entity `ti`: damage, kill credit, and
    /// the victim's reactions — a boar aggros its attacker (with the
    /// two-hit/villager lock), a wolf aggros any attacker, a deer
    /// flees, an idle military unit retaliates. `threat` is where
    /// the hit came from (the melee attacker's position, or a
    /// projectile's spawn point).
    #[allow(clippy::too_many_arguments)]
    fn apply_hit(
        &mut self,
        attacker_id: u32,
        attacker_owner: u8,
        threat: (I16F16, I16F16),
        attacks: &[(u8, i32)],
        ti: usize,
        mishit: bool,
        atk_elev: u8,
    ) {
        if self.entities[ti].garrisoned_in.is_some() {
            return; // GAR-02: garrisoned units cannot be damaged
        }
        // M6: damage reads the target's *effective* armor — Loom's
        // +1/+2 and the Blacksmith armor lines land here (CMB-01
        // against the patched row, TEC-06). M7: the elevation
        // factor compares the attacker's level (`atk_elev` — the
        // shooter's at release for a projectile, PRJ-10) against
        // the target's level where it stands now.
        let tspec = self.unit_stats(ti);
        let tgt_elev = self.entity_height(ti);
        // MAP-07 × CMB-01 (the working default the rulebook
        // declares for the sources' ambiguity): across a cliff
        // the downhill bonus applies but the uphill penalty does
        // not — an attacker striking *up* a cliff face fights at
        // factor 1, never ×0.75. "Across a cliff" is the hit's
        // own line: from the threat point (the melee attacker's
        // position, or a projectile's release point) to the
        // target's ground. Clamp the target's effective elevation
        // to the attacker's in exactly that case; every other
        // pairing reads its heights plainly.
        let eff_tgt_elev = if atk_elev < tgt_elev
            && self.grid.segment_crosses_cliff(
                threat,
                (self.entities[ti].pos_x, self.entities[ti].pos_y),
            ) {
            atk_elev
        } else {
            tgt_elev
        };
        // CMB-13 composition (verified 2026-10-22): the mishit ½ is
        // one of the multipliers INSIDE the minimum-1 clamp, not a
        // division after it — `compute_damage_hit` composes
        // elevation × mishit × Σ and clamps once, at the end.
        let dmg = compute_damage_hit(attacks, &tspec, atk_elev, eff_tgt_elev, mishit);
        {
            let t = &mut self.entities[ti];
            t.hp -= dmg;
            t.last_hit_by = Some(attacker_id);
            // BLD-18: the owner-level attribution the death sweep
            // credits — for entity attackers this is simply the
            // attacker's owner; for a building's arrow it is the
            // only attribution there is.
            t.last_hit_owner = Some(attacker_owner);
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
            EntityKind::Wolf => {
                // Retaliation (AI-08): a wolf answers ANY non-Gaia
                // attacker — the on-sight exception list (Scouts
                // and kin) governs who it hunts unprovoked, not
                // who it fights back against.
                let ai = self.entity_index(attacker_id);
                if let Some(ai) = ai {
                    let (aowner, adead) = (self.entities[ai].owner, self.entities[ai].dead);
                    if aowner != OWNER_GAIA && !adead {
                        let w = &mut self.entities[ti];
                        w.aggro_target = Some(attacker_id);
                        w.returning_home = false;
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
                // auto-attack — task rule.) CMB-09: the answer is
                // stance-gated — a No Attack unit never retaliates,
                // and a Stand Ground unit answers only an attacker
                // already inside its attack range (it will not ride
                // out to one). The target is marked auto-acquired:
                // the chase gates apply to it like any other.
                // A building's arrow has no entity to answer
                // (BLD-18): its `attacker_id` is the sentinel, no
                // entity index resolves, and the victim does not
                // retaliate — it has no building-attack order to
                // give itself here; the player (or AI) answers for
                // it, as in the reference.
                let t = &self.entities[ti];
                if is_player_owner(t.owner)
                    && t.attack_target.is_none()
                    && t.order.is_none()
                    && self.entity_index(attacker_id).is_some()
                {
                    let answers = match t.stance {
                        crate::Stance::NoAttack => false,
                        crate::Stance::StandGround => {
                            let range = if tspec.range_max > I16F16::ZERO {
                                tspec.range_max
                            } else {
                                MELEE_REACH
                            };
                            steer::vec_len(t.pos_x - threat.0, t.pos_y - threat.1) <= range
                        }
                        _ => true,
                    };
                    if answers {
                        let e = &mut self.entities[ti];
                        e.attack_target = Some(attacker_id);
                        e.attack_auto = true;
                    }
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
        let (bx, by) = (b.pos_x, b.pos_y);
        // Distance to the footprint rectangle (0 when beside it).
        // Package J: the rectangle is the building's own — a
        // gate's 4×1 span clamps per axis via `half_extents`.
        let (hx, hy) = b.half_extents();
        let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let qx = (px - bx).clamp(-hx, hx) + bx;
        let qy = (py - by).clamp(-hy, hy) + by;
        let dist = steer::vec_len(px - qx, py - qy);
        // PRJ-08: the firing band applies against buildings too —
        // the distance here is to the footprint's edge, so a
        // minimum-range attacker must stand off from the wall.
        let in_range = if spec.range_max > I16F16::ZERO {
            dist <= spec.range_max && dist >= spec.range_min
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
            // CMB-09: stance gates on auto-acquired targets, as in
            // attack_step — a Stand Ground unit never rides out to
            // a building, and a Defensive unit breaks off at its
            // leash and returns to post.
            if self.entities[idx].attack_auto && is_player_owner(self.entities[idx].owner) {
                match self.entities[idx].stance {
                    crate::Stance::StandGround => {
                        self.entities[idx].clear_combat();
                        return;
                    }
                    crate::Stance::Defensive => {
                        let e = &self.entities[idx];
                        let d = steer::vec_len(
                            e.pos_x - e.stance_anchor_x,
                            e.pos_y - e.stance_anchor_y,
                        );
                        if d >= DEFENSIVE_LEASH {
                            let (ax, ay) = (e.stance_anchor_x, e.stance_anchor_y);
                            self.entities[idx].clear_combat();
                            self.set_move_order(idx, ax, ay);
                            return;
                        }
                    }
                    _ => {}
                }
            }
            // Chase the nearest standable ring cell; re-path when the
            // current order no longer leads beside the footprint.
            // PRJ-08: a minimum-range attacker stands off at
            // range_min + margin from the edge instead of the
            // melee 1.0, and its order only still serves while the
            // point it leads to sits inside the firing band.
            let stand_off = if spec.range_min > I16F16::ZERO {
                spec.range_min + I16F16::from_bits(98_304) // +1.5 margin
            } else {
                I16F16::from_bits(65_536) // 1.0 — the melee stand-off
            };
            let need = match &self.entities[idx].order {
                None => true,
                Some(o) => {
                    let (ox, oy) = (o.target_x, o.target_y);
                    let qx2 = (ox - bx).clamp(-hx, hx) + bx;
                    let qy2 = (oy - by).clamp(-hy, hy) + by;
                    let od = steer::vec_len(ox - qx2, oy - qy2);
                    if spec.range_min > I16F16::ZERO {
                        od < spec.range_min || od > spec.range_max
                    } else {
                        od > I16F16::from_num(6)
                    }
                }
            };
            if need {
                let cells = self.buildings[bi].ring_cells();
                match self.building_approach_point(idx, &cells, (bx, by), hx, hy, stand_off) {
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
    /// `(ax, ay)`: a point `stand_off` (1.0 for melee and ordinary
    /// ranged attackers; range_min + margin for a minimum-range
    /// attacker, PRJ-08) outside the footprint's
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
        cells: &[(u32, u32)],
        center_pos: (I16F16, I16F16),
        hx: I16F16,
        hy: I16F16,
        stand_off: I16F16,
    ) -> Option<(I16F16, I16F16)> {
        /// A stand point this close to another living unit's
        /// center counts as taken (units rest ~2.0 apart under
        /// separation): groups take the next spot along.
        const OCCUPIED: I16F16 = I16F16::from_bits(98_304); // 1.5

        let from = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let attacker_id = self.entities[idx].id;
        // Package J: the ring cells, the footprint's center and
        // its per-axis half extents come from the building itself
        // (`ring_cells` / `half_extents`) — a gate's 4×1 span
        // rings and clamps as a rectangle, every other kind as
        // its square, exactly as before.
        let (bx, by) = center_pos;
        // (distance² to the attacker, ring order, stand point,
        // occupied) — sorted by the first two for a deterministic
        // preference order.
        let mut candidates: Vec<(i64, usize, I16F16, I16F16, bool)> = Vec::new();
        for (order, (cx, cy)) in cells.iter().enumerate() {
            let (cx, cy) = (*cx, *cy);
            if cx >= crate::GRID_DIM || cy >= crate::GRID_DIM {
                continue;
            }
            let center = self.grid.cell_center(cx, cy);
            // The edge point nearest this cell's center, and the
            // outward direction from it: axis-aligned for edge
            // cells, diagonal for corner cells.
            let ex = (center.0 - bx).clamp(-hx, hx) + bx;
            let ey = (center.1 - by).clamp(-hy, hy) + by;
            let (dx, dy) = (center.0 - ex, center.1 - ey);
            let len = steer::vec_len(dx, dy);
            if len == I16F16::ZERO {
                continue; // cannot happen: ring centers lie outside the rect
            }
            let stand = (ex + dx / len * stand_off, ey + dy / len * stand_off);
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
        let (bx, by, bid) = (b.pos_x, b.pos_y, b.id);
        // Package J: per-axis halves (a gate's span rectangle).
        let (hx, hy) = b.half_extents();
        if spec.range_max > I16F16::ZERO {
            // PRJ-08: no release from inside the dead zone (edge
            // distance below range_min) — no projectile, no RNG
            // draw, mirroring resolve_strike; attack_building_step
            // repositions the shooter to its stand-off.
            let qx = (ax - bx).clamp(-hx, hx) + bx;
            let qy = (ay - by).clamp(-hy, hy) + by;
            if steer::vec_len(ax - qx, ay - qy) >= spec.range_min {
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
                    attacker_building: None,
                    owner,
                    attacker_elev: self.entity_height(idx),
                    attacks: spec.attacks.to_vec(),
                });
            }
        } else {
            let qx = (ax - bx).clamp(-hx, hx) + bx;
            let qy = (ay - by).clamp(-hy, hy) + by;
            if steer::vec_len(ax - qx, ay - qy) <= MELEE_REACH {
                let atk_elev = self.entity_height(idx);
                let attacks = ram_building_attacks(
                    self.entities[idx].kind,
                    self.entities[idx].garrison.len(),
                    spec.attacks,
                );
                self.damage_building(bid, &attacks, atk_elev, owner);
                // CMB-17 (Castle P3a): a Petard's landed strike
                // detonates it at the footprint's edge point.
                if self.entities[idx].kind == EntityKind::Petard {
                    self.petard_detonate(idx, (qx, qy), spec.attacks, None, Some(bid));
                }
            }
        }
        let e = &mut self.entities[idx];
        e.strike_tick = None;
        e.next_attack_tick = self.tick + spec.reload_ticks;
        e.attack_engaged = true;
    }

    /// CMB-17 (Castle P3a): the Petard's detonation. Called from
    /// both strike resolutions the tick a Petard's melee strike
    /// LANDS (a whiffed strike — target out of reach at the
    /// landing tick, CMB-06 — does not detonate it). The primary
    /// target has already taken the strike itself; the blast then
    /// applies the same attack table to everyone else within
    /// `PETARD_BLAST_RADIUS` of the detonation point — every
    /// living entity (the Petard itself excluded) and every
    /// placed building whose footprint edge is inside the radius,
    /// friend or foe alike: the Petard is a blast-flagged weapon
    /// under CMB-12. Resolution order is the engine's: entities
    /// in ascending id order, then buildings in ascending id
    /// order; deaths resolve in the phase's death sweep as usual.
    /// Finally the Petard is consumed — its HP is set to 0 and
    /// the death sweep removes it this same tick.
    fn petard_detonate(
        &mut self,
        idx: usize,
        point: (I16F16, I16F16),
        attacks: &[(u8, i32)],
        primary_unit: Option<usize>,
        primary_building: Option<u32>,
    ) {
        let attacker_id = self.entities[idx].id;
        let owner = self.entities[idx].owner;
        let atk_elev = self.entity_height(idx);
        let radius = crate::units::PETARD_BLAST_RADIUS;
        // Entities in the blast (ascending id = vec order).
        let victims: Vec<usize> = (0..self.entities.len())
            .filter(|&vi| {
                vi != idx
                    && Some(vi) != primary_unit
                    && !self.entities[vi].dead
                    && self.entities[vi].garrisoned_in.is_none()
                    && steer::vec_len(
                        self.entities[vi].pos_x - point.0,
                        self.entities[vi].pos_y - point.1,
                    ) <= radius
            })
            .collect();
        for vi in victims {
            self.apply_hit(attacker_id, owner, point, attacks, vi, false, atk_elev);
        }
        // Buildings in the blast (ascending id = vec order):
        // distance is measured to the footprint's edge, the
        // building-geometry convention of the whole module.
        let bids: Vec<u32> = self
            .buildings
            .iter()
            .filter(|b| {
                Some(b.id) != primary_building && {
                    // Package J: per-axis halves (gate spans).
                    let (hx, hy) = b.half_extents();
                    let qx = (point.0 - b.pos_x).clamp(-hx, hx) + b.pos_x;
                    let qy = (point.1 - b.pos_y).clamp(-hy, hy) + b.pos_y;
                    steer::vec_len(point.0 - qx, point.1 - qy) <= radius
                }
            })
            .map(|b| b.id)
            .collect();
        for bid in bids {
            self.damage_building(bid, attacks, atk_elev, owner);
        }
        // Consumed by its own detonation.
        let e = &mut self.entities[idx];
        e.hp = I16F16::ZERO;
        e.clear_combat();
    }

    /// BLD-18: every armed building's tick, in building-id order.
    fn building_weapon_step_all(&mut self) {
        for bi in 0..self.buildings.len() {
            self.building_weapon_step(bi);
        }
    }

    /// Distance from a building's footprint edge to a point: the
    /// nearest point of the footprint's rectangle clamped against
    /// the point (PRJ-08's building geometry — the same math
    /// `attack_building_step` uses against a building target).
    fn footprint_edge_dist(bx: I16F16, by: I16F16, half: I16F16, px: I16F16, py: I16F16) -> I16F16 {
        let qx = (px - bx).clamp(-half, half) + bx;
        let qy = (py - by).clamp(-half, half) + by;
        steer::vec_len(px - qx, py - qy)
    }

    /// BLD-18: one armed building's tick — target validation,
    /// acquisition, and the CMB-04/05 delay/reload discipline,
    /// mirroring `attack_step`'s shape with three building-shaped
    /// differences, all deliberate:
    ///
    /// - RANGE IS MEASURED FROM THE FOOTPRINT'S EDGE (PRJ-08's
    ///   building band), for units and buildings alike; the target
    ///   building's own extent comes out of the measurement the
    ///   same way (rect-to-rect).
    /// - ACQUISITION follows the CMB-08 chain (closest enemy unit,
    ///   then the enemy Town Center, then the closest other enemy
    ///   building, Farms excluded) with ONE deviation: herdables
    ///   and huntables are never auto-targeted — a tower does not
    ///   spend arrows on sheep or deer; predators (boars) qualify,
    ///   as they do for units.
    /// - TARGETING IS VISIBILITY-INDEPENDENT, exactly like unit
    ///   acquisition (SIM-04's determinism): the sim has no fog
    ///   check for any attacker, and buildings invent none.
    ///
    /// Only a COMPLETED building fires — a foundation is inert
    /// timber and stone.
    fn building_weapon_step(&mut self, bi: usize) {
        let tick = self.tick;
        let b = &self.buildings[bi];
        if !b.complete {
            return;
        }
        let (owner, bx, by, size) = (b.owner, b.pos_x, b.pos_y, b.kind.footprint_size());
        let Some(w) = self.building_weapon(owner, b.kind) else {
            return;
        };
        let half = I16F16::from_num(size as i32 * 2);

        // --- validate the standing target -------------------------
        let mut target_unit = self.buildings[bi].weapon_target;
        let mut target_bld = self.buildings[bi].weapon_target_building;
        let mut edge_dist: Option<I16F16> = None;
        if let Some(tid) = target_unit {
            match self.entity_index(tid) {
                Some(ti) if !self.entities[ti].dead && self.entities[ti].garrisoned_in.is_none() => {
                    let t = &self.entities[ti];
                    let d = Self::footprint_edge_dist(bx, by, half, t.pos_x, t.pos_y);
                    if d > w.range_max {
                        target_unit = None;
                    } else {
                        edge_dist = Some(d);
                    }
                }
                _ => target_unit = None,
            }
        }
        if target_unit.is_none() {
            if let Some(tbid) = target_bld {
                match self.building_index(tbid) {
                    Some(tbi) => {
                        let tb = &self.buildings[tbi];
                        let thalf =
                            I16F16::from_num(tb.kind.footprint_size() as i32 * 2);
                        let dx = ((tb.pos_x - bx).abs() - (half + thalf)).max(I16F16::ZERO);
                        let dy = ((tb.pos_y - by).abs() - (half + thalf)).max(I16F16::ZERO);
                        let d = steer::vec_len(dx, dy);
                        if d > w.range_max {
                            target_bld = None;
                        } else {
                            edge_dist = Some(d);
                        }
                    }
                    None => target_bld = None,
                }
            }
        }
        if target_unit.is_none() && target_bld.is_none() {
            // The engagement ended (target died, left the band, or
            // the building was razed): the weapon resets, as an
            // entity's `clear_combat` does.
            if self.buildings[bi].weapon_target.is_some()
                || self.buildings[bi].weapon_target_building.is_some()
            {
                let b = &mut self.buildings[bi];
                b.weapon_target = None;
                b.weapon_target_building = None;
                b.weapon_strike_tick = None;
                b.weapon_engaged = false;
            }
            // --- acquire (the CMB-08 chain) ----------------------
            // 1. The closest enemy unit in the band — player units
            // of the other side, and predators. Herdables and
            // huntables never qualify (see the step docs).
            let mut best: Option<(i64, u32, I16F16)> = None;
            for e in &self.entities {
                if e.dead || e.kind == EntityKind::TownCenter || e.garrisoned_in.is_some() {
                    continue;
                }
                let hostile_unit = is_player_owner(e.owner) && e.owner != owner;
                if !hostile_unit && e.kind != EntityKind::Boar {
                    continue;
                }
                if e.kind == EntityKind::Sheep || e.kind == EntityKind::Deer {
                    continue;
                }
                let d = Self::footprint_edge_dist(bx, by, half, e.pos_x, e.pos_y);
                if d > w.range_max {
                    continue;
                }
                let key = d.to_bits() as i64 * (d.to_bits() as i64);
                let better = match best {
                    None => true,
                    Some((bd, bid2, _)) => (key, e.id) < (bd, bid2),
                };
                if better {
                    best = Some((key, e.id, d));
                }
            }
            if let Some((_, id, d)) = best {
                target_unit = Some(id);
                edge_dist = Some(d);
            } else {
                // 2. The enemy Town Center in the band.
                for e in &self.entities {
                    if e.dead || e.kind != EntityKind::TownCenter {
                        continue;
                    }
                    if !is_player_owner(e.owner) || e.owner == owner {
                        continue;
                    }
                    let d = Self::footprint_edge_dist(bx, by, half, e.pos_x, e.pos_y);
                    if d <= w.range_max {
                        target_unit = Some(e.id);
                        edge_dist = Some(d);
                        break;
                    }
                }
            }
            if target_unit.is_none() {
                // 3. The closest other enemy building in the band
                // (Farms excluded, as in CMB-08).
                let mut bestb: Option<(i64, u32, I16F16)> = None;
                for ob in &self.buildings {
                    if ob.owner == owner || ob.kind == BuildingKind::Farm {
                        continue;
                    }
                    let thalf = I16F16::from_num(ob.kind.footprint_size() as i32 * 2);
                    let dx = ((ob.pos_x - bx).abs() - (half + thalf)).max(I16F16::ZERO);
                    let dy = ((ob.pos_y - by).abs() - (half + thalf)).max(I16F16::ZERO);
                    let d = steer::vec_len(dx, dy);
                    if d > w.range_max {
                        continue;
                    }
                    let key = d.to_bits() as i64 * (d.to_bits() as i64);
                    let better = match bestb {
                        None => true,
                        Some((bd, bid2, _)) => (key, ob.id) < (bd, bid2),
                    };
                    if better {
                        bestb = Some((key, ob.id, d));
                    }
                }
                if let Some((_, id, d)) = bestb {
                    target_bld = Some(id);
                    edge_dist = Some(d);
                }
            }
            if target_unit.is_some() || target_bld.is_some() {
                let b = &mut self.buildings[bi];
                b.weapon_target = target_unit;
                b.weapon_target_building = target_bld;
            }
        }

        // --- the firing discipline (CMB-04/05, building form) ----
        let Some(d) = edge_dist else { return };
        if d < w.range_min {
            // The dead zone (PRJ-08): the target is held but cannot
            // be shot — the tower waits, exactly as a ranged unit
            // whose strike lands inside its minimum range.
            return;
        }
        let b = &self.buildings[bi];
        // GAR-05: the stored garrison arrow count overrides the
        // profile's base volley while it is set (towers/Castle).
        let volley = if b.garrison_arrows > 0 {
            b.garrison_arrows
        } else {
            w.volley
        };
        if let Some(st) = b.weapon_strike_tick {
            if tick >= st {
                self.building_weapon_release(bi, &w, volley);
            }
        } else if tick >= b.weapon_next_tick {
            if b.weapon_engaged {
                self.building_weapon_release(bi, &w, volley);
            } else {
                self.buildings[bi].weapon_strike_tick = Some(tick + w.attack_delay_ticks);
            }
        }
    }

    /// Loose one arrow from building `bi` at its standing target
    /// (BLD-18): the release mirrors `resolve_strike`'s ranged
    /// branch — the accuracy roll happens now (a miss still flies
    /// and lands scattered, PRJ-05), the flight time is
    /// distance/speed rounded up, and the reload restarts from this
    /// tick even when the shot is skipped because the target has
    /// slipped inside the minimum range (PRJ-08's resolve rule).
    fn building_weapon_release(
        &mut self,
        bi: usize,
        w: &crate::build::BuildingWeapon,
        volley: u32,
    ) {
        let tick = self.tick;
        let b = &self.buildings[bi];
        let (bid, owner, bx, by, size) = (b.id, b.owner, b.pos_x, b.pos_y, b.kind.footprint_size());
        let half = I16F16::from_num(size as i32 * 2);
        // Aim + band from the target as it stands NOW.
        let resolved = if let Some(tid) = b.weapon_target {
            self.entity_index(tid).and_then(|ti| {
                let t = &self.entities[ti];
                if t.dead || t.garrisoned_in.is_some() {
                    None
                } else {
                    Some((
                        (t.pos_x, t.pos_y),
                        Some(tid),
                        None,
                        Self::footprint_edge_dist(bx, by, half, t.pos_x, t.pos_y),
                    ))
                }
            })
        } else if let Some(tbid) = b.weapon_target_building {
            self.building_index(tbid).map(|tbi| {
                let tb = &self.buildings[tbi];
                let thalf = I16F16::from_num(tb.kind.footprint_size() as i32 * 2);
                let dx = ((tb.pos_x - bx).abs() - (half + thalf)).max(I16F16::ZERO);
                let dy = ((tb.pos_y - by).abs() - (half + thalf)).max(I16F16::ZERO);
                (
                    (tb.pos_x, tb.pos_y),
                    None,
                    Some(tbid),
                    steer::vec_len(dx, dy),
                )
            })
        } else {
            None
        };
        let Some((aim0, intended, intended_building, edge_d)) = resolved else {
            return;
        };
        {
            let b = &mut self.buildings[bi];
            b.weapon_strike_tick = None;
            b.weapon_next_tick = tick + w.reload_ticks;
            b.weapon_engaged = true;
        }
        if edge_d < w.range_min {
            return; // PRJ-08: inside the band — no shot at all.
        }
        // PRJ-05 (Castle P3c): Ballistics leads a moving UNIT
        // target for building weapons too (building targets do
        // not move — their aim stays the center).
        let aim = match intended {
            Some(tid) if self.has_tech(owner, crate::tech::TechId::Ballistics) => {
                match self.entity_index(tid) {
                    Some(ti) => Self::ballistics_aim(
                        bx,
                        by,
                        aim0.0,
                        aim0.1,
                        self.entities[ti].vel_x,
                        self.entities[ti].vel_y,
                        w.projectile_speed,
                    ),
                    None => aim0,
                }
            }
            _ => aim0,
        };
        // The accuracy roll (SIM-04: RNG draws happen only here, in
        // the combat phase, in attacker order — buildings draw
        // after every entity). Accuracy 100 draws nothing.
        let mut impact = aim;
        if w.accuracy < 100 {
            let roll = self.rng.below(100);
            if roll >= w.accuracy {
                let sb = w.scatter.to_bits();
                let span = (2 * sb + 1) as u32;
                let dx = self.rng.below(span) as i32 - sb;
                let dy = self.rng.below(span) as i32 - sb;
                impact = (aim.0 + I16F16::from_bits(dx), aim.1 + I16F16::from_bits(dy));
            }
        }
        let dist_bits = steer::vec_len(aim.0 - bx, aim.1 - by).to_bits() as i64;
        let speed_bits = w.projectile_speed.to_bits() as i64;
        let flight = ((dist_bits + speed_bits - 1) / speed_bits).max(1) as u64;
        // Castle P3a: the volley — `volley` projectiles share the
        // release's aim, flight time and attack snapshot (the
        // Castle's base 5; the tower's 1 makes this loop the old
        // single push). Accuracy 100 on both armed kinds draws no
        // per-projectile RNG, so SIM-04's draw discipline is
        // untouched; a future sub-100 building weapon would draw
        // per projectile here, in volley order.
        for _ in 0..volley.max(1) {
            self.projectiles.push(Projectile {
                spawn_x: bx,
                spawn_y: by,
                impact_x: impact.0,
                impact_y: impact.1,
                spawn_tick: tick,
                impact_tick: tick + flight,
                intended,
                intended_building,
                attacker: crate::build::BUILDING_ATTACKER_ID,
                attacker_building: Some(bid),
                owner,
                attacker_elev: self.height_at_world(bx, by),
                attacks: w.attacks.to_vec(),
            });
        }
    }

    /// GAR-06 (Castle P3b): every Town Center's weapon tick, in
    /// TC order — after the placed buildings' weapon steps.
    fn tc_weapon_step_all(&mut self) {
        let tc_ids: Vec<u32> = self.tc_states.iter().map(|s| s.tc_id).collect();
        for tc in tc_ids {
            self.tc_weapon_step(tc);
        }
    }

    /// GAR-06: one Town Center's weapon tick. The TC fires ONLY
    /// while it holds a garrison (GAR-06); its arrow count is the
    /// stored GAR-05 total (minimum one while garrisoned — the
    /// recompute guarantees it). Acquisition and the firing
    /// discipline mirror the placed buildings' step (CMB-08 chain,
    /// CMB-04/05 delay/reload), with range measured from the TC
    /// footprint's edge and no minimum range.
    fn tc_weapon_step(&mut self, tc: u32) {
        let tick = self.tick;
        let si = match self.tc_states.iter().position(|s| s.tc_id == tc) {
            Some(si) => si,
            None => return,
        };
        let ei = match self.entity_index(tc) {
            Some(ei) if !self.entities[ei].dead => ei,
            _ => return,
        };
        let (owner, bx, by) = {
            let e = &self.entities[ei];
            (e.owner, e.pos_x, e.pos_y)
        };
        let half = I16F16::from_num(8); // the TC's 4x4 footprint
        if self.tc_states[si].garrison.is_empty() {
            // No garrison, no fire — and no held engagement.
            let st = &mut self.tc_states[si];
            st.weapon_target = None;
            st.weapon_target_building = None;
            st.weapon_strike_tick = None;
            st.weapon_engaged = false;
            return;
        }
        let w = crate::TOWN_CENTER_WEAPON;

        // --- validate the standing target -------------------------
        let mut target_unit = self.tc_states[si].weapon_target;
        let mut target_bld = self.tc_states[si].weapon_target_building;
        let mut edge_dist: Option<I16F16> = None;
        if let Some(tid) = target_unit {
            match self.entity_index(tid) {
                Some(ti)
                    if !self.entities[ti].dead
                        && self.entities[ti].garrisoned_in.is_none() =>
                {
                    let t = &self.entities[ti];
                    let d = Self::footprint_edge_dist(bx, by, half, t.pos_x, t.pos_y);
                    if d > w.range_max {
                        target_unit = None;
                    } else {
                        edge_dist = Some(d);
                    }
                }
                _ => target_unit = None,
            }
        }
        if target_unit.is_none() {
            if let Some(tbid) = target_bld {
                match self.building_index(tbid) {
                    Some(tbi) => {
                        let tb = &self.buildings[tbi];
                        let thalf =
                            I16F16::from_num(tb.kind.footprint_size() as i32 * 2);
                        let dx = ((tb.pos_x - bx).abs() - (half + thalf)).max(I16F16::ZERO);
                        let dy = ((tb.pos_y - by).abs() - (half + thalf)).max(I16F16::ZERO);
                        let d = steer::vec_len(dx, dy);
                        if d > w.range_max {
                            target_bld = None;
                        } else {
                            edge_dist = Some(d);
                        }
                    }
                    None => target_bld = None,
                }
            }
        }
        if target_unit.is_none() && target_bld.is_none() {
            if self.tc_states[si].weapon_target.is_some()
                || self.tc_states[si].weapon_target_building.is_some()
            {
                let st = &mut self.tc_states[si];
                st.weapon_target = None;
                st.weapon_target_building = None;
                st.weapon_strike_tick = None;
                st.weapon_engaged = false;
            }
            // --- acquire (the CMB-08 chain, as for towers) -------
            let mut best: Option<(i64, u32, I16F16)> = None;
            for e in &self.entities {
                if e.dead || e.kind == EntityKind::TownCenter || e.garrisoned_in.is_some() {
                    continue;
                }
                let hostile_unit = is_player_owner(e.owner) && e.owner != owner;
                if !hostile_unit && e.kind != EntityKind::Boar {
                    continue;
                }
                if e.kind == EntityKind::Sheep || e.kind == EntityKind::Deer {
                    continue;
                }
                let d = Self::footprint_edge_dist(bx, by, half, e.pos_x, e.pos_y);
                if d > w.range_max {
                    continue;
                }
                let key = d.to_bits() as i64 * (d.to_bits() as i64);
                let better = match best {
                    None => true,
                    Some((bd, bid2, _)) => (key, e.id) < (bd, bid2),
                };
                if better {
                    best = Some((key, e.id, d));
                }
            }
            if let Some((_, id, d)) = best {
                target_unit = Some(id);
                edge_dist = Some(d);
            } else {
                for e in &self.entities {
                    if e.dead || e.kind != EntityKind::TownCenter {
                        continue;
                    }
                    if !is_player_owner(e.owner) || e.owner == owner {
                        continue;
                    }
                    let d = Self::footprint_edge_dist(bx, by, half, e.pos_x, e.pos_y);
                    if d <= w.range_max {
                        target_unit = Some(e.id);
                        edge_dist = Some(d);
                        break;
                    }
                }
            }
            if target_unit.is_none() {
                let mut bestb: Option<(i64, u32, I16F16)> = None;
                for ob in &self.buildings {
                    if ob.owner == owner || ob.kind == BuildingKind::Farm {
                        continue;
                    }
                    let thalf = I16F16::from_num(ob.kind.footprint_size() as i32 * 2);
                    let dx = ((ob.pos_x - bx).abs() - (half + thalf)).max(I16F16::ZERO);
                    let dy = ((ob.pos_y - by).abs() - (half + thalf)).max(I16F16::ZERO);
                    let d = steer::vec_len(dx, dy);
                    if d > w.range_max {
                        continue;
                    }
                    let key = d.to_bits() as i64 * (d.to_bits() as i64);
                    let better = match bestb {
                        None => true,
                        Some((bd, bid2, _)) => (key, ob.id) < (bd, bid2),
                    };
                    if better {
                        bestb = Some((key, ob.id, d));
                    }
                }
                if let Some((_, id, d)) = bestb {
                    target_bld = Some(id);
                    edge_dist = Some(d);
                }
            }
            if target_unit.is_some() || target_bld.is_some() {
                let st = &mut self.tc_states[si];
                st.weapon_target = target_unit;
                st.weapon_target_building = target_bld;
            }
        }

        // --- the firing discipline (CMB-04/05) --------------------
        let Some(_d) = edge_dist else { return };
        let st = &self.tc_states[si];
        if let Some(stk) = st.weapon_strike_tick {
            if tick >= stk {
                self.tc_weapon_release(tc, &w);
            }
        } else if tick >= st.weapon_next_tick {
            if st.weapon_engaged {
                self.tc_weapon_release(tc, &w);
            } else {
                self.tc_states[si].weapon_strike_tick = Some(tick + w.attack_delay_ticks);
            }
        }
    }

    /// Loose the Town Center's garrison arrows at its standing
    /// target (GAR-06): the GAR-05 stored count of ordinary
    /// projectiles, attributed to the TC's owner with no shooting
    /// building (the TC is an entity; `attacker_building` stays
    /// None and the kill credit rides the projectile's owner).
    fn tc_weapon_release(&mut self, tc: u32, w: &crate::build::BuildingWeapon) {
        let tick = self.tick;
        let si = match self.tc_states.iter().position(|s| s.tc_id == tc) {
            Some(si) => si,
            None => return,
        };
        let ei = match self.entity_index(tc) {
            Some(ei) if !self.entities[ei].dead => ei,
            _ => return,
        };
        let (owner, bx, by) = {
            let e = &self.entities[ei];
            (e.owner, e.pos_x, e.pos_y)
        };
        let half = I16F16::from_num(8);
        let st = &self.tc_states[si];
        let resolved = if let Some(tid) = st.weapon_target {
            self.entity_index(tid).and_then(|ti| {
                let t = &self.entities[ti];
                if t.dead || t.garrisoned_in.is_some() {
                    None
                } else {
                    Some(((t.pos_x, t.pos_y), Some(tid), None))
                }
            })
        } else if let Some(tbid) = st.weapon_target_building {
            self.building_index(tbid).map(|tbi| {
                let tb = &self.buildings[tbi];
                ((tb.pos_x, tb.pos_y), None, Some(tbid))
            })
        } else {
            None
        };
        let Some((aim0, intended, intended_building)) = resolved else {
            return;
        };
        // PRJ-05 (Castle P3c): the Town Center's garrison arrows
        // lead a moving unit target under Ballistics, like every
        // other ranged release.
        let aim = match intended {
            Some(tid) if self.has_tech(owner, crate::tech::TechId::Ballistics) => {
                match self.entity_index(tid) {
                    Some(ti) => Self::ballistics_aim(
                        bx,
                        by,
                        aim0.0,
                        aim0.1,
                        self.entities[ti].vel_x,
                        self.entities[ti].vel_y,
                        w.projectile_speed,
                    ),
                    None => aim0,
                }
            }
            _ => aim0,
        };
        let arrows = self.tc_states[si].garrison_arrows.max(1);
        {
            let st = &mut self.tc_states[si];
            st.weapon_strike_tick = None;
            st.weapon_next_tick = tick + w.reload_ticks;
            st.weapon_engaged = true;
        }
        let _ = half;
        // Accuracy 100 (the TC profile): no RNG draw, as with the
        // armed buildings.
        let dist_bits = steer::vec_len(aim.0 - bx, aim.1 - by).to_bits() as i64;
        let speed_bits = w.projectile_speed.to_bits() as i64;
        let flight = ((dist_bits + speed_bits - 1) / speed_bits).max(1) as u64;
        for _ in 0..arrows {
            self.projectiles.push(Projectile {
                spawn_x: bx,
                spawn_y: by,
                impact_x: aim.0,
                impact_y: aim.1,
                spawn_tick: tick,
                impact_tick: tick + flight,
                intended,
                intended_building,
                attacker: crate::build::BUILDING_ATTACKER_ID,
                attacker_building: None,
                owner,
                attacker_elev: self.height_at_world(bx, by),
                attacks: w.attacks.to_vec(),
            });
        }
    }

    /// Apply one landed hit's damage to a placed building (M5) and
    /// destroy it when its HP falls below 1 (CMB-02's death rule,
    /// applied to buildings). Damage runs through the ordinary
    /// formula against a synthetic spec carrying the building table's
    /// armor and the All Buildings + Standard Buildings classes —
    /// so class bonuses (e.g. a future anti-building row) work
    /// against buildings with no special cases (CMB-01/03).
    pub(crate) fn damage_building(
        &mut self,
        bid: u32,
        attacks: &[(u8, i32)],
        atk_elev: u8,
        attacker_owner: u8,
    ) {
        let bi = match self.building_index(bid) {
            Some(bi) => bi,
            None => return,
        };
        let mut spec = building_spec(
            self.buildings[bi].kind,
            self.age_of(self.buildings[bi].owner),
        );
        // Castle P3c: layer the owner's University patches over
        // the table row (Masonry +1/+1/+3, the Fortified Wall's
        // 12/12/+8 and 3000 HP — `Sim::building_armor` /
        // `building_max_hp` own the layering).
        {
            let b = &self.buildings[bi];
            let (melee, pierce, building) = self.building_armor(b.kind, b.owner);
            spec.armor_melee = melee;
            spec.armor_pierce = pierce;
            spec.armor_building = building;
            spec.max_hp = self.building_max_hp(b.kind, b.owner);
            // BLD-20's foundation half (DE update 42848): a wall
            // FOUNDATION — palisade or stone — has melee armor 0;
            // its standing armor arrives with completion. (This
            // package's scope is the wall family, the two kinds
            // the rulebook's BLD-13/BLD-20 notes record.)
            if !b.complete && b.kind.is_wall() {
                spec.armor_melee = 0;
            }
        }
        // M7: the elevation factor reads the building's own ground
        // (its footprint-center cell) against the attacker's level.
        let b = &self.buildings[bi];
        let tgt_elev = self.height_at_world(b.pos_x, b.pos_y);
        let dmg = compute_damage_elev(attacks, &spec, atk_elev, tgt_elev);
        self.buildings[bi].hp -= dmg;
        if self.buildings[bi].hp < I16F16::from_num(1) {
            // VIC-04: the lethal hit credits the attacker's owner
            // with the building's resource cost (wood + stone —
            // BLD-18's kinds are the first to carry a stone cost;
            // every earlier kind's stone reads 0, so their credit
            // is unchanged), foundation or complete. A Delete never
            // reaches this path (it calls `destroy_building`
            // directly), so self-demolition credits no one.
            let (bowner, bvalue) = {
                let b = &self.buildings[bi];
                (b.owner, b.kind.total_cost().to_num::<i32>() as i64)
            };
            self.credit_destruction(attacker_owner, bowner, bvalue);
            self.destroy_building(bid);
        }
    }

    /// Remove a destroyed building (M5): free its footprint cells,
    /// retire a farm's node with it, put its builders and repairers
    /// back to idle
    /// (a *completed* building's loss leaves its queue and rally
    /// moot — the record is gone), and clear every attacker aimed at
    /// it. Cost is not refunded (no rule grants one).
    pub(crate) fn destroy_building(&mut self, bid: u32) {
        let bi = match self.building_index(bid) {
            Some(bi) => bi,
            None => return,
        };
        // GAR-03 (Castle P3b): the garrison ejects unharmed beside
        // the building before the record is removed.
        self.eject_host(crate::entity::GarrisonRef::Building(bid));
        let b = self.buildings.remove(bi);
        if b.kind.blocks_footprint() {
            for (cx, cy) in b.footprint_cells() {
                self.grid.set_dynamic(cx, cy, false);
            }
        }
        // Package J: a gate's block is owned by the gate refresh,
        // not by `blocks_footprint` — with the record gone the
        // refresh can no longer see the span, so destruction
        // frees its cells explicitly, whatever state it died in.
        if b.kind.is_gate() {
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
                // Package J: per-axis halves (gate spans) + slack.
                let (hx, hy) = b.half_extents();
                if (p.impact_x - b.pos_x).abs() <= hx + HIT_SLACK
                    && (p.impact_y - b.pos_y).abs() <= hy + HIT_SLACK
                {
                    self.damage_building(bid, &p.attacks, p.attacker_elev, p.owner);
                }
            }
            return;
        }
        if let Some(tid) = p.intended {
            if let Some(ti) = self.entity_index(tid) {
                if !self.entities[ti].dead && self.entities[ti].garrisoned_in.is_none() {
                    let tspec = units::spec(self.entities[ti].kind);
                    let d = steer::vec_len(
                        self.entities[ti].pos_x - p.impact_x,
                        self.entities[ti].pos_y - p.impact_y,
                    );
                    if d <= tspec.radius + HIT_SLACK {
                        self.apply_hit(p.attacker, p.owner, (p.spawn_x, p.spawn_y), &p.attacks, ti, false, p.attacker_elev);
                        return;
                    }
                }
            }
        }
        // Mishit scan, ascending id: no friendly fire (archer flag).
        for ti in 0..self.entities.len() {
            let e = &self.entities[ti];
            if e.dead
                || e.kind == EntityKind::TownCenter
                || e.owner == p.owner
                || e.garrisoned_in.is_some()
            {
                continue;
            }
            let tspec = units::spec(e.kind);
            let d = steer::vec_len(e.pos_x - p.impact_x, e.pos_y - p.impact_y);
            if d <= tspec.radius + HIT_SLACK {
                self.apply_hit(p.attacker, p.owner, (p.spawn_x, p.spawn_y), &p.attacks, ti, true, p.attacker_elev);
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
            // GAR-03 (Castle P3b): a dying Town Center ejects its
            // garrison unharmed beside it, like a destroyed building
            // (before the immutable borrow below is taken).
            if !self.entities[idx].dead
                && self.entities[idx].hp < I16F16::from_num(1)
                && self.entities[idx].kind == EntityKind::TownCenter
            {
                let tc_id = self.entities[idx].id;
                self.eject_host(crate::entity::GarrisonRef::TownCenter(tc_id));
            }
            // GAR-10 (Castle P4a): a dying Battering Ram ejects
            // its passengers ALIVE at the ring around its final
            // position, before the corpse is marked — the mobile
            // host's destruction ejection (the only forced
            // ejection a Ram ever performs). The host record is
            // still alive at this point of the sweep, so the exit
            // ring reads its position normally.
            if !self.entities[idx].dead
                && self.entities[idx].hp < I16F16::from_num(1)
                && self.entities[idx].kind == EntityKind::BatteringRam
                && !self.entities[idx].garrison.is_empty()
            {
                let ram_id = self.entities[idx].id;
                self.eject_host(crate::entity::GarrisonRef::Unit(ram_id));
            }
            let e = &self.entities[idx];
            if e.dead || e.hp >= I16F16::from_num(1) {
                continue;
            }
            let (kind, px, py, killer) = (e.kind, e.pos_x, e.pos_y, e.last_hit_by);
            let killer_owner = e.last_hit_owner;
            let victim_owner = e.owner;
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
            // VIC-04: the last hitter is credited with the kill —
            // the same attribution the carcass rule below uses.
            // Cross-player kills only: `credit_destruction` sorts
            // out Gaia killers (a boar earns nobody anything),
            // same-owner kills (your own sheep on your own
            // villager's knife) and cost-free victims; a Delete
            // arrives here with `last_hit_by` already cleared, so
            // it credits no one.
            // BLD-18: the credit resolves through the hit's owner —
            // for entity attackers that is exactly the killer-entity
            // lookup this replaces; for a building's arrow there is
            // no killer entity (the sentinel), and the tower's owner
            // is credited. The carcass rule below still resolves the
            // killer *entity* — a building never harvests.
            if let Some(kowner) = killer_owner {
                self.credit_destruction(kowner, victim_owner, crate::unit_cost_value(kind));
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

    // CMB-01/02/13 regression (Codex review follow-up, 2026-10-22):
    // the documented DE order composes ALL multipliers — elevation
    // AND the mishit ½ (k_miss) — BEFORE the minimum-1 clamp:
    // damage = max(1, k_elev × k_miss × Σ). Sources: wiki, Attack
    // (the exact AoE2 formula) and wiki, Armor class (Damage
    // calculation steps 5–6: "the minimum damage rule is applied
    // after all the attack multipliers"), both fetched 2026-10-22.
    // Staged through `apply_hit` on a sandbox sim so the tests pin
    // the behavior end-to-end, not just the pure formula.
    fn sandbox_pair(sim: &Sim) -> (u32, u8, usize, usize) {
        // (attacker id, attacker owner, a villager index, the scout
        // index) from the standard sandbox start.
        let mut vil = None;
        let mut scout = None;
        for (i, e) in sim.entities.iter().enumerate() {
            match e.kind {
                EntityKind::Villager if vil.is_none() => vil = Some(i),
                EntityKind::ScoutCavalry if scout.is_none() => scout = Some(i),
                _ => {}
            }
        }
        let scout = scout.expect("sandbox start has a scout");
        (sim.entities[scout].id, sim.entities[scout].owner, vil.expect("sandbox start has villagers"), scout)
    }

    #[test]
    fn mishit_at_full_one_deals_one_not_a_half() {
        // THE flagged case: Σ = 1 on flat ground. A direct hit deals
        // 1 (the clamp); a mishit must also deal 1 — the halving
        // happens before the clamp, so it can never push a landed
        // hit below the minimum.
        let mut sim = Sim::new(5);
        let (atk, atk_owner, vil, _) = sandbox_pair(&sim);
        let h = sim.entity_height(vil);
        let pos = (sim.entities[vil].pos_x, sim.entities[vil].pos_y);
        sim.entities[vil].hp = I16F16::from_num(10);
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 1)], vil, true, h);
        assert_eq!(sim.entities[vil].hp, I16F16::from_num(9));
        // Control: the direct hit deals the same 1.
        sim.entities[vil].hp = I16F16::from_num(10);
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 1)], vil, false, h);
        assert_eq!(sim.entities[vil].hp, I16F16::from_num(9));
        // Above the clamp band the halving is untouched: Σ = 4
        // mishit deals exactly 2, Σ = 3 exactly 1.5.
        sim.entities[vil].hp = I16F16::from_num(10);
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 4)], vil, true, h);
        assert_eq!(sim.entities[vil].hp, I16F16::from_num(8));
        sim.entities[vil].hp = I16F16::from_num(10);
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 3)], vil, true, h);
        assert_eq!(sim.entities[vil].hp, I16F16::from_bits(557_056)); // 8.5
    }

    #[test]
    fn mishit_against_full_armor_still_deals_the_minimum() {
        // High armor: pierce 2 into the Scout Cavalry's 2 pierce
        // armor → Σ = 0 with a participating pair. DE: max(1, ½ × 0)
        // = 1 — a landed mishit never deals 0.5 (or 0).
        let mut sim = Sim::new(5);
        let (atk, atk_owner, _, scout) = sandbox_pair(&sim);
        let h = sim.entity_height(scout);
        let pos = (sim.entities[scout].pos_x, sim.entities[scout].pos_y);
        sim.entities[scout].hp = I16F16::from_num(100);
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 2)], scout, true, h);
        assert_eq!(sim.entities[scout].hp, I16F16::from_num(99));
        // Control: the direct hit against the same armor is the
        // clamped 1 as well.
        sim.entities[scout].hp = I16F16::from_num(100);
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 2)], scout, false, h);
        assert_eq!(sim.entities[scout].hp, I16F16::from_num(99));
    }

    #[test]
    fn mishit_composes_with_elevation_before_the_clamp() {
        // Σ = 1 uphill: product 1.25, mishit 0.625 → clamped to 1.
        // Σ = 4 uphill: product 5, mishit exactly 2.5 (no clamp).
        let mut sim = Sim::new(5);
        let (atk, atk_owner, vil, _) = sandbox_pair(&sim);
        let h = sim.entity_height(vil);
        let pos = (sim.entities[vil].pos_x, sim.entities[vil].pos_y);
        sim.entities[vil].hp = I16F16::from_num(10);
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 1)], vil, true, h + 1);
        assert_eq!(sim.entities[vil].hp, I16F16::from_num(9));
        sim.entities[vil].hp = I16F16::from_num(10);
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 4)], vil, true, h + 1);
        assert_eq!(sim.entities[vil].hp, I16F16::from_bits(491_520)); // 7.5
    }

    #[test]
    fn fractional_hp_and_death_boundaries_under_mishits() {
        // CMB-02 interplay: death is HP < 1, and mishit damage is
        // fractional — the two must compose at the boundary.
        let mut sim = Sim::new(5);
        let (atk, atk_owner, vil, _) = sandbox_pair(&sim);
        let h = sim.entity_height(vil);
        let pos = (sim.entities[vil].pos_x, sim.entities[vil].pos_y);
        // 2.5 HP − 1.5 (mishit of Σ = 3) = exactly 1.0 → survives.
        sim.entities[vil].hp = I16F16::from_bits(163_840); // 2.5
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 3)], vil, true, h);
        assert_eq!(sim.entities[vil].hp, I16F16::from_num(1));
        for _ in 0..5 {
            sim.tick();
        }
        assert!(!sim.entities[vil].dead);
        // 1.5 HP − 1.5 = 0 → dies in the sweep (a hit that kills
        // below 1 HP, not at some rounded threshold).
        sim.entities[vil].hp = I16F16::from_bits(98_304); // 1.5
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 3)], vil, true, h);
        assert_eq!(sim.entities[vil].hp, I16F16::ZERO);
        for _ in 0..5 {
            sim.tick();
        }
        assert!(sim.entities[vil].dead);
    }

    #[test]
    fn mishit_minimum_kills_at_the_boundary() {
        // 1.9 HP takes a mishit whose pre-clamp value is 0.5
        // (Σ = 1): DE deals the minimum 1 → 0.9 HP → dead. Under
        // the old order (clamp, then halve) the same hit dealt
        // 0.5 and the villager lived at 1.4 HP.
        let mut sim = Sim::new(5);
        let (atk, atk_owner, vil, _) = sandbox_pair(&sim);
        let h = sim.entity_height(vil);
        let pos = (sim.entities[vil].pos_x, sim.entities[vil].pos_y);
        sim.entities[vil].hp = I16F16::from_bits(124_518); // ≈1.9
        sim.apply_hit(atk, atk_owner, pos, &[(cls::PIERCE, 1)], vil, true, h);
        for _ in 0..5 {
            sim.tick();
        }
        assert!(sim.entities[vil].dead);
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
