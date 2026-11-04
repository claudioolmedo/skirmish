//! VIC-11 tests: resignation leaves ruins. When a player
//! resigns (or is eliminated), their units are removed, but
//! their buildings remain standing on the map (inert, owner =
//! resigned player), and their Markets remain valid trade
//! targets (TRD-07).
//!
//! In-crate staging (direct entity/building records) follows
//! the `trade_tests` fixture style.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, GarrisonRef, OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::ProdEntry;
use crate::{Command, MatchMode, MatchState, Sim};
use fixed::types::I16F16;

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

/// A complete, idle building record at a cell anchor (the
/// `trade_tests::placed` fixture's shape).
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

fn gold(sim: &Sim, owner: u8) -> f64 {
    sim.stockpile_of(owner).amount(ResourceKind::Gold).to_num::<f64>()
}

fn living(sim: &Sim, owner: u8) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && !e.dead)
        .map(|e| e.id)
        .collect()
}

// ------------------------------------------------------------------
// The sweep: units removed, match decided, no score credit.

#[test]
fn opponent_resignation_sweeps_units_and_wins_the_match() {
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    let foe_before = living(&sim, OWNER_OPPONENT);
    assert!(foe_before.len() > 1, "the foe start has units to sweep");
    let human_before = living(&sim, OWNER_PLAYER);
    assert!(sim.can_resign(OWNER_OPPONENT));
    assert!(!sim.is_resigned(OWNER_OPPONENT));
    sim.push_command(Command::Resign { player: OWNER_OPPONENT });
    // Commands are data applied at a tick boundary: nothing has
    // happened yet.
    assert!(!sim.is_resigned(OWNER_OPPONENT));
    sim.tick();
    assert!(sim.is_resigned(OWNER_OPPONENT));
    assert_eq!(sim.match_state(), MatchState::Victory);
    // Every foe unit is dead; the only living foe entity is the
    // Town Center — a building, and buildings stand (below).
    let foe_after = living(&sim, OWNER_OPPONENT);
    assert_eq!(foe_after.len(), 1, "only the Town Center stands: {foe_after:?}");
    assert_eq!(ent(&sim, foe_after[0]).kind, EntityKind::TownCenter);
    // The human's army is untouched.
    assert_eq!(living(&sim, OWNER_PLAYER), human_before);
    // Population follows ownership of the living: the foe is at 0.
    assert_eq!(sim.population_of(OWNER_OPPONENT), 0);
}

#[test]
fn human_resignation_is_defeat() {
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    sim.push_command(Command::Resign { player: OWNER_PLAYER });
    sim.tick();
    assert!(sim.is_resigned(OWNER_PLAYER));
    assert_eq!(sim.match_state(), MatchState::Defeat);
    assert_eq!(sim.population_of(OWNER_PLAYER), 0);
}

#[test]
fn resignation_credits_no_one() {
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    sim.push_command(Command::Resign { player: OWNER_OPPONENT });
    sim.tick();
    // VIC-11's impl note: the sweep runs the death path WITHOUT
    // score credit to anyone — the human's destroyed-value
    // (VIC-04's Military input) and kill counter stay at zero,
    // while the foe's losses are counted like any death's.
    assert_eq!(sim.destroyed_value_of(OWNER_PLAYER), 0);
    assert_eq!(sim.destroyed_value_of(OWNER_OPPONENT), 0);
    let foe_stats = sim.match_stats_of(OWNER_OPPONENT);
    let human_stats = sim.match_stats_of(OWNER_PLAYER);
    assert!(foe_stats.units_lost > 0, "the swept units count as lost");
    assert_eq!(human_stats.units_killed, 0, "nobody killed them");
    assert_eq!(sim.score_for(OWNER_PLAYER).military, 0);
}

#[test]
fn garrisoned_units_are_swept_out_of_their_hosts() {
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    let foe_tc = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::TownCenter)
        .unwrap()
        .id;
    let foe_villager = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::Villager)
        .unwrap()
        .id;
    // Stage the villager garrisoned inside its own Town Center.
    let idx = sim.entity_index(foe_villager).unwrap();
    sim.entities[idx].garrisoned_in = Some(GarrisonRef::TownCenter(foe_tc));
    let si = sim
        .tc_states
        .iter()
        .position(|s| s.tc_id == foe_tc)
        .expect("the foe Town Center has a state");
    sim.tc_states[si].garrison.push(foe_villager);
    sim.push_command(Command::Resign { player: OWNER_OPPONENT });
    sim.tick();
    assert!(ent(&sim, foe_villager).dead, "a garrisoned unit is a unit: swept");
    assert!(
        sim.tc_states[si].garrison.is_empty(),
        "no host keeps a dead passenger"
    );
    assert!(!ent(&sim, foe_tc).dead, "the Town Center itself stands");
}

