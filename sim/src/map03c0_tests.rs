//! MAP03-C0 tests: the map-kind plumbing — `MapKind` threaded
//! through the generator, the `Sim` constructors, the WASM ABI
//! surface and the save restore, with Arabia the only kind.
//!
//! The package emits nothing: the proofs are all identity
//! proofs. `generate(seed)` is byte-identical to
//! `generate_for(Arabia, seed)`; a kind-built match hashes
//! identically to the same match built through the pre-C0
//! constructors (the kind joins the hash only when non-default,
//! and no non-default kind exists); a saved default-kind match
//! restores as Arabia with its hash intact. Together with the
//! unmodified determinism pin, these pin the golden hash
//! `f89f443f9bd8a7fc` immobile by construction — the first
//! emitting kind (C1+) inherits the plumbing, not a retrofit.

use crate::mapgen::{generate, generate_for, MapKind};
use crate::{MatchMode, Sim, TeamSettings, VictorySettings};

#[test]
fn map_kind_tags_round_trip_and_default() {
    // Arabia is the default, carries ABI tag 0, and no other
    // tag names a kind yet.
    assert_eq!(MapKind::default(), MapKind::Arabia);
    assert!(MapKind::Arabia.is_default());
    assert_eq!(MapKind::Arabia.tag(), 0);
    assert_eq!(MapKind::from_tag(0), Some(MapKind::Arabia));
    assert_eq!(MapKind::from_tag(1), None);
    assert_eq!(MapKind::from_tag(255), None);
}

#[test]
fn generate_for_arabia_is_generate_over_soak_seeds() {
    // The scoping handoff's byte-equality proof: the kind-taking
    // generator IS the historical generator for the only kind,
    // over the census soak set (probe seeds + the 0..64 sweep).
    let mut seeds: Vec<u64> = vec![1, 2, 3, 7, 42, 99, 1234, 5555];
    seeds.extend(0..64);
    for seed in seeds {
        assert_eq!(
            generate(seed),
            generate_for(MapKind::Arabia, seed),
            "seed {seed}: generate_for(Arabia) diverged from generate"
        );
    }
}

#[test]
fn kind_built_matches_hash_identically_to_pre_c0_constructors() {
    // Every construction path that names the kind explicitly
    // builds the same match as the path that predates it — at
    // construction and after ticks run (the conditional hash
    // block never fires for the default kind).
    for seed in [1u64, 42, 99] {
        let plain = Sim::new_match(seed, MatchMode::AiNormal);
        let mut by_kind = Sim::new_match_kind(seed, MatchMode::AiNormal, MapKind::Arabia);
        let mut by_full = Sim::with_settings_teams_kind(
            seed,
            MatchMode::AiNormal,
            VictorySettings::conquest(),
            TeamSettings::default(),
            MapKind::Arabia,
        );
        assert_eq!(by_kind.map_kind(), MapKind::Arabia);
        assert_eq!(by_full.map_kind(), MapKind::Arabia);
        assert_eq!(plain.state_hash(), by_kind.state_hash());
        assert_eq!(plain.state_hash(), by_full.state_hash());
        let mut plain = plain;
        for _ in 0..30 {
            plain.tick();
            by_kind.tick();
            by_full.tick();
        }
        assert_eq!(plain.state_hash(), by_kind.state_hash());
        assert_eq!(plain.state_hash(), by_full.state_hash());
    }
    // The sandbox path reports the default kind too.
    assert_eq!(Sim::new(7).map_kind(), MapKind::Arabia);
}

#[test]
fn save_round_trip_restores_the_default_kind() {
    // No SAVE_VERSION bump (see the MAP03-C0 note at
    // SAVE_VERSION): a v21 save carries no kind state because
    // no non-default kind exists, and the restore is Arabia by
    // construction — with the load-time hash check passing,
    // which it could not if the restored kind entered the hash.
    let mut sim = Sim::new_match_kind(42, MatchMode::AiNormal, MapKind::Arabia);
    for _ in 0..30 {
        sim.tick();
    }
    let bytes = sim.save_game();
    let restored = Sim::load_game(&bytes).expect("a save we wrote must load");
    assert_eq!(restored.map_kind(), MapKind::Arabia);
    assert_eq!(restored.state_hash(), sim.state_hash());
}
