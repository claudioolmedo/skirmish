//! Skirmish simulation core — headless and deterministic.
//!
//! This crate contains *all* game rules and *no* rendering, input handling,
//! networking, wall-clock time, or threads. Given the same seed and the
//! same command stream, it must produce the exact same state on every
//! platform, including WebAssembly. See `ARCHITECTURE.md` for the full
//! contract.
//!
//! M1 scope: unit movement — `Move` commands, A* on a coarse grid, steering
//! (separation + arrival). M2 scope: the economy loop — resources, nodes,
//! villagers that gather/carry/deposit, and a Town Center that trains
//! villagers. M2.5 scope: construction — villagers build Houses (+5
//! population room each) and Farms (175-food nodes), training stalls at
//! the population cap, and placement is validated in the sim. Combat
//! (M3) arrives later.

mod build;
mod econ;
mod entity;
mod grid;
mod hash;
mod rng;
mod steer;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use build::{
    Building, BuildingKind, BUILD_RANGE, FARM_BUILD_TICKS, FARM_WOOD_COST, FOOTPRINT_CELLS,
    HOUSE_BUILD_TICKS, HOUSE_POP_SUPPORT, HOUSE_WOOD_COST, TOWN_CENTER_POP_SUPPORT,
};
pub use econ::{
    NodeKind, ResourceKind, ResourceNode, Stockpile, CARRY_CAPACITY, DEPOSIT_RANGE, GATHER_RANGE,
    TRAIN_QUEUE_CAP, VILLAGER_FOOD_COST, VILLAGER_TRAIN_TICKS,
};
pub use entity::{Entity, EntityKind, MoveOrder, Task};
pub use grid::{Grid, CELL_SIZE, GRID_DIM, WORLD_HALF};
pub use hash::Fnv1a64;
pub use rng::Pcg32;
pub use steer::{MAX_SPEED, SEPARATION_RADIUS, SLOW_RADIUS, WAYPOINT_REACH};

use fixed::types::I16F16;

/// Simulation ticks per second. Fixed forever: the tick count is the only
/// clock the sim knows.
pub const TICKS_PER_SECOND: u64 = 60;

/// Town Center cell. Cell (23, 23) — world (-6, -6) — sits just west of
/// the vertical wall's gap, on open ground.
const TC_CELL: (u32, u32) = (23, 23);
/// The Town Center is always entity 0.
pub const TOWN_CENTER_ID: u32 = 0;
/// Starting villagers: 3, placed at fixed offsets from the Town Center.
pub const START_VILLAGER_COUNT: u32 = 3;
const START_VILLAGER_OFFSETS: [(i32, i32); 3] = [(5, 0), (-5, 0), (0, 5)];
/// Spawn offsets tried (in order) for a freshly trained villager: the
/// first one landing on a free cell wins. Distinct from the starting
/// offsets so a trained villager never spawns exactly on a starter.
const TRAIN_SPAWN_OFFSETS: [(i32, i32); 8] = [
    (4, 0),
    (-4, 0),
    (0, 4),
    (0, -4),
    (4, 4),
    (4, -4),
    (-4, 4),
    (-4, -4),
];

/// World position clamp: entities never leave [-99, 99].
/// (const via raw bits: 99 × 2^16 = 6488064.)
const MIN_POS: I16F16 = I16F16::from_bits(-6_488_064);
const MAX_POS: I16F16 = I16F16::from_bits(6_488_064);

/// Commands are *data*, pushed by the outside world and applied in FIFO
/// order at tick start. This shape is what makes future lockstep
/// multiplayer and replays possible: a match is (seed, settings,
/// commands).
///
/// M1 adds `Move`; M2 adds `Gather` and `TrainVillager`; M2.5 adds
/// `Build`. Attack commands land in M3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Does nothing. Exists so the command pipeline is exercised.
    Noop,
    /// Order one entity to move to a fixed-point world target. The path
    /// is recomputed immediately and deterministically when the command
    /// is applied; a later `Move` for the same entity replaces the order.
    /// For a villager this also cancels its gather task (the carry is
    /// kept, un-deposited). The Town Center ignores `Move`.
    Move { entity: u32, x: I16F16, y: I16F16 },
    /// Order a villager to work a resource node: gather, haul to the
    /// Town Center, deposit, repeat, until re-commanded or the node is
    /// gone. Ignored for non-villagers, unknown ids, and depleted nodes.
    Gather { entity: u32, node: u32 },
    /// Enqueue a villager at the Town Center. The 50-food cost is
    /// deducted *on enqueue*; rejected (no deduction) when food is
    /// insufficient or the queue is full (15).
    TrainVillager,
    /// Order a villager to construct a building whose 2×2-cell footprint
    /// is anchored at the cell containing the world point `(x, y)`. The
    /// wood cost is deducted *at placement*; the order is rejected
    /// (nothing deducted, nothing placed) when the villager is invalid,
    /// the footprint is out of bounds, overlaps a wall, building, node
    /// or the Town Center, no ring cell is reachable, or wood is short.
    /// If an *incomplete* building of the same kind already stands at
    /// that anchor, the villager is assigned to resume it instead (no
    /// further cost). Ignored for non-villagers and unknown ids.
    Build {
        entity: u32,
        kind: BuildingKind,
        x: I16F16,
        y: I16F16,
    },
}

/// Why Town Center training cannot currently proceed (M2.5). The
/// numeric tags are the WASM ABI contract: 0 ok, 1 no food,
/// 2 queue full, 3 housed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainBlock {
    /// Training would proceed: room in the queue, food to pay, and
    /// population room for the result.
    Ok,
    /// The stockpile cannot cover the 50-food cost.
    NoFood,
    /// The training queue is at its cap (15).
    QueueFull,
    /// Population is at capacity: queued villagers cannot finish
    /// training until more housing stands (the AoE2 "housed" state).
    Housed,
}

