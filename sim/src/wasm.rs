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

use wasm_bindgen::prelude::*;

use fixed::types::I16F16;

use crate::build::BuildingKind;
use crate::econ::ResourceKind;
use crate::entity::EntityKind;
use crate::grid;
use crate::{Command, QueuedOrder, Sim};

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

    /// Enqueue a Militia at a completed Barracks (M3). Returns
    /// whether the order *would* be accepted (queue room + funds);
    /// the cost is deducted when the command is applied next tick.
    pub fn push_train_militia(&mut self, building: u32) -> bool {
        self.push_train_at(building, EntityKind::Militia)
    }

    /// Enqueue an Archer at a completed Barracks (M3) — see
    /// `push_train_militia`.
    pub fn push_train_archer(&mut self, building: u32) -> bool {
        self.push_train_at(building, EntityKind::Archer)
    }

    fn push_train_at(&mut self, building: u32, kind: EntityKind) -> bool {
        let ok = self.inner.can_train_at(building, kind);
        if ok {
            self.inner.push_command(Command::TrainAt { building, kind });
        }
        ok
    }

    /// Why training `kind_tag` (3 militia, 4 archer) at `building`
    /// cannot currently proceed: same tags as `train_block_reason`
    /// (0 ok, 1 cannot afford, 2 queue full / not trainable here,
    /// 3 housed). UI affordance only.
    pub fn building_train_block(&self, building: u32, kind_tag: u32) -> u32 {
        match EntityKind::from_tag(kind_tag as u8) {
            Some(kind) => self.inner.train_at_block(building, kind).tag() as u32,
            None => 2,
        }
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

    /// Placed buildings as a flat array: eleven values per building —
    /// `[id, kind, x, y, progress, complete, cell_x, cell_y, size,
    /// queue_len, train_progress]` — in building id order. `kind`
    /// tags: 0 house, 1 farm, 2 barracks (M3). `progress` is the
    /// construction fraction 0..1 (1 when complete); `complete` is
    /// 0/1; `size` is the footprint edge in cells; `queue_len` and
    /// `train_progress` (0..1 on the queue's head) describe a
    /// production building's queue. A completed farm is listed here
    /// (its food shows up as a node in `nodes()`); an
    /// under-construction farm appears only here.
    pub fn buildings(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.buildings().len() * 11);
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

    /// Villagers waiting in the Town Center training queue.
    pub fn train_queue_len(&self) -> u32 {
        self.inner.train_queue_len()
    }

    /// Per-entity kind tags in id order: 0 = villager, 1 = town center,
    /// 2 = scout cavalry, 3 = militia, 4 = archer, 5 = sheep, 6 = boar,
    /// 7 = deer (M3).
    pub fn entity_kinds(&self) -> Vec<u8> {
        self.inner.entities().iter().map(|e| e.kind.tag()).collect()
    }

    /// Per-entity owner tags in id order (M3): 0 = the player,
    /// 1 = Gaia. A sheep flips 1 → 0 when captured.
    pub fn entity_owners(&self) -> Vec<u8> {
        self.inner.entities().iter().map(|e| e.owner).collect()
    }

    /// Per-entity hit points as flat `[hp, max_hp, …]` pairs in id
    /// order (M3). An entity with hp < 1 is dead (the CMB-02 rule) —
    /// the renderer skips it. The Town Center reports its full table
    /// HP; buildings take no damage in M3.
    pub fn entity_hp(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(self.inner.entities().len() * 2);
        for e in self.inner.entities() {
            out.push(e.hp.to_num::<f64>());
            out.push(crate::units::spec(e.kind).max_hp as f64);
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
    pub fn stockpile(&self) -> Vec<f64> {
        self.inner
            .stockpile()
            .amounts()
            .iter()
            .map(|a| a.to_num::<f64>())
            .collect()
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

    /// Obstacle cells as row-major bytes (0 = open, 1 = blocked).
    pub fn obstacles(&self) -> Vec<u8> {
        self.inner.grid().blocked_bytes()
    }
}
