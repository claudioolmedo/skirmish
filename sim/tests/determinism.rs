//! Determinism harness — the heart of M0.
//!
//! These tests prove the sim's core contract: the same (seed, commands)
//! always yields the exact same per-tick state hashes, and different seeds
//! diverge. If any of these fail, the sim has a nondeterminism bug and
//! nothing else may proceed until it is fixed.

use fixed::types::I16F16;
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
fn hash_parts_localize_a_divergence() {
    // SIM-06: the subsystem checksums are prefixes of the one
    // canonical stream — `full` always equals `state_hash()` — and
    // they localize a divergence. Two runs differing only by a move
    // order agree on the RNG and world parts and disagree from the
    // entity (targeting) part on; two runs differing by seed
    // disagree already at the world part (different generated map).
    let mut a = Sim::new(42);
    let mut b = Sim::new(42);
    // Move the first villager of run B somewhere (entity 0 is a
    // villager in the standard opening package).
    let villager = b
        .entities()
        .iter()
        .find(|e| e.kind == skirmish_sim::EntityKind::Villager)
        .map(|e| e.id)
        .expect("opening package has a villager");
    b.push_command(Command::Move {
        entity: villager,
        x: I16F16::from_num(20),
        y: I16F16::from_num(20),
    });
    for _ in 0..30 {
        a.tick();
        b.tick();
    }
    let (pa, pb) = (a.state_hash_parts(), b.state_hash_parts());
    assert_eq!(pa.full, a.state_hash());
    assert_eq!(pb.full, b.state_hash());
    assert_eq!(pa.rng, pb.rng, "a move order draws no RNG");
    assert_eq!(pa.world, pb.world, "the generated world is identical");
    assert_ne!(pa.targeting, pb.targeting, "the moved villager is entity state");
    assert_ne!(pa.full, pb.full);

    let mut c = Sim::new(43);
    for _ in 0..30 {
        c.tick();
    }
    let pc = c.state_hash_parts();
    assert_ne!(pa.world, pc.world, "a different seed generates a different map");
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
    //
    // M2.5 deliberate update: the hash now also covers the grid's
    // dynamic (building-blocked) cell layer, the buildings list with
    // construction progress, and the villagers' construction
    // assignment. No commands are issued in this scenario, so the stream
    // changes only through the wider hash coverage. New value verified
    // stable across independent runs (replay example, seed 42, 360
    // ticks, run twice).
    //
    // M3 deliberate update: the standard start gains the Scout Cavalry
    // and the Gaia herds (8 sheep, 2 boars, 3 deer at fixed cells), and
    // the hash now also covers node decay, building production queues,
    // entity HP/owner/death/combat/animal state, and projectiles in
    // flight. No commands are issued in this scenario, so nothing moves
    // or fights — the stream changes through the new spawns and the
    // wider coverage alone. New value verified stable across
    // independent runs (replay example, seed 42, 360 ticks, run twice).
    //
    // M4 deliberate update: the hash now also covers the per-entity
    // order queues (SEL-12) and the rally points — the Town Center's
    // and every building's, with their snapped nodes (SEL-13). No
    // commands are issued in this scenario, every queue is empty and
    // every rally unset — the stream changes through the wider
    // coverage alone. New value verified stable across independent
    // runs (replay example, seed 42, 360 ticks, run twice).
    //
    // M5 deliberate update: the hash now also covers the match mode,
    // BOTH players' stockpiles (the opponent slot sits untouched at
    // its starting values in this sandbox), the per-Town-Center
    // production states (queue/progress/rally, replacing the single
    // global TC fields), building owner + HP, the entities'
    // building-attack target, projectiles' building target, and the
    // AI's wave clock. No commands are issued in this scenario and
    // the sandbox start is unchanged — the stream changes through
    // the wider coverage alone. New value verified stable across
    // independent runs (replay example, seed 42, 360 ticks, run
    // twice).
    //
    // M6 deliberate update: the hash now also covers the per-player
    // progression state (age + researched-tech mask), the typed
    // production queues (unit/research entry codes, replacing the
    // bare queue count and kind tags), and each villager's current
    // deposit destination. No commands are issued in this scenario —
    // every queue is empty, every player is in the Dark Age with no
    // research, and nobody hauls — the stream changes through the
    // wider coverage alone. New value verified stable across
    // independent runs (replay example, seed 42, 360 ticks, run
    // twice).
    //
    // M7 deliberate update: the world itself changed — the fixed
    // arena is replaced by the seeded Arabia generator (seed 42's
    // generated layout: Town Center, package and heights all differ
    // from the old hand-authored cells), the grid is 120×120 with
    // terrain/height/tree-block layers, and the hash now also
    // covers those layers (chunk-fed), both players' fog-of-war
    // grids, and each projectile's release elevation. No commands
    // are issued in this scenario — the stream changes through the
    // new world and the wider coverage alone. New value verified
    // stable across independent runs (replay example, seed 42, 360
    // ticks, run twice).
    //
    // Pace correction (2026-10): every unit speed moved to the
    // absolute reference tile scale (≈18.75× slower than the M1/M2
    // prototype pace the table had kept), and steering caps the
    // separation term at half a unit's speed so it can no longer
    // cancel an order outright. The golden value deliberately does
    // NOT move: this scenario issues no commands and nothing moves
    // or fights in 360 ticks — speeds and separation never enter
    // it — so an unchanged hash here is itself part of the
    // correction's evidence. (Live-match hash streams, which do
    // move units, all change; they are pinned by their own replay
    // tests, not by this tripwire.)
    //
    // Forest contiguity (2026-10-20): the generator's woodlines
    // changed shape only — discs now fill solid (no interior
    // holes) with ragged rims, and the fallback forests lost
    // their modulo holes. Seed 42's per-start tree count moved
    // 57 -> 64 (per-seed before/after counts for 8 seeds are
    // pinned in mapgen::tests::forests_are_contiguous_masses);
    // no commands are issued in this scenario and no mechanic
    // changed — the stream changes through the generated layout
    // alone. New value verified stable across independent runs
    // (replay example, seed 42, 360 ticks, run twice).
    //
    // Herdables rework (2026-10-20, deliberate update): the hash
    // now also serializes each sheep's wander schedule, Gaia sheep
    // genuinely wander (seeded-RNG steps around their spawn post,
    // AI-06), conversion counts nearby units and unguarded sheep
    // can be stolen, and a sheep's Line of Sight is 2 tiles (wiki,
    // Herdable animal) instead of 3. Scenario equivalence for THIS
    // tripwire was verified directly against the previous build:
    // at tick 360 the entity census is identical (3 villagers,
    // 1 scout, 8 sheep all still Gaia, 2 boars, 6 deer, 1 Town
    // Center), every non-sheep entity's position and HP are
    // bit-identical, and the sheep positions are bit-identical
    // too (the wander cadence is lazy; no step has landed yet) —
    // the value moves on the new hash coverage and the new sheep
    // rules alone, not on any change to the world this scenario
    // builds. New value verified stable across independent runs
    // (run-to-run determinism test in this file).
    //
    // Study-2 remainder (2026-10-21, deliberate update): two
    // documented-DE corrections, each re-baselined separately so
    // the attribution is exact. (1) Town Center sight 6 -> 8
    // tiles (VIS-03; the fog grids are hashed state, so the wider
    // starting discs move the stream): 93c1e963b7a09619 ->
    // ade9599580510543. (2) Arabia gold 7+4 -> 7+4+4 (MAP-09 /
    // OPENING-SPEC §2.3: a second 4-tile secondary per start,
    // farther out at a split anchor; seed 42's layout gains 8 gold
    // nodes): ade9599580510543 -> 042e33a17a6192ae. The Militia
    // cost correction shipped in the same run (60 F -> 50 F,
    // update 141935) does NOT move this value: the scenario
    // issues no commands and trains nothing — costs never enter
    // it. (3) BLD-18 (2026-10-22): Entity::last_hit_owner — the
    // owner-level kill attribution a building's arrows need —
    // is hashed (marker byte 0xB4 on the merged tree; 0xB0–0xB3
    // in the entity section belong to CMB-09/10/11) whenever a
    // hit has landed. No hit lands in this scenario (nothing
    // fights in 360 ticks — the boars stay unprovoked), so the
    // field never enters the stream; every BLD-18 building-side
    // state (weapon fields, attacker_building, the repair stone
    // accumulator) is likewise conditional and adds nothing
    // here — no tower stands in this scenario. The value below
    // does NOT move for BLD-18: the package's own correction
    // commit established the same on its pre-merge tree (its
    // real value there was 042e33a17a6192ae, unchanged — an
    // earlier landing draft had re-pinned from a mislabeled
    // tool result).
    // New value verified stable across independent runs
    // (run-to-run determinism test in this file).
    //
    // Orders & stances (2026-10-22, deliberate update): the hash
    // now also serializes unit stances and standing orders
    // (CMB-09/10/11), conditionally — written only when a stance
    // differs from Aggressive, a target was auto-acquired, or an
    // attack-move/patrol is active. This scenario issues no
    // commands and nothing fights in 360 ticks, so the sole
    // contributor is the starting Scout Cavalry's Stand Ground
    // default stance (CMB-09), serialized from spawn. Attribution
    // verified directly: with only the two spawn stance
    // assignments disabled (both seats), the stream returns to
    // exactly 042e33a17a6192ae — no other byte of this package
    // enters this scenario. 042e33a17a6192ae -> 3c2febd6b29076a4.
    // New value verified stable across independent runs
    // (run-to-run determinism test in this file).
    // Predator wolves (2026-10-22, deliberate update, AI-08):
    // 042e33a17a6192ae -> 3e6361a3d06dbb8f. Step attribution: the
    // ONLY change entering this scenario is the map generator's
    // new wolf pack (3 wolves per start at 26–34 tiles; the
    // sandbox spawns the human's pack — 3 new Gaia entities in
    // the hashed entity stream). No commands are issued, the pack
    // stands 24+ tiles from the nearest unit (far outside every
    // LoS in the scenario, the wolf's own 12 included), so no
    // wolf moves, hunts, or fights in 360 ticks — the stream
    // moves on the placement alone. New value verified stable
    // across independent runs (run-to-run determinism test in
    // this file).
    //
    // Landing on main (2026-10-22, evening): the wolf arrived
    // renumbered — kind tag 9 -> 11 (9/10 were taken on main by
    // the Spearman/Skirmisher) and PREDATOR class 13 -> 17
    // (13-16 taken) — and the two deliberate steps compose:
    // 042e33a17a6192ae -> 3c2febd6b29076a4 (orders, above) ->
    // 607d975bdd71e8eb (wolves). Probe attribution on the merged
    // tree, each factor isolated: wolf generation removed ->
    // exactly 3c2febd6b29076a4 (main's value); the two spawn
    // stance assignments disabled -> exactly bb55a57ce00f1005,
    // identical to the pristine wolf branch with only the tag
    // renumbered; both reverted -> exactly 042e33a17a6192ae.
    // The wolf step is placement plus its generator-stream
    // wake: skipping only the wolf SPAWNS (generation intact)
    // lands at 55a3cc8d30f1ef77, not main's value — the pack's
    // generation draws regenerate the second start's package
    // downstream (the MAP-09 mechanism), so the step is the
    // mapgen change entire, with no other package inside it.
    let mut sim = Sim::new(42);
    let mut hash = 0;
    for _ in 0..360 {
        hash = sim.tick();
    }
    assert_eq!(sim.tick_count(), 360);
    assert_eq!(format!("{:016x}", hash), "607d975bdd71e8eb");
}
