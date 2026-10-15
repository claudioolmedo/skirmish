//! Skirmish simulation core — headless and deterministic.
//!
//! This crate contains *all* game rules and *no* rendering, input handling,
//! networking, wall-clock time, or threads. Given the same seed and the same
//! command stream, it must produce the exact same state on every platform,
//! including WebAssembly. See `ARCHITECTURE.md` for the full contract.
//!
//! M1 scope: unit movement — `Move` commands, A* on a coarse grid, steering
//! (separation + arrival). Economy (M2) and combat (M3) arrive later.

mod entity;
mod grid;
mod hash;
mod rng;
mod steer;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use entity::{Entity, MoveOrder};
pub use grid::{Grid, CELL_SIZE, GRID_DIM, WORLD_HALF};
pub use hash::Fnv1a64;
pub use rng::Pcg32;
pub use steer::{MAX_SPEED, SEPARATION_RADIUS, SLOW_RADIUS, WAYPOINT_REACH};

use fixed::types::I16F16;

/// Simulation ticks per second. Fixed forever: the tick count is the only
/// clock the sim knows.
pub const TICKS_PER_SECOND: u64 = 60;

/// Number of demo entities. Gameplay entities replace these in M2–M3; the
/// count exists to give the movement systems something to integrate.
pub const DEMO_ENTITY_COUNT: u32 = 8;

/// World position clamp: entities never leave [-99, 99].
/// (const via raw bits: 99 × 2^16 = 6488064.)
const MIN_POS: I16F16 = I16F16::from_bits(-6_488_064);
const MAX_POS: I16F16 = I16F16::from_bits(6_488_064);

/// Commands are *data*, pushed by the outside world and applied in FIFO
/// order at tick start. This shape is what makes future lockstep multiplayer
/// and replays possible: a match is (seed, settings, commands).
///
/// M1 adds `Move`. Gather/attack commands land in M2–M3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Does nothing. Exists so the command pipeline is exercised.
    Noop,
    /// Order one entity to move to a fixed-point world target. The path is
    /// recomputed immediately and deterministically when the command is
    /// applied; a later `Move` for the same entity replaces the order.
    Move { entity: u32, x: I16F16, y: I16F16 },
}

/// The headless deterministic simulation.
pub struct Sim {
    tick: u64,
    rng: Pcg32,
    grid: Grid,
    /// Demo entities, always sorted by `id`. Never a HashMap.
    entities: Vec<Entity>,
    /// FIFO command queue, drained at the start of every tick.
    commands: Vec<Command>,
}

