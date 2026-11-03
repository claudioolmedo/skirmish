//! Monk core (Castle P5a, MNK-01's healing slice + MNK-10):
//! the Monk's heal order, its idle auto-task, the range chase
//! and the per-tick healing accumulator. Conversion, faith as a
//! spent resource, and relics are the P5b/P5d slices — they join
//! this same phase (MNK-01's phase order: after combat, in id
//! order) and this same entity field block (hash marker 0xBB).
//! Castle P5d adds the relic lifecycle at this module's tail
//! (MNK-09/ECO-16): pickup, carry, deposit, retrieve, the
//! Monastery trickle, and the read-side queries the client and
//! the AI consume.
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

use crate::build::BuildingKind;
use crate::entity::{is_player_owner, EntityKind, Task};
use crate::econ::ResourceKind;
use crate::steer;
use crate::tech::TechId;
use crate::{Sim, MELEE_REACH};

/// MNK-03: the conversion range, 9 tiles = 36 world units,
/// measured center-to-center like every other range in the sim.
pub const CONVERT_RANGE: I16F16 = I16F16::from_bits(2_359_296); // 36.0

/// MNK-03 / TEC-10 (Castle P5f): Block Printing's bonus, +3
/// tiles = 12 world units on the conversion range (12 tiles
/// total). Read at the one range check in `conversion_step`
/// through `conversion_range`; the monk's line of sight (11
/// tiles, the unit table row) is a separate constant and is
/// NOT touched — no extra behavior is invented here.
pub const BLOCK_PRINTING_RANGE_BONUS: I16F16 = I16F16::from_bits(786_432); // 12.0

/// MNK-03: one conversion interval (CI) ≈ 1.25 s = 75 ticks at
/// 60 Hz. The ≈ is in the documented model itself; 75 is an
/// implementation constant (the 2 400-tick 40 s build is the rate
/// precedent), never a more precise DE value than the source
/// gives.
pub const CI_TICKS: u32 = 75;

/// MNK-02: full faith, in the entity record's milli-percent
/// (0..=100_000).
pub const FAITH_FULL: i32 = 100_000;

/// MNK-02: faith regenerates to full in 62 s (3 720 ticks); with
/// Illumination (Imperial) in 33 s (1 980 ticks). The Illumination
/// row is a data constant read through the tech-flag helper —
/// the tech itself landed in Castle P5f (TechId 40).
pub const FAITH_REGEN_TICKS: i64 = 3_720;
pub const FAITH_REGEN_TICKS_ILLUMINATION: i64 = 1_980;

/// MNK-03: the first CI that rolls, and the CI that guarantees
/// conversion, before MNK-05's resistance shifts.
pub const FIRST_ROLL_CI: u32 = 5;
pub const GUARANTEE_CI: u32 = 9;

/// MNK-03/MNK-05: the base per-CI chance, in percent; the rolled
/// chance against a target is `BASE_CHANCE_PCT / max(1, level)`.
pub const BASE_CHANCE_PCT: u32 = 38;

/// MNK-04 (Castle P5c): the building CI row — the first roll
/// lands at CI 15 and conversion is guaranteed at CI 25. The
/// per-CI chance is the level-3 row of the one MNK-05 formula
/// (38/3 ≈ 13%, the figure MNK-04's text pins).
pub const BUILDING_FIRST_ROLL_CI: u32 = 15;
pub const BUILDING_GUARANTEE_CI: u32 = 25;

/// The MNK-05 chance for a resistance `level`: 38 / level,
/// rounded to the nearest percent — one formula giving the
/// documented rows exactly (level 1 → 38%, level 2 → 19%,
/// level 3 → 13%).
fn chance_pct(level: u32) -> u32 {
    let level = level.max(1);
    (BASE_CHANCE_PCT + level / 2) / level
}

/// MNK-04's contact kinds: the units that convert only with the
/// monk ADJACENT even though they are units — Battering Rams
/// here; Armored Elephants and Trebuchets join this predicate
/// when those kinds exist (the Redemption wiki article's list:
/// buildings, Trebuchets, Siege Towers and Battering Rams are
/// contact-only; every OTHER siege weapon converts at range).
fn is_contact_unit(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::BatteringRam)
}

/// MNK-10: the heal range, 4 tiles = 16 world units, measured
/// center-to-center like every other range in the sim
/// (`Sim::distance_to`).
pub const HEAL_RANGE: I16F16 = I16F16::from_bits(1_048_576); // 16.0

/// MNK-10: the heal rate, 150 HP per minute — 2.5 HP/s exactly.
/// The accumulator below restores precisely this over any whole
/// minute, with no rounding drift (the GAR-04 shelter pattern).
pub const HEAL_HP_PER_MIN: i64 = 150;

/// MNK-09 (Castle P5d): the carrying monk's line of sight —
/// 9 tiles = 36 world units (his base 11 minus the documented
/// 2). Applied at the one LoS lookup (`Sim::unit_stats`), never
/// written into the table row.
pub const CARRY_LOS: I16F16 = I16F16::from_bits(2_359_296); // 36.0

/// MNK-09 (Castle P5d): the pickup touch distance — a monk
/// ordered onto a ground relic takes it once his center stands
/// this close to the relic's center (2 tiles). An
/// implementation constant in the steering-arrival family (the
/// generic waypoint reach is 1.0): the sources name no pickup
/// radius; the relic's cell is unblocked, so the monk can
/// always close the last of this distance.
pub const RELIC_TOUCH: I16F16 = I16F16::from_bits(524_288); // 8.0

/// ECO-16 (Castle P5d): one garrisoned relic pays 30 gold per
/// minute (0.5 gold/s) to its Monastery's owner, continuously,
/// until removed. The trickle accumulator in `relic_tick`
/// restores precisely this over any whole minute, with no
/// rounding drift (the GAR-04 remainder pattern).
pub const RELIC_GOLD_PER_MIN: i64 = 30;