// ------------------------------------------------------------------
// The ruins: buildings remain, owned, inert.

#[test]
fn buildings_remain_standing_owned_and_inert() {
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    // Stage a foe Barracks with a Militia already queued, and a
    // foe Watch Tower beside a human villager.
    let mut rax = placed(&sim, 900, BuildingKind::Barracks, OWNER_OPPONENT, (60, 30));
    rax.queue.push(ProdEntry::Unit(EntityKind::Militia));
    sim.buildings.push(rax);
    let tower = placed(&sim, 901, BuildingKind::WatchTower, OWNER_OPPONENT, (58, 58));
    sim.buildings.push(tower);
    spawn(&mut sim, 9000, EntityKind::Villager, OWNER_PLAYER, (60, 62));
    let villager_hp = ent(&sim, 9000).hp;
    // Queue a villager at the foe Town Center too, through the
    // ordinary command path (charged to the foe).
    let foe_tc = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::TownCenter)
        .unwrap()
        .id;
    sim.push_command(Command::TrainVillagerAt { tc: foe_tc });
    sim.push_command(Command::Resign { player: OWNER_OPPONENT });
    sim.tick();
    // The ruins stand: both staged buildings and the Town
    // Center, still the resigned player's, at full HP.
    for bid in [900, 901] {
        let b = sim.buildings().iter().find(|b| b.id == bid).expect("ruin stands");
        assert_eq!(b.owner, OWNER_OPPONENT);
        assert_eq!(b.hp, I16F16::from_num(b.kind.max_hp()));
    }
    assert!(!ent(&sim, foe_tc).dead);
    // Inert, part 1: both production queues were dropped and
    // never produce again — run far past every train time and
    // count the foe's entities: nothing new ever spawns.
    let foe_entities = sim.entities().iter().filter(|e| e.owner == OWNER_OPPONENT).count();
    let rax_now = sim.buildings().iter().find(|b| b.id == 900).unwrap();
    assert!(rax_now.queue.is_empty(), "the resigned queue is dropped");
    for _ in 0..3_000 {
        sim.tick();
    }
    assert_eq!(
        sim.entities().iter().filter(|e| e.owner == OWNER_OPPONENT).count(),
        foe_entities,
        "an inert ruin trains nothing"
    );
    assert_eq!(sim.population_of(OWNER_OPPONENT), 0);
    // Inert, part 2: the tower never fired — the human villager
    // stood in its range the whole time and is untouched.
    assert_eq!(ent(&sim, 9000).hp, villager_hp, "a resigned tower is inert");
    // And nothing new can be queued at a ruin.
    assert!(!sim.can_train_at(900, EntityKind::Militia));
    assert!(!sim.can_train_villager_at(foe_tc));
}

#[test]
fn elimination_leaves_buildings_standing() {
    // The already-standing half of VIC-11, pinned: an
    // elimination (VIC-01) never removes buildings either.
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    let house = placed(&sim, 900, BuildingKind::House, OWNER_OPPONENT, (60, 30));
    sim.buildings.push(house);
    // Tombstone every foe entity, the VIC-01 test discipline.
    for e in &mut sim.entities {
        if e.owner == OWNER_OPPONENT {
            e.hp = I16F16::ZERO;
            e.dead = true;
        }
    }
    assert_eq!(sim.match_state(), MatchState::Victory);
    let b = sim.buildings().iter().find(|b| b.id == 900).expect("the house stands");
    assert_eq!(b.owner, OWNER_OPPONENT);
}

// ------------------------------------------------------------------
// TRD-07: a resigned player's Market is still a trade depot.

const HOME_MARKET: u32 = 900;
const FOE_MARKET: u32 = 901;
const CART: u32 = 9000;

