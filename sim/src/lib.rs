//! Skirmish simulation core — headless and deterministic.
//!
//! This crate contains *all* game rules and *no* rendering, input handling,
//! networking, wall-clock time, or threads. Given the same seed and the same
//! command stream, it must produce the exact same state on every platform,
//! including WebAssembly. See `ARCHITECTURE.md` for the full contract.
//!
//! M0 scope: fixed-point math, a seeded RNG inside the state, a fixed 60 Hz
//! tick loop, and a per-tick state hash — exercised by a handful of demo
//! entities. Gameplay (units, economy, combat) arrives in M1–M3.

mod entity;
mod hash;
mod rng;

#[cfg(target_arch = "wasm32")]
mod wasm;

pub use entity::Entity;
pub use hash::Fnv1a64;
pub use rng::Pcg32;

use fixed::types::I16F16;

/// Simulation ticks per second. Fixed forever: the tick count is the only
/// clock the sim knows.
pub const TICKS_PER_SECOND: u64 = 60;

/// Number of demo entities spawned in M0. Gameplay entities replace these
/// in M1–M3; the count exists only to give the determinism harness
/// something to integrate.
pub const DEMO_ENTITY_COUNT: u32 = 8;

/// Commands are *data*, pushed by the outside world and applied in FIFO
/// order at the start of each tick. This shape is what makes future lockstep
/// multiplayer and replays possible: a match is (seed, settings, commands).
///
/// M0 defines only a placeholder. Real commands (move, gather, attack, …)
/// land in M1–M3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Does nothing. Exists so the command pipeline is exercised from M0.
    Noop,
}

/// The headless deterministic simulation.
pub struct Sim {
    tick: u64,
    rng: Pcg32,
    /// Demo entities, always sorted by `id`. Never a HashMap.
    entities: Vec<Entity>,
    /// FIFO command queue, drained at the start of every tick.
    commands: Vec<Command>,
}

impl Sim {
    /// Create a new simulation from a seed. Everything — entity spawn included
    /// — is derived from the seed through the sim's own RNG.
    pub fn new(seed: u64) -> Self {
        let mut rng = Pcg32::new(seed);
        let mut entities = Vec::with_capacity(DEMO_ENTITY_COUNT as usize);
        for id in 0..DEMO_ENTITY_COUNT {
            entities.push(Entity {
                id,
                pos_x: spawn_coord(&mut rng),
                pos_y: spawn_coord(&mut rng),
                vel_x: spawn_vel(&mut rng),
                vel_y: spawn_vel(&mut rng),
            });
        }
        Sim {
            tick: 0,
            rng,
            entities,
            commands: Vec::new(),
        }
    }

    /// Queue a command to be applied at the start of the next tick.
    pub fn push_command(&mut self, command: Command) {
        self.commands.push(command);
    }

    /// Advance the simulation by exactly one tick (1/60 s of game time).
    ///
    /// Returns the canonical per-tick state hash. Pure function of
    /// (seed, commands): no wall clock, no randomness outside `self.rng`.
    pub fn tick(&mut self) -> u64 {
        // 1. Apply queued commands in FIFO order (deterministic).
        for command in self.commands.drain(..) {
            match command {
                Command::Noop => {}
            }
        }

        // 2. M0 demo behavior: every 60 ticks, re-roll one entity's velocity
        //    through the seeded RNG. This exercises the RNG-inside-state
        //    path so the harness actually covers it.
        if self.tick.is_multiple_of(TICKS_PER_SECOND) {
            let index = self.rng.below(self.entities.len() as u32) as usize;
            let entity = &mut self.entities[index];
            entity.vel_x = spawn_vel(&mut self.rng);
            entity.vel_y = spawn_vel(&mut self.rng);
        }

        // 3. Integrate demo entities: pos += vel, in id order, fixed-point.
        for entity in self.entities.iter_mut() {
            entity.pos_x += entity.vel_x;
            entity.pos_y += entity.vel_y;
        }

        self.tick += 1;
        self.state_hash()
    }

    /// Ticks elapsed since creation. The *only* clock in the sim.
    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    /// Read-only view of the demo entities (id-sorted).
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Canonical per-tick state hash: FNV-1a over the tick counter, the RNG
    /// words, and every entity field as raw bits, in a fixed order.
    /// Platform-independent by construction.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv1a64::new();
        h.write_u64(self.tick);
        let (rng_state, rng_inc) = self.rng.state_words();
        h.write_u64(rng_state);
        h.write_u64(rng_inc);
        h.write_u32(self.entities.len() as u32);
        for e in &self.entities {
            h.write_u32(e.id);
            h.write_i32(e.pos_x.to_bits());
            h.write_i32(e.pos_y.to_bits());
            h.write_i32(e.vel_x.to_bits());
            h.write_i32(e.vel_y.to_bits());
        }
        h.finish()
    }

    /// State hash as a 16-character lowercase hex string.
    pub fn state_hash_hex(&self) -> String {
        format!("{:016x}", self.state_hash())
    }
}

/// Spawn coordinate in [-100, 100): integer tenths, no floats involved.
fn spawn_coord(rng: &mut Pcg32) -> I16F16 {
    let tenths = rng.below(2000) as i32 - 1000;
    I16F16::from_num(tenths) / I16F16::from_num(10)
}

/// Spawn velocity component in [-2, 2): integer tenths, no floats involved.
fn spawn_vel(rng: &mut Pcg32) -> I16F16 {
    let tenths = rng.below(40) as i32 - 20;
    I16F16::from_num(tenths) / I16F16::from_num(10)
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
}
