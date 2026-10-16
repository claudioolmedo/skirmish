//! Determinism harness — the heart of M0.
//!
//! These tests prove the sim's core contract: the same (seed, commands)
//! always yields the exact same per-tick state hashes, and different seeds
//! diverge. If any of these fail, the sim has a nondeterminism bug and
//! nothing else may proceed until it is fixed.

use skirmish_sim::{Command, Sim, TICKS_PER_SECOND};

/// Ticks per scenario: 600 = 10 seconds of game time at 60 Hz, enough to
/// cover several RNG-driven velocity nudges (one every 60 ticks).
const TICKS: u64 = 600;

#[test]
fn same_seed_produces_bit_identical_hashes() {
    let mut a = Sim::new(12345);
    let mut b = Sim::new(12345);
    for tick in 0..TICKS {
        let ha = a.tick();
        let hb = b.tick();
        assert_eq!(
            ha, hb,
            "identical runs diverged at tick {}: {:016x} != {:016x}",
            tick, ha, hb
        );
    }
}

#[test]
fn same_seed_with_same_commands_stays_identical() {
    let mut a = Sim::new(999);
    let mut b = Sim::new(999);
    for tick in 0..TICKS {
        // Same command stream on both runs must not break determinism.
        if tick % 37 == 0 {
            a.push_command(Command::Noop);
            b.push_command(Command::Noop);
        }
        assert_eq!(a.tick(), b.tick(), "diverged at tick {}", tick);
    }
}

#[test]
fn different_seeds_diverge() {
    let mut a = Sim::new(1);
    let mut b = Sim::new(2);
    let mut diverged_at = None;
    for tick in 0..TICKS {
        if a.tick() != b.tick() {
            diverged_at = Some(tick);
            break;
        }
    }
    assert!(
        diverged_at.is_some(),
        "different seeds produced identical hashes for {} ticks",
        TICKS
    );
}

#[test]
fn headless_replay_matches_live_run() {
    // A "replay" is just a fresh Sim from the same seed run for the same
    // number of ticks. The full hash streams must match exactly.
    fn run(seed: u64) -> Vec<u64> {
        let mut sim = Sim::new(seed);
        (0..TICKS).map(|_| sim.tick()).collect()
    }
    assert_eq!(run(777), run(777));
    assert_ne!(run(777), run(778));
}

#[test]
fn hash_covers_rng_state() {
    // Two sims that differ only in RNG draws must hash differently even if
    // we could not observe it otherwise. The 60-tick velocity nudge draws
    // from the RNG, so after enough ticks the states must differ.
    let mut a = Sim::new(4242);
    let mut b = Sim::new(4243);
    for _ in 0..3 * TICKS_PER_SECOND {
        a.tick();
        b.tick();
    }
    assert_ne!(a.state_hash(), b.state_hash());
}

#[test]
fn golden_hash_regression() {
    // Tripwire: the exact hash stream for seed 42 must never change
    // silently. If this fails after an intentional sim change, update the
    // expected values deliberately — never blindly.
    //
    // M1 deliberate update: entities no longer drift on RNG nudges (they
    // stand until ordered), spawn avoids wall cells, and the hash now covers
    // the grid layout plus move orders. New value verified stable across
    // independent runs (replay example, seed 42, 360 ticks).
    //
    // M2 deliberate update: the demo entities are replaced by the real
    // starting layout (Town Center + 3 villagers at fixed positions, no
    // RNG spawn), the hash now also covers the stockpile, node amounts,
    // the training queue, entity kinds, and villager work state, and
    // separation became displacement-based with mutual shove. New value
    // verified stable across independent runs (replay example, seed 42,
    // 360 ticks, run twice).
    let mut sim = Sim::new(42);
    let mut hash = 0;
    for _ in 0..360 {
        hash = sim.tick();
    }
    assert_eq!(sim.tick_count(), 360);
    assert_eq!(format!("{:016x}", hash), "ea0f1386f7fc2ed5");
}