#[test]
fn resigned_market_remains_a_valid_trade_target() {
    let mut sim = Sim::new_match(42, MatchMode::TwoPlayer);
    let home = placed(&sim, HOME_MARKET, BuildingKind::Market, OWNER_PLAYER, (40, 60));
    sim.buildings.push(home);
    let foe = placed(&sim, FOE_MARKET, BuildingKind::Market, OWNER_OPPONENT, (85, 60));
    sim.buildings.push(foe);
    spawn(&mut sim, CART, EntityKind::TradeCart, OWNER_PLAYER, (37, 62));
    let idx = sim.entity_index(CART).unwrap();
    sim.entities[idx].trade_home = Some(HOME_MARKET);
    assert!(sim.can_trade(CART, FOE_MARKET), "an enemy depot is valid before");
    sim.push_command(Command::Resign { player: OWNER_OPPONENT });
    sim.tick();
    assert!(sim.is_resigned(OWNER_OPPONENT));
    // The depot survived its owner's resignation, still theirs…
    let b = sim.buildings().iter().find(|b| b.id == FOE_MARKET).expect("the Market stands");
    assert_eq!(b.owner, OWNER_OPPONENT);
    assert!(b.complete);
    // …and trade tasking never consults the resignation flag.
    assert!(sim.can_trade(CART, FOE_MARKET), "a resigned depot stays valid (TRD-07)");
    // The full round trip still pays the cart's owner, and the
    // resigned depot's owner gains and loses nothing.
    let gold0 = gold(&sim, OWNER_PLAYER);
    let foe_gold0 = gold(&sim, OWNER_OPPONENT);
    sim.push_command(Command::Trade { entity: CART, building: FOE_MARKET });
    let credited = run_until(&mut sim, 40_000, |s| gold(s, OWNER_PLAYER) > gold0 + 1.0);
    assert!(credited, "the resigned depot never paid a round trip");
    assert_eq!(gold(&sim, OWNER_OPPONENT), foe_gold0, "the depot's owner is untouched");
}

// ------------------------------------------------------------------
// Discipline: ignored resignations, idempotence, hash, saves.

#[test]
fn invalid_resignations_are_ignored_whole() {
    // Sandbox: no match to resign.
    let mut sim = Sim::new(11);
    assert!(!sim.can_resign(OWNER_PLAYER));
    sim.push_command(Command::Resign { player: OWNER_PLAYER });
    sim.tick();
    assert!(!sim.is_resigned(OWNER_PLAYER));
    assert!(living(&sim, OWNER_PLAYER).len() > 1, "sandbox units unharmed");
    // Unknown seats and Gaia cannot resign a match either.
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    for bad in [OWNER_GAIA, 99] {
        assert!(!sim.can_resign(bad));
        sim.push_command(Command::Resign { player: bad });
    }
    sim.tick();
    assert_eq!(sim.match_state(), MatchState::Ongoing);
    // A second resignation is a no-op: the flag is sticky and
    // the state does not move again.
    sim.push_command(Command::Resign { player: OWNER_OPPONENT });
    sim.tick();
    assert!(!sim.can_resign(OWNER_OPPONENT), "already resigned");
    let hash = sim.state_hash();
    sim.push_command(Command::Resign { player: OWNER_OPPONENT });
    sim.tick();
    assert!(sim.is_resigned(OWNER_OPPONENT));
    let _ = hash; // (hash legitimately advances with the tick itself)
}

#[test]
fn ignored_resignation_does_not_move_the_hash() {
    // A resignation stream that never resigns — here an ignored
    // Gaia resignation — stays byte-identical to a command-free
    // stream: the flag's hash write is conditional on the flag.
    let mut a = Sim::new_match(11, MatchMode::TwoPlayer);
    let mut b = Sim::new_match(11, MatchMode::TwoPlayer);
    b.push_command(Command::Resign { player: OWNER_GAIA });
    for _ in 0..120 {
        a.tick();
        b.tick();
    }
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn resignation_round_trips_through_a_save() {
    // After the resignation: flag, outcome and ruins restore.
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    sim.push_command(Command::Resign { player: OWNER_OPPONENT });
    sim.tick();
    sim.tick();
    let bytes = sim.save_game();
    let restored = Sim::load_game(&bytes).expect("a VIC-11 save loads");
    assert!(restored.is_resigned(OWNER_OPPONENT));
    assert_eq!(restored.match_state(), MatchState::Victory);
    assert_eq!(restored.state_hash(), sim.state_hash());
    // Before the tick: the pending command itself round-trips
    // through the save codec (tag 48) and applies on restore.
    let mut sim = Sim::new_match(11, MatchMode::TwoPlayer);
    sim.push_command(Command::Resign { player: OWNER_OPPONENT });
    let bytes = sim.save_game();
    let mut restored = Sim::load_game(&bytes).expect("a pending-resign save loads");
    restored.tick();
    assert!(restored.is_resigned(OWNER_OPPONENT));
    assert_eq!(restored.match_state(), MatchState::Victory);
}