/// MNK-09's relic state (Castle P5d): the lifecycle the plan
/// prescribes — a relic is on the ground, carried by a monk, or
/// garrisoned in a Monastery, exactly one of the three at any
/// time. Derived from the entity record's two pointers
/// (`relic_carried_by` / `relic_garrisoned_in`); never stored
/// separately, so it can never disagree with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelicState {
    Ground,
    CarriedBy(u32),
    GarrisonedIn(u32),
}

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
        // Castle P5b: a heal order also ends a conversion task
        // (the monk is re-tasked by hand; CI progress stays
        // sticky — MNK-07 names only the manual MOVE as a reset).
        e.conversion_target = None;
        e.conversion_target_building = None;
        e.ci_ticks = 0;
        // Castle P5d: a heal order also ends a relic errand
        // (the carry itself is NOT an errand — a carrying monk
        // ordered to heal keeps his relic, MNK-09).
        e.relic_pickup_target = None;
        e.relic_deposit_target = None;
        if e.heal_target != Some(target) {
            e.heal_target = Some(target);
            e.heal_remainder = 0;
        }
    }

    /// Whether the Imperial Illumination flag stands for `owner`
    /// (MNK-02's 33 s recharge). Read in P5b; the tech itself
    /// landed in Castle P5f (TechId 40, Monastery, 120 G / 65 s).
    fn illumination_researched(&self, owner: u8) -> bool {
        self.has_tech(owner, TechId::Illumination)
    }

    /// Whether the Imperial Theocracy flag stands for `owner`
    /// (MNK-06's rest rule). Read in P5b; the tech itself landed
    /// in Castle P5f (TechId 41, Monastery, 200 G / 75 s).
    fn theocracy_researched(&self, owner: u8) -> bool {
        self.has_tech(owner, TechId::Theocracy)
    }

    /// Whether the Imperial Block Printing flag stands for
    /// `owner` (MNK-03's +3 conversion range). The tech landed
    /// in Castle P5f (TechId 39, Monastery, 200 G / 55 s).
    fn block_printing_researched(&self, owner: u8) -> bool {
        self.has_tech(owner, TechId::BlockPrinting)
    }

    /// Whether the Imperial Faith flag stands for `owner`
    /// (MNK-05's +4/+4 shift): checked against the TARGET's
    /// owner, like Devotion (Castle P5f, TechId 42, Monastery,
    /// 550 F / 750 G / 60 s, requires Devotion).
    fn faith_researched(&self, owner: u8) -> bool {
        self.has_tech(owner, TechId::Faith)
    }

    /// The conversion range for a monk of `owner` (MNK-03):
    /// the base 9 tiles, +3 tiles with Block Printing. The one
    /// read every unit-conversion range check goes through.
    fn conversion_range(&self, owner: u8) -> I16F16 {
        if self.block_printing_researched(owner) {
            CONVERT_RANGE + BLOCK_PRINTING_RANGE_BONUS
        } else {
            CONVERT_RANGE
        }
    }

    /// Whether the Redemption flag stands for `owner` (MNK-08's
    /// siege/building gate). Read in P5b; the tech itself landed
    /// in Castle P5c (TechId 37, Monastery, 475 G / 50 s).
    fn redemption_researched(&self, owner: u8) -> bool {
        self.has_tech(owner, TechId::Redemption)
    }

    /// Whether the Heresy flag stands for `owner` (MNK-11):
    /// checked in the success handler against the VICTIM owner's
    /// set (Castle P5c, TechId 38, Monastery, 1000 G / 60 s).
    fn heresy_researched(&self, owner: u8) -> bool {
        self.has_tech(owner, TechId::Heresy)
    }

    /// MNK-08 eligibility of the target at `ti` for a monk of
    /// `monk_owner` — the complete P5b function (plan §3.3.6):
    /// heroes and Kings are never convertible — refused by the
    /// CLS-06 `immune_conversion` flag on the unit row (the King
    /// kind landed with Package C / VIC-06 and sets it); the
    /// kinds that DO exist from DE's never-list are refused
    /// here: Town Centers, and by the siege gate below
    /// everything Redemption would unlock, which includes
    /// Castles/Monasteries/walls as placed buildings — building
    /// conversion itself is P5c);
    /// animals and herdables are not conversion targets at all;
    /// enemy monks require Atonement; siege-class units require
    /// Redemption (closed until P5c); villagers and ordinary
    /// military are always eligible. Garrisoned units are never
    /// offered (GAR-02 — they are out of the field).
    fn eligible_target(&self, monk_owner: u8, ti: usize) -> bool {
        let t = &self.entities[ti];
        if t.dead || t.garrisoned_in.is_some() || t.owner == monk_owner {
            return false;
        }
        if crate::player_slot(t.owner).is_none() {
            return false; // Gaia: animals, herdables, wildlife.
        }
        match t.kind {
            EntityKind::TownCenter
            | EntityKind::Sheep
            | EntityKind::Boar
            | EntityKind::Deer
            | EntityKind::Wolf => return false,
            _ => {}
        }
        if crate::units::spec(t.kind).immune_conversion {
            return false; // CLS-06: the King (VIC-06) — never convertible.
        }
        if t.kind == EntityKind::Monk && !self.has_tech(monk_owner, TechId::Atonement) {
            return false;
        }
        if crate::units::spec(t.kind).classes & crate::classes::bit(crate::classes::SIEGE)
            != 0
            && !self.redemption_researched(monk_owner)
        {
            return false;
        }
        true
    }

    /// Whether a `Convert` order's TARGET is eligible right now
    /// (MNK-08 only — no faith gate): the client reads this for
    /// cursor feedback and the AI for target choice, never
    /// reimplementing the list. The monk must be a living,
    /// fielded Monk of a player.
    pub fn can_convert_target(&self, entity: u32, target: u32) -> bool {
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
        self.eligible_target(m.owner, ti)
    }

    /// Whether a `Convert` command would be accepted right now:
    /// an eligible target (MNK-08) AND full faith (MNK-02 — a
    /// monk with partial faith cannot start a conversion, though
    /// healing is always available).
    pub fn can_convert(&self, entity: u32, target: u32) -> bool {
        if !self.can_convert_target(entity, target) {
            return false;
        }
        let mi = self.entity_index(entity).unwrap();
        // MNK-09 (Castle P5d): a monk carrying a relic cannot
        // convert — he delivers it instead. The refusal is
        // sim-side validation here; the client mirrors it
        // through the `carrying_relic` read.
        if self.entities[mi].carrying_relic.is_some() {
            return false;
        }
        self.entities[mi].faith >= FAITH_FULL
    }

    /// MNK-08 eligibility of the BUILDING at `bi` for a monk of
    /// `monk_owner` (Castle P5c — the building arm of the one
    /// eligibility function): the monk's owner must hold
    /// Redemption; the building must be COMPLETE (foundations are
    /// on DE's never list), enemy-owned and player-owned; and its
    /// kind must not be on the never-even-with-Redemption list:
    /// Town Centers, Castles, Monasteries, walls, gates, Farms
    /// (and Wonders / Kreposts / Fortified Churches / Fish Traps —
    /// kinds that do not exist here, named in DE's list and
    /// refused by construction when they land, as the unit arm
    /// documents). Everything else — House, Barracks, camps,
    /// Market, Blacksmith, ranges, Stable, towers, University,
    /// Siege Workshop — converts.
    fn eligible_building(&self, monk_owner: u8, bi: usize) -> bool {
        if !self.redemption_researched(monk_owner) {
            return false;
        }
        let b = &self.buildings[bi];
        if !b.complete || b.owner == monk_owner {
            return false;
        }
        if crate::player_slot(b.owner).is_none() {
            return false; // Gaia-owned structures are not targets.
        }
        !matches!(
            b.kind,
            BuildingKind::TownCenter
                | BuildingKind::Castle
                | BuildingKind::Monastery
                | BuildingKind::PalisadeWall
                | BuildingKind::StoneWall
                | BuildingKind::PalisadeGate
                | BuildingKind::StoneGate
                | BuildingKind::Farm
        )
    }

    /// Whether a `ConvertBuilding` order's TARGET is eligible
    /// right now (MNK-08 only — no faith gate): the client reads
    /// this for cursor feedback, the sim re-validates on apply.
    pub fn can_convert_building_target(&self, entity: u32, building: u32) -> bool {
        let (Some(mi), Some(bi)) = (self.entity_index(entity), self.building_index(building))
        else {
            return false;
        };
        let m = &self.entities[mi];
        if m.kind != EntityKind::Monk || !self.orderable(mi) {
            return false;
        }
        self.eligible_building(m.owner, bi)
    }

    /// Whether a `ConvertBuilding` command would be accepted
    /// right now: an eligible building (MNK-08) AND full faith
    /// (MNK-02).
    pub fn can_convert_building(&self, entity: u32, building: u32) -> bool {
        if !self.can_convert_building_target(entity, building) {
            return false;
        }
        let mi = self.entity_index(entity).unwrap();
        // MNK-09 (Castle P5d): the carrying refusal, building
        // arm — see `can_convert`.
        if self.entities[mi].carrying_relic.is_some() {
            return false;
        }
        self.entities[mi].faith >= FAITH_FULL
    }

    /// Apply a `ConvertBuilding` command (MNK-04): replace
    /// semantics like every direct order; the monk phase walks
    /// the monk adjacent to the footprint and advances the
    /// building CI row.
    pub(crate) fn issue_convert_building(&mut self, entity: u32, building: u32) {
        if !self.can_convert_building(entity, building) {
            return;
        }
        let idx = self.entity_index(entity).unwrap();
        self.entities[idx].order_queue.clear();
        self.entities[idx].order_from_queue = false;
        self.apply_convert_building(idx, building);
    }

    /// The mechanics of a building convert order, shared by the
    /// `ConvertBuilding` command and queued-order execution.
    /// The resistance stamps encode the building row (MNK-04/05)
    /// in the same fields the unit row uses: level 3, and shifts
    /// that lift the unit bases (first roll CI 5, guarantee CI 9)
    /// to the building bases (CI 15 / CI 25) — the +10/+16 deltas
    /// are the row difference, with the target owner's Devotion
    /// shift (MNK-05's +1/+1, read at start like the unit row)
    /// added on top. Sticky progress (MNK-07): switching unit →
    /// building does NOT reset the counter — only the
    /// building → unit switch does (see `apply_convert`).
    pub(crate) fn apply_convert_building(&mut self, idx: usize, building: u32) {
        let bi = self.building_index(building).unwrap();
        let target_owner = self.buildings[bi].owner;
        let devotion = self.has_tech(target_owner, TechId::Devotion);
        let shift = if devotion { 1 } else { 0 };
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
        e.heal_target = None;
        e.heal_remainder = 0;
        e.relic_pickup_target = None;
        e.relic_deposit_target = None;
        e.conversion_target = None;
        e.conversion_target_building = Some(building);
        e.conversion_level = 3;
        e.conversion_start_shift = (BUILDING_FIRST_ROLL_CI - FIRST_ROLL_CI) as i32 + shift;
        e.conversion_guarantee_shift =
            (BUILDING_GUARANTEE_CI - GUARANTEE_CI) as i32 + shift;
    }

    /// A monk's faith as a whole percent (0..100) — the client's
    /// selection-line read (MNK-02). Missing/non-monk ids read 0.
    pub fn monk_faith(&self, entity: u32) -> i32 {
        match self.entity_index(entity) {
            Some(i) if self.entities[i].kind == EntityKind::Monk => {
                self.entities[i].faith / 1000
            }
            _ => 0,
        }
    }

    /// Apply a `Convert` command (MNK-03): the monk takes the
    /// target as its conversion task — replace semantics like
    /// every direct order — and the monk phase walks it into
    /// range and advances CIs. The resistance row (MNK-05) is
    /// stamped NOW, at the start: level (Scout-line 2, else 1),
    /// start shift (Scout +3, Devotion on the TARGET's owner
    /// +1, Faith on the TARGET's owner +4 — the Imperial tech
    /// landed in Castle P5f) and guarantee shift (Scout +1,
    /// Devotion +1, Faith +4).
    /// Sticky progress (MNK-07): `ci_progress`/`ci_ticks` are
    /// NOT reset by a target switch — the counter lives on the
    /// monk; only the stamps are re-read for the new target.
    pub(crate) fn issue_convert(&mut self, entity: u32, target: u32) {
        if !self.can_convert(entity, target) {
            return;
        }
        let idx = self.entity_index(entity).unwrap();
        self.entities[idx].order_queue.clear();
        self.entities[idx].order_from_queue = false;
        self.apply_convert(idx, target);
    }

    /// The mechanics of a convert order, shared by the `Convert`
    /// command and by queued-order execution (which must not
    /// clear the queue behind it). Callers validate first
    /// (`can_convert`).
    pub(crate) fn apply_convert(&mut self, idx: usize, target: u32) {
        // MNK-07, the building→unit arm (Castle P5c — its test
        // was assigned to this slice by the plan, §5): a monk
        // leaving a BUILDING conversion for a unit target resets
        // the sticky CI counter (the update-95810 rule). A
        // unit→unit switch keeps it, as does unit→building
        // (`apply_convert_building` never zeroes it either).
        if self.entities[idx].conversion_target_building.is_some() {
            let e = &mut self.entities[idx];
            e.ci_progress = 0;
            e.ci_ticks = 0;
        }
        let ti = self.entity_index(target).unwrap();
        let (level, scout) = {
            let k = self.entities[ti].kind;
            let scout = matches!(k, EntityKind::ScoutCavalry | EntityKind::LightCavalry);
            (if scout { 2u8 } else { 1u8 }, scout)
        };
        let target_owner = self.entities[ti].owner;
        let devotion = self.has_tech(target_owner, TechId::Devotion);
        let faith = self.faith_researched(target_owner);
        let start_shift =
            (if scout { 3 } else { 0 }) + (if devotion { 1 } else { 0 }) + (if faith { 4 } else { 0 });
        let guarantee_shift =
            (if scout { 1 } else { 0 }) + (if devotion { 1 } else { 0 }) + (if faith { 4 } else { 0 });
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
        e.heal_target = None;
        e.heal_remainder = 0;
        e.relic_pickup_target = None;
        e.relic_deposit_target = None;
        e.conversion_target = Some(target);
        e.conversion_target_building = None;
        e.conversion_level = level;
        e.conversion_start_shift = start_shift;
        e.conversion_guarantee_shift = guarantee_shift;
    }

    /// Faith regen for one monk (MNK-02): full in 62 s (33 s
    /// under Illumination), exact over the span through the
    /// remainder accumulator — no rounding drift.
    fn regen_faith(&mut self, idx: usize) {
        let e = &self.entities[idx];
        if e.faith >= FAITH_FULL {
            return;
        }
        let span = if self.illumination_researched(e.owner) {
            FAITH_REGEN_TICKS_ILLUMINATION
        } else {
            FAITH_REGEN_TICKS
        };
        let total = e.faith_remainder + FAITH_FULL as i64;
        let add = total / span;
        let e = &mut self.entities[idx];
        e.faith += add as i32;
        if e.faith >= FAITH_FULL {
            e.faith = FAITH_FULL;
            e.faith_remainder = 0;
        } else {
            e.faith_remainder = total % span;
        }
    }

    /// One monk's conversion advance for this tick. Returns true
    /// while the monk is busy converting (standing or chasing),
    /// so the caller skips its heal work — a converting monk does
    /// not also heal (its hands are busy; the DE reading the
    /// heal arm's own one-target discipline mirrors).
    fn conversion_step(&mut self, idx: usize) -> bool {
        if self.entities[idx].conversion_target_building.is_some() {
            return self.conversion_building_step(idx);
        }
        let Some(target) = self.entities[idx].conversion_target else {
            return false;
        };
        let owner = self.entities[idx].owner;
        let valid = match self.entity_index(target) {
            Some(ti) => self.eligible_target(owner, ti),
            None => false,
        };
        if !valid {
            // Target died, converted, garrisoned or stopped being
            // eligible: the task ends; sticky progress SURVIVES
            // (MNK-07 — only the in-CI tick count restarts).
            let e = &mut self.entities[idx];
            e.conversion_target = None;
            e.ci_ticks = 0;
            return false;
        }
        let ti = self.entity_index(target).unwrap();
        let (tx, ty) = (self.entities[ti].pos_x, self.entities[ti].pos_y);
        let dist = self.distance_to(idx, tx, ty);
        // MNK-04 (Castle P5c): contact units (the Battering Ram)
        // convert only with the monk ADJACENT — the CMB-06 melee
        // reach against the target's center — while every other
        // siege weapon (Mangonel, Scorpion) converts at the full
        // range 9 like any unit (the Redemption article's split).
        let limit = if is_contact_unit(self.entities[ti].kind) {
            crate::combat::MELEE_REACH
        } else {
            self.conversion_range(owner)
        };
        if dist > limit {
            // Automatic chase (MNK-07): does NOT reset progress.
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
            return true;
        }
        // In range: stand and chant. Any walk order is over.
        if self.entities[idx].order.is_some() {
            let e = &mut self.entities[idx];
            e.order = None;
            e.vel_x = I16F16::ZERO;
            e.vel_y = I16F16::ZERO;
        }
        {
            let e = &mut self.entities[idx];
            e.ci_ticks += 1;
            if e.ci_ticks < CI_TICKS {
                return true;
            }
            e.ci_ticks = 0;
            e.ci_progress += 1;
        }
        let (progress, first_roll, guarantee, level) = {
            let e = &self.entities[idx];
            let first_roll =
                (FIRST_ROLL_CI as i32 + e.conversion_start_shift).max(1) as u32;
            let guarantee = ((GUARANTEE_CI as i32 + e.conversion_guarantee_shift)
                .max(first_roll as i32)) as u32;
            (e.ci_progress, first_roll, guarantee, e.conversion_level.max(1) as u32)
        };
        // RNG discipline (plan §3.3.4): exactly one draw per
        // rolled CI boundary, in monk-phase id order; NO draw for
        // warm-up CIs and NO draw for the guarantee (a counter).
        if progress >= guarantee {
            self.conversion_success(idx, target);
        } else if progress >= first_roll {
            let chance = chance_pct(level);
            if self.rng.below(100) < chance {
                self.conversion_success(idx, target);
            }
        }
        true
    }

    /// One monk's BUILDING conversion advance for this tick
    /// (Castle P5c, MNK-04): the same CI machinery as the unit
    /// row, with the range check replaced by ADJACENCY to the
    /// footprint — the monk stands in the ring around the
    /// building (footprint distance within the CMB-06 melee
    /// reach, the same clamp geometry combat uses against
    /// buildings) and makes NO progress from range 9. The CI
    /// window itself rides the stamps `apply_convert_building`
    /// set (first roll CI 15, guarantee CI 25, level 3 → 13%).
    fn conversion_building_step(&mut self, idx: usize) -> bool {
        let Some(building) = self.entities[idx].conversion_target_building else {
            return false;
        };
        let owner = self.entities[idx].owner;
        let valid = match self.building_index(building) {
            Some(bi) => self.eligible_building(owner, bi),
            None => false,
        };
        if !valid {
            // Building destroyed, converted or no longer
            // eligible: the task ends; sticky progress SURVIVES
            // (MNK-07 — it is the building→UNIT switch by hand
            // that resets, not the target's loss).
            let e = &mut self.entities[idx];
            e.conversion_target_building = None;
            e.ci_ticks = 0;
            return false;
        }
        let bi = self.building_index(building).unwrap();
        let b = &self.buildings[bi];
        let (bx, by, kind) = (b.pos_x, b.pos_y, b.kind);
        let (mx, my) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let dist = self.distance_to_footprint(mx, my, bx, by, kind);
        if dist > crate::combat::MELEE_REACH {
            // Automatic chase (MNK-07): steer to the standing
            // point just outside the footprint edge on the monk's
            // approach line (the drop-off steer geometry), and
            // only re-path when the standing order is stale. The
            // chase does NOT reset progress.
            let half = I16F16::from_num(kind.footprint_size() as i32 * 2);
            let qx = mx.clamp(bx - half, bx + half);
            let qy = my.clamp(by - half, by + half);
            let (dx, dy) = (mx - qx, my - qy);
            let (sx, sy) = if dx == I16F16::ZERO && dy == I16F16::ZERO {
                (bx + half + I16F16::from_num(2), by)
            } else {
                let len = crate::steer::vec_len(dx, dy);
                let out = I16F16::from_num(2);
                (qx + dx * out / len, qy + dy * out / len)
            };
            let stale = match &self.entities[idx].order {
                None => true,
                Some(o) => {
                    crate::steer::vec_len(o.target_x - sx, o.target_y - sy)
                        > I16F16::from_num(2)
                }
            };
            if stale {
                self.set_move_order(idx, sx, sy);
            }
            return true;
        }
        // Adjacent: stand and chant. Any walk order is over.
        if self.entities[idx].order.is_some() {
            let e = &mut self.entities[idx];
            e.order = None;
            e.vel_x = I16F16::ZERO;
            e.vel_y = I16F16::ZERO;
        }
        {
            let e = &mut self.entities[idx];
            e.ci_ticks += 1;
            if e.ci_ticks < CI_TICKS {
                return true;
            }
            e.ci_ticks = 0;
            e.ci_progress += 1;
        }
        let (progress, first_roll, guarantee, level) = {
            let e = &self.entities[idx];
            let first_roll =
                (FIRST_ROLL_CI as i32 + e.conversion_start_shift).max(1) as u32;
            let guarantee = ((GUARANTEE_CI as i32 + e.conversion_guarantee_shift)
                .max(first_roll as i32)) as u32;
            (e.ci_progress, first_roll, guarantee, e.conversion_level.max(1) as u32)
        };
        // RNG discipline, identical to the unit row: exactly one
        // draw per rolled CI boundary, none in warm-up, none at
        // the guarantee.
        if progress >= guarantee {
            self.conversion_building_success(idx, building);
        } else if progress >= first_roll {
            let chance = chance_pct(level);
            if self.rng.below(100) < chance {
                self.conversion_building_success(idx, building);
            }
        }
        true
    }

    /// Conversion success (MNK-06 + MNK-12): the target changes
    /// sides through the one transfer function, then the
    /// participant set — every monk of the converting owner whose
    /// record names this target, ascending id — rests: ALL drain
    /// faith to 0, unless the owner has Theocracy, in which case
    /// only the lowest-id participant drains (the others keep
    /// their faith; their records still clear, the target is
    /// theirs now). Drained monks' CI progress resets with the
    /// rest; non-drained participants keep theirs (MNK-07's
    /// counter is not a success penalty).
    fn conversion_success(&mut self, monk_idx: usize, target: u32) {
        let new_owner = self.entities[monk_idx].owner;
        let participants: Vec<usize> = (0..self.entities.len())
            .filter(|&i| {
                let e = &self.entities[i];
                e.kind == EntityKind::Monk
                    && !e.dead
                    && e.owner == new_owner
                    && e.conversion_target == Some(target)
            })
            .collect();
        // MNK-11 (Castle P5c): Heresy — checked against the
        // VICTIM owner's tech set at the success moment. The
        // conversion "succeeded" (the participants below still
        // rest), but a victim whose owner holds Heresy DIES
        // instead of changing sides: its HP is zeroed and the
        // ordinary death sweep (CMB-14, next tick's combat
        // phase — the monk phase runs after combat) takes it,
        // corpse and counters included. The kill is attributed
        // to the converting owner for VIC-04, exactly as the
        // transfer arm credits converted value; no killer ENTITY
        // is named, so no carcass-harvest side path opens.
        let victim_owner = self
            .entity_index(target)
            .map(|ti| self.entities[ti].owner)
            .unwrap_or(new_owner);
        if victim_owner != new_owner && self.heresy_researched(victim_owner) {
            if let Some(ti) = self.entity_index(target) {
                let e = &mut self.entities[ti];
                e.hp = I16F16::ZERO;
                e.last_hit_by = None;
                e.last_hit_owner = Some(new_owner);
            }
        } else {
            self.transfer_ownership(target, new_owner);
        }
        self.rest_participants(&participants, new_owner, false);
    }

    /// Building conversion success (Castle P5c, MNK-04 + MNK-11):
    /// the building routes through `transfer_building_ownership`
    /// — or, when the victim's owner holds Heresy, it is
    /// DESTROYED instead (Passo 0.1: the wiki Heresy article —
    /// "Converted buildings are destroyed" — sides with the
    /// dossier against the rulebook's earlier "buildings are
    /// exempt" line, which this slice's commit corrects with the
    /// citation). Destruction rides `destroy_building`, so the
    /// garrison ejects unharmed (GAR-03) and the footprint frees
    /// exactly as a razed building's does; the VIC-04 credit and
    /// the INF-11 building counters are recorded for the
    /// converting owner first, mirroring the lethal-hit path in
    /// `damage_building`. Participants rest exactly as in the
    /// unit arm (MNK-06).
    fn conversion_building_success(&mut self, monk_idx: usize, building: u32) {
        let new_owner = self.entities[monk_idx].owner;
        let participants: Vec<usize> = (0..self.entities.len())
            .filter(|&i| {
                let e = &self.entities[i];
                e.kind == EntityKind::Monk
                    && !e.dead
                    && e.owner == new_owner
                    && e.conversion_target_building == Some(building)
            })
            .collect();
        let victim_owner = self
            .building_index(building)
            .map(|bi| self.buildings[bi].owner)
            .unwrap_or(new_owner);
        if victim_owner != new_owner && self.heresy_researched(victim_owner) {
            let value = self
                .building_index(building)
                .map(|bi| self.buildings[bi].kind.total_cost().to_num::<i32>() as i64)
                .unwrap_or(0);
            self.credit_destruction(new_owner, victim_owner, value);
            self.note_building_destroyed(new_owner, victim_owner);
            self.destroy_building(building);
        } else {
            self.transfer_building_ownership(building, new_owner);
        }
        self.rest_participants(&participants, new_owner, true);
    }

    /// MNK-06's shared rest, factored for the unit and building
    /// success arms: every participant's record clears; ALL
    /// participants drain faith to 0 — unless the converting
    /// owner has Theocracy, in which case only the lowest-id
    /// participant drains (the others keep their faith; their
    /// records still clear). Drained monks' CI progress resets
    /// with the rest; non-drained participants keep theirs.
    /// `building` selects which target field the records clear.
    fn rest_participants(&mut self, participants: &[usize], new_owner: u8, building: bool) {
        let theocracy = self.theocracy_researched(new_owner);
        for (n, &pi) in participants.iter().enumerate() {
            let drains = !theocracy || n == 0;
            let e = &mut self.entities[pi];
            if building {
                e.conversion_target_building = None;
            } else {
                e.conversion_target = None;
            }
            e.ci_ticks = 0;
            if drains {
                e.faith = 0;
                e.faith_remainder = 0;
                e.ci_progress = 0;
            }
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
                // it heals nobody, converts nobody, and any task
                // it held is over (CI progress survives — MNK-07
                // names only the manual move as a reset).
                let e = &mut self.entities[idx];
                e.heal_target = None;
                e.heal_remainder = 0;
                e.conversion_target = None;
                e.conversion_target_building = None;
                e.ci_ticks = 0;
                // Castle P5d: a garrisoned monk runs no relic
                // errand either — but a relic he CARRIES stays
                // carried (it shelters with him, and its
                // position stays his stored position).
                e.relic_pickup_target = None;
                e.relic_deposit_target = None;
                continue;
            }
            // Castle P5b phase order per monk (plan §3.3.1):
            // faith regen → conversion advance → heal advance.
            // Castle P5d extends the order with the plan's
            // fourth item — relic bookkeeping — between
            // conversion and heal: a monk walking a relic
            // errand is busy with it, exactly as a converting
            // monk is busy chanting.
            self.regen_faith(idx);
            if self.conversion_step(idx) {
                continue;
            }
            if self.relic_step(idx) {
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

    // =================================================================
    // Castle P5d — the relic lifecycle (MNK-09 / ECO-16)
    //
    // The relic is an entity (kind tag 21, Gaia-owned); its state
    // lives in two pointers on its own record and is mirrored by
    // the monk's `carrying_relic` and the Monastery's `relics`
    // list — three views of one fact, written together at every
    // transition so they cannot drift apart:
    //
    //   pickup (ground)      monk takes it: carried_by=monk,
    //                        carrying=relic, positions sync
    //   deposit              into an own Monastery: garrisoned_in=
    //                        monastery, list pushes, carry clears,
    //                        the ECO-16 trickle starts
    //   retrieve             reverse of deposit, out of an own
    //                        Monastery: the trickle stops
    //   carrier dies         combat's death sweep drops it on the
    //                        ground where he died (MNK-09)
    //   Monastery destroyed  combat's destroy path drops its
    //                        store on the ground at its position
    //
    // Pickup/deposit ride direct orders only (the P5 plan §2:
    // no queued forms — a queued Move onto a relic tile stays a
    // plain move, as DE treats shift-click monks).
    // =================================================================

    /// MNK-09 read: a relic entity's lifecycle state, derived
    /// from its own pointers. `None` when the id is not a relic.
    pub fn relic_state(&self, relic: u32) -> Option<RelicState> {
        let idx = self.entity_index(relic)?;
        let e = &self.entities[idx];
        if e.kind != EntityKind::Relic {
            return None;
        }
        Some(match (e.relic_carried_by, e.relic_garrisoned_in) {
            (Some(m), _) => RelicState::CarriedBy(m),
            (None, Some(b)) => RelicState::GarrisonedIn(b),
            (None, None) => RelicState::Ground,
        })
    }

    /// MNK-09 read: the relic a monk carries, if any — the
    /// client's "show it being carried" read and the AI's
    /// deliver/don't-convert check.
    pub fn carrying_relic(&self, entity: u32) -> Option<u32> {
        self.entity_index(entity)
            .map(|idx| self.entities[idx].carrying_relic)
            .filter(|c| c.is_some())
            .flatten()
    }

    /// MNK-09 read: how many relics a Monastery holds (the
    /// client panel's count). 0 for anything else.
    pub fn monastery_relic_count(&self, building: u32) -> u32 {
        self.building_index(building)
            .map(|bi| self.buildings[bi].relics.len() as u32)
            .unwrap_or(0)
    }

    /// The flat relic census for the client's side channel:
    /// every live relic entity with its state, in id order —
    /// (relic id, state, holder), holder being the carrying
    /// monk or the holding Monastery (0 on the ground).
    pub fn relic_states(&self) -> Vec<(u32, RelicState)> {
        self.entities
            .iter()
            .filter(|e| e.kind == EntityKind::Relic && !e.dead)
            .map(|e| {
                (e.id, match (e.relic_carried_by, e.relic_garrisoned_in) {
                    (Some(m), _) => RelicState::CarriedBy(m),
                    (None, Some(b)) => RelicState::GarrisonedIn(b),
                    (None, None) => RelicState::Ground,
                })
            })
            .collect()
    }

    /// Shared read-side check for an orderable fielded monk
    /// (the heal order's checks): living, Monk, player-owned,
    /// not garrisoned, not dead.
    fn relic_monk_ok(&self, entity: u32) -> Option<usize> {
        let idx = self.entity_index(entity)?;
        let e = &self.entities[idx];
        if e.kind != EntityKind::Monk
            || !is_player_owner(e.owner)
            || e.garrisoned_in.is_some()
            || e.dead
        {
            return None;
        }
        Some(idx)
    }

    /// `Command::PickupRelic` validation (MNK-09): a living,
    /// fielded, player-owned monk not already carrying, against
    /// a relic on the ground, or a relic garrisoned in one of
    /// the monk owner's own completed Monasteries (the DE
    /// retrieve order). A relic carried by anyone — including a
    /// monk of the same owner — cannot be taken.
    pub fn can_pickup_relic(&self, entity: u32, relic: u32) -> bool {
        let Some(mi) = self.relic_monk_ok(entity) else {
            return false;
        };
        if self.entities[mi].carrying_relic.is_some() {
            return false;
        }
        let Some(ri) = self.entity_index(relic) else {
            return false;
        };
        let r = &self.entities[ri];
        if r.kind != EntityKind::Relic || r.dead {
            return false;
        }
        match (r.relic_carried_by, r.relic_garrisoned_in) {
            (None, None) => true,
            (None, Some(b)) => {
                let owner = self.entities[mi].owner;
                self.building_index(b).is_some_and(|bi| {
                    let bb = &self.buildings[bi];
                    bb.kind == BuildingKind::Monastery && bb.complete && bb.owner == owner
                })
            }
            _ => false,
        }
    }

    /// `Command::DepositRelic` validation (MNK-09): a carrying
    /// monk (the same orderable-monk checks) depositing into a
    /// completed Monastery of his own owner.
    pub fn can_deposit_relic(&self, entity: u32, building: u32) -> bool {
        let Some(mi) = self.relic_monk_ok(entity) else {
            return false;
        };
        if self.entities[mi].carrying_relic.is_none() {
            return false;
        }
        let owner = self.entities[mi].owner;
        self.building_index(building).is_some_and(|bi| {
            let b = &self.buildings[bi];
            b.kind == BuildingKind::Monastery && b.complete && b.owner == owner
        })
    }

    /// `Command::PickupRelic` — silence while the id is invalid
    /// or the monk is garrisoned (the heal order's contract).
    pub fn issue_pickup_relic(&mut self, entity: u32, relic: u32) {
        if !self.can_pickup_relic(entity, relic) {
            return;
        }
        let idx = self.entity_index(entity).unwrap();
        self.entities[idx].order_queue.clear();
        self.entities[idx].order_from_queue = false;
        self.apply_pickup_relic(idx, relic);
    }

    /// `Command::DepositRelic` — the same silence contract.
    pub fn issue_deposit_relic(&mut self, entity: u32, building: u32) {
        if !self.can_deposit_relic(entity, building) {
            return;
        }
        let idx = self.entity_index(entity).unwrap();
        self.entities[idx].order_queue.clear();
        self.entities[idx].order_from_queue = false;
        self.apply_deposit_relic(idx, building);
    }

    /// The mechanics of a pickup order, without the queue
    /// clearing (the apply-convert layering): every task
    /// pointer yields to the new errand, exactly like a heal
    /// order's — the monk's `order` itself is set by the chase
    /// in `relic_step` on the next tick.
    fn apply_pickup_relic(&mut self, idx: usize, relic: u32) {
        let e = &mut self.entities[idx];
        e.task = Task::Idle;
        e.task_node = None;
        e.attack_target = None;
        e.attack_building = None;
        e.strike_tick = None;
        e.attack_engaged = false;
        e.aggro_target = None;
        e.returning_home = false;
        e.attack_move = None;
        e.patrol.clear();
        e.patrol_next = 0;
        e.order_from_queue = false;
        e.garrison_target = None;
        e.auto_explore = false;
        e.auto_target = None;
        e.heal_target = None;
        e.heal_remainder = 0;
        e.conversion_target = None;
        e.conversion_target_building = None;
        e.ci_ticks = 0;
        // Defensive: a deposit errand and a pickup errand
        // never coexist (each new order replaces the last).
        e.relic_deposit_target = None;
        e.relic_pickup_target = Some(relic);
    }

    /// The mechanics of a deposit order (same layering as
    /// `apply_pickup_relic`).
    fn apply_deposit_relic(&mut self, idx: usize, building: u32) {
        let e = &mut self.entities[idx];
        e.task = Task::Idle;
        e.task_node = None;
        e.attack_target = None;
        e.attack_building = None;
        e.strike_tick = None;
        e.attack_engaged = false;
        e.aggro_target = None;
        e.returning_home = false;
        e.attack_move = None;
        e.patrol.clear();
        e.patrol_next = 0;
        e.order_from_queue = false;
        e.garrison_target = None;
        e.auto_explore = false;
        e.auto_target = None;
        e.heal_target = None;
        e.heal_remainder = 0;
        e.conversion_target = None;
        e.conversion_target_building = None;
        e.ci_ticks = 0;
        e.relic_pickup_target = None;
        e.relic_deposit_target = Some(building);
    }

    /// The standing point beside a building for a relic errand:
    /// the monk's position clamped into the footprint (the
    /// building-conversion chase geometry), so the arrival test
    /// (footprint distance ≤ MELEE_REACH, the same adjacency
    /// rule building conversion uses) can fire.
    fn relic_stand_point(
        &self,
        idx: usize,
        bi: usize,
    ) -> (I16F16, I16F16) {
        let (mx, my) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let b = &self.buildings[bi];
        let half = I16F16::from_num(b.kind.footprint_size() as i32 * 2);
        (
            mx.clamp(b.pos_x - half, b.pos_x + half),
            my.clamp(b.pos_y - half, b.pos_y + half),
        )
    }

    /// One monk's relic step (the `monk_tick` busy ladder's
    /// relic rung): sync a carried relic to its monk, then
    /// advance the errand in hand. Returns true while the monk
    /// is walking (or completing) an errand — he does not heal
    /// this tick, the MNK-01 single-task rule; a monk merely
    /// CARRYING with no errand returns false: he may still
    /// heal (MNK-09 names no healing exclusion).
    fn relic_step(&mut self, idx: usize) -> bool {
        // Carry sync: the relic rides exactly at its monk's
        // position (its world identity while carried; the
        // client draws it at the monk).
        if let Some(r) = self.entities[idx].carrying_relic {
            if let Some(ri) = self.entity_index(r) {
                let (mx, my) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
                let rel = &mut self.entities[ri];
                rel.pos_x = mx;
                rel.pos_y = my;
            }
        }
        if let Some(relic) = self.entities[idx].relic_pickup_target {
            return self.relic_pickup_step(idx, relic);
        }
        if let Some(building) = self.entities[idx].relic_deposit_target {
            // A deposit errand presupposes the carry; losing it
            // (impossible through the sim's own paths — nothing
            // takes a relic out of a living monk's hands) ends
            // the errand silently.
            if self.entities[idx].carrying_relic.is_none() {
                self.entities[idx].relic_deposit_target = None;
                return false;
            }
            return self.relic_deposit_step(idx, building);
        }
        false
    }

    /// The pickup errand's advance: chase the ground relic and
    /// take it inside `RELIC_TOUCH`, or walk to an own
    /// Monastery and lift the relic out at adjacency. A target
    /// that stops being takeable mid-errand (taken by another
    /// monk, the Monastery lost) ends the errand in place —
    /// the DE stand-and-wait behavior for a spent order.
    fn relic_pickup_step(&mut self, idx: usize, relic: u32) -> bool {
        let monk_id = self.entities[idx].id;
        let monk_owner = self.entities[idx].owner;
        let Some(ri) = self.entity_index(relic) else {
            self.entities[idx].relic_pickup_target = None;
            return false;
        };
        let (carried_by, garrisoned_in) = (
            self.entities[ri].relic_carried_by,
            self.entities[ri].relic_garrisoned_in,
        );
        if carried_by.is_some() {
            // Someone else's hands now (or, impossibly, our
            // own — completion clears the errand itself).
            self.entities[idx].relic_pickup_target = None;
            return false;
        }
        match garrisoned_in {
            None => {
                // Ground: the touch test, else chase.
                let (mx, my) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
                let (rx, ry) = (self.entities[ri].pos_x, self.entities[ri].pos_y);
                if steer::vec_len(rx - mx, ry - my) <= RELIC_TOUCH {
                    let rel = &mut self.entities[ri];
                    rel.relic_carried_by = Some(monk_id);
                    rel.pos_x = mx;
                    rel.pos_y = my;
                    let e = &mut self.entities[idx];
                    e.carrying_relic = Some(relic);
                    e.relic_pickup_target = None;
                    e.order = None;
                    return true;
                }
                // Chase in the heal/conversion pattern: the
                // walk is (re)installed only when there is no
                // order yet, or the standing order is not headed
                // at the relic — a walking monk does not re-path
                // every tick.
                let stale = match &self.entities[idx].order {
                    None => true,
                    Some(o) => {
                        steer::vec_len(o.target_x - rx, o.target_y - ry)
                            > I16F16::from_num(2)
                    }
                };
                if stale {
                    self.set_move_order(idx, rx, ry);
                }
                true
            }
            Some(b) => {
                // Retrieve: still ours to take?
                let Some(bi) = self.building_index(b) else {
                    self.entities[idx].relic_pickup_target = None;
                    return false;
                };
                let (bx, by, bkind, bowner, bcomplete) = {
                    let bb = &self.buildings[bi];
                    (bb.pos_x, bb.pos_y, bb.kind, bb.owner, bb.complete)
                };
                if bkind != BuildingKind::Monastery || !bcomplete || bowner != monk_owner {
                    self.entities[idx].relic_pickup_target = None;
                    return false;
                }
                let (mx, my) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
                if self.distance_to_footprint(mx, my, bx, by, bkind) <= MELEE_REACH {
                    // Take it out: the store, the pointers, the
                    // carry — written together (the lifecycle
                    // invariant). The ECO-16 trickle stops with
                    // the removal (its count reads this list).
                    self.buildings[bi].relics.retain(|&id| id != relic);
                    let (mx2, my2) = (mx, my);
                    let rel = &mut self.entities[ri];
                    rel.relic_garrisoned_in = None;
                    rel.relic_carried_by = Some(monk_id);
                    rel.pos_x = mx2;
                    rel.pos_y = my2;
                    let e = &mut self.entities[idx];
                    e.carrying_relic = Some(relic);
                    e.relic_pickup_target = None;
                    e.order = None;
                    return true;
                }
                let (qx, qy) = self.relic_stand_point(idx, bi);
                let stale = match &self.entities[idx].order {
                    None => true,
                    Some(o) => {
                        steer::vec_len(o.target_x - qx, o.target_y - qy)
                            > I16F16::from_num(2)
                    }
                };
                if stale {
                    self.set_move_order(idx, qx, qy);
                }
                true
            }
        }
    }

    /// The deposit errand's advance: walk to the own Monastery
    /// and garrison the relic at adjacency (the pickup step's
    /// mirror). The Monastery record takes the relic; the
    /// relic's world position becomes the Monastery's (it is
    /// off the map while garrisoned — clients draw garrisoned
    /// relics only as the panel count).
    fn relic_deposit_step(&mut self, idx: usize, building: u32) -> bool {
        let monk_id = self.entities[idx].id;
        let monk_owner = self.entities[idx].owner;
        let Some(relic) = self.entities[idx].carrying_relic else {
            self.entities[idx].relic_deposit_target = None;
            return false;
        };
        let Some(bi) = self.building_index(building) else {
            self.entities[idx].relic_deposit_target = None;
            return false;
        };
        let (bx, by, bkind, bowner, bcomplete) = {
            let b = &self.buildings[bi];
            (b.pos_x, b.pos_y, b.kind, b.owner, b.complete)
        };
        if bkind != BuildingKind::Monastery || !bcomplete || bowner != monk_owner {
            self.entities[idx].relic_deposit_target = None;
            return false;
        }
        let (mx, my) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        if self.distance_to_footprint(mx, my, bx, by, bkind) <= MELEE_REACH {
            self.buildings[bi].relics.push(relic);
            if let Some(ri) = self.entity_index(relic) {
                let rel = &mut self.entities[ri];
                rel.relic_carried_by = None;
                rel.relic_garrisoned_in = Some(building);
                rel.pos_x = bx;
                rel.pos_y = by;
            }
            let e = &mut self.entities[idx];
            e.carrying_relic = None;
            e.relic_deposit_target = None;
            e.order = None;
            let _ = monk_id;
            return true;
        }
        let (qx, qy) = self.relic_stand_point(idx, bi);
        let stale = match &self.entities[idx].order {
            None => true,
            Some(o) => {
                steer::vec_len(o.target_x - qx, o.target_y - qy)
                    > I16F16::from_num(2)
            }
        };
        if stale {
            self.set_move_order(idx, qx, qy);
        }
        true
    }

    /// The ECO-16 trickle (Castle P5d), run once per tick after
    /// the monk phase: every COMPLETED Monastery holding relics
    /// pays its owner 30 gold per minute per relic —
    /// continuously, until a relic is removed (MNK-09 retrieve
    /// or the Monastery's destruction). The accumulator is the
    /// GAR-04/Treadmill pattern on the building record: a
    /// Monastery with no relics accrues nothing and writes
    /// nothing (determinism discipline: no new hash input for
    /// stateless buildings — the hash block itself is
    /// conditional on a non-empty store or remainder).
    ///
    /// Per tick per relic: 30 gold × 65536 bits ÷ 3600 ticks.
    /// The remainder stays < 3600, so any whole minute pays
    /// exactly 30 gold per relic — no rounding drift.
    pub(crate) fn relic_tick(&mut self) {
        for bi in 0..self.buildings.len() {
            let b = &self.buildings[bi];
            if b.kind != BuildingKind::Monastery || !b.complete || b.relics.is_empty() {
                continue;
            }
            let total = self.buildings[bi].relic_gold_remainder
                + self.buildings[bi].relics.len() as i64 * RELIC_GOLD_PER_MIN * 65_536;
            let pay = total / 3_600;
            self.buildings[bi].relic_gold_remainder = total % 3_600;
            if pay > 0 {
                let owner = self.buildings[bi].owner;
                self.stockpile_mut(owner).add(ResourceKind::Gold, I16F16::from_bits(pay as i32));
            }
        }
    }
}