impl TrainBlock {
    pub fn tag(self) -> u8 {
        match self {
            TrainBlock::Ok => 0,
            TrainBlock::NoFood => 1,
            TrainBlock::QueueFull => 2,
            TrainBlock::Housed => 3,
        }
    }
}

/// Outcome of build validation (see `Sim::validate_build`): resume an
/// incomplete building at the requested anchor, place a new one there,
/// or reject the order whole.
enum BuildVerdict {
    Resume(u32),
    Place(u32, u32),
    Reject,
}

/// The headless deterministic simulation.
pub struct Sim {
    tick: u64,
    rng: Pcg32,
    grid: Grid,
    /// All entities, always sorted by `id`. Never a HashMap.
    /// Entity 0 is the Town Center; the rest are villagers (M2).
    entities: Vec<Entity>,
    /// FIFO command queue, drained at the start of every tick.
    commands: Vec<Command>,
    /// The player's resource stockpile (M2).
    stockpile: Stockpile,
    /// Resource nodes in id order (M2). Depleted nodes stay in the list
    /// with zero amount — inert, but still hashed.
    nodes: Vec<ResourceNode>,
    /// Villagers waiting in the Town Center training queue (M2).
    train_queue: u32,
    /// Ticks of training progress on the villager at the queue's head.
    train_progress: u64,
    /// Placed buildings in id order (M2.5): under construction and
    /// complete. A completed Farm's record stays while its node lives
    /// (footprint registry), then retires with it.
    buildings: Vec<Building>,
    /// Next building id. Buildings are occasionally removed (depleted
    /// farms), so ids cannot be derived from the list length.
    next_building_id: u32,
}

impl Sim {
    /// Create a new simulation from a seed. The M2 starting layout is
    /// fixed (Town Center, 3 villagers, node layout, stockpile), so the
    /// seed currently feeds only the in-state RNG — its words are hashed
    /// every tick, keeping seeds divergent from tick one. Later systems
    /// (combat, AI) will draw from it.
    pub fn new(seed: u64) -> Self {
        let rng = Pcg32::new(seed);
        let grid = Grid::new();
        let (tc_x, tc_y) = grid.cell_center(TC_CELL.0, TC_CELL.1);

        let mut entities = Vec::with_capacity(1 + START_VILLAGER_COUNT as usize);
        entities.push(Entity::town_center(TOWN_CENTER_ID, tc_x, tc_y));
        for (i, &(ox, oy)) in START_VILLAGER_OFFSETS.iter().enumerate() {
            entities.push(Entity::villager(
                TOWN_CENTER_ID + 1 + i as u32,
                tc_x + I16F16::from_num(ox),
                tc_y + I16F16::from_num(oy),
            ));
        }

        let nodes = econ::NODE_SPECS
            .iter()
            .enumerate()
            .map(|(i, &(kind, cx, cy))| {
                let (pos_x, pos_y) = grid.cell_center(cx, cy);
                ResourceNode {
                    id: i as u32,
                    kind,
                    pos_x,
                    pos_y,
                    amount_left: kind.full_amount(),
                }
            })
            .collect();

        Sim {
            tick: 0,
            rng,
            grid,
            entities,
            commands: Vec::new(),
            stockpile: Stockpile::starting(),
            nodes,
            train_queue: 0,
            train_progress: 0,
            buildings: Vec::new(),
            next_building_id: 0,
        }
    }

