//! Demo entities for the M0 determinism harness.
//!
//! M0 proves the *machinery* of determinism (fixed-point math, seeded RNG,
//! per-tick hashing), not gameplay. These entities are deliberately trivial:
//! points with fixed-point positions and velocities, integrated every tick.
//! Real units, economy, and combat arrive in M1–M3 and will replace this
//! scaffolding — the determinism rules, however, are permanent.

use fixed::types::I16F16;

/// A demo entity: an id plus fixed-point position and velocity.
///
/// Entities are stored in a `Vec` sorted by `id` (ids are assigned in order
/// at spawn). Iteration order is therefore always deterministic — there is
/// intentionally no hash map anywhere in the sim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entity {
    pub id: u32,
    pub pos_x: I16F16,
    pub pos_y: I16F16,
    pub vel_x: I16F16,
    pub vel_y: I16F16,
}
