//! Teams T2 tests (VIC-12, allied victory pooling): the
//! VictorySettings flag (WASM/hash bit 7), the team-scope fold
//! over the existing per-player victory predicates, pooled
//! relic/Wonder censuses with team-keyed holders, team score
//! means (VIC-04/05), the KotH team-of-winner read, and the
//! independence of the two switches (allied victory vs the
//! team assignment itself — TEAMS-SPEC §4's trap). Fixture
//! style follows `victory_relic_tests` / `victory_wonder_tests`
//! (direct records). Expectations are the execution plan's
//! T2.4 list (a)–(g).

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::{
    Command, MatchMode, MatchState, Sim, TeamSettings, VictorySettings, NO_TEAM,
    RELIC_VICTORY_DURATION_TICKS, WONDER_VICTORY_DURATION_TICKS,
};
use fixed::types::I16F16;

const SAME: TeamSettings = TeamSettings { teams: [1, 1], locked_teams: true };
const DISTINCT: TeamSettings = TeamSettings { teams: [1, 2], locked_teams: true };

fn settings(allied: bool) -> VictorySettings {
    VictorySettings {
        allied_victory: allied,
        ..VictorySettings::conquest()
    }
}

fn pooled_sim(victory: VictorySettings, teams: TeamSettings) -> Sim {
    Sim::with_settings_teams(42, MatchMode::TwoPlayer, victory, teams)
}

/// Mark every entity of `owner` dead (the tombstone way, as
/// the lib's own elimination tests stage it).
fn wipe(sim: &mut Sim, owner: u8) {
    for e in sim.entities.iter_mut() {
        if e.owner == owner {
            e.hp = I16F16::ZERO;
            e.dead = true;
        }
    }
}

fn settle(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

// ------------------------------------------------------------------
// Fixtures (the relic/wonder suites' placed-record shape).

fn placed(sim: &Sim, id: u32, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> Building {
    let size = kind.footprint_size();
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    Building {
        id,
        kind,
        owner,
        hp: I16F16::from_num(kind.max_hp()),
        cell_x: cell.0,
        cell_y: cell.1,
        pos_x: x0 + half - I16F16::from_num(2),
        pos_y: y0 + half - I16F16::from_num(2),
        progress_thirds: kind.build_thirds(),
        repair_hp_remainder: 0,
        repair_cost_remainder: 0,
        repair_stone_remainder: 0,
        repair_gold_remainder: 0,
        build_rate_remainder: 0,
        gate_along_x: true,
        queue_rate_remainder: 0,
        complete: true,
        node_id: None,
        queue: Vec::new(),
        train_progress: 0,
        rally: None,
        rally_node: None,
        weapon_target: None,
        weapon_target_building: None,
        weapon_strike_tick: None,
        weapon_next_tick: 0,
        weapon_engaged: false,
        garrison: Vec::new(),
        garrison_arrows: 0,
        relics: Vec::new(),
        relic_gold_remainder: 0,
    }
}

fn tc_cell(sim: &Sim, owner: u8) -> (u32, u32) {
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == EntityKind::TownCenter)
        .unwrap();
    sim.grid().world_to_cell(tc.pos_x, tc.pos_y)
}

fn add_monastery(sim: &mut Sim, owner: u8, id: u32) -> u32 {
    let (cx, cy) = tc_cell(sim, owner);
    let b = placed(sim, id, BuildingKind::Monastery, owner, (cx + 6, cy + 6));
    sim.buildings.push(b);
    if sim.next_building_id <= id {
        sim.next_building_id = id + 1;
    }
    id
}

fn add_wonder(sim: &mut Sim, owner: u8, id: u32) -> u32 {
    let (cx, cy) = tc_cell(sim, owner);
    let b = placed(sim, id, BuildingKind::Wonder, owner, (cx + 6, cy + 6));
    sim.buildings.push(b);
    if sim.next_building_id <= id {
        sim.next_building_id = id + 1;
    }
    id
}

fn relic_ids(sim: &Sim) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Relic && !e.dead)
        .map(|e| e.id)
        .collect()
}

fn give_garrison(sim: &mut Sim, building: u32, relic: u32) {
    let (bx, by) = {
        let b = sim.buildings().iter().find(|b| b.id == building).unwrap();
        (b.pos_x, b.pos_y)
    };
    let b = sim
        .buildings
        .iter_mut()
        .find(|b| b.id == building)
        .unwrap();
    b.relics.push(relic);
    let r = sim.entities.iter_mut().find(|e| e.id == relic).unwrap();
    r.relic_garrisoned_in = Some(building);
    r.pos_x = bx;
    r.pos_y = by;
}

// ------------------------------------------------------------------
// The flag itself: bit 7, default off, conditional hash.