    /// Queue a command to be applied at the start of the next tick.
    pub fn push_command(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// The navigation grid (read-only; obstacles are static).
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The player's resource stockpile (read-only).
    pub fn stockpile(&self) -> &Stockpile {
        &self.stockpile
    }

    /// Resource nodes in id order, depleted ones included (amount zero).
    pub fn nodes(&self) -> &[ResourceNode] {
        &self.nodes
    }

    /// Villagers currently waiting in the Town Center training queue.
    pub fn train_queue_len(&self) -> u32 {
        self.train_queue
    }

    /// Placed buildings in id order, under construction and complete.
    pub fn buildings(&self) -> &[Building] {
        &self.buildings
    }

    /// Current population: living villagers plus those in the training
    /// queue (a queued villager is already spoken for).
    pub fn population(&self) -> u32 {
        self.villager_count() + self.train_queue
    }

    /// Population capacity: the Town Center's room (+5) plus every
    /// completed House's (+5 each), per the mechanics study §9.4.
    pub fn population_cap(&self) -> u32 {
        let mut cap = TOWN_CENTER_POP_SUPPORT;
        for b in &self.buildings {
            if b.complete {
                cap += b.kind.pop_support();
            }
        }
        cap
    }

    /// Why training cannot currently proceed (for UI display). Checked
    /// in display-priority order: queue full, housed, no food.
    pub fn train_block_reason(&self) -> TrainBlock {
        if self.train_queue >= TRAIN_QUEUE_CAP {
            return TrainBlock::QueueFull;
        }
        if self.population() >= self.population_cap() {
            return TrainBlock::Housed;
        }
        if self.stockpile.amount(ResourceKind::Food) < VILLAGER_FOOD_COST {
            return TrainBlock::NoFood;
        }
        TrainBlock::Ok
    }

    /// Would a `Build` command be accepted right now? A pure query —
    /// the same validation `issue_build` re-runs at apply time. For UI
    /// affordances (placement ghost, button states).
    pub fn can_build(&self, entity: u32, kind: BuildingKind, x: I16F16, y: I16F16) -> bool {
        !matches!(
            self.validate_build(entity, kind, clamp_pos(x), clamp_pos(y)),
            BuildVerdict::Reject
        )
    }

    /// Number of villager entities (the Town Center is not counted).
    pub fn villager_count(&self) -> u32 {
        self.entities
            .iter()
            .filter(|e| e.kind == EntityKind::Villager)
            .count() as u32
    }

    /// Would a `TrainVillager` command be accepted right now? (Food
    /// covers the cost and the queue has room.) The command path
    /// re-checks at apply time; this is for UI affordances.
    pub fn can_train_villager(&self) -> bool {
        self.train_queue < TRAIN_QUEUE_CAP
            && self.stockpile.amount(ResourceKind::Food) >= VILLAGER_FOOD_COST
    }

    /// The Town Center entity.
    pub fn town_center(&self) -> &Entity {
        &self.entities[TOWN_CENTER_ID as usize]
    }

    /// Advance the simulation by exactly one tick (1/60 s of game time).
    ///
    /// Returns the canonical per-tick state hash. Pure function of
    /// (seed, commands): no wall clock, no randomness outside `self.rng`.
    pub fn tick(&mut self) -> u64 {
        // 1. Apply queued commands in FIFO order (deterministic). Taken
        //    out of the queue first so the appliers may borrow `self`.
        let commands: Vec<Command> = std::mem::take(&mut self.commands);
        for command in commands {
            match command {
                Command::Noop => {}
                Command::Move { entity, x, y } => self.issue_move(entity, x, y),
                Command::Gather { entity, node } => self.issue_gather(entity, node),
                Command::TrainVillager => self.issue_train_villager(),
                Command::Build { entity, kind, x, y } => self.issue_build(entity, kind, x, y),
            }
        }

        // 2. Movement. Steering is decided from an immutable snapshot,
        //    then integrated — two phases keep borrows simple and the
        //    update order deterministic. The snapshot holds *villagers
        //    only*: buildings neither push nor get pushed, and a hauler
        //    must be able to reach the Town Center's exact position to
        //    complete its drop-off order.
        let snapshot: Vec<(I16F16, I16F16)> = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Villager)
            .map(|e| (e.pos_x, e.pos_y))
            .collect();
        // Entity index -> index into the villager-only snapshot.
        let mut snap_idx = vec![usize::MAX; self.entities.len()];
        {
            let mut k = 0;
            for (i, e) in self.entities.iter().enumerate() {
                if e.kind == EntityKind::Villager {
                    snap_idx[i] = k;
                    k += 1;
                }
            }
        }
        let mut plans: Vec<(I16F16, I16F16, usize, bool)> = Vec::with_capacity(self.entities.len());
        for (i, e) in self.entities.iter().enumerate() {
            plans.push(match &e.order {
                Some(o) if snap_idx[i] != usize::MAX => {
                    steer::steer(&snapshot, snap_idx[i], &o.waypoints, o.wp_index)
                }
                // No order: the villager stands still *except* for the
                // separation shove — a stationary unit must never be an
                // immovable wall on another's path (head-on pushes could
                // otherwise cancel a traveler's velocity exactly, a
                // deadlock found in M2 testing).
                None if snap_idx[i] != usize::MAX => {
                    let (vx, vy) = steer::shove(&snapshot, snap_idx[i]);
                    (vx, vy, 0, false)
                }
                _ => (I16F16::ZERO, I16F16::ZERO, 0, false),
            });
        }
        for (e, (vx, vy, idx, arrived)) in self.entities.iter_mut().zip(plans) {
            if arrived {
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            } else {
                if let Some(o) = e.order.as_mut() {
                    o.wp_index = idx;
                }
                e.vel_x = vx;
                e.vel_y = vy;
                e.pos_x = clamp_pos(e.pos_x + vx);
                e.pos_y = clamp_pos(e.pos_y + vy);
            }
        }

        // 3. Economy: Town Center training, then villager work cycles.
        self.economy_tick();

        self.tick += 1;
        self.state_hash()
    }

