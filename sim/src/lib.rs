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
//! the population cap, and placement is validated in the sim. M3 scope:
//! combat — HP and table-driven damage, attack orders, Gaia animals
//! (sheep/boar/deer) and hunting with decaying carcasses, the starting
//! Scout Cavalry, the Barracks training Militia and Archers, and
//! projectiles as the first consumer of the seeded RNG. M4 scope:
//! input & command completion — per-entity queued orders (SEL-12),
//! rally points on the Town Center and production buildings (SEL-13),
//! gatherer retargeting when a node depletes (ECO-09), and the idle
//! queries behind the idle keys and indicators (SEL-08/09, INF-03).
//! M5 scope: a real match — per-player state (stockpiles, population,
//! building ownership), a mirrored opponent start, and a scripted AI
//! opponent (`ai` module) that plays through the same command queue
//! as the human (AI-01/02), at Easy/Normal behavior profiles
//! (AI-03/04); buildings gain HP and can be attacked and destroyed,
//! and Conquest decides the match (VIC-01): a player with no
//! buildings and no units has lost, and the AI resigns when it has
//! no Town Center and no villagers left (AI-05).
//! M6 scope: ages, technologies, and the economy buildings —
//! per-player age (Dark → Feudal) and researched-tech state
//! (TEC-01/04/06), research as timed entries in the hosts' shared
//! production queues (the Town Center's queue is shared with
//! villager training, TEC-03), the drop-off camps with
//! nearest-valid-drop-off routing and force drop-off (ECO-05/06/07),
//! the Mill gate on Farms, and the Feudal buildings (Archery Range,
//! Stable, Blacksmith) with the Militia line's Man-at-Arms upgrade
//! (CLS-03/04).
//! M7 scope: the world itself — a deterministic seeded Arabia
//! generator (`mapgen`) replaces the fixed arena: the rulebook's
//! 120×120-tile 2-player map (MAP-02), per-start resource packages
//! (OPENING-SPEC §2.3) mirrored for exact fairness, generated
//! terrain with water (MAP-03/08) and a height field (MAP-05) that
//! feeds the elevation combat factor (MAP-06/CMB-01), trees as
//! blocking nodes (MAP-04), and per-player fog of war with
//! exploration memory (VIS-01…08), hash-covered and recomputed on
//! a bounded interval (see `recompute_visibility`).

mod ai;
mod build;
mod classes;
mod combat;
mod econ;
mod entity;
mod grid;
mod hash;
mod mapgen;
mod rng;
mod steer;
mod tech;
mod units;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use build::{
    build_rate_thirds, Building, BuildingKind, ARCHERY_RANGE_BUILD_TICKS, ARCHERY_RANGE_WOOD_COST,
    BARRACKS_BUILD_TICKS, BARRACKS_WOOD_COST, BLACKSMITH_BUILD_TICKS, BLACKSMITH_WOOD_COST,
    BUILD_RANGE, CAMP_BUILD_TICKS, CAMP_WOOD_COST, FARM_BUILD_TICKS, FARM_WOOD_COST,
    FOOTPRINT_CELLS, HOUSE_BUILD_TICKS, HOUSE_POP_SUPPORT, HOUSE_WOOD_COST, STABLE_BUILD_TICKS,
    STABLE_WOOD_COST, TOWN_CENTER_POP_SUPPORT,
};
pub use combat::{compute_damage, compute_damage_elev, elevation_factor, Projectile, MELEE_REACH};
pub use econ::{
    NodeKind, ResourceKind, ResourceNode, Stockpile, CARRY_CAPACITY, DEPOSIT_RANGE, GATHER_RANGE,
    HUNT_CARRY_CAPACITY, RALLY_NODE_SNAP_RADIUS, RETARGET_RADIUS, TRAIN_QUEUE_CAP,
    VILLAGER_FOOD_COST, VILLAGER_TRAIN_TICKS,
};
pub use entity::{
    is_player_owner, Dropoff, Entity, EntityKind, MoveOrder, QueuedOrder, Task, OWNER_GAIA,
    OWNER_OPPONENT, OWNER_PLAYER,
};
pub use grid::{Grid, CELL_SIZE, GRID_DIM, TERRAIN_LAND, TERRAIN_WATER, WORLD_HALF};
pub use mapgen::GeneratedMap;
pub use hash::Fnv1a64;
pub use rng::Pcg32;
pub use steer::{MAX_SPEED, SEPARATION_RADIUS, SLOW_RADIUS, WAYPOINT_REACH};
pub use tech::{Age, ProdEntry, ResearchBlock, TechHost, TechId};
pub use units::{spec as unit_spec, UnitSpec};

use fixed::types::I16F16;

/// Simulation ticks per second. Fixed forever: the tick count is the only
/// clock the sim knows.
pub const TICKS_PER_SECOND: u64 = 60;

/// The Town Center is always entity 0. Its cell is no longer a
/// constant: the M7 generator places it (see `mapgen`), mirrored
/// for the opponent, different on every seed.
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

/// The starting Scout Cavalry's offset from the Town Center (M3):
/// the standard start fields one scout beside the TC (OPENING-SPEC
/// §2.1). Diagonal, so it never shares a villager's spawn point.
const START_SCOUT_OFFSET: (i32, i32) = (5, 5);

/// Fog of war recomputation interval, in ticks (M7, VIS-01): the
/// per-player visibility grids are rebuilt from scratch this often
/// (and once at match creation) — never per tick — so the cost is
/// bounded by (sources × sight-disc area) per interval regardless
/// of map size or unit count growth. Between recomputations the
/// grids stand as last computed; 15 ticks is a quarter second, far
/// below the time anything needs to cross a sight radius.
pub const FOG_RECOMPUTE_INTERVAL: u64 = 15;
/// The Town Center's sight radius in tiles (M7, VIS-02/03): the
/// unit table's Town Center row carries no LoS (it never acquires
/// targets), so the fog model needs its own value. ADOPTED —
/// building LoS values are not pinned by the project docs
/// (UNVERIFIED); Town Watch / Town Patrol (+4) are not implemented.
pub const TC_SIGHT_TILES: u32 = 6;
/// A placed building's sight radius in tiles (M7, VIS-02/03).
/// ADOPTED for every kind — see `TC_SIGHT_TILES` (UNVERIFIED).
pub const BUILDING_SIGHT_TILES: u32 = 4;
/// The hard cap on any sight radius, in tiles (VIS-04).
pub const MAX_SIGHT_TILES: u32 = 20;

/// World position clamp: entities never leave [-239, 239].
/// (const via raw bits: 239 × 2^16 = 15663104.)
const MIN_POS: I16F16 = I16F16::from_bits(-15_663_104);
const MAX_POS: I16F16 = I16F16::from_bits(15_663_104);

/// Maximum queued orders held behind an entity's current order
/// (M4, SEL-12). A queue-deeper-than-playable bound: shift-click
/// chains in real play are a handful of orders; beyond the cap,
/// further `Queue` commands are ignored. (An implementation bound —
/// the rulebook records no cap for the reference game.)
pub const ORDER_QUEUE_CAP: usize = 16;

/// Commands are *data*, pushed by the outside world and applied in FIFO
/// order at tick start. This shape is what makes future lockstep
/// multiplayer and replays possible: a match is (seed, settings,
/// commands).
///
/// M1 adds `Move`; M2 adds `Gather` and `TrainVillager`; M2.5 adds
/// `Build`; M3 adds `Attack` and `TrainAt`; M4 adds `Queue` and
/// `SetRally`; M5 adds `TrainVillagerAt` (the per-Town-Center form
/// the AI opponent uses — `TrainVillager` remains the human Town
/// Center shorthand) and `AttackBuilding`; M6 adds the research
/// commands (`ResearchTc`, `ResearchTcAt`, `ResearchAt`) and
/// `ForceDeposit` (the force-drop-off gesture, ECO-07).
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
    /// Enqueue a villager at a specific Town Center (M5) — the form
    /// the AI opponent uses for its own Town Center; identical rules
    /// to `TrainVillager`, charged to that Town Center's owner.
    TrainVillagerAt { tc: u32 },
    /// Order a villager to construct a building whose square footprint
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
    /// Order a unit to attack a target entity (M3). The attacker
    /// chases and strikes until the target dies or a new order lands.
    /// Valid attackers: any living player unit with an attack row
    /// (villager, Scout, Militia, Archer). Valid targets: Gaia
    /// entities, the player's own sheep (killing your own
    /// herdables is an ordinary order — ECO-13), and — since M5 —
    /// any entity of *another* player, its Town Center included.
    /// Anything else is ignored.
    Attack { entity: u32, target: u32 },
    /// Order a unit to attack a placed building (M5): chase its
    /// footprint and strike until it is destroyed or a new order
    /// lands. Valid attackers: any living player unit with an attack
    /// row (same as `Attack`). Valid targets: a building owned by
    /// *another* player (your own buildings are never targets).
    /// Buildings have HP and armor from the study's building table
    /// (see `BuildingKind::max_hp`).
    AttackBuilding { entity: u32, building: u32 },
    /// Enqueue a unit at a production building (M3: the Barracks
    /// trains Militia and Archers). The cost is deducted *on enqueue*;
    /// rejected (no deduction) when the building is missing,
    /// unfinished, not a production building, the kind is not
    /// trainable there, funds are short, or the queue is full (15).
    TrainAt { building: u32, kind: EntityKind },
    /// Queue an order behind an entity's current one (M4, SEL-12 —
    /// the Shift+click form of an order). The entity executes its
    /// current order first, then the queued ones FIFO. A *non-queued*
    /// command for the same entity replaces its current order and
    /// clears the whole queue (the pre-M4 semantics). Queued orders
    /// for an idle entity start on the same tick's queue-advance step.
    Queue { entity: u32, order: QueuedOrder },
    /// Set a rally point (M4, SEL-13): `building: None` targets the
    /// Town Center, `Some(id)` a production building. The point is
    /// clamped into the world; if it lands within snap range of a
    /// live resource node it also records that node, so new villagers
    /// go straight to work on it. Rally points are pure data — they
    /// take effect when the next unit finishes training there.
    SetRally {
        building: Option<u32>,
        x: I16F16,
        y: I16F16,
    },
    /// Enqueue a technology at the human player's Town Center (M6):
    /// Loom or the Feudal Age advancement — the only technologies
    /// the Town Center hosts. The cost is charged on enqueue and the
    /// item shares the queue (and its head-of-line progress) with
    /// villager training (TEC-03/05). Rejected silently when the
    /// tech is foreign to the Town Center, already researched, its
    /// requirements are unmet, the queue is full, or funds are
    /// short (see `Sim::research_block_tc`).
    ResearchTc { tech: TechId },
    /// Enqueue a technology at a specific Town Center (M6) — the
    /// form the AI opponent uses for its own Town Center; identical
    /// rules to `ResearchTc`, charged to that Town Center's owner.
    ResearchTcAt { tc: u32, tech: TechId },
    /// Enqueue a technology at a placed building (M6): the
    /// Blacksmith's lines, or the Man-at-Arms upgrade at the
    /// Barracks. The item shares the building's queue with any
    /// units in line (TEC-03). Same rejection discipline as
    /// `ResearchTc` (see `Sim::research_block_at`).
    ResearchAt { building: u32, tech: TechId },
    /// Force drop-off (M6, ECO-07): a villager carrying resources is
    /// ordered to haul them to the nearest valid drop-off for what
    /// it carries *now* — and, the deposit made, to return to its
    /// previous work on its own: its gather assignment (or its
    /// construction site) is kept as the resume marker and the work
    /// cycle resumes from it. This is the order behind the player's
    /// right-click on their own Town Center or camp. Rejected when
    /// the villager carries nothing or no valid drop-off stands.
    ForceDeposit { entity: u32 },
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

/// What kind of game a `Sim` is (M5). Sandbox is the pre-M5 free
/// build: one player, no opponent, no win/lose — preserved exactly.
/// The `Ai*` modes spawn the mirrored opponent start and run the
/// scripted AI (see the `ai` module); the tag only selects the AI's
/// *behavior* profile (AI-03: difficulty changes behavior, never
/// physics — costs, stats and the damage formula are identical).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchMode {
    /// One player, no opponent — the M0–M4 sandbox.
    Sandbox,
    /// Scripted opponent, gentler profile (slower decisions, smaller
    /// economy/army targets, smaller and rarer attack waves).
    AiEasy,
    /// Scripted opponent, full profile.
    AiNormal,
}

impl MatchMode {
    /// Stable tag for the state hash and the WASM ABI:
    /// 0 sandbox, 1 AI Easy, 2 AI Normal.
    pub fn tag(self) -> u8 {
        match self {
            MatchMode::Sandbox => 0,
            MatchMode::AiEasy => 1,
            MatchMode::AiNormal => 2,
        }
    }

    /// Inverse of `tag`; unknown values map to `None`.
    pub fn from_tag(tag: u8) -> Option<MatchMode> {
        match tag {
            0 => Some(MatchMode::Sandbox),
            1 => Some(MatchMode::AiEasy),
            2 => Some(MatchMode::AiNormal),
            _ => None,
        }
    }

    /// Whether this mode has an opponent (and therefore a match to
    /// win or lose).
    pub fn is_match(self) -> bool {
        self != MatchMode::Sandbox
    }
}

/// The match outcome, from the human player's seat (M5, VIC-01).
/// Derived from state on every call — elimination is monotonic (the
/// dead never come back and a player with no Town Center and no units
/// can never produce again), so the result can never flip back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchState {
    /// The match is still being played (always, in Sandbox).
    Ongoing,
    /// The opponent has been eliminated (or resigned, AI-05).
    Victory,
    /// The human player has been eliminated.
    Defeat,
}

impl MatchState {
    /// WASM ABI tag: 0 ongoing, 1 victory, 2 defeat.
    pub fn tag(self) -> u8 {
        match self {
            MatchState::Ongoing => 0,
            MatchState::Victory => 1,
            MatchState::Defeat => 2,
        }
    }
}

/// One Town Center's production state (M5): its production queue,
/// progress, and rally point. Before M5 these lived as loose fields
/// on `Sim` for the single Town Center; a match has two Town Centers
/// (one per player), each with its own — the state a `TrainVillagerAt`
/// command and the economy phase operate on. Part of the hashed state.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TcState {
    /// The Town Center entity this state belongs to.
    tc_id: u32,
    /// The shared production queue (M6, TEC-03): villagers in
    /// training and technologies in research, head first — before
    /// M6 this was a bare villager count. Progress accrues on the
    /// head only; a unit head stalls while its owner is housed, a
    /// research head does not (population gates units, not ideas).
    queue: Vec<ProdEntry>,
    /// Ticks of progress on the queue's head entry.
    progress: u64,
    /// Rally point (SEL-13): where a freshly trained villager walks
    /// on spawn. `None` = unset — the villager stays at its spawn
    /// point just outside the Town Center (the reference game's
    /// default). `rally_node` is the node the point snapped to, if
    /// any: the new villager gathers it instead of walking.
    rally: Option<(I16F16, I16F16)>,
    rally_node: Option<u32>,
}

impl TcState {
    /// Villagers waiting in this queue (research entries do not
    /// occupy population — M6).
    fn queued_villagers(&self) -> u32 {
        self.queue
            .iter()
            .filter(|e| matches!(e, ProdEntry::Unit(_)))
            .count() as u32
    }
}

/// One player's progression state (M6): the age they stand in and
/// the technologies they have researched — per player slot, like
/// the stockpiles. Effects are never stored on units; every stat
/// lookup layers this state over the base table (TEC-06, see
/// `Sim::stats_for`). Part of the hashed state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PlayerState {
    age: Age,
    /// Bitmask of researched `TechId`s (`TechId::bit`). The Feudal
    /// Age advancement is recorded by `age` itself, not a bit.
    techs: u64,
}

/// The headless deterministic simulation.
pub struct Sim {
    tick: u64,
    rng: Pcg32,
    grid: Grid,
    /// What kind of game this is (M5): sandbox, or a match against
    /// the scripted AI at one of the two behavior profiles. Set at
    /// construction; a match's definition is (seed, mode, commands).
    mode: MatchMode,
    /// All entities, always sorted by `id`. Never a HashMap.
    /// Entity 0 is the human Town Center; in a match the opponent's
    /// Town Center and start follow the standard layout.
    entities: Vec<Entity>,
    /// FIFO command queue, drained at the start of every tick.
    commands: Vec<Command>,
    /// The players' resource stockpiles (M2, per-player since M5),
    /// indexed by player slot (see `player_slot`): slot 0 is the
    /// human, slot 1 the opponent. In Sandbox slot 1 simply sits at
    /// its starting values, unused.
    stockpiles: [Stockpile; 2],
    /// The players' progression state (M6): age + researched
    /// technologies, indexed by player slot like the stockpiles.
    players: [PlayerState; 2],
    /// Resource nodes in id order (M2). Depleted nodes stay in the list
    /// with zero amount — inert, but still hashed.
    nodes: Vec<ResourceNode>,
    /// Per-Town-Center production state (M5), in Town Center spawn
    /// order: the human's first, the opponent's second (matches only).
    tc_states: Vec<TcState>,
    /// Placed buildings in id order (M2.5): under construction and
    /// complete. A completed Farm's record stays while its node lives
    /// (footprint registry), then retires with it.
    buildings: Vec<Building>,
    /// Next building id. Buildings are occasionally removed (depleted
    /// farms, M5 destruction), so ids cannot be derived from the list
    /// length.
    next_building_id: u32,
    /// Projectiles in flight (M3), in spawn order.
    projectiles: Vec<Projectile>,
    /// The scripted opponent's bookkeeping (M5) — wave scheduling
    /// only; every *decision* reads live sim state, so nothing else
    /// needs storing. Inert in Sandbox.
    ai: ai::AiState,
    /// Fog of war (M7, VIS-01): per-player-slot visibility grids,
    /// row-major over the map, one byte per cell — 0 unexplored,
    /// 1 explored (remembered), 2 currently visible. Rebuilt on
    /// `FOG_RECOMPUTE_INTERVAL` by `recompute_visibility`; part of
    /// the hashed state, so a replay must reproduce every player's
    /// knowledge exactly (and the AI's fog-bound decisions replay
    /// with it).
    fog: [Vec<u8>; 2],
    /// The cell indices currently at visibility 2, per slot — the
    /// demotion list for the next recompute (explored memory is
    /// what remains when a cell leaves this list). Bookkeeping for
    /// `fog`; derivable from it, so not hashed separately.
    fog_visible: [Vec<u32>; 2],
}

