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
//! (sheep/boar/deer/wolves) and hunting with decaying carcasses, the starting
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
mod market;
mod rng;
mod steer;
mod tech;
mod units;
#[cfg(test)]
mod housed_tests;
#[cfg(test)]
mod market_tests;
#[cfg(test)]
mod new_units_tests;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use build::{
    build_rate_thirds, repair_rate_hp_per_min, wall_line_cells, Building, BuildingKind,
    ARCHERY_RANGE_BUILD_TICKS, ARCHERY_RANGE_WOOD_COST, BARRACKS_BUILD_TICKS,
    BARRACKS_WOOD_COST, BLACKSMITH_BUILD_TICKS, BLACKSMITH_WOOD_COST, BUILD_RANGE,
    CAMP_BUILD_TICKS, CAMP_WOOD_COST, FARM_BUILD_TICKS, FARM_WOOD_COST, FOOTPRINT_CELLS,
    HOUSE_BUILD_TICKS, HOUSE_POP_SUPPORT, HOUSE_WOOD_COST, REPAIR_COST_FRACTION,
    REPAIR_HP_PER_MIN_EXTRA, REPAIR_HP_PER_MIN_FIRST, STABLE_BUILD_TICKS, STABLE_WOOD_COST,
    TOWN_CENTER_POP_SUPPORT,
};
pub use combat::{
    compute_damage, compute_damage_elev, compute_damage_hit, elevation_factor, Projectile,
    DEFENSIVE_LEASH,
    MELEE_REACH,
};
pub use econ::{
    NodeKind, ResourceKind, ResourceNode, Stockpile, CARRY_CAPACITY, DEPOSIT_RANGE, GATHER_RANGE,
    HUNT_CARRY_CAPACITY, RALLY_NODE_SNAP_RADIUS, RETARGET_RADIUS, TRAIN_QUEUE_CAP,
    TREE_STAND_INSET, VILLAGER_FOOD_COST, VILLAGER_TRAIN_TICKS,
};
pub use entity::{
    is_player_owner, Dropoff, Entity, EntityKind, MoveOrder, QueuedOrder, Stance, Task,
    OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER,
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
/// targets), so the fog model needs its own value. 8 tiles is the
/// reference game's documented Town Center LoS (public building
/// tables; study 2's exploration lesson is anchored on this disc —
/// the scout's spiral starts at the edge of the TC's line of
/// sight). Town Watch / Town Patrol (+4) are not implemented.
pub const TC_SIGHT_TILES: u32 = 8;
/// A placed building's sight radius in tiles (M7, VIS-02/03).
/// ADOPTED for every other kind (UNVERIFIED) — the Town Center's
/// own value is documented; see `TC_SIGHT_TILES`.
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
/// `ForceDeposit` (the force-drop-off gesture, ECO-07); the repair
/// milestone (BLD-07) adds `Repair`.
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
    /// Order a villager to raise a line of wall segments (BLD-13):
    /// the dragged line from the cell containing `(x0, y0)` to the
    /// cell containing `(x1, y1)`, expanded deterministically by
    /// `build::wall_line_cells`. Each segment validates exactly
    /// like a single `Build` of the same kind (same gates, same
    /// Town Center reservation, same per-segment cost of 3 wood
    /// paid at placement); a segment that fails validation — an
    /// obstructed cell, the wood running out mid-line — is skipped
    /// and leaves a gap, never a partial charge. The villager is
    /// assigned to the first segment that takes effect and queues
    /// plain `Build` orders for the rest, so it walks the line
    /// raising segment after segment (later segments resume the
    /// standing foundations, exactly as if ordered one by one).
    /// If no segment takes effect the whole order is rejected and
    /// nothing changes. Wall kinds only; any other kind rejects.
    BuildLine {
        entity: u32,
        kind: BuildingKind,
        x0: I16F16,
        y0: I16F16,
        x1: I16F16,
        y1: I16F16,
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
    /// Set a specific Town Center's rally point (M8): the
    /// per-Town-Center form of `SetRally { building: None, .. }`,
    /// which names the human Town Center implicitly. Lockstep
    /// application uses this form for both seats, so a rally order
    /// from either player means the same thing on every client.
    /// The rules are identical to the implicit form (the point is
    /// clamped; a nearby live node is recorded as the snap target).
    SetRallyTc {
        tc: u32,
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
    /// Order a villager to repair one of its own player's placed
    /// buildings (BLD-07): travel to the building's ring like a
    /// builder, then restore its HP at the documented flat rate
    /// (750 HP/min for the first repairer, +375 HP/min per
    /// additional one), charged as a wood drip totaling 50% of the
    /// building's build cost over a full 0→100% repair. Valid
    /// targets are *complete*, *damaged* buildings owned by the
    /// villager's player — enemy, Gaia, undamaged, full and
    /// under-construction targets are ignored (an unfinished
    /// building is resumed via `Build`, not repaired). The Town
    /// Center is an entity in this architecture, not a placed
    /// building, and is NOT a repair target. When the owner's
    /// stockpile cannot cover a tick's drip, the repairer stops
    /// and goes idle (the documented DE behavior).
    Repair { entity: u32, building: u32 },
    /// Buy one lot (100) of a commodity at a completed Market
    /// (TRD-01..03): the building's owner pays the current buy
    /// price in gold and receives 100 of the commodity; the global
    /// rate then rises by 2 for everyone. The whole transaction is
    /// rejected (nothing moves) when the building is missing, not
    /// a Market, unfinished, the resource is gold, or the owner's
    /// gold is short (see `Sim::can_market_buy`).
    MarketBuy { building: u32, resource: ResourceKind },
    /// Sell one lot (100) of a commodity at a completed Market
    /// (TRD-01..03): the building's owner gives 100 of the
    /// commodity and receives the current sell price in gold; the
    /// global rate then falls by 2 for everyone. Rejected whole
    /// under the same discipline as `MarketBuy` (see
    /// `Sim::can_market_sell`).
    MarketSell { building: u32, resource: ResourceKind },
    /// Force drop-off (M6, ECO-07): a villager carrying resources is
    /// ordered to haul them to the nearest valid drop-off for what
    /// it carries *now* — and, the deposit made, to return to its
    /// previous work on its own: its gather assignment (or its
    /// construction site) is kept as the resume marker and the work
    /// cycle resumes from it. This is the order behind the player's
    /// right-click on their own Town Center or camp. Rejected when
    /// the villager carries nothing or no valid drop-off stands.
    ForceDeposit { entity: u32 },
    /// Toggle AutoExplore (SEL-17) on a player's Scout Cavalry.
    /// With `enabled`, the scout hands its movement to the sim's
    /// frontier walker: it repeatedly rides to the nearest
    /// unexplored cell of its owner's fog grid until a manual order
    /// lands or the mode is toggled off (off stops it where it
    /// stands). Valid for any living Scout Cavalry owned by a
    /// player — the starting scout and trained scouts alike
    /// (current DE, update 111772: every scout has the option);
    /// other kinds, dead or unknown entities ignore the command.
    /// The toggle replaces the scout's current order and queue,
    /// like any fresh direct order.
    SetAutoExplore { entity: u32, enabled: bool },
    /// Attack-move (CMB-10): the unit walks to the point, engaging
    /// whatever enters its sight en route (the CMB-08 chain), then
    /// resumes the walk and holds at the destination. Military
    /// units only — a unit that can be ordered to attack (the
    /// `can_attack_move` rule). Replaces the unit's current order
    /// and queue, like any fresh direct order.
    AttackMove { entity: u32, x: I16F16, y: I16F16 },
    /// Patrol (CMB-11): the unit loops between its current position
    /// and the point, engaging en route like an attack-move. On a
    /// unit that already patrols, the point is appended to the
    /// loop instead of restarting it. Military units only.
    Patrol { entity: u32, x: I16F16, y: I16F16 },
    /// Set a military unit's stance (CMB-09): a persistent setting
    /// governing automatic engagement only — it never cancels the
    /// unit's current order, queue, or target. Setting Defensive
    /// anchors the leash post where the unit stands now.
    SetStance { entity: u32, stance: Stance },
    /// Stop (SEL-14): halt a unit's current order immediately —
    /// move, work task, attack engagement, AutoExplore and the
    /// whole queued-order chain all end; the unit stands idle
    /// where it is. A villager keeps what it carries (the load is
    /// only ever counted on deposit). Valid for any orderable
    /// entity (the `orderable` rule: living, player-owned,
    /// mobile) — buildings hold no orders to stop; their queue is
    /// cancelled entry by entry instead (TEC-05, below).
    Stop { entity: u32 },
    /// Delete a unit deliberately (SEL-14): the entity dies this
    /// tick through the ordinary death sweep (CMB-14) — orders
    /// and carried resources vanish with it, population frees the
    /// same tick, and an animal leaves its carcass exactly as if
    /// killed (minus a killer: nobody starts harvesting it).
    /// Valid for any living entity owned by a player, the Town
    /// Center included (it is a building for this rule's
    /// purposes). Gaia's wild animals are not yours to delete.
    DeleteEntity { entity: u32 },
    /// Delete a placed building deliberately (SEL-14): the
    /// building is removed through the ordinary destruction path
    /// (CMB-15) — footprint freed, queue dropped, support
    /// withdrawn, its builders and repairers put back to idle.
    /// A building still under construction is a *cancelled
    /// foundation* and refunds 100% of its wood cost (BLD-02);
    /// a completed building refunds nothing.
    DeleteBuilding { building: u32 },
    /// Cancel one entry of a Town Center's production queue
    /// (TEC-05): the entry is removed and its full cost refunded
    /// to the Town Center's owner — cancelling the head also
    /// discards its progress (the next entry starts fresh).
    /// Out-of-range indices and dead/unknown Town Centers are
    /// ignored.
    CancelQueueTc { tc: u32, index: u32 },
    /// Cancel one entry of a placed building's production queue
    /// (TEC-05): the same rule as the Town Center form, against
    /// the building's owner.
    CancelQueueBuilding { building: u32, index: u32 },
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
    /// Population is at capacity: training is refused until more
    /// housing stands (the "housed" state). The refusal is at
    /// enqueue time (Claudio's decision, 2026-10-27, bot-loop
    /// HOUSED front): a unit is never queued into a stall at 0% —
    /// see `can_train_at` / `can_train_villager_at`.
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
    /// Two human players (M8, lockstep multiplayer): the mirrored
    /// opponent start spawns exactly as in the AI modes and Conquest
    /// applies, but no script drives the second seat — every command
    /// for both players arrives from outside, through the ordinary
    /// command queue. The mode exists so a networked match is still
    /// just (seed, mode, command stream), like every other mode.
    TwoPlayer,
}

impl MatchMode {
    /// Stable tag for the state hash and the WASM ABI:
    /// 0 sandbox, 1 AI Easy, 2 AI Normal, 3 two-player (M8).
    pub fn tag(self) -> u8 {
        match self {
            MatchMode::Sandbox => 0,
            MatchMode::AiEasy => 1,
            MatchMode::AiNormal => 2,
            MatchMode::TwoPlayer => 3,
        }
    }

    /// Inverse of `tag`; unknown values map to `None`.
    pub fn from_tag(tag: u8) -> Option<MatchMode> {
        match tag {
            0 => Some(MatchMode::Sandbox),
            1 => Some(MatchMode::AiEasy),
            2 => Some(MatchMode::AiNormal),
            3 => Some(MatchMode::TwoPlayer),
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
    /// head only; a unit head stalls only if the cap has dropped
    /// below the living population (enqueue itself is refused at
    /// the cap — see `can_train_villager_at`), while a research
    /// head never stalls (population gates units, not ideas).
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
    /// VIC-04 run counter: the summed resource cost of every enemy
    /// unit and building this player has destroyed (kills credited
    /// through `Entity::last_hit_by` at the death sweep; buildings
    /// through `combat::damage_building`'s lethal hit). The
    /// Military score is 20% of this value. Real hashed state —
    /// see the conditional write in `hash_parts`.
    destroyed_value: u64,
}

/// One player's score breakdown (VIC-04; wiki, Score): the four
/// category scores and their sum, each an integer (the category
/// terms are floored by integer division — the wiki pins the
/// percentages, not a rounding rule).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Score {
    /// 20% of the resource cost of all enemy units and buildings
    /// the player destroyed or converted.
    pub military: i64,
    /// 10% of resources currently held (plus tribute paid — no
    /// tribute exists in this game, so the term is 0) + 20% of the
    /// resource cost of surviving units and standard buildings.
    pub economy: i64,
    /// 20% of the resource cost of every researched technology
    /// + 10 points per 1% of the map explored.
    pub technology: i64,
    /// 20% of the cost of Castles and Wonders constructed. No
    /// such building kind exists yet, so this is structurally 0.
    pub society: i64,
    /// Military + Economy + Technology + Society.
    pub total: i64,
}

/// The resource value (cost) of a unit kind: the sum of its
/// production cost components in `units::spec` (VIC-04's "resource
/// value (cost)"). Kinds with no production cost — the Town
/// Center, herdables, wild animals — are worth 0.
fn unit_cost_value(kind: EntityKind) -> i64 {
    let s = units::spec(kind);
    s.food_cost as i64 + s.wood_cost as i64 + s.gold_cost as i64
}

/// SIM-06 subsystem checksums: the running values of the one
/// canonical state hash (SIM-05) at subsystem boundaries. Each
/// field is the hash of the stream prefix up to that boundary, so
/// comparing two runs part by part names the first subsystem whose
/// state diverged: `rng` (tick + RNG words), `world` (grid layers —
/// terrain, heights, and the block layers pathfinding reads),
/// `economy` (progression, stockpiles, nodes, queues, buildings),
/// `targeting` (entities incl. their combat/targeting state, and
/// projectiles). `full` is the complete `state_hash()` value; the
/// stream's tail (AI clock, fog grids) is covered by `full` alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HashParts {
    pub rng: u64,
    pub world: u64,
    pub economy: u64,
    pub targeting: u64,
    pub full: u64,
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
    /// The match's global commodity exchange rates (TRD-02): one
    /// rate per commodity, shared by both players, moved only by
    /// executed Market commands — deterministic sim state, hashed
    /// once any rate has moved (see `hash_parts`).
    market: crate::market::Market,
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
        // CMB-09: the starting scout's Dark Age default stance is
        // Stand Ground — it never rides out to engage on its own
        // (MECHANICS-STUDY §9.1); trained scouts default to
        // Aggressive like every other military unit.
        let mut next_id = TOWN_CENTER_ID + 1 + START_VILLAGER_COUNT;
        let mut starting_scout = Entity::spawned(
            next_id,
            EntityKind::ScoutCavalry,
            entity::OWNER_PLAYER,
            tc_x + I16F16::from_num(START_SCOUT_OFFSET.0),
            tc_y + I16F16::from_num(START_SCOUT_OFFSET.1),
        );
        starting_scout.stance = Stance::StandGround;
        entities.push(starting_scout);
        next_id += 1;
        // Gaia animals at their generated cells, in the package's
        // fixed order (sheep, boars, deer, wolves) — the historical
        // id layout: 5–12 sheep, 13–14 boars, 15–20 deer, 21–23
        // wolves (AI-08 appended the pack, earlier ids unmoved).
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
            market: crate::market::Market::new(),
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
    /// boars, deer, wolves — all Gaia-owned, as placed). The package is the
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
        // The opponent's starting scout stands at Stand Ground
        // too (CMB-09 — the same default as the human's).
        let mut foe_scout = Entity::spawned(
            next_id,
            EntityKind::ScoutCavalry,
            entity::OWNER_OPPONENT,
            tc_x - I16F16::from_num(START_SCOUT_OFFSET.0),
            tc_y - I16F16::from_num(START_SCOUT_OFFSET.1),
        );
        foe_scout.stance = Stance::StandGround;
        self.entities.push(foe_scout);
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
                // BLD-18: the sight radius is the kind's own —
                // the generic 4 for ordinary buildings (unchanged),
                // the tower's and outpost's larger discs for the
                // two structures that exist to see.
                let (cx, cy) = self.grid.world_to_cell(b.pos_x, b.pos_y);
                self.stamp_sight(slot, cx, cy, b.kind.sight_tiles_at(self.age_of(owner)));
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

    /// The match's global exchange rates in canonical commodity
    /// order (food, wood, stone) — TRD-02's shared state, read by
    /// the client so prices are never recomputed outside the sim.
    pub fn market_rates(&self) -> [i32; 3] {
        self.market.rates()
    }

    /// The gold a seller receives for one lot of `resource` now.
    pub fn market_sell_price(&self, resource: ResourceKind) -> i32 {
        self.market.sell_price(resource)
    }

    /// The gold a buyer pays for one lot of `resource` now.
    pub fn market_buy_price(&self, resource: ResourceKind) -> i32 {
        self.market.buy_price(resource)
    }

    /// The owner of a standing, completed Market at `building`, if
    /// the id names one (the trade commands' validity half).
    fn market_owner(&self, building: u32) -> Option<u8> {
        let b = &self.buildings[self.building_index(building)?];
        if b.kind == BuildingKind::Market && b.complete {
            Some(b.owner)
        } else {
            None
        }
    }

    /// Whether a `MarketBuy` command would execute right now: a
    /// completed Market, a real commodity (never gold), and the
    /// owner's gold covering the current buy price (TRD-01..03).
    pub fn can_market_buy(&self, building: u32, resource: ResourceKind) -> bool {
        if !crate::market::is_commodity(resource) {
            return false;
        }
        match self.market_owner(building) {
            Some(owner) => {
                self.stockpile_of(owner).amount(ResourceKind::Gold)
                    >= I16F16::from_num(self.market.buy_price(resource))
            }
            None => false,
        }
    }

    /// Whether a `MarketSell` command would execute right now:
    /// the same building discipline, and the owner holding a full
    /// lot (100) of the commodity.
    pub fn can_market_sell(&self, building: u32, resource: ResourceKind) -> bool {
        if !crate::market::is_commodity(resource) {
            return false;
        }
        match self.market_owner(building) {
            Some(owner) => {
                self.stockpile_of(owner).amount(resource)
                    >= I16F16::from_num(crate::market::TRADE_LOT)
            }
            None => false,
        }
    }

    /// Apply a `MarketBuy` command: atomic — gold out, lot in,
    /// rate up — or nothing at all (TRD-01..03).
    fn issue_market_buy(&mut self, building: u32, resource: ResourceKind) {
        if !self.can_market_buy(building, resource) {
            return;
        }
        let Some(owner) = self.market_owner(building) else {
            return;
        };
        let price = self.market.buy_price(resource);
        let paid = self
            .stockpile_mut(owner)
            .spend(ResourceKind::Gold, I16F16::from_num(price));
        debug_assert!(paid);
        if paid {
            self.stockpile_mut(owner)
                .add(resource, I16F16::from_num(crate::market::TRADE_LOT));
            self.market.apply_buy(resource);
        }
    }

    /// Apply a `MarketSell` command: atomic — lot out, gold in,
    /// rate down — or nothing at all (TRD-01..03).
    fn issue_market_sell(&mut self, building: u32, resource: ResourceKind) {
        if !self.can_market_sell(building, resource) {
            return;
        }
        let Some(owner) = self.market_owner(building) else {
            return;
        };
        let price = self.market.sell_price(resource);
        let paid = self
            .stockpile_mut(owner)
            .spend(resource, I16F16::from_num(crate::market::TRADE_LOT));
        debug_assert!(paid);
        if paid {
            self.stockpile_mut(owner)
                .add(ResourceKind::Gold, I16F16::from_num(price));
            self.market.apply_sell(resource);
        }
    }

    /// Credit a destruction to the player who inflicted it
    /// (VIC-04, Military): `value` is the destroyed thing's
    /// resource cost. Only cross-player kills count — a Gaia
    /// killer earns nobody anything, and destroying your own
    /// unit or building (the Delete commands) credits no one:
    /// entity deletes reach the death sweep with `last_hit_by`
    /// already cleared, and building deletes call
    /// `destroy_building` directly, never this path.
    pub(crate) fn credit_destruction(&mut self, creditor: u8, victim_owner: u8, value: i64) {
        if value <= 0 {
            return;
        }
        let (Some(cs), Some(vs)) = (player_slot(creditor), player_slot(victim_owner)) else {
            return;
        };
        if cs == vs {
            return;
        }
        self.players[cs].destroyed_value += value as u64;
    }

    /// The summed resource cost credited to a player for enemy
    /// units and buildings destroyed (VIC-04's Military input).
    /// Gaia and unknown tags read 0.
    pub fn destroyed_value_of(&self, owner: u8) -> u64 {
        match player_slot(owner) {
            Some(slot) => self.players[slot].destroyed_value,
            None => 0,
        }
    }

    /// A player's score, computed from live state (VIC-04; wiki,
    /// Score — the formula the rulebook entry pins). Pure read:
    /// every term derives from current state plus the one run
    /// counter (`PlayerState::destroyed_value`) — kills cannot be
    /// recomputed from state, everything else can: stockpile and
    /// surviving values are read off the world, researched-tech
    /// cost off the progression state, explored share off the
    /// fog grid. Conversion crediting rides the same destruction
    /// counter in the reference formula; the only conversion in
    /// this game is herdable capture (AI-06), and herdables
    /// carry no resource cost in the unit table, so the term
    /// contributes 0 by data.
    pub fn score_for(&self, owner: u8) -> Score {
        let Some(slot) = player_slot(owner) else {
            return Score::default();
        };
        let p = &self.players[slot];
        // Military: 20% of destroyed/converted enemy value.
        let military = p.destroyed_value as i64 * 20 / 100;
        // Economy: 10% of the current stockpile (tribute paid is
        // the wiki's other 10% term — no tribute system exists,
        // so it is 0) + 20% of surviving units and standard
        // buildings. Every placed building counts, complete or
        // under construction; the Town Center entity's table cost
        // is 0, so it contributes nothing.
        let stock: i64 = self.stockpiles[slot]
            .amounts()
            .iter()
            .map(|a| a.to_num::<i32>() as i64)
            .sum();
        let mut surviving: i64 = 0;
        for e in &self.entities {
            if !e.dead && e.owner == owner {
                surviving += unit_cost_value(e.kind);
            }
        }
        for b in &self.buildings {
            if b.owner == owner {
                surviving += b.kind.wood_cost().to_num::<i32>() as i64;
            }
        }
        let economy = stock * 10 / 100 + surviving * 20 / 100;
        // Technology: 20% of researched-tech cost (the Feudal
        // Age advancement is a researched technology — `has_tech`
        // reads it off the age; auto-granted techs don't exist in
        // this game) + 10 per 1% of the map explored (fog bytes
        // >= 1 are explored-or-visible, VIS-01).
        let mut tech_cost: i64 = 0;
        for t in TechId::ALL {
            if self.has_tech(owner, t) {
                let (f, w, g, s) = t.cost();
                tech_cost += f as i64 + w as i64 + g as i64 + s as i64;
            }
        }
        let cells = self.fog[slot].len() as i64;
        let explored = self.fog[slot].iter().filter(|&&c| c >= 1).count() as i64;
        let explored_pct = if cells > 0 { explored * 100 / cells } else { 0 };
        let technology = tech_cost * 20 / 100 + explored_pct * 10;
        // Society: 20% of Castles and Wonders constructed — no
        // such building kind exists in the table yet.
        let society: i64 = 0;
        Score {
            military,
            economy,
            technology,
            society,
            total: military + economy + technology + society,
        }
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
    /// age advancement "technologies" read off the age state
    /// itself (Feudal from M6; Castle from the Castle P1 package).
    pub fn has_tech(&self, owner: u8, tech: TechId) -> bool {
        if tech == TechId::FeudalAge {
            return self.age_of(owner) >= Age::Feudal;
        }
        if tech == TechId::CastleAge {
            return self.age_of(owner) >= Age::Castle;
        }
        match player_slot(owner) {
            Some(slot) => self.players[slot].techs & tech.bit() != 0,
            None => false,
        }
    }

    /// A player's researched-technology bitmask (M6), with the
    /// age advancement bits set while the player stands in (or
    /// past) the matching Age — the client's one-call view of
    /// "what do I have".
    pub fn tech_mask_of(&self, owner: u8) -> u64 {
        let mut mask = match player_slot(owner) {
            Some(slot) => self.players[slot].techs,
            None => 0,
        };
        if self.age_of(owner) >= Age::Feudal {
            mask |= TechId::FeudalAge.bit();
        }
        if self.age_of(owner) >= Age::Castle {
            mask |= TechId::CastleAge.bit();
        }
        mask
    }

    /// The effective stat row of a unit kind for an owner (M6,
    /// TEC-06): the base table row layered with the owner's age
    /// (CLS-04 — the Scout's Feudal row) and researched technologies
    /// (Loom, Fletching, the Blacksmith armor lines, and the
    /// 2026-10-22 Feudal completion set: Forging, Scale Barding
    /// Armor, Bloodlines). Every stat
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
            s.speed = I16F16::from_bits(6_772); // 1.55 tiles/s (absolute tile scale)
            s.los = I16F16::from_bits(1_572_864); // 24 (6 tiles)
        }
        // BLD-11 for the Town Center entity: its armor follows
        // the owner's Age column (study §5: melee 3/4/5, pierce
        // 5/6/7 in Dark/Feudal/Castle; HP is 2400 at every Age).
        // The placed-building half of the same step lives in
        // `BuildingKind::armor_*_at`; the Town Center is an
        // entity, so its step is patched here like CLS-04's.
        if kind == EntityKind::TownCenter {
            match p.age {
                Age::Feudal => {
                    s.armor_melee = 4;
                    s.armor_pierce = 6;
                }
                Age::Castle => {
                    s.armor_melee = 5;
                    s.armor_pierce = 7;
                }
                Age::Dark => {}
            }
        }
        // TEC-04 (Loom): villagers +15 HP, +1 melee / +2 pierce
        // armor, +0.1 tiles/s (RULEBOOK TEC-06/CLS-05 — see the
        // tech module docs for the study discrepancy note).
        if kind == EntityKind::Villager && p.techs & TechId::Loom.bit() != 0 {
            s.max_hp += 15;
            s.armor_melee += 1;
            s.armor_pierce += 2;
            s.speed += I16F16::from_bits(437); // +0.1 tiles/s (absolute tile scale)
        }
        // Wheelbarrow (ECO-03): villagers +10% speed — TEC-06's
        // first percentage patch. The scalar multiplies the layered
        // row (base + the flat adds above), in exact integer bits:
        // speed × 11/10, truncating the sub-bit remainder.
        if kind == EntityKind::Villager && p.techs & TechId::Wheelbarrow.bit() != 0 {
            s.speed = I16F16::from_bits(((s.speed.to_bits() as i64 * 11) / 10) as i32);
        }
        // Hand Cart (ECO-03's second link): villagers +10% speed
        // again (wiki, Hand Cart) — the same percentage patch
        // applied a second time over the Wheelbarrow layer, the
        // sequential multiplication of the reference game's tech
        // effects (×1.1 ×1.1 over the flat-layered row).
        if kind == EntityKind::Villager && p.techs & TechId::HandCart.bit() != 0 {
            s.speed = I16F16::from_bits(((s.speed.to_bits() as i64 * 11) / 10) as i32);
        }
        // Fletching (study §8): archers +1 attack, +1 tile of range.
        // The Skirmisher shares the patch (the wiki's Fletching
        // article names Skirmishers among the beneficiaries —
        // "arrow/javelin-shooting units"), with its own table.
        if kind == EntityKind::Archer && p.techs & TechId::Fletching.bit() != 0 {
            s.attacks = units::ARCHER_FLETCHING_ATTACKS;
            s.range_max += I16F16::from_bits(262_144); // +4.0 (1 tile)
        }
        if kind == EntityKind::Skirmisher && p.techs & TechId::Fletching.bit() != 0 {
            s.attacks = units::SKIRMISHER_FLETCHING_ATTACKS;
            s.range_max += I16F16::from_bits(262_144); // +4.0 (1 tile)
        }
        // Scale Mail Armor (study §8): infantry +1/+1 armor — the
        // Spearman is infantry and takes the patch with the line.
        if matches!(
            kind,
            EntityKind::Militia | EntityKind::ManAtArms | EntityKind::Spearman
        ) && p.techs & TechId::ScaleMail.bit() != 0
        {
            s.armor_melee += 1;
            s.armor_pierce += 1;
        }
        // Padded Archer Armor (study §8): archers +1/+1 armor —
        // the Skirmisher is a foot archer of the Archery class
        // and takes the patch with the line.
        if matches!(kind, EntityKind::Archer | EntityKind::Skirmisher)
            && p.techs & TechId::PaddedArcherArmor.bit() != 0
        {
            s.armor_melee += 1;
            s.armor_pierce += 1;
        }
        // The Feudal completion set (2026-10-22; wiki pages in
        // the tech module docs). Cavalry membership is read off
        // the row's own class mask (CLS-01), never a kind list —
        // a future mounted kind inherits both patches by its
        // table row alone.
        let cavalry = s.classes & crate::classes::bit(crate::classes::CAVALRY) != 0;
        // Forging: infantry and cavalry melee attack +1 — the
        // affected rows swap to their Forging attack tables
        // (units.rs); bonus pairs are unchanged.
        if p.techs & TechId::Forging.bit() != 0 {
            match kind {
                EntityKind::Militia => s.attacks = units::MILITIA_FORGING_ATTACKS,
                EntityKind::ManAtArms => s.attacks = units::MAN_AT_ARMS_FORGING_ATTACKS,
                EntityKind::Spearman => s.attacks = units::SPEARMAN_FORGING_ATTACKS,
                EntityKind::ScoutCavalry => {
                    s.attacks = if p.age >= Age::Feudal {
                        units::SCOUT_FEUDAL_FORGING_ATTACKS
                    } else {
                        units::SCOUT_FORGING_ATTACKS
                    };
                }
                _ => {}
            }
        }
        // Scale Barding Armor: cavalry +1/+1 armor.
        if cavalry && p.techs & TechId::ScaleBarding.bit() != 0 {
            s.armor_melee += 1;
            s.armor_pierce += 1;
        }
        // Bloodlines: mounted units +20 max HP (the current-HP
        // grant for units already standing is the completion
        // sweep in `apply_research`, Loom's precedent).
        if cavalry && p.techs & TechId::Bloodlines.bit() != 0 {
            s.max_hp += 20;
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

    /// The effective weapon of a placed building kind for `owner`
    /// (BLD-18): the kind's base profile (`BuildingKind::weapon`)
    /// with the owner's researched patches layered on — the TEC-06
    /// lookup form, mirroring `stats_for`. Fletching's documented
    /// tower line applies here (+1 attack, +1 range; the tech's own
    /// table in tech.rs names Towers since M6, waiting for a tower
    /// attack to exist). `None` for unarmed kinds.
    pub(crate) fn building_weapon(
        &self,
        owner: u8,
        kind: BuildingKind,
    ) -> Option<crate::build::BuildingWeapon> {
        let mut w = kind.weapon()?;
        if kind == BuildingKind::WatchTower && self.has_tech(owner, TechId::Fletching) {
            w.attacks = crate::build::WATCH_TOWER_FLETCHING_ATTACKS;
            w.range_max += I16F16::from_num(4); // +1 tile (TEC-06)
        }
        Some(w)
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

    /// Completed buildings counting toward the Castle Age research
    /// requirement (TEC-01 Castle rung): the Feudal-born kinds
    /// (see `BuildingKind::counts_toward_castle`).
    pub fn castle_building_count(&self, owner: u8) -> u32 {
        self.buildings
            .iter()
            .filter(|b| b.owner == owner && b.complete && b.kind.counts_toward_castle())
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

    /// The match outcome from the human's seat (VIC-01): in a
    /// match, a player with no living population units and no
    /// building capable of producing units (Town Center or a
    /// placed production building, at any construction stage)
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

    /// Conquest elimination (VIC-01): no living population units
    /// and no building capable of producing units — the Town
    /// Center (an entity here) or a placed production building
    /// (Barracks / Archery Range / Stable). A production building
    /// counts at any construction stage: a foundation can still
    /// become a producer, the same convention the M5 form applied
    /// to buildings generally. Any other building — a House, a
    /// camp, a Mill — does NOT keep a unit-less player alive:
    /// the M5 form required razing every last building, which
    /// was stricter than the rule's letter.
    fn eliminated(&self, owner: u8) -> bool {
        if self.tc_entity_of(owner).is_some() {
            return false;
        }
        if self
            .buildings
            .iter()
            .any(|b| b.owner == owner && b.kind.produces_units())
        {
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
        self.tc_queue_of(TOWN_CENTER_ID)
    }

    /// A specific Town Center's production queue (M8): the
    /// per-Town-Center form of `tc_queue`, so a networked client can
    /// show its own Town Center's queue whichever seat it plays.
    pub fn tc_queue_of(&self, tc: u32) -> &[ProdEntry] {
        match self.tc_state(tc) {
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
        self.tc_progress_bits_of(TOWN_CENTER_ID)
    }

    /// A specific Town Center's head-of-queue progress (M8): the
    /// per-Town-Center form of `tc_progress_bits`.
    pub fn tc_progress_bits_of(&self, tc: u32) -> i32 {
        let s = match self.tc_state(tc) {
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

    /// Population capacity of one player (M5): +5 per *living*
    /// Town Center of the player (the starting one, plus every
    /// additional TC built from the Castle Age — BLD-15's
    /// extension; a razed TC's room is lost with it, the House
    /// rule) plus that player's completed Houses.
    pub fn population_cap_of(&self, owner: u8) -> u32 {
        let mut cap = 0;
        for e in &self.entities {
            if e.kind == EntityKind::TownCenter && e.owner == owner && !e.dead {
                cap += TOWN_CENTER_POP_SUPPORT;
            }
        }
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
    /// Why training a villager at THIS Town Center is blocked right
    /// now (M8: the per-Town-Center form, public for the WASM ABI;
    /// `train_block` is this for the human Town Center).
    pub fn train_block_at(&self, tc: u32) -> TrainBlock {
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
        // The unit-level age gate (BLD-15): a Feudal unit at a
        // Dark-Age building (the Spearman at the Barracks) is not
        // trainable yet. Reported as QueueFull — the panel greys
        // the button on the age itself, so the flash text this
        // tag feeds is never the player's only explanation.
        if self.age_of(b.owner) < kind.train_age() {
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
    /// Town Center contract: queue room, funds and population room
    /// gate the enqueue. Being housed REFUSES the enqueue
    /// (Claudio's decision, 2026-10-27, bot-loop HOUSED front):
    /// population room is read as living + already-queued units
    /// against the cap (`population_of`), so a player can queue
    /// exactly up to the cap and never one unit beyond it — no
    /// unit ever sits in a queue stalled at 0% for lack of a House.
    /// (The economy phase still stalls a unit head if the cap
    /// *drops* below the living population, e.g. a House is razed
    /// while units are queued — that guard keeps the cap absolute.)
    pub fn can_train_at(&self, building: u32, kind: EntityKind) -> bool {
        let bi = match self.building_index(building) {
            Some(bi) => bi,
            None => return false,
        };
        let b = &self.buildings[bi];
        b.complete
            && b.kind.is_production()
            && b.kind.trains(kind)
            && self.age_of(b.owner) >= kind.train_age()
            && (b.queue.len() as u32) < TRAIN_QUEUE_CAP
            && self.population_of(b.owner) < self.population_cap_of(b.owner)
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
    /// human Town Center? (Food covers the cost, the queue has
    /// room, and there is population room — being housed refuses
    /// the enqueue, see `can_train_at`.) The command path
    /// re-checks at apply time; this is for UI affordances.
    pub fn can_train_villager(&self) -> bool {
        self.can_train_villager_at(TOWN_CENTER_ID)
    }

    /// Would a villager enqueue be accepted at Town Center `tc`
    /// (M5)? The per-Town-Center form of `can_train_villager`,
    /// charged to that Town Center's owner.
    pub fn can_train_villager_at(&self, tc: u32) -> bool {
        let (owner, queue) = match (self.entity_index(tc), self.tc_state(tc)) {
            (Some(i), Some(s)) if !self.entities[i].dead => {
                (self.entities[i].owner, s.queue.len() as u32)
            }
            _ => return false,
        };
        queue < TRAIN_QUEUE_CAP
            && self.population_of(owner) < self.population_cap_of(owner)
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
        if tech == TechId::CastleAge && self.castle_building_count(owner) < 2 {
            return ResearchBlock::Locked;
        }
        // TEC-07: a tech→tech prerequisite unmet locks the
        // research (Hand Cart behind Wheelbarrow — the first
        // link of this kind).
        if let Some(req) = tech.requires_tech() {
            if !self.has_tech(owner, req) {
                return ResearchBlock::Locked;
            }
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
    /// The per-Town-Center research block (M8: public for the WASM
    /// ABI; `research_block_tc` is this for the human Town Center).
    pub fn research_block_at_tc(&self, tc: u32, tech: TechId) -> ResearchBlock {
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
    /// lookup reads the owner's researched set (TEC-06). These do:
    /// - Feudal Age flips the owner's age (TEC-04: empire-wide, at
    ///   once — lookups key on the age, so nothing else moves).
    /// - Loom grants existing villagers the +15 HP with the new
    ///   maximum (id order; the DE behavior for a stat research —
    ///   their current HP rises with the max).
    /// - Man-at-Arms swaps every living Militia of the owner to
    ///   the Man-at-Arms kind (CLS-03's line upgrade: position,
    ///   orders and current HP are kept; the max rises — no heal).
    /// - Bloodlines (2026-10-22) grants existing mounted units
    ///   the +20 HP with the new maximum — Loom's rule, on the
    ///   cavalry class mask.
    /// - Horse Collar (2026-10-22) tops up the owner's standing
    ///   farms: since DE update 169123 a farm standing at the
    ///   research tick gains 75 × (remaining ⁄ full)², rounded up
    ///   to whole food (wiki, Horse Collar — its worked example:
    ///   a half-full farm gains 19, the quarter of 75 rounded up;
    ///   the articles' examples all round up but no rule is
    ///   stated — ECO-14, inferred and followed). Farms raised
    ///   afterwards are stamped at creation (see the farm
    ///   completion path).
    fn apply_research(&mut self, owner: u8, tech: TechId) {
        let Some(slot) = player_slot(owner) else {
            return;
        };
        match tech {
            TechId::FeudalAge => {
                self.players[slot].age = Age::Feudal;
            }
            TechId::CastleAge => {
                self.players[slot].age = Age::Castle;
            }
            TechId::Loom => {
                self.players[slot].techs |= tech.bit();
                for e in &mut self.entities {
                    if e.owner == owner && e.kind == EntityKind::Villager && !e.dead {
                        e.hp += I16F16::from_num(15);
                    }
                }
            }
            TechId::Bloodlines => {
                self.players[slot].techs |= tech.bit();
                for e in &mut self.entities {
                    if e.owner == owner
                        && !e.dead
                        && units::spec(e.kind).classes
                            & crate::classes::bit(crate::classes::CAVALRY)
                            != 0
                    {
                        e.hp += I16F16::from_num(20);
                    }
                }
            }
            TechId::HorseCollar => {
                self.players[slot].techs |= tech.bit();
                let full = NodeKind::Farm.full_amount();
                let farm_nodes: Vec<u32> = self
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.owner == owner && b.kind == BuildingKind::Farm && b.complete
                    })
                    .filter_map(|b| b.node_id)
                    .collect();
                for nid in farm_nodes {
                    if let Some(node) = self.nodes.iter_mut().find(|n| n.id == nid) {
                        let frac = node.amount_left / full;
                        // ECO-14: the articles' worked examples all
                        // round the result UP to whole food (a half
                        // farm gains 19, not 18.75) — no rounding
                        // rule is stated, so the round-up is
                        // inferred; it is followed here.
                        let top_up = (I16F16::from_num(75) * frac * frac).ceil();
                        node.amount_left += top_up;
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
    /// Would a `Stop` command be accepted right now? (SEL-14.)
    /// The entity must be able to take player orders at all (the
    /// shared `orderable` rule). A pure query for UI affordances;
    /// the command path re-validates at apply time.
    pub fn can_stop(&self, entity: u32) -> bool {
        match self.entity_index(entity) {
            Some(idx) => self.orderable(idx),
            None => false,
        }
    }

    /// Apply a `Stop` command (SEL-14): end the entity's order,
    /// work task, attack engagement, AutoExplore and queued chain
    /// where it stands. The carry is kept (SEL-14's letter: a
    /// stopped villager keeps what it carries).
    fn issue_stop(&mut self, entity: u32) {
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return,
        };
        if !self.orderable(idx) {
            return;
        }
        self.entities[idx].order_queue.clear();
        self.stop_task(idx);
        let e = &mut self.entities[idx];
        e.clear_combat();
        e.auto_explore = false;
        e.auto_target = None;
        // CMB-10/11: Stop also ends any standing order.
        e.attack_move = None;
        e.patrol.clear();
        e.patrol_next = 0;
    }

    /// Would a `DeleteEntity` command be accepted right now?
    /// (SEL-14: any living entity owned by a player — Gaia's wild
    /// animals are not deletable.)
    pub fn can_delete_entity(&self, entity: u32) -> bool {
        match self.entity_index(entity) {
            Some(idx) => {
                let e = &self.entities[idx];
                !e.dead && e.owner != entity::OWNER_GAIA
            }
            None => false,
        }
    }

    /// Apply a `DeleteEntity` command (SEL-14): zero the HP and
    /// let this tick's death sweep (CMB-14) do the rest — the one
    /// death path frees population, orders and carry, and turns a
    /// deleted animal into its carcass. `last_hit_by` is cleared
    /// first: a deliberate delete has no killer, so no villager
    /// starts harvesting the carcass on anyone's behalf.
    fn issue_delete_entity(&mut self, entity: u32) {
        if !self.can_delete_entity(entity) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        let e = &mut self.entities[idx];
        e.last_hit_by = None;
        e.last_hit_owner = None;
        e.hp = I16F16::ZERO;
    }

    /// Would a `DeleteBuilding` command be accepted right now?
    /// (SEL-14: the building exists. Placed buildings are only
    /// ever player-owned in this architecture.)
    pub fn can_delete_building(&self, building: u32) -> bool {
        self.building_index(building).is_some()
    }

    /// Apply a `DeleteBuilding` command (SEL-14 + BLD-02): a
    /// foundation is a cancellation — refund its full wood cost
    /// first — then remove the building through the ordinary
    /// destruction path (CMB-15). A completed building refunds
    /// nothing.
    fn issue_delete_building(&mut self, building: u32) {
        let bi = match self.building_index(building) {
            Some(i) => i,
            None => return,
        };
        let (complete, owner, wood, stone) = {
            let b = &self.buildings[bi];
            (b.complete, b.owner, b.kind.wood_cost(), b.kind.stone_cost())
        };
        if !complete {
            // BLD-02: a foundation refunds its FULL cost — wood
            // and, for BLD-18's stone-cost kinds, stone (every
            // earlier kind's stone reads 0).
            self.stockpile_mut(owner).add(ResourceKind::Wood, wood);
            if stone > I16F16::ZERO {
                self.stockpile_mut(owner).add(ResourceKind::Stone, stone);
            }
        }
        self.destroy_building(building);
    }

    /// Refund one queue entry's full cost to its owner (TEC-05):
    /// the exact inverse of the enqueue spends (`spend_cost` /
    /// `spend_tech`).
    fn refund_entry(&mut self, owner: u8, entry: ProdEntry) {
        match entry {
            ProdEntry::Unit(kind) => {
                let spec = units::spec(kind);
                for (res, amount) in [
                    (ResourceKind::Food, spec.food_cost),
                    (ResourceKind::Wood, spec.wood_cost),
                    (ResourceKind::Gold, spec.gold_cost),
                ] {
                    if amount > 0 {
                        self.stockpile_mut(owner)
                            .add(res, I16F16::from_num(amount));
                    }
                }
            }
            ProdEntry::Tech(tech) => {
                let (food, wood, gold, stone) = tech.cost();
                for (res, amount) in [
                    (ResourceKind::Food, food),
                    (ResourceKind::Wood, wood),
                    (ResourceKind::Gold, gold),
                    (ResourceKind::Stone, stone),
                ] {
                    if amount > 0 {
                        self.stockpile_mut(owner)
                            .add(res, I16F16::from_num(amount));
                    }
                }
            }
        }
    }

    /// Would a `CancelQueueTc` command be accepted right now?
    /// (TEC-05: the Town Center stands and the index is in range.)
    pub fn can_cancel_queue_tc(&self, tc: u32, index: u32) -> bool {
        let alive = matches!(self.entity_index(tc), Some(i) if !self.entities[i].dead);
        alive
            && matches!(self.tc_state(tc), Some(s) if (index as usize) < s.queue.len())
    }

    /// Apply a `CancelQueueTc` command (TEC-05): remove the entry
    /// and refund its full cost to the Town Center's owner;
    /// cancelling the head discards its progress with it.
    fn issue_cancel_queue_tc(&mut self, tc: u32, index: u32) {
        if !self.can_cancel_queue_tc(tc, index) {
            return;
        }
        let owner = self.entities[self.entity_index(tc).expect("validated")].owner;
        let entry = {
            let s = self
                .tc_states
                .iter_mut()
                .find(|s| s.tc_id == tc)
                .expect("validated");
            let entry = s.queue.remove(index as usize);
            if index == 0 {
                s.progress = 0;
            }
            entry
        };
        self.refund_entry(owner, entry);
    }

    /// Would a `CancelQueueBuilding` command be accepted right
    /// now? (TEC-05: the building stands and the index is in
    /// range.)
    pub fn can_cancel_queue_building(&self, building: u32, index: u32) -> bool {
        matches!(self.building_index(building),
            Some(bi) if (index as usize) < self.buildings[bi].queue.len())
    }

    /// Apply a `CancelQueueBuilding` command (TEC-05): the Town
    /// Center rule against a placed building's queue and owner.
    fn issue_cancel_queue_building(&mut self, building: u32, index: u32) {
        if !self.can_cancel_queue_building(building, index) {
            return;
        }
        let bi = self.building_index(building).expect("validated");
        let (owner, entry) = {
            let b = &mut self.buildings[bi];
            let entry = b.queue.remove(index as usize);
            if index == 0 {
                b.train_progress = 0;
            }
            (b.owner, entry)
        };
        self.refund_entry(owner, entry);
    }

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
            e.order_from_queue = false;
            e.clear_combat();
            e.task = Task::Returning;
            e.task_dropoff = Some(target);
        }
        // If the drop-off cannot be steered to at all, the
        // Returning step resolves the stop itself next tick.
        self.steer_to_dropoff(idx, owner, target);
    }

    // --- AutoExplore (SEL-17) ------------------------------------
    // Any Scout Cavalry of either player may hand its movement
    // to a frontier walker: it rides to the nearest unexplored
    // cell of its OWNER's fog grid, re-picking on arrival (or at
    // the fog cadence when its target was explored by other
    // means). Everything here is a pure function of hashed state
    // — the fog grid, positions, the tick counter — so a replay
    // reproduces every ride exactly.

    /// Would a `SetAutoExplore` command be accepted right now?
    /// The entity must be a living Scout Cavalry owned by a
    /// player (SEL-17: eligibility is every scout — starting and
    /// trained alike — per current DE, update 111772, and
    /// Claudio's 2026-10-20 ruling "tem que ser como no
    /// original"; each scout carries its own flag and target, so
    /// any number may explore independently at once).
    pub fn can_auto_explore(&self, entity: u32) -> bool {
        match self.entity_index(entity) {
            Some(idx) => {
                let e = &self.entities[idx];
                !e.dead && e.kind == EntityKind::ScoutCavalry && is_player_owner(e.owner)
            }
            None => false,
        }
    }

    /// Whether AutoExplore is currently on for an entity (the
    /// client's button state; tests pin the toggle/cancel rules
    /// through this).
    pub fn auto_explore_on(&self, entity: u32) -> bool {
        match self.entity_index(entity) {
            Some(idx) => self.entities[idx].auto_explore,
            None => false,
        }
    }

    /// Apply a `SetAutoExplore` command: validate, then set the
    /// mode. Either way the toggle replaces the scout's current
    /// order and queue (a fresh direct order, SEL-12) and clears
    /// any combat engagement; with the mode off the scout simply
    /// stops where it stands.
    fn issue_auto_explore(&mut self, entity: u32, enabled: bool) {
        if !self.can_auto_explore(entity) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        let e = &mut self.entities[idx];
        e.auto_explore = enabled;
        e.auto_target = None;
        e.order = None;
        e.order_queue.clear();
        e.order_from_queue = false;
        // CMB-10/11: the toggle is a fresh order — it also ends
        // any standing order (a scout can hold a patrol).
        e.attack_move = None;
        e.patrol.clear();
        e.patrol_next = 0;
        e.vel_x = I16F16::ZERO;
        e.vel_y = I16F16::ZERO;
        e.clear_combat();
        e.task = Task::Idle;
        e.task_node = None;
        e.task_building = None;
        e.task_dropoff = None;
    }

    // -- Orders & stances (CMB-09/10/11) --------------------------------
    //
    // Attack-move, patrol, and the four unit stances. Evidence
    // (fetched 2026-10-22): the AoE wiki's "Unit stance" page for
    // the stance behaviours and the Aggressive default; the
    // official Age of Empires forums' attack-move thread ("Wonky
    // Attack Move" — the expected behaviour: enemy units and/or
    // buildings entering the unit's LoS are attacked, then the
    // unit resumes toward its destination) and the wiki's
    // "Hotkey" page (DE's dedicated attack-move key is R; patrol
    // is a panel command) for attack-move; the GameFAQs Age of
    // Kings strategy guide for patrol ("the unit moves back and
    // forth between its current position and the designated
    // point... Way points also work with patrols"). The one
    // number no source pins — how far a Defensive unit chases —
    // is a tuned constant: see combat::DEFENSIVE_LEASH.

    /// Would an `AttackMove` or `Patrol` command be accepted
    /// right now? The entity must be a living military unit owned
    /// by a player, with an attack row — DE gives these orders
    /// (and the stance row) to military units only; villagers
    /// engage through the explicit `Attack` command alone.
    pub fn can_attack_move(&self, entity: u32) -> bool {
        match self.entity_index(entity) {
            Some(idx) => self.combat_orderable(idx),
            None => false,
        }
    }

    /// The index form of `can_attack_move`, shared by the queued
    /// execution paths.
    fn combat_orderable(&self, idx: usize) -> bool {
        let e = &self.entities[idx];
        !e.dead
            && is_player_owner(e.owner)
            && e.kind.is_military()
            && !units::spec(e.kind).attacks.is_empty()
    }

    /// Would a `SetStance` command be accepted right now? A living
    /// military unit owned by a player (CMB-09).
    pub fn can_set_stance(&self, entity: u32) -> bool {
        match self.entity_index(entity) {
            Some(idx) => {
                let e = &self.entities[idx];
                !e.dead && is_player_owner(e.owner) && e.kind.is_military()
            }
            None => false,
        }
    }

    /// The entity's current stance, if it exists (the client's
    /// stance-row state; tests pin the defaults through this).
    pub fn stance_of(&self, entity: u32) -> Option<Stance> {
        self.entity_index(entity).map(|idx| self.entities[idx].stance)
    }

    /// Whether the entity is currently patrolling (CMB-11).
    pub fn patrolling(&self, entity: u32) -> bool {
        match self.entity_index(entity) {
            Some(idx) => !self.entities[idx].patrol.is_empty(),
            None => false,
        }
    }

    /// Whether the entity is currently walking an attack-move
    /// (CMB-10).
    pub fn attack_moving(&self, entity: u32) -> bool {
        match self.entity_index(entity) {
            Some(idx) => self.entities[idx].attack_move.is_some(),
            None => false,
        }
    }

    /// Apply an `AttackMove` command (CMB-10): validate, replace
    /// the unit's order and queue (SEL-12), and install the
    /// standing order.
    fn issue_attack_move(&mut self, entity: u32, x: I16F16, y: I16F16) {
        if !self.can_attack_move(entity) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        self.entities[idx].order_queue.clear();
        self.entities[idx].order_from_queue = false;
        self.apply_attack_move(idx, x, y);
    }

    /// The mechanics of an attack-move order, shared by the
    /// `AttackMove` command and queued-order execution: cancel the
    /// unit's work, combat, AutoExplore and any patrol, walk to
    /// the resolved destination, and remember the destination as
    /// the standing order — the acquisition chain (CMB-08, under
    /// the stance gates of CMB-09) fights whatever enters sight
    /// on the way, and the standing-order step resumes the walk
    /// after each fight. If no move order can be installed the
    /// attack-move is rejected wholesale (no standing order).
    fn apply_attack_move(&mut self, idx: usize, x: I16F16, y: I16F16) {
        if !self.combat_orderable(idx) {
            return;
        }
        {
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
        }
        if let Some(dest) = self.install_move_destination(idx, x, y) {
            self.entities[idx].attack_move = Some(dest);
        }
    }

    /// Apply a `Patrol` command (CMB-11): on a unit that already
    /// patrols, the point is APPENDED to its loop — patrol legs
    /// are chained one order at a time, and the current leg is
    /// never disturbed. Otherwise the command replaces the unit's
    /// order and queue (SEL-12) and starts a fresh loop.
    fn issue_patrol(&mut self, entity: u32, x: I16F16, y: I16F16) {
        if !self.can_attack_move(entity) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        if !self.entities[idx].patrol.is_empty() {
            self.patrol_append(idx, x, y);
            return;
        }
        self.entities[idx].order_queue.clear();
        self.entities[idx].order_from_queue = false;
        self.apply_patrol(idx, x, y);
    }

    /// Append a point to the unit's active patrol loop (clamped
    /// into the world). The point is raw, not snapped: a leg the
    /// steering cannot reach is skipped by the standing-order
    /// step, like a chase target that cannot be reached.
    fn patrol_append(&mut self, idx: usize, x: I16F16, y: I16F16) {
        let p = (clamp_pos(x), clamp_pos(y));
        self.entities[idx].patrol.push(p);
    }

    /// The mechanics of starting a patrol, shared by the `Patrol`
    /// command and queued-order execution: the loop is [where the
    /// unit stands now, the resolved destination], and the unit
    /// walks the destination leg at once. If no move order can be
    /// installed the patrol is rejected wholesale.
    fn apply_patrol(&mut self, idx: usize, x: I16F16, y: I16F16) {
        if !self.combat_orderable(idx) {
            return;
        }
        let start = {
            let e = &self.entities[idx];
            (e.pos_x, e.pos_y)
        };
        {
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
        }
        if let Some(dest) = self.install_move_destination(idx, x, y) {
            let e = &mut self.entities[idx];
            e.patrol = vec![start, dest];
            e.patrol_next = 1;
        }
    }

    /// Apply a `SetStance` command (CMB-09): validate and set.
    /// A stance is a setting, not an order — the unit's order,
    /// queue, target and standing orders all survive it. Setting
    /// Defensive anchors the leash post where the unit stands now;
    /// afterwards the standing-order step moves the post to
    /// wherever the unit last stood idle.
    fn issue_set_stance(&mut self, entity: u32, stance: Stance) {
        if !self.can_set_stance(entity) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        let e = &mut self.entities[idx];
        e.stance = stance;
        if stance == Stance::Defensive {
            e.stance_anchor_x = e.pos_x;
            e.stance_anchor_y = e.pos_y;
        }
    }

    /// The standing-order step (tick phase 5.6, after AutoExplore,
    /// before the AI): attack-move arrival and resume, patrol leg
    /// advance, and the Defensive return-to-post (CMB-09).
    ///
    /// Combat owns the unit while it has a target — the
    /// acquisition chain keeps acquiring under an attack-move or
    /// patrol (the unit fights what it meets), and this step only
    /// resumes the walk once the fight is over. A fight that ended
    /// mid-walk can leave the chase's move order behind (headed at
    /// the dead target's last position); when it does not head at
    /// the standing destination, the walk is re-aimed here.
    fn unit_orders_tick(&mut self) {
        for idx in 0..self.entities.len() {
            // Everything the step needs, copied out — the actions
            // below borrow `self` mutably.
            let (eligible, fighting, order_target, attack_move, patrol_point, patrol_len, stance) = {
                let e = &self.entities[idx];
                let n = e.patrol.len();
                (
                    !e.dead && is_player_owner(e.owner) && e.kind.is_military(),
                    e.attack_target.is_some() || e.attack_building.is_some(),
                    e.order.as_ref().map(|o| (o.target_x, o.target_y)),
                    e.attack_move,
                    if n > 0 {
                        Some(e.patrol[e.patrol_next % n])
                    } else {
                        None
                    },
                    n,
                    e.stance,
                )
            };
            if !eligible {
                continue;
            }
            if let Some((ax, ay)) = attack_move {
                if fighting {
                    continue;
                }
                match order_target {
                    None => {
                        let d = self.distance_to(idx, ax, ay);
                        if d <= I16F16::from_num(1) {
                            // Arrived: the attack-move completes —
                            // the unit holds at its destination.
                            self.entities[idx].attack_move = None;
                        } else if !self.set_move_order(idx, ax, ay) {
                            // Unreachable from here: the standing
                            // order ends, like a failed chase.
                            self.entities[idx].attack_move = None;
                        }
                    }
                    Some(t) => {
                        if t != (ax, ay) {
                            self.set_move_order(idx, ax, ay);
                        }
                    }
                }
                continue;
            }
            if let Some((px, py)) = patrol_point {
                if fighting || order_target.is_some() {
                    continue;
                }
                let d = self.distance_to(idx, px, py);
                if d > I16F16::from_num(1) {
                    // An unreachable leg is skipped, not retried
                    // forever: one A* attempt per tick at most,
                    // mirroring the combat chase's failure rule.
                    self.set_move_order(idx, px, py);
                }
                let e = &mut self.entities[idx];
                e.patrol_next = (e.patrol_next + 1) % patrol_len.max(1);
                continue;
            }
            // Defensive return-to-post (CMB-09): a Defensive unit
            // standing idle away from its post walks back to it —
            // the documented "returning to their original
            // position". The post itself is set when the stance is
            // issued and moves to the destination of each explicit
            // move order (see apply_move); it never drifts to
            // wherever a fight happened to end.
            if stance == Stance::Defensive && order_target.is_none() && !fighting {
                let (ax, ay) = {
                    let e = &self.entities[idx];
                    (e.stance_anchor_x, e.stance_anchor_y)
                };
                if self.distance_to(idx, ax, ay) > I16F16::from_num(2) {
                    self.set_move_order(idx, ax, ay);
                }
            }
        }
    }

    /// The per-tick AutoExplore step (tick phase 5.5): every scout
    /// with the mode on and no ride in progress picks its next
    /// frontier cell. A scout with a combat target is left to the
    /// combat phase — an engagement (however it arose) must not
    /// fight the walker.
    fn auto_explore_tick(&mut self) {
        for idx in 0..self.entities.len() {
            {
                let e = &self.entities[idx];
                if !e.auto_explore || e.dead {
                    continue;
                }
            }
            if self.entities[idx].attack_target.is_some()
                || self.entities[idx].attack_building.is_some()
            {
                continue;
            }
            let slot = match player_slot(self.entities[idx].owner) {
                Some(s) => s,
                None => continue,
            };
            // Cadence re-pick: if the target cell has been explored
            // since it was picked (the scout's own LoS usually gets
            // there first), drop the ride at the fog recompute
            // cadence and re-pick from where the scout stands.
            if self.tick % FOG_RECOMPUTE_INTERVAL == 0 {
                if let Some((tx, ty)) = self.entities[idx].auto_target {
                    if self.fog[slot][(ty * GRID_DIM + tx) as usize] != 0 {
                        let e = &mut self.entities[idx];
                        e.auto_target = None;
                        e.order = None;
                        e.vel_x = I16F16::ZERO;
                        e.vel_y = I16F16::ZERO;
                    }
                }
            }
            if self.entities[idx].order.is_some() {
                continue;
            }
            self.auto_explore_pick(idx, slot);
        }
    }

    /// Pick the exploration target for scout `idx`: the nearest
    /// unexplored cell of its owner's fog grid (squared cell
    /// distance; ties go to the lowest linear cell index — the
    /// scan is row-major, so the choice is a pure function of the
    /// grid and the scout's cell). Unreachable or blocked cells
    /// are skipped in that same order (at most 32 candidates are
    /// path-tested per pick); if none accepts a path the scout
    /// stands and the pick retries next tick.
    fn auto_explore_pick(&mut self, idx: usize, slot: usize) {
        self.entities[idx].auto_target = None;
        let (px, py) = (self.entities[idx].pos_x, self.entities[idx].pos_y);
        let (scx, scy) = self.grid.world_to_cell(px, py);
        let mut cands: Vec<(i64, u32)> = Vec::new();
        for cy in 0..GRID_DIM {
            for cx in 0..GRID_DIM {
                if self.fog[slot][(cy * GRID_DIM + cx) as usize] == 0 {
                    let dx = cx as i64 - scx as i64;
                    let dy = cy as i64 - scy as i64;
                    cands.push((dx * dx + dy * dy, cy * GRID_DIM + cx));
                }
            }
        }
        cands.sort_unstable();
        for &(_, lin) in cands.iter().take(32) {
            let (cx, cy) = (lin % GRID_DIM, lin / GRID_DIM);
            if self.grid.is_blocked(cx, cy) {
                continue;
            }
            let (wx, wy) = self.grid.cell_center(cx, cy);
            if self.set_move_order(idx, wx, wy) {
                self.entities[idx].auto_target = Some((cx, cy));
                return;
            }
        }
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

    /// Would a `Repair` command be accepted right now? (BLD-07.)
    /// The repairer must be a living villager, and the target a
    /// *complete*, *damaged* placed building owned by the
    /// villager's own player — never an enemy's or Gaia's, never
    /// an undamaged/full one, and never one still under
    /// construction (a foundation is resumed via `Build`). The
    /// Town Center, an entity rather than a placed building, is
    /// never a repair target. A pure query mirroring
    /// `issue_repair`'s validation, for UI affordances; the
    /// command path re-validates at apply time.
    pub fn can_repair(&self, entity: u32, building: u32) -> bool {
        let (Some(idx), Some(bi)) = (self.entity_index(entity), self.building_index(building))
        else {
            return false;
        };
        let e = &self.entities[idx];
        if e.kind != EntityKind::Villager || e.dead {
            return false;
        }
        let b = &self.buildings[bi];
        b.complete
            && b.owner == e.owner
            && b.hp < I16F16::from_num(b.kind.max_hp_at(self.age_of(b.owner)))
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
            QueuedOrder::Repair { building } => self.can_repair(entity, building),
            QueuedOrder::AttackMove { .. } => self.can_attack_move(entity),
            QueuedOrder::Patrol { .. } => self.can_attack_move(entity),
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
        self.tc_rally_of(TOWN_CENTER_ID)
    }

    /// A specific Town Center's rally point (M8): the
    /// per-Town-Center form of `tc_rally`, same shape.
    pub fn tc_rally_of(&self, tc: u32) -> Option<(I16F16, I16F16, Option<u32>)> {
        let s = self.tc_state(tc)?;
        s.rally.map(|(x, y)| (x, y, s.rally_node))
    }

    /// A placed building's rally point, same shape as `tc_rally`.
    /// `None` when the building is unknown or its rally is unset.
    pub fn building_rally(&self, building: u32) -> Option<(I16F16, I16F16, Option<u32>)> {
        let bi = self.building_index(building)?;
        let b = &self.buildings[bi];
        b.rally.map(|(x, y)| (x, y, b.rally_node))
    }

    /// The entity's order route as world points, for the client's
    /// waypoint-pennant render (INF-09): the current order's
    /// target — included while the order belongs to a queued
    /// chain (orders still queued behind it, or the order itself
    /// entered execution from the queue: `order_from_queue`),
    /// matching the reference, where a plain order shows no
    /// persistent flag and a queued chain flags every point
    /// including the leg in progress and the last leg — followed
    /// by each queued order's target in FIFO order, and finally
    /// by the legs of an active patrol (CMB-11). Object
    /// orders resolve to their target's current position (node,
    /// entity, or building). Render-only: read-only; the route
    /// itself is never hashed (its inputs are).
    pub fn order_route(&self, entity: u32) -> Vec<(I16F16, I16F16)> {
        let mut out = Vec::new();
        let Some(ei) = self.entity_index(entity) else {
            return out;
        };
        let e = &self.entities[ei];
        if e.dead {
            return out;
        }
        if !e.order_queue.is_empty() || e.order_from_queue {
            if let Some(p) = self.current_order_target(e) {
                out.push(p);
            }
        }
        for q in &e.order_queue {
            if let Some(p) = self.queued_order_target(q) {
                out.push(p);
            }
        }
        // CMB-11: a patrol's legs are the unit's standing route —
        // they flag like a queued chain's points, from the leg it
        // heads to next, all the way around the loop.
        if !e.patrol.is_empty() {
            let n = e.patrol.len();
            for k in 0..n {
                out.push(e.patrol[(e.patrol_next + k) % n]);
            }
        }
        out
    }

    /// The world point the entity's CURRENT order is aimed at, if
    /// it has one. Work assignments resolve to the assignment (the
    /// node / building), not the steering point of the current
    /// leg — a hauling villager's gather order targets its node
    /// on the outbound and the return leg alike.
    fn current_order_target(&self, e: &Entity) -> Option<(I16F16, I16F16)> {
        if let Task::Repair { building } = e.task {
            return self.building_pos(building);
        }
        if let Some(node) = e.task_node {
            if matches!(e.task, Task::ToNode | Task::Gathering | Task::Returning) {
                return self.node_pos(node);
            }
        }
        if let Some(b) = e.task_building {
            return self.building_pos(b);
        }
        if let Some(b) = e.attack_building {
            return self.building_pos(b);
        }
        if let Some(o) = &e.order {
            return Some((o.target_x, o.target_y));
        }
        if let Some(t) = e.attack_target {
            return self.entity_pos(t);
        }
        None
    }

    /// The world point a QUEUED order is aimed at (INF-09), if
    /// its target still exists.
    fn queued_order_target(&self, q: &QueuedOrder) -> Option<(I16F16, I16F16)> {
        match *q {
            QueuedOrder::Move { x, y } => Some((x, y)),
            QueuedOrder::Build { x, y, .. } => Some((x, y)),
            QueuedOrder::Gather { node } => self.node_pos(node),
            QueuedOrder::Attack { target } => self.entity_pos(target),
            QueuedOrder::Repair { building } => self.building_pos(building),
            QueuedOrder::AttackMove { x, y } => Some((x, y)),
            QueuedOrder::Patrol { x, y } => Some((x, y)),
        }
    }

    fn entity_pos(&self, id: u32) -> Option<(I16F16, I16F16)> {
        let e = &self.entities[self.entity_index(id)?];
        if e.dead {
            None
        } else {
            Some((e.pos_x, e.pos_y))
        }
    }

    fn node_pos(&self, id: u32) -> Option<(I16F16, I16F16)> {
        let n = &self.nodes[self.node_index(id)?];
        Some((n.pos_x, n.pos_y))
    }

    fn building_pos(&self, id: u32) -> Option<(I16F16, I16F16)> {
        let b = &self.buildings[self.building_index(id)?];
        Some((b.pos_x, b.pos_y))
    }

    /// Idle villagers (M4, SEL-08): living human villagers with no
    /// work task, no move order, and no attack target — the
    /// definition the rulebook gives ("no work task and no move
    /// order"). Id order. Behind the client's idle-villager key and
    /// indicator (INF-03). Human-only: the AI reads its own idle
    /// state directly, and the client's indicators are the human's.
    pub fn idle_villagers(&self) -> Vec<u32> {
        self.idle_villagers_of(entity::OWNER_PLAYER)
    }

    /// Idle villagers of a specific player (M8): the per-player form
    /// of `idle_villagers`, same definition, any seat.
    pub fn idle_villagers_of(&self, owner: u8) -> Vec<u32> {
        self.entities
            .iter()
            .filter(|e| {
                e.kind == EntityKind::Villager
                    && e.owner == owner
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
        self.idle_military_of(entity::OWNER_PLAYER)
    }

    /// Idle military of a specific player (M8): the per-player form
    /// of `idle_military`, same definition, any seat. A unit under
    /// a standing order (attack-move or patrol, CMB-10/11) is at
    /// work, not idle — between legs it has no move order, so the
    /// standing-order state is part of the test.
    pub fn idle_military_of(&self, owner: u8) -> Vec<u32> {
        self.entities
            .iter()
            .filter(|e| {
                e.kind.is_military()
                    && e.owner == owner
                    && !e.dead
                    && e.order.is_none()
                    && e.attack_target.is_none()
                    && e.attack_building.is_none()
                    && e.attack_move.is_none()
                    && e.patrol.is_empty()
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
                Command::BuildLine {
                    entity,
                    kind,
                    x0,
                    y0,
                    x1,
                    y1,
                } => self.issue_build_line(entity, kind, x0, y0, x1, y1),
                Command::Attack { entity, target } => self.issue_attack(entity, target),
                Command::AttackBuilding { entity, building } => {
                    self.issue_attack_building(entity, building)
                }
                Command::TrainAt { building, kind } => self.issue_train_at(building, kind),
                Command::Queue { entity, order } => self.issue_queue(entity, order),
                Command::SetRally { building, x, y } => self.issue_set_rally(building, x, y),
                Command::SetRallyTc { tc, x, y } => self.issue_set_rally_tc(tc, x, y),
                Command::ResearchTc { tech } => self.issue_research_tc(tech),
                Command::ResearchTcAt { tc, tech } => self.issue_research_tc_at(tc, tech),
                Command::ResearchAt { building, tech } => self.issue_research_at(building, tech),
                Command::Repair { entity, building } => self.issue_repair(entity, building),
                Command::MarketBuy { building, resource } => {
                    self.issue_market_buy(building, resource)
                }
                Command::MarketSell { building, resource } => {
                    self.issue_market_sell(building, resource)
                }
                Command::ForceDeposit { entity } => self.issue_force_deposit(entity),
                Command::Stop { entity } => self.issue_stop(entity),
                Command::DeleteEntity { entity } => self.issue_delete_entity(entity),
                Command::DeleteBuilding { building } => {
                    self.issue_delete_building(building)
                }
                Command::CancelQueueTc { tc, index } => {
                    self.issue_cancel_queue_tc(tc, index)
                }
                Command::CancelQueueBuilding { building, index } => {
                    self.issue_cancel_queue_building(building, index)
                }
                Command::SetAutoExplore { entity, enabled } => {
                    self.issue_auto_explore(entity, enabled)
                }
                Command::AttackMove { entity, x, y } => self.issue_attack_move(entity, x, y),
                Command::Patrol { entity, x, y } => self.issue_patrol(entity, x, y),
                Command::SetStance { entity, stance } => self.issue_set_stance(entity, stance),
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
                    o.arrive,
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

        // 5.5. AutoExplore (SEL-17): a Scout with the mode
        //    on and no ride in progress picks its next frontier cell
        //    from its owner's fog grid. Runs after the queues (a
        //    queued order that just started owns the scout — and
        //    issuing it already cancelled the mode) and before the
        //    AI step, so the order it installs steers next tick like
        //    any other.
        self.auto_explore_tick();

        // 5.6. Standing orders (CMB-10/11): an attack-move walk
        //    resumes after each fight and completes at its
        //    destination, patrol legs advance, and Defensive units
        //    walk back to their post (CMB-09). After the queues
        //    and AutoExplore, before the AI step, like the phase
        //    above.
        self.unit_orders_tick();

        // 6. The scripted opponent decides (M5): on its cadence the
        //    AI reads live state and enqueues ordinary commands into
        //    the same FIFO the human uses (AI-01) — they apply at the
        //    next tick's step 1 like any command. No state is touched
        //    directly, so a match is still just (seed, mode, the
        //    human's commands) and replays bit-identically.
        if matches!(self.mode, MatchMode::AiEasy | MatchMode::AiNormal)
            && self.tick % ai::config(self.mode).cadence == 0
        {
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
    /// Unknown entities and genuinely unreachable targets clear the order —
    /// the entity simply holds position; a target on a blocked cell snaps
    /// to the nearest open cell instead (see `apply_move`). For a villager, `Move` cancels
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
        self.entities[idx].order_from_queue = false;
        self.apply_move(idx, x, y);
    }

    /// The mechanics of a move order, shared by the `Move` command
    /// and by queued-order execution (which must not clear the rest
    /// of the queue behind it).
    ///
    /// A PLAYER move aimed at a BLOCKED cell (a tree, a berry
    /// bush, a building's footprint) snaps to the nearest open
    /// cell: the reference game walks the unit up to the obstacle
    /// instead of ignoring the click. The dropped order was the
    /// "my sheep won't walk" play-test break (Claudio,
    /// 2026-10-19) — a plain move is a captured sheep's ONLY
    /// order, and its clicks land around the TC, houses and
    /// trees. The snap lives in `install_move_destination`, not in
    /// `set_move_order`: the sim's internal steering (hauler
    /// drop-off approaches, gather standing cells, combat chases)
    /// calls that directly and its fail-then-fallback logic
    /// depends on the honest answer.
    fn apply_move(&mut self, idx: usize, x: I16F16, y: I16F16) {
        {
            let e = &mut self.entities[idx];
            e.task = Task::Idle;
            e.task_node = None;
            e.task_building = None;
            e.task_dropoff = None;
            e.clear_combat();
            // SEL-17: a manual order takes the scout back — the
            // AutoExplore walker never survives one.
            e.auto_explore = false;
            e.auto_target = None;
            // CMB-10/11: a plain move also ends any standing order.
            e.attack_move = None;
            e.patrol.clear();
            e.patrol_next = 0;
        }
        // CMB-09: a Defensive unit's post follows its explicit
        // move orders — the post is the resolved destination.
        let installed = self.install_move_destination(idx, x, y);
        if self.entities[idx].stance == Stance::Defensive {
            if let Some((tx, ty)) = installed {
                let e = &mut self.entities[idx];
                e.stance_anchor_x = tx;
                e.stance_anchor_y = ty;
            }
        }
    }

    /// Install a move order toward a player-given destination,
    /// with the blocked-cell snap of `apply_move` (shared by the
    /// attack-move and patrol applies, CMB-10/11, whose stored
    /// destinations must be the RESOLVED points). Returns the
    /// installed order's final target, or `None` when no order
    /// could be installed (the destination is unreachable and not
    /// on a blocked cell).
    fn install_move_destination(
        &mut self,
        idx: usize,
        x: I16F16,
        y: I16F16,
    ) -> Option<(I16F16, I16F16)> {
        let (tx, ty) = (clamp_pos(x), clamp_pos(y));
        if self.set_move_order(idx, tx, ty) {
            return Some((tx, ty));
        }
        let (gx, gy) = self.grid.world_to_cell(tx, ty);
        if self.grid.is_blocked(gx, gy) {
            if let Some(cell) = self.grid.nearest_free_cell(gx, gy, 4) {
                let (cx, cy) = self.grid.cell_center(cell.0, cell.1);
                if self.set_move_order(idx, cx, cy) {
                    return Some((cx, cy));
                }
            }
        }
        None
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
        self.entities[idx].order_from_queue = false;
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
        e.auto_explore = false;
        e.auto_target = None;
        e.order = None;
        e.attack_target = Some(target);
        e.attack_building = None;
        e.strike_tick = None;
        e.attack_engaged = false;
        e.next_attack_tick = 0;
        // CMB-09: an explicit order's provenance — the stance
        // chase gates never apply to it. CMB-10/11: it also ends
        // any standing order.
        e.attack_auto = false;
        e.attack_move = None;
        e.patrol.clear();
        e.patrol_next = 0;
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
        self.entities[idx].order_from_queue = false;
        let e = &mut self.entities[idx];
        e.task = Task::Idle;
        e.task_node = None;
        e.task_building = None;
        e.task_dropoff = None;
        e.auto_explore = false;
        e.auto_target = None;
        e.order = None;
        e.attack_target = None;
        e.attack_building = Some(building);
        e.strike_tick = None;
        e.attack_engaged = false;
        e.next_attack_tick = 0;
        // CMB-09/10/11: explicit-order provenance (see apply_attack).
        e.attack_auto = false;
        e.attack_move = None;
        e.patrol.clear();
        e.patrol_next = 0;
    }

    /// Apply a `Repair` command (BLD-07): validate (same rules as
    /// `can_repair`), cancel the villager's current work, and put
    /// it on repair duty for the building. No cost is charged up
    /// front — the wood drip runs per tick of actual repair in
    /// `repair_tick`. A non-queued `Repair` also clears the
    /// villager's order queue (M4, SEL-12: replace semantics); a
    /// rejected one changes nothing at all.
    fn issue_repair(&mut self, entity: u32, building: u32) {
        if !self.can_repair(entity, building) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        self.entities[idx].order_queue.clear();
        self.entities[idx].order_from_queue = false;
        self.apply_repair(idx, building);
    }

    /// The mechanics of a repair order, shared by the `Repair`
    /// command and by queued-order execution (which must not clear
    /// the rest of the queue behind it). Returns whether the order
    /// took effect (repairer assigned).
    fn apply_repair(&mut self, idx: usize, building: u32) -> bool {
        let entity = self.entities[idx].id;
        if !self.can_repair(entity, building) {
            return false;
        }
        self.assign_repairer(idx, building);
        true
    }

    /// Put villager `idx` on repair duty for building `bid`
    /// (BLD-07): the mirror of `assign_builder` — the villager
    /// travels to the best ring standing spot, and its work step
    /// takes it from travel to in-range work. The repair
    /// assignment lives in the `Task::Repair` phase itself, so the
    /// gather and construction assignments are both cleared (a
    /// fresh order of any work kind clears the others, as with
    /// `Build`/`Gather`).
    fn assign_repairer(&mut self, idx: usize, bid: u32) {
        let (ax, ay, size) = match self.building_index(bid) {
            Some(bi) => {
                let b = &self.buildings[bi];
                (b.cell_x, b.cell_y, b.kind.footprint_size())
            }
            None => return,
        };
        {
            let e = &mut self.entities[idx];
            e.task = Task::Repair { building: bid };
            e.task_node = None;
            e.task_building = None;
            e.task_dropoff = None;
        }
        match self.approach_point(idx, ax, ay, size) {
            Some((px, py)) => {
                self.set_move_order(idx, px, py);
            }
            None => {
                // No standing spot right now (crowded site): the
                // work step probes again — and gives up to Idle if
                // the site stays unreachable (mirrors
                // `assign_builder`).
                let e = &mut self.entities[idx];
                e.order = None;
                e.vel_x = I16F16::ZERO;
                e.vel_y = I16F16::ZERO;
            }
        }
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
        self.entities[idx].order_from_queue = false;
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
        // SEL-17: a gather order is a manual order (defensive —
        // only villagers gather, and a villager never carries the
        // AutoExplore flag; the cancel keeps the rule total).
        self.entities[idx].auto_explore = false;
        self.entities[idx].auto_target = None;
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
    /// (MAP-04), so the order targets a standing point beside it
    /// instead: the reachable ring cell with the shortest path
    /// (ties break in fixed ring order — the search is bounded at
    /// the tree's 8 neighbors, once per order, never per tick),
    /// and within that cell the point at the trunk (see
    /// `tree_stand_point`) — DE's cutter works at the tree tile's
    /// edge, not from the neighboring tile's center (Claudio's
    /// live report, 2026-10-24: from the cell center he stood a
    /// full tile from the trunk, "muito longe").
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
                let stand = tree_stand_point(nx, ny, center.0, center.1);
                if let Some(wps) = self.grid.find_path(from, stand) {
                    let better = match &best {
                        None => true,
                        Some((len, _, _)) => wps.len() < *len,
                    };
                    if better {
                        best = Some((wps.len(), stand, wps));
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
                    // A tree order settles AT its stand point (see
                    // the ToNode arrival rule in `work_tick`): the
                    // tight arrival radius stops steering from
                    // completing the order a full generic radius
                    // short of the trunk.
                    arrive: steer::TREE_ARRIVE,
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
            self.entities[idx].order_from_queue = false;
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
                match self.place_building(idx, kind, cell_x, cell_y) {
                    Some(id) => {
                        self.assign_builder(idx, id);
                        true
                    }
                    None => false,
                }
            }
        }
    }

    /// Place one validated foundation for builder `idx`: charge
    /// the builder's owner the kind's wood cost (and its stone
    /// cost, for the stone-cost kinds), register the
    /// building (blocking its footprint cells when the kind is
    /// solid), and return its id. The caller assigns the builder.
    /// Shared by `apply_build` (single placement) and
    /// `apply_build_line` (one call per segment). Validation is
    /// the caller's job (`validate_build`); the spend re-checks
    /// anyway so a failure can never place a free building.
    fn place_building(
        &mut self,
        idx: usize,
        kind: BuildingKind,
        cell_x: u32,
        cell_y: u32,
    ) -> Option<u32> {
        let builder_owner = self.entities[idx].owner;
        // Stone-cost kinds (BLD-18's tower and outpost; the Town
        // Center's 100 stone, Castle Age P1): the stone is
        // re-checked BEFORE either spend lands, so a failure can
        // never take the wood and refuse the building (the P1
        // inline placement's guarantee, carried into the shared
        // path).
        if self.stockpile_of(builder_owner).amount(ResourceKind::Stone) < kind.stone_cost() {
            return None;
        }
        if !self
            .stockpile_mut(builder_owner)
            .spend(ResourceKind::Wood, kind.wood_cost())
        {
            return None;
        }
        // Stone-cost kinds spend their stone too (BLD-18's tower
        // and outpost; the Town Center's 100, Castle P1) — if the
        // stone spend ever failed after the wood went through, the
        // wood is refunded (the re-check above and validation both
        // proved the stone, so this is belt-and-braces; kinds
        // without a stone cost skip it and behave exactly as
        // before).
        if kind.stone_cost() > I16F16::ZERO
            && !self
                .stockpile_mut(builder_owner)
                .spend(ResourceKind::Stone, kind.stone_cost())
        {
            self.stockpile_mut(builder_owner)
                .add(ResourceKind::Wood, kind.wood_cost());
            return None;
        }
        let id = self.next_building_id;
        self.next_building_id += 1;
        let (pos_x, pos_y) = build::footprint_center(cell_x, cell_y, kind.footprint_size());
        let building = Building {
            id,
            kind,
            owner: builder_owner,
            // BLD-05: a foundation starts at the foot of
            // the construction-HP line (1 HP), not at full
            // HP; the construction phase raises it with
            // progress from here.
            hp: build::construction_hp_ceiling(
                kind.max_hp_at(self.age_of(builder_owner)),
                0,
                kind.build_thirds(),
            ),
            cell_x,
            cell_y,
            pos_x,
            pos_y,
            progress_thirds: 0,
            repair_hp_remainder: 0,
            repair_cost_remainder: 0,
            repair_stone_remainder: 0,
            complete: false,
            node_id: None,
            queue: Vec::new(),
            train_progress: 0,
            rally: None,
            rally_node: None,
            weapon_target: None,
            weapon_target_building: None,
            weapon_strike_tick: None,
            weapon_next_tick: 0,
            weapon_engaged: false,
        };
        if kind.blocks_footprint() && !kind.is_wall() {
            for (cx, cy) in building.footprint_cells() {
                self.grid.set_dynamic(cx, cy, true);
            }
        }
        // BLD-20: a WALL foundation does not block its cell yet —
        // in DE an untouched foundation is walkable ground (that
        // is why quickwalling needs each segment "built up
        // slightly" before it blocks; dossier §3). The cell joins
        // the blocking grid when construction first advances the
        // segment (`construction_tick`) and is guaranteed blocked
        // at completion (`complete_building`).
        self.buildings.push(building);
        Some(id)
    }

    /// Apply a `BuildLine` command (BLD-13, BLD-20): expand the
    /// dragged line deterministically (`build::wall_line_cells`)
    /// and run every segment through the ordinary build
    /// validation. The segments that take effect stand as
    /// foundations at once; this villager's primary target is the
    /// first segment **no other villager is already building**
    /// (BLD-20: DE tasks every selected villager to the line and
    /// they distribute along it — the client issues the line to
    /// each selected villager, and this rule keeps them from all
    /// piling onto segment one), and every other segment becomes
    /// a queued `Build` order behind it, so a lone villager walks
    /// the line raising segment after segment in drag order.
    /// Segments that fail validation are skipped — a dragged line
    /// over a tree, a node, a Town Center's reserved ground or
    /// past the stockpile leaves gaps, never partial charges.
    /// Like `issue_build`, a line that takes effect replaces the
    /// villager's order queue; a line where nothing takes effect
    /// changes nothing at all.
    fn issue_build_line(
        &mut self,
        entity: u32,
        kind: BuildingKind,
        x0: I16F16,
        y0: I16F16,
        x1: I16F16,
        y1: I16F16,
    ) {
        if !kind.is_wall() {
            return;
        }
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return,
        };
        if self.entities[idx].kind != EntityKind::Villager || self.entities[idx].dead {
            return;
        }
        let (ax, ay) = self.grid.world_to_cell(clamp_pos(x0), clamp_pos(y0));
        let (bx, by) = self.grid.world_to_cell(clamp_pos(x1), clamp_pos(y1));
        // Every segment that took effect, in drag order: its
        // building id plus the cell-center point a queued Build
        // order names. The primary target is chosen after the
        // walk (distribution, see the doc above).
        let mut effected: Vec<(u32, I16F16, I16F16)> = Vec::new();
        for (cx, cy) in build::wall_line_cells(ax, ay, bx, by) {
            // A point strictly inside the segment's cell — the
            // same anchor convention the client's placement uses.
            let (wx, wy) = build::footprint_center(cx, cy, 1);
            match self.validate_build(entity, kind, wx, wy) {
                BuildVerdict::Reject => {}
                BuildVerdict::Resume(bid) => {
                    effected.push((bid, wx, wy));
                }
                BuildVerdict::Place(px, py) => {
                    if let Some(id) = self.place_building(idx, kind, px, py) {
                        effected.push((id, wx, wy));
                    }
                }
            }
        }
        if !effected.is_empty() {
            // Distribution: the first segment no OTHER villager
            // is travelling to or building right now; if every
            // segment is claimed, share the first (the ordinary
            // multi-builder rate then applies to it).
            let claimed = |bid: u32| {
                self.entities.iter().any(|e| {
                    e.id != entity
                        && !e.dead
                        && e.task_building == Some(bid)
                        && matches!(e.task, Task::ToBuild | Task::Building)
                })
            };
            let primary = effected
                .iter()
                .position(|(bid, _, _)| !claimed(*bid))
                .unwrap_or(0);
            let (primary_bid, _, _) = effected[primary];
            self.assign_builder(idx, primary_bid);
            let followups: Vec<QueuedOrder> = effected
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != primary)
                .map(|(_, (_, wx, wy))| QueuedOrder::Build { kind, x: *wx, y: *wy })
                .collect();
            // Replace semantics (the `issue_build` contract): the
            // line clears whatever the villager had queued, then
            // the line's own remaining segments line up behind the
            // segment under construction.
            self.entities[idx].order_queue.clear();
            self.entities[idx].order_queue.extend(followups);
            self.entities[idx].order_from_queue = false;
        }
    }

    /// Would a `BuildLine` command take effect right now — at
    /// least one segment of the dragged line would be placed or
    /// resumed? A pure query mirroring `issue_build_line`'s
    /// per-segment validation (each segment's own verdict already
    /// includes the stockpile check for one segment; a line the
    /// stockpile cannot even start reports rejection, while
    /// segments past the budget are the applier's truncation,
    /// not a rejection).
    pub fn can_build_line(
        &self,
        entity: u32,
        kind: BuildingKind,
        x0: I16F16,
        y0: I16F16,
        x1: I16F16,
        y1: I16F16,
    ) -> bool {
        if !kind.is_wall() {
            return false;
        }
        let idx = match self.entity_index(entity) {
            Some(i) => i,
            None => return false,
        };
        if self.entities[idx].kind != EntityKind::Villager || self.entities[idx].dead {
            return false;
        }
        let owner = self.entities[idx].owner;
        let budget = self.stockpile_of(owner).amount(ResourceKind::Wood);
        let budget_stone = self.stockpile_of(owner).amount(ResourceKind::Stone);
        let (ax, ay) = self.grid.world_to_cell(clamp_pos(x0), clamp_pos(y0));
        let (bx, by) = self.grid.world_to_cell(clamp_pos(x1), clamp_pos(y1));
        for (cx, cy) in build::wall_line_cells(ax, ay, bx, by) {
            let (wx, wy) = build::footprint_center(cx, cy, 1);
            match self.validate_build(entity, kind, wx, wy) {
                BuildVerdict::Resume(_) => return true,
                BuildVerdict::Place(_, _) => {
                    if budget >= kind.wood_cost() && budget_stone >= kind.stone_cost() {
                        return true;
                    }
                }
                BuildVerdict::Reject => {}
            }
        }
        false
    }

    /// Apply a `Queue` command (M4, SEL-12): append the order to the
    /// entity's queue. If the entity is idle the queue-advance step
    /// later this same tick starts it — a queued order for an idle
    /// unit behaves like a plain order with no visible delay.
    fn issue_queue(&mut self, entity: u32, order: QueuedOrder) {
        // CMB-11: a patrol point for a unit that already patrols
        // appends to its loop NOW — the loop never ends, so a
        // patrol point queued behind it could never execute. (The
        // append validates on its own: combat-orderable, not the
        // queue cap — nothing is being queued.)
        if let QueuedOrder::Patrol { x, y } = order {
            if self.can_attack_move(entity)
                && self
                    .entity_index(entity)
                    .map(|idx| !self.entities[idx].patrol.is_empty())
                    .unwrap_or(false)
            {
                let idx = self.entity_index(entity).expect("validated");
                self.entities[idx].auto_explore = false;
                self.entities[idx].auto_target = None;
                self.patrol_append(idx, x, y);
                return;
            }
        }
        if !self.can_queue(entity, order) {
            return;
        }
        let idx = self.entity_index(entity).expect("validated");
        // SEL-17: queueing an order is a manual order too — it
        // cancels AutoExplore, so the queue can actually advance
        // (the walker would otherwise keep the scout busy forever).
        self.entities[idx].auto_explore = false;
        self.entities[idx].auto_target = None;
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
                    // CMB-10/11: a standing order owns the unit —
                    // its queue waits behind the attack-move (which
                    // completes) and behind a patrol (which never
                    // does, by design).
                    || e.attack_move.is_some()
                    || !e.patrol.is_empty()
                {
                    continue;
                }
            }
            while !self.entities[idx].order_queue.is_empty() {
                let order = self.entities[idx].order_queue.remove(0);
                if self.execute_queued(idx, order) {
                    // INF-09: the order now executing came from
                    // the queue — its waypoint keeps its flag
                    // until it completes, even once the queue
                    // behind it is empty.
                    self.entities[idx].order_from_queue = true;
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
            QueuedOrder::Repair { building } => self.apply_repair(idx, building),
            QueuedOrder::AttackMove { x, y } => {
                if !self.combat_orderable(idx) {
                    return false;
                }
                self.apply_attack_move(idx, x, y);
                true
            }
            QueuedOrder::Patrol { x, y } => {
                if !self.combat_orderable(idx) {
                    return false;
                }
                // Defensive twin of the issue_queue rule: a patrol
                // that started since the order was queued takes the
                // point as a new leg instead of restarting.
                if !self.entities[idx].patrol.is_empty() {
                    self.patrol_append(idx, x, y);
                } else {
                    self.apply_patrol(idx, x, y);
                }
                true
            }
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

    /// Apply a `SetRallyTc` command (M8): the `None` case of
    /// `issue_set_rally` for an arbitrary Town Center — the rally
    /// lands on that Town Center's own state, whichever seat owns
    /// it. A missing or dead Town Center rejects silently.
    fn issue_set_rally_tc(&mut self, tc: u32, x: I16F16, y: I16F16) {
        if !self.can_set_rally_tc(tc) {
            return;
        }
        let (x, y) = (clamp_pos(x), clamp_pos(y));
        let node = self.rally_snap_node(x, y);
        if let Some(s) = self.tc_states.iter_mut().find(|s| s.tc_id == tc) {
            s.rally = Some((x, y));
            s.rally_node = node;
        }
    }

    /// Would a `SetRallyTc` command be accepted right now? A Town
    /// Center always holds a rally point (the `can_set_rally(None)`
    /// rule), so this only asks that the Town Center exists and
    /// lives.
    pub fn can_set_rally_tc(&self, tc: u32) -> bool {
        match self.entity_index(tc) {
            Some(i) => {
                self.entities[i].kind == EntityKind::TownCenter && !self.entities[i].dead
            }
            None => false,
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
        // building under construction belongs to its placer). The
        // order's cell may be ANY cell of the standing footprint,
        // not only its anchor (2026-10-22, SEL-11: a right-click
        // smart order lands wherever on the foundation the player
        // clicked; the placement ghost always names the anchor and
        // resolves identically).
        for b in &self.buildings {
            let bsize = b.kind.footprint_size();
            if !b.complete
                && b.kind == kind
                && b.owner == self.entities[idx].owner
                && ax >= b.cell_x
                && ax < b.cell_x + bsize
                && ay >= b.cell_y
                && ay < b.cell_y + bsize
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
        // Town Center footprint reservation (BLD-16, 2026-10-19): a
        // living Town Center's 4x4 is PASSABLE to units — the TC is
        // an entity and never enters the blocking grid — but its
        // ground is reserved against building placement, both
        // players', the way the reference game reserves the TC's
        // tiles for construction. The 4x4 is centered on the TC's
        // cell (anchor = its cell − 1 on each axis), the sim half of
        // the 4x4 art contract (half-extent 8 world units). At most
        // two TCs stand in a match, so 32 slots always suffice.
        let mut tc_reserved = [(0u32, 0u32); 32];
        let mut tc_reserved_len = 0usize;
        for e in &self.entities {
            if e.kind == EntityKind::TownCenter && !e.dead {
                let (tx, ty) = self.grid.world_to_cell(e.pos_x, e.pos_y);
                for dy in -1i32..=2 {
                    for dx in -1i32..=2 {
                        let (rx, ry) = (tx as i32 + dx, ty as i32 + dy);
                        if rx >= 0 && ry >= 0 && tc_reserved_len < tc_reserved.len() {
                            tc_reserved[tc_reserved_len] = (rx as u32, ry as u32);
                            tc_reserved_len += 1;
                        }
                    }
                }
            }
        }
        let cells = build::footprint_cells(ax, ay, size);
        for &(cx, cy) in &cells {
            // Walls and already-blocking buildings.
            if self.grid.is_blocked(cx, cy) {
                return BuildVerdict::Reject;
            }
            // Any living Town Center's reserved 4x4 cell (BLD-16:
            // both players' — supersedes the pre-2026-10-19 check,
            // which reserved only the TC's single center cell).
            if tc_reserved[..tc_reserved_len].contains(&(cx, cy)) {
                return BuildVerdict::Reject;
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
            // WALLS are exempt too (BLD-20, walls dossier §3): DE
            // placement never checks units — a wall line dragged
            // through milling villagers and sheep founds every
            // segment, and `complete_building`'s displacement
            // cure moves anyone standing on a segment when it
            // completes, so the entombment the check guards
            // against cannot persist for a 1×1 wall either. The
            // exemption is what makes the dragged line the wall:
            // no silent holes where a unit happened to stand.
            if kind != BuildingKind::Farm && !kind.is_wall() {
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
        // The cost must be payable at placement — by the *builder's*
        // owner (M5: each player builds from their own stockpile).
        // The stone half likewise, for the stone-cost kinds
        // (BLD-18's tower and outpost; the Town Center, Castle P1).
        if self.stockpile_of(builder_owner).amount(ResourceKind::Wood) < kind.wood_cost() {
            return BuildVerdict::Reject;
        }
        if self.stockpile_of(builder_owner).amount(ResourceKind::Stone) < kind.stone_cost() {
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
            // A 1×1 site's builder must stand BESIDE his segment
            // (SMALL_BUILD_RANGE, 5.8): targeting the ring cell's
            // center livelocked a diagonal approach — steering
            // completes the order up to 1.0 short of the center,
            // stranding him up to ≈6.66 from the segment's center,
            // outside build range; the re-issued order then
            // completed instantly, forever (the stone-line stall
            // the 2026-10-24 walls tests caught). So a 1×1 stand
            // point is pulled in along the ray from the site's
            // center to 4.6 units: still inside the ring cell
            // (which spans 2.0–6.0 orthogonally, 2.83–8.49
            // diagonally), and his walk crosses into build range
            // before steering can complete the order.
            let target = if size == 1 {
                let site = self.grid.cell_center(ax, ay);
                let (dx, dy) = (center.0 - site.0, center.1 - site.1);
                let len = steer::vec_len(dx, dy);
                if len > I16F16::ZERO {
                    let stand = I16F16::from_bits(301_466); // ≈4.6
                    (site.0 + dx * stand / len, site.1 + dy * stand / len)
                } else {
                    center
                }
            } else {
                center
            };
            if self.grid.find_path(from, target).is_some() {
                return Some(target);
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
                arrive: steer::WAYPOINT_REACH,
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
        // head accrues progress; since the HOUSED front
        // (2026-10-27) enqueue is refused at the cap, a *unit*
        // head can only stall when the cap has *dropped* below
        // the living population (a House or Town Center razed
        // while units were queued): the queue then keeps its
        // place and its progress and resumes the tick housing
        // frees up, so the cap can never be exceeded. A *research*
        // head accrues regardless — population gates units, not
        // ideas. Completion spawns only while there is room.
        // (M3: the room check counts every living
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
        self.repair_tick();
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
                // tree, so a woodcutter arrives at his stand point
                // beside it (see `tree_stand_point`): 2.5 units
                // from the trunk orthogonally — inside the gather
                // range — and ≈3.33 at a diagonal corner, beyond
                // it. For trees, arrival certifies when he stands
                // in a ring cell AND within HIS stand's distance +
                // the tree order's arrival radius
                // (`steer::TREE_ARRIVE`, 0.5): 3.0 from an
                // orthogonal cell, ≈3.83 from a diagonal one. His
                // order carries that same tight radius precisely
                // so steering cannot complete it earlier, a full
                // generic radius short of the stand — with the
                // generic `WAYPOINT_REACH` the order completed at
                // up to ~4.3 from the trunk and he chopped from
                // there (Claudio's live report, 2026-10-24: "ele
                // fica muito longe"). The bounded completion
                // branch (order done, ≤4.4) covers only the
                // crowded case, where a crewmate's separation
                // shove holds him slightly off the stand point;
                // clean approaches flip on the distance branch
                // first. Mere ring-cell *entry* must not certify
                // (the pre-2026-10-24 rule): crossing into the far
                // edge of a neighboring cell stops the cutter up
                // to ~1.5 tiles from the trunk.
                let (beside_tree, diag_cell) = {
                    let ni = self.node_index(node_id).expect("live node checked");
                    if self.nodes[ni].kind == NodeKind::Tree {
                        let (vx, vy) = self.grid.world_to_cell(
                            self.entities[idx].pos_x,
                            self.entities[idx].pos_y,
                        );
                        let (tx, ty) = self.grid.world_to_cell(nx, ny);
                        let (adx, ady) = (vx.abs_diff(tx), vy.abs_diff(ty));
                        (adx <= 1 && ady <= 1, adx == 1 && ady == 1)
                    } else {
                        (false, false)
                    }
                };
                let stand_limit = if diag_cell {
                    I16F16::from_bits(251_000) // ≈3.83 = diagonal stand 3.33 + TREE_ARRIVE
                } else {
                    I16F16::from_bits(196_608) // 3.0 = orthogonal stand 2.5 + TREE_ARRIVE
                };
                let at_tree_stand = beside_tree
                    && (d <= stand_limit
                        || (self.entities[idx].order.is_none()
                            && d <= I16F16::from_bits(288_358))); // 4.4, crowded case only
                if d <= GATHER_RANGE || at_tree_stand {
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
                // The gather rate is the node's base rate patched
                // by the gatherer's owner's economy technologies
                // (TEC-06 percentage patches, applied at lookup):
                // Double-Bit Axe on trees, Gold / Stone Mining on
                // their mines (tech::patched_gather_rate).
                let owner_techs = player_slot(self.entities[idx].owner)
                    .map(|s| self.players[s].techs)
                    .unwrap_or(0);
                let rate = crate::tech::patched_gather_rate(self.nodes[ni].kind, owner_techs);
                // Carry capacity is the node's, not a global (ECO-03):
                // 35 from a hunt carcass, 10 elsewhere — scaled by
                // the gatherer's owner's Wheelbarrow (13 / 44) when
                // researched. A villager arriving already fuller
                // than this node's cap (it hauled meat, then was
                // re-tasked) has negative room: `take` clamps to
                // nothing and the full check below sends it home to
                // deposit first.
                let wheelbarrow = owner_techs & TechId::Wheelbarrow.bit() != 0;
                let hand_cart = owner_techs & TechId::HandCart.bit() != 0;
                let cap = self.nodes[ni]
                    .kind
                    .carry_cap_tiered(wheelbarrow, hand_cart);
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
            Task::Repair { building: bid } => {
                let bi = match self.building_index(bid) {
                    Some(bi) => bi,
                    None => {
                        // The building is gone (destroyed while
                        // the repairer worked or travelled — the
                        // destruction sweep also clears repairers;
                        // this covers every other removal path).
                        self.stop_task(idx);
                        return;
                    }
                };
                let (complete, hp, max_hp, bx, by, ax, ay, range, size) = {
                    let b = &self.buildings[bi];
                    (
                        b.complete,
                        b.hp,
                        I16F16::from_num(b.kind.max_hp_at(self.age_of(b.owner))),
                        b.pos_x,
                        b.pos_y,
                        b.cell_x,
                        b.cell_y,
                        b.kind.build_range(),
                        b.kind.footprint_size(),
                    )
                };
                if !complete || hp >= max_hp {
                    // Not repairable after all, or the repair
                    // finished (another repairer's tick can land
                    // the final HP before this step runs): the job
                    // is done — stand down, mirroring how a
                    // builder leaves a finished site.
                    self.stop_task(idx);
                    return;
                }
                let d = self.distance_to(idx, bx, by);
                if d <= range {
                    // In range: stand and work. The HP restore
                    // itself is applied by `repair_tick`, which
                    // counts the in-range repairers each tick.
                    let e = &mut self.entities[idx];
                    e.order = None;
                    e.vel_x = I16F16::ZERO;
                    e.vel_y = I16F16::ZERO;
                } else if self.entities[idx].order.is_none() {
                    match self.approach_point(idx, ax, ay, size) {
                        Some((px, py)) => {
                            self.set_move_order(idx, px, py);
                        }
                        // No standable ring cell is reachable:
                        // give up and idle instead of probing the
                        // ring every tick (mirrors `ToBuild`).
                        None => self.stop_task(idx),
                    }
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
        // BLD-20: wall foundations whose construction starts this
        // tick — their cells join the blocking grid now (an
        // untouched wall foundation is walkable; a started one
        // blocks, the DE quickwall rule; dossier §3).
        let mut started_walls: Vec<(u32, u32)> = Vec::new();
        for (bi, &n) in counts.iter().enumerate() {
            if n == 0 || self.buildings[bi].complete {
                continue;
            }
            // BLD-05: HP grows with construction — the ceiling
            // rises along the proportional line as progress accrues,
            // and the standing HP rises by the same amount (damage
            // taken mid-build persists: HP stays exactly as far
            // below the ceiling as the damage left it).
            let (kind, owner, id) = {
                let b = &self.buildings[bi];
                (b.kind, b.owner, b.id)
            };
            let max_hp = kind.max_hp_at(self.age_of(owner));
            let total = kind.build_thirds();
            let b = &mut self.buildings[bi];
            let ceiling_before = build::construction_hp_ceiling(max_hp, b.progress_thirds, total);
            if b.progress_thirds == 0 && kind.is_wall() {
                started_walls.push((b.cell_x, b.cell_y));
            }
            b.progress_thirds += build::build_rate_thirds(n);
            let ceiling_after = build::construction_hp_ceiling(max_hp, b.progress_thirds, total);
            if ceiling_after > ceiling_before {
                b.hp = (b.hp + (ceiling_after - ceiling_before)).min(ceiling_after);
            }
            if b.progress_thirds >= total {
                completed.push(id);
            }
        }
        for (cx, cy) in started_walls {
            self.grid.set_dynamic(cx, cy, true);
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

    /// The repair phase of a tick (BLD-07): runs immediately after
    /// the construction phase, under the same ordering discipline.
    /// Every complete, damaged building with at least one repairer
    /// in range gains HP at the documented flat rate —
    /// `build::repair_rate_hp_per_min(n)` for n in-range repairers
    /// (750 HP/min for the first, +375 HP/min per additional),
    /// independent of the building's build time or max HP — and
    /// pays for the HP actually restored as a continuous wood drip
    /// totaling `REPAIR_COST_FRACTION` (50%) of the build cost over
    /// a full 0→100% repair, charged to the building owner's
    /// stockpile. Both the HP restore and the drip accumulate
    /// fractionally on the building (fixed-point remainders), so
    /// the totals hold exactly over the whole repair. If the owner
    /// cannot cover a tick's drip, that tick restores nothing and
    /// the building's repairers stop and go idle — the documented
    /// DE behavior ("if resources run out, Villagers stop repairing
    /// and stand idle"). A building reaching full HP likewise
    /// releases all its repairers, mirroring construction
    /// completion. The repairer count is taken fresh each tick, so
    /// repairers joining or leaving simply change the pace.
    fn repair_tick(&mut self) {
        // Count the in-range repairers of each building, in
        // building order — "in range" is the construction range
        // test (distance to the building's center within its kind's
        // build range), the same test the work step maintains.
        let mut counts: Vec<u32> = vec![0; self.buildings.len()];
        for (idx, e) in self.entities.iter().enumerate() {
            if e.dead || e.kind != EntityKind::Villager {
                continue;
            }
            if let Task::Repair { building: bid } = e.task {
                if let Some(bi) = self.building_index(bid) {
                    let b = &self.buildings[bi];
                    if b.complete
                        && self.distance_to(idx, b.pos_x, b.pos_y) <= b.kind.build_range()
                    {
                        counts[bi] += 1;
                    }
                }
            }
        }
        let mut finished: Vec<u32> = Vec::new();
        let mut unfunded: Vec<u32> = Vec::new();
        for (bi, &n) in counts.iter().enumerate() {
            if n == 0 {
                continue;
            }
            let (owner, kind) = (self.buildings[bi].owner, self.buildings[bi].kind);
            let max_hp_bits = (kind.max_hp_at(self.age_of(owner)) as i64) << 16;
            let hp_bits = self.buildings[bi].hp.to_bits() as i64;
            let missing = max_hp_bits - hp_bits;
            if missing <= 0 {
                finished.push(self.buildings[bi].id);
                continue;
            }
            // HP restore: add this tick's rate (HP bits per
            // minute) to the fractional accumulator; the quotient
            // over 3600 (ticks per minute) is the whole HP bits
            // restored this tick, the remainder carries.
            let acc = self.buildings[bi].repair_hp_remainder
                + build::repair_rate_hp_per_min(n) * 65_536;
            let mut restored = acc / 3600;
            let mut hp_rem = acc % 3600;
            if restored > missing {
                restored = missing;
                hp_rem = 0;
            }
            if restored <= 0 {
                self.buildings[bi].repair_hp_remainder = hp_rem;
                continue;
            }
            // Resource drip for exactly the HP about to be
            // restored: cost × REPAIR_COST_FRACTION × restored /
            // max_hp, in numerator form so the total over a full
            // repair is the fraction of the build cost to < 1 bit.
            // Wood for every kind; stone too for BLD-18's
            // stone-cost kinds (their stone accumulator rides
            // alongside — for wood-only kinds it stays 0 and the
            // stone spend is a no-op of 0).
            let cost_den = max_hp_bits * 65_536;
            let cost_acc = self.buildings[bi].repair_cost_remainder
                + kind.wood_cost().to_bits() as i64
                    * restored
                    * build::REPAIR_COST_FRACTION.to_bits() as i64;
            let drip = cost_acc / cost_den;
            let cost_rem = cost_acc % cost_den;
            let stone_acc = self.buildings[bi].repair_stone_remainder
                + kind.stone_cost().to_bits() as i64
                    * restored
                    * build::REPAIR_COST_FRACTION.to_bits() as i64;
            let stone_drip = stone_acc / cost_den;
            let stone_rem = stone_acc % cost_den;
            if !self
                .stockpile_mut(owner)
                .spend(ResourceKind::Wood, I16F16::from_bits(drip as i32))
            {
                // Cannot cover this tick's drip: nothing is
                // restored, no accumulator moves, and the
                // repairers stop and stand idle (BLD-07).
                unfunded.push(self.buildings[bi].id);
                continue;
            }
            if !self
                .stockpile_mut(owner)
                .spend(ResourceKind::Stone, I16F16::from_bits(stone_drip as i32))
            {
                // The wood went through but the stone did not:
                // refund the wood so the tick is atomic — nothing
                // restored, no accumulator moves (BLD-07's letter).
                self.stockpile_mut(owner)
                    .add(ResourceKind::Wood, I16F16::from_bits(drip as i32));
                unfunded.push(self.buildings[bi].id);
                continue;
            }
            let done = restored == missing;
            let b = &mut self.buildings[bi];
            b.hp = I16F16::from_bits((hp_bits + restored) as i32);
            b.repair_hp_remainder = if done { 0 } else { hp_rem };
            b.repair_cost_remainder = if done { 0 } else { cost_rem };
            b.repair_stone_remainder = if done { 0 } else { stone_rem };
            if done {
                finished.push(b.id);
            }
        }
        for bid in finished {
            self.clear_repairers(bid);
        }
        for bid in unfunded {
            self.clear_repairers(bid);
        }
    }

    /// Release every repairer assigned to building `bid`: the job
    /// is over (the building is full, or the owner can no longer
    /// pay for the repair) — each repairer goes idle where it
    /// stands, mirroring how construction completion stands its
    /// builders down.
    fn clear_repairers(&mut self, bid: u32) {
        for idx in 0..self.entities.len() {
            if self.entities[idx].task == (Task::Repair { building: bid }) {
                self.stop_task(idx);
            }
        }
    }

    /// A Town Center foundation completes (Castle P1, BLD-15's
    /// extension): the construction record converts into a Town
    /// Center *entity* — the TC's one true form in this
    /// architecture (BLD-16) — with its own production state. The
    /// footprint's blocked cells free up (a standing TC never
    /// blocks its ground), the builders stand down, and damage
    /// taken while building persists onto the entity (BLD-05's
    /// standing HP; an undamaged foundation completes at the
    /// line's top, the full 2400). The entity stands on the
    /// center of the footprint's (1,1) cell — the starting Town
    /// Center's convention, under which the BLD-16 reserve of the
    /// entity is exactly the placed 4×4 footprint.
    fn convert_to_town_center(&mut self, bid: u32) {
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
        for idx in 0..self.entities.len() {
            if self.entities[idx].task == Task::Building
                && self.entities[idx].task_building == Some(bid)
            {
                self.stop_task(idx);
            }
        }
        let (px, py) = self.grid.cell_center(b.cell_x + 1, b.cell_y + 1);
        let id = self.entities.len() as u32;
        let mut e = Entity::spawned(id, EntityKind::TownCenter, b.owner, px, py);
        let max_hp = I16F16::from_num(self.stats_for(b.owner, EntityKind::TownCenter).max_hp);
        e.hp = b.hp.min(max_hp);
        self.entities.push(e);
        self.tc_states.push(TcState {
            tc_id: id,
            queue: Vec::new(),
            progress: 0,
            rally: None,
            rally_node: None,
        });
    }

    /// Mark a building complete and apply its function: a Farm becomes
    /// a food node (and its active builders start working it); a House
    /// simply starts counting toward the population cap. A Town
    /// Center foundation converts into the TC entity instead (see
    /// `convert_to_town_center`).
    fn complete_building(&mut self, bid: u32) {
        let bi = match self.building_index(bid) {
            Some(bi) => bi,
            None => return,
        };
        if self.buildings[bi].complete {
            return;
        }
        self.buildings[bi].complete = true;
        // BLD-20: a completed wall segment blocks its cell,
        // unconditionally (a started one already does — this is
        // the guarantee for a segment whose whole construction
        // landed between two blocking updates).
        if self.buildings[bi].kind.is_wall() {
            let (cx, cy) = (self.buildings[bi].cell_x, self.buildings[bi].cell_y);
            self.grid.set_dynamic(cx, cy, true);
        }
        if self.buildings[bi].kind == BuildingKind::TownCenter {
            self.convert_to_town_center(bid);
            return;
        }
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
        // Horse Collar (ECO-14): a farm raised by an owner who
        // holds the tech is stamped with the raised amount
        // (175 + 75) at creation — a stored value, not a live
        // lookup (standing farms are topped up proportionally
        // once, at the research tick, in `apply_research`).
        let farm_owner = self.buildings[bi].owner;
        if player_slot(farm_owner)
            .map(|s| self.players[s].techs & TechId::HorseCollar.bit() != 0)
            .unwrap_or(false)
        {
            self.nodes[nid as usize].amount_left += I16F16::from_num(75);
        }
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
    /// For subsystem-level forensics see `state_hash_parts` (SIM-06).
    pub fn state_hash(&self) -> u64 {
        self.hash_parts().full
    }

    /// SIM-06: the same canonical byte stream as `state_hash`,
    /// with the running hash snapshotted at subsystem boundaries,
    /// so a desync report can name the diverging subsystem instead
    /// of just the tick. The snapshots are pure reads of the FNV
    /// state — not one extra byte enters the stream, so `full`
    /// is bit-identical to the historical `state_hash()` value
    /// (the golden-hash tests pin that identity).
    pub fn state_hash_parts(&self) -> HashParts {
        self.hash_parts()
    }

    fn hash_parts(&self) -> HashParts {
        let mut h = Fnv1a64::new();
        h.write_u64(self.tick);
        let (rng_state, rng_inc) = self.rng.state_words();
        h.write_u64(rng_state);
        h.write_u64(rng_inc);
        // SIM-06 boundary: the RNG stream.
        let part_rng = h.finish();
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
        // SIM-06 boundary: world state + the pathfinding substrate
        // (terrain, heights, and both block layers).
        let part_world = h.finish();
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
            // VIC-04: the destroyed-value counter is hashed state,
            // written only while nonzero (the repair-assignment /
            // pennant-bit precedent) so a kill-free stream — the
            // golden scenario included — stays byte-identical to
            // the pre-score stream.
            if p.destroyed_value > 0 {
                h.write_u64(p.destroyed_value);
            }
        }
        // TRD-02: the global exchange rates join the hash once any
        // rate has moved off its start value (the VIC-04
        // conditional-write precedent above), so a trade-free
        // stream — the golden scenario included — stays
        // byte-identical to the pre-Market stream, and a traded
        // stream can never silently agree with an untraded one.
        if !self.market.is_default() {
            for rate in self.market.rates() {
                h.write_i32(rate);
            }
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
            // Repair accumulators (BLD-07): fractional HP and the
            // wood drip are state, so a repair in flight replays
            // bit-identically.
            h.write_u64(b.repair_hp_remainder as u64);
            h.write_u64(b.repair_cost_remainder as u64);
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
            // BLD-18: the building-weapon state and the repair
            // stone accumulator — written ONLY while non-default,
            // behind a marker byte (the destroyed_value /
            // AutoExplore precedents), so a match with no armed
            // building engaged and no tower under repair stays
            // byte-identical to the pre-towers stream.
            if b.repair_stone_remainder != 0 {
                h.write_u8(0xB7);
                h.write_u64(b.repair_stone_remainder as u64);
            }
            if b.weapon_target.is_some()
                || b.weapon_target_building.is_some()
                || b.weapon_strike_tick.is_some()
                || b.weapon_next_tick != 0
                || b.weapon_engaged
            {
                h.write_u8(0xB6);
                match b.weapon_target {
                    None => h.write_u8(0),
                    Some(t) => {
                        h.write_u8(1);
                        h.write_u32(t);
                    }
                }
                match b.weapon_target_building {
                    None => h.write_u8(0),
                    Some(t) => {
                        h.write_u8(1);
                        h.write_u32(t);
                    }
                }
                match b.weapon_strike_tick {
                    None => h.write_u8(0),
                    Some(t) => {
                        h.write_u8(1);
                        h.write_u64(t);
                    }
                }
                h.write_u64(b.weapon_next_tick);
                h.write_u8(u8::from(b.weapon_engaged));
            }
        }
        // SIM-06 boundary: progression, stockpiles, nodes, queues
        // and buildings — the economy block.
        let part_economy = h.finish();
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
                    h.write_i32(o.arrive.to_bits());
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
                    QueuedOrder::Repair { building } => h.write_u32(building),
                    QueuedOrder::AttackMove { x, y } => {
                        h.write_i32(x.to_bits());
                        h.write_i32(y.to_bits());
                    }
                    QueuedOrder::Patrol { x, y } => {
                        h.write_i32(x.to_bits());
                        h.write_i32(y.to_bits());
                    }
                }
            }
            // INF-09: the current order's queue provenance
            // (`order_from_queue`) — written ONLY when set,
            // exactly the way the repair assignment below is
            // written only in phase, so a state with no queued
            // chain in flight hashes exactly as before (the
            // golden stream is untouched). The byte cannot be
            // mistaken for what follows: task tags are 0..=6.
            if e.order_from_queue {
                h.write_u8(0xF1);
            }
            // M2: work state — task phase, assignment, carry.
            h.write_u8(e.task.tag());
            // Repair assignment (BLD-07): written ONLY for a
            // repair-phase entity, exactly the way the construction
            // assignment below writes its building id when one is
            // set — so a state with no repair in flight hashes
            // exactly as before (the golden stream is untouched).
            if let Task::Repair { building } = e.task {
                h.write_u32(building);
            }
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
            // BLD-18: the last hitter's OWNER — written only while
            // set, behind a marker byte (the AutoExplore
            // precedent), so a hit-free stream stays byte-identical
            // to the pre-towers stream. (Any landed hit sets it, so
            // a combat stream's hash moves the tick the first
            // pre-BLD-18 hit lands — deliberate, step-attributed:
            // the attribution is real state the death sweep reads.)
            if let Some(o) = e.last_hit_owner {
                // Marker 0xB4 on the merged tree: the entity
                // section's 0xB0–0xB3 belong to CMB-09/10/11
                // (stance / attack_auto / attack-move / patrol).
                h.write_u8(0xB4);
                h.write_u8(o);
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
            // SEL-17: AutoExplore state — written ONLY while the
            // mode is on, exactly the Task::Repair pattern above,
            // so a match that never toggles it hashes
            // byte-identically to before (the golden stream is
            // untouched); a match that does is fully covered — the
            // flag and its current frontier cell are state, and a
            // replay must reproduce every ride.
            if e.auto_explore {
                h.write_u8(0xAE);
                match e.auto_target {
                    None => h.write_u8(0),
                    Some((cx, cy)) => {
                        h.write_u8(1);
                        h.write_u32(cx);
                        h.write_u32(cy);
                    }
                }
            }
            // Sheep wander schedule (AI-06): derived from tick + RNG,
            // but hashed like every other entity field so a wander
            // divergence can never hide.
            h.write_u64(e.wander_until);
            // CMB-09/10/11: stance and standing orders — written
            // conditionally, on the SEL-17/INF-09 pattern above, so
            // a state where none of them is in play hashes exactly
            // as before. Two default-state writers exist: each
            // seat's starting Scout Cavalry stands at Stand Ground
            // from spawn (CMB-09 — attributed in determinism.rs,
            // the one intended golden mover), and a Defensive post
            // is anchored state whenever the stance is set.
            if e.stance != Stance::Aggressive {
                h.write_u8(0xB0);
                h.write_u8(e.stance.tag());
                if e.stance == Stance::Defensive {
                    h.write_i32(e.stance_anchor_x.to_bits());
                    h.write_i32(e.stance_anchor_y.to_bits());
                }
            }
            if e.attack_auto {
                h.write_u8(0xB1);
            }
            if let Some((ax, ay)) = e.attack_move {
                h.write_u8(0xB2);
                h.write_i32(ax.to_bits());
                h.write_i32(ay.to_bits());
            }
            if !e.patrol.is_empty() {
                h.write_u8(0xB3);
                h.write_u32(e.patrol.len() as u32);
                h.write_u32(e.patrol_next as u32);
                for &(px, py) in &e.patrol {
                    h.write_i32(px.to_bits());
                    h.write_i32(py.to_bits());
                }
            }
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
            // BLD-18: the shooting building, when the shot came
            // from one — written only while set, behind a marker
            // byte, so entity-loosed streams stay byte-identical.
            if let Some(bid) = p.attacker_building {
                // Marker 0xB5 on the merged tree (0xB1 in the
                // entity section belongs to CMB-09 attack_auto).
                h.write_u8(0xB5);
                h.write_u32(bid);
            }
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
        // SIM-06 boundary: entities (positions, orders, combat and
        // targeting state) and projectiles in flight.
        let part_targeting = h.finish();
        // M5: the AI opponent's scheduling state (its decisions read
        // live state, but the wave clock is state of its own).
        h.write_u64(self.ai.next_wave_tick);
        // M7: both players' fog-of-war grids (VIS-01) — what each
        // player knows is state: the AI's decisions are functions
        // of its grid, so a replay must reproduce it exactly.
        for slot in &self.fog {
            h.write_bytes_chunked(slot);
        }
        HashParts {
            rng: part_rng,
            world: part_world,
            economy: part_economy,
            targeting: part_targeting,
            full: h.finish(),
        }
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

/// The move-order target for a woodcutter whose tree's center is
/// `(nx, ny)`, when his standing cell is the ring cell centered at
/// `(cx, cy)`: on the ray from the trunk toward that cell, just
/// outside the tree's own cell — `TREE_STAND_INSET` past the cell
/// boundary (2.0 units from the center orthogonally, 2√2 ≈ 2.828
/// diagonally), so the stand sits 2.5 units from the trunk
/// orthogonally and ≈3.33 diagonally, and the cutter's body ends
/// at the trunk the DE way. The order targets the stand itself
/// and carries the tight `steer::TREE_ARRIVE` radius, so steering
/// settles him there instead of completing a full generic radius
/// short. Before 2026-10-24 the order targeted the ring cell's
/// center — 4.0–5.66 units out — and the cutter visibly chopped
/// from a full tile away (Claudio's live report).
fn tree_stand_point(nx: I16F16, ny: I16F16, cx: I16F16, cy: I16F16) -> (I16F16, I16F16) {
    let dx = cx - nx;
    let dy = cy - ny;
    let len = steer::vec_len(dx, dy);
    if len <= I16F16::ZERO {
        return (cx, cy);
    }
    // Distance from the cell center to the cell boundary along
    // this ray: the half-width (2.0) orthogonally; the corner
    // (2√2) diagonally.
    let boundary = if dx == I16F16::ZERO || dy == I16F16::ZERO {
        I16F16::from_num(2)
    } else {
        I16F16::from_bits(185_364) // 2√2 ≈ 2.82843
    };
    let dist = boundary + TREE_STAND_INSET;
    (nx + dx * dist / len, ny + dy * dist / len)
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
    fn move_to_blocked_target_walks_up_to_it() {
        // A move aimed into a blocked cell no longer drops the
        // order (pre-2026-10-19 behavior): the unit walks to the
        // nearest open cell beside the obstacle, as in the
        // reference game — for a captured sheep, whose only
        // order is a plain move, the dropped order read as
        // "the sheep won't walk" (Claudio's play-test).
        let mut sim = Sim::new(11);
        let vil = sim.entities().iter().find(|e| e.id == 1).unwrap();
        let (vx, vy) = (vil.pos_x, vil.pos_y);
        // M7: the arena walls are gone — the blocked target is a
        // standing tree's cell (MAP-04: a tree blocks its cell).
        // Take the tree nearest the villager so the walk is short.
        let tree = sim
            .nodes()
            .iter()
            .filter(|n| n.kind == NodeKind::Tree)
            .min_by_key(|n| {
                let dx = n.pos_x.to_num::<f64>() - vx.to_num::<f64>();
                let dy = n.pos_y.to_num::<f64>() - vy.to_num::<f64>();
                (dx * dx + dy * dy) as i64
            })
            .expect("a tree");
        let (tx, ty) = (tree.pos_x, tree.pos_y);
        sim.push_command(Command::Move {
            entity: 1,
            x: tx,
            y: ty,
        });
        for _ in 0..3600 {
            sim.tick();
        }
        let v = sim.entities().iter().find(|e| e.id == 1).unwrap();
        assert!(v.order.is_none(), "the snapped walk completes");
        let d = ((v.pos_x - tx) * (v.pos_x - tx) + (v.pos_y - ty) * (v.pos_y - ty)).sqrt();
        assert!(
            d < I16F16::from_num(7),
            "villager ended {d} wu from the tree it was ordered onto — it should stand beside it"
        );
        assert!(
            v.pos_x != vx || v.pos_y != vy,
            "the villager actually walked"
        );
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
            repair_hp_remainder: 0,
            repair_cost_remainder: 0,
            repair_stone_remainder: 0,
            complete: false,
            node_id: None,
            queue: Vec::new(),
            train_progress: 0,
            rally: None,
            rally_node: None,
            weapon_target: None,
            weapon_target_building: None,
            weapon_strike_tick: None,
            weapon_next_tick: 0,
            weapon_engaged: false,
        }
    }

    #[test]
    fn a_lone_non_production_building_no_longer_keeps_its_owner_alive() {
        // VIC-01's letter: elimination is no units + no building
        // capable of producing units. A lone House (even under
        // construction) is not a producer — with every unit gone
        // its owner is eliminated. (The M5 form required razing
        // every last building; that was stricter than the rule.)
        let mut sim = Sim::new_match(11, MatchMode::AiNormal);
        sim.buildings.push(placed_house(entity::OWNER_PLAYER));
        wipe_units(&mut sim, entity::OWNER_PLAYER);
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
    fn a_lone_production_building_keeps_its_owner_alive() {
        // The other half of VIC-01's letter: a standing production
        // building — even a mere Barracks foundation — means the
        // player can still produce, so no elimination while it
        // stands; raze it and the elimination lands.
        let mut sim = Sim::new_match(11, MatchMode::AiNormal);
        let mut rax = placed_house(entity::OWNER_PLAYER);
        rax.id = 900;
        rax.kind = BuildingKind::Barracks;
        sim.buildings.push(rax);
        wipe_units(&mut sim, entity::OWNER_PLAYER);
        assert_eq!(sim.match_state(), MatchState::Ongoing);
        sim.buildings.clear();
        assert_eq!(sim.match_state(), MatchState::Defeat);
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

    #[test]
    fn horse_collar_top_up_rounds_up() {
        // ECO-14 (update 169123): the standing-farm top-up is
        // 75 × (remaining ⁄ full)², rounded UP to whole food —
        // the wiki's worked example: a half farm (87.5 of 175)
        // gains 19, not the raw 18.75, landing at 106.5. The
        // articles state no rounding rule; the round-up is
        // inferred from their examples and followed (see
        // `apply_research`). The integration scenario in
        // tests/feudal_techs.rs pins the same rule end to end.
        let mut sim = Sim::new(5);
        let nid = 999_001u32;
        let half = I16F16::from_num(175) / I16F16::from_num(2);
        sim.nodes.push(ResourceNode {
            id: nid,
            kind: NodeKind::Farm,
            pos_x: I16F16::from_num(0),
            pos_y: I16F16::from_num(0),
            amount_left: half,
            decay_per_tick: I16F16::from_num(0),
        });
        sim.buildings.push(Building {
            id: 999_002,
            kind: BuildingKind::Farm,
            owner: OWNER_PLAYER,
            hp: I16F16::from_num(480),
            cell_x: 0,
            cell_y: 0,
            pos_x: I16F16::from_num(0),
            pos_y: I16F16::from_num(0),
            progress_thirds: 0,
            repair_hp_remainder: 0,
            repair_cost_remainder: 0,
            repair_stone_remainder: 0,
            complete: true,
            node_id: Some(nid),
            queue: Vec::new(),
            train_progress: 0,
            rally: None,
            rally_node: None,
            weapon_target: None,
            weapon_target_building: None,
            weapon_strike_tick: None,
            weapon_next_tick: 0,
            weapon_engaged: false,
        });
        sim.apply_research(OWNER_PLAYER, TechId::HorseCollar);
        let after = sim
            .nodes()
            .iter()
            .find(|n| n.id == nid)
            .unwrap()
            .amount_left;
        assert_eq!(
            after,
            I16F16::from_num(213) / I16F16::from_num(2),
            "half farm after Horse Collar: {after} (want 106.5 = 87.5 + 19)"
        );
    }
}
