//! Teams T1 tests (2026-11-05): the sim core of the teams
//! cluster (VIS-08's foundation) — `TeamSettings` with the
//! `allied()` / `team_of()` / `same_team()` predicates, the
//! derived shared-vision read (`visibility_shared`), the
//! conditional hash write (marker 0xC4) and the save v17
//! round-trip. Expectations are the dossier's pinned model
//! (TEAMS-SPEC §3): defaults are distinct teams and reproduce
//! today exactly; `NO_TEAM` players are enemies, not allies;
//! the union is a read over per-player truth, never stored.
//! In-crate staging follows the `tribute_vision_tests` style.

use crate::entity::{EntityKind, OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER};
use crate::{Command, MatchMode, Sim, TeamSettings, VictorySettings, NO_TEAM};
use fixed::types::I16F16;

fn allied_sim(seed: u64) -> Sim {
    Sim::with_settings_teams(
        seed,
        MatchMode::TwoPlayer,
        VictorySettings::conquest(),
        TeamSettings { teams: [1, 1] },
    )
}

fn foe_scout(sim: &Sim) -> (u32, I16F16, I16F16) {
    let e = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::ScoutCavalry)
        .expect("the opponent start has a Scout Cavalry");
    (e.id, e.pos_x, e.pos_y)
}

/// Run the staged alliance of the plan's test (b): the foe
/// scout rides a little deeper into its own half, on both the
/// allied sim and its distinct-teams twin, with the identical
/// command stream.
fn staged_pair() -> (Sim, Sim) {
    let mut allied = allied_sim(42);
    let mut distinct = Sim::new_match(42, MatchMode::TwoPlayer);
    for sim in [&mut allied, &mut distinct] {
        let (scout, x, y) = foe_scout(sim);
        sim.push_command(Command::Move {
            entity: scout,
            x: x + I16F16::from_num(8),
            y,
        });
        for _ in 0..180 {
            sim.tick();
        }
    }
    (allied, distinct)
}

// ------------------------------------------------------------------
// (a) Defaults: no allies, and the shared read is the own grid.

#[test]
fn defaults_have_no_allies_and_shared_is_own_grid() {
    let mut sim = Sim::new_match(7, MatchMode::AiNormal);
    for _ in 0..120 {
        sim.tick();
    }
    assert_eq!(sim.team_settings(), TeamSettings::default());
    assert!(sim.team_settings().is_default());
    assert_eq!(sim.team_of(OWNER_PLAYER), Some(1));
    assert_eq!(sim.team_of(OWNER_OPPONENT), Some(2));
    assert!(!sim.allied(OWNER_PLAYER, OWNER_OPPONENT));
    assert!(!sim.allied(OWNER_OPPONENT, OWNER_PLAYER));
    assert!(!sim.same_team(OWNER_PLAYER, OWNER_OPPONENT));
    // In a running match under defaults the union is the own
    // grid, cell for cell, for both seats.
    assert_eq!(
        sim.visibility_shared(OWNER_PLAYER),
        sim.visibility(OWNER_PLAYER).to_vec()
    );
    assert_eq!(
        sim.visibility_shared(OWNER_OPPONENT),
        sim.visibility(OWNER_OPPONENT).to_vec()
    );
}

#[test]
fn team_predicate_edge_cases() {
    let sim = allied_sim(7);
    // Self is never its own ally, but does share its own team.
    assert!(!sim.allied(OWNER_PLAYER, OWNER_PLAYER));
    assert!(sim.same_team(OWNER_PLAYER, OWNER_PLAYER));
    assert!(sim.allied(OWNER_PLAYER, OWNER_OPPONENT));
    assert!(sim.allied(OWNER_OPPONENT, OWNER_PLAYER));
    // Gaia and unknown tags: no team, no allies, no grid.
    assert_eq!(sim.team_of(OWNER_GAIA), None);
    assert_eq!(sim.team_of(99), None);
    assert!(!sim.allied(OWNER_PLAYER, OWNER_GAIA));
    assert!(!sim.allied(OWNER_GAIA, OWNER_OPPONENT));
    assert!(!sim.same_team(OWNER_PLAYER, OWNER_GAIA));
    assert!(sim.visibility_shared(OWNER_GAIA).is_empty());
    assert!(sim.visibility_shared(99).is_empty());
}

