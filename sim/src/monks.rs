//! Monk core (Castle P5a, MNK-01's healing slice + MNK-10):
//! the Monk's heal order, its idle auto-task, the range chase
//! and the per-tick healing accumulator. Conversion, faith as a
//! spent resource, and relics are the P5b/P5d slices — they join
//! this same phase (MNK-01's phase order: after combat, in id
//! order) and this same entity field block (hash marker 0xBB).
//!
//! MNK-10's documented row (wiki, "Monk (Age of Empires II)" +
//! "Healing", via the Castle Age dossier §5.6): heal range 4
//! tiles, rate 150 HP/min (= 2.5 HP/s), one allied unit at a
//! time; monks never heal buildings, siege units or ships, and
//! healing does not interrupt the healed unit's own orders.
//! "Siege" is read off the class mask (CLS-01 discipline): the
//! Battering Ram carries SIEGE and is excluded; the Petard's row
//! is INFANTRY only, so it stays healable, as in the reference.
//! The Town Center is an entity in this architecture but a
//! building for this rule — excluded by kind.

use fixed::types::I16F16;

use crate::entity::{EntityKind, Task};
use crate::Sim;

/// MNK-10: the heal range, 4 tiles = 16 world units, measured
/// center-to-center like every other range in the sim
/// (`Sim::distance_to`).
pub const HEAL_RANGE: I16F16 = I16F16::from_bits(1_048_576); // 16.0

/// MNK-10: the heal rate, 150 HP per minute — 2.5 HP/s exactly.
/// The accumulator below restores precisely this over any whole
/// minute, with no rounding drift (the GAR-04 shelter pattern).
pub const HEAL_HP_PER_MIN: i64 = 150;

impl Sim {
    /// Whether a `Heal` command would be accepted right now: the
    /// actor is a living, fielded Monk of a player (the
    /// `orderable` discipline every unit order shares), and the
    /// target is one of its owner's wounded healable units
    /// (`healable_target`) — never the monk itself (a monk cannot
    /// heal itself in the reference).
    pub fn can_heal(&self, entity: u32, target: u32) -> bool {
        if entity == target {
            return false;
        }
        let (Some(mi), Some(ti)) = (self.entity_index(entity), self.entity_index(target))
        else {
            return false;
        };
        let m = &self.entities[mi];
        if m.kind != EntityKind::Monk || !self.orderable(mi) {
            return false;
        }
        self.healable_target(m.owner, ti)
    }

    /// Whether the entity at `ti` is a valid heal target for
    /// `owner`'s monks (MNK-10's exclusions): alive, owned by the
    /// monk's owner, in the field (not garrisoned), not the Town
    /// Center (a building for this rule), not a SIEGE-class unit,
    /// and below its effective maximum HP.
    fn healable_target(&self, owner: u8, ti: usize) -> bool {
        let t = &self.entities[ti];
        if t.dead || t.owner != owner || t.garrisoned_in.is_some() {
            return false;
        }
        if t.kind == EntityKind::TownCenter {
            return false;
        }
        if crate::units::spec(t.kind).classes & crate::classes::bit(crate::classes::SIEGE)
            != 0
        {
            return false;
        }
        t.hp < I16F16::from_num(self.unit_stats(ti).max_hp)
    }

    /// Apply a `Heal` command (MNK-10): the monk takes the target
    /// as its heal task — replacing whatever it was doing, on the
    /// replace semantics every direct order shares (a non-queued
    /// order clears the queue, SEL-12) — and the monk phase walks
    /// it into range and heals. A pending garrison walk is
    /// cancelled too: the heal order is a full re-task, and a monk
    /// ordered to heal must not slip into a building on its way.
    pub(crate) fn issue_heal(&mut self, entity: u32, target: u32) {
        if !self.can_heal(entity, target) {
            return;
        }
        let idx = self.entity_index(entity).unwrap();
        self.entities[idx].order_queue.clear();
        self.entities[idx].order_from_queue = false;
        let e = &mut self.entities[idx];
        e.task = Task::Idle;
        e.task_node = None;
        e.task_building = None;
        e.task_dropoff = None;
        e.clear_combat();
        e.auto_explore = false;
        e.auto_target = None;
        e.attack_move = None;
        e.patrol.clear();
        e.patrol_next = 0;
        e.garrison_target = None;
        e.garrison_prod_entry = false;
        if e.heal_target != Some(target) {
            e.heal_target = Some(target);
            e.heal_remainder = 0;
        }
    }

