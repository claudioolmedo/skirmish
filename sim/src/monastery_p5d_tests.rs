//! Castle P5d tests (2026-11-02): RELICS — the generator's five
//! relics (MAP-13: two mirrored pairs + an equal-distance
//! center relic, exactly fair distance multisets, contested
//! ground only), the relic entity (kind tag 21, Gaia-owned,
//! indestructible: no attack path names it), the Monk's pickup
//! / carry / deposit / retrieve lifecycle (MNK-09: LoS 11 → 9
//! while carrying, conversion refused while carrying, healing
//! NOT excluded), the ECO-16 trickle (30 gold/min per
//! garrisoned relic, exact over any whole minute, uncapped
//! store), carrier death dropping the relic where he died, a
//! converted carrier keeping his relic, a destroyed Monastery
//! dropping its store, the save round-trip at SAVE_VERSION 10,
//! replay bit-identity of a scripted relic command stream, and
//! the scripted AI's fetch-and-deposit slice (§3.7).
//!
//! Values: RULEBOOK MNK-09 / ECO-16 / MAP-13 (created in this
//! commit) and the P5 execution plan §1.4/§3.5. Fixture style
//! follows `monastery_p5a_tests` (direct records; ages set on
//! the player slots).

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_GAIA, OWNER_OPPONENT, OWNER_PLAYER};
use crate::mapgen;
use crate::monks::RelicState;
use crate::tech::Age;
use crate::{Command, MatchMode, Sim};
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

fn sim_in_castle() -> Sim {
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Castle;
    sim.players[1].age = Age::Castle;
    sim
}

/// The player's Town Center cell — fixtures anchor on the
/// flattened, cleared start ground around it.
fn tc_cell(sim: &Sim) -> (u32, u32) {
    let tc = ent(sim, 0);
    sim.grid().world_to_cell(tc.pos_x, tc.pos_y)
}

fn gold_bits(sim: &Sim, owner: u8) -> i64 {
    sim.stockpile_of(owner).amount(ResourceKind::Gold).to_bits() as i64
}

/// Wire a carry directly (the two pointers + the monk's field,
/// positions synced) — the state `relic_step` maintains.
fn give_carry(sim: &mut Sim, monk: u32, relic: u32) {
    let (mx, my) = {
        let m = ent(sim, monk);
        (m.pos_x, m.pos_y)
    };
    let m = sim.entities.iter_mut().find(|e| e.id == monk).unwrap();
    m.carrying_relic = Some(relic);
    let r = sim.entities.iter_mut().find(|e| e.id == relic).unwrap();
    r.relic_carried_by = Some(monk);
    r.pos_x = mx;
    r.pos_y = my;
}

/// Wire a garrison directly (the Monastery list + the relic's
/// pointer + its position at the Monastery's).
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
// The row, the placement, the spawn.

#[test]
fn relic_row_is_a_gaia_prop_with_no_combat_identity() {
    assert_eq!(EntityKind::Relic.tag(), 21);
    assert_eq!(EntityKind::from_tag(21), Some(EntityKind::Relic));
    let spec = crate::units::spec(EntityKind::Relic);
    assert!(spec.attacks.is_empty(), "CLS-08's sibling: no attack table");
    assert_eq!(spec.speed, I16F16::ZERO, "a relic never walks");
    assert!(!EntityKind::Relic.is_mobile());
}

#[test]
fn mapgen_places_five_relics_exactly_fair() {
    for seed in [1u64, 2, 3, 7, 42, 99, 1234, 55555] {
        let map = mapgen::generate(seed);
        assert!(mapgen::validate(&map), "seed {seed} must validate");
        assert_eq!(map.relics.len(), 5, "seed {seed}: exactly five relics");
        let (ta, tb) = (map.starts[0].tc_cell, map.starts[1].tc_cell);
        let cheb = |c: (u32, u32), t: (u32, u32)| {
            c.0.abs_diff(t.0).max(c.1.abs_diff(t.1))
        };
        let mut da: Vec<u32> = map.relics.iter().map(|&c| cheb(c, ta)).collect();
        let mut db: Vec<u32> = map.relics.iter().map(|&c| cheb(c, tb)).collect();
        // MAP-13's fairness predicate: identical distance
        // multisets to the two starts.
        da.sort_unstable();
        db.sort_unstable();
        assert_eq!(da, db, "seed {seed}: distance multisets must match");
        // Contested ground only: no relic inside a start's
        // immediate base area, and no two relics share a cell.
        for &c in &map.relics {
            assert!(cheb(c, ta) >= 20, "seed {seed}: relic too close to A");
            assert!(cheb(c, tb) >= 20, "seed {seed}: relic too close to B");
        }
        let mut cells = map.relics.clone();
        cells.sort_unstable();
        cells.dedup();
        assert_eq!(cells.len(), 5, "seed {seed}: relic cells are distinct");
    }
}