// ------------------------------------------------------------------
// (b) Staged alliance: the union is real, the truth stays separate.

#[test]
fn staged_alliance_unions_vision_but_truth_stays_separate() {
    let (allied, distinct) = staged_pair();

    // Per-player truth is identical between the allied sim and
    // its distinct-teams twin under the same stream: staging an
    // alliance changed no fog grid (recompute is untouched).
    assert_eq!(
        allied.visibility(OWNER_PLAYER),
        distinct.visibility(OWNER_PLAYER)
    );
    assert_eq!(
        allied.visibility(OWNER_OPPONENT),
        distinct.visibility(OWNER_OPPONENT)
    );
    // The distinct twin's shared read is still just its own grid.
    assert_eq!(
        distinct.visibility_shared(OWNER_PLAYER),
        distinct.visibility(OWNER_PLAYER).to_vec()
    );

    // The foe scout stands deep in its own half, far from the
    // human Town Center.
    let (_, sx, sy) = foe_scout(&allied);
    let (scx, scy) = allied.grid().world_to_cell(sx, sy);
    let tc = allied
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::TownCenter)
        .expect("a Town Center");
    let (tcx, tcy) = allied.grid().world_to_cell(tc.pos_x, tc.pos_y);
    assert!(
        (scx as i32 - tcx as i32).abs().max((scy as i32 - tcy as i32).abs()) > 30,
        "the staged scout must stand far from the human start"
    );
    // The scout's own cell: the foe sees it (2), the human's own
    // grid has never explored the foe's half (0) — and the
    // human's SHARED grid shows it (2). The union is real, and
    // it lives only in the shared read.
    let idx = (scy * crate::GRID_DIM + scx) as usize;
    assert_eq!(allied.visibility(OWNER_OPPONENT)[idx], 2);
    assert_eq!(allied.visibility(OWNER_PLAYER)[idx], 0);
    assert_eq!(allied.visibility_shared(OWNER_PLAYER)[idx], 2);
    assert_eq!(allied.visibility_shared(OWNER_OPPONENT)[idx], 2);

    // The shared grid is the cell-wise max over the team,
    // everywhere — and it genuinely differs from the own grid.
    let shared = allied.visibility_shared(OWNER_PLAYER);
    let own = allied.visibility(OWNER_PLAYER);
    let foe = allied.visibility(OWNER_OPPONENT);
    assert_ne!(shared, own.to_vec());
    for i in 0..shared.len() {
        assert_eq!(shared[i], own[i].max(foe[i]), "cell {i}");
    }
}

// ------------------------------------------------------------------
// (c) NO_TEAM × NO_TEAM is not allied.

#[test]
fn no_team_players_are_not_allies() {
    let sim = Sim::with_settings_teams(
        7,
        MatchMode::TwoPlayer,
        VictorySettings::conquest(),
        TeamSettings {
            teams: [NO_TEAM, NO_TEAM],
        },
    );
    assert_eq!(sim.team_of(OWNER_PLAYER), Some(NO_TEAM));
    assert_eq!(sim.team_of(OWNER_OPPONENT), Some(NO_TEAM));
    assert!(!sim.allied(OWNER_PLAYER, OWNER_OPPONENT));
    assert!(!sim.allied(OWNER_OPPONENT, OWNER_PLAYER));
    assert!(!sim.same_team(OWNER_PLAYER, OWNER_OPPONENT));
    // No sharing either: equal raw bytes do not union the grids.
    assert_eq!(
        sim.visibility_shared(OWNER_PLAYER),
        sim.visibility(OWNER_PLAYER).to_vec()
    );
    // A NO_TEAM seat beside a teamed seat is likewise unaffiliated.
    let mixed = Sim::with_settings_teams(
        7,
        MatchMode::TwoPlayer,
        VictorySettings::conquest(),
        TeamSettings {
            teams: [NO_TEAM, 1],
        },
    );
    assert!(!mixed.allied(OWNER_PLAYER, OWNER_OPPONENT));
    assert!(!mixed.same_team(OWNER_PLAYER, OWNER_OPPONENT));
}

