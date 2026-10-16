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

use wasm_bindgen::prelude::*;

use fixed::types::I16F16;

use crate::econ::ResourceKind;
use crate::grid;
use crate::{Command, Sim};

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

    /// Per-entity kind tags in id order: 0 = villager, 1 = town center.
    pub fn entity_kinds(&self) -> Vec<u8> {
        self.inner.entities().iter().map(|e| e.kind.tag()).collect()
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
    /// 0 tree, 1 berry bush, 2 gold mine, 3 stone mine. Depleted nodes
    /// are included with `amount_left` 0 (the preview hides them).
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
