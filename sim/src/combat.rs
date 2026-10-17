//! Combat (M3): damage, attack orders, animals, projectiles.
//!
//! Implements the rulebook's combat stack (§CMB/PRJ/CLS) inside the
//! deterministic sim:
//!
//! - Damage is table-driven (CMB-01): for each (class, value) pair in
//!   the attacker's row, if the target belongs to that class the pair
//!   contributes max(0, value − armor); the total is at least 1.
//!   Melee/pierce pairs always participate, mitigated by the matching
//!   armor (which may be negative later — the formula already adds).
//!   Elevation is ×1: the map is flat.
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

use crate::classes as cls;
use crate::econ::{NodeKind, ResourceNode};
use crate::entity::{EntityKind, Task, OWNER_GAIA, OWNER_PLAYER};
use crate::units::{self, UnitSpec};
use crate::{steer, Sim};

/// Melee reach, center to center: 3.0 world units. Tuning constant:
/// separation holds two fighting units ~2.0–2.2 units apart, and the
/// reference lands melee hits at radius-sum distance — 3.0 gives the
/// slack CMB-06's landing check needs without letting units hit
/// across a crowd.
pub const MELEE_REACH: I16F16 = I16F16::from_bits(196_608); // 3.0
/// A Gaia sheep is captured when a player unit comes this close
/// (OPENING-SPEC §2.2: captured by adjacency/proximity).
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
    /// Attacker entity id (kill credit).
    pub attacker: u32,
    /// Attacker owner (friendly-fire exclusion).
    pub owner: u8,
    /// Snapshot of the attacker's (class, value) attack table.
    pub attacks: Vec<(u8, i32)>,
}

/// CMB-01 damage of one landed attack against a target, as a whole
/// number (all table values are integers). Pure — unit-tested
/// directly. Returns 0 only when no attack pair participates (an
/// attacker with no applicable class at all); any participating
/// attack deals at least 1.
pub fn compute_damage(attacks: &[(u8, i32)], target: &UnitSpec) -> i32 {
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
    if participated {
        total.max(1)
    } else {
        0
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
        self.projectile_step();
        self.death_sweep();
    }

    /// Sheep capture (ECO-13): a Gaia sheep with any living player
    /// unit (not the Town Center) within capture range changes sides.
    /// Ownership then stays — stealing a captured sheep back is a
    /// multiplayer rule for later.
    fn capture_sheep(&mut self) {
        for idx in 0..self.entities.len() {
            let e = &self.entities[idx];
            if e.kind != EntityKind::Sheep || e.owner != OWNER_GAIA || e.dead {
                continue;
            }
            let (sx, sy) = (e.pos_x, e.pos_y);
            let captured = self.entities.iter().any(|u| {
                u.owner == OWNER_PLAYER
                    && !u.dead
                    && u.kind.is_mobile()
                    && steer::vec_len(u.pos_x - sx, u.pos_y - sy) <= CAPTURE_RANGE
            });
            if captured {
                self.entities[idx].owner = OWNER_PLAYER;
            }
        }
    }

    /// Gaia behavior: the boar's lure loop and deer flight. Runs
    /// before the attack machines so a boar's aggro becomes its
    /// attack target on the same tick.
    fn animal_ai(&mut self) {
        for idx in 0..self.entities.len() {
            let (kind, dead) = (self.entities[idx].kind, self.entities[idx].dead);
            if dead {
                continue;
            }
            match kind {
                EntityKind::Boar => self.boar_ai(idx),
                EntityKind::Deer => self.deer_ai(idx),
                _ => {}
            }
        }
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
                if self.entities[idx].order.is_none() {
                    self.set_move_order(idx, hx, hy);
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
                u.owner == OWNER_PLAYER
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

    /// Auto-acquisition (CMB-08): idle Militia and Archers engage the
    /// closest boar inside their LoS. Only boars qualify in M3 —
    /// herdables and deer are never auto-targeted (killing them is
    /// always an explicit order), villagers never auto-attack, and
    /// the Scout's Dark Age default stance is Stand Ground (CMB-09),
    /// so it does not acquire either.
    fn acquire_targets(&mut self) {
        for idx in 0..self.entities.len() {
            let e = &self.entities[idx];
            if e.dead
                || e.owner != OWNER_PLAYER
                || !matches!(e.kind, EntityKind::Militia | EntityKind::Archer)
                || e.attack_target.is_some()
                || e.order.is_some()
            {
                continue;
            }
            let spec = units::spec(e.kind);
            let (px, py) = (e.pos_x, e.pos_y);
            let mut best: Option<(i64, u32)> = None; // (distance² bits, id)
            for t in &self.entities {
                if t.kind != EntityKind::Boar || t.dead {
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
        let (tid, kind) = match self.entities[idx].attack_target {
            Some(t) if !self.entities[idx].dead => (t, self.entities[idx].kind),
            _ => return,
        };
        let spec = units::spec(kind);
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
                self.set_move_order(idx, tx, ty);
            }
        }
    }

    /// Land one strike from attacker `idx` on target `ti`: melee
    /// damage now (if still in reach), or a projectile release for a
    /// ranged attacker. Either way the reload restarts from this
    /// landing tick (CMB-05) and the engagement is underway.
    fn resolve_strike(&mut self, idx: usize, ti: usize) {
        let spec = units::spec(self.entities[idx].kind);
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
                attacker: attacker_id,
                owner,
                attacks: spec.attacks.to_vec(),
            });
        } else {
            // Melee: lands only if the target is still in reach at
            // the landing tick (CMB-06); otherwise it whiffs.
            let dist = steer::vec_len(tx - ax, ty - ay);
            if dist <= MELEE_REACH {
                self.apply_hit(attacker_id, (ax, ay), spec.attacks, ti, false);
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
    ) {
        let tspec = units::spec(self.entities[ti].kind);
        let full = compute_damage(attacks, tspec);
        let dmg = if mishit {
            // CMB-13: a mishit deals 50% (fixed-point halving is exact).
            I16F16::from_num(full) / I16F16::from_num(2)
        } else {
            I16F16::from_num(full)
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
                    if aowner == OWNER_PLAYER && !adead {
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
                // Retaliation: an idle military unit answers its
                // attacker. (Villagers never auto-attack — task rule.)
                let t = &self.entities[ti];
                if t.owner == OWNER_PLAYER && t.attack_target.is_none() && t.order.is_none() {
                    self.entities[ti].attack_target = Some(attacker_id);
                }
            }
            _ => {}
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
        if let Some(tid) = p.intended {
            if let Some(ti) = self.entity_index(tid) {
                if !self.entities[ti].dead {
                    let tspec = units::spec(self.entities[ti].kind);
                    let d = steer::vec_len(
                        self.entities[ti].pos_x - p.impact_x,
                        self.entities[ti].pos_y - p.impact_y,
                    );
                    if d <= tspec.radius + HIT_SLACK {
                        self.apply_hit(p.attacker, (p.spawn_x, p.spawn_y), &p.attacks, ti, false);
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
                self.apply_hit(p.attacker, (p.spawn_x, p.spawn_y), &p.attacks, ti, true);
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
