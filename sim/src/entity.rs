//! Entities and move orders (M1).
//!
//! M0 proved the *machinery* of determinism with trivial drifting points.
//! M1 gives those entities real behavior: move orders with A* waypoint
//! trails, followed with steering (separation + arrival). Economy (M2) and
//! combat (M3) will grow this struct further — the determinism rules,
//! however, are permanent.
//!
//! Entities are stored in a `Vec` sorted by `id` (ids are assigned in order
//! at spawn). Iteration order is therefore always deterministic — there is
//! intentionally no hash map anywhere in the sim.

use fixed::types::I16F16;

/// An entity: id, fixed-point position/velocity, and an optional move order.
///
/// `Vec` inside `MoveOrder` means `Entity` is `Clone` but not `Copy`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
    pub id: u32,
    pub pos_x: I16F16,
    pub pos_y: I16F16,
    pub vel_x: I16F16,
    pub vel_y: I16F16,
    /// Active move order, if any. Plain data — part of the hashed state.
    pub order: Option<MoveOrder>,
}

/// A move order: the commanded target plus the A* waypoint trail leading to
/// it. `wp_index` is the waypoint currently being steered toward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveOrder {
    pub target_x: I16F16,
    pub target_y: I16F16,
    /// Waypoints in world coordinates: cell centers, ending exactly on the
    /// commanded target point.
    pub waypoints: Vec<(I16F16, I16F16)>,
    pub wp_index: usize,
}