    /// Apply a `Move` command: clamp the target into the world, recompute
    /// the A* path immediately (deterministically), and install the order.
    /// Unknown entities and unreachable/blocked targets clear the order —
    /// the entity simply holds position. For a villager, `Move` cancels
    /// its gather task; the carry is kept (counted only on a later
    /// deposit — e.g. after a fresh `Gather` order). The Town Center
    /// cannot move and ignores the command.
    fn issue_move(&mut self, entity: u32, x: I16F16, y: I16F16) {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return,
        };
        if self.entities[idx].kind != EntityKind::Villager {
            return;
        }
        {
            let e = &mut self.entities[idx];
            e.task = Task::Idle;
            e.task_node = None;
            e.task_building = None;
        }
        self.set_move_order(idx, clamp_pos(x), clamp_pos(y));
    }

    /// Apply a `Gather` command: assign the villager to the node. If it
    /// carries a *different* resource, it first hauls that home (the
    /// assignment is kept, so work resumes at the new node after the
    /// deposit); otherwise it heads straight for the node.
    fn issue_gather(&mut self, entity: u32, node: u32) {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return,
        };
        if self.entities[idx].kind != EntityKind::Villager {
            return;
        }
        match self.node_index(node) {
            Some(ni) if !self.nodes[ni].is_depleted() => {}
            _ => return,
        }
        self.begin_gather(idx, node);
    }

    /// Assign villager `idx` to work node `node` (which must be live).
    /// Shared by the `Gather` command and by farm completion, where the
    /// builder starts working the field it just raised.
    fn begin_gather(&mut self, idx: usize, node: u32) {
        let (nx, ny, resource) = match self.node_index(node) {
            Some(ni) => {
                let n = &self.nodes[ni];
                (n.pos_x, n.pos_y, n.resource())
            }
            None => return,
        };
        self.entities[idx].task_node = Some(node);
        self.entities[idx].task_building = None;
        match self.entities[idx].carry_kind {
            Some(k) if k != resource && self.entities[idx].carry > I16F16::ZERO => {
                self.entities[idx].task = Task::Returning;
                let (tx, ty) = self.town_center_pos();
                self.set_move_order(idx, tx, ty);
            }
            _ => {
                self.entities[idx].task = Task::ToNode;
                self.set_move_order(idx, nx, ny);
            }
        }
    }

    /// Apply a `TrainVillager` command: deduct the food cost and enqueue,
    /// or reject silently (insufficient food / full queue).
    fn issue_train_villager(&mut self) {
        if self.train_queue >= TRAIN_QUEUE_CAP {
            return;
        }
        if !self
            .stockpile
            .spend(ResourceKind::Food, VILLAGER_FOOD_COST)
        {
            return;
        }
        self.train_queue += 1;
    }

    /// Apply a `Build` command: validate in the sim, deduct the wood at
    /// placement, register the building (blocking its footprint cells
    /// when the kind is solid), and send the villager to the site. On
    /// any validation failure the order is rejected whole — no partial
    /// spends, no phantom buildings.
    fn issue_build(&mut self, entity: u32, kind: BuildingKind, x: I16F16, y: I16F16) {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return,
        };
        match self.validate_build(entity, kind, clamp_pos(x), clamp_pos(y)) {
            BuildVerdict::Reject => {}
            BuildVerdict::Resume(bid) => self.assign_builder(idx, bid),
            BuildVerdict::Place(cell_x, cell_y) => {
                // Validation proved the stockpile covers the cost; the
                // spend re-checks anyway so a failure can never place a
                // free building.
                if !self.stockpile.spend(ResourceKind::Wood, kind.wood_cost()) {
                    return;
                }
                let id = self.next_building_id;
                self.next_building_id += 1;
                let (pos_x, pos_y) = build::footprint_center(cell_x, cell_y);
                let building = Building {
                    id,
                    kind,
                    cell_x,
                    cell_y,
                    pos_x,
                    pos_y,
                    progress_ticks: 0,
                    complete: false,
                    node_id: None,
                };
                if kind.blocks_footprint() {
                    for (cx, cy) in building.footprint_cells() {
                        self.grid.set_dynamic(cx, cy, true);
                    }
                }
                self.buildings.push(building);
                self.assign_builder(idx, id);
            }
        }
    }

    /// The single source of build validation, shared by the `Build`
    /// command path and the read-only `can_build` query, so the two can
    /// never drift apart.
    fn validate_build(
        &self,
        entity: u32,
        kind: BuildingKind,
        x: I16F16,
        y: I16F16,
    ) -> BuildVerdict {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return BuildVerdict::Reject,
        };
        if self.entities[idx].kind != EntityKind::Villager {
            return BuildVerdict::Reject;
        }
        let (ax, ay) = self.grid.world_to_cell(x, y);
        // Resume before placement checks: the footprint of a standing
        // incomplete building is *supposed* to be occupied — by itself.
        for b in &self.buildings {
            if !b.complete && b.kind == kind && b.cell_x == ax && b.cell_y == ay {
                return BuildVerdict::Resume(b.id);
            }
        }
        // The whole 2×2 footprint must be inside the grid.
        if ax + 1 >= GRID_DIM || ay + 1 >= GRID_DIM {
            return BuildVerdict::Reject;
        }
        let cells = build::footprint_cells(ax, ay);
        for &(cx, cy) in &cells {
            // Walls and already-blocking buildings.
            if self.grid.is_blocked(cx, cy) {
                return BuildVerdict::Reject;
            }
            // The Town Center's cell.
            if (cx, cy) == TC_CELL {
                return BuildVerdict::Reject;
            }
            // Resource nodes sit on cell centers; a footprint may not
            // swallow a live one. Depleted nodes reserve nothing —
            // like the reference game, what is harvested is gone and
            // the ground is free again (this is also what lets a
            // depleted farm's own plot be replanted).
            for n in &self.nodes {
                if !n.is_depleted()
                    && self.grid.world_to_cell(n.pos_x, n.pos_y) == (cx, cy)
                {
                    return BuildVerdict::Reject;
                }
            }
        }
        // Overlap with any standing building's footprint (farms do not
        // block cells, so they are only visible in this registry).
        // Two 2×2 footprints overlap iff their anchors are within one
        // cell on both axes.
        for b in &self.buildings {
            if ax.abs_diff(b.cell_x) <= 1 && ay.abs_diff(b.cell_y) <= 1 {
                return BuildVerdict::Reject;
            }
        }
        // The wood must be payable at placement.
        if self.stockpile.amount(ResourceKind::Wood) < kind.wood_cost() {
            return BuildVerdict::Reject;
        }
        // Reachable-ish: at least one ring cell stands open and the
        // builder has a path to it.
        if self.approach_point(idx, ax, ay).is_none() {
            return BuildVerdict::Reject;
        }
        BuildVerdict::Place(ax, ay)
    }

    /// Put villager `idx` on construction duty for building `bid`:
    /// travel phase, with a move order to the best standing spot.
    fn assign_builder(&mut self, idx: usize, bid: u32) {
        let (ax, ay) = match self.building_index(bid) {
            Some(bi) => {
                let b = &self.buildings[bi];
                (b.cell_x, b.cell_y)
            }
            None => return,
        };
        {
            let e = &mut self.entities[idx];
            e.task = Task::ToBuild;
            e.task_building = Some(bid);
            e.task_node = None;
        }
        match self.approach_point(idx, ax, ay) {
            Some((px, py)) => self.set_move_order(idx, px, py),
            None => {
                // No standing spot right now (crowded site): the work
                // step retries every tick, like gather/deposit orders.
                let e = &mut self.entities[idx];
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
        }
    }

    /// The builder's standing spot for the footprint anchored at
    /// `(ax, ay)`: the first ring cell (fixed order) that is in bounds,
    /// unblocked, and reachable from the villager's position.
    fn approach_point(&self, idx: usize, ax: u32, ay: u32) -> Option<(I16F16, I16F16)> {
        let from = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        for (cx, cy) in build::ring_cells(ax, ay) {
            if cx >= GRID_DIM || cy >= GRID_DIM {
                continue;
            }
            if self.grid.is_blocked(cx, cy) {
                continue;
            }
            let center = self.grid.cell_center(cx, cy);
            if self.grid.find_path(from, center).is_some() {
                return Some(center);
            }
        }
        None
    }

    fn building_index(&self, id: u32) -> Option<usize> {
        self.buildings.iter().position(|b| b.id == id)
    }

    /// Install a freshly computed A* move order toward a world point.
    /// On failure (blocked/unreachable) the order is cleared and the
    /// entity holds position; work-state callers re-issue as needed.
    fn set_move_order(&mut self, idx: usize, x: I16F16, y: I16F16) {
        let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let order = match self.grid.find_path((px, py), (x, y)) {
            Some(wps) if !wps.is_empty() => Some(MoveOrder {
                target_x: x,
                target_y: y,
                waypoints: wps,
                wp_index: 0,
            }),
            _ => None,
        };
        let e = &mut self.entities[idx];
        match order {
            Some(o) => e.order = Some(o),
            None => {
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
        }
    }

    /// The economy phase of a tick: training progress at the Town Center,
    /// then every villager's work state machine, in id order, then
    /// construction progress (M2.5).
    fn economy_tick(&mut self) {
        // Training stalls while the population is at capacity — the
        // "housed" state: the queue keeps its place and its progress,
        // and resumes the tick housing frees up. Completion spawns only
        // while there is room, so the cap can never be exceeded.
        if self.train_queue > 0 && self.villager_count() < self.population_cap() {
            self.train_progress += 1;
            if self.train_progress >= VILLAGER_TRAIN_TICKS {
                self.train_progress = 0;
                self.train_queue -= 1;
                self.spawn_trained_villager();
            }
        }
        for idx in 0..self.entities.len() {
            if self.entities[idx].kind == EntityKind::Villager {
                self.work_tick(idx);
            }
        }
        self.construction_tick();
    }

    /// One villager's work state machine step. Transitions trigger on
    /// *distance* (arrival ranges), not only on move-order completion:
    /// separation can hold a villager slightly off its target point, and
    /// the cycle must not deadlock on that.
    fn work_tick(&mut self, idx: usize) {
        match self.entities[idx].task {
            Task::Idle => {}
            Task::ToNode => {
                let node_id = match self.entities[idx].task_node {
                    Some(n) => n,
                    None => {
                        self.entities[idx].task = Task::Idle;
                        return;
                    }
                };
                let (nx, ny) = match self.live_node_pos(node_id) {
                    Some(p) => p,
                    None => {
                        // The node is gone (depleted while heading there).
                        self.leave_for_deposit_or_idle(idx);
                        return;
                    }
                };
                let d = self.distance_to(idx, nx, ny);
                if d <= GATHER_RANGE {
                    let e = &mut self.entities[idx];
                    e.task = Task::Gathering;
                    e.order = None;
                    e.vel_x = I16F16::ZERO;
                    e.vel_y = I16F16::ZERO;
                } else if self.entities[idx].order.is_none() {
                    self.set_move_order(idx, nx, ny);
                }
            }
            Task::Gathering => {
                let node_id = match self.entities[idx].task_node {
                    Some(n) => n,
                    None => {
                        self.entities[idx].task = Task::Idle;
                        return;
                    }
                };
                let ni = match self.node_index(node_id) {
                    Some(ni) if !self.nodes[ni].is_depleted() => ni,
                    _ => {
                        // Depleted (by this or another villager).
                        if self.entities[idx].carry > I16F16::ZERO {
                            self.begin_return(idx);
                        } else {
                            self.stop_task(idx);
                        }
                        return;
                    }
                };
                let rate = self.nodes[ni].kind.gather_rate();
                let room = CARRY_CAPACITY - self.entities[idx].carry;
                let take = rate.min(room).min(self.nodes[ni].amount_left);
                if take > I16F16::ZERO {
                    self.nodes[ni].amount_left -= take;
                    let resource = self.nodes[ni].resource();
                    let e = &mut self.entities[idx];
                    e.carry += take;
                    if e.carry_kind.is_none() {
                        e.carry_kind = Some(resource);
                    }
                }
                let full = self.entities[idx].carry >= CARRY_CAPACITY;
                if full || self.nodes[ni].is_depleted() {
                    if self.entities[idx].carry > I16F16::ZERO {
                        self.begin_return(idx);
                    } else {
                        self.stop_task(idx);
                    }
                }
            }
            Task::Returning => {
                let (tx, ty) = self.town_center_pos();
                let d = self.distance_to(idx, tx, ty);
                if d <= DEPOSIT_RANGE {
                    self.deposit(idx);
                } else if self.entities[idx].order.is_none() {
                    self.set_move_order(idx, tx, ty);
                }
            }
            Task::ToBuild => {
                let bid = match self.entities[idx].task_building {
                    Some(b) => b,
                    None => {
                        self.entities[idx].task = Task::Idle;
                        return;
                    }
                };
                let bi = match self.building_index(bid) {
                    Some(bi) => bi,
                    None => {
                        self.stop_task(idx);
                        return;
                    }
                };
                if self.buildings[bi].complete {
                    self.leave_finished_site(idx);
                    return;
                }
                let (bx, by) = (self.buildings[bi].pos_x, self.buildings[bi].pos_y);
                let (ax, ay) = (self.buildings[bi].cell_x, self.buildings[bi].cell_y);
                let d = self.distance_to(idx, bx, by);
                if d <= BUILD_RANGE {
                    let e = &mut self.entities[idx];
                    e.task = Task::Building;
                    e.order = None;
                    e.vel_x = I16F16::ZERO;
                    e.vel_y = I16F16::ZERO;
                } else if self.entities[idx].order.is_none() {
                    if let Some((px, py)) = self.approach_point(idx, ax, ay) {
                        self.set_move_order(idx, px, py);
                    }
                }
            }
            Task::Building => {
                let bid = match self.entities[idx].task_building {
                    Some(b) => b,
                    None => {
                        self.entities[idx].task = Task::Idle;
                        return;
                    }
                };
                let bi = match self.building_index(bid) {
                    Some(bi) => bi,
                    None => {
                        self.stop_task(idx);
                        return;
                    }
                };
                if self.buildings[bi].complete {
                    self.leave_finished_site(idx);
                    return;
                }
                // Drifted out of build range (separation shove): go
                // back to the travel phase and re-approach. While in
                // range, progress itself is applied by the construction
                // phase — this arm only maintains the phase.
                let (bx, by) = (self.buildings[bi].pos_x, self.buildings[bi].pos_y);
                if self.distance_to(idx, bx, by) > BUILD_RANGE {
                    self.entities[idx].task = Task::ToBuild;
                }
            }
        }
    }

    /// A villager whose building finished while it was travelling to or
    /// standing at the site: for a Farm, go work the new field; for a
    /// House, the job is simply done.
    fn leave_finished_site(&mut self, idx: usize) {
        let bid = self.entities[idx].task_building;
        let node_id = bid.and_then(|b| {
            self.building_index(b)
                .and_then(|bi| self.buildings[bi].node_id)
        });
        match node_id {
            Some(nid) => self.begin_gather(idx, nid),
            None => self.stop_task(idx),
        }
    }

    /// The construction phase of a tick (M2.5): every villager in the
    /// `Building` phase adds one tick of progress to its building;
    /// buildings reaching their build time complete. Depleted farms
    /// retire (their footprint frees up for new placement).
    fn construction_tick(&mut self) {
        let mut completed: Vec<u32> = Vec::new();
        for idx in 0..self.entities.len() {
            if self.entities[idx].task != Task::Building {
                continue;
            }
            let bid = match self.entities[idx].task_building {
                Some(b) => b,
                None => continue,
            };
            if let Some(bi) = self.building_index(bid) {
                if !self.buildings[bi].complete {
                    self.buildings[bi].progress_ticks += 1;
                    if self.buildings[bi].progress_ticks >= self.buildings[bi].kind.build_ticks()
                    {
                        completed.push(bid);
                    }
                }
            }
        }
        for bid in completed {
            self.complete_building(bid);
        }
        // Retire depleted farms: the node is inert and the field's
        // ground returns to the map.
        let retire: Vec<u32> = self
            .buildings
            .iter()
            .filter(|b| {
                b.complete
                    && b.kind == BuildingKind::Farm
                    && match b.node_id {
                        Some(nid) => self
                            .node_index(nid)
                            .map(|ni| self.nodes[ni].is_depleted())
                            .unwrap_or(true),
                        None => false,
                    }
            })
            .map(|b| b.id)
            .collect();
        if !retire.is_empty() {
            self.buildings.retain(|b| !retire.contains(&b.id));
        }
    }

    /// Mark a building complete and apply its function: a Farm becomes
    /// a food node (and its active builders start working it); a House
    /// simply starts counting toward the population cap.
    fn complete_building(&mut self, bid: u32) {
        let bi = match self.building_index(bid) {
            Some(bi) => bi,
            None => return,
        };
        if self.buildings[bi].complete {
            return;
        }
        self.buildings[bi].complete = true;
        if self.buildings[bi].kind != BuildingKind::Farm {
            // House: active builders are done; travellers notice the
            // completion in their own work step and stand down.
            for idx in 0..self.entities.len() {
                if self.entities[idx].task == Task::Building
                    && self.entities[idx].task_building == Some(bid)
                {
                    self.stop_task(idx);
                }
            }
            return;
        }
        let (px, py) = (self.buildings[bi].pos_x, self.buildings[bi].pos_y);
        let nid = self.nodes.len() as u32;
        self.nodes.push(ResourceNode {
            id: nid,
            kind: NodeKind::Farm,
            pos_x: px,
            pos_y: py,
            amount_left: NodeKind::Farm.full_amount(),
        });
        if let Some(bi) = self.building_index(bid) {
            self.buildings[bi].node_id = Some(nid);
        }
        // The builders who raised the field start farming it.
        for idx in 0..self.entities.len() {
            if self.entities[idx].task == Task::Building
                && self.entities[idx].task_building == Some(bid)
            {
                self.begin_gather(idx, nid);
            }
        }
    }

    /// Deposit the villager's carry into the stockpile, then resume the
    /// gather assignment if its node is still alive; otherwise go idle.
    fn deposit(&mut self, idx: usize) {
        let (carry, kind) = {
            let e = &mut self.entities[idx];
            let taken = (e.carry, e.carry_kind);
            e.carry = I16F16::ZERO;
            e.carry_kind = None;
            e.order = None;
            e.vel_x = I16F16::ZERO;
            e.vel_y = I16F16::ZERO;
            taken
        };
        if let Some(kind) = kind {
            if carry > I16F16::ZERO {
                self.stockpile.add(kind, carry);
            }
        }
        match self.entities[idx].task_node {
            Some(node_id) => match self.live_node_pos(node_id) {
                Some((nx, ny)) => {
                    self.entities[idx].task = Task::ToNode;
                    self.set_move_order(idx, nx, ny);
                }
                None => self.stop_task(idx),
            },
            None => self.stop_task(idx),
        }
    }

    /// Send the villager home to deposit, keeping its gather assignment.
    fn begin_return(&mut self, idx: usize) {
        self.entities[idx].task = Task::Returning;
        let (tx, ty) = self.town_center_pos();
        self.set_move_order(idx, tx, ty);
    }

    /// The villager's node vanished mid-task: haul the carry home if
    /// there is one (the deposit step then retires the assignment),
    /// otherwise stop where it stands.
    fn leave_for_deposit_or_idle(&mut self, idx: usize) {
        if self.entities[idx].carry > I16F16::ZERO {
            self.begin_return(idx);
        } else {
            self.stop_task(idx);
        }
    }

    /// Clear the work assignment (gather or construction) and halt the
    /// villager.
    fn stop_task(&mut self, idx: usize) {
        let e = &mut self.entities[idx];
        e.task = Task::Idle;
        e.task_node = None;
        e.task_building = None;
        e.order = None;
        e.vel_x = I16F16::ZERO;
        e.vel_y = I16F16::ZERO;
    }

    /// Spawn a trained villager adjacent to the Town Center.
    fn spawn_trained_villager(&mut self) {
        let (tx, ty) = self.town_center_pos();
        let mut pos = (tx + I16F16::from_num(TRAIN_SPAWN_OFFSETS[0].0), ty);
        for &(ox, oy) in &TRAIN_SPAWN_OFFSETS {
            let p = (tx + I16F16::from_num(ox), ty + I16F16::from_num(oy));
            let (cx, cy) = self.grid.world_to_cell(p.0, p.1);
            if !self.grid.is_blocked(cx, cy) {
                pos = p;
                break;
            }
        }
        // Ids are sequential and entities are never removed in M2, so
        // the next id is the current count.
        let id = self.entities.len() as u32;
        self.entities.push(Entity::villager(id, pos.0, pos.1));
    }

    fn entity_index(&self, id: u32) -> Option<usize> {
        self.entities.iter().position(|e| e.id == id)
    }

    fn node_index(&self, id: u32) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    /// Position of a node that can still be gathered from, else `None`.
    fn live_node_pos(&self, id: u32) -> Option<(I16F16, I16F16)> {
        self.node_index(id).and_then(|ni| {
            let n = &self.nodes[ni];
            if n.is_depleted() {
                None
            } else {
                Some((n.pos_x, n.pos_y))
            }
        })
    }

    fn town_center_pos(&self) -> (I16F16, I16F16) {
        let tc = self.town_center();
        (tc.pos_x, tc.pos_y)
    }

    /// Fixed-point distance from an entity to a world point.
    fn distance_to(&self, idx: usize, x: I16F16, y: I16F16) -> I16F16 {
        let e = &self.entities[idx];
        steer::vec_len(x - e.pos_x, y - e.pos_y)
    }

    /// Ticks elapsed since creation. The *only* clock in the sim.
    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    /// Read-only view of the entities (id-sorted).
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Canonical per-tick state hash: FNV-1a over the tick counter, the
    /// RNG words, the grid layout, the stockpile, the nodes, the training
    /// queue, and every entity field (including orders and work state) as
    /// raw bits, in a fixed order. Platform-independent by construction.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv1a64::new();
        h.write_u64(self.tick);
        let (rng_state, rng_inc) = self.rng.state_words();
        h.write_u64(rng_state);
        h.write_u64(rng_inc);
        // Grid layout is constant, but hashing it pins the layout to the
        // golden-hash tripwire: any accidental layout change is caught.
        h.write_u32(self.grid.width());
        h.write_u32(self.grid.height());
        for b in self.grid.blocked_bytes() {
            h.write_u8(b);
        }
        // M2.5: the dynamic layer — cells blocked by placed buildings.
        for b in self.grid.dynamic_bytes() {
            h.write_u8(b);
        }
        // M2: stockpile in canonical resource order.
        for amount in self.stockpile.amounts() {
            h.write_i32(amount.to_bits());
        }
        // M2: nodes (id order), then the Town Center training state.
        h.write_u32(self.nodes.len() as u32);
        for n in &self.nodes {
            h.write_u32(n.id);
            h.write_u8(n.kind.tag());
            h.write_i32(n.pos_x.to_bits());
            h.write_i32(n.pos_y.to_bits());
            h.write_i32(n.amount_left.to_bits());
        }
        h.write_u32(self.train_queue);
        h.write_u64(self.train_progress);
        // M2.5: buildings (id order), construction state included.
        h.write_u32(self.buildings.len() as u32);
        for b in &self.buildings {
            h.write_u32(b.id);
            h.write_u8(b.kind.tag());
            h.write_u32(b.cell_x);
            h.write_u32(b.cell_y);
            h.write_i32(b.pos_x.to_bits());
            h.write_i32(b.pos_y.to_bits());
            h.write_u64(b.progress_ticks);
            h.write_u8(u8::from(b.complete));
            match b.node_id {
                None => h.write_u8(0),
                Some(n) => {
                    h.write_u8(1);
                    h.write_u32(n);
                }
            }
        }
        h.write_u32(self.entities.len() as u32);
        for e in &self.entities {
            h.write_u32(e.id);
            h.write_u8(e.kind.tag());
            h.write_i32(e.pos_x.to_bits());
            h.write_i32(e.pos_y.to_bits());
            h.write_i32(e.vel_x.to_bits());
            h.write_i32(e.vel_y.to_bits());
            match &e.order {
                None => h.write_u8(0),
                Some(o) => {
                    h.write_u8(1);
                    h.write_i32(o.target_x.to_bits());
                    h.write_i32(o.target_y.to_bits());
                    h.write_u32(o.wp_index as u32);
                    h.write_u32(o.waypoints.len() as u32);
                    for &(wx, wy) in &o.waypoints {
                        h.write_i32(wx.to_bits());
                        h.write_i32(wy.to_bits());
                    }
                }
            }
            // M2: work state — task phase, assignment, carry.
            h.write_u8(e.task.tag());
            match e.task_node {
                None => h.write_u8(0),
                Some(n) => {
                    h.write_u8(1);
                    h.write_u32(n);
                }
            }
            // M2.5: construction assignment.
            match e.task_building {
                None => h.write_u8(0),
                Some(b) => {
                    h.write_u8(1);
                    h.write_u32(b);
                }
            }
            h.write_i32(e.carry.to_bits());
            h.write_u8(match e.carry_kind {
                None => 0,
                Some(k) => k.tag(),
            });
        }
        h.finish()
    }

    /// State hash as a 16-character lowercase hex string.
    pub fn state_hash_hex(&self) -> String {
        format!("{:016x}", self.state_hash())
    }
}