impl Sim {
    /// Create a new sandbox simulation from a seed — the pre-M5
    /// game: one player, no opponent, no win/lose. Since M7 the
    /// starting layout is *generated* from the seed (the Arabia
    /// generator — see `with_mode`); the seed also feeds the
    /// in-state RNG, whose words are hashed every tick, keeping
    /// seeds divergent from tick one. Combat is the first system
    /// that draws from it (archer accuracy, PRJ-03).
    pub fn new(seed: u64) -> Self {
        Self::with_mode(seed, MatchMode::Sandbox)
    }

    /// Create a match against the scripted AI opponent (M5):
    /// `MatchMode::AiEasy` or `MatchMode::AiNormal`. Both starts
    /// come from the seeded generator; the opponent's is the
    /// human's rotational mirror (see `spawn_opponent_start`).
    pub fn new_match(seed: u64, mode: MatchMode) -> Self {
        debug_assert!(mode.is_match());
        Self::with_mode(seed, mode)
    }

    /// The shared constructor (M7): the map comes from the seeded
    /// Arabia generator (`mapgen::generate` — terrain, heights and
    /// both starts' packages), the human start spawns from package
    /// 0, and in a match the opponent spawns from package 1 (the
    /// generated mirror). The in-state RNG is seeded exactly as
    /// before and feeds only gameplay draws — generation runs on
    /// its own derived stream, so combat's draw sequence is
    /// untouched by however many candidates generation tried.
    pub fn with_mode(seed: u64, mode: MatchMode) -> Self {
        let rng = Pcg32::new(seed);
        let map = mapgen::generate(seed);
        let grid = map.grid.clone();
        let pkg = &map.starts[0];
        let (tc_x, tc_y) = grid.cell_center(pkg.tc_cell.0, pkg.tc_cell.1);

        let mut entities = Vec::with_capacity(64);
        entities.push(Entity::town_center(TOWN_CENTER_ID, tc_x, tc_y));
        for (i, &(ox, oy)) in START_VILLAGER_OFFSETS.iter().enumerate() {
            entities.push(Entity::villager(
                TOWN_CENTER_ID + 1 + i as u32,
                tc_x + I16F16::from_num(ox),
                tc_y + I16F16::from_num(oy),
            ));
        }
        // The starting Scout Cavalry (M3, OPENING-SPEC §2.1).
        let mut next_id = TOWN_CENTER_ID + 1 + START_VILLAGER_COUNT;
        entities.push(Entity::spawned(
            next_id,
            EntityKind::ScoutCavalry,
            entity::OWNER_PLAYER,
            tc_x + I16F16::from_num(START_SCOUT_OFFSET.0),
            tc_y + I16F16::from_num(START_SCOUT_OFFSET.1),
        ));
        next_id += 1;
        // Gaia animals at their generated cells, in the package's
        // fixed order (sheep, boars, deer) — the historical id
        // layout: 5–12 sheep, 13–14 boars, 15–20 deer.
        for &(kind, cx, cy) in &pkg.animals {
            let (pos_x, pos_y) = grid.cell_center(cx, cy);
            entities.push(Entity::spawned(
                next_id,
                kind,
                entity::OWNER_GAIA,
                pos_x,
                pos_y,
            ));
            next_id += 1;
        }

        // Nodes: the human's package, then (matches) the opponent's,
        // then the neutral dressing — ids in that fixed order.
        let mut nodes: Vec<ResourceNode> = Vec::new();
        let mut node_specs: Vec<(NodeKind, u32, u32)> = Vec::new();
        node_specs.extend_from_slice(&pkg.nodes);
        if mode.is_match() {
            node_specs.extend_from_slice(&map.starts[1].nodes);
        }
        node_specs.extend_from_slice(&map.neutral_nodes);
        for (i, &(kind, cx, cy)) in node_specs.iter().enumerate() {
            let (pos_x, pos_y) = grid.cell_center(cx, cy);
            nodes.push(ResourceNode::placed(i as u32, kind, pos_x, pos_y));
        }

        let cell_total = (GRID_DIM * GRID_DIM) as usize;
        let mut sim = Sim {
            tick: 0,
            rng,
            grid,
            mode,
            entities,
            commands: Vec::new(),
            stockpiles: [Stockpile::starting(), Stockpile::starting()],
            players: [PlayerState::default(), PlayerState::default()],
            nodes,
            tc_states: vec![TcState {
                tc_id: TOWN_CENTER_ID,
                queue: Vec::new(),
                progress: 0,
                rally: None,
                rally_node: None,
            }],
            buildings: Vec::new(),
            next_building_id: 0,
            projectiles: Vec::new(),
            ai: ai::AiState::default(),
            fog: [vec![0; cell_total], vec![0; cell_total]],
            fog_visible: [Vec::new(), Vec::new()],
        };
        if mode.is_match() {
            sim.spawn_opponent_start(&map.starts[1]);
        }
        // VIS-07: a match starts nearly blind — only each start's
        // surroundings are known, from this first computation.
        sim.recompute_visibility();
        sim
    }

    /// The opponent's start (M5, generated in M7): package 1 of the
    /// generated map — its own Town Center, 3 villagers and a Scout
    /// at the mirrored offsets, and its own full animal set (sheep,
    /// boars, deer — all Gaia-owned, as placed). The package is the
    /// human's mirrored through the map center by the generator, so
    /// the start is exactly as strong as the human's.
    fn spawn_opponent_start(&mut self, pkg: &mapgen::StartPackage) {
        let (tc_x, tc_y) = self.grid.cell_center(pkg.tc_cell.0, pkg.tc_cell.1);
        let mut next_id = self.entities.len() as u32;

        // Town Center (an entity, like the human's).
        let enemy_tc = next_id;
        self.entities.push(Entity::spawned(
            enemy_tc,
            EntityKind::TownCenter,
            entity::OWNER_OPPONENT,
            tc_x,
            tc_y,
        ));
        next_id += 1;
        self.tc_states.push(TcState {
            tc_id: enemy_tc,
            queue: Vec::new(),
            progress: 0,
            rally: None,
            rally_node: None,
        });
        // Villagers and Scout at the mirrored start offsets (the
        // generator keeps the Town Center's 3×3 clear for them).
        for &(ox, oy) in START_VILLAGER_OFFSETS.iter() {
            self.entities.push(Entity::spawned(
                next_id,
                EntityKind::Villager,
                entity::OWNER_OPPONENT,
                tc_x - I16F16::from_num(ox),
                tc_y - I16F16::from_num(oy),
            ));
            next_id += 1;
        }
        self.entities.push(Entity::spawned(
            next_id,
            EntityKind::ScoutCavalry,
            entity::OWNER_OPPONENT,
            tc_x - I16F16::from_num(START_SCOUT_OFFSET.0),
            tc_y - I16F16::from_num(START_SCOUT_OFFSET.1),
        ));
        next_id += 1;
        // The opponent's own animals at their generated cells.
        for &(kind, cx, cy) in &pkg.animals {
            let (pos_x, pos_y) = self.grid.cell_center(cx, cy);
            self.entities.push(Entity::spawned(
                next_id,
                kind,
                entity::OWNER_GAIA,
                pos_x,
                pos_y,
            ));
            next_id += 1;
        }
    }

    /// Rebuild both players' visibility grids (M7, VIS-01/02):
    /// every cell currently seen drops to *explored* (memory
    /// persists), then every living unit and standing building of
    /// each player stamps its sight disc as *visible*. Sight is a
    /// pure radius (VIS-05: no occlusion, no raycasting), from the
    /// unit table's LoS via the owner's effective row (a Feudal
    /// Scout sees 6 tiles, VIS-03), capped at 20 tiles (VIS-04);
    /// buildings use the adopted sight constants. Runs at match
    /// creation and every `FOG_RECOMPUTE_INTERVAL` ticks — bounded
    /// work: sources × disc area, independent of map area.
    fn recompute_visibility(&mut self) {
        for slot in 0..2usize {
            let owner = if slot == 0 {
                entity::OWNER_PLAYER
            } else {
                entity::OWNER_OPPONENT
            };
            for i in std::mem::take(&mut self.fog_visible[slot]) {
                self.fog[slot][i as usize] = 1; // visible → explored
            }
            for idx in 0..self.entities.len() {
                let e = &self.entities[idx];
                if e.dead || e.owner != owner {
                    continue;
                }
                let r = if e.kind == EntityKind::TownCenter {
                    TC_SIGHT_TILES
                } else {
                    let los = self.unit_stats(idx).los;
                    (los / CELL_SIZE).to_num::<i32>().clamp(0, MAX_SIGHT_TILES as i32) as u32
                };
                if r == 0 {
                    continue;
                }
                let (cx, cy) = self.grid.world_to_cell(e.pos_x, e.pos_y);
                self.stamp_sight(slot, cx, cy, r);
            }
            for bi in 0..self.buildings.len() {
                let b = &self.buildings[bi];
                if b.owner != owner {
                    continue;
                }
                let (cx, cy) = self.grid.world_to_cell(b.pos_x, b.pos_y);
                self.stamp_sight(slot, cx, cy, BUILDING_SIGHT_TILES);
            }
        }
    }

