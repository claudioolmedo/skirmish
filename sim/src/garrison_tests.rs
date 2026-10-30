//! Castle P3b tests (2026-10-29): the GAR core — garrison
//! capacities and the resolved per-building eligibility column
//! (GAR-01), untouchable garrisoned units (GAR-02), ejection
//! incl. the ≤20% HP rule and its re-garrison block (GAR-03),
//! shelter healing base rates (GAR-04), arrow scaling and caps
//! (GAR-05), the Town Center firing only while garrisoned
//! (GAR-06), the Town Bell snapshot/restore (GAR-07), and
//! rally-garrison incl. production garrison (GAR-08).
//!
//! Fixture style follows `castle_p3_tests` (direct records);
//! expectations are the scope/plan DE values, not recomputed ones.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, GarrisonRef, OWNER_OPPONENT, OWNER_PLAYER};
use crate::{Command, Sim};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

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
        build_rate_remainder: 0,
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
    }
}

fn own_villager(sim: &Sim) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
        .unwrap()
        .id
}

// ------------------------------------------------------------------
// GAR-01: the eligibility data column.

#[test]
fn gar01_eligibility_and_capacities_match_the_resolved_column() {
    // Capacities: towers 5, Castle 20, production 10, others none.
    assert_eq!(BuildingKind::WatchTower.garrison_capacity(), Some(5));
    assert_eq!(BuildingKind::Outpost.garrison_capacity(), Some(5));
    assert_eq!(BuildingKind::Castle.garrison_capacity(), Some(20));
    assert_eq!(BuildingKind::Barracks.garrison_capacity(), Some(10));
    assert_eq!(BuildingKind::ArcheryRange.garrison_capacity(), Some(10));
    assert_eq!(BuildingKind::Stable.garrison_capacity(), Some(10));
    assert_eq!(BuildingKind::House.garrison_capacity(), None);
    assert_eq!(crate::TC_GARRISON_CAPACITY, 15);
    // Foot set in towers; foot + mounted in the Castle only.
    for kind in [
        EntityKind::Villager,
        EntityKind::Militia,
        EntityKind::ManAtArms,
        EntityKind::Spearman,
        EntityKind::Archer,
        EntityKind::Skirmisher,
    ] {
        assert!(kind.is_foot_garrisonable(), "{kind:?}");
        assert!(BuildingKind::WatchTower.admits_defensive(kind));
        assert!(BuildingKind::Castle.admits_defensive(kind));
    }
    for kind in [
        EntityKind::ScoutCavalry,
        EntityKind::LightCavalry,
        EntityKind::Knight,
        EntityKind::CavalryArcher,
    ] {
        assert!(kind.is_mounted_garrisonable(), "{kind:?}");
        assert!(!BuildingKind::WatchTower.admits_defensive(kind));
        assert!(BuildingKind::Castle.admits_defensive(kind));
    }
    // The Petard is never garrisonable (siege/suicide family).
    assert!(!EntityKind::Petard.is_foot_garrisonable());
    assert!(!EntityKind::Petard.is_mounted_garrisonable());
    assert!(!BuildingKind::Castle.admits_defensive(EntityKind::Petard));
    // Production buildings admit nobody defensively (GAR-08 only).
    assert!(!BuildingKind::Barracks.admits_defensive(EntityKind::Militia));
    // Shelter healing base rates (GAR-04): towers 0.1 HP/s, Castle 0.2.
    assert_eq!(BuildingKind::WatchTower.garrison_heal_hp_per_min(), 6);
    assert_eq!(BuildingKind::Castle.garrison_heal_hp_per_min(), 12);
    assert_eq!(BuildingKind::Barracks.garrison_heal_hp_per_min(), 0);
    // The TC weapon row (GAR-06): 5 pierce, range 6, RoF 2.0.
    let w = crate::TOWN_CENTER_WEAPON;
    assert_eq!(w.attacks, &[(crate::classes::PIERCE, 5)]);
    assert_eq!(f(w.range_max), 24.0);
    assert_eq!(w.reload_ticks, 120);
}