/// Clamp a world coordinate into [-99, 99] without floats.
fn clamp_pos(v: I16F16) -> I16F16 {
    if v < MIN_POS {
        MIN_POS
    } else if v > MAX_POS {
        MAX_POS
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_is_deterministic() {
        let a = Sim::new(5);
        let b = Sim::new(5);
        assert_eq!(a.entities(), b.entities());
        assert_eq!(a.nodes(), b.nodes());
    }

    #[test]
    fn starting_layout_is_on_open_ground() {
        let sim = Sim::new(5);
        for e in sim.entities() {
            let (cx, cy) = sim.grid.world_to_cell(e.pos_x, e.pos_y);
            assert!(
                !sim.grid.is_blocked(cx, cy),
                "entity {} starts inside a wall",
                e.id
            );
        }
        for n in sim.nodes() {
            let (cx, cy) = sim.grid.world_to_cell(n.pos_x, n.pos_y);
            assert!(
                !sim.grid.is_blocked(cx, cy),
                "node {} sits inside a wall",
                n.id
            );
        }
    }

    #[test]
    fn tick_increments_clock() {
        let mut sim = Sim::new(5);
        assert_eq!(sim.tick_count(), 0);
        sim.tick();
        assert_eq!(sim.tick_count(), 1);
    }

    #[test]
    fn entities_stay_id_sorted() {
        let mut sim = Sim::new(5);
        // Train one villager so the id sequence grows, then verify order.
        sim.push_command(Command::TrainVillager);
        for _ in 0..1600 {
            sim.tick();
        }
        let ids: Vec<u32> = sim.entities().iter().map(|e| e.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn noop_command_keeps_hash_stable() {
        let mut a = Sim::new(11);
        let mut b = Sim::new(11);
        b.push_command(Command::Noop);
        for _ in 0..200 {
            assert_eq!(a.tick(), b.tick());
        }
    }

    #[test]
    fn move_to_blocked_target_holds_position() {
        let mut sim = Sim::new(11);
        let before: Vec<(I16F16, I16F16)> =
            sim.entities().iter().map(|e| (e.pos_x, e.pos_y)).collect();
        // (2, -46): cell (25, 13) — inside the vertical wall.
        sim.push_command(Command::Move {
            entity: 1,
            x: I16F16::from_num(2),
            y: I16F16::from_num(-46),
        });
        for _ in 0..120 {
            sim.tick();
        }
        let after: Vec<(I16F16, I16F16)> =
            sim.entities().iter().map(|e| (e.pos_x, e.pos_y)).collect();
        assert_eq!(before, after, "entity moved toward a blocked target");
        let v = sim.entities().iter().find(|e| e.id == 1).unwrap();
        assert!(v.order.is_none());
    }

    #[test]
    fn move_unknown_entity_is_ignored() {
        let mut sim = Sim::new(11);
        sim.push_command(Command::Move {
            entity: 999,
            x: I16F16::from_num(10),
            y: I16F16::from_num(10),
        });
        sim.push_command(Command::Gather {
            entity: 999,
            node: 0,
        });
        for _ in 0..60 {
            sim.tick();
        }
        assert!(sim.entities().iter().all(|e| e.order.is_none()));
        assert!(sim.entities().iter().all(|e| e.task == Task::Idle));
    }

    #[test]
    fn town_center_ignores_move_and_gather() {
        let mut sim = Sim::new(11);
        let tc_before = (sim.town_center().pos_x, sim.town_center().pos_y);
        sim.push_command(Command::Move {
            entity: TOWN_CENTER_ID,
            x: I16F16::from_num(50),
            y: I16F16::from_num(50),
        });
        sim.push_command(Command::Gather {
            entity: TOWN_CENTER_ID,
            node: 0,
        });
        for _ in 0..120 {
            sim.tick();
        }
        let tc = sim.town_center();
        assert_eq!((tc.pos_x, tc.pos_y), tc_before, "Town Center moved");
        assert!(tc.order.is_none());
        assert_eq!(tc.task, Task::Idle);
    }

    #[test]
    fn move_override_keeps_carry_undeposited() {
        // Documented M2 choice: a Move override cancels the gather task
        // but the villager keeps its carry — it is not deposited until a
        // later work cycle delivers it.
        let mut sim = Sim::new(11);
        let berry = sim
            .nodes()
            .iter()
            .find(|n| n.kind == NodeKind::BerryBush)
            .unwrap()
            .id;
        sim.push_command(Command::Gather {
            entity: 1,
            node: berry,
        });
        // Wait until the villager is actually gathering with some carry.
        let mut gathering = false;
        for _ in 0..1200 {
            sim.tick();
            let v = sim.entities().iter().find(|e| e.id == 1).unwrap();
            if v.task == Task::Gathering && v.carry > I16F16::ZERO {
                gathering = true;
                break;
            }
        }
        assert!(gathering, "villager never started gathering");
        let food_before = sim.stockpile().amount(ResourceKind::Food);
        sim.push_command(Command::Move {
            entity: 1,
            x: I16F16::from_num(30),
            y: I16F16::from_num(30),
        });
        for _ in 0..30 {
            sim.tick();
        }
        let v = sim.entities().iter().find(|e| e.id == 1).unwrap();
        assert_eq!(v.task, Task::Idle, "Move must cancel the gather task");
        assert!(v.carry > I16F16::ZERO, "carry must be kept, not lost");
        assert_eq!(
            sim.stockpile().amount(ResourceKind::Food),
            food_before,
            "carry must not be deposited by a Move override"
        );
    }
}