#[test]
fn allied_victory_flag_is_bit_7_and_off_by_default() {
    let def = VictorySettings::conquest();
    assert!(!def.allied_victory);
    assert!(def.is_default());
    assert_eq!(def.flags(), 0);
    let on = settings(true);
    assert!(!on.is_default());
    assert_eq!(on.flags(), 128, "allied victory packs at bit 7");
    // The settings join the hash while non-default (the victory
    // block carries the flags byte): an allied-on stream and an
    // allied-off stream under identical teams/seeds diverge
    // from tick 0, while two allied-on streams agree.
    let a = pooled_sim(on, DISTINCT);
    let b = pooled_sim(settings(false), DISTINCT);
    let c = pooled_sim(on, DISTINCT);
    assert_ne!(a.state_hash(), b.state_hash());
    assert_eq!(a.state_hash(), c.state_hash());
    assert_eq!(a.victory_settings(), on);
}

// ------------------------------------------------------------------
// (a) Distinct teams + allied on ≡ allied off, outcome for outcome.

#[test]
fn distinct_teams_allied_on_matches_off_outcome_for_outcome() {
    // Fresh match: ongoing either way.
    for allied in [true, false] {
        let sim = pooled_sim(settings(allied), DISTINCT);
        assert_eq!(sim.match_state(), MatchState::Ongoing);
    }
    // Scripted eliminations: each scenario applied to a fresh
    // pair (allied on / allied off) must decide identically.
    for allied in [true, false] {
        let mut sim = pooled_sim(settings(allied), DISTINCT);
        wipe(&mut sim, OWNER_OPPONENT);
        assert_eq!(sim.match_state(), MatchState::Victory, "allied={allied}");
    }
    for allied in [true, false] {
        let mut sim = pooled_sim(settings(allied), DISTINCT);
        wipe(&mut sim, OWNER_PLAYER);
        assert_eq!(sim.match_state(), MatchState::Defeat, "allied={allied}");
    }
    for allied in [true, false] {
        let mut sim = pooled_sim(settings(allied), DISTINCT);
        wipe(&mut sim, OWNER_PLAYER);
        wipe(&mut sim, OWNER_OPPONENT);
        assert_eq!(sim.match_state(), MatchState::Defeat, "allied={allied}");
    }
    for allied in [true, false] {
        let mut sim = pooled_sim(settings(allied), DISTINCT);
        sim.push_command(Command::Resign {
            player: OWNER_OPPONENT,
        });
        sim.tick();
        assert_eq!(sim.match_state(), MatchState::Victory, "allied={allied}");
    }
}

// ------------------------------------------------------------------
// (b) Same team + allied on: one member's fall does not end it;
// both members' fall does (seat 0's team check keeps precedence).

#[test]
fn same_team_allied_on_elimination_pools() {
    let mut sim = pooled_sim(settings(true), SAME);
    wipe(&mut sim, OWNER_OPPONENT);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    wipe(&mut sim, OWNER_PLAYER);
    assert_eq!(sim.match_state(), MatchState::Defeat);

    let mut sim = pooled_sim(settings(true), SAME);
    wipe(&mut sim, OWNER_PLAYER);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    wipe(&mut sim, OWNER_OPPONENT);
    assert_eq!(sim.match_state(), MatchState::Defeat);
}

// ------------------------------------------------------------------
// (c) Same team + allied OFF: per-player exactly as today — the
// two switches are independent (§4's trap, pinned).

#[test]
fn same_team_allied_off_stays_per_player() {
    let mut sim = pooled_sim(settings(false), SAME);
    wipe(&mut sim, OWNER_OPPONENT);
    assert_eq!(sim.match_state(), MatchState::Victory);
    let mut sim = pooled_sim(settings(false), SAME);
    wipe(&mut sim, OWNER_PLAYER);
    assert_eq!(sim.match_state(), MatchState::Defeat);
}

// ------------------------------------------------------------------
// (g) Resignation pools the same way; mixed causes still end it.

#[test]
fn same_team_allied_on_resignation_pools() {
    let mut sim = pooled_sim(settings(true), SAME);
    sim.push_command(Command::Resign {
        player: OWNER_OPPONENT,
    });
    sim.tick();
    assert!(sim.is_resigned(OWNER_OPPONENT));
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    sim.push_command(Command::Resign {
        player: OWNER_PLAYER,
    });
    sim.tick();
    assert_eq!(sim.match_state(), MatchState::Defeat);

    // Mixed causes: one member eliminated, the other resigned.
    let mut sim = pooled_sim(settings(true), SAME);
    wipe(&mut sim, OWNER_OPPONENT);
    sim.push_command(Command::Resign {
        player: OWNER_PLAYER,
    });
    sim.tick();
    assert_eq!(sim.match_state(), MatchState::Defeat);
}

// ------------------------------------------------------------------
// (d) Relic pooling: split stores accrue together; split across
// opposing stores does not; allied off keeps the census per-player.

