//! Thin WebAssembly ABI (M1–M2).
//!
//! Compiled only for `wasm32-unknown-unknown`. This is the narrow boundary
//! between the headless sim and the outside world: commands in, state out.
//! The renderer (M4) — and the debug preview — talk *only* through this
//! surface. The renderer must never compute game rules.
//!
//! M1 exposes: create, tick, state hash, move orders, and read-only state
//! accessors (entity positions, active paths, grid layout) for rendering.
//! M2 adds: gather/train commands, stockpile, node list, entity kinds,
//! villager and training-queue counts.
//! M2.5 adds: build commands (House/Farm), the building list, and
//! population / training-block accessors.
//! M3 adds: attack orders, the Barracks build command, Barracks
//! training (Militia/Archer) with its block-reason query, entity
//! HP/owner arrays, and the projectile list.
//! M4 adds: queued orders (SEL-12 — the `*_queued` variants of the
//! move/gather/attack/build commands), rally points (SEL-13 —
//! set/get for the Town Center and production buildings), and the
//! idle lists behind the idle keys and indicators (SEL-08/09,
//! INF-03).
//! M5 adds: match construction (`new_match` — Sandbox stays the
//! plain constructor), the match-state getter (Conquest), the
//! per-player stockpile getter, building attack orders, and owner +
//! HP columns on the building list (buildings take damage now).
//! M6 adds: the six new build commands (camps, Archery Range,
//! Stable, Blacksmith) with their queued variants, Scout training
//! (the Stable), research commands and block-reason queries for
//! the Town Center and placed buildings, the age/tech getters, the
//! typed queue arrays (units and research share the queues), the
//! Town Center's head-progress getter, and force drop-off. Entity
//! HP maximums are now the owner's *effective* maxima (Loom).
//! M7: the world channels — `terrain()` and `heights()` flatten
//! the generated map's layers (row-major, one byte per cell,
//! matching `obstacles()`' indexing), and `fog()` serves the LOCAL
//! player's visibility grid (0 unexplored / 1 explored / 2 visible)
//! — the only knowledge the client may render from. `obstacles()`
//! now means water ∪ standing trees ∪ placed buildings
//! (MAP-03/04), so its existing consumers follow the world.
//! Repair (BLD-07) adds: `push_repair` and its queued form
//! `push_repair_queued` — a villager order against a placed
//! building id, like `push_attack_building`'s shape.

use wasm_bindgen::prelude::*;

use fixed::types::I16F16;

use crate::build::BuildingKind;
use crate::econ::ResourceKind;
use crate::entity::EntityKind;
use crate::grid;
use crate::tech::TechId;
use crate::{Command, MatchMode, QueuedOrder, Sim, Stance};

/// A `Sim` owned by JavaScript. One instance per match.
#[wasm_bindgen]
pub struct WasmSim {
    inner: Sim,
}