#[test]
fn mapgen_relic_placement_is_deterministic() {
    for seed in [7u64, 42, 2026] {
        let a = mapgen::generate(seed);
        let b = mapgen::generate(seed);
        assert_eq!(a.relics, b.relics, "seed {seed}: same seed, same relics");
    }
}

#[test]
fn sim_spawns_the_five_relics_as_gaia_ground_objects() {
    let sim = Sim::new(42);
    let map = mapgen::generate(42);
    let states = sim.relic_states();
    assert_eq!(states.len(), 5);
    for (i, (id, state)) in states.iter().enumerate() {
        assert_eq!(*state, RelicState::Ground);
        let e = ent(&sim, *id);
        assert_eq!(e.kind, EntityKind::Relic);
        assert_eq!(e.owner, OWNER_GAIA);
        // Spawned last: ids follow both starts' populations,
        // on the generator's cells, in placement order.
        let (cx, cy) = sim.grid().world_to_cell(e.pos_x, e.pos_y);
        assert_eq!((cx, cy), map.relics[i]);
    }
}

// ------------------------------------------------------------------
// Pickup and the carry.

#[test]
fn monk_walks_to_a_relic_and_picks_it_up() {
    let mut sim = sim_in_castle();
    let tc = tc_cell(&sim);
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (tc.0 + 5, tc.1));
    spawn(&mut sim, 901, EntityKind::Relic, OWNER_GAIA, (tc.0 + 13, tc.1));
    assert!(sim.can_pickup_relic(900, 901));
    sim.push_command(Command::PickupRelic {
        entity: 900,
        relic: 901,
    });
    let got = run_until(&mut sim, 6_000, |s| s.carrying_relic(900) == Some(901));
    assert!(got, "the monk never reached the relic");
    assert_eq!(sim.relic_state(901), Some(RelicState::CarriedBy(900)));
    // The carry rides at the monk: order him away and the
    // relic's position tracks his, tick by tick.
    let (mx, my) = (ent(&sim, 900).pos_x, ent(&sim, 900).pos_y);
    let (tx, ty) = sim.grid().cell_center(tc.0 + 5, tc.1 + 8);
    sim.push_command(Command::Move {
        entity: 900,
        x: tx,
        y: ty,
    });
    for _ in 0..30 {
        sim.tick();
    }
    let m = ent(&sim, 900);
    assert!(m.pos_x != mx || m.pos_y != my, "the monk should be walking");
    let r = ent(&sim, 901);
    assert_eq!((r.pos_x, r.pos_y), (m.pos_x, m.pos_y), "the relic rides at its monk");
}

#[test]
fn pickup_refusals_hold() {
    let mut sim = sim_in_castle();
    let tc = tc_cell(&sim);
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (tc.0 + 5, tc.1));
    spawn(&mut sim, 902, EntityKind::Monk, OWNER_PLAYER, (tc.0 + 6, tc.1));
    spawn(&mut sim, 901, EntityKind::Relic, OWNER_GAIA, (tc.0 + 7, tc.1));
    spawn(&mut sim, 903, EntityKind::Relic, OWNER_GAIA, (tc.0 + 8, tc.1));
    // A non-monk cannot pick up; a monk cannot take a relic
    // another monk carries; a carrying monk cannot take a second.
    spawn(&mut sim, 904, EntityKind::Villager, OWNER_PLAYER, (tc.0 + 5, tc.1 + 1));
    assert!(!sim.can_pickup_relic(904, 901), "villagers do not fetch relics");
    give_carry(&mut sim, 900, 901);
    assert!(!sim.can_pickup_relic(902, 901), "a carried relic is taken");
    assert!(!sim.can_pickup_relic(900, 903), "one relic per monk");
    // A relic garrisoned in the ENEMY's Monastery is not
    // retrievable by us — only the owner's monks may lift it out.
    sim.buildings.push(placed(
        &sim,
        900,
        BuildingKind::Monastery,
        OWNER_OPPONENT,
        (tc.0 + 20, tc.1 + 10),
    ));
    give_garrison(&mut sim, 900, 903);
    assert!(!sim.can_pickup_relic(902, 903), "the enemy's store is not ours to raid");
    spawn(&mut sim, 905, EntityKind::Monk, OWNER_OPPONENT, (tc.0 + 19, tc.1 + 10));
    assert!(sim.can_pickup_relic(905, 903), "the owner's monk may retrieve it");
}