    /// Stamp one sight disc (radius `r` tiles, integer disc) as
    /// visible for a player slot.
    fn stamp_sight(&mut self, slot: usize, cx: u32, cy: u32, r: u32) {
        let r = r as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                if x < 0 || y < 0 || x >= GRID_DIM as i32 || y >= GRID_DIM as i32 {
                    continue;
                }
                let i = ((y as u32) * GRID_DIM + (x as u32)) as usize;
                if self.fog[slot][i] != 2 {
                    self.fog[slot][i] = 2;
                    self.fog_visible[slot].push(i as u32);
                }
            }
        }
    }

    /// A player's fog-of-war grid (M7, VIS-01): row-major bytes over
    /// the whole map — 0 unexplored, 1 explored, 2 currently
    /// visible. Gaia (and unknown tags) have no grid: an empty
    /// slice. This is the ABI's fog source and the AI's only
    /// window on the enemy.
    pub fn visibility(&self, owner: u8) -> &[u8] {
        match player_slot(owner) {
            Some(slot) => &self.fog[slot],
            None => &[],
        }
    }

    /// One cell's visibility state for a player (0/1/2 as in
    /// `visibility`); out-of-grid cells and non-players read 0.
    pub fn cell_visibility(&self, owner: u8, cx: u32, cy: u32) -> u8 {
        if cx >= GRID_DIM || cy >= GRID_DIM {
            return 0;
        }
        match player_slot(owner) {
            Some(slot) => self.fog[slot][(cy * GRID_DIM + cx) as usize],
            None => 0,
        }
    }

    /// Whether a cell is *currently visible* to a player.
    pub fn cell_visible_to(&self, owner: u8, cx: u32, cy: u32) -> bool {
        self.cell_visibility(owner, cx, cy) == 2
    }

    /// Whether a cell has ever been seen by a player (explored or
    /// currently visible — VIS-06's remembered knowledge).
    pub fn cell_explored_by(&self, owner: u8, cx: u32, cy: u32) -> bool {
        self.cell_visibility(owner, cx, cy) >= 1
    }

    /// The elevation level (0..=4) of the cell containing a world
    /// point — MAP-05 data for the combat factor and the renderer.
    pub fn height_at_world(&self, x: I16F16, y: I16F16) -> u8 {
        let (cx, cy) = self.grid.world_to_cell(x, y);
        self.grid.height_at(cx, cy)
    }

    /// The elevation level of the cell an entity stands on.
    pub(crate) fn entity_height(&self, idx: usize) -> u8 {
        let e = &self.entities[idx];
        self.height_at_world(e.pos_x, e.pos_y)
    }
    /// Queue a command to be applied at the start of the next tick.
    pub fn push_command(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// The navigation grid (read-only; obstacles are static).
    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    /// The human player's resource stockpile (read-only).
    pub fn stockpile(&self) -> &Stockpile {
        &self.stockpiles[0]
    }

    /// Any player's resource stockpile by owner tag (M5). Gaia has
    /// no stockpile; asking for it (or an unknown tag) yields the
    /// human's — callers pass player tags.
    pub fn stockpile_of(&self, owner: u8) -> &Stockpile {
        match player_slot(owner) {
            Some(slot) => &self.stockpiles[slot],
            None => &self.stockpiles[0],
        }
    }

    /// The mutable stockpile of a player (slot-resolved like
    /// `stockpile_of`).
    fn stockpile_mut(&mut self, owner: u8) -> &mut Stockpile {
        let slot = player_slot(owner).unwrap_or(0);
        &mut self.stockpiles[slot]
    }

    /// What kind of game this is (M5).
    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    /// The age a player stands in (M6, TEC-01). Gaia has no age;
    /// asking for it yields the Dark Age.
    pub fn age_of(&self, owner: u8) -> Age {
        match player_slot(owner) {
            Some(slot) => self.players[slot].age,
            None => Age::Dark,
        }
    }

    /// Whether a player has completed a technology (M6). The
    /// Feudal Age "technology" reads off the age state itself.
    pub fn has_tech(&self, owner: u8, tech: TechId) -> bool {
        if tech == TechId::FeudalAge {
            return self.age_of(owner) >= Age::Feudal;
        }
        match player_slot(owner) {
            Some(slot) => self.players[slot].techs & tech.bit() != 0,
            None => false,
        }
    }

    /// A player's researched-technology bitmask (M6), with the
    /// Feudal Age bit set while the player stands in Feudal — the
    /// client's one-call view of "what do I have".
    pub fn tech_mask_of(&self, owner: u8) -> u64 {
        let mut mask = match player_slot(owner) {
            Some(slot) => self.players[slot].techs,
            None => 0,
        };
        if self.age_of(owner) >= Age::Feudal {
            mask |= TechId::FeudalAge.bit();
        }
        mask
    }

    /// The effective stat row of a unit kind for an owner (M6,
    /// TEC-06): the base table row layered with the owner's age
    /// (CLS-04 — the Scout's Feudal row) and researched technologies
    /// (Loom, Fletching, the Blacksmith armor lines). Every stat
    /// consumer — movement speed, combat, the HP display — reads
    /// through here, so a completed research changes the whole
    /// empire's numbers the tick it lands, with no per-unit state
    /// to sweep (the two completion *sweeps* that do exist — Loom's
    /// current-HP grant and the Man-at-Arms kind swap — are in
    /// `apply_research`). Gaia's rows are never patched.
    pub fn stats_for(&self, owner: u8, kind: EntityKind) -> UnitSpec {
        let mut s = *units::spec(kind);
        let Some(slot) = player_slot(owner) else {
            return s;
        };
        let p = self.players[slot];
        // CLS-04: the Scout Cavalry's automatic Feudal row —
        // attack 3 → 5, speed 1.20 → 1.55 tiles/s, LoS 4 → 6.
        if kind == EntityKind::ScoutCavalry && p.age >= Age::Feudal {
            s.attacks = units::SCOUT_FEUDAL_ATTACKS;
            s.speed = I16F16::from_bits(126_976); // 1.9375 (1.55 × 1.25)
            s.los = I16F16::from_bits(1_572_864); // 24 (6 tiles)
        }
        // TEC-04 (Loom): villagers +15 HP, +1 melee / +2 pierce
        // armor, +0.1 tiles/s (RULEBOOK TEC-06/CLS-05 — see the
        // tech module docs for the study discrepancy note).
        if kind == EntityKind::Villager && p.techs & TechId::Loom.bit() != 0 {
            s.max_hp += 15;
            s.armor_melee += 1;
            s.armor_pierce += 2;
            s.speed += I16F16::from_bits(8_192); // +0.125 (0.1 × 1.25)
        }
        // Fletching (study §8): archers +1 attack, +1 tile of range.
        if kind == EntityKind::Archer && p.techs & TechId::Fletching.bit() != 0 {
            s.attacks = units::ARCHER_FLETCHING_ATTACKS;
            s.range_max += I16F16::from_bits(262_144); // +4.0 (1 tile)
        }
        // Scale Mail Armor (study §8): infantry +1/+1 armor.
        if matches!(kind, EntityKind::Militia | EntityKind::ManAtArms)
            && p.techs & TechId::ScaleMail.bit() != 0
        {
            s.armor_melee += 1;
            s.armor_pierce += 1;
        }
        // Padded Archer Armor (study §8): archers +1/+1 armor.
        if kind == EntityKind::Archer && p.techs & TechId::PaddedArcherArmor.bit() != 0 {
            s.armor_melee += 1;
            s.armor_pierce += 1;
        }
        s
    }

    /// The effective stat row of the entity at `idx` (M6): its
    /// owner's row via `stats_for`. The combat phase's standard
    /// read — damage, range, LoS and HP all key on the owner.
    pub(crate) fn unit_stats(&self, idx: usize) -> UnitSpec {
        let e = &self.entities[idx];
        self.stats_for(e.owner, e.kind)
    }

    /// How many completed buildings of `kind` a player owns (M6) —
    /// the built-set half of BLD-15's availability rule.
    pub fn completed_buildings(&self, owner: u8, kind: BuildingKind) -> u32 {
        self.buildings
            .iter()
            .filter(|b| b.owner == owner && b.kind == kind && b.complete)
            .count() as u32
    }

    /// Whether a player owns a completed building of `kind` (M6).
    fn has_completed_building(&self, owner: u8, kind: BuildingKind) -> bool {
        self.completed_buildings(owner, kind) > 0
    }

    /// Completed buildings counting toward the Feudal Age research
    /// requirement (M6, TEC-01): Dark-Age buildings other than
    /// Houses and Farms (see `BuildingKind::counts_toward_feudal`).
    pub fn feudal_building_count(&self, owner: u8) -> u32 {
        self.buildings
            .iter()
            .filter(|b| b.owner == owner && b.complete && b.kind.counts_toward_feudal())
            .count() as u32
    }

    /// A Town Center's production state, by Town Center entity id.
    fn tc_state(&self, tc_id: u32) -> Option<&TcState> {
        self.tc_states.iter().find(|s| s.tc_id == tc_id)
    }

    /// The living Town Center entity owned by `owner`, if any.
    fn tc_entity_of(&self, owner: u8) -> Option<&Entity> {
        self.entities.iter().find(|e| {
            e.kind == EntityKind::TownCenter && e.owner == owner && !e.dead
        })
    }

    /// The nearest valid drop-off for `owner`'s load of `kind`,
    /// measured from `(from_x, from_y)` (M6, ECO-06): the owner's
    /// living Town Center (accepts everything, distance to its
    /// center) and the owner's *completed* drop-off buildings that
    /// accept the resource (distance to the footprint's edge, the
    /// same clamp math combat uses against buildings). Nearest by
    /// straight-line distance; ties break Town Center first, then
    /// lowest building id (scan order + strictly-nearer replaces).
    /// `None` when nowhere accepts the load — the villager keeps
    /// its carry and idles, the pre-M6 no-Town-Center behavior.
    fn deposit_target(
        &self,
        owner: u8,
        kind: ResourceKind,
        from_x: I16F16,
        from_y: I16F16,
    ) -> Option<Dropoff> {
        let mut best: Option<(i64, Dropoff)> = None;
        if let Some(tc) = self.tc_entity_of(owner) {
            let d = steer::vec_len(tc.pos_x - from_x, tc.pos_y - from_y);
            let db = d.to_bits() as i64;
            best = Some((db * db, Dropoff::TownCenter));
        }
        for b in &self.buildings {
            if b.owner != owner || !b.complete || !b.kind.accepts(kind) {
                continue;
            }
            let d = self.distance_to_footprint(from_x, from_y, b.pos_x, b.pos_y, b.kind);
            let db = d.to_bits() as i64;
            let key = db * db;
            if best.map(|(bk, _)| key < bk).unwrap_or(true) {
                best = Some((key, Dropoff::Building(b.id)));
            }
        }
        best.map(|(_, t)| t)
    }

    /// Distance from a point to a building's footprint rectangle
    /// (0 when the point is beside or inside it): the point clamped
    /// into the rect, then the offset length — combat's building
    /// geometry, shared.
    fn distance_to_footprint(
        &self,
        px: I16F16,
        py: I16F16,
        bx: I16F16,
        by: I16F16,
        kind: BuildingKind,
    ) -> I16F16 {
        let half = I16F16::from_num(kind.footprint_size() as i32 * 2);
        let qx = (px - bx).clamp(-half, half) + bx;
        let qy = (py - by).clamp(-half, half) + by;
        steer::vec_len(px - qx, py - qy)
    }

    /// The world point a hauler at `(from_x, from_y)` steers toward
    /// for a deposit target (M6). The Town Center is its center —
    /// it never blocks the nav grid, so pathing straight at it
    /// works (the M2 behavior). A camp's center is inside its own
    /// solid footprint, and `find_path` refuses a blocked goal — so
    /// a camp is steered to the point 2 units outside its
    /// footprint's edge along the hauler's approach line (the
    /// footprint-clamp point pushed outward): inside the deposit
    /// range by construction, in a free ring cell in every normal
    /// placement.
    fn dropoff_steer_point(
        &self,
        owner: u8,
        target: Dropoff,
        from_x: I16F16,
        from_y: I16F16,
    ) -> Option<(I16F16, I16F16)> {
        match target {
            Dropoff::TownCenter => {
                self.tc_entity_of(owner).map(|tc| (tc.pos_x, tc.pos_y))
            }
            Dropoff::Building(bid) => {
                let bi = self.building_index(bid)?;
                let b = &self.buildings[bi];
                let half = I16F16::from_num(b.kind.footprint_size() as i32 * 2);
                let qx = from_x.clamp(b.pos_x - half, b.pos_x + half);
                let qy = from_y.clamp(b.pos_y - half, b.pos_y + half);
                let (dx, dy) = (from_x - qx, from_y - qy);
                if dx == I16F16::ZERO && dy == I16F16::ZERO {
                    // The hauler is somehow inside the footprint:
                    // steer to the east edge's standing point.
                    return Some((b.pos_x + half + I16F16::from_num(2), b.pos_y));
                }
                let len = steer::vec_len(dx, dy);
                let out = I16F16::from_num(2);
                Some((qx + dx * out / len, qy + dy * out / len))
            }
        }
    }

    /// Whether a hauler of `owner` at `(px, py)` has arrived at its
    /// deposit target (M6): the Town Center by the center-distance
    /// `DEPOSIT_RANGE` rule M2 shipped; a camp by distance to the
    /// footprint's edge against the same range (a camp's center is
    /// unreachable — its footprint is solid). A target that no
    /// longer exists has not been arrived at (the caller
    /// re-resolves instead).
    fn dropoff_arrived(&self, owner: u8, px: I16F16, py: I16F16, target: Dropoff) -> bool {
        match target {
            Dropoff::TownCenter => match self.tc_entity_of(owner) {
                Some(tc) => steer::vec_len(px - tc.pos_x, py - tc.pos_y) <= DEPOSIT_RANGE,
                None => false,
            },
            Dropoff::Building(bid) => match self.building_index(bid) {
                Some(bi) => {
                    let b = &self.buildings[bi];
                    self.distance_to_footprint(px, py, b.pos_x, b.pos_y, b.kind)
                        <= DEPOSIT_RANGE
                }
                None => false,
            },
        }
    }

    /// The match outcome from the human's seat (M5, VIC-01): in a
    /// match, a player with no Town Center, no placed buildings
    /// (under construction counts) and no living population units
    /// has lost; if both fall on the same tick the human loses the
    /// tie (checked first — the same precedence beats even the
    /// AI's resignation). The AI also resigns earlier (AI-05):
    /// no Town Center and no villagers means no path to rebuild.
    /// Sandbox is always `Ongoing`.
    pub fn match_state(&self) -> MatchState {
        if !self.mode.is_match() {
            return MatchState::Ongoing;
        }
        if self.eliminated(entity::OWNER_PLAYER) {
            return MatchState::Defeat;
        }
        if self.ai_resigned() {
            return MatchState::Victory;
        }
        if self.eliminated(entity::OWNER_OPPONENT) {
            return MatchState::Victory;
        }
        MatchState::Ongoing
    }

    /// AI-05 resignation: the opponent has no Town Center and no
    /// villagers — it cannot rebuild its economy, so it resigns
    /// rather than playing to the last unit.
    fn ai_resigned(&self) -> bool {
        self.tc_entity_of(entity::OWNER_OPPONENT).is_none()
            && !self.entities.iter().any(|e| {
                e.owner == entity::OWNER_OPPONENT
                    && e.kind == EntityKind::Villager
                    && !e.dead
            })
    }

    /// Conquest elimination (VIC-01, M5 form): no living Town Center,
    /// no placed buildings at all, and no living population units.
    fn eliminated(&self, owner: u8) -> bool {
        if self.tc_entity_of(owner).is_some() {
            return false;
        }
        if self.buildings.iter().any(|b| b.owner == owner) {
            return false;
        }
        !self.entities.iter().any(|e| {
            e.owner == owner && !e.dead && e.kind.counts_population()
        })
    }

    /// Resource nodes in id order, depleted ones included (amount zero).
    pub fn nodes(&self) -> &[ResourceNode] {
        &self.nodes
    }

    /// Entries waiting in the human Town Center's production queue
    /// (M6: villagers in training *and* technologies in research —
    /// the queue is shared, TEC-03).
    pub fn train_queue_len(&self) -> u32 {
        self.tc_state(TOWN_CENTER_ID)
            .map(|s| s.queue.len() as u32)
            .unwrap_or(0)
    }

    /// The human Town Center's queue entries in order (M6) — for the
    /// client's queue display (head first).
    pub fn tc_queue(&self) -> &[ProdEntry] {
        match self.tc_state(TOWN_CENTER_ID) {
            Some(s) => &s.queue,
            None => &[],
        }
    }

    /// A placed building's queue entries in order (M6). Empty for an
    /// unknown building.
    pub fn building_queue(&self, building: u32) -> &[ProdEntry] {
        match self.building_index(building) {
            Some(bi) => &self.buildings[bi].queue,
            None => &[],
        }
    }

    /// Progress on the human Town Center queue's head entry as a
    /// fraction in raw bits (0 … 65536) — the `train_frac_bits`
    /// counterpart for the Town Center (M6).
    pub fn tc_progress_bits(&self) -> i32 {
        let s = match self.tc_state(TOWN_CENTER_ID) {
            Some(s) => s,
            None => return 0,
        };
        match s.queue.first() {
            None => 0,
            Some(&head) => {
                let total = head.ticks() as i64;
                if total <= 0 {
                    return 0;
                }
                let done = (s.progress as i64).min(total);
                ((done * 65536) / total) as i32
            }
        }
    }

    /// Placed buildings in id order, under construction and complete.
    pub fn buildings(&self) -> &[Building] {
        &self.buildings
    }

    /// Living population of the human player: alive player units
    /// that occupy a slot — villagers, the Scout, Militia, Archers
    /// (M3). Captured sheep and the Town Center never count (see
    /// `EntityKind::counts_population`).
    pub fn living_population(&self) -> u32 {
        self.living_population_of(entity::OWNER_PLAYER)
    }

    /// Living population of one player (M5): the per-player form of
    /// `living_population`.
    pub fn living_population_of(&self, owner: u8) -> u32 {
        self.entities
            .iter()
            .filter(|e| e.owner == owner && !e.dead && e.kind.counts_population())
            .count() as u32
    }

    /// Current population of the human player: living population
    /// plus every production queue (a queued unit is already spoken
    /// for) — the Town Center queue and all Barracks queues.
    pub fn population(&self) -> u32 {
        self.population_of(entity::OWNER_PLAYER)
    }

    /// Current population of one player (M5): living population plus
    /// that player's Town Center queue and production-building
    /// queues.
    pub fn population_of(&self, owner: u8) -> u32 {
        let tc_queue = self
            .tc_entity_of(owner)
            .and_then(|tc| self.tc_state(tc.id))
            .map(|s| s.queued_villagers())
            .unwrap_or(0);
        let queued: u32 = self
            .buildings
            .iter()
            .filter(|b| b.owner == owner)
            .map(|b| b.queued_units())
            .sum();
        self.living_population_of(owner) + tc_queue + queued
    }

    /// Projectiles currently in flight, in spawn order (M3).
    pub fn projectiles(&self) -> &[Projectile] {
        &self.projectiles
    }

    /// Population capacity of the human player: the Town Center's
    /// room (+5) plus every completed House's (+5 each), per the
    /// mechanics study §9.4.
    pub fn population_cap(&self) -> u32 {
        self.population_cap_of(entity::OWNER_PLAYER)
    }

    /// Population capacity of one player (M5): the Town Center's
    /// base room plus that player's completed Houses.
    pub fn population_cap_of(&self, owner: u8) -> u32 {
        let mut cap = TOWN_CENTER_POP_SUPPORT;
        for b in &self.buildings {
            if b.owner == owner && b.complete {
                cap += b.kind.pop_support();
            }
        }
        cap
    }

    /// Why training cannot currently proceed at the human Town
    /// Center (for UI display). Checked in display-priority order:
    /// queue full, housed, no food.
    pub fn train_block_reason(&self) -> TrainBlock {
        self.train_block_at(TOWN_CENTER_ID)
    }

    /// Why training cannot currently proceed at Town Center `tc`
    /// (M5): the per-Town-Center form of `train_block_reason`, read
    /// against that Town Center's owner's population and stockpile.
    /// A missing or dead Town Center reports as `QueueFull` (the
    /// generic refusal).
    fn train_block_at(&self, tc: u32) -> TrainBlock {
        let (owner, queue) = match (self.entity_index(tc), self.tc_state(tc)) {
            (Some(i), Some(s)) if !self.entities[i].dead => {
                (self.entities[i].owner, s.queue.len() as u32)
            }
            _ => return TrainBlock::QueueFull,
        };
        if queue >= TRAIN_QUEUE_CAP {
            return TrainBlock::QueueFull;
        }
        if self.population_of(owner) >= self.population_cap_of(owner) {
            return TrainBlock::Housed;
        }
        if self.stockpile_of(owner).amount(ResourceKind::Food) < VILLAGER_FOOD_COST {
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

    /// Number of *living* villagers (the Town Center is not counted,
    /// and neither are the dead — M3).
    pub fn villager_count(&self) -> u32 {
        self.entities
            .iter()
            .filter(|e| e.kind == EntityKind::Villager && !e.dead)
            .count() as u32
    }

    /// Would an `Attack` command be accepted right now? A pure query
    /// mirroring `issue_attack`'s validation, for UI affordances; the
    /// command path re-validates at apply time. M5 generalizes the
    /// seats: the attacker may be any player's unit, and the target
    /// may be Gaia, another player's entity (Town Center included),
    /// or a sheep of anyone's (ECO-13).
    pub fn can_attack(&self, entity: u32, target: u32) -> bool {
        let (Some(ai), Some(ti)) = (self.entity_index(entity), self.entity_index(target))
        else {
            return false;
        };
        let a = &self.entities[ai];
        let t = &self.entities[ti];
        if a.dead || t.dead || entity == target {
            return false;
        }
        if !entity::is_player_owner(a.owner) || units::spec(a.kind).attacks.is_empty() {
            return false;
        }
        t.owner == entity::OWNER_GAIA
            || t.owner != a.owner
            || t.kind == EntityKind::Sheep
    }

    /// Why training `kind` at `building` cannot currently proceed
    /// (M3) — the Barracks counterpart of `train_block_reason`, with
    /// the same tag contract. A missing/unfinished/non-production
    /// building reports as `QueueFull` (a generic refusal: the client
    /// only asks for buildings it knows are complete Barracks).
    /// Population and funds are read against the building's *owner*
    /// (M5).
    pub fn train_at_block(&self, building: u32, kind: EntityKind) -> TrainBlock {
        let bi = match self.building_index(building) {
            Some(bi) => bi,
            None => return TrainBlock::QueueFull,
        };
        let b = &self.buildings[bi];
        if !b.complete || !b.kind.is_production() || !b.kind.trains(kind) {
            return TrainBlock::QueueFull;
        }
        if b.queue.len() as u32 >= TRAIN_QUEUE_CAP {
            return TrainBlock::QueueFull;
        }
        if self.population_of(b.owner) >= self.population_cap_of(b.owner) {
            return TrainBlock::Housed;
        }
        if !self.can_afford(b.owner, kind) {
            return TrainBlock::NoFood;
        }
        TrainBlock::Ok
    }

    /// Would a `TrainAt` command be accepted right now? Mirrors the
    /// Town Center contract: queue room and funds gate the enqueue;
    /// being housed does not (production stalls instead, and resumes
    /// when housing frees up).
    pub fn can_train_at(&self, building: u32, kind: EntityKind) -> bool {
        let bi = match self.building_index(building) {
            Some(bi) => bi,
            None => return false,
        };
        let b = &self.buildings[bi];
        b.complete
            && b.kind.is_production()
            && b.kind.trains(kind)
            && (b.queue.len() as u32) < TRAIN_QUEUE_CAP
            && self.can_afford(b.owner, kind)
    }

    /// Whether a player's stockpile covers a unit kind's full table
    /// cost (M5: per-player).
    fn can_afford(&self, owner: u8, kind: EntityKind) -> bool {
        let spec = units::spec(kind);
        let stockpile = self.stockpile_of(owner);
        stockpile.amount(ResourceKind::Food) >= I16F16::from_num(spec.food_cost)
            && stockpile.amount(ResourceKind::Wood) >= I16F16::from_num(spec.wood_cost)
            && stockpile.amount(ResourceKind::Gold) >= I16F16::from_num(spec.gold_cost)
    }

    /// Deduct a unit kind's full table cost from a player's
    /// stockpile. Callers check `can_afford` first, so the spends
    /// cannot fail partway.
    fn spend_cost(&mut self, owner: u8, kind: EntityKind) {
        let spec = units::spec(kind);
        if spec.food_cost > 0 {
            self.stockpile_mut(owner)
                .spend(ResourceKind::Food, I16F16::from_num(spec.food_cost));
        }
        if spec.wood_cost > 0 {
            self.stockpile_mut(owner)
                .spend(ResourceKind::Wood, I16F16::from_num(spec.wood_cost));
        }
        if spec.gold_cost > 0 {
            self.stockpile_mut(owner)
                .spend(ResourceKind::Gold, I16F16::from_num(spec.gold_cost));
        }
    }

    /// Would a `TrainVillager` command be accepted right now at the
    /// human Town Center? (Food covers the cost and the queue has
    /// room.) The command path re-checks at apply time; this is for
    /// UI affordances.
    pub fn can_train_villager(&self) -> bool {
        self.can_train_villager_at(TOWN_CENTER_ID)
    }

    /// Would a villager enqueue be accepted at Town Center `tc`
    /// (M5)? The per-Town-Center form of `can_train_villager`,
    /// charged to that Town Center's owner.
    fn can_train_villager_at(&self, tc: u32) -> bool {
        let (owner, queue) = match (self.entity_index(tc), self.tc_state(tc)) {
            (Some(i), Some(s)) if !self.entities[i].dead => {
                (self.entities[i].owner, s.queue.len() as u32)
            }
            _ => return false,
        };
        queue < TRAIN_QUEUE_CAP
            && self.stockpile_of(owner).amount(ResourceKind::Food) >= VILLAGER_FOOD_COST
    }

    /// Whether a player's stockpile covers a technology's full
    /// cost (M6).
    fn can_afford_tech(&self, owner: u8, tech: TechId) -> bool {
        let (food, wood, gold, stone) = tech.cost();
        let s = self.stockpile_of(owner);
        s.amount(ResourceKind::Food) >= I16F16::from_num(food)
            && s.amount(ResourceKind::Wood) >= I16F16::from_num(wood)
            && s.amount(ResourceKind::Gold) >= I16F16::from_num(gold)
            && s.amount(ResourceKind::Stone) >= I16F16::from_num(stone)
    }

    /// Deduct a technology's full cost from a player's stockpile.
    /// Callers check `can_afford_tech` first, so the spends cannot
    /// fail partway.
    fn spend_tech(&mut self, owner: u8, tech: TechId) {
        let (food, wood, gold, stone) = tech.cost();
        for (kind, amount) in [
            (ResourceKind::Food, food),
            (ResourceKind::Wood, wood),
            (ResourceKind::Gold, gold),
            (ResourceKind::Stone, stone),
        ] {
            if amount > 0 {
                self.stockpile_mut(owner)
                    .spend(kind, I16F16::from_num(amount));
            }
        }
    }

    /// The shared research gates (M6), checked against a host
    /// queue the caller resolved (the host's existence/completeness
    /// and the tech's host relation are the caller's part — a
    /// missing host reports as `QueueFull`, the generic refusal,
    /// mirroring the training pattern): already researched or
    /// already queued anywhere → `AlreadyDone`; age unmet or (for
    /// the Feudal advancement) fewer than 2 qualifying buildings →
    /// `Locked`; queue at cap → `QueueFull`; funds short →
    /// `NoFunds`. TEC-07's prerequisites, in order.
    fn check_research(&self, owner: u8, tech: TechId, queue: &[ProdEntry]) -> ResearchBlock {
        if self.has_tech(owner, tech) {
            return ResearchBlock::AlreadyDone;
        }
        // A technology may only be researched once at a time, in
        // whichever of the owner's queues already holds it.
        let queued_somewhere = self
            .tc_states
            .iter()
            .filter(|s| {
                self.entity_index(s.tc_id)
                    .map(|i| self.entities[i].owner == owner)
                    .unwrap_or(false)
            })
            .any(|s| s.queue.contains(&ProdEntry::Tech(tech)))
            || self
                .buildings
                .iter()
                .filter(|b| b.owner == owner)
                .any(|b| b.queue.contains(&ProdEntry::Tech(tech)));
        if queued_somewhere {
            return ResearchBlock::AlreadyDone;
        }
        if self.age_of(owner) < tech.age_required() {
            return ResearchBlock::Locked;
        }
        if tech == TechId::FeudalAge && self.feudal_building_count(owner) < 2 {
            return ResearchBlock::Locked;
        }
        if queue.len() as u32 >= TRAIN_QUEUE_CAP {
            return ResearchBlock::QueueFull;
        }
        if !self.can_afford_tech(owner, tech) {
            return ResearchBlock::NoFunds;
        }
        ResearchBlock::Ok
    }

    /// Why research at the human Town Center cannot currently
    /// proceed (M6) — for the client's button states.
    pub fn research_block_tc(&self, tech: TechId) -> ResearchBlock {
        self.research_block_at_tc(TOWN_CENTER_ID, tech)
    }

    /// Why research at Town Center `tc` cannot currently proceed
    /// (M6): the per-Town-Center form of `research_block_tc`.
    /// Technologies foreign to the Town Center report as
    /// `QueueFull` (the generic refusal).
    fn research_block_at_tc(&self, tc: u32, tech: TechId) -> ResearchBlock {
        if tech.host() != TechHost::TownCenter {
            return ResearchBlock::QueueFull;
        }
        let (owner, queue) = match (self.entity_index(tc), self.tc_state(tc)) {
            (Some(i), Some(s)) if !self.entities[i].dead => {
                (self.entities[i].owner, &s.queue)
            }
            _ => return ResearchBlock::QueueFull,
        };
        self.check_research(owner, tech, queue)
    }

    /// Why research at a placed building cannot currently proceed
    /// (M6): a missing building reports as `QueueFull`, an
    /// unfinished one or a tech foreign to it as `Locked`.
    pub fn research_block_at(&self, building: u32, tech: TechId) -> ResearchBlock {
        let bi = match self.building_index(building) {
            Some(bi) => bi,
            None => return ResearchBlock::QueueFull,
        };
        let b = &self.buildings[bi];
        if tech.host() != TechHost::Building(b.kind) {
            return ResearchBlock::Locked;
        }
        if !b.complete {
            return ResearchBlock::Locked;
        }
        self.check_research(b.owner, tech, &b.queue)
    }

    /// Would a Town Center research command be accepted right now?
    pub fn can_research_tc(&self, tech: TechId) -> bool {
        self.research_block_tc(tech) == ResearchBlock::Ok
    }

    /// Would a building research command be accepted right now?
    pub fn can_research_at(&self, building: u32, tech: TechId) -> bool {
        self.research_block_at(building, tech) == ResearchBlock::Ok
    }

    /// Apply a `ResearchTc` command (M6): charge and enqueue at the
    /// human Town Center, or reject silently (see
    /// `research_block_tc` for the gates).
    fn issue_research_tc(&mut self, tech: TechId) {
        self.issue_research_tc_at(TOWN_CENTER_ID, tech);
    }

    /// The shared Town Center research mechanics behind `ResearchTc`
    /// and `ResearchTcAt` (M6) — exactly how the AI opponent
    /// researches at its own Town Center (AI-01).
    fn issue_research_tc_at(&mut self, tc: u32, tech: TechId) {
        if self.research_block_at_tc(tc, tech) != ResearchBlock::Ok {
            return;
        }
        let owner = self.entities[self.entity_index(tc).expect("validated")].owner;
        self.spend_tech(owner, tech);
        if let Some(s) = self.tc_states.iter_mut().find(|s| s.tc_id == tc) {
            s.queue.push(ProdEntry::Tech(tech));
        }
    }

    /// Apply a `ResearchAt` command (M6): charge the building's
    /// owner and enqueue at the building, or reject silently.
    fn issue_research_at(&mut self, building: u32, tech: TechId) {
        if self.research_block_at(building, tech) != ResearchBlock::Ok {
            return;
        }
        let bi = self.building_index(building).expect("validated");
        let owner = self.buildings[bi].owner;
        self.spend_tech(owner, tech);
        self.buildings[bi].queue.push(ProdEntry::Tech(tech));
    }

    /// A completed research lands (M6): record it on the owner and
    /// run its completion sweep, if any. Effects that are pure data
    /// patches (Fletching, the armor lines) need no sweep — every
    /// lookup reads the owner's researched set (TEC-06). Two do:
    /// - Feudal Age flips the owner's age (TEC-04: empire-wide, at
    ///   once — lookups key on the age, so nothing else moves).
    /// - Loom grants existing villagers the +15 HP with the new
    ///   maximum (id order; the DE behavior for a stat research —
    ///   their current HP rises with the max).
    /// - Man-at-Arms swaps every living Militia of the owner to
    ///   the Man-at-Arms kind (CLS-03's line upgrade: position,
    ///   orders and current HP are kept; the max rises — no heal).
    fn apply_research(&mut self, owner: u8, tech: TechId) {
        let Some(slot) = player_slot(owner) else {
            return;
        };
        match tech {
            TechId::FeudalAge => {
                self.players[slot].age = Age::Feudal;
            }
            TechId::Loom => {
                self.players[slot].techs |= tech.bit();
                for e in &mut self.entities {
                    if e.owner == owner && e.kind == EntityKind::Villager && !e.dead {
                        e.hp += I16F16::from_num(15);
                    }
                }
            }
            TechId::ManAtArms => {
                self.players[slot].techs |= tech.bit();
                for e in &mut self.entities {
                    if e.owner == owner && e.kind == EntityKind::Militia && !e.dead {
                        e.kind = EntityKind::ManAtArms;
                    }
                }
            }
            _ => {
                self.players[slot].techs |= tech.bit();
            }
        }
    }

    /// Villagers waiting in Town Center `tc`'s queue (M6) — the
    /// AI's production accounting (research entries excluded).
    pub(crate) fn tc_queued_villagers(&self, tc: u32) -> u32 {
        self.tc_state(tc).map(|s| s.queued_villagers()).unwrap_or(0)
    }

    /// Whether a Town Center's queue is empty (M6) — the AI only
    /// starts research on an empty queue, so research never delays
    /// a villager already in line behind it (and vice versa).
    pub(crate) fn tc_queue_empty(&self, tc: u32) -> bool {
        self.tc_state(tc).map(|s| s.queue.is_empty()).unwrap_or(true)
    }

    /// Would a `ForceDeposit` command be accepted right now? (M6,
    /// ECO-07.) The villager must be alive, carrying something, and
    /// have somewhere valid to take it.
    pub fn can_force_deposit(&self, entity: u32) -> bool {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return false,
        };
        let e = &self.entities[idx];
        if e.kind != EntityKind::Villager || e.dead || e.carry <= I16F16::ZERO {
            return false;
        }
        match e.carry_kind {
            Some(kind) => self
                .deposit_target(e.owner, kind, e.pos_x, e.pos_y)
                .is_some(),
            None => false,
        }
    }

    /// Apply a `ForceDeposit` command (M6, ECO-07): the villager's
    /// phase flips to `Returning` toward the nearest valid drop-off
    /// for its load, *keeping* its gather assignment (or its
    /// construction site) as the resume marker — the deposit step
    /// sends it back to that work on its own. Like every non-queued
    /// order it clears the villager's order queue and any combat
    /// engagement; the carry is untouched until the deposit lands.
    fn issue_force_deposit(&mut self, entity: u32) {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return,
        };
        let (owner, kind, px, py) = {
            let e = &self.entities[idx];
            if e.kind != EntityKind::Villager || e.dead || e.carry <= I16F16::ZERO {
                return;
            }
            match e.carry_kind {
                Some(k) => (e.owner, k, e.pos_x, e.pos_y),
                None => return,
            }
        };
        let Some(target) = self.deposit_target(owner, kind, px, py) else {
            return;
        };
        {
            let e = &mut self.entities[idx];
            e.order_queue.clear();
            e.clear_combat();
            e.task = Task::Returning;
            e.task_dropoff = Some(target);
        }
        // If the drop-off cannot be steered to at all, the
        // Returning step resolves the stop itself next tick.
        self.steer_to_dropoff(idx, owner, target);
    }

    /// Would an `AttackBuilding` command be accepted right now?
    /// (M5.) Mirrors `can_attack` for placed buildings: the attacker
    /// must be a living player unit with an attack row, and the
    /// building must belong to *another* player.
    pub fn can_attack_building(&self, entity: u32, building: u32) -> bool {
        let (Some(ai), Some(bi)) = (self.entity_index(entity), self.building_index(building))
        else {
            return false;
        };
        let a = &self.entities[ai];
        if a.dead {
            return false;
        }
        if !entity::is_player_owner(a.owner) || units::spec(a.kind).attacks.is_empty() {
            return false;
        }
        self.buildings[bi].owner != a.owner
            && entity::is_player_owner(self.buildings[bi].owner)
    }

    /// Would a `Queue` command be accepted right now? (M4, SEL-12.)
    /// Light validation only — the order re-validates fully when it
    /// *executes*, because the world can change while it waits: a
    /// target can die, a node can deplete, a build spot can fill.
    /// Here we check the entity can take orders at all, the
    /// order-specific shape (Gather/Build are villager work, Attack
    /// must be legal right now), and the queue cap.
    pub fn can_queue(&self, entity: u32, order: QueuedOrder) -> bool {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return false,
        };
        if !self.orderable(idx) || self.entities[idx].order_queue.len() >= ORDER_QUEUE_CAP {
            return false;
        }
        match order {
            QueuedOrder::Move { .. } => true,
            QueuedOrder::Gather { node } => {
                self.entities[idx].kind == EntityKind::Villager
                    && self.node_index(node).is_some()
            }
            QueuedOrder::Attack { target } => self.can_attack(entity, target),
            QueuedOrder::Build { .. } => self.entities[idx].kind == EntityKind::Villager,
        }
    }

    /// Would a `SetRally` command be accepted right now? (M4,
    /// SEL-13.) The Town Center always holds a rally point; placed
    /// buildings only if they are production buildings (in M4: the
    /// Barracks). Setting a rally on a building still under
    /// construction is allowed, as in the reference game.
    pub fn can_set_rally(&self, building: Option<u32>) -> bool {
        match building {
            None => true,
            Some(bid) => self
                .building_index(bid)
                .map(|bi| self.buildings[bi].kind.is_production())
                .unwrap_or(false),
        }
    }

    /// The human Town Center's rally point as `(x, y, snapped_node)`,
    /// or `None` when unset (M4, SEL-13). For the client's flag render.
    pub fn tc_rally(&self) -> Option<(I16F16, I16F16, Option<u32>)> {
        let s = self.tc_state(TOWN_CENTER_ID)?;
        s.rally.map(|(x, y)| (x, y, s.rally_node))
    }

    /// A placed building's rally point, same shape as `tc_rally`.
    /// `None` when the building is unknown or its rally is unset.
    pub fn building_rally(&self, building: u32) -> Option<(I16F16, I16F16, Option<u32>)> {
        let bi = self.building_index(building)?;
        let b = &self.buildings[bi];
        b.rally.map(|(x, y)| (x, y, b.rally_node))
    }

    /// Idle villagers (M4, SEL-08): living human villagers with no
    /// work task, no move order, and no attack target — the
    /// definition the rulebook gives ("no work task and no move
    /// order"). Id order. Behind the client's idle-villager key and
    /// indicator (INF-03). Human-only: the AI reads its own idle
    /// state directly, and the client's indicators are the human's.
    pub fn idle_villagers(&self) -> Vec<u32> {
        self.entities
            .iter()
            .filter(|e| {
                e.kind == EntityKind::Villager
                    && e.owner == entity::OWNER_PLAYER
                    && !e.dead
                    && e.task == Task::Idle
                    && e.order.is_none()
                    && e.attack_target.is_none()
                    && e.attack_building.is_none()
            })
            .map(|e| e.id)
            .collect()
    }

    /// Idle military (M4, SEL-09): living human military units
    /// (Scout, Militia, Archer) with no move order and no attack
    /// target. Id order. The mirror of `idle_villagers`.
    pub fn idle_military(&self) -> Vec<u32> {
        self.entities
            .iter()
            .filter(|e| {
                e.kind.is_military()
                    && e.owner == entity::OWNER_PLAYER
                    && !e.dead
                    && e.order.is_none()
                    && e.attack_target.is_none()
                    && e.attack_building.is_none()
            })
            .map(|e| e.id)
            .collect()
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
                Command::TrainVillagerAt { tc } => self.issue_train_villager_at(tc),
                Command::Build { entity, kind, x, y } => self.issue_build(entity, kind, x, y),
                Command::Attack { entity, target } => self.issue_attack(entity, target),
                Command::AttackBuilding { entity, building } => {
                    self.issue_attack_building(entity, building)
                }
                Command::TrainAt { building, kind } => self.issue_train_at(building, kind),
                Command::Queue { entity, order } => self.issue_queue(entity, order),
                Command::SetRally { building, x, y } => self.issue_set_rally(building, x, y),
                Command::ResearchTc { tech } => self.issue_research_tc(tech),
                Command::ResearchTcAt { tc, tech } => self.issue_research_tc_at(tc, tech),
                Command::ResearchAt { building, tech } => self.issue_research_at(building, tech),
                Command::ForceDeposit { entity } => self.issue_force_deposit(entity),
            }
        }

        // 2. Movement. Steering is decided from an immutable snapshot,
        //    then integrated — two phases keep borrows simple and the
        //    update order deterministic. The snapshot holds every
        //    *mobile, living* entity (M3: military units and animals
        //    steer too), each with its table speed and collision
        //    radius. The Town Center is excluded: buildings neither
        //    push nor get pushed, and a hauler must be able to reach
        //    the Town Center's exact position to complete its drop-off
        //    order. Dead entities neither move nor push.
        let snapshot: Vec<steer::SnapshotEntry> = self
            .entities
            .iter()
            .filter(|e| e.kind.is_mobile() && !e.dead)
            .map(|e| (e.pos_x, e.pos_y, units::spec(e.kind).radius))
            .collect();
        // Entity index -> index into the snapshot.
        let mut snap_idx = vec![usize::MAX; self.entities.len()];
        {
            let mut k = 0;
            for (i, e) in self.entities.iter().enumerate() {
                if e.kind.is_mobile() && !e.dead {
                    snap_idx[i] = k;
                    k += 1;
                }
            }
        }
        let mut plans: Vec<(I16F16, I16F16, usize, bool)> = Vec::with_capacity(self.entities.len());
        for (i, e) in self.entities.iter().enumerate() {
            let speed = self.current_speed(i);
            plans.push(match &e.order {
                Some(o) if snap_idx[i] != usize::MAX => steer::steer(
                    &snapshot,
                    snap_idx[i],
                    &o.waypoints,
                    o.wp_index,
                    speed,
                    !e.suppress_separation,
                ),
                // No order: the unit stands still *except* for the
                // separation shove — a stationary unit must never be an
                // immovable wall on another's path (head-on pushes could
                // otherwise cancel a traveler's velocity exactly, a
                // deadlock found in M2 testing). A melee unit engaged
                // in an attack is exempt (MOV-04): it holds its ground.
                None if snap_idx[i] != usize::MAX && !e.suppress_separation => {
                    let (vx, vy) = steer::shove(&snapshot, snap_idx[i], speed);
                    (vx, vy, 0, false)
                }
                _ => (I16F16::ZERO, I16F16::ZERO, 0, false),
            });
        }
        let grid = &self.grid;
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
                // M7: axis-separated slide against TREE cells. The
                // A* corner rule lets a diagonal leg pass between
                // two trunks (the only way into a felled tree's
                // freed cell, MAP-04), but a continuous unit
                // steering between waypoints cuts the corner by a
                // hair and its center clips the flanking tree
                // cell — the integrity pass then teleported it
                // back, the task layer re-issued the same order,
                // and the woodcutter looped forever at the mouth
                // of the gap (the seed-109 stall). Sliding along
                // the blocked axis instead — the standard grid
                // mover — rounds the corner into the standing
                // cell. Only the tree layer slides: buildings and
                // water keep the M6 integrity teleport below.
                let (ox, oy) = (e.pos_x, e.pos_y);
                let (nx, ny) = (clamp_pos(ox + vx), clamp_pos(oy + vy));
                let in_tree = |x: I16F16, y: I16F16| {
                    let (cx, cy) = grid.world_to_cell(x, y);
                    grid.is_node_blocked(cx, cy)
                };
                let (fx, fy) = if !in_tree(nx, ny) {
                    (nx, ny)
                } else if !in_tree(nx, oy) {
                    (nx, oy)
                } else if !in_tree(ox, ny) {
                    (ox, ny)
                } else {
                    (ox, oy)
                };
                e.pos_x = fx;
                e.pos_y = fy;
            }
        }

        // 2.5 Grid integrity (M6): separation can push a unit into
        //    a blocked cell (a wall cell, a building footprint) —
        //    and a unit left standing there can never be pathed to
        //    again, stranding it beyond any attacker (a Conquest
        //    match once hinged on a Scout entombed in the wall
        //    column). No living unit ends a tick inside a blocked
        //    cell: it is displaced to the nearest open cell.
        for idx in 0..self.entities.len() {
            if self.entities[idx].dead {
                continue;
            }
            let cell = self
                .grid
                .world_to_cell(self.entities[idx].pos_x, self.entities[idx].pos_y);
            if self.grid.is_blocked(cell.0, cell.1) {
                self.displace_out_of_blocked(idx);
            }
        }

        // 3. Combat (M3, SIM-07: after movement, before economy):
        //    animal behavior, attack machines, projectiles, deaths,
        //    carcass decay lives in the economy phase below.
        self.combat_phase();

        // 4. Economy: carcass decay, training (Town Center + Barracks),
        //    then villager work cycles and construction.
        self.economy_tick();

        // 5. Order queues (M4, SEL-12): an entity that became fully
        //    idle this tick starts its next queued order, so a chain
        //    has no dead tick between links.
        self.advance_queues();

        // 6. The scripted opponent decides (M5): on its cadence the
        //    AI reads live state and enqueues ordinary commands into
        //    the same FIFO the human uses (AI-01) — they apply at the
        //    next tick's step 1 like any command. No state is touched
        //    directly, so a match is still just (seed, mode, the
        //    human's commands) and replays bit-identically.
        if self.mode.is_match() && self.tick % ai::config(self.mode).cadence == 0 {
            ai::decide(self);
        }

        // 7. Fog of war (M7): the visibility grids recompute on
        //    their fixed interval, after every system that moves
        //    things — the AI above decided on the previous
        //    interval's knowledge, like a player reacting to what
        //    they saw a moment ago.
        if self.tick % FOG_RECOMPUTE_INTERVAL == 0 {
            self.recompute_visibility();
        }

        self.tick += 1;
        self.state_hash()
    }

    /// The speed steering uses for an entity this tick: its table
    /// speed, except a boar with an attack target, which uses its
    /// faster chase speed.
    fn current_speed(&self, idx: usize) -> I16F16 {
        let e = &self.entities[idx];
        if e.kind == EntityKind::Boar && e.attack_target.is_some() {
            return units::BOAR_CHASE_SPEED;
        }
        // M6: the owner's effective row — Loom's villager speed and
        // the Scout's Feudal speed enter movement here (TEC-06).
        self.unit_stats(idx).speed
    }

    /// Can this entity take player orders at all? Living, owned by a
    /// *player* (human or opponent — M5), mobile. Shared by the
    /// `Move` path and the queue validation. Gaia's animals take no
    /// orders; herdables are driven by capture logic instead.
    fn orderable(&self, idx: usize) -> bool {
        let e = &self.entities[idx];
        !e.dead && e.owner != entity::OWNER_GAIA && e.kind.is_mobile()
    }

    /// Apply a `Move` command: clamp the target into the world, recompute
    /// the A* path immediately (deterministically), and install the order.
    /// Unknown entities and unreachable/blocked targets clear the order —
    /// the entity simply holds position. For a villager, `Move` cancels
    /// its gather task; the carry is kept (counted only on a later
    /// deposit — e.g. after a fresh `Gather` order). A `Move` also
    /// cancels any attack engagement. Orderable entities (M3): living
    /// player units — villagers, military, the Scout, and captured
    /// sheep. The Town Center, the dead, and Gaia's wild animals
    /// ignore the command. A non-queued `Move` also clears the
    /// entity's order queue (M4, SEL-12: replace semantics).
    fn issue_move(&mut self, entity: u32, x: I16F16, y: I16F16) {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return,
        };
        if !self.orderable(idx) {
            return;
        }
        self.entities[idx].order_queue.clear();
        self.apply_move(idx, x, y);
    }

    /// The mechanics of a move order, shared by the `Move` command
    /// and by queued-order execution (which must not clear the rest
    /// of the queue behind it).
    fn apply_move(&mut self, idx: usize, x: I16F16, y: I16F16) {
        {
            let e = &mut self.entities[idx];
            e.task = Task::Idle;
            e.task_node = None;
            e.task_building = None;
            e.task_dropoff = None;
            e.clear_combat();
        }
        self.set_move_order(idx, clamp_pos(x), clamp_pos(y));
    }

    /// Apply an `Attack` command (M3): validate (same rules as
    /// `can_attack`), cancel the attacker's current work/move, and set
    /// its attack target. The combat phase handles the chase, the
    /// strikes, and the kill. A non-queued `Attack` also clears the
    /// entity's order queue (M4, SEL-12: replace semantics).
    fn issue_attack(&mut self, entity: u32, target: u32) {
        if !self.can_attack(entity, target) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        self.entities[idx].order_queue.clear();
        self.apply_attack(idx, target);
    }

    /// The mechanics of an attack order, shared by the `Attack`
    /// command and by queued-order execution.
    fn apply_attack(&mut self, idx: usize, target: u32) {
        let e = &mut self.entities[idx];
        e.task = Task::Idle;
        e.task_node = None;
        e.task_building = None;
        e.task_dropoff = None;
        e.order = None;
        e.attack_target = Some(target);
        e.attack_building = None;
        e.strike_tick = None;
        e.attack_engaged = false;
        e.next_attack_tick = 0;
    }

    /// Apply an `AttackBuilding` command (M5): the building
    /// counterpart of `issue_attack` — validate (same rules as
    /// `can_attack_building`), cancel the attacker's current work,
    /// and set its building target. The combat phase's building step
    /// chases the footprint and strikes it. A non-queued order also
    /// clears the entity's order queue (SEL-12 replace semantics).
    fn issue_attack_building(&mut self, entity: u32, building: u32) {
        if !self.can_attack_building(entity, building) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        self.entities[idx].order_queue.clear();
        let e = &mut self.entities[idx];
        e.task = Task::Idle;
        e.task_node = None;
        e.task_building = None;
        e.task_dropoff = None;
        e.order = None;
        e.attack_target = None;
        e.attack_building = Some(building);
        e.strike_tick = None;
        e.attack_engaged = false;
        e.next_attack_tick = 0;
    }

    /// Apply a `TrainAt` command (M3): deduct the unit's table cost
    /// and enqueue it at the production building, or reject silently
    /// (see `can_train_at` for the gates).
    fn issue_train_at(&mut self, building: u32, kind: EntityKind) {
        if !self.can_train_at(building, kind) {
            return;
        }
        let bi = self.building_index(building).expect("validated");
        let owner = self.buildings[bi].owner;
        self.spend_cost(owner, kind);
        self.buildings[bi].queue.push(ProdEntry::Unit(kind));
    }

    /// Apply a `Gather` command: assign the villager to the node. If it
    /// carries a *different* resource, it first hauls that home (the
    /// assignment is kept, so work resumes at the new node after the
    /// deposit); otherwise it heads straight for the node. A
    /// non-queued `Gather` also clears the villager's order queue
    /// (M4, SEL-12: replace semantics).
    fn issue_gather(&mut self, entity: u32, node: u32) {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return,
        };
        if self.entities[idx].kind != EntityKind::Villager || self.entities[idx].dead {
            return;
        }
        match self.node_index(node) {
            Some(ni) if !self.nodes[ni].is_depleted() => {}
            _ => return,
        }
        self.entities[idx].order_queue.clear();
        self.begin_gather(idx, node);
    }

    /// Assign villager `idx` to work node `node` (which must be live).
    /// Shared by the `Gather` command and by farm completion, where the
    /// builder starts working the field it just raised.
    fn begin_gather(&mut self, idx: usize, node: u32) {
        let resource = match self.node_index(node) {
            Some(ni) => self.nodes[ni].resource(),
            None => return,
        };
        self.entities[idx].task_node = Some(node);
        self.entities[idx].task_building = None;
        match self.entities[idx].carry_kind {
            Some(k) if k != resource && self.entities[idx].carry > I16F16::ZERO => {
                // Haul the other resource home first — to the
                // nearest valid drop-off for *that* load (M6,
                // ECO-06; before M6, always the Town Center). With
                // nowhere valid to haul to, head for the node and
                // keep the carry (it tops up what it can and idles
                // full — see `begin_return`).
                let (owner, px, py) = {
                    let e = &self.entities[idx];
                    (e.owner, e.pos_x, e.pos_y)
                };
                match self.deposit_target(owner, k, px, py) {
                    Some(target) => {
                        self.entities[idx].task = Task::Returning;
                        self.entities[idx].task_dropoff = Some(target);
                        if !self.steer_to_dropoff(idx, owner, target) {
                            // No reachable drop-off: head for the
                            // node and keep the carry (documented
                            // above).
                            self.entities[idx].task = Task::ToNode;
                            self.entities[idx].task_dropoff = None;
                            self.move_order_to_node(idx, node);
                        }
                    }
                    None => {
                        self.entities[idx].task = Task::ToNode;
                        self.move_order_to_node(idx, node);
                    }
                }
            }
            _ => {
                self.entities[idx].task = Task::ToNode;
                self.move_order_to_node(idx, node);
            }
        }
    }

    /// Install a move order taking entity `idx` to work node
    /// `node_id` (M7). Ordinary nodes are pathed to directly, as
    /// always. A TREE node's own cell is blocked by the tree
    /// (MAP-04), so the order targets the best standing cell beside
    /// it instead: the reachable ring cell with the shortest path
    /// (ties break in fixed ring order — the search is bounded at
    /// the tree's 8 neighbors, once per order, never per tick).
    /// Returns whether an order was installed; on failure the
    /// entity's order is cleared, exactly like `set_move_order`.
    fn move_order_to_node(&mut self, idx: usize, node_id: u32) -> bool {
        let (nx, ny, kind) = match self.node_index(node_id) {
            Some(ni) => {
                let n = &self.nodes[ni];
                (n.pos_x, n.pos_y, n.kind)
            }
            None => return false,
        };
        if kind != NodeKind::Tree {
            return self.set_move_order(idx, nx, ny);
        }
        let (tx, ty) = self.grid.world_to_cell(nx, ny);
        let from = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let mut best: Option<(usize, (I16F16, I16F16), Vec<(I16F16, I16F16)>)> = None;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (cx, cy) = (tx as i32 + dx, ty as i32 + dy);
                if cx < 0 || cy < 0 || cx >= GRID_DIM as i32 || cy >= GRID_DIM as i32 {
                    continue;
                }
                let (cx, cy) = (cx as u32, cy as u32);
                if self.grid.is_blocked(cx, cy) {
                    continue;
                }
                let center = self.grid.cell_center(cx, cy);
                if let Some(wps) = self.grid.find_path(from, center) {
                    let better = match &best {
                        None => true,
                        Some((len, _, _)) => wps.len() < *len,
                    };
                    if better {
                        best = Some((wps.len(), center, wps));
                    }
                }
            }
        }
        match best {
            Some((_, target, wps)) => {
                let e = &mut self.entities[idx];
                e.order = Some(MoveOrder {
                    target_x: target.0,
                    target_y: target.1,
                    waypoints: wps,
                    wp_index: 0,
                });
                true
            }
            None => {
                let e = &mut self.entities[idx];
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
                false
            }
        }
    }

    /// Apply a `TrainVillager` command: deduct the food cost and enqueue
    /// at the human Town Center, or reject silently (insufficient food /
    /// full queue).
    fn issue_train_villager(&mut self) {
        self.issue_train_villager_at(TOWN_CENTER_ID);
    }

    /// The shared villager-enqueue mechanics behind `TrainVillager`
    /// and `TrainVillagerAt` (M5): the cost is charged to the Town
    /// Center's owner and the queue is that Town Center's own — this
    /// is exactly how the AI opponent trains its villagers (AI-01:
    /// ordinary commands, no side channels).
    fn issue_train_villager_at(&mut self, tc: u32) {
        if !self.can_train_villager_at(tc) {
            return;
        }
        let owner = self.entities[self.entity_index(tc).expect("validated")].owner;
        if !self
            .stockpile_mut(owner)
            .spend(ResourceKind::Food, VILLAGER_FOOD_COST)
        {
            return;
        }
        if let Some(s) = self.tc_states.iter_mut().find(|s| s.tc_id == tc) {
            s.queue.push(ProdEntry::Unit(EntityKind::Villager));
        }
    }

    /// Apply a `Build` command: validate in the sim, deduct the wood at
    /// placement, register the building (blocking its footprint cells
    /// when the kind is solid), and send the villager to the site. On
    /// any validation failure the order is rejected whole — no partial
    /// spends, no phantom buildings. A non-queued `Build` that takes
    /// effect also clears the villager's order queue (M4, SEL-12:
    /// replace semantics); a rejected one changes nothing at all,
    /// queue included — the pre-M4 contract.
    fn issue_build(&mut self, entity: u32, kind: BuildingKind, x: I16F16, y: I16F16) {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return,
        };
        if self.apply_build(entity, kind, x, y) {
            self.entities[idx].order_queue.clear();
        }
    }

    /// The mechanics of a build order, shared by the `Build` command
    /// and by queued-order execution (which must not clear the rest
    /// of the queue behind it). Returns whether the order took
    /// effect (builder assigned: new placement or a resume).
    fn apply_build(&mut self, entity: u32, kind: BuildingKind, x: I16F16, y: I16F16) -> bool {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return false,
        };
        match self.validate_build(entity, kind, clamp_pos(x), clamp_pos(y)) {
            BuildVerdict::Reject => false,
            BuildVerdict::Resume(bid) => {
                self.assign_builder(idx, bid);
                true
            }
            BuildVerdict::Place(cell_x, cell_y) => {
                // Validation proved the builder's owner can pay; the
                // spend re-checks anyway so a failure can never place
                // a free building.
                let builder_owner = self.entities[idx].owner;
                if !self
                    .stockpile_mut(builder_owner)
                    .spend(ResourceKind::Wood, kind.wood_cost())
                {
                    return false;
                }
                let id = self.next_building_id;
                self.next_building_id += 1;
                let (pos_x, pos_y) =
                    build::footprint_center(cell_x, cell_y, kind.footprint_size());
                let building = Building {
                    id,
                    kind,
                    owner: builder_owner,
                    hp: I16F16::from_num(kind.max_hp()),
                    cell_x,
                    cell_y,
                    pos_x,
                    pos_y,
                    progress_thirds: 0,
                    complete: false,
                    node_id: None,
                    queue: Vec::new(),
                    train_progress: 0,
                    rally: None,
                    rally_node: None,
                };
                if kind.blocks_footprint() {
                    for (cx, cy) in building.footprint_cells() {
                        self.grid.set_dynamic(cx, cy, true);
                    }
                }
                self.buildings.push(building);
                self.assign_builder(idx, id);
                true
            }
        }
    }

    /// Apply a `Queue` command (M4, SEL-12): append the order to the
    /// entity's queue. If the entity is idle the queue-advance step
    /// later this same tick starts it — a queued order for an idle
    /// unit behaves like a plain order with no visible delay.
    fn issue_queue(&mut self, entity: u32, order: QueuedOrder) {
        if !self.can_queue(entity, order) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        self.entities[idx].order_queue.push(order);
    }

    /// Start the next queued order of an idle entity (M4, SEL-12).
    /// Runs once per tick, after the economy phase: any entity that
    /// is fully idle — no work task, no move order, no attack target —
    /// pops its queue head and executes it exactly like the
    /// corresponding base command. An order that no longer validates
    /// at execution time (its target died, its node depleted, its
    /// build spot or budget vanished) is dropped and the next one is
    /// tried, so a stale link never wedges the chain.
    fn advance_queues(&mut self) {
        for idx in 0..self.entities.len() {
            {
                let e = &self.entities[idx];
                if e.dead
                    || e.order_queue.is_empty()
                    || e.task != Task::Idle
                    || e.order.is_some()
                    || e.attack_target.is_some()
                    || e.attack_building.is_some()
                {
                    continue;
                }
            }
            while !self.entities[idx].order_queue.is_empty() {
                let order = self.entities[idx].order_queue.remove(0);
                if self.execute_queued(idx, order) {
                    break;
                }
            }
        }
    }

    /// Execute one queued order for entity `idx` — the apply-half of
    /// the corresponding base command, without the replace
    /// semantics. Returns whether the order took effect.
    fn execute_queued(&mut self, idx: usize, order: QueuedOrder) -> bool {
        let entity = self.entities[idx].id;
        match order {
            QueuedOrder::Move { x, y } => {
                if !self.orderable(idx) {
                    return false;
                }
                self.apply_move(idx, x, y);
                true
            }
            QueuedOrder::Gather { node } => {
                if self.entities[idx].kind != EntityKind::Villager || self.entities[idx].dead {
                    return false;
                }
                match self.node_index(node) {
                    Some(ni) if !self.nodes[ni].is_depleted() => {}
                    _ => return false,
                }
                self.begin_gather(idx, node);
                true
            }
            QueuedOrder::Attack { target } => {
                if !self.can_attack(entity, target) {
                    return false;
                }
                self.apply_attack(idx, target);
                true
            }
            QueuedOrder::Build { kind, x, y } => self.apply_build(entity, kind, x, y),
        }
    }

    /// Apply a `SetRally` command (M4, SEL-13): store the point (and
    /// the node it snaps to, if any) on the Town Center or the
    /// production building. Takes effect on the next spawn there.
    fn issue_set_rally(&mut self, building: Option<u32>, x: I16F16, y: I16F16) {
        if !self.can_set_rally(building) {
            return;
        }
        let (x, y) = (clamp_pos(x), clamp_pos(y));
        let node = self.rally_snap_node(x, y);
        match building {
            None => {
                if let Some(s) = self
                    .tc_states
                    .iter_mut()
                    .find(|s| s.tc_id == TOWN_CENTER_ID)
                {
                    s.rally = Some((x, y));
                    s.rally_node = node;
                }
            }
            Some(bid) => {
                let bi = self.building_index(bid).expect("validated");
                self.buildings[bi].rally = Some((x, y));
                self.buildings[bi].rally_node = node;
            }
        }
    }

    /// The live node a rally point at `(x, y)` snaps to: the nearest
    /// non-depleted node within `RALLY_NODE_SNAP_RADIUS`, ties broken
    /// by lowest id (nodes are scanned in id order and only a strictly
    /// nearer node replaces the current best — the same discipline
    /// as combat's target acquisition).
    fn rally_snap_node(&self, x: I16F16, y: I16F16) -> Option<u32> {
        let mut best: Option<(i64, u32)> = None; // (distance² bits, id)
        for n in &self.nodes {
            if n.is_depleted() {
                continue;
            }
            let d = steer::vec_len(n.pos_x - x, n.pos_y - y);
            if d > econ::RALLY_NODE_SNAP_RADIUS {
                continue;
            }
            let db = d.to_bits() as i64;
            let key = (db * db, n.id);
            if best.map(|b| key < b).unwrap_or(true) {
                best = Some(key);
            }
        }
        best.map(|(_, id)| id)
    }

    /// Send a freshly trained unit to its building's rally point
    /// (M4, SEL-13): a villager whose rally snapped to a still-live
    /// node goes straight to work on it; anyone else walks to the
    /// rally point. No rally set → the unit stays at its spawn point,
    /// just outside the building (the reference game's default).
    fn apply_spawn_rally(
        &mut self,
        idx: usize,
        rally: Option<(I16F16, I16F16)>,
        rally_node: Option<u32>,
    ) {
        let (rx, ry) = match rally {
            Some(p) => p,
            None => return,
        };
        if self.entities[idx].kind == EntityKind::Villager {
            if let Some(nid) = rally_node {
                if self.live_node_pos(nid).is_some() {
                    self.begin_gather(idx, nid);
                    return;
                }
            }
        }
        self.set_move_order(idx, rx, ry);
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
        if self.entities[idx].kind != EntityKind::Villager || self.entities[idx].dead {
            return BuildVerdict::Reject;
        }
        let size = kind.footprint_size();
        let (ax, ay) = self.grid.world_to_cell(x, y);
        // Resume before placement checks: the footprint of a standing
        // incomplete building is *supposed* to be occupied — by
        // itself. Only the owner's own villagers resume it (M5: a
        // building under construction belongs to its placer).
        for b in &self.buildings {
            if !b.complete
                && b.kind == kind
                && b.owner == self.entities[idx].owner
                && b.cell_x == ax
                && b.cell_y == ay
            {
                return BuildVerdict::Resume(b.id);
            }
        }
        // M6 placement gates (BLD-15: availability keys on the
        // owner's age and built set) — checked for fresh placements
        // only; resuming a standing foundation is always allowed.
        let builder_owner = self.entities[idx].owner;
        if kind == BuildingKind::Farm && !self.has_completed_building(builder_owner, BuildingKind::Mill) {
            // Farms require a standing Mill (OPENING-SPEC §2.4).
            return BuildVerdict::Reject;
        }
        if self.age_of(builder_owner) < kind.age_required() {
            return BuildVerdict::Reject;
        }
        if let Some(req) = kind.requires_building() {
            if !self.has_completed_building(builder_owner, req) {
                return BuildVerdict::Reject;
            }
        }
        // The whole footprint must be inside the grid.
        if ax + size > GRID_DIM || ay + size > GRID_DIM {
            return BuildVerdict::Reject;
        }
        let cells = build::footprint_cells(ax, ay, size);
        for &(cx, cy) in &cells {
            // Walls and already-blocking buildings.
            if self.grid.is_blocked(cx, cy) {
                return BuildVerdict::Reject;
            }
            // Any Town Center's cell (M5: both players' — the TC is
            // an entity, so its cell is wherever it stands).
            for e in &self.entities {
                if e.kind == EntityKind::TownCenter
                    && !e.dead
                    && self.grid.world_to_cell(e.pos_x, e.pos_y) == (cx, cy)
                {
                    return BuildVerdict::Reject;
                }
            }
            // A living unit standing on the footprint blocks
            // placement (any owner, Gaia included — the reference
            // game's rule). It is also what keeps a completed
            // building from entombing a unit in a cell no attacker
            // can path to, which used to make Conquest unwinnable
            // when the entombed unit was the loser's last (M6 fix).
            // Farms are exempt: they never block, and their farmer
            // stands on them by design. The ordering builder is
            // exempt: it steps to the ring to work.
            if kind != BuildingKind::Farm {
                for e in &self.entities {
                    if e.id != entity
                        && !e.dead
                        && self.grid.world_to_cell(e.pos_x, e.pos_y) == (cx, cy)
                    {
                        return BuildVerdict::Reject;
                    }
                }
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
        // Two square footprints overlap iff their anchor ranges
        // intersect on both axes (each spans `size` cells).
        for b in &self.buildings {
            let bs = b.kind.footprint_size();
            if ax < b.cell_x + bs && b.cell_x < ax + size && ay < b.cell_y + bs && b.cell_y < ay + size {
                return BuildVerdict::Reject;
            }
        }
        // The wood must be payable at placement — by the *builder's*
        // owner (M5: each player builds from their own stockpile).
        if self.stockpile_of(builder_owner).amount(ResourceKind::Wood) < kind.wood_cost() {
            return BuildVerdict::Reject;
        }
        // Reachable-ish: at least one ring cell stands open and the
        // builder has a path to it.
        if self.approach_point(idx, ax, ay, size).is_none() {
            return BuildVerdict::Reject;
        }
        BuildVerdict::Place(ax, ay)
    }

    /// Put villager `idx` on construction duty for building `bid`:
    /// travel phase, with a move order to the best standing spot.
    fn assign_builder(&mut self, idx: usize, bid: u32) {
        let (ax, ay, size) = match self.building_index(bid) {
            Some(bi) => {
                let b = &self.buildings[bi];
                (b.cell_x, b.cell_y, b.kind.footprint_size())
            }
            None => return,
        };
        {
            let e = &mut self.entities[idx];
            e.task = Task::ToBuild;
            e.task_building = Some(bid);
            e.task_node = None;
        }
        match self.approach_point(idx, ax, ay, size) {
            Some((px, py)) => {
                self.set_move_order(idx, px, py);
            }
            None => {
                // No standing spot right now (crowded site): the
                // work step probes again — and gives up to Idle if
                // the site stays unreachable (M6 perf fix).
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
    fn approach_point(
        &self,
        idx: usize,
        ax: u32,
        ay: u32,
        size: u32,
    ) -> Option<(I16F16, I16F16)> {
        let from = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        for (cx, cy) in build::ring_cells(ax, ay, size) {
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

    /// Nearest cell to `from` that is not blocked (either layer),
    /// scanning Chebyshev rings in fixed (ring, row, column)
    /// order; `from` itself is never returned — callers use this
    /// to displace a unit out of a blocked cell. Bounded; `None`
    /// if the whole neighborhood is solid.
    fn nearest_open_cell(&self, from: (u32, u32)) -> Option<(u32, u32)> {
        for r in 1..=8i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (x, y) = (from.0 as i32 + dx, from.1 as i32 + dy);
                    if x < 0 || y < 0 || x >= GRID_DIM as i32 || y >= GRID_DIM as i32 {
                        continue;
                    }
                    let (x, y) = (x as u32, y as u32);
                    if !self.grid.is_blocked(x, y) {
                        return Some((x, y));
                    }
                }
            }
        }
        None
    }

    /// Move entity `idx` out of the blocked cell it stands in, to
    /// the center of the nearest open cell, clearing its move
    /// order (M6 grid integrity — see the per-tick pass in
    /// `Sim::tick` and the completion displacement in
    /// `complete_building`). No-op when no open cell is near.
    fn displace_out_of_blocked(&mut self, idx: usize) {
        let cell = self
            .grid
            .world_to_cell(self.entities[idx].pos_x, self.entities[idx].pos_y);
        if let Some((nx, ny)) = self.nearest_open_cell(cell) {
            let (wx, wy) = self.grid.cell_center(nx, ny);
            let e = &mut self.entities[idx];
            e.pos_x = wx;
            e.pos_y = wy;
            e.vel_x = I16F16::ZERO;
            e.vel_y = I16F16::ZERO;
            e.order = None;
        }
    }

    fn building_index(&self, id: u32) -> Option<usize> {
        self.buildings.iter().position(|b| b.id == id)
    }

    /// Install a freshly computed A* move order toward a world point.
    /// On failure (blocked/unreachable) the order is cleared and the
    /// entity holds position. Returns whether an order was
    /// installed: per-tick callers MUST react to `false` with a
    /// terminal transition (idle / order complete) — retrying next
    /// tick re-runs a full-grid A* that will keep failing until the
    /// building set changes, which froze late-game matches (M6
    /// perf fix; the building set is the only thing that changes
    /// reachability, and completion/destruction re-tasks everyone
    /// affected through other paths).
    fn set_move_order(&mut self, idx: usize, x: I16F16, y: I16F16) -> bool {
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
        let installed = order.is_some();
        let e = &mut self.entities[idx];
        match order {
            Some(o) => e.order = Some(o),
            None => {
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
        }
        installed
    }

    /// The economy phase of a tick: carcass decay (M3), training
    /// progress at the Town Center and the production buildings, then
    /// every living villager's work state machine, in id order, then
    /// construction progress (M2.5).
    fn economy_tick(&mut self) {
        // Carcass decay (ECO-12): carcasses — and only carcasses —
        // lose food continuously from the moment of death, whether or
        // not anyone is working them. Applied before work, so a
        // gatherer takes from the post-decay amount.
        for n in &mut self.nodes {
            if n.decay_per_tick > I16F16::ZERO && n.amount_left > I16F16::ZERO {
                n.amount_left = if n.amount_left > n.decay_per_tick {
                    n.amount_left - n.decay_per_tick
                } else {
                    I16F16::ZERO
                };
            }
        }
        // Production (M6, TEC-03): every Town Center runs its
        // shared queue — villagers and research in one FIFO. The
        // head accrues progress; a *unit* head stalls while its
        // owner is at the population cap (the "housed" state: the
        // queue keeps its place and its progress, and resumes the
        // tick housing frees up, so the cap can never be exceeded),
        // while a *research* head accrues regardless — population
        // gates units, not ideas. Completion spawns only while
        // there is room. (M3: the room check counts every living
        // population unit — villagers, Scout, military — not just
        // villagers. M5: every Town Center runs its own queue,
        // checked against its owner's population and capacity.)
        let tc_ids: Vec<u32> = self.tc_states.iter().map(|s| s.tc_id).collect();
        for tc_id in tc_ids {
            let owner = match self.entity_index(tc_id) {
                Some(i) if !self.entities[i].dead => self.entities[i].owner,
                _ => continue,
            };
            let si = self
                .tc_states
                .iter()
                .position(|s| s.tc_id == tc_id)
                .expect("listed");
            let Some(head) = self.tc_states[si].queue.first().copied() else {
                continue;
            };
            if matches!(head, ProdEntry::Unit(_))
                && self.living_population_of(owner) >= self.population_cap_of(owner)
            {
                continue; // housed: this Town Center's queue stalls
            }
            self.tc_states[si].progress += 1;
            if self.tc_states[si].progress >= head.ticks() {
                self.tc_states[si].progress = 0;
                self.tc_states[si].queue.remove(0);
                match head {
                    ProdEntry::Unit(_) => self.spawn_trained_villager(tc_id),
                    ProdEntry::Tech(tech) => self.apply_research(owner, tech),
                }
            }
        }
        // Placed-building queues (M3 production; M6 research
        // shares them): the same discipline as the Town Center's —
        // unit heads stall while housed, research heads do not.
        for bi in 0..self.buildings.len() {
            if !self.buildings[bi].complete || self.buildings[bi].queue.is_empty() {
                continue;
            }
            let owner = self.buildings[bi].owner;
            let head = self.buildings[bi].queue[0];
            if matches!(head, ProdEntry::Unit(_))
                && self.living_population_of(owner) >= self.population_cap_of(owner)
            {
                continue; // housed: this building's queue stalls too
            }
            self.buildings[bi].train_progress += 1;
            if self.buildings[bi].train_progress >= head.ticks() {
                self.buildings[bi].train_progress = 0;
                self.buildings[bi].queue.remove(0);
                let bid = self.buildings[bi].id;
                match head {
                    ProdEntry::Unit(kind) => self.spawn_trained_unit(bid, kind),
                    ProdEntry::Tech(tech) => self.apply_research(owner, tech),
                }
            }
        }
        for idx in 0..self.entities.len() {
            if self.entities[idx].kind == EntityKind::Villager && !self.entities[idx].dead {
                self.work_tick(idx);
            }
        }
        self.construction_tick();
    }

    /// Spawn a unit trained at a production building: on the first
    /// free ring cell around the footprint (fixed order), falling back
    /// to edge offsets and finally the building's center — a completed
    /// training must never silently eat the unit.
    fn spawn_trained_unit(&mut self, building: u32, kind: EntityKind) {
        let bi = match self.building_index(building) {
            Some(bi) => bi,
            None => return,
        };
        let b = &self.buildings[bi];
        let (ax, ay, size) = (b.cell_x, b.cell_y, b.kind.footprint_size());
        let (cx0, cy0) = (b.pos_x, b.pos_y);
        let (rally, rally_node) = (b.rally, b.rally_node);
        let owner = b.owner; // the unit belongs to the building's player (M5)
        let mut pos = None;
        for (cx, cy) in build::ring_cells(ax, ay, size) {
            if cx >= GRID_DIM || cy >= GRID_DIM {
                continue;
            }
            if !self.grid.is_blocked(cx, cy) {
                pos = Some(self.grid.cell_center(cx, cy));
                break;
            }
        }
        if pos.is_none() {
            let half = I16F16::from_num(size as i32 * 2) + I16F16::from_num(2);
            for (ox, oy) in [
                (half, I16F16::ZERO),
                (-half, I16F16::ZERO),
                (I16F16::ZERO, half),
                (I16F16::ZERO, -half),
            ] {
                let p = (cx0 + ox, cy0 + oy);
                let (ccx, ccy) = self.grid.world_to_cell(p.0, p.1);
                if !self.grid.is_blocked(ccx, ccy) {
                    pos = Some(p);
                    break;
                }
            }
        }
        let (px, py) = pos.unwrap_or((cx0, cy0));
        // CLS-03 (M6): with the Man-at-Arms upgrade researched, new
        // Militia production uses the new tier's row — the queue
        // entry still says Militia; the spawn is a Man-at-Arms.
        let kind = if kind == EntityKind::Militia && self.has_tech(owner, TechId::ManAtArms) {
            EntityKind::ManAtArms
        } else {
            kind
        };
        let id = self.entities.len() as u32;
        self.entities.push(Entity::spawned(id, kind, owner, px, py));
        // Spawn at the owner's *effective* maximum (M6: a villager
        // trained after Loom starts at 40 HP, not the base 25).
        let idx = self.entities.len() - 1;
        let max_hp = self.unit_stats(idx).max_hp;
        self.entities[idx].hp = I16F16::from_num(max_hp);
        // Rally (M4, SEL-13): the new unit walks to the building's
        // rally point (military units just walk — only villagers can
        // be sent straight to a node, and Barracks train none).
        self.apply_spawn_rally(idx, rally, rally_node);
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
                // M7 (MAP-04): a tree's own cell is blocked by the
                // tree, so a woodcutter "arrives" by standing on any
                // cell beside it — the gather range alone (2.5
                // units) can never be met from a neighboring cell
                // center 4 units away.
                let beside_tree = {
                    let ni = self.node_index(node_id).expect("live node checked");
                    if self.nodes[ni].kind == NodeKind::Tree {
                        let (vx, vy) = self.grid.world_to_cell(
                            self.entities[idx].pos_x,
                            self.entities[idx].pos_y,
                        );
                        let (tx, ty) = self.grid.world_to_cell(nx, ny);
                        vx.abs_diff(tx) <= 1 && vy.abs_diff(ty) <= 1
                    } else {
                        false
                    }
                };
                if d <= GATHER_RANGE || beside_tree {
                    let e = &mut self.entities[idx];
                    e.task = Task::Gathering;
                    e.order = None;
                    e.vel_x = I16F16::ZERO;
                    e.vel_y = I16F16::ZERO;
                } else if self.entities[idx].order.is_none()
                    && !self.move_order_to_node(idx, node_id)
                {
                    // The node cannot be reached from here (a
                    // building completed around it, or around the
                    // villager): drop the assignment and idle —
                    // retrying would re-run a failing full-grid A*
                    // every tick (M6 perf fix).
                    self.stop_task(idx);
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
                        // With a load on its back the villager hauls
                        // it home first (the deposit step resolves
                        // what comes next); empty-handed, it looks
                        // for the node's same-resource neighbor right
                        // away (ECO-09).
                        if self.entities[idx].carry > I16F16::ZERO {
                            self.begin_return(idx);
                        } else if !self.retarget_after_depletion(idx, node_id) {
                            self.stop_task(idx);
                        }
                        return;
                    }
                };
                let rate = self.nodes[ni].kind.gather_rate();
                // Carry capacity is the node's, not a global (ECO-03):
                // 35 from a hunt carcass, 10 elsewhere. A villager
                // arriving already fuller than this node's cap (it
                // hauled meat, then was re-tasked) has negative room:
                // `take` clamps to nothing and the full check below
                // sends it home to deposit first.
                let cap = self.nodes[ni].kind.carry_cap();
                let room = cap - self.entities[idx].carry;
                let take = rate.min(room).min(self.nodes[ni].amount_left);
                if take > I16F16::ZERO {
                    self.nodes[ni].amount_left -= take;
                    // MAP-04 (M7): a tree is its cell's block *until
                    // depleted* — the tick it runs dry, its cell
                    // opens (the stump does not block; the forest
                    // can be chopped through).
                    if self.nodes[ni].kind == NodeKind::Tree
                        && self.nodes[ni].amount_left <= I16F16::ZERO
                    {
                        let (cx, cy) = self
                            .grid
                            .world_to_cell(self.nodes[ni].pos_x, self.nodes[ni].pos_y);
                        self.grid.set_node_block(cx, cy, false);
                    }
                    let resource = self.nodes[ni].resource();
                    let e = &mut self.entities[idx];
                    e.carry += take;
                    if e.carry_kind.is_none() {
                        e.carry_kind = Some(resource);
                    }
                }
                let full = self.entities[idx].carry >= cap;
                if full || self.nodes[ni].is_depleted() {
                    if self.entities[idx].carry > I16F16::ZERO {
                        self.begin_return(idx);
                    } else {
                        self.stop_task(idx);
                    }
                }
            }
            Task::Returning => {
                // Haul to the nearest valid drop-off for the load
                // (M6, ECO-06) — re-resolved every tick, so a camp
                // completing mid-haul (or the destination falling)
                // reroutes the trip, and the recorded target is
                // always the current one. The move order itself is
                // re-issued only when the target *changes* or the
                // order runs out: a camp's steering point depends on
                // the approach line, and re-issuing it every tick
                // would re-path forever. With nowhere valid to go
                // the villager stops, carry kept — the pre-M6
                // no-Town-Center behavior, generalized.
                let (owner, kind, px, py) = {
                    let e = &self.entities[idx];
                    (e.owner, e.carry_kind, e.pos_x, e.pos_y)
                };
                let target = match kind {
                    Some(k) => self.deposit_target(owner, k, px, py),
                    None => None,
                };
                match target {
                    Some(t) => {
                        if self.dropoff_arrived(owner, px, py, t) {
                            self.deposit(idx);
                        } else if self.entities[idx].task_dropoff != Some(t)
                            || self.entities[idx].order.is_none()
                        {
                            self.entities[idx].task_dropoff = Some(t);
                            if !self.steer_to_dropoff(idx, owner, t) {
                                // The drop-off cannot be reached
                                // from here: stop with the load kept
                                // (re-taskable) instead of re-pathing
                                // every tick (M6 perf fix).
                                self.stop_task(idx);
                            }
                        }
                    }
                    None => {
                        self.entities[idx].task_dropoff = None;
                        self.stop_task(idx);
                    }
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
                let (range, size) = (
                    self.buildings[bi].kind.build_range(),
                    self.buildings[bi].kind.footprint_size(),
                );
                let d = self.distance_to(idx, bx, by);
                if d <= range {
                    let e = &mut self.entities[idx];
                    e.task = Task::Building;
                    e.order = None;
                    e.vel_x = I16F16::ZERO;
                    e.vel_y = I16F16::ZERO;
                } else if self.entities[idx].order.is_none() {
                    match self.approach_point(idx, ax, ay, size) {
                        Some((px, py)) => {
                            self.set_move_order(idx, px, py);
                        }
                        // No standable ring cell is reachable: the
                        // site is enclosed — give up and idle
                        // instead of probing the ring (a find_path
                        // per cell) every tick (M6 perf fix).
                        None => self.stop_task(idx),
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
                let range = self.buildings[bi].kind.build_range();
                if self.distance_to(idx, bx, by) > range {
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

    /// The construction phase of a tick (M2.5): every building with
    /// at least one villager in the `Building` phase gains progress
    /// at the AoE2 multi-builder rate — total time T × 3/(n+2) for
    /// n active builders, accrued in thirds of a tick so the rate is
    /// exact (see `build::build_rate_thirds`). The builder count is
    /// taken fresh each tick, so builders joining or leaving
    /// mid-build simply change the pace from that tick on. Buildings
    /// reaching their build total complete. Depleted farms retire
    /// (their footprint frees up for new placement).
    fn construction_tick(&mut self) {
        // Count the active builders of each building, in building
        // order (ids are assigned in placement order, so this — like
        // every other sim iteration — is deterministic).
        let mut counts: Vec<u32> = vec![0; self.buildings.len()];
        for e in &self.entities {
            if e.task != Task::Building {
                continue;
            }
            if let Some(bid) = e.task_building {
                if let Some(bi) = self.building_index(bid) {
                    counts[bi] += 1;
                }
            }
        }
        let mut completed: Vec<u32> = Vec::new();
        for (bi, &n) in counts.iter().enumerate() {
            if n == 0 || self.buildings[bi].complete {
                continue;
            }
            let b = &mut self.buildings[bi];
            b.progress_thirds += build::build_rate_thirds(n);
            if b.progress_thirds >= b.kind.build_thirds() {
                completed.push(b.id);
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
        // Entombment cure (M6): a unit that wandered (or was
        // pushed) onto the footprint while the building rose is
        // displaced to the nearest open cell when it completes —
        // a unit left standing inside a blocking footprint sits in
        // a cell no path can target, which used to strand sheep
        // beyond hunting and could strand a player's last unit
        // beyond any attacker. Placement already refuses *new*
        // overlaps (validate_build); this covers everything that
        // moved in afterwards. Deterministic: entities in id
        // order, the ring search in fixed order.
        if self.buildings[bi].kind != BuildingKind::Farm {
            let (ax, ay, size) = (
                self.buildings[bi].cell_x,
                self.buildings[bi].cell_y,
                self.buildings[bi].kind.footprint_size(),
            );
            let cells = build::footprint_cells(ax, ay, size);
            for idx in 0..self.entities.len() {
                if self.entities[idx].dead {
                    continue;
                }
                let cell = self.grid
                    .world_to_cell(self.entities[idx].pos_x, self.entities[idx].pos_y);
                if cells.contains(&cell) {
                    self.displace_out_of_blocked(idx);
                }
            }
        }
        // M6 (OPENING-SPEC §2.4, Bucket B): a builder who finishes
        // a drop-off building deposits whatever it carries into the
        // new building on the spot, when the building accepts the
        // load — the classic "build a camp on the woodline, keep
        // chopping" flow. Only the active builders (the `Building`
        // phase) are on site to do it; the credit is the building
        // owner's, like every deposit.
        if self.buildings[bi].kind.is_dropoff() {
            let kind = self.buildings[bi].kind;
            for idx in 0..self.entities.len() {
                let e = &self.entities[idx];
                if e.task != Task::Building || e.task_building != Some(bid) {
                    continue;
                }
                if e.carry > I16F16::ZERO {
                    if let Some(res) = e.carry_kind {
                        if kind.accepts(res) {
                            let (carry, owner) = (e.carry, e.owner);
                            let e = &mut self.entities[idx];
                            e.carry = I16F16::ZERO;
                            e.carry_kind = None;
                            self.stockpile_mut(owner).add(res, carry);
                        }
                    }
                }
            }
        }
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
        self.nodes.push(ResourceNode::placed(nid, NodeKind::Farm, px, py));
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

    /// Deposit the villager's carry into *its owner's* stockpile
    /// (M5), then resume the kept assignment: the gather node if it
    /// is still alive (retargeting to a same-resource neighbor when
    /// it died — ECO-09), or the construction site it was pulled
    /// from by a force drop-off (M6, ECO-07's resume); otherwise go
    /// idle.
    fn deposit(&mut self, idx: usize) {
        let (carry, kind) = {
            let e = &mut self.entities[idx];
            let taken = (e.carry, e.carry_kind);
            e.carry = I16F16::ZERO;
            e.carry_kind = None;
            e.task_dropoff = None;
            e.order = None;
            e.vel_x = I16F16::ZERO;
            e.vel_y = I16F16::ZERO;
            taken
        };
        if let Some(kind) = kind {
            if carry > I16F16::ZERO {
                let owner = self.entities[idx].owner;
                self.stockpile_mut(owner).add(kind, carry);
            }
        }
        match self.entities[idx].task_node {
            Some(node_id) => match self.live_node_pos(node_id) {
                Some(_) => {
                    self.entities[idx].task = Task::ToNode;
                    self.move_order_to_node(idx, node_id);
                }
                // The assignment died while the villager hauled its
                // last load home: ECO-09's "or after depositing"
                // case — continue at a same-resource neighbor of the
                // dead node if one is in reach, else go idle.
                None => {
                    if !self.retarget_after_depletion(idx, node_id) {
                        self.stop_task(idx);
                    }
                }
            },
            None => match self.entities[idx].task_building {
                // M6: a force-dropped builder returns to its site —
                // back on construction duty if the building is still
                // rising, or through the finished-site path if it
                // completed meanwhile.
                Some(bid) => match self.building_index(bid) {
                    Some(bi) if !self.buildings[bi].complete => {
                        self.assign_builder(idx, bid);
                    }
                    Some(_) => self.leave_finished_site(idx),
                    None => self.stop_task(idx),
                },
                None => self.stop_task(idx),
            },
        }
    }

    /// ECO-09 retargeting (M4): villager `idx`'s node `dead_node`
    /// just ran dry — find the nearest live node of the *same
    /// resource* within `RETARGET_RADIUS` **of the depleted node**
    /// (the same woodline / berry patch / mine cluster; the radius is
    /// a tuned constant, UNVERIFIED in public sources — see
    /// `econ::RETARGET_RADIUS`) and put the villager to work on it.
    /// Carcass nodes count like any other node of their resource
    /// (Food): the rulebook's ECO-09 text excludes nothing, and the
    /// retarget only ever happens empty-handed or right after a
    /// deposit, so the hunt carry cap (35) can never leak into a
    /// half-filled load. Ties break to the lowest node id (id-order
    /// scan, strictly-nearer replaces). Returns whether a new
    /// assignment was made.
    fn retarget_after_depletion(&mut self, idx: usize, dead_node: u32) -> bool {
        let (resource, dx, dy) = match self.node_index(dead_node) {
            Some(ni) => {
                let n = &self.nodes[ni];
                (n.resource(), n.pos_x, n.pos_y)
            }
            None => return false,
        };
        let mut best: Option<(i64, u32)> = None; // (distance² bits, id)
        for n in &self.nodes {
            if n.id == dead_node || n.is_depleted() || n.resource() != resource {
                continue;
            }
            let d = steer::vec_len(n.pos_x - dx, n.pos_y - dy);
            if d > econ::RETARGET_RADIUS {
                continue;
            }
            let db = d.to_bits() as i64;
            let key = (db * db, n.id);
            if best.map(|b| key < b).unwrap_or(true) {
                best = Some(key);
            }
        }
        match best {
            Some((_, nid)) => {
                self.begin_gather(idx, nid);
                true
            }
            None => false,
        }
    }

    /// Steer a hauler toward its deposit target. The geometric
    /// steer point goes first; for a building, if that point cannot
    /// be pathed to — a camp hugging its woodline (M7's generated
    /// forests) can have the push-out point land inside the trees —
    /// fall back to the building's proven standing point
    /// (`approach_point`: a free ring cell with a real path), the
    /// same ground placement validation used. `false` only when
    /// the target is genuinely unreachable from here.
    fn steer_to_dropoff(&mut self, idx: usize, owner: u8, target: Dropoff) -> bool {
        let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let Some((tx, ty)) = self.dropoff_steer_point(owner, target, px, py) else {
            return false;
        };
        if self.set_move_order(idx, tx, ty) {
            return true;
        }
        if let Dropoff::Building(bid) = target {
            if let Some(bi) = self.building_index(bid) {
                let (ax, ay, size) = {
                    let b = &self.buildings[bi];
                    (b.cell_x, b.cell_y, b.kind.footprint_size())
                };
                if let Some((axp, ayp)) = self.approach_point(idx, ax, ay, size) {
                    return self.set_move_order(idx, axp, ayp);
                }
            }
        }
        false
    }

    /// Send the villager home to deposit, keeping its gather
    /// assignment. "Home" is the nearest valid drop-off for the
    /// load (M6, ECO-06; before M6, always the owner's Town
    /// Center); with nowhere valid left the villager simply stops,
    /// carry kept.
    fn begin_return(&mut self, idx: usize) {
        let (owner, kind, px, py) = {
            let e = &self.entities[idx];
            (e.owner, e.carry_kind, e.pos_x, e.pos_y)
        };
        let target = match kind {
            Some(k) => self.deposit_target(owner, k, px, py),
            None => None,
        };
        match target {
            Some(t) => {
                self.entities[idx].task = Task::Returning;
                self.entities[idx].task_dropoff = Some(t);
                if !self.steer_to_dropoff(idx, owner, t) {
                    // Unreachable from here: stop, load kept (the
                    // M6 terminal-transition rule — never re-path a
                    // failing route every tick).
                    self.stop_task(idx);
                }
            }
            None => self.stop_task(idx),
        }
    }

    /// The villager's node vanished mid-task: haul the carry home if
    /// there is one (the deposit step then retires the assignment),
    /// otherwise retarget to a same-resource neighbor of the dead
    /// node (ECO-09) or stop where it stands.
    fn leave_for_deposit_or_idle(&mut self, idx: usize) {
        if self.entities[idx].carry > I16F16::ZERO {
            self.begin_return(idx);
        } else {
            let dead = self.entities[idx].task_node;
            let retargeted = match dead {
                Some(nid) => self.retarget_after_depletion(idx, nid),
                None => false,
            };
            if !retargeted {
                self.stop_task(idx);
            }
        }
    }

    /// Clear the work assignment (gather or construction) and halt the
    /// villager.
    fn stop_task(&mut self, idx: usize) {
        let e = &mut self.entities[idx];
        e.task = Task::Idle;
        e.task_node = None;
        e.task_building = None;
        e.task_dropoff = None;
        e.order = None;
        e.vel_x = I16F16::ZERO;
        e.vel_y = I16F16::ZERO;
    }

    /// Spawn a trained villager adjacent to Town Center `tc_id`
    /// (M5: each Town Center spawns its own, owned by its player,
    /// rallied by its own rally point).
    fn spawn_trained_villager(&mut self, tc_id: u32) {
        let ti = match self.entity_index(tc_id) {
            Some(i) => i,
            None => return,
        };
        let (tx, ty) = (self.entities[ti].pos_x, self.entities[ti].pos_y);
        let owner = self.entities[ti].owner;
        let mut pos = (tx + I16F16::from_num(TRAIN_SPAWN_OFFSETS[0].0), ty);
        for &(ox, oy) in &TRAIN_SPAWN_OFFSETS {
            let p = (tx + I16F16::from_num(ox), ty + I16F16::from_num(oy));
            let (cx, cy) = self.grid.world_to_cell(p.0, p.1);
            if !self.grid.is_blocked(cx, cy) {
                pos = p;
                break;
            }
        }
        // Ids are sequential: dead entities are tombstoned, never
        // removed (M3), so the next id is always the current count.
        let id = self.entities.len() as u32;
        self.entities
            .push(Entity::spawned(id, EntityKind::Villager, owner, pos.0, pos.1));
        // Spawn at the owner's effective maximum (M6, Loom — see
        // `spawn_trained_unit`).
        let idx0 = self.entities.len() - 1;
        let max_hp = self.unit_stats(idx0).max_hp;
        self.entities[idx0].hp = I16F16::from_num(max_hp);
        // Rally (M4, SEL-13): a new villager walks to its Town
        // Center's rally point — or straight to work, when the rally
        // snapped to a live resource node.
        let (rally, rally_node) = match self.tc_state(tc_id) {
            Some(s) => (s.rally, s.rally_node),
            None => (None, None),
        };
        let idx = self.entities.len() - 1;
        self.apply_spawn_rally(idx, rally, rally_node);
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
        // The grid layers, chunk-fed (M7): terrain and heights from
        // the generator, the tree-block layer (MAP-04 — a falling
        // tree changes the hash), and the dynamic layer of placed
        // buildings (M2.5). Hashing them pins the generated layout
        // to the golden-hash tripwire: any accidental layout change
        // is caught.
        h.write_u32(self.grid.width());
        h.write_u32(self.grid.height());
        h.write_bytes_chunked(self.grid.terrain_bytes());
        h.write_bytes_chunked(self.grid.height_bytes());
        h.write_bytes_chunked(self.grid.node_block_bytes());
        h.write_bytes_chunked(self.grid.dynamic_bytes());
        // M5: the match mode — a sandbox and a match with identical
        // openings must never share a hash stream.
        h.write_u8(self.mode.tag());
        // M6: per-player progression — age and the researched-tech
        // mask, in slot order. New hashed state (TEC-06: research is
        // state, so replays diverge the tick a research lands — as
        // they must). This deliberately changes the golden hash.
        for p in &self.players {
            h.write_u8(p.age.tag());
            h.write_u64(p.techs);
        }
        // M2: stockpiles in canonical resource order. M5: both
        // players' stockpiles, in slot order (human, opponent).
        for slot in &self.stockpiles {
            for amount in slot.amounts() {
                h.write_i32(amount.to_bits());
            }
        }
        // M2: nodes (id order), then the Town Center training state.
        // M3: node decay joins (carcasses are state, not decoration).
        h.write_u32(self.nodes.len() as u32);
        for n in &self.nodes {
            h.write_u32(n.id);
            h.write_u8(n.kind.tag());
            h.write_i32(n.pos_x.to_bits());
            h.write_i32(n.pos_y.to_bits());
            h.write_i32(n.amount_left.to_bits());
            h.write_i32(n.decay_per_tick.to_bits());
        }
        // M5: every Town Center's production state (queue, progress,
        // rally — M4's SEL-13 fields, now per Town Center).
        // M6: the queue is a typed entry list (units + research,
        // TEC-03) — entries are written as their flat codes. This
        // deliberately changes the golden hash.
        h.write_u32(self.tc_states.len() as u32);
        for s in &self.tc_states {
            h.write_u32(s.tc_id);
            h.write_u32(s.queue.len() as u32);
            for e in &s.queue {
                h.write_u32(e.code());
            }
            h.write_u64(s.progress);
            match s.rally {
                None => h.write_u8(0),
                Some((x, y)) => {
                    h.write_u8(1);
                    h.write_i32(x.to_bits());
                    h.write_i32(y.to_bits());
                }
            }
            match s.rally_node {
                None => h.write_u8(0),
                Some(n) => {
                    h.write_u8(1);
                    h.write_u32(n);
                }
            }
        }
        // M2.5: buildings (id order), construction state included.
        // M3: production queues join.
        h.write_u32(self.buildings.len() as u32);
        for b in &self.buildings {
            h.write_u32(b.id);
            h.write_u8(b.kind.tag());
            // M5: the building's owner and its hit points.
            h.write_u8(b.owner);
            h.write_i32(b.hp.to_bits());
            h.write_u32(b.cell_x);
            h.write_u32(b.cell_y);
            h.write_i32(b.pos_x.to_bits());
            h.write_i32(b.pos_y.to_bits());
            h.write_u64(b.progress_thirds);
            h.write_u8(u8::from(b.complete));
            match b.node_id {
                None => h.write_u8(0),
                Some(n) => {
                    h.write_u8(1);
                    h.write_u32(n);
                }
            }
            h.write_u32(b.queue.len() as u32);
            for &q in &b.queue {
                // M6: entries are typed (unit/research) — the flat
                // entry code replaces M3's bare kind tag.
                h.write_u32(q.code());
            }
            h.write_u64(b.train_progress);
            // M4: rally point + snapped node (SEL-13).
            match b.rally {
                None => h.write_u8(0),
                Some((x, y)) => {
                    h.write_u8(1);
                    h.write_i32(x.to_bits());
                    h.write_i32(y.to_bits());
                }
            }
            match b.rally_node {
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
            // M4: the order queue behind the current order (SEL-12) —
            // length and every queued order's data.
            h.write_u32(e.order_queue.len() as u32);
            for q in &e.order_queue {
                h.write_u8(q.tag());
                match *q {
                    QueuedOrder::Move { x, y } => {
                        h.write_i32(x.to_bits());
                        h.write_i32(y.to_bits());
                    }
                    QueuedOrder::Gather { node } => h.write_u32(node),
                    QueuedOrder::Attack { target } => h.write_u32(target),
                    QueuedOrder::Build { kind, x, y } => {
                        h.write_u8(kind.tag());
                        h.write_i32(x.to_bits());
                        h.write_i32(y.to_bits());
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
            // M6: the current deposit destination (ECO-06) — a
            // hauler's rerouting is state, so it is hashed like
            // every other work field. Deliberate hash change.
            match e.task_dropoff {
                None => h.write_u8(0),
                Some(d) => {
                    h.write_u8(d.tag());
                    if let Dropoff::Building(b) = d {
                        h.write_u32(b);
                    }
                }
            }
            h.write_i32(e.carry.to_bits());
            h.write_u8(match e.carry_kind {
                None => 0,
                Some(k) => k.tag(),
            });
            // M3: ownership, hit points, death, and combat state.
            h.write_u8(e.owner);
            h.write_i32(e.hp.to_bits());
            h.write_u8(u8::from(e.dead));
            match e.attack_target {
                None => h.write_u8(0),
                Some(t) => {
                    h.write_u8(1);
                    h.write_u32(t);
                }
            }
            // M5: the building under attack, if any.
            match e.attack_building {
                None => h.write_u8(0),
                Some(t) => {
                    h.write_u8(1);
                    h.write_u32(t);
                }
            }
            match e.strike_tick {
                None => h.write_u8(0),
                Some(t) => {
                    h.write_u8(1);
                    h.write_u64(t);
                }
            }
            h.write_u64(e.next_attack_tick);
            h.write_u8(u8::from(e.attack_engaged));
            match e.last_hit_by {
                None => h.write_u8(0),
                Some(t) => {
                    h.write_u8(1);
                    h.write_u32(t);
                }
            }
            h.write_u8(u8::from(e.suppress_separation));
            // M3: animal state — post, aggro, lure bookkeeping, flight.
            h.write_i32(e.home_x.to_bits());
            h.write_i32(e.home_y.to_bits());
            match e.aggro_target {
                None => h.write_u8(0),
                Some(t) => {
                    h.write_u8(1);
                    h.write_u32(t);
                }
            }
            h.write_u32(e.hits_taken);
            h.write_u8(u8::from(e.aggro_locked));
            h.write_u8(u8::from(e.returning_home));
            h.write_u64(e.flee_until);
            h.write_i32(e.flee_from_x.to_bits());
            h.write_i32(e.flee_from_y.to_bits());
        }
        // M3: projectiles in flight, in spawn order.
        h.write_u32(self.projectiles.len() as u32);
        for p in &self.projectiles {
            h.write_i32(p.spawn_x.to_bits());
            h.write_i32(p.spawn_y.to_bits());
            h.write_i32(p.impact_x.to_bits());
            h.write_i32(p.impact_y.to_bits());
            h.write_u64(p.spawn_tick);
            h.write_u64(p.impact_tick);
            match p.intended {
                None => h.write_u8(0),
                Some(t) => {
                    h.write_u8(1);
                    h.write_u32(t);
                }
            }
            // M5: the building the shot was fired at, if any.
            match p.intended_building {
                None => h.write_u8(0),
                Some(t) => {
                    h.write_u8(1);
                    h.write_u32(t);
                }
            }
            h.write_u32(p.attacker);
            h.write_u8(p.owner);
            // M7: the attacker's release-time elevation (PRJ-10) —
            // a shot's hill factor is fixed at release.
            h.write_u8(p.attacker_elev);
            h.write_u32(p.attacks.len() as u32);
            for &(class, value) in &p.attacks {
                h.write_u8(class);
                h.write_i32(value);
            }
        }
        // M5: the AI opponent's scheduling state (its decisions read
        // live state, but the wave clock is state of its own).
        h.write_u64(self.ai.next_wave_tick);
        // M7: both players' fog-of-war grids (VIS-01) — what each
        // player knows is state: the AI's decisions are functions
        // of its grid, so a replay must reproduce it exactly.
        for slot in &self.fog {
            h.write_bytes_chunked(slot);
        }
        h.finish()
    }

    /// State hash as a 16-character lowercase hex string.
    pub fn state_hash_hex(&self) -> String {
        format!("{:016x}", self.state_hash())
    }
}

/// The stockpile slot of a player owner tag (M5): the human is
/// slot 0, the opponent slot 1. Gaia has no slot.
fn player_slot(owner: u8) -> Option<usize> {
    match owner {
        entity::OWNER_PLAYER => Some(0),
        entity::OWNER_OPPONENT => Some(1),
        _ => None,
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
        // M7: on a generated map — every entity stands on open
        // ground, and every node stands on open ground too, with
        // exactly one exception: a tree node stands ON its own
        // blocking cell (MAP-04) — blocked *by itself*, by design.
        let sim = Sim::new(5);
        for e in sim.entities() {
            let (cx, cy) = sim.grid.world_to_cell(e.pos_x, e.pos_y);
            assert!(
                !sim.grid.is_blocked(cx, cy),
                "entity {} starts inside a blocked cell",
                e.id
            );
        }
        for n in sim.nodes() {
            let (cx, cy) = sim.grid.world_to_cell(n.pos_x, n.pos_y);
            if n.kind == NodeKind::Tree {
                assert!(
                    sim.grid.is_node_blocked(cx, cy),
                    "tree node {} does not block its cell (MAP-04)",
                    n.id
                );
            } else {
                assert!(
                    !sim.grid.is_blocked(cx, cy),
                    "node {} sits inside a blocked cell",
                    n.id
                );
            }
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
        // M7: the arena walls are gone — the blocked target is now
        // a standing tree's cell (MAP-04: a tree blocks its cell).
        let tree = sim
            .nodes()
            .iter()
            .find(|n| n.kind == NodeKind::Tree)
            .expect("a tree");
        let (tx, ty) = (tree.pos_x, tree.pos_y);
        sim.push_command(Command::Move {
            entity: 1,
            x: tx,
            y: ty,
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

    // ---- M5: match state (Conquest) -------------------------------
    //
    // The elimination predicate is derived state; these tests set up
    // the board directly (in-module access) rather than simulating
    // whole battles — the full combat path to Victory/Defeat is
    // covered end-to-end in tests/match.rs.

    /// Mark every entity of `owner` dead (the tombstone way: hp < 1
    /// and the dead flag, as the death sweep leaves them).
    fn wipe_units(sim: &mut Sim, owner: u8) {
        for e in &mut sim.entities {
            if e.owner == owner {
                e.hp = I16F16::ZERO;
                e.dead = true;
            }
        }
    }

    #[test]
    fn sandbox_never_decides() {
        let mut sim = Sim::new(11);
        wipe_units(&mut sim, entity::OWNER_PLAYER);
        assert_eq!(sim.match_state(), MatchState::Ongoing);
    }

    #[test]
    fn fresh_match_is_ongoing_for_both_profiles() {
        for mode in [MatchMode::AiEasy, MatchMode::AiNormal] {
            let sim = Sim::new_match(11, mode);
            assert_eq!(sim.match_state(), MatchState::Ongoing);
            // The mirrored start: two Town Centers, two stockpiles,
            // mirrored node layout appended.
            assert_eq!(
                sim.entities()
                    .iter()
                    .filter(|e| e.kind == EntityKind::TownCenter)
                    .count(),
                2
            );
            assert_eq!(sim.stockpile_of(entity::OWNER_OPPONENT), sim.stockpile());
            // M7: the generated layout carries two starts' packages
            // plus neutral dressing — strictly more nodes than one
            // start's package (a Sandbox sim with the same seed).
            assert!(sim.nodes().len() > Sim::new(11).nodes().len());
            assert_eq!(sim.population_of(entity::OWNER_OPPONENT), 4);
            assert_eq!(sim.population_cap_of(entity::OWNER_OPPONENT), 5);
        }
    }

    #[test]
    fn eliminating_the_opponent_is_victory() {
        let mut sim = Sim::new_match(11, MatchMode::AiNormal);
        wipe_units(&mut sim, entity::OWNER_OPPONENT);
        assert_eq!(sim.match_state(), MatchState::Victory);
    }

    #[test]
    fn eliminating_the_human_is_defeat_even_on_a_tie() {
        let mut sim = Sim::new_match(11, MatchMode::AiNormal);
        wipe_units(&mut sim, entity::OWNER_PLAYER);
        assert_eq!(sim.match_state(), MatchState::Defeat);
        // Both gone on the same read: the human loses the tie.
        wipe_units(&mut sim, entity::OWNER_OPPONENT);
        assert_eq!(sim.match_state(), MatchState::Defeat);
    }

    /// A bare-bones placed building record for predicate tests.
    fn placed_house(owner: u8) -> Building {
        Building {
            id: 0,
            kind: BuildingKind::House,
            owner,
            hp: I16F16::from_num(BuildingKind::House.max_hp()),
            cell_x: 30,
            cell_y: 30,
            pos_x: I16F16::ZERO,
            pos_y: I16F16::ZERO,
            progress_thirds: 0,
            complete: false,
            node_id: None,
            queue: Vec::new(),
            train_progress: 0,
            rally: None,
            rally_node: None,
        }
    }

    #[test]
    fn a_lone_building_keeps_its_owner_alive() {
        // The human has no resignation rule: with every unit gone
        // but a House standing, Conquest has not eliminated them —
        // the building must fall too. (Under construction counts:
        // this record is incomplete.)
        let mut sim = Sim::new_match(11, MatchMode::AiNormal);
        sim.buildings.push(placed_house(entity::OWNER_PLAYER));
        wipe_units(&mut sim, entity::OWNER_PLAYER);
        assert_eq!(sim.match_state(), MatchState::Ongoing);
        sim.buildings.clear();
        assert_eq!(sim.match_state(), MatchState::Defeat);
        // The opponent mirror ends as Victory — there the AI-05
        // resignation (no Town Center, no villagers) fires before
        // the building clause is even consulted.
        let mut sim2 = Sim::new_match(11, MatchMode::AiNormal);
        sim2.buildings.push(placed_house(entity::OWNER_OPPONENT));
        wipe_units(&mut sim2, entity::OWNER_OPPONENT);
        assert_eq!(sim2.match_state(), MatchState::Victory);
    }

    #[test]
    fn ai_resignation_needs_no_villagers_either() {
        // Opponent Town Center dead but a villager alive: no
        // resignation, no elimination — the match goes on (the
        // villager could, in principle, be the seed of a rebuild).
        let mut sim = Sim::new_match(11, MatchMode::AiNormal);
        let opp_tc = sim.tc_entity_of(entity::OWNER_OPPONENT).unwrap().id;
        let i = sim.entity_index(opp_tc).unwrap();
        sim.entities[i].hp = I16F16::ZERO;
        sim.entities[i].dead = true;
        assert_eq!(sim.match_state(), MatchState::Ongoing);
        // Kill the villagers too (Scout left alive): resignation —
        // Victory even though an opponent unit still stands.
        for e in &mut sim.entities {
            if e.owner == entity::OWNER_OPPONENT && e.kind == EntityKind::Villager {
                e.hp = I16F16::ZERO;
                e.dead = true;
            }
        }
        assert_eq!(sim.match_state(), MatchState::Victory);
    }
}