impl Sim {
    /// Create a new simulation from a seed. Everything — entity spawn
    /// included — is derived from the seed through the sim's own RNG.
    /// Spawn points avoid obstacle cells (deterministic resampling).
    pub fn new(seed: u64) -> Self {
        let mut rng = Pcg32::new(seed);
        let grid = Grid::new();
        let mut entities = Vec::with_capacity(DEMO_ENTITY_COUNT as usize);
        for id in 0..DEMO_ENTITY_COUNT {
            let (pos_x, pos_y) = spawn_point(&mut rng, &grid);
            entities.push(Entity {
                id,
                pos_x,
                pos_y,
                vel_x: I16F16::ZERO,
                vel_y: I16F16::ZERO,
                order: None,
            });
        }
        Sim {
            tick: 0,
            rng,
            grid,
            entities,
            commands: Vec::new(),
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

    /// Advance the simulation by exactly one tick (1/60 s of game time).
    ///
    /// Returns the canonical per-tick state hash. Pure function of
    /// (seed, commands): no wall clock, no randomness outside `self.rng`.
    pub fn tick(&mut self) -> u64 {
        // 1. Apply queued commands in FIFO order (deterministic). Taken out
        //    of the queue first so `issue_move` may borrow `self`.
        let commands: Vec<Command> = std::mem::take(&mut self.commands);
        for command in commands {
            match command {
                Command::Noop => {}
                Command::Move { entity, x, y } => self.issue_move(entity, x, y),
            }
        }

        // 2. Steering is decided from an immutable id-ordered snapshot, then
        //    integrated. Two phases keep borrows simple and the update order
        //    deterministic.
        let snapshot: Vec<(I16F16, I16F16)> =
            self.entities.iter().map(|e| (e.pos_x, e.pos_y)).collect();
        let mut plans: Vec<(I16F16, I16F16, usize, bool)> = Vec::with_capacity(snapshot.len());
        for (i, e) in self.entities.iter().enumerate() {
            plans.push(match &e.order {
                None => (I16F16::ZERO, I16F16::ZERO, 0, false),
                Some(o) => {
                    let (vx, vy, idx, arrived) =
                        steer::steer(&snapshot, i, &o.waypoints, o.wp_index);
                    (vx, vy, idx, arrived)
                }
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

        self.tick += 1;
        self.state_hash()
    }

    /// Apply a `Move` command: clamp the target into the world, recompute
    /// the A* path immediately (deterministically), and install the order.
    /// Unknown entities and unreachable/blocked targets clear the order —
    /// the entity simply holds position.
    fn issue_move(&mut self, entity: u32, x: I16F16, y: I16F16) {
        let x = clamp_pos(x);
        let y = clamp_pos(y);
        let idx = match self.entities.iter().position(|e| e.id == entity) {
            Some(i) => i,
            None => return,
        };
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

    /// Ticks elapsed since creation. The *only* clock in the sim.
    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    /// Read-only view of the entities (id-sorted).
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Canonical per-tick state hash: FNV-1a over the tick counter, the RNG
    /// words, the grid layout, and every entity field (including orders) as
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
        h.write_u32(self.entities.len() as u32);
        for e in &self.entities {
            h.write_u32(e.id);
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

/// Spawn coordinate in [-100, 100): integer tenths, no floats involved.
fn spawn_coord(rng: &mut Pcg32) -> I16F16 {
    let tenths = rng.below(2000) as i32 - 1000;
    I16F16::from_num(tenths) / I16F16::from_num(10)
}

/// Spawn point avoiding obstacle cells. Resamples through the seeded RNG
/// (deterministic); falls back to the first open cell center, which is also
/// deterministic. The demo map is mostly open, so the fallback is unreachable
/// in practice.
fn spawn_point(rng: &mut Pcg32, grid: &Grid) -> (I16F16, I16F16) {
    for _ in 0..32 {
        let x = spawn_coord(rng);
        let y = spawn_coord(rng);
        let (cx, cy) = grid.world_to_cell(x, y);
        if !grid.is_blocked(cx, cy) {
            return (x, y);
        }
    }
    for cy in 0..grid.height() {
        for cx in 0..grid.width() {
            if !grid.is_blocked(cx, cy) {
                return grid.cell_center(cx, cy);
            }
        }
    }
    (I16F16::ZERO, I16F16::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_is_deterministic() {
        let a = Sim::new(5);
        let b = Sim::new(5);
        assert_eq!(a.entities(), b.entities());
    }

    #[test]
    fn spawn_avoids_walls() {
        let sim = Sim::new(5);
        for e in sim.entities() {
            let (cx, cy) = sim.grid.world_to_cell(e.pos_x, e.pos_y);
            assert!(
                !sim.grid.is_blocked(cx, cy),
                "entity {} spawned inside a wall",
                e.id
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
        for _ in 0..130 {
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
            entity: 0,
            x: I16F16::from_num(2),
            y: I16F16::from_num(-46),
        });
        for _ in 0..120 {
            sim.tick();
        }
        let after: Vec<(I16F16, I16F16)> =
            sim.entities().iter().map(|e| (e.pos_x, e.pos_y)).collect();
        assert_eq!(before, after, "entity moved toward a blocked target");
        assert!(sim.entities()[0].order.is_none());
    }

    #[test]
    fn move_unknown_entity_is_ignored() {
        let mut sim = Sim::new(11);
        sim.push_command(Command::Move {
            entity: 999,
            x: I16F16::from_num(10),
            y: I16F16::from_num(10),
        });
        for _ in 0..60 {
            sim.tick();
        }
        assert!(sim.entities().iter().all(|e| e.order.is_none()));
    }
}
