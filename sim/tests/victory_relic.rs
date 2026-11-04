//! VIC-03 (relic victory) — public-API pins.
//!
//! The countdown lifecycle itself (start / tick / cancel /
//! expiry) is proven in-crate in `sim/src/victory_relic_tests.rs`
//! with P5d-style direct-record fixtures, because garrisoning a
//! relic is monk gameplay with no public state mutator. This
//! file pins the surface every consumer sees: the setting is
//! opt-in and hashed, the Tiny duration is VIC-02's table value,
//! plain Conquest streams stay byte-identical, a relic match
//! with no complete hold never counts down, the setting round-
//! trips through a save (format v12), and identical streams
//! replay identically.

use skirmish_sim::{
    MatchMode, MatchState, Sim, VictorySettings, RELIC_VICTORY_DURATION_TICKS,
};

fn relic_settings() -> VictorySettings {
    VictorySettings {
        relic_victory: true,
        ..VictorySettings::conquest()
    }
}

fn settle(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

#[test]
fn relic_victory_is_opt_in_and_off_by_default() {
    assert!(!VictorySettings::conquest().relic_victory);
    assert!(VictorySettings::conquest().is_default());
    assert!(!relic_settings().is_default());
    let sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, relic_settings());
    assert_eq!(sim.victory_settings(), relic_settings());
    assert!(sim.victory_settings().relic_victory);
}

#[test]
fn tiny_duration_is_vic02s_table_value() {
    // VIC-02's table as re-fetched at landing (current wiki,
    // post-MORE_MAP_SIZES): 200 years (Tiny) at 1 year = 5 s of
    // game time, 60 ticks/s — 60,000 ticks.
    assert_eq!(RELIC_VICTORY_DURATION_TICKS, 60_000);
}

#[test]
fn conquest_streams_stay_byte_identical() {
    // The relic setting defaults off and the countdown state is
    // hashed only while a hold stands, so a plain match and an
    // explicit-Conquest match are the same stream — the golden
    // hash cannot move.
    let mut a = Sim::with_mode(42, MatchMode::TwoPlayer);
    let mut b = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::conquest());
    settle(&mut a, 600);
    settle(&mut b, 600);
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn relic_setting_is_hashed_state() {
    // Two matches under different victory settings can never
    // silently agree (the settings block is hashed off-default).
    let relic = Sim::new_match_victory(42, MatchMode::TwoPlayer, relic_settings());
    let conquest = Sim::with_mode(42, MatchMode::TwoPlayer);
    assert_ne!(relic.state_hash(), conquest.state_hash());
}

#[test]
fn no_complete_hold_means_no_countdown() {
    // A fresh relic match: all five relics lie on the ground, so
    // no countdown runs and the match plays on.
    let mut sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, relic_settings());
    settle(&mut sim, 600);
    assert_eq!(sim.relic_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn relic_matches_replay_deterministically() {
    let mut a = Sim::new_match_victory(42, MatchMode::TwoPlayer, relic_settings());
    let mut b = Sim::new_match_victory(42, MatchMode::TwoPlayer, relic_settings());
    for _ in 0..10 {
        settle(&mut a, 60);
        settle(&mut b, 60);
        assert_eq!(a.state_hash(), b.state_hash());
    }
}

#[test]
fn relic_setting_round_trips_through_a_save() {
    let mut sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, relic_settings());
    settle(&mut sim, 120);
    let bytes = sim.save_game();
    let restored = Sim::load_game(&bytes).expect("the save restores");
    assert_eq!(restored.victory_settings(), relic_settings());
    assert_eq!(restored.state_hash(), sim.state_hash());
    assert_eq!(restored.relic_countdown(), None);
}