#[test]
fn gar01_command_level_eligibility() {
    let mut sim = Sim::new(7);
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::WatchTower, OWNER_PLAYER, (58, 58)));
    sim.buildings
        .push(placed(&sim, 901, BuildingKind::Castle, OWNER_PLAYER, (70, 58)));
    sim.buildings
        .push(placed(&sim, 902, BuildingKind::Barracks, OWNER_PLAYER, (58, 70)));
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (60, 62));
    spawn(&mut sim, 901, EntityKind::Petard, OWNER_PLAYER, (66, 62));
    spawn(&mut sim, 902, EntityKind::Militia, OWNER_PLAYER, (60, 66));
    let vil = own_villager(&sim);
    let tc = sim.town_center().id;
    // Mounted: Castle yes, tower no, TC no.
    assert!(sim.can_garrison_building(900, 901));
    assert!(!sim.can_garrison_building(900, 900));
    assert!(!sim.can_garrison_tc(900, tc));
    // Petard: nowhere.
    assert!(!sim.can_garrison_building(901, 901));
    assert!(!sim.can_garrison_tc(901, tc));
    // Foot: everywhere defensive; production manual entry refused.
    assert!(sim.can_garrison_building(902, 900));
    assert!(sim.can_garrison_building(902, 901));
    assert!(sim.can_garrison_tc(902, tc));
    assert!(!sim.can_garrison_building(902, 902));
    assert!(!sim.can_garrison_building(vil, 902));
    // Enemy buildings are never a shelter.
    sim.buildings
        .push(placed(&sim, 903, BuildingKind::WatchTower, OWNER_OPPONENT, (80, 58)));
    assert!(!sim.can_garrison_building(902, 903));
}

#[test]
fn gar01_tower_capacity_is_five() {
    let mut sim = Sim::new(7);
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::WatchTower, OWNER_PLAYER, (58, 58)));
    for i in 0..6u32 {
        spawn(
            &mut sim,
            900 + i,
            EntityKind::Villager,
            OWNER_PLAYER,
            (56 + i, 62),
        );
        sim.push_command(Command::GarrisonBuilding {
            entity: 900 + i,
            building: 900,
        });
    }
    run_until(&mut sim, 20_000, |s| s.garrison_count_building(900) >= 5);
    // Settle: the sixth order found the tower full on arrival.
    for _ in 0..600 {
        sim.tick();
    }
    assert_eq!(sim.garrison_count_building(900), 5);
    let inside = (900..906)
        .filter(|&id| ent(&sim, id).garrisoned_in.is_some())
        .count();
    assert_eq!(inside, 5);
}

// ------------------------------------------------------------------
// GAR-02: untouchable.

#[test]
fn gar02_garrisoned_unit_cannot_be_damaged_or_counted_idle() {
    let mut sim = Sim::new(7);
    let vil = own_villager(&sim);
    let tc = sim.town_center().id;
    sim.push_command(Command::GarrisonTc { entity: vil, tc });
    let inside = run_until(&mut sim, 20_000, |s| s.garrison_count_tc(tc) == 1);
    assert!(inside, "the villager never garrisoned in the TC");
    assert_eq!(ent(&sim, vil).garrisoned_in, Some(GarrisonRef::TownCenter(tc)));
    assert!(!sim.idle_villagers().contains(&vil), "garrisoned counts as idle");
    // An enemy Militia ordered onto the garrisoned villager can
    // never land a hit: the target is out of the field.
    let (vx, vy) = (ent(&sim, vil).pos_x, ent(&sim, vil).pos_y);
    let (cx, cy) = sim.grid().world_to_cell(vx, vy);
    spawn(&mut sim, 900, EntityKind::Militia, OWNER_OPPONENT, (cx + 1, cy));
    sim.push_command(Command::Attack {
        entity: 900,
        target: vil,
    });
    for _ in 0..3600 {
        sim.tick();
    }
    assert_eq!(f(ent(&sim, vil).hp), 25.0, "garrisoned villager took damage");
    assert_eq!(sim.garrison_count_tc(tc), 1);
}