#[test]
fn carrying_restrictions_los_convert_refused_heal_allowed() {
    let mut sim = sim_in_castle();
    let tc = tc_cell(&sim);
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (tc.0 + 5, tc.1));
    spawn(&mut sim, 901, EntityKind::Relic, OWNER_GAIA, (tc.0 + 6, tc.1));
    // An enemy villager in conversion range and a wounded ally
    // beside him: before the pickup, both orders validate.
    spawn(&mut sim, 910, EntityKind::Villager, OWNER_OPPONENT, (tc.0 + 7, tc.1));
    spawn(&mut sim, 911, EntityKind::Villager, OWNER_PLAYER, (tc.0 + 5, tc.1 + 1));
    {
        let v = sim.entities.iter_mut().find(|e| e.id == 911).unwrap();
        v.hp = I16F16::from_num(10);
    }
    let idx = sim.entity_index(900).unwrap();
    assert_eq!(sim.unit_stats(idx).los, I16F16::from_num(44), "base monk LoS 11");
    assert!(sim.can_convert(900, 910), "empty-handed, conversion is open");
    assert!(sim.can_heal(900, 911));
    give_carry(&mut sim, 900, 901);
    // MNK-09: LoS 9 while carrying; conversion refused;
    // healing NOT excluded (the plan §1.4 names no heal ban).
    assert_eq!(sim.unit_stats(idx).los, I16F16::from_num(36), "carrying LoS 9");
    assert!(!sim.can_convert(900, 910), "a carrying monk delivers, not converts");
    assert!(sim.can_heal(900, 911), "a carrying monk still heals");
    // And he actually heals while carrying: the auto-task and
    // the accumulator run with the relic in hand.
    let healed = run_until(&mut sim, 3_000, |s| ent(s, 911).hp > I16F16::from_num(10));
    assert!(healed, "the carrying monk never healed his ally");
    assert_eq!(sim.carrying_relic(900), Some(901), "healing never drops the relic");
}

#[test]
fn a_relic_cannot_be_attacked() {
    let mut sim = sim_in_castle();
    let tc = tc_cell(&sim);
    spawn(&mut sim, 900, EntityKind::Militia, OWNER_PLAYER, (tc.0 + 5, tc.1));
    spawn(&mut sim, 901, EntityKind::Relic, OWNER_GAIA, (tc.0 + 6, tc.1));
    assert!(!sim.can_attack(900, 901), "MNK-09: relics are indestructible");
}

// ------------------------------------------------------------------
// Deposit, the trickle, retrieve.

/// Fixture: a carrying monk six tiles from his own Monastery.
fn carrying_monk_and_monastery() -> (Sim, u32, u32, u32) {
    let mut sim = sim_in_castle();
    let tc = tc_cell(&sim);
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (tc.0 + 5, tc.1));
    spawn(&mut sim, 901, EntityKind::Relic, OWNER_GAIA, (tc.0 + 6, tc.1));
    sim.buildings.push(placed(
        &sim,
        900,
        BuildingKind::Monastery,
        OWNER_PLAYER,
        (tc.0 + 12, tc.1),
    ));
    give_carry(&mut sim, 900, 901);
    (sim, 900, 901, 900)
}