    /// Per-tick monk work (Castle P5a): for each Monk in id
    /// order — validate its heal task, auto-task an idle monk to
    /// the nearest wounded healable ally inside its sight, chase
    /// an out-of-range target, and accrue healing to an in-range
    /// one. Runs as tick step 4.6, after garrison and after
    /// combat (MNK-01's phase order).
    pub(crate) fn monk_tick(&mut self) {
        for idx in 0..self.entities.len() {
            if self.entities[idx].kind != EntityKind::Monk || self.entities[idx].dead {
                continue;
            }
            let owner = self.entities[idx].owner;
            if self.entities[idx].garrisoned_in.is_some() {
                // GAR-02: a garrisoned monk is out of the field —
                // it heals nobody, and any task it held is over.
                let e = &mut self.entities[idx];
                e.heal_target = None;
                e.heal_remainder = 0;
                continue;
            }
            // Validate the standing task; a target that died,
            // filled up, garrisoned or stopped being healable
            // ends it (remainder resets with the task).
            let mut target_idx = match self.entities[idx].heal_target {
                Some(t) => match self.entity_index(t) {
                    Some(ti) if self.healable_target(owner, ti) => Some(ti),
                    _ => {
                        let e = &mut self.entities[idx];
                        e.heal_target = None;
                        e.heal_remainder = 0;
                        None
                    }
                },
                None => None,
            };
            // Auto-task (MNK-10): a monk standing idle — no task,
            // no move order, no garrison walk — takes the nearest
            // wounded healable ally inside its own sight (ties to
            // the lowest id, CMB-08's tie-break).
            if target_idx.is_none()
                && self.entities[idx].order.is_none()
                && self.entities[idx].garrison_target.is_none()
            {
                if let Some(ti) = self.nearest_healable(idx, owner) {
                    let id = self.entities[ti].id;
                    let e = &mut self.entities[idx];
                    e.heal_target = Some(id);
                    e.heal_remainder = 0;
                    target_idx = Some(ti);
                }
            }
            let Some(ti) = target_idx else {
                continue;
            };
            let (tx, ty) = (self.entities[ti].pos_x, self.entities[ti].pos_y);
            let dist = self.distance_to(idx, tx, ty);
            if dist > HEAL_RANGE {
                // Chase: (re)install the walk toward the target's
                // current position — but only when the standing
                // order is not already headed there (the target
                // moved more than half a tile), so a walking monk
                // does not re-path every tick.
                let stale = match &self.entities[idx].order {
                    None => true,
                    Some(o) => {
                        crate::steer::vec_len(o.target_x - tx, o.target_y - ty)
                            > I16F16::from_num(2)
                    }
                };
                if stale {
                    self.set_move_order(idx, tx, ty);
                }
                continue;
            }
            // In range: stand and heal. Any walk order is over.
            if self.entities[idx].order.is_some() {
                let e = &mut self.entities[idx];
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
            let max_hp = I16F16::from_num(self.unit_stats(ti).max_hp);
            let total = self.entities[idx].heal_remainder + HEAL_HP_PER_MIN * 65_536;
            let heal_bits = total / 3600;
            self.entities[idx].heal_remainder = total % 3600;
            let t = &mut self.entities[ti];
            t.hp += I16F16::from_bits(heal_bits as i32);
            if t.hp > max_hp {
                t.hp = max_hp;
            }
            if t.hp >= max_hp {
                // Whole: the task completes (the accumulator
                // resets at full HP, the GAR-04 discipline).
                let e = &mut self.entities[idx];
                e.heal_target = None;
                e.heal_remainder = 0;
            }
        }
    }

    /// The nearest wounded healable ally of `owner` inside the
    /// monk at `idx`'s sight, for the auto-task: squared-distance
    /// bits then lowest id (CMB-08's tie-break). `None` when no
    /// candidate stands in sight.
    fn nearest_healable(&self, idx: usize, owner: u8) -> Option<usize> {
        let m = &self.entities[idx];
        let los = self.unit_stats(idx).los;
        let (px, py) = (m.pos_x, m.pos_y);
        let mut best: Option<(i64, u32, usize)> = None;
        for ti in 0..self.entities.len() {
            if ti == idx || !self.healable_target(owner, ti) {
                continue;
            }
            let t = &self.entities[ti];
            let d = crate::steer::vec_len(t.pos_x - px, t.pos_y - py);
            if d > los {
                continue;
            }
            let db = d.to_bits() as i64;
            let key = (db * db, t.id, ti);
            if best.map(|b| key < b).unwrap_or(true) {
                best = Some(key);
            }
        }
        best.map(|(_, _, ti)| ti)
    }
}
