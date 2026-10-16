//! Entities, move orders, and villager work state (M1–M2).
//!
//! M0 proved the *machinery* of determinism with trivial drifting points.
//! M1 gave those entities real behavior: move orders with A* waypoint
//! trails, followed with steering (separation + arrival). M2 adds kinds
//! (villager / Town Center) and the villager work state machine — the
//! determinism rules, however, are permanent.
//!
//! Entities are stored in a `Vec` sorted by `id` (ids are assigned in
//! order at spawn). Iteration order is therefore always deterministic —
//! there is intentionally no hash map anywhere in the sim.

use fixed::types::I16F16;

use crate::econ::ResourceKind;

/// What an entity is. Villagers move and work; the Town Center is a static
/// building (drop-off point and villager producer). M3 adds military
/// kinds alongside these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Villager,
    TownCenter,
}

impl EntityKind {
    /// Stable tag for the state hash and the WASM kind array:
    /// 0 = villager, 1 = town center.
    pub fn tag(self) -> u8 {
        match self {
            EntityKind::Villager => 0,
            EntityKind::TownCenter => 1,
        }
    }
}

/// The villager work state machine (M2). A villager's persistent *order*
/// is its `task_node`; `Task` is the phase of the work cycle it is in:
///
/// - `Gather` command: `Idle → ToNode` (or `→ Returning` first, if the
///   villager carries a different resource — it deposits that first).
/// - `ToNode`: on arrival within gather range, `→ Gathering`.
/// - `Gathering`: when the carry fills up or the node runs dry,
///   `→ Returning` (or `→ Idle` if the node died with an empty carry).
/// - `Returning`: on arrival at the Town Center the carry is deposited,
///   then `→ ToNode` again if the assigned node is still alive,
///   otherwise `→ Idle`.
/// - A `Move` command clears the task from any phase (the villager keeps
///   whatever it carries — see `carry`). The Town Center never works:
///   its task is always `Idle`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    /// No work order. The entity may still hold a plain move order.
    Idle,
    /// Travelling to the task node to start gathering.
    ToNode,
    /// At the task node, accumulating resources into `carry`.
    Gathering,
    /// Travelling to the Town Center to deposit the carry.
    Returning,
}

impl Task {
    /// Stable tag for the state hash: 0 idle, 1 to-node, 2 gathering,
    /// 3 returning.
    pub fn tag(self) -> u8 {
        match self {
            Task::Idle => 0,
            Task::ToNode => 1,
            Task::Gathering => 2,
            Task::Returning => 3,
        }
    }
}

/// An entity: id, kind, fixed-point position/velocity, an optional move
/// order, and (for villagers) work state.
///
/// `Vec` inside `MoveOrder` means `Entity` is `Clone` but not `Copy`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub id: u32,
    pub kind: EntityKind,
    pub pos_x: I16F16,
    pub pos_y: I16F16,
    pub vel_x: I16F16,
    pub vel_y: I16F16,
    /// Active move order, if any. Plain data — part of the hashed state.
    /// Movement is the *mechanism*; the work state machine (re)issues
    /// move orders to get villagers where their task needs them.
    pub order: Option<MoveOrder>,
    /// Current phase of the work cycle (villagers only).
    pub task: Task,
    /// The persistent gather assignment: the node this villager works
    /// until re-commanded. Survives deposit trips; cleared by a `Move`
    /// command or when the node is gone and the carry is delivered.
    pub task_node: Option<u32>,
    /// Resources currently carried. Counted in the stockpile only when
    /// deposited at the Town Center. A `Move` override keeps the carry —
    /// it is neither deposited nor lost, just still on the villager.
    pub carry: I16F16,
    /// What `carry` holds. `None` iff `carry` is zero.
    pub carry_kind: Option<ResourceKind>,
}

impl Entity {
    /// A fresh idle villager at a position.
    pub fn villager(id: u32, pos_x: I16F16, pos_y: I16F16) -> Self {
        Entity {
            id,
            kind: EntityKind::Villager,
            pos_x,
            pos_y,
            vel_x: I16F16::ZERO,
            vel_y: I16F16::ZERO,
            order: None,
            task: Task::Idle,
            task_node: None,
            carry: I16F16::ZERO,
            carry_kind: None,
        }
    }

    /// The static Town Center building at a position.
    pub fn town_center(id: u32, pos_x: I16F16, pos_y: I16F16) -> Self {
        Entity {
            kind: EntityKind::TownCenter,
            ..Entity::villager(id, pos_x, pos_y)
        }
    }
}

/// A move order: the commanded target plus the A* waypoint trail leading
/// to it. `wp_index` is the waypoint currently being steered toward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveOrder {
    pub target_x: I16F16,
    pub target_y: I16F16,
    /// Waypoints in world coordinates: cell centers, ending exactly on
    /// the commanded target point.
    pub waypoints: Vec<(I16F16, I16F16)>,
    pub wp_index: usize,
}