#[test]
fn deposit_garrisons_and_the_trickle_is_exact() {
    let (mut sim, monk, relic, monastery) = carrying_monk_and_monastery();
    assert!(sim.can_deposit_relic(monk, monastery));
    let g0 = gold_bits(&sim, OWNER_PLAYER);
    sim.push_command(Command::DepositRelic {
        entity: monk,
        building: monastery,
    });
    let done = run_until(&mut sim, 6_000, |s| {
        s.relic_state(relic) == Some(RelicState::GarrisonedIn(monastery))
    });
    assert!(done, "the monk never delivered the relic");
    assert_eq!(sim.carrying_relic(monk), None);
    assert_eq!(sim.monastery_relic_count(monastery), 1);
    // ECO-16: 0.5 gold/s — exactly 15 gold in 30 s, exactly 30
    // in the full minute, no drift across minutes. (The walk
    // here happened before the first garrisoned tick; measure
    // from the deposit tick onward.)
    let g1 = gold_bits(&sim, OWNER_PLAYER);
    for _ in 0..1_800 {
        sim.tick();
    }
    assert_eq!(
        gold_bits(&sim, OWNER_PLAYER) - g1,
        15 * 65_536,
        "30 s of one relic pays exactly 15 gold"
    );
    for _ in 0..1_800 {
        sim.tick();
    }
    assert_eq!(
        gold_bits(&sim, OWNER_PLAYER) - g1,
        30 * 65_536,
        "60 s of one relic pays exactly 30 gold"
    );
    for _ in 0..3_600 {
        sim.tick();
    }
    assert_eq!(
        gold_bits(&sim, OWNER_PLAYER) - g1,
        60 * 65_536,
        "the second minute pays exactly 30 more — no drift"
    );
    let _ = g0;
}

#[test]
fn retrieve_stops_the_trickle_and_the_store_is_uncapped() {
    let (mut sim, monk, relic, monastery) = carrying_monk_and_monastery();
    // Two more relics straight into the same store: MNK-09's
    // capacity is uncapped (its own rule note — "garrison 10"
    // is the Monastery's production shelter, not a relic cap).
    let tc = tc_cell(&sim);
    spawn(&mut sim, 902, EntityKind::Relic, OWNER_GAIA, (tc.0 + 20, tc.1 + 4));
    spawn(&mut sim, 903, EntityKind::Relic, OWNER_GAIA, (tc.0 + 21, tc.1 + 4));
    // The fixture wired relic 901 as the monk's CARRY; hand it
    // into the store properly first (clear the carry pair, as
    // the save test does) — the monk must be empty-handed for
    // the retrieve below to be a legal order at all.
    {
        let m = sim.entities.iter_mut().find(|e| e.id == monk).unwrap();
        m.carrying_relic = None;
        let r = sim.entities.iter_mut().find(|e| e.id == relic).unwrap();
        r.relic_carried_by = None;
    }
    give_garrison(&mut sim, monastery, relic);
    give_garrison(&mut sim, monastery, 902);
    give_garrison(&mut sim, monastery, 903);
    assert_eq!(sim.monastery_relic_count(monastery), 3);
    let g0 = gold_bits(&sim, OWNER_PLAYER);
    for _ in 0..3_600 {
        sim.tick();
    }
    assert_eq!(
        gold_bits(&sim, OWNER_PLAYER) - g0,
        90 * 65_536,
        "three relics pay 90 gold a minute"
    );
    // Retrieve one: the (now empty-handed) monk lifts it out,
    // and the trickle drops to the remaining two.
    assert!(sim.can_pickup_relic(monk, relic));
    sim.push_command(Command::PickupRelic {
        entity: monk,
        relic,
    });
    let got = run_until(&mut sim, 6_000, |s| s.carrying_relic(monk) == Some(relic));
    assert!(got, "the retrieve never completed");
    assert_eq!(sim.monastery_relic_count(monastery), 2);
    let g1 = gold_bits(&sim, OWNER_PLAYER);
    for _ in 0..3_600 {
        sim.tick();
    }
    assert_eq!(
        gold_bits(&sim, OWNER_PLAYER) - g1,
        60 * 65_536,
        "two relics pay 60 a minute after the retrieve"
    );
}

// ------------------------------------------------------------------
// Death, conversion, destruction — the relic outlives hands and
// buildings.