#[wasm_bindgen]
impl WasmSim {
    /// Create a simulation from a seed.
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u32) -> WasmSim {
        WasmSim {
            inner: Sim::new(seed as u64),
        }
    }

    /// Create a match against the scripted AI opponent (M5):
    /// `mode_tag` 1 = AI Easy, 2 = AI Normal, 3 = Two-player (M8)
    /// (the `MatchMode` tags;
    /// 0 or anything unknown falls back to the plain sandbox
    /// constructor's game). The opponent spawns mirrored across the
    /// map and plays through the ordinary command queue from the
    /// first tick.
    #[wasm_bindgen]
    pub fn new_match(seed: u32, mode_tag: u32) -> WasmSim {
        let mode = MatchMode::from_tag(mode_tag as u8).unwrap_or(MatchMode::Sandbox);
        WasmSim {
            inner: Sim::with_mode(seed as u64, mode),
        }
    }

    /// The match mode this sim was created with (M5): 0 sandbox,
    /// 1 AI Easy, 2 AI Normal.
    pub fn mode(&self) -> u32 {
        self.inner.mode().tag() as u32
    }

    /// The match outcome (M5, Conquest): 0 ongoing, 1 victory,
    /// 2 defeat — from the human player's seat. Always 0 in
    /// Sandbox. The client polls this; the sim keeps ticking
    /// harmlessly after a decision.
    pub fn match_state(&self) -> u32 {
        self.inner.match_state().tag() as u32
    }

    /// Advance exactly one 60 Hz tick. Returns the per-tick state hash
    /// as a 16-character hex string.
    pub fn tick(&mut self) -> String {
        let hash = self.inner.tick();
        format!("{:016x}", hash)
    }

    /// Ticks elapsed. Returned as u32 (wraps after ~2.26 years at 60 Hz —
    /// fine for the prototype; widen if a match ever needs more).
    pub fn tick_count(&self) -> u32 {
        self.inner.tick_count() as u32
    }

    /// Placeholder command input.
    pub fn push_noop(&mut self) {
        self.inner.push_command(Command::Noop);
    }

    /// Order one entity to move to a world point.
    ///
    /// Floats are accepted ONLY here at the JS boundary and converted to
    /// fixed-point immediately — they never enter the sim.
    pub fn push_move(&mut self, entity: u32, x: f64, y: f64) {
        self.inner.push_command(Command::Move {
            entity,
            x: I16F16::from_num(x),
            y: I16F16::from_num(y),
        });
    }

    /// Order a villager to work a resource node (gather → haul → deposit
    /// → repeat). Ids only — no floats cross this boundary here.
    pub fn push_gather(&mut self, entity: u32, node: u32) {
        self.inner.push_command(Command::Gather { entity, node });
    }

    /// Enqueue a villager at the Town Center (50 food, deducted when the
    /// command is applied next tick). Returns whether the order *would*
    /// be accepted against the current state — a UI affordance; the sim
    /// re-validates when the command is applied.
    pub fn push_train_villager(&mut self) -> bool {
        let ok = self.inner.can_train_villager();
        if ok {
            self.inner.push_command(Command::TrainVillager);
        }
        ok
    }

    /// Order a unit to attack a target entity (M3): chase and strike
    /// until the target dies or a new order lands. Returns whether
    /// the order *would* be accepted against the current state (the
    /// sim re-validates when the command is applied).
    pub fn push_attack(&mut self, entity: u32, target: u32) -> bool {
        let ok = self.inner.can_attack(entity, target);
        if ok {
            self.inner.push_command(Command::Attack { entity, target });
        }
        ok
    }

    /// Order a unit to attack a placed building (M5): chase its
    /// footprint and strike until it is destroyed or a new order
    /// lands. Only another player's building is a valid target.
    /// Returns whether the order *would* be accepted.
    pub fn push_attack_building(&mut self, entity: u32, building: u32) -> bool {
        let ok = self.inner.can_attack_building(entity, building);
        if ok {
            self.inner
                .push_command(Command::AttackBuilding { entity, building });
        }
        ok
    }

    /// Order a villager to repair one of its own player's placed
    /// buildings (BLD-07): travel to the building's ring and
    /// restore its HP at the documented flat rate (750 HP/min for
    /// the first repairer, +375 per additional), charged as a wood
    /// drip totaling 50% of the build cost over a full repair.
    /// Only complete, damaged buildings of the villager's own
    /// player are valid targets. Returns whether the order *would*
    /// be accepted against the current state (the sim re-validates
    /// when the command is applied).
    pub fn push_repair(&mut self, entity: u32, building: u32) -> bool {
        let ok = self.inner.can_repair(entity, building);
        if ok {
            self.inner
                .push_command(Command::Repair { entity, building });
        }
        ok
    }

    /// Enqueue a Militia at a completed Barracks (M3). Returns
    /// whether the order *would* be accepted (queue room + funds);
    /// the cost is deducted when the command is applied next tick.
    pub fn push_train_militia(&mut self, building: u32) -> bool {
        self.push_train_at(building, EntityKind::Militia)
    }

    /// Enqueue an Archer at a completed Archery Range (M6 — the
    /// Range is the Archer's home since M6; in M3 the Barracks
    /// trained them by brief). See `push_train_militia`.
    pub fn push_train_archer(&mut self, building: u32) -> bool {
        self.push_train_at(building, EntityKind::Archer)
    }

    /// Enqueue a Scout Cavalry at a completed Stable (M6). See
    /// `push_train_militia`.
    pub fn push_train_scout(&mut self, building: u32) -> bool {
        self.push_train_at(building, EntityKind::ScoutCavalry)
    }

    /// Enqueue a Spearman at a completed Barracks (Feudal Age —
    /// `can_train_at` carries the unit-level age gate). See
    /// `push_train_militia`.
    pub fn push_train_spearman(&mut self, building: u32) -> bool {
        self.push_train_at(building, EntityKind::Spearman)
    }

    /// Enqueue a Skirmisher at a completed Archery Range. See
    /// `push_train_militia`.
    pub fn push_train_skirmisher(&mut self, building: u32) -> bool {
        self.push_train_at(building, EntityKind::Skirmisher)
    }

    fn push_train_at(&mut self, building: u32, kind: EntityKind) -> bool {
        let ok = self.inner.can_train_at(building, kind);
        if ok {
            self.inner.push_command(Command::TrainAt { building, kind });
        }
        ok
    }

    /// Why training `kind_tag` (3 militia, 4 archer, 2 scout) at
    /// `building` cannot currently proceed: same tags as
    /// `train_block_reason` (0 ok, 1 cannot afford, 2 queue full /
    /// not trainable here, 3 housed). UI affordance only.
    pub fn building_train_block(&self, building: u32, kind_tag: u32) -> u32 {
        match EntityKind::from_tag(kind_tag as u8) {
            Some(kind) => self.inner.train_at_block(building, kind).tag() as u32,
            None => 2,
        }
    }

    // --- M6: research ---------------------------------------------
    // Research rides the same enqueue pattern as training: the
    // cost is charged when the command applies, the item occupies
    // its host's shared queue (TEC-03/05).

    /// Enqueue Loom at the Town Center (50 gold, 25 s). Returns
    /// whether the order *would* be accepted against current state.
    pub fn push_research_loom(&mut self) -> bool {
        self.push_research_tc(TechId::Loom)
    }

    /// Enqueue the Feudal Age advancement at the Town Center
    /// (500 food, 130 s; needs 2 completed Dark-Age buildings).
    pub fn push_research_feudal(&mut self) -> bool {
        self.push_research_tc(TechId::FeudalAge)
    }

    /// Enqueue Wheelbarrow at the Town Center (175 food, 50 wood,
    /// 75 s; Feudal Age — ECO-03).
    pub fn push_research_wheelbarrow(&mut self) -> bool {
        self.push_research_tc(TechId::Wheelbarrow)
    }

    /// Enqueue the Castle Age advancement at the Town Center
    /// (800 food, 200 gold, 160 s; needs 2 completed Feudal-Age
    /// buildings — TEC-01 Castle rung).
    pub fn push_research_castle_age(&mut self) -> bool {
        self.push_research_tc(TechId::CastleAge)
    }

    /// Enqueue Hand Cart at the Town Center (300 food, 200 wood,
    /// 55 s; Castle Age, requires Wheelbarrow — ECO-03).
    pub fn push_research_hand_cart(&mut self) -> bool {
        self.push_research_tc(TechId::HandCart)
    }

    fn push_research_tc(&mut self, tech: TechId) -> bool {
        let ok = self.inner.can_research_tc(tech);
        if ok {
            self.inner.push_command(Command::ResearchTc { tech });
        }
        ok
    }

    /// Enqueue a technology at a placed building by tech tag
    /// (2 fletching, 3 scale mail, 4 padded archer armor at the
    /// Blacksmith; 5 man-at-arms at the Barracks). Returns whether
    /// the order *would* be accepted.
    pub fn push_research_at(&mut self, building: u32, tech_tag: u32) -> bool {
        let Some(tech) = TechId::from_tag(tech_tag as u8) else {
            return false;
        };
        let ok = self.inner.can_research_at(building, tech);
        if ok {
            self.inner
                .push_command(Command::ResearchAt { building, tech });
        }
        ok
    }

    /// Why research `tech_tag` at the Town Center cannot currently
    /// proceed: 0 ok, 1 cannot afford, 2 queue full / not a Town
    /// Center technology, 3 already researched or queued,
    /// 4 requirements not met. UI affordance only.
    pub fn research_block_tc(&self, tech_tag: u32) -> u32 {
        match TechId::from_tag(tech_tag as u8) {
            Some(tech) => self.inner.research_block_tc(tech).tag() as u32,
            None => 2,
        }
    }

    /// Why research `tech_tag` at `building` cannot currently
    /// proceed: same tags as `research_block_tc`. UI affordance.
    pub fn research_block_at(&self, building: u32, tech_tag: u32) -> u32 {
        match TechId::from_tag(tech_tag as u8) {
            Some(tech) => self.inner.research_block_at(building, tech).tag() as u32,
            None => 2,
        }
    }

    /// The human player's current age (M6): 0 Dark Age, 1 Feudal.
    pub fn age(&self) -> u32 {
        self.inner.age_of(crate::entity::OWNER_PLAYER).tag() as u32
    }

    /// The human player's researched-technology bitmask (M6): bit
    /// per tech tag (`TechId::tag`); the Feudal bit reads set while
    /// the player stands in the Feudal Age.
    pub fn tech_mask(&self) -> u32 {
        self.inner.tech_mask_of(crate::entity::OWNER_PLAYER) as u32
    }

    /// The Town Center's queue as flat entry codes, head first
    /// (M6): unit kinds keep their entity tag; technologies are
    /// `100 + tech tag`. Behind the panel's queue display.
    pub fn tc_queue(&self) -> Vec<u32> {
        self.inner.tc_queue().iter().map(|e| e.code()).collect()
    }

    /// A placed building's queue as flat entry codes (M6) — see
    /// `tc_queue`.
    pub fn building_queue(&self, building: u32) -> Vec<u32> {
        self.inner
            .building_queue(building)
            .iter()
            .map(|e| e.code())
            .collect()
    }

    /// Progress on the Town Center queue's head entry, 0..1 (M6).
    pub fn tc_progress(&self) -> f64 {
        I16F16::from_bits(self.inner.tc_progress_bits()).to_num::<f64>()
    }

    /// Force drop-off (M6, ECO-07): order a loaded villager to haul
    /// its carry to the nearest valid drop-off now, then return to
    /// its previous work on its own. Returns whether the order
    /// *would* be accepted (carrying something, somewhere to go).
    pub fn push_force_deposit(&mut self, entity: u32) -> bool {
        let ok = self.inner.can_force_deposit(entity);
        if ok {
            self.inner.push_command(Command::ForceDeposit { entity });
        }
        ok
    }

    /// Toggle AutoExplore (SEL-17) on a player's Scout Cavalry:
    /// with `enabled` the scout explores its owner's fog frontier
    /// on its own; disabling stops it where it stands. Returns
    /// whether the order *would* be accepted (a living Scout
    /// Cavalry owned by a player — every scout is eligible,
    /// starting and trained alike); the sim re-validates when the
    /// command is applied.
    pub fn push_auto_explore(&mut self, entity: u32, enabled: bool) -> bool {
        let ok = self.inner.can_auto_explore(entity);
        if ok {
            self.inner.push_command(Command::SetAutoExplore { entity, enabled });
        }
        ok
    }

    /// Attack-move (CMB-10): the unit walks to the point,
    /// engaging whatever enters its sight en route, then resumes
    /// and holds at the destination. Returns whether the order
    /// *would* be accepted (a living military unit owned by a
    /// player, with an attack row); the sim re-validates when the
    /// command is applied.
    pub fn push_attack_move(&mut self, entity: u32, x: f64, y: f64) -> bool {
        let ok = self.inner.can_attack_move(entity);
        if ok {
            self.inner.push_command(Command::AttackMove {
                entity,
                x: I16F16::from_num(x),
                y: I16F16::from_num(y),
            });
        }
        ok
    }

    /// Patrol (CMB-11): the unit loops between where it stands and
    /// the point, engaging en route like an attack-move; on a unit
    /// that already patrols, the point is appended to its loop.
    /// Returns whether the order *would* be accepted (same rule as
    /// `push_attack_move`); the sim re-validates at apply time.
    pub fn push_patrol(&mut self, entity: u32, x: f64, y: f64) -> bool {
        let ok = self.inner.can_attack_move(entity);
        if ok {
            self.inner.push_command(Command::Patrol {
                entity,
                x: I16F16::from_num(x),
                y: I16F16::from_num(y),
            });
        }
        ok
    }

    /// Set a military unit's stance (CMB-09) by tag (0 Aggressive,
    /// 1 Defensive, 2 Stand Ground, 3 No Attack). A stance is a
    /// setting, not an order — nothing else about the unit
    /// changes. Returns whether the order *would* be accepted (a
    /// living military unit owned by a player, and a valid tag);
    /// the sim re-validates when the command is applied.
    pub fn push_set_stance(&mut self, entity: u32, stance: u8) -> bool {
        let Some(stance) = Stance::from_tag(stance) else {
            return false;
        };
        let ok = self.inner.can_set_stance(entity);
        if ok {
            self.inner
                .push_command(Command::SetStance { entity, stance });
        }
        ok
    }

    /// Stop a unit's current order immediately (SEL-14): it
    /// stands idle where it is; a villager keeps its carry.
    /// Returns whether the order *would* be accepted (any living,
    /// player-owned, mobile entity); the sim re-validates when
    /// the command is applied.
    pub fn push_stop(&mut self, entity: u32) -> bool {
        let ok = self.inner.can_stop(entity);
        if ok {
            self.inner.push_command(Command::Stop { entity });
        }
        ok
    }

    /// Delete a unit deliberately (SEL-14): any living entity
    /// owned by a player, the Town Center included. It dies this
    /// tick through the ordinary death sweep; an animal leaves
    /// its carcass. Returns whether the order *would* be
    /// accepted; the sim re-validates at apply time.
    pub fn push_delete(&mut self, entity: u32) -> bool {
        let ok = self.inner.can_delete_entity(entity);
        if ok {
            self.inner
                .push_command(Command::DeleteEntity { entity });
        }
        ok
    }

    /// Delete a placed building deliberately (SEL-14): a
    /// foundation is a cancellation and refunds its full wood
    /// cost (BLD-02); a completed building refunds nothing.
    /// Returns whether the order *would* be accepted; the sim
    /// re-validates at apply time.
    pub fn push_delete_building(&mut self, building: u32) -> bool {
        let ok = self.inner.can_delete_building(building);
        if ok {
            self.inner
                .push_command(Command::DeleteBuilding { building });
        }
        ok
    }

    /// Cancel one entry of a Town Center's production queue
    /// (TEC-05), refunding its full cost to the TC's owner.
    /// Returns whether the order *would* be accepted; the sim
    /// re-validates at apply time.
    pub fn push_cancel_queue_tc(&mut self, tc: u32, index: u32) -> bool {
        let ok = self.inner.can_cancel_queue_tc(tc, index);
        if ok {
            self.inner
                .push_command(Command::CancelQueueTc { tc, index });
        }
        ok
    }

    /// Cancel one entry of a placed building's production queue
    /// (TEC-05), refunding its full cost to the building's owner.
    /// Returns whether the order *would* be accepted; the sim
    /// re-validates at apply time.
    pub fn push_cancel_queue_building(&mut self, building: u32, index: u32) -> bool {
        let ok = self.inner.can_cancel_queue_building(building, index);
        if ok {
            self.inner.push_command(Command::CancelQueueBuilding {
                building,
                index,
            });
        }
        ok
    }

    /// Per-entity stance tags in entity-id order (CMB-09:
    /// 0 Aggressive, 1 Defensive, 2 Stand Ground, 3 No Attack) —
    /// the command panel's pressed state for the stance row.
    pub fn entity_stance(&self) -> Vec<u8> {
        self.inner
            .entities()
            .iter()
            .map(|e| e.stance.tag())
            .collect()
    }

    /// Per-entity AutoExplore flags (0/1) in entity-id order — the
    /// command panel's pressed state for the toggle button.
    pub fn entity_auto_explore(&self) -> Vec<u8> {
        self.inner
            .entities()
            .iter()
            .map(|e| u8::from(e.auto_explore))
            .collect()
    }

    /// Could AutoExplore (SEL-17) be toggled on this entity right
    /// now — a living Scout Cavalry owned by a player? The client
    /// mirrors its button placement and the G key from this query
    /// (it replaces the old starting-scout id lookup: eligibility
    /// is a per-entity rule now, not a spawn-order identity).
    pub fn entity_can_auto_explore(&self, entity: u32) -> bool {
        self.inner.can_auto_explore(entity)
    }

    /// Order a villager to build a House with its 2×2-cell footprint
    /// anchored at the cell containing the world point `(x, y)`. The
    /// 25-wood cost is deducted at placement (next tick). Returns
    /// whether the order *would* be accepted against the current state
    /// (same pattern as `push_train_villager`); the sim re-validates
    /// when the command is applied.
    pub fn push_build_house(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::House, x, y)
    }

    /// Order a villager to build a Farm (60 wood) — see
    /// `push_build_house`.
    pub fn push_build_farm(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::Farm, x, y)
    }

    /// Order a villager to build a Barracks (175 wood, 3×3-cell
    /// footprint) — see `push_build_house`.
    pub fn push_build_barracks(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::Barracks, x, y)
    }

    /// Order a villager to build a Lumber Camp (100 wood, 2×2 —
    /// M6; the wood drop-off) — see `push_build_house`.
    pub fn push_build_lumber_camp(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::LumberCamp, x, y)
    }

    /// Order a villager to build a Mill (100 wood, 2×2 — M6; the
    /// food drop-off, and the Farm prerequisite) — see
    /// `push_build_house`.
    pub fn push_build_mill(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::Mill, x, y)
    }

    /// Order a villager to build a Mining Camp (100 wood, 2×2 —
    /// M6; the gold + stone drop-off) — see `push_build_house`.
    pub fn push_build_mining_camp(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::MiningCamp, x, y)
    }

    /// Order a villager to build an Archery Range (175 wood, 3×3 —
    /// M6; Feudal Age, requires a Barracks) — see
    /// `push_build_house`.
    pub fn push_build_archery_range(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::ArcheryRange, x, y)
    }

    /// Order a villager to build a Stable (175 wood, 3×3 — M6;
    /// Feudal Age, requires a Barracks) — see `push_build_house`.
    pub fn push_build_stable(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::Stable, x, y)
    }

    /// Order a villager to build a Blacksmith (150 wood, 3×3 — M6;
    /// Feudal Age; the technology host) — see `push_build_house`.
    pub fn push_build_blacksmith(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::Blacksmith, x, y)
    }

    /// Order a villager to build one Palisade Wall segment
    /// (3 wood, 1×1 — BLD-13) — see `push_build_house`. A dragged
    /// line of segments goes through `push_build_wall_line`.
    pub fn push_build_palisade_wall(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::PalisadeWall, x, y)
    }

    /// Order a villager to raise a dragged line of Palisade Wall
    /// segments (BLD-13) from the cell containing `(x0, y0)` to
    /// the cell containing `(x1, y1)`: the sim expands the line
    /// deterministically, validates and charges each segment
    /// (3 wood) individually — obstructed cells and segments past
    /// the stockpile are skipped — and walks the villager along
    /// the line building segment after segment. Returns whether
    /// at least one segment would take effect; the sim
    /// re-validates when the command is applied.
    pub fn push_build_wall_line(
        &mut self,
        entity: u32,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
    ) -> bool {
        let (fx0, fy0) = (I16F16::from_num(x0), I16F16::from_num(y0));
        let (fx1, fy1) = (I16F16::from_num(x1), I16F16::from_num(y1));
        let ok = self
            .inner
            .can_build_line(entity, BuildingKind::PalisadeWall, fx0, fy0, fx1, fy1);
        if ok {
            self.inner.push_command(Command::BuildLine {
                entity,
                kind: BuildingKind::PalisadeWall,
                x0: fx0,
                y0: fy0,
                x1: fx1,
                y1: fy1,
            });
        }
        ok
    /// Order a villager to build an additional Town Center
    /// (275 wood + 100 stone, 4×4 — Castle Age; the foundation
    /// converts into a Town Center entity on completion) — see
    /// `push_build_house`.
    pub fn push_build_town_center(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build(entity, BuildingKind::TownCenter, x, y)
    }

    fn push_build(&mut self, entity: u32, kind: BuildingKind, x: f64, y: f64) -> bool {
        let (fx, fy) = (I16F16::from_num(x), I16F16::from_num(y));
        let ok = self.inner.can_build(entity, kind, fx, fy);
        if ok {
            self.inner.push_command(Command::Build {
                entity,
                kind,
                x: fx,
                y: fy,
            });
        }
        ok
    }

    // --- M4: queued orders (SEL-12) --------------------------------
    // The Shift+click forms: the order is appended behind the
    // entity's current one and executes when the entity goes idle.
    // Acceptance mirrors `Sim::can_queue` (light validation — the
    // order re-validates fully when it executes).

    /// Queue a move order behind the entity's current order.
    pub fn push_move_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_queued(
            entity,
            QueuedOrder::Move {
                x: I16F16::from_num(x),
                y: I16F16::from_num(y),
            },
        )
    }

    /// Queue a gather order behind the villager's current order.
    pub fn push_gather_queued(&mut self, entity: u32, node: u32) -> bool {
        self.push_queued(entity, QueuedOrder::Gather { node })
    }

    /// Queue an attack order behind the unit's current order.
    pub fn push_attack_queued(&mut self, entity: u32, target: u32) -> bool {
        self.push_queued(entity, QueuedOrder::Attack { target })
    }

    /// Queue a repair order behind the villager's current order
    /// (BLD-07): the repair starts when the villager goes idle,
    /// and re-validates fully at execution — see `push_repair`.
    pub fn push_repair_queued(&mut self, entity: u32, building: u32) -> bool {
        self.push_queued(entity, QueuedOrder::Repair { building })
    }

    /// Queue an attack-move behind the unit's current order
    /// (CMB-10): it executes when the unit goes idle.
    pub fn push_attack_move_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_queued(
            entity,
            QueuedOrder::AttackMove {
                x: I16F16::from_num(x),
                y: I16F16::from_num(y),
            },
        )
    }

    /// Queue a patrol order behind the unit's current order
    /// (CMB-11). On a unit that already patrols, the point appends
    /// to its loop immediately instead (see `Sim::issue_queue`).
    pub fn push_patrol_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_queued(
            entity,
            QueuedOrder::Patrol {
                x: I16F16::from_num(x),
                y: I16F16::from_num(y),
            },
        )
    }

    /// Queue a House build behind the villager's current order. The
    /// wood is charged when the order *executes*, not when queued —
    /// see `Sim::can_queue`.
    pub fn push_build_house_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::House, x, y)
    }

    /// Queue a Farm build — see `push_build_house_queued`.
    pub fn push_build_farm_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::Farm, x, y)
    }

    /// Queue a Barracks build — see `push_build_house_queued`.
    pub fn push_build_barracks_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::Barracks, x, y)
    }

    /// Queue a Lumber Camp build (M6) — see `push_build_house_queued`.
    pub fn push_build_lumber_camp_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::LumberCamp, x, y)
    }

    /// Queue a Mill build (M6) — see `push_build_house_queued`.
    pub fn push_build_mill_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::Mill, x, y)
    }

    /// Queue a Mining Camp build (M6) — see `push_build_house_queued`.
    pub fn push_build_mining_camp_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::MiningCamp, x, y)
    }

    /// Queue an Archery Range build (M6) — see `push_build_house_queued`.
    pub fn push_build_archery_range_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::ArcheryRange, x, y)
    }

    /// Queue a Stable build (M6) — see `push_build_house_queued`.
    pub fn push_build_stable_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::Stable, x, y)
    }

    /// Queue a Blacksmith build (M6) — see `push_build_house_queued`.
    pub fn push_build_blacksmith_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::Blacksmith, x, y)
    }

    /// Queue a single Palisade Wall segment (BLD-13) — see
    /// `push_build_house_queued`.
    pub fn push_build_palisade_wall_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::PalisadeWall, x, y)
    /// Queue a Town Center build (Castle Age) — see
    /// `push_build_house_queued`.
    pub fn push_build_town_center_queued(&mut self, entity: u32, x: f64, y: f64) -> bool {
        self.push_build_queued(entity, BuildingKind::TownCenter, x, y)
    }

    fn push_build_queued(&mut self, entity: u32, kind: BuildingKind, x: f64, y: f64) -> bool {
        self.push_queued(
            entity,
            QueuedOrder::Build {
                kind,
                x: I16F16::from_num(x),
                y: I16F16::from_num(y),
            },
        )
    }

    fn push_queued(&mut self, entity: u32, order: QueuedOrder) -> bool {
        let ok = self.inner.can_queue(entity, order);
        if ok {
            self.inner.push_command(Command::Queue { entity, order });
        }
        ok
    }

    // --- M4: rally points (SEL-13) ---------------------------------

    /// Set the Town Center's rally point: new villagers walk there on
    /// spawn — or straight to work, when the point snaps to a live
    /// resource node (within snap range). Returns whether the order
    /// would be accepted (the Town Center always accepts).
    pub fn push_set_rally_tc(&mut self, x: f64, y: f64) -> bool {
        let ok = self.inner.can_set_rally(None);
        if ok {
            self.inner.push_command(Command::SetRally {
                building: None,
                x: I16F16::from_num(x),
                y: I16F16::from_num(y),
            });
        }
        ok
    }

    /// Set a production building's rally point (M4: the Barracks).
    /// Returns whether the building accepts a rally point.
    pub fn push_set_rally_building(&mut self, building: u32, x: f64, y: f64) -> bool {
        let ok = self.inner.can_set_rally(Some(building));
        if ok {
            self.inner.push_command(Command::SetRally {
                building: Some(building),
                x: I16F16::from_num(x),
                y: I16F16::from_num(y),
            });
        }
        ok
    }

    /// All rally points as a flat array, four values per entry —
    /// `[x, y, node, has]` — entry 0 is the Town Center, entries
    /// 1.. follow `buildings()` order. `node` is the snapped resource
    /// node id, or -1; `has` is 0 when no rally is set there (the
    /// x/y are then 0 and meaningless).
    pub fn rally_points(&self) -> Vec<f64> {
        fn push(out: &mut Vec<f64>, r: Option<(I16F16, I16F16, Option<u32>)>) {
            match r {
                Some((x, y, node)) => {
                    out.push(x.to_num::<f64>());
                    out.push(y.to_num::<f64>());
                    out.push(node.map(|n| n as f64).unwrap_or(-1.0));
                    out.push(1.0);
                }
                None => out.extend_from_slice(&[0.0, 0.0, -1.0, 0.0]),
            }
        }
        let mut out = Vec::with_capacity((self.inner.buildings().len() + 1) * 4);
        push(&mut out, self.inner.tc_rally());
        for b in self.inner.buildings() {
            push(&mut out, self.inner.building_rally(b.id));
        }
        out
    }

    /// One entity's order route as flat world-point pairs
    /// `[x0, y0, x1, y1, ...]` (INF-09): the current order's
    /// target (only while orders are queued behind it) followed
    /// by each queued order's target. Behind the client's
    /// waypoint-pennant render. Render-only, like `rally_points`.
    pub fn entity_order_route(&self, entity: u32) -> Vec<f64> {
        self.inner
            .order_route(entity)
            .iter()
            .flat_map(|(x, y)| [x.to_num::<f64>(), y.to_num::<f64>()])
            .collect()
    }

    // --- M4: idle queries (SEL-08/09, INF-03) -----------------------

    /// Ids of the idle villagers (no work task, no move order, no
    /// attack target), in id order. Behind the client's idle-villager
    /// key and the top-bar idle indicator.
    pub fn idle_villagers(&self) -> Vec<u32> {
        self.inner.idle_villagers()
    }

    /// Ids of the idle military units (Scout, Militia, Archer), in
    /// id order. The mirror of `idle_villagers`.
    pub fn idle_military(&self) -> Vec<u32> {
        self.inner.idle_military()
    }

    /// Placed buildings as a flat array: thirteen values per
    /// building — `[id, kind, x, y, progress, complete, cell_x,
    /// cell_y, size, queue_len, train_progress, owner, hp]` — in
    /// building id order. `kind` tags: 0 house, 1 farm, 2 barracks
    /// (M3); 3 lumber camp, 4 mill, 5 mining camp, 6 archery range,
    /// 7 stable, 8 blacksmith (M6); 9 palisade wall (BLD-13). `progress` is the construction fraction 0..1 (1 when
    /// complete); `complete` is 0/1; `size` is the footprint edge in
    /// cells; `queue_len` and `train_progress` (0..1 on the queue's
    /// head) describe a production building's queue. M5 adds `owner`
    /// (0 human, 2 opponent — the entity owner tags) and `hp`, the
    /// remaining-HP fraction 0..1 (buildings take damage now; a
    /// destroyed building leaves the list entirely). A completed
    /// farm is listed here (its food shows up as a node in
    /// `nodes()`); an under-construction farm appears only here.
    pub fn buildings(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.buildings().len() * 13);
        for b in self.inner.buildings() {
            out.push(b.id as f64);
            out.push(b.kind.tag() as f64);
            out.push(b.pos_x.to_num::<f64>());
            out.push(b.pos_y.to_num::<f64>());
            out.push(I16F16::from_bits(b.progress_frac_bits()).to_num::<f64>());
            out.push(if b.complete { 1.0 } else { 0.0 });
            out.push(b.cell_x as f64);
            out.push(b.cell_y as f64);
            out.push(b.kind.footprint_size() as f64);
            out.push(b.queue.len() as f64);
            out.push(I16F16::from_bits(b.train_frac_bits()).to_num::<f64>());
            out.push(b.owner as f64);
            out.push(
                I16F16::from_bits(b.hp_frac_bits(self.inner.age_of(b.owner))).to_num::<f64>(),
            );
        }
        out
    }

    /// Current population: living villagers plus the training queue.
    pub fn population(&self) -> u32 {
        self.inner.population()
    }

    /// Population capacity (Town Center +5, each completed House +5).
    pub fn population_cap(&self) -> u32 {
        self.inner.population_cap()
    }

    /// Why Town Center training cannot currently proceed:
    /// 0 ok, 1 no food, 2 queue full, 3 housed (at population cap).
    pub fn train_block_reason(&self) -> u32 {
        self.inner.train_block_reason().tag() as u32
    }

    /// Current state hash as hex, without advancing the sim.
    pub fn state_hash(&self) -> String {
        self.inner.state_hash_hex()
    }

    /// Number of entities (Town Center + villagers).
    pub fn entity_count(&self) -> u32 {
        self.inner.entities().len() as u32
    }

    /// Number of villagers (Town Center not counted).
    pub fn villager_count(&self) -> u32 {
        self.inner.villager_count()
    }

    /// Entries waiting in the Town Center queue (villagers and,
    /// since M6, research — the queue is shared).
    pub fn train_queue_len(&self) -> u32 {
        self.inner.train_queue_len()
    }

    /// Per-entity kind tags in id order: 0 = villager, 1 = town center,
    /// 2 = scout cavalry, 3 = militia, 4 = archer, 5 = sheep, 6 = boar,
    /// 7 = deer (M3), 8 = man-at-arms (M6).
    pub fn entity_kinds(&self) -> Vec<u8> {
        self.inner.entities().iter().map(|e| e.kind.tag()).collect()
    }

    /// Per-entity owner tags in id order (M3): 0 = the human player,
    /// 1 = Gaia, 2 = the AI opponent (M5). A sheep flips 1 → its
    /// capturer's tag when captured.
    pub fn entity_owners(&self) -> Vec<u8> {
        self.inner.entities().iter().map(|e| e.owner).collect()
    }

    /// Per-entity hit points as flat `[hp, max_hp, …]` pairs in id
    /// order (M3). An entity with hp < 1 is dead (the CMB-02 rule) —
    /// the renderer skips it. The Town Center reports its full table
    /// HP; buildings take no damage in M3. M6: `max_hp` is the
    /// owner's *effective* maximum (a Loomed villager reads 40).
    pub fn entity_hp(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.entities().len() * 2);
        for e in self.inner.entities() {
            out.push(e.hp.to_num::<f64>());
            out.push(self.inner.stats_for(e.owner, e.kind).max_hp as f64);
        }
        out
    }

    /// Projectiles in flight (M3), flat: six values per projectile —
    /// `[spawn_x, spawn_y, impact_x, impact_y, spawn_tick,
    /// impact_tick]` — in spawn order. The renderer interpolates the
    /// current position from `tick_count()`; the sim owns the truth.
    pub fn projectiles(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.projectiles().len() * 6);
        for p in self.inner.projectiles() {
            out.push(p.spawn_x.to_num::<f64>());
            out.push(p.spawn_y.to_num::<f64>());
            out.push(p.impact_x.to_num::<f64>());
            out.push(p.impact_y.to_num::<f64>());
            out.push(p.spawn_tick as f64);
            out.push(p.impact_tick as f64);
        }
        out
    }

    /// Flat `[x0, y0, x1, y1, …]` entity positions as f64. Rendering only —
    /// the sim itself never sees these floats.
    pub fn positions(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.entities().len() * 2);
        for e in self.inner.entities() {
            out.push(e.pos_x.to_num::<f64>());
            out.push(e.pos_y.to_num::<f64>());
        }
        out
    }

    /// Remaining waypoints of an entity's active move order, flat
    /// `[x0, y0, …]`. Empty when the entity has no order.
    pub fn entity_path(&self, entity: u32) -> Vec<f64> {
        let mut out = Vec::new();
        if let Some(e) = self.inner.entities().iter().find(|e| e.id == entity) {
            if let Some(o) = &e.order {
                for &(wx, wy) in o.waypoints.iter().skip(o.wp_index) {
                    out.push(wx.to_num::<f64>());
                    out.push(wy.to_num::<f64>());
                }
            }
        }
        out
    }

    /// Stockpile as `[food, wood, gold, stone]` (f64 at the boundary).
    /// The human player's — unchanged since M2; use `stockpile_for`
    /// for another seat (M5).
    pub fn stockpile(&self) -> Vec<f64> {
        self.inner
            .stockpile()
            .amounts()
            .iter()
            .map(|a| a.to_num::<f64>())
            .collect()
    }

    /// Any player's stockpile by owner tag (M5): `[food, wood, gold,
    /// stone]` for owner 0 (human) or 2 (opponent).
    pub fn stockpile_for(&self, owner: u32) -> Vec<f64> {
        self.inner
            .stockpile_of(owner as u8)
            .amounts()
            .iter()
            .map(|a| a.to_num::<f64>())
            .collect()
    }

    /// Any player's score by owner tag (VIC-04):
    /// `[military, economy, technology, society, total]` for owner
    /// 0 (human) or 2 (opponent); Gaia and unknown tags read the
    /// zero breakdown. Behind the end-of-match score table.
    pub fn score_for(&self, owner: u32) -> Vec<f64> {
        let s = self.inner.score_for(owner as u8);
        vec![
            s.military as f64,
            s.economy as f64,
            s.technology as f64,
            s.society as f64,
            s.total as f64,
        ]
    }

    /// One stockpile resource by name: convenience getters for the HUD.
    pub fn food(&self) -> f64 {
        self.inner
            .stockpile()
            .amount(ResourceKind::Food)
            .to_num::<f64>()
    }

    pub fn wood(&self) -> f64 {
        self.inner
            .stockpile()
            .amount(ResourceKind::Wood)
            .to_num::<f64>()
    }

    pub fn gold(&self) -> f64 {
        self.inner
            .stockpile()
            .amount(ResourceKind::Gold)
            .to_num::<f64>()
    }

    pub fn stone(&self) -> f64 {
        self.inner
            .stockpile()
            .amount(ResourceKind::Stone)
            .to_num::<f64>()
    }

    /// Resource nodes as a flat array: five values per node —
    /// `[id, x, y, kind, amount_left]` — in node id order. `kind` tags:
    /// 0 tree, 1 berry bush, 2 gold mine, 3 stone mine, 4 farm (M2.5 —
    /// a completed farm), 5 hunt carcass, 6 herd carcass (M3).
    /// Depleted nodes are included with `amount_left` 0 (the preview
    /// hides them).
    pub fn nodes(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.nodes().len() * 5);
        for n in self.inner.nodes() {
            out.push(n.id as f64);
            out.push(n.pos_x.to_num::<f64>());
            out.push(n.pos_y.to_num::<f64>());
            out.push(n.kind.tag() as f64);
            out.push(n.amount_left.to_num::<f64>());
        }
        out
    }

    /// Navigation grid dimensions, in cells.
    pub fn grid_width(&self) -> u32 {
        self.inner.grid().width()
    }

    pub fn grid_height(&self) -> u32 {
        self.inner.grid().height()
    }

    /// Cell edge length in world units.
    pub fn cell_size(&self) -> f64 {
        grid::CELL_SIZE.to_num::<f64>()
    }

    /// World half-extent in world units: the world spans [-half, half).
    pub fn world_half(&self) -> f64 {
        grid::WORLD_HALF.to_num::<f64>()
    }

    /// Obstacle cells as row-major bytes (0 = open, 1 = blocked:
    /// water and standing-tree cells — terrain ∪ node block — plus
    /// placed-building cells). (Before M7 this was the arena's
    /// fixed wall layout; the meaning for consumers — "a land unit
    /// cannot stand here" — is unchanged.)
    pub fn obstacles(&self) -> Vec<u8> {
        self.inner.grid().blocked_bytes()
    }

    /// The generated map's terrain layer (M7, MAP-03): flat
    /// row-major bytes, one per cell — 0 land, 1 water.
    pub fn terrain(&self) -> Vec<u8> {
        self.inner.grid().terrain_bytes().to_vec()
    }

    /// The generated map's height layer (M7, MAP-05): flat
    /// row-major bytes, one elevation level per cell (0..=4).
    pub fn heights(&self) -> Vec<u8> {
        self.inner.grid().height_bytes().to_vec()
    }

    /// The LOCAL player's fog-of-war grid (M7, VIS-01): flat
    /// row-major bytes, one per cell — 0 unexplored, 1 explored,
    /// 2 currently visible. The client renders strictly from this:
    /// enemy/Gaia units and projectiles only where it reads 2,
    /// enemy buildings as last-known where it reads 1.
    pub fn fog(&self) -> Vec<u8> {
        self.inner.visibility(crate::entity::OWNER_PLAYER).to_vec()
    }

    // --- M8: per-seat forms for lockstep multiplayer ----------------
    // A networked client plays either seat, so the Town Center
    // orders and the per-player queries it needs must name the seat
    // explicitly. Every function below is a thin additive wrapper
    // over an existing sim operation; the seat-0 forms above are
    // unchanged, and nothing here alters any existing behavior.

    /// Enqueue a villager at a SPECIFIC Town Center (any seat):
    /// `push_train_villager` for an explicit Town Center.
    pub fn push_train_villager_at(&mut self, tc: u32) -> bool {
        let ok = self.inner.can_train_villager_at(tc);
        if ok {
            self.inner.push_command(Command::TrainVillagerAt { tc });
        }
        ok
    }

    /// Enqueue a Town Center technology (Loom 0, Feudal Age 1) at a
    /// SPECIFIC Town Center: `push_research_tc` for an explicit
    /// Town Center.
    pub fn push_research_tc_at(&mut self, tc: u32, tech_tag: u32) -> bool {
        let tech = match TechId::from_tag(tech_tag as u8) {
            Some(t) => t,
            None => return false,
        };
        let ok = self.inner.research_block_at_tc(tc, tech).tag() == 0;
        if ok {
            self.inner.push_command(Command::ResearchTcAt { tc, tech });
        }
        ok
    }

    /// Set a SPECIFIC Town Center's rally point (any seat):
    /// `push_set_rally_tc` for an explicit Town Center.
    pub fn push_set_rally_tc_of(&mut self, tc: u32, x: f64, y: f64) -> bool {
        let ok = self.inner.can_set_rally_tc(tc);
        if ok {
            self.inner.push_command(Command::SetRallyTc {
                tc,
                x: I16F16::from_num(x),
                y: I16F16::from_num(y),
            });
        }
        ok
    }

    /// `train_block_reason` for a specific Town Center (same tags).
    pub fn train_block_reason_at(&self, tc: u32) -> u32 {
        self.inner.train_block_at(tc).tag() as u32
    }

    /// `research_block_tc` for a specific Town Center (same tags).
    pub fn research_block_tc_at(&self, tc: u32, tech_tag: u32) -> u32 {
        match TechId::from_tag(tech_tag as u8) {
            Some(tech) => self.inner.research_block_at_tc(tc, tech).tag() as u32,
            None => 2,
        }
    }

    /// A specific Town Center's queue as flat entry codes — see
    /// `tc_queue`.
    pub fn tc_queue_for(&self, tc: u32) -> Vec<u32> {
        self.inner.tc_queue_of(tc).iter().map(|e| e.code()).collect()
    }

    /// Progress on a specific Town Center queue's head entry, 0..1.
    pub fn tc_progress_for(&self, tc: u32) -> f64 {
        I16F16::from_bits(self.inner.tc_progress_bits_of(tc)).to_num::<f64>()
    }

    /// A specific Town Center's rally point as `[x, y, node, has]` —
    /// the single-entry form of one `rally_points` entry.
    pub fn tc_rally_of(&self, tc: u32) -> Vec<f64> {
        match self.inner.tc_rally_of(tc) {
            Some((x, y, node)) => vec![
                x.to_num::<f64>(),
                y.to_num::<f64>(),
                node.map(|n| n as f64).unwrap_or(-1.0),
                1.0,
            ],
            None => vec![0.0, 0.0, -1.0, 0.0],
        }
    }

    /// Population (units + queued) of a specific player.
    pub fn population_for(&self, owner: u32) -> u32 {
        self.inner.population_of(owner as u8)
    }

    /// Population cap of a specific player.
    pub fn population_cap_for(&self, owner: u32) -> u32 {
        self.inner.population_cap_of(owner as u8)
    }

    /// Current age tag of a specific player (0 Dark, 1 Feudal).
    pub fn age_for(&self, owner: u32) -> u32 {
        self.inner.age_of(owner as u8).tag() as u32
    }

    /// Researched-technology bitmask of a specific player.
    pub fn tech_mask_for(&self, owner: u32) -> u32 {
        self.inner.tech_mask_of(owner as u8) as u32
    }

    /// The fog-of-war grid of a specific player — `fog` for an
    /// explicit seat.
    pub fn fog_for(&self, owner: u32) -> Vec<u8> {
        self.inner.visibility(owner as u8).to_vec()
    }

    /// Ids of a specific player's idle villagers.
    pub fn idle_villagers_for(&self, owner: u32) -> Vec<u32> {
        self.inner.idle_villagers_of(owner as u8)
    }

    /// Ids of a specific player's idle military.
    pub fn idle_military_for(&self, owner: u32) -> Vec<u32> {
        self.inner.idle_military_of(owner as u8)
    }
}