// ------------------------------------------------------------------
// GAR-03: ejection, the ≤20% rule, the re-garrison block, voluntary exit.

#[test]
fn gar03_low_hp_ejects_and_blocks_regarrison_until_above_a_fifth() {
    let mut sim = Sim::new(7);
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::WatchTower, OWNER_PLAYER, (58, 58)));
    spawn(&mut sim, 900, EntityKind::Villager, OWNER_PLAYER, (56, 62));
    spawn(&mut sim, 901, EntityKind::Archer, OWNER_PLAYER, (60, 62));
    for id in [900, 901] {
        sim.push_command(Command::GarrisonBuilding {
            entity: id,
            building: 900,
        });
    }
    run_until(&mut sim, 20_000, |s| s.garrison_count_building(900) == 2);
    // Exactly 20% HP (170 of 850): eject, unharmed.
    let bi = sim.buildings.iter().position(|b| b.id == 900).unwrap();
    sim.buildings[bi].hp = I16F16::from_num(170);
    sim.tick();
    assert_eq!(sim.garrison_count_building(900), 0);
    assert_eq!(f(ent(&sim, 900).hp), 25.0);
    assert_eq!(f(ent(&sim, 901).hp), 30.0);
    assert!(ent(&sim, 900).garrisoned_in.is_none());
    // Re-garrison is blocked at ≤20%.
    assert!(!sim.can_garrison_building(900, 900));
    // Above the threshold it opens again.
    sim.buildings[bi].hp = I16F16::from_num(171);
    assert!(sim.can_garrison_building(900, 900));
}

#[test]
fn gar03_destruction_and_voluntary_ungarrison_eject_unharmed() {
    let mut sim = Sim::new(7);
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::WatchTower, OWNER_PLAYER, (58, 58)));
    sim.buildings
        .push(placed(&sim, 901, BuildingKind::WatchTower, OWNER_PLAYER, (70, 58)));
    spawn(&mut sim, 900, EntityKind::Villager, OWNER_PLAYER, (56, 62));
    spawn(&mut sim, 901, EntityKind::Villager, OWNER_PLAYER, (72, 62));
    sim.push_command(Command::GarrisonBuilding {
        entity: 900,
        building: 900,
    });
    sim.push_command(Command::GarrisonBuilding {
        entity: 901,
        building: 901,
    });
    run_until(&mut sim, 20_000, |s| {
        s.garrison_count_building(900) == 1 && s.garrison_count_building(901) == 1
    });
    // Voluntary ungarrison: out at the exit point, idle.
    sim.push_command(Command::UngarrisonBuilding { building: 900 });
    sim.tick();
    assert_eq!(sim.garrison_count_building(900), 0);
    assert!(ent(&sim, 900).garrisoned_in.is_none());
    assert_eq!(f(ent(&sim, 900).hp), 25.0);
    // Destruction (the deliberate-delete path runs destruction):
    // the garrison walks away from the rubble alive.
    sim.push_command(Command::DeleteBuilding { building: 901 });
    run_until(&mut sim, 600, |s| s.buildings().iter().all(|b| b.id != 901));
    assert!(sim.buildings().iter().all(|b| b.id != 901));
    assert!(!ent(&sim, 901).dead);
    assert_eq!(f(ent(&sim, 901).hp), 25.0);
    assert!(ent(&sim, 901).garrisoned_in.is_none());
}

// ------------------------------------------------------------------
// GAR-04: shelter healing base rates.