#[test]
fn carrier_death_drops_the_relic_where_he_died() {
    let mut sim = sim_in_castle();
    let tc = tc_cell(&sim);
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (tc.0 + 5, tc.1));
    spawn(&mut sim, 901, EntityKind::Relic, OWNER_GAIA, (tc.0 + 6, tc.1));
    give_carry(&mut sim, 900, 901);
    let (px, py) = (ent(&sim, 900).pos_x, ent(&sim, 900).pos_y);
    {
        let m = sim.entities.iter_mut().find(|e| e.id == 900).unwrap();
        m.hp = I16F16::ZERO;
    }
    sim.tick();
    assert!(ent(&sim, 900).dead);
    assert_eq!(sim.relic_state(901), Some(RelicState::Ground));
    let r = ent(&sim, 901);
    assert_eq!((r.pos_x, r.pos_y), (px, py), "the relic drops at the death position");
    // Anyone's monk may take it now — including the enemy's.
    spawn(&mut sim, 905, EntityKind::Monk, OWNER_OPPONENT, (tc.0 + 6, tc.1 + 1));
    assert!(sim.can_pickup_relic(905, 901));
    sim.push_command(Command::PickupRelic {
        entity: 905,
        relic: 901,
    });
    let taken = run_until(&mut sim, 3_000, |s| s.carrying_relic(905) == Some(901));
    assert!(taken, "the enemy monk never took the dropped relic");
}

#[test]
fn a_converted_carrier_keeps_his_relic() {
    let mut sim = sim_in_castle();
    let tc = tc_cell(&sim);
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (tc.0 + 5, tc.1));
    spawn(&mut sim, 901, EntityKind::Relic, OWNER_GAIA, (tc.0 + 6, tc.1));
    give_carry(&mut sim, 900, 901);
    // MNK-09: conversion changes the monk's owner, never the
    // relic's state — its carrier pointer just names him.
    sim.transfer_ownership(900, OWNER_OPPONENT);
    assert_eq!(ent(&sim, 900).owner, OWNER_OPPONENT);
    assert_eq!(sim.carrying_relic(900), Some(901));
    assert_eq!(sim.relic_state(901), Some(RelicState::CarriedBy(900)));
}

#[test]
fn a_destroyed_monastery_drops_its_relics() {
    let (mut sim, _monk, relic, monastery) = carrying_monk_and_monastery();
    let tc = tc_cell(&sim);
    spawn(&mut sim, 902, EntityKind::Relic, OWNER_GAIA, (tc.0 + 22, tc.1 + 4));
    give_garrison(&mut sim, monastery, relic);
    give_garrison(&mut sim, monastery, 902);
    assert_eq!(sim.monastery_relic_count(monastery), 2);
    let (bx, by) = {
        let b = sim.buildings().iter().find(|b| b.id == monastery).unwrap();
        (b.pos_x, b.pos_y)
    };
    sim.destroy_building(monastery);
    assert!(sim.building_index(monastery).is_none());
    for r in [relic, 902] {
        assert_eq!(sim.relic_state(r), Some(RelicState::Ground));
        let e = ent(&sim, r);
        assert_eq!((e.pos_x, e.pos_y), (bx, by), "the store drops at the Monastery");
    }
}

// ------------------------------------------------------------------
// Save / replay.

#[test]
fn save_load_round_trips_relic_state() {
    let (mut sim, monk, relic, monastery) = carrying_monk_and_monastery();
    // Hand the fixture's carried relic into the store (clear
    // the carry pair first, as a real deposit would).
    {
        let m = sim.entities.iter_mut().find(|e| e.id == monk).unwrap();
        m.carrying_relic = None;
        let r = sim.entities.iter_mut().find(|e| e.id == relic).unwrap();
        r.relic_carried_by = None;
    }
    give_garrison(&mut sim, monastery, relic);
    // A second relic carried, and a trickle mid-flight
    // (remainder non-zero) so the v10 building record is
    // exercised, not just the entity one.
    let tc = tc_cell(&sim);
    spawn(&mut sim, 902, EntityKind::Relic, OWNER_GAIA, (tc.0 + 8, tc.1 + 2));
    give_carry(&mut sim, monk, 902);
    sim.buildings
        .iter_mut()
        .find(|b| b.id == monastery)
        .unwrap()
        .relic_gold_remainder = 1_234;
    let bytes = sim.save_game();
    let restored = Sim::load_game(&bytes).expect("a v10 save we wrote must load");
    assert_eq!(restored.relic_state(relic), Some(RelicState::GarrisonedIn(monastery)));
    assert_eq!(restored.carrying_relic(monk), Some(902));
    assert_eq!(restored.monastery_relic_count(monastery), 1);
    let b = restored
        .buildings()
        .iter()
        .find(|b| b.id == monastery)
        .unwrap();
    assert_eq!(b.relic_gold_remainder, 1_234);
    // The restored sim keeps trickling identically.
    let mut restored = restored;
    let g0 = gold_bits(&restored, OWNER_PLAYER);
    for _ in 0..3_600 {
        restored.tick();
    }
    let g1 = gold_bits(&sim, OWNER_PLAYER);
    for _ in 0..3_600 {
        sim.tick();
    }
    assert_eq!(
        gold_bits(&restored, OWNER_PLAYER) - g0,
        gold_bits(&sim, OWNER_PLAYER) - g1,
        "original and restored trickle identically"
    );
    let _ = monk;
}