// ------------------------------------------------------------------
// (d)+(f) Conditional hash: defaults write nothing, non-defaults
// diverge — and only there.

#[test]
fn explicit_default_teams_stream_matches_implicit_default() {
    let mut a = Sim::new_match(9, MatchMode::TwoPlayer);
    let mut b = Sim::with_settings_teams(
        9,
        MatchMode::TwoPlayer,
        VictorySettings::conquest(),
        TeamSettings::default(),
    );
    let mut c = Sim::with_settings(9, MatchMode::TwoPlayer, VictorySettings::conquest());
    for tick in 0..300 {
        let (ha, hb, hc) = (a.tick(), b.tick(), c.tick());
        assert_eq!(ha, hb, "explicit defaults diverged at tick {tick}");
        assert_eq!(ha, hc, "with_settings diverged at tick {tick}");
    }
}

#[test]
fn non_default_teams_change_the_hash_only_in_the_settings_block() {
    let a = Sim::new_match(9, MatchMode::TwoPlayer);
    let b = allied_sim(9);
    // At tick 0 the two states differ ONLY in team settings:
    // the hash parts agree through the world boundary and
    // diverge from the economy part on — the teams block sits
    // between them, exactly where hash_parts writes it.
    let (pa, pb) = (a.state_hash_parts(), b.state_hash_parts());
    assert_eq!(pa.rng, pb.rng);
    assert_eq!(pa.world, pb.world);
    assert_ne!(pa.economy, pb.economy);
    assert_ne!(pa.full, pb.full);
    assert_ne!(a.state_hash(), b.state_hash());
    // And the divergence persists as the match runs.
    let mut a = a;
    let mut b = b;
    for _ in 0..120 {
        a.tick();
        b.tick();
    }
    assert_ne!(a.state_hash(), b.state_hash());
    // A swapped distinct pair ([2, 1]) is behaviorally
    // distinct-teamed but literally non-default state, and it
    // hashes as such.
    let swapped = Sim::with_settings_teams(
        9,
        MatchMode::TwoPlayer,
        VictorySettings::conquest(),
        TeamSettings { teams: [2, 1] },
    );
    assert!(!swapped.team_settings().is_default());
    assert!(!swapped.allied(OWNER_PLAYER, OWNER_OPPONENT));
    assert_ne!(swapped.state_hash(), a.state_hash());
}

// ------------------------------------------------------------------
// (e) Save round-trip under a non-default team setting.

#[test]
fn save_round_trip_under_non_default_teams() {
    let (sim, _) = staged_pair();
    let bytes = sim.save_game();
    let restored = Sim::load_game(&bytes).expect("a v17 save we wrote must load");
    assert_eq!(restored.tick_count(), sim.tick_count());
    assert_eq!(restored.state_hash(), sim.state_hash());
    assert_eq!(restored.team_settings(), sim.team_settings());
    assert!(restored.allied(OWNER_PLAYER, OWNER_OPPONENT));
    assert_eq!(
        restored.visibility_shared(OWNER_PLAYER),
        sim.visibility_shared(OWNER_PLAYER)
    );
    // Byte-identical re-save: the format is canonical.
    assert_eq!(restored.save_game(), bytes);
}

#[test]
fn save_round_trip_under_default_teams_keeps_defaults() {
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    for _ in 0..120 {
        sim.tick();
    }
    let restored = Sim::load_game(&sim.save_game()).expect("default save loads");
    assert_eq!(restored.state_hash(), sim.state_hash());
    assert_eq!(restored.team_settings(), TeamSettings::default());
    assert!(!restored.allied(OWNER_PLAYER, OWNER_OPPONENT));
}