#[test]
fn gar04_shelter_healing_rates() {
    let mut sim = Sim::new(7);
    let vil = own_villager(&sim);
    let tc = sim.town_center().id;
    let vi = sim.entities.iter().position(|e| e.id == vil).unwrap();
    sim.entities[vi].hp = I16F16::from_num(10);
    sim.push_command(Command::GarrisonTc { entity: vil, tc });
    run_until(&mut sim, 20_000, |s| s.garrison_count_tc(tc) == 1);
    for _ in 0..600 {
        sim.tick();
    }
    // 0.1 HP/s × 10 s = +1 HP (the entry tick itself also heals
    // once — arrivals run before healing in the garrison tick —
    // so the gain lands a single tick's 109 bits past exact).
    let gain = f(ent(&sim, vil).hp) - 10.0;
    assert!(gain > 0.99 && gain < 1.01, "TC shelter gain {gain}");
    // The Castle heals at 0.2 HP/s: a Knight at 50 gains +2 in 10 s.
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::Castle, OWNER_PLAYER, (70, 58)));
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (66, 62));
    let ki = sim.entities.iter().position(|e| e.id == 900).unwrap();
    sim.entities[ki].hp = I16F16::from_num(50);
    sim.push_command(Command::GarrisonBuilding {
        entity: 900,
        building: 900,
    });
    run_until(&mut sim, 20_000, |s| s.garrison_count_building(900) == 1);
    for _ in 0..600 {
        sim.tick();
    }
    // 0.2 HP/s × 10 s = +2 HP (same one-tick entry allowance).
    let gain = f(ent(&sim, 900).hp) - 50.0;
    assert!(gain > 1.99 && gain < 2.01, "Castle shelter gain {gain}");
}

// ------------------------------------------------------------------
// GAR-05: arrow scaling and caps (measured through the recompute).

fn stage_garrison(sim: &mut Sim, host: GarrisonRef, ids: &[u32]) {
    for &id in ids {
        let ei = sim.entities.iter().position(|e| e.id == id).unwrap();
        sim.entities[ei].garrisoned_in = Some(host);
    }
    match host {
        GarrisonRef::Building(bid) => {
            let bi = sim.buildings.iter().position(|b| b.id == bid).unwrap();
            sim.buildings[bi].garrison = ids.to_vec();
        }
        GarrisonRef::TownCenter(tc) => {
            let si = sim.tc_states.iter().position(|s| s.tc_id == tc).unwrap();
            sim.tc_states[si].garrison = ids.to_vec();
        }
    }
    sim.recompute_garrison_arrows(host);
}