#[test]
fn scripted_relic_stream_replays_bit_identically() {
    fn fixture() -> Sim {
        let mut sim = sim_in_castle();
        let tc = tc_cell(&sim);
        spawn(&mut sim, 900, EntityKind::Monk, OWNER_PLAYER, (tc.0 + 5, tc.1));
        spawn(&mut sim, 901, EntityKind::Relic, OWNER_GAIA, (tc.0 + 10, tc.1));
        sim.buildings.push(placed(
            &sim,
            900,
            BuildingKind::Monastery,
            OWNER_PLAYER,
            (tc.0 + 16, tc.1),
        ));
        sim
    }
    let mut a = fixture();
    let mut b = fixture();
    let mut phase = 0;
    for i in 0..6_000u64 {
        if i == 10 {
            for s in [&mut a, &mut b] {
                s.push_command(Command::PickupRelic {
                    entity: 900,
                    relic: 901,
                });
            }
        }
        if phase == 0 && a.carrying_relic(900) == Some(901) {
            phase = 1;
            for s in [&mut a, &mut b] {
                s.push_command(Command::DepositRelic {
                    entity: 900,
                    building: 900,
                });
            }
        }
        if phase == 1 && a.relic_state(901) == Some(RelicState::GarrisonedIn(900)) {
            phase = 2;
            for s in [&mut a, &mut b] {
                s.push_command(Command::PickupRelic {
                    entity: 900,
                    relic: 901,
                });
            }
        }
        let ha = a.tick();
        let hb = b.tick();
        assert_eq!(ha, hb, "hash divergence at step {i}");
    }
    assert_eq!(phase, 2, "the script ran pickup → deposit → retrieve");
    assert_eq!(a.carrying_relic(900), Some(901));
}

// ------------------------------------------------------------------
// The AI slice (§3.7): fetch the nearest known relic and bring
// it home.

#[test]
fn ai_monk_fetches_a_relic_and_deposits_it() {
    let mut sim = Sim::new_match(7, MatchMode::AiNormal);
    sim.players[1].age = Age::Castle;
    for kind in [
        ResourceKind::Food,
        ResourceKind::Wood,
        ResourceKind::Gold,
        ResourceKind::Stone,
    ] {
        sim.stockpile_mut(OWNER_OPPONENT).add(kind, I16F16::from_num(5000));
    }
    let monk = run_until(&mut sim, 600_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Monk && e.owner == OWNER_OPPONENT && !e.dead)
            && s.buildings().iter().any(|b| {
                b.owner == OWNER_OPPONENT && b.kind == BuildingKind::Monastery && b.complete
            })
    });
    assert!(monk, "the Castle-Age AI never fielded a Monk with a Monastery");
    // Lay one of the map's real relics beside the AI
    // Monastery — in plain sight of every monk the moment he
    // exits training, so the AI legitimately knows it
    // (plan_relic_fetch reads only what the AI can see; a relic
    // laid beside a monk mid-rally-walk would fall out of his
    // fog as he walks away, and the AI would rightly never
    // fetch what it cannot see).
    let (bx, by) = {
        let b = sim
            .buildings()
            .iter()
            .find(|b| {
                b.owner == OWNER_OPPONENT && b.kind == BuildingKind::Monastery && b.complete
            })
            .unwrap();
        (b.pos_x, b.pos_y)
    };
    let relic_id = sim.relic_states()[0].0;
    {
        let r = sim.entities.iter_mut().find(|e| e.id == relic_id).unwrap();
        r.pos_x = bx + I16F16::from_num(8);
        r.pos_y = by;
    }
    let home = run_until(&mut sim, 240_000, |s| {
        matches!(s.relic_state(relic_id), Some(RelicState::GarrisonedIn(b))
            if s.buildings().iter().any(|bb| bb.id == b && bb.owner == OWNER_OPPONENT))
    });
    assert!(home, "the AI monk never fetched and deposited the relic");
}

