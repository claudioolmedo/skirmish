//! VIC-03 tests (relic victory, Standard games): the all-relics
//! countdown — it starts only when one player garrisons *every*
//! relic on the map in their Monasteries, ticks down from the
//! Tiny duration (`RELIC_VICTORY_DURATION_TICKS`, VIC-02's table:
//! 100 years at 5 s/year), cancels outright when any relic leaves
//! a Monastery, and expires into the holder's win through
//! `match_state`. Plain Conquest (setting off) never counts down,
//! however many relics a player holds; Sandbox never counts down;
//! a map with zero relics never starts ("all of zero" is not a
//! hold). Fixture style follows `monastery_p5d_tests` (direct
//! records — the garrison wiring is P5d's own helper shape).

use crate::build::{Building, BuildingKind};
use crate::entity::{EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::{
    Command, MatchMode, MatchState, Sim, VictorySettings, RELIC_VICTORY_DURATION_TICKS,
};
use fixed::types::I16F16;

fn relic_settings() -> VictorySettings {
    VictorySettings {
        relic_victory: true,
        ..VictorySettings::conquest()
    }
}

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

/// Add a completed Monastery for `owner` near their Town Center
/// and return its building id.
fn add_monastery(sim: &mut Sim, owner: u8, id: u32) -> u32 {
    let (cx, cy) = tc_cell(sim, owner);
    let b = placed(sim, id, BuildingKind::Monastery, owner, (cx + 6, cy + 6));
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

/// Wire a garrison directly (the Monastery list + the relic's
/// pointer + its position at the Monastery's) — the state
/// `relic_step` maintains (P5d's helper).
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

/// Undo a garrison the way a retrieve does (the Monastery list
/// and the relic's pointer clear together; the relic is back on
/// the ground).
fn lift_garrison(sim: &mut Sim, building: u32, relic: u32) {
    let b = sim
        .buildings
        .iter_mut()
        .find(|b| b.id == building)
        .unwrap();
    b.relics.retain(|&id| id != relic);
    let r = sim.entities.iter_mut().find(|e| e.id == relic).unwrap();
    r.relic_garrisoned_in = None;
}

/// Wire a carry directly (P5d's helper): a monk of `owner`'s —
/// spawned for the fixture beside their Town Center — holds the
/// relic. Returns nothing; the relic is off the ground and in no
/// Monastery.
fn give_carry(sim: &mut Sim, relic: u32, monk: u32, owner: u8) {
    let (cx, cy) = tc_cell(sim, owner);
    let (x, y) = sim.grid().cell_center(cx + 3, cy + 3);
    sim.entities
        .push(crate::entity::Entity::spawned(monk, EntityKind::Monk, owner, x, y));
    let m = sim.entities.iter_mut().find(|e| e.id == monk).unwrap();
    m.carrying_relic = Some(relic);
    let r = sim.entities.iter_mut().find(|e| e.id == relic).unwrap();
    r.relic_carried_by = Some(monk);
    r.pos_x = x;
    r.pos_y = y;
}

fn settle(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

fn relic_match(seed: u64) -> Sim {
    Sim::new_match_victory(seed, MatchMode::TwoPlayer, relic_settings())
}

// ------------------------------------------------------------------

#[test]
fn duration_is_tiny_from_vic02s_table() {
    // 200 years (Tiny, current table) × 5 s/year × 60 ticks/s.
    assert_eq!(RELIC_VICTORY_DURATION_TICKS, 200 * 5 * 60);
}

#[test]
fn countdown_starts_only_with_all_relics_garrisoned() {
    let mut sim = relic_match(42);
    let monastery = add_monastery(&mut sim, OWNER_PLAYER, 900);
    let relics = relic_ids(&sim);
    assert_eq!(relics.len(), 5, "MAP-13 spawns five relics");
    // Four of five: a partial hold never starts the countdown.
    for &r in &relics[..4] {
        give_garrison(&mut sim, monastery, r);
    }
    settle(&mut sim, 120);
    assert_eq!(sim.relic_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    // The fifth completes the set: the countdown starts on the
    // next tick, at the full duration minus that tick.
    give_garrison(&mut sim, monastery, relics[4]);
    settle(&mut sim, 1);
    assert_eq!(
        sim.relic_countdown(),
        Some((OWNER_PLAYER, RELIC_VICTORY_DURATION_TICKS - 1))
    );
}

#[test]
fn countdown_ticks_down_while_the_hold_stands() {
    let mut sim = relic_match(42);
    let monastery = add_monastery(&mut sim, OWNER_PLAYER, 900);
    for r in relic_ids(&sim) {
        give_garrison(&mut sim, monastery, r);
    }
    settle(&mut sim, 600);
    assert_eq!(
        sim.relic_countdown(),
        Some((OWNER_PLAYER, RELIC_VICTORY_DURATION_TICKS - 600))
    );
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn countdown_cancels_when_a_relic_leaves_the_monastery() {
    let mut sim = relic_match(42);
    let monastery = add_monastery(&mut sim, OWNER_PLAYER, 900);
    let relics = relic_ids(&sim);
    for &r in &relics {
        give_garrison(&mut sim, monastery, r);
    }
    settle(&mut sim, 500);
    assert!(sim.relic_countdown().is_some());
    // A retrieve breaks the hold: the countdown is gone, not
    // paused — and completing the set again restarts it from
    // the full duration.
    lift_garrison(&mut sim, monastery, relics[0]);
    settle(&mut sim, 1);
    assert_eq!(sim.relic_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    give_garrison(&mut sim, monastery, relics[0]);
    settle(&mut sim, 1);
    assert_eq!(
        sim.relic_countdown(),
        Some((OWNER_PLAYER, RELIC_VICTORY_DURATION_TICKS - 1))
    );
}

#[test]
fn countdown_cancels_when_the_monastery_is_deleted() {
    // DeleteBuilding routes through the destruction path, which
    // drops the Monastery's whole relic store on the ground
    // (P5d) — the hold breaks with it.
    let mut sim = relic_match(42);
    let monastery = add_monastery(&mut sim, OWNER_PLAYER, 900);
    for r in relic_ids(&sim) {
        give_garrison(&mut sim, monastery, r);
    }
    settle(&mut sim, 300);
    assert!(sim.relic_countdown().is_some());
    sim.push_command(Command::DeleteBuilding { building: monastery });
    settle(&mut sim, 2);
    assert_eq!(sim.relic_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn countdown_expires_into_the_holders_win() {
    let mut sim = relic_match(42);
    let monastery = add_monastery(&mut sim, OWNER_PLAYER, 900);
    for r in relic_ids(&sim) {
        give_garrison(&mut sim, monastery, r);
    }
    settle(&mut sim, RELIC_VICTORY_DURATION_TICKS - 1);
    assert_eq!(sim.relic_countdown(), Some((OWNER_PLAYER, 1)));
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    settle(&mut sim, 1);
    assert_eq!(sim.relic_countdown(), Some((OWNER_PLAYER, 0)));
    assert_eq!(sim.match_state(), MatchState::Victory);
}

#[test]
fn countdown_expires_into_defeat_when_the_opponent_holds() {
    let mut sim = relic_match(42);
    let monastery = add_monastery(&mut sim, OWNER_OPPONENT, 901);
    for r in relic_ids(&sim) {
        give_garrison(&mut sim, monastery, r);
    }
    settle(&mut sim, RELIC_VICTORY_DURATION_TICKS);
    assert_eq!(sim.relic_countdown(), Some((OWNER_OPPONENT, 0)));
    assert_eq!(sim.match_state(), MatchState::Defeat);
}

#[test]
fn zero_relics_on_the_map_never_starts_a_countdown() {
    let mut sim = relic_match(42);
    add_monastery(&mut sim, OWNER_PLAYER, 900);
    sim.entities.retain(|e| e.kind != EntityKind::Relic);
    settle(&mut sim, 600);
    assert_eq!(sim.relic_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn ground_and_carried_relics_are_not_a_hold() {
    // All five on the ground (the opening state): no countdown.
    let mut sim = relic_match(42);
    add_monastery(&mut sim, OWNER_PLAYER, 900);
    settle(&mut sim, 120);
    assert_eq!(sim.relic_countdown(), None);
    // Four garrisoned and the fifth in a monk's hands: still not
    // a hold — the carried relic is in no Monastery.
    let monastery = 900;
    let relics = relic_ids(&sim);
    for &r in &relics[..4] {
        give_garrison(&mut sim, monastery, r);
    }
    give_carry(&mut sim, relics[4], 9_000, OWNER_PLAYER);
    settle(&mut sim, 120);
    assert_eq!(sim.relic_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn conquest_never_counts_down_even_with_all_relics_held() {
    let mut sim = Sim::new_match_victory(42, MatchMode::TwoPlayer, VictorySettings::conquest());
    let monastery = add_monastery(&mut sim, OWNER_PLAYER, 900);
    for r in relic_ids(&sim) {
        give_garrison(&mut sim, monastery, r);
    }
    settle(&mut sim, RELIC_VICTORY_DURATION_TICKS + 10);
    assert_eq!(sim.relic_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn sandbox_never_counts_down() {
    let mut sim = Sim::with_settings(42, MatchMode::Sandbox, relic_settings());
    let monastery = add_monastery(&mut sim, OWNER_PLAYER, 900);
    for r in relic_ids(&sim) {
        give_garrison(&mut sim, monastery, r);
    }
    settle(&mut sim, 600);
    assert_eq!(sim.relic_countdown(), None);
    assert_eq!(sim.match_state(), MatchState::Ongoing);
}

#[test]
fn countdown_state_is_hashed_and_deterministic() {
    let mut a = relic_match(42);
    let mut b = relic_match(42);
    for sim in [&mut a, &mut b] {
        let monastery = add_monastery(sim, OWNER_PLAYER, 900);
        for r in relic_ids(sim) {
            give_garrison(sim, monastery, r);
        }
    }
    settle(&mut a, 100);
    settle(&mut b, 100);
    assert_eq!(a.state_hash(), b.state_hash());
    // The accrued countdown is hashed state: the hash moves as
    // the count accrues, so two countdowns can never silently
    // agree (and a replay reproduces every tick of it).
    let h100 = a.state_hash();
    settle(&mut a, 100);
    assert_ne!(a.state_hash(), h100);
}

#[test]
fn countdown_round_trips_through_a_save() {
    let mut sim = relic_match(42);
    let monastery = add_monastery(&mut sim, OWNER_PLAYER, 900);
    for r in relic_ids(&sim) {
        give_garrison(&mut sim, monastery, r);
    }
    settle(&mut sim, 1_234);
    let before = sim.relic_countdown();
    assert!(before.is_some());
    let bytes = sim.save_game();
    let mut restored = Sim::load_game(&bytes).expect("the save restores");
    assert_eq!(restored.victory_settings(), relic_settings());
    assert_eq!(restored.relic_countdown(), before);
    // And both continue identically from the restore point.
    settle(&mut sim, 60);
    settle(&mut restored, 60);
    assert_eq!(restored.state_hash(), sim.state_hash());
}