#[test]
fn gar05_arrow_formula_and_caps() {
    let mut sim = Sim::new(7);
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::WatchTower, OWNER_PLAYER, (58, 58)));
    sim.buildings
        .push(placed(&sim, 901, BuildingKind::Castle, OWNER_PLAYER, (70, 58)));
    let tc = sim.town_center().id;
    // One villager in the tower: floor(2.5 / 2.5) = 1 extra → 2 total.
    spawn(&mut sim, 900, EntityKind::Villager, OWNER_PLAYER, (56, 62));
    stage_garrison(&mut sim, GarrisonRef::Building(900), &[900]);
    assert_eq!(sim.garrison_arrows_building(900), 2);
    // Five villagers: 5 extra, capped at +4 → 5 total.
    for i in 1..5u32 {
        spawn(&mut sim, 900 + i, EntityKind::Villager, OWNER_PLAYER, (56, 63));
    }
    stage_garrison(&mut sim, GarrisonRef::Building(900), &[900, 901, 902, 903, 904]);
    assert_eq!(sim.garrison_arrows_building(900), 5);
    // The Town Center: 1 villager → 1 arrow total (floor(2.5/2.5)=1);
    // fifteen villagers → capped at 10 total; empty → 0.
    stage_garrison(&mut sim, GarrisonRef::TownCenter(tc), &[900]);
    assert_eq!(sim.garrison_arrows_tc(tc), 1);
    let mut many: Vec<u32> = Vec::new();
    for i in 0..15u32 {
        spawn(&mut sim, 920 + i, EntityKind::Villager, OWNER_PLAYER, (40, 40));
        many.push(920 + i);
    }
    stage_garrison(&mut sim, GarrisonRef::TownCenter(tc), &many);
    assert_eq!(sim.garrison_arrows_tc(tc), 10);
    stage_garrison(&mut sim, GarrisonRef::TownCenter(tc), &[]);
    assert_eq!(sim.garrison_arrows_tc(tc), 0);
    // The Castle: a mounted guest adds nothing (base volley 5);
    // ten foot archers (2.0 DPS each) add floor(20 / 5.5) = 3 → 8.
    spawn(&mut sim, 950, EntityKind::Knight, OWNER_PLAYER, (66, 62));
    stage_garrison(&mut sim, GarrisonRef::Building(901), &[950]);
    assert_eq!(sim.garrison_arrows_building(901), 5);
    let mut archers: Vec<u32> = Vec::new();
    for i in 0..10u32 {
        spawn(&mut sim, 960 + i, EntityKind::Archer, OWNER_PLAYER, (66, 63));
        archers.push(960 + i);
    }
    stage_garrison(&mut sim, GarrisonRef::Building(901), &archers);
    assert_eq!(sim.garrison_arrows_building(901), 8);
    // One Skirmisher (2 pierce / 3.0 s ≈ 0.67 DPS) in the TC:
    // floor(0.67 / 2.5) = 0 extra, but GAR-06's minimum holds 1.
    spawn(&mut sim, 980, EntityKind::Skirmisher, OWNER_PLAYER, (40, 41));
    stage_garrison(&mut sim, GarrisonRef::TownCenter(tc), &[980]);
    assert_eq!(sim.garrison_arrows_tc(tc), 1);
}

// ------------------------------------------------------------------
// GAR-06: the TC fires only while garrisoned.

#[test]
fn gar06_tc_fires_only_while_garrisoned() {
    let mut sim = Sim::new(7);
    let (tc_id, tcx, tcy) = {
        let tc = sim.town_center();
        (tc.id, tc.pos_x, tc.pos_y)
    };
    let (ccx, ccy) = sim.grid().world_to_cell(tcx, tcy);
    // An enemy villager ~4 tiles east of the TC footprint's edge.
    spawn(
        &mut sim,
        900,
        EntityKind::Villager,
        OWNER_OPPONENT,
        (ccx + 6, ccy),
    );
    for _ in 0..1200 {
        sim.tick();
    }
    assert_eq!(
        f(ent(&sim, 900).hp),
        25.0,
        "an ungarrisoned Town Center fired"
    );
    // Garrison one villager: the TC opens fire (5 pierce an arrow).
    let vil = own_villager(&sim);
    sim.push_command(Command::GarrisonTc { entity: vil, tc: tc_id });
    let hurt = run_until(&mut sim, 20_000, |s| ent(s, 900).hp < I16F16::from_num(25));
    assert!(hurt, "the garrisoned Town Center never fired");
}

// ------------------------------------------------------------------
// GAR-07: Town Bell snapshot and restore.