#[test]
fn relic_pooling_across_team_stores() {
    let mut vic = settings(true);
    vic.relic_victory = true;
    // Same team, relics split 3/2 across the two Monasteries:
    // the team's stores together hold every relic, so the
    // countdown accrues under the team's key (team 1).
    let mut sim = pooled_sim(vic, SAME);
    let mine = add_monastery(&mut sim, OWNER_PLAYER, 900);
    let theirs = add_monastery(&mut sim, OWNER_OPPONENT, 901);
    let relics = relic_ids(&sim);
    assert_eq!(relics.len(), 5);
    for &r in &relics[..3] {
        give_garrison(&mut sim, mine, r);
    }
    for &r in &relics[3..] {
        give_garrison(&mut sim, theirs, r);
    }
    settle(&mut sim, 1);
    assert_eq!(
        sim.relic_countdown(),
        Some((1, RELIC_VICTORY_DURATION_TICKS - 1)),
        "the pooled holder is the team key"
    );
    // Opposing stores (distinct teams), same split: no hold.
    let mut sim = pooled_sim(vic, DISTINCT);
    let mine = add_monastery(&mut sim, OWNER_PLAYER, 900);
    let theirs = add_monastery(&mut sim, OWNER_OPPONENT, 901);
    let relics = relic_ids(&sim);
    for &r in &relics[..3] {
        give_garrison(&mut sim, mine, r);
    }
    for &r in &relics[3..] {
        give_garrison(&mut sim, theirs, r);
    }
    settle(&mut sim, 120);
    assert_eq!(sim.relic_countdown(), None);
    // Same team but allied victory OFF: the census is per-player
    // and the split is nobody's hold (switch independence).
    let mut off = settings(false);
    off.relic_victory = true;
    let mut sim = pooled_sim(off, SAME);
    let mine = add_monastery(&mut sim, OWNER_PLAYER, 900);
    let theirs = add_monastery(&mut sim, OWNER_OPPONENT, 901);
    let relics = relic_ids(&sim);
    for &r in &relics[..3] {
        give_garrison(&mut sim, mine, r);
    }
    for &r in &relics[3..] {
        give_garrison(&mut sim, theirs, r);
    }
    settle(&mut sim, 120);
    assert_eq!(sim.relic_countdown(), None);
}

#[test]
fn relic_pooling_expires_into_the_teams_win() {
    let mut vic = settings(true);
    vic.relic_victory = true;
    let mut sim = pooled_sim(vic, SAME);
    let mine = add_monastery(&mut sim, OWNER_PLAYER, 900);
    let theirs = add_monastery(&mut sim, OWNER_OPPONENT, 901);
    let relics = relic_ids(&sim);
    for &r in &relics[..3] {
        give_garrison(&mut sim, mine, r);
    }
    for &r in &relics[3..] {
        give_garrison(&mut sim, theirs, r);
    }
    settle(&mut sim, RELIC_VICTORY_DURATION_TICKS);
    assert_eq!(sim.relic_countdown(), Some((1, 0)));
    assert_eq!(sim.match_state(), MatchState::Victory);
}

// ------------------------------------------------------------------
// (e) Wonder pooling: an ally's Wonder holds for the team.

#[test]
fn wonder_pooling_holds_for_the_team() {
    let mut vic = settings(true);
    vic.wonder_victory = true;
    // The foe's Wonder, same team: seat 0's side holds, keyed
    // by the team (1), and expiry is seat 0's victory.
    let mut sim = pooled_sim(vic, SAME);
    add_wonder(&mut sim, OWNER_OPPONENT, 900);
    settle(&mut sim, 1);
    assert_eq!(
        sim.wonder_countdown(),
        Some((1, WONDER_VICTORY_DURATION_TICKS - 1))
    );
    settle(&mut sim, WONDER_VICTORY_DURATION_TICKS - 1);
    assert_eq!(sim.match_state(), MatchState::Victory);
    // Distinct teams: the foe's Wonder is the foe team's hold
    // (key 2), not seat 0's.
    let mut sim = pooled_sim(vic, DISTINCT);
    add_wonder(&mut sim, OWNER_OPPONENT, 900);
    settle(&mut sim, 1);
    assert_eq!(
        sim.wonder_countdown(),
        Some((2, WONDER_VICTORY_DURATION_TICKS - 1))
    );
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

// ------------------------------------------------------------------
// (f) Team score mean: the surface and both score endings.

#[test]
fn team_score_is_the_mean_of_member_totals() {
    // Distinct teams: each team's mean is its member's score.
    let sim = pooled_sim(settings(true), DISTINCT);
    assert_eq!(sim.team_score_for(1), sim.score_for(OWNER_PLAYER));
    assert_eq!(sim.team_score_for(2), sim.score_for(OWNER_OPPONENT));
    // No members: NO_TEAM and unknown ids read the zero score.
    assert_eq!(sim.team_score_for(NO_TEAM), Default::default());
    assert_eq!(sim.team_score_for(99), Default::default());

    // Same team, staged apart by a stockpile gift (Economy is
    // 10% of the stockpile): the team score is the mean.
    let mut sim = pooled_sim(settings(true), SAME);
    sim.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(10_000));
    let human = sim.score_for(OWNER_PLAYER);
    let foe = sim.score_for(OWNER_OPPONENT);
    assert!(human.total > foe.total, "the staging must separate them");
    let team = sim.team_score_for(1);
    assert_eq!(team.total, (human.total + foe.total) / 2);
    assert_eq!(team.economy, (human.economy + foe.economy) / 2);
    assert_eq!(team.military, (human.military + foe.military) / 2);
}