#[test]
fn gar07_town_bell_shelters_and_restores() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = own_villager(&sim);
    // Walk the villager a few tiles out (still inside the 23-tile
    // bell radius) and remember the spot as its snapshot position.
    let (tx, ty) = (ent(&sim, vil).pos_x, ent(&sim, vil).pos_y);
    let (dest_x, dest_y) = (tx + I16F16::from_num(20), ty);
    sim.push_command(Command::Move {
        entity: vil,
        x: dest_x,
        y: dest_y,
    });
    run_until(&mut sim, 20_000, |s| {
        crate::steer::vec_len(ent(s, vil).pos_x - dest_x, ent(s, vil).pos_y - dest_y)
            < I16F16::from_num(2)
    });
    let (sx, sy) = (ent(&sim, vil).pos_x, ent(&sim, vil).pos_y);
    // Ring: every own villager (all start within the radius) goes
    // to shelter — the TC itself is the nearest for all of them.
    sim.push_command(Command::TownBell { tc });
    let sheltered = run_until(&mut sim, 30_000, |s| {
        s.entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
            .all(|e| e.garrisoned_in.is_some())
    });
    assert!(sheltered, "the bell did not shelter every villager");
    assert!(sim.tc_bell_active(tc));
    assert!(ent(&sim, vil).bell_snapshot.is_some());
    // Ring again: back to work — out of shelter, snapshot consumed,
    // and the villager walks back to its snapshot position.
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    assert!(!sim.tc_bell_active(tc));
    assert!(ent(&sim, vil).bell_snapshot.is_none());
    assert!(ent(&sim, vil).garrisoned_in.is_none());
    let back = run_until(&mut sim, 30_000, |s| {
        crate::steer::vec_len(ent(s, vil).pos_x - sx, ent(s, vil).pos_y - sy)
            < I16F16::from_num(3)
    });
    assert!(back, "the villager did not return to its snapshot position");
}

// ------------------------------------------------------------------
// GAR-08: rally-garrison, incl. production garrison.

#[test]
fn gar08_rally_garrison_into_a_tower_and_a_production_building() {
    let mut sim = Sim::new(7);
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::WatchTower, OWNER_PLAYER, (58, 58)));
    sim.buildings
        .push(placed(&sim, 901, BuildingKind::Barracks, OWNER_PLAYER, (70, 58)));
    sim.buildings
        .push(placed(&sim, 902, BuildingKind::Stable, OWNER_PLAYER, (58, 70)));
    // Population room for both trainees (the HOUSED rule refuses
    // the second enqueue at the starting cap of 5).
    sim.buildings
        .push(placed(&sim, 903, BuildingKind::House, OWNER_PLAYER, (50, 50)));
    // The Barracks rallies onto the tower; the Stable rallies onto
    // the Barracks (a production host — entry only via this path).
    let (tpos, bpos) = {
        let t = sim.buildings().iter().find(|b| b.id == 900).unwrap();
        let b = sim.buildings().iter().find(|b| b.id == 901).unwrap();
        ((t.pos_x, t.pos_y), (b.pos_x, b.pos_y))
    };
    for (bid, rp) in [(901u32, tpos), (902u32, bpos)] {
        let bi = sim.buildings.iter().position(|b| b.id == bid).unwrap();
        sim.buildings[bi].rally = Some(rp);
    }
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    sim.push_command(Command::TrainAt {
        building: 901,
        kind: EntityKind::Militia,
    });
    sim.push_command(Command::TrainAt {
        building: 902,
        kind: EntityKind::ScoutCavalry,
    });
    let done = run_until(&mut sim, 40_000, |s| {
        s.garrison_count_building(900) == 1 && s.garrison_count_building(901) == 1
    });
    assert!(done, "rally-garrison never completed");
    let militia = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Militia)
        .unwrap();
    assert_eq!(
        militia.garrisoned_in,
        Some(GarrisonRef::Building(900)),
        "the rallied Militia did not garrison in the tower"
    );
    let scout = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::ScoutCavalry && e.owner == OWNER_PLAYER && e.id > 100)
        .or_else(|| {
            sim.entities()
                .iter()
                .find(|e| e.kind == EntityKind::ScoutCavalry && e.garrisoned_in.is_some())
        })
        .unwrap();
    assert_eq!(
        scout.garrisoned_in,
        Some(GarrisonRef::Building(901)),
        "the rallied Scout did not production-garrison in the Barracks"
    );
    // Production garrison: eject freely, but NEVER re-enter.
    let scout_id = scout.id;
    sim.push_command(Command::UngarrisonBuilding { building: 901 });
    sim.tick();
    assert_eq!(sim.garrison_count_building(901), 0);
    assert!(!sim.can_garrison_building(scout_id, 901));
}