#[test]
fn score_target_compares_team_means_while_allied() {
    // Probe the staged totals on a target-free twin, then set
    // the target between the team mean and the human's solo
    // total: the human alone has "reached" it, the team has not.
    let mut probe = pooled_sim(settings(true), SAME);
    probe.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(10_000));
    let human = probe.score_for(OWNER_PLAYER).total;
    let foe = probe.score_for(OWNER_OPPONENT).total;
    let mean = (human + foe) / 2;
    assert!(human > mean + 1, "staging gap too small: {human} vs {mean}");

    let mut vic = settings(true);
    vic.score_target = mean + 1;
    let mut sim = pooled_sim(vic, SAME);
    sim.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(10_000));
    assert_eq!(sim.score_for(OWNER_PLAYER).total, human);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    // Lift the ally: the mean crosses the target and the team wins.
    sim.stockpiles[1].add(ResourceKind::Gold, I16F16::from_num(20_000));
    assert_eq!(sim.match_state(), MatchState::Victory);
}

#[test]
fn time_limit_compares_team_means_while_allied() {
    // Distinct teams at expiry: the higher (singleton) mean wins,
    // exactly the per-player ending.
    let mut vic = settings(true);
    vic.time_limit_ticks = 60;
    let mut sim = pooled_sim(vic, DISTINCT);
    sim.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(10_000));
    settle(&mut sim, 61);
    assert_eq!(sim.match_state(), MatchState::Victory);
    // Same team at expiry: no opposing scope exists — the sole
    // team stands trivially highest, so the clock decides for
    // it (only once the limit is actually reached).
    let mut sim = pooled_sim(vic, SAME);
    sim.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(10_000));
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    settle(&mut sim, 61);
    assert_eq!(sim.match_state(), MatchState::Victory);
}

// ------------------------------------------------------------------
// KotH: the outcome read becomes the team of the winner.

#[test]
fn koth_outcome_reads_the_winners_team() {
    // Same team: the foe seat's Monument win is the team's win.
    let mut sim = pooled_sim(settings(true), SAME);
    sim.koth_winner = Some(OWNER_OPPONENT);
    assert_eq!(sim.match_state(), MatchState::Victory);
    // Distinct teams: the foe's win is seat 0's defeat.
    let mut sim = pooled_sim(settings(true), DISTINCT);
    sim.koth_winner = Some(OWNER_OPPONENT);
    assert_eq!(sim.match_state(), MatchState::Defeat);
    // Allied off, same team assignment: per-player as today.
    let mut sim = pooled_sim(settings(false), SAME);
    sim.koth_winner = Some(OWNER_OPPONENT);
    assert_eq!(sim.match_state(), MatchState::Defeat);
}

// ------------------------------------------------------------------
// Save: the flag and a pooled (team-keyed) holder round-trip
// through the existing blocks — no new version (plan T2.3).

#[test]
fn pooled_countdown_and_flag_round_trip_through_a_save() {
    let mut vic = settings(true);
    vic.relic_victory = true;
    let mut sim = pooled_sim(vic, SAME);
    let mine = add_monastery(&mut sim, OWNER_PLAYER, 900);
    let theirs = add_monastery(&mut sim, OWNER_OPPONENT, 901);
    let relics = relic_ids(&sim);
    for &r in &relics[..3] {
        give_garrison(&mut sim, mine, r);
    }
    for &r in &relics[3..] {
        give_garrison(&mut sim, theirs, r);
    }
    settle(&mut sim, 1_234);
    let before = sim.relic_countdown();
    assert_eq!(before.map(|(h, _)| h), Some(1), "holder is the team key");
    let bytes = sim.save_game();
    let restored = Sim::load_game(&bytes).expect("a save we wrote must load");
    assert_eq!(restored.victory_settings(), vic);
    assert!(restored.victory_settings().allied_victory);
    assert_eq!(restored.relic_countdown(), before);
    assert_eq!(restored.state_hash(), sim.state_hash());
    assert_eq!(restored.save_game(), bytes);
}
