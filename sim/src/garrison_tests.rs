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
use crate::entity::{Entity, EntityKind, GarrisonRef, Task, OWNER_OPPONENT, OWNER_PLAYER};
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
        gate_along_x: true,
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
        GarrisonRef::Unit(uid) => {
            let ei = sim.entities.iter().position(|e| e.id == uid).unwrap();
            sim.entities[ei].garrison = ids.to_vec();
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

// ------------------------------------------------------------------
// GAR-10 (Castle P4a): the Battering Ram — a mobile garrison host.

#[test]
fn gar10_eligibility_matrix() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::BatteringRam, OWNER_PLAYER, (58, 58));
    spawn(&mut sim, 901, EntityKind::BatteringRam, OWNER_OPPONENT, (80, 80));
    // Foot units are admitted: villagers, infantry, foot archers.
    for (i, kind) in [
        EntityKind::Villager,
        EntityKind::Militia,
        EntityKind::ManAtArms,
        EntityKind::Spearman,
        EntityKind::Archer,
        EntityKind::Skirmisher,
    ]
    .iter()
    .enumerate()
    {
        spawn(&mut sim, 910 + i as u32, *kind, OWNER_PLAYER, (56, 62));
        assert!(
            sim.can_garrison_unit(910 + i as u32, 900),
            "{kind:?} refused by a Ram"
        );
    }
    // The discriminating refusals: mounted and siege never enter.
    for (i, kind) in [
        EntityKind::Knight,
        EntityKind::CavalryArcher,
        EntityKind::ScoutCavalry,
        EntityKind::Petard,
        EntityKind::TradeCart,
        EntityKind::BatteringRam,
    ]
    .iter()
    .enumerate()
    {
        spawn(&mut sim, 920 + i as u32, *kind, OWNER_PLAYER, (56, 64));
        assert!(
            !sim.can_garrison_unit(920 + i as u32, 900),
            "{kind:?} admitted by a Ram"
        );
    }
    // An enemy Ram is never a shelter; a Ram is not a building
    // or TC passenger either (siege never garrisons, GAR-01).
    let vil = own_villager(&sim);
    assert!(!sim.can_garrison_unit(vil, 901));
    let tc = sim.town_center().id;
    assert!(!sim.can_garrison_tc(900, tc));
    assert!(!sim.can_garrison_building(900, 900));
    // A Siege Workshop is a production garrison of 10 (GAR-08).
    assert_eq!(BuildingKind::SiegeWorkshop.garrison_capacity(), Some(10));
    assert!(BuildingKind::SiegeWorkshop.is_production_garrison());
}

#[test]
fn gar10_capacity_six_is_a_hard_stop() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::BatteringRam, OWNER_PLAYER, (58, 58));
    for i in 0..7u32 {
        spawn(&mut sim, 910 + i, EntityKind::Militia, OWNER_PLAYER, (56 + i, 62));
        sim.push_command(Command::GarrisonUnit {
            entity: 910 + i,
            host: 900,
        });
    }
    run_until(&mut sim, 30_000, |s| s.garrison_count_unit(900) >= 6);
    for _ in 0..600 {
        sim.tick();
    }
    assert_eq!(sim.garrison_count_unit(900), 6);
    let inside = (910..917)
        .filter(|&id| ent(&sim, id).garrisoned_in == Some(GarrisonRef::Unit(900)))
        .count();
    assert_eq!(inside, 6, "a seventh passenger slipped inside");
    // And the query agrees the Ram is full.
    spawn(&mut sim, 920, EntityKind::Archer, OWNER_PLAYER, (56, 64));
    assert!(!sim.can_garrison_unit(920, 900));
}

#[test]
fn gar10_derived_speed_values_and_live_movement() {
    // The derived table itself: 0.6 / 0.65 / 0.9 tiles/s at
    // 0 / 1 / 6 passengers, in the unit table's bit conversion.
    assert_eq!(crate::garrison::ram_speed(0).to_bits(), 2_621);
    assert_eq!(crate::garrison::ram_speed(1).to_bits(), 2_839);
    assert_eq!(crate::garrison::ram_speed(6).to_bits(), 3_932);
    // The cap is structural: a (hypothetical) seventh passenger
    // could not push past 0.9.
    assert_eq!(crate::garrison::ram_speed(7).to_bits(), 3_932);
    // Live: over the same 10 s on the same ground, a full Ram
    // covers ~1.5x the empty one's distance.
    let travel = |passengers: u32| -> f64 {
        let mut sim = Sim::new(7);
        spawn(&mut sim, 900, EntityKind::BatteringRam, OWNER_PLAYER, (20, 60));
        if passengers > 0 {
            let mut ids = Vec::new();
            for i in 0..passengers {
                spawn(
                    &mut sim,
                    910 + i,
                    EntityKind::Militia,
                    OWNER_PLAYER,
                    (24, 60),
                );
                ids.push(910 + i);
            }
            stage_garrison(&mut sim, GarrisonRef::Unit(900), &ids);
        }
        let (sx, sy) = (ent(&sim, 900).pos_x, ent(&sim, 900).pos_y);
        sim.push_command(Command::Move {
            entity: 900,
            x: sx + I16F16::from_num(200),
            y: sy,
        });
        for _ in 0..600 {
            sim.tick();
        }
        f(ent(&sim, 900).pos_x - sx)
    };
    let empty = travel(0);
    let full = travel(6);
    assert!(empty > 15.0, "empty Ram barely moved: {empty}");
    assert!(
        full > empty * 1.4,
        "loaded Ram ({full}) not clearly faster than empty ({empty})"
    );
}

#[test]
fn gar10_derived_building_attack_per_passenger() {
    // One landed strike against a staged enemy House, measured
    // in-sim: the House's Dark column (melee armor −2, building
    // armor 0) makes the arithmetic exact — melee pair
    // 2 − (−2) = 4, buildings pair 150 (+10 per passenger).
    let strike_damage = |passengers: u32| -> f64 {
        let mut sim = Sim::new(7);
        sim.buildings.push(placed(
            &sim,
            900,
            BuildingKind::House,
            OWNER_OPPONENT,
            (60, 60),
        ));
        spawn(&mut sim, 900, EntityKind::BatteringRam, OWNER_PLAYER, (58, 61));
        if passengers > 0 {
            let mut ids = Vec::new();
            for i in 0..passengers {
                spawn(
                    &mut sim,
                    910 + i,
                    EntityKind::Militia,
                    OWNER_PLAYER,
                    (50, 50),
                );
                ids.push(910 + i);
            }
            stage_garrison(&mut sim, GarrisonRef::Unit(900), &ids);
        }
        let before = sim
            .buildings()
            .iter()
            .find(|b| b.id == 900)
            .unwrap()
            .hp;
        sim.push_command(Command::AttackBuilding {
            entity: 900,
            building: 900,
        });
        // Ids collide by design in this fixture style (building
        // 900, ram 900 — separate id spaces, as in the engine).
        let mut delta = 0.0;
        for _ in 0..20_000 {
            sim.tick();
            let now = sim
                .buildings()
                .iter()
                .find(|b| b.id == 900)
                .map(|b| b.hp);
            match now {
                Some(hp) if hp < before => {
                    delta = f(before - hp);
                    break;
                }
                None => break,
                _ => {}
            }
        }
        delta
    };
    assert_eq!(strike_damage(0), 154.0, "empty Ram strike");
    assert_eq!(strike_damage(1), 164.0, "one-passenger Ram strike");
    assert_eq!(strike_damage(6), 214.0, "full Ram strike");
}

#[test]
fn gar10_passengers_survive_the_rams_destruction() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::BatteringRam, OWNER_PLAYER, (58, 58));
    let mut ids = Vec::new();
    for i in 0..3u32 {
        spawn(&mut sim, 910 + i, EntityKind::Militia, OWNER_PLAYER, (50, 50));
        ids.push(910 + i);
    }
    stage_garrison(&mut sim, GarrisonRef::Unit(900), &ids);
    // While the Ram is beaten on, the passengers are untouchable
    // (GAR-02): an enemy Militia lands its amplified strikes on
    // the Ram alone (13 a hit vs the −3 melee armor).
    spawn(&mut sim, 920, EntityKind::Militia, OWNER_OPPONENT, (59, 58));
    sim.push_command(Command::Attack {
        entity: 920,
        target: 900,
    });
    for _ in 0..600 {
        sim.tick();
    }
    let militia_full = crate::units::spec(EntityKind::Militia).max_hp as f64;
    for &id in &ids {
        assert_eq!(
            f(ent(&sim, id).hp),
            militia_full,
            "passenger {id} was hurt inside"
        );
    }
    assert!(f(ent(&sim, 900).hp) < 175.0, "the Ram took no damage");
    // Destruction: the passengers eject ALIVE at the ring around
    // the Ram's final position (GAR-10's only forced ejection).
    let ri = sim.entities.iter().position(|e| e.id == 900).unwrap();
    sim.entities[ri].hp = I16F16::ZERO;
    sim.tick();
    assert!(ent(&sim, 900).dead);
    let (rx, ry) = (ent(&sim, 900).pos_x, ent(&sim, 900).pos_y);
    for &id in &ids {
        let p = ent(&sim, id);
        assert!(!p.dead, "passenger {id} died with the Ram");
        assert!(p.garrisoned_in.is_none());
        let d = crate::steer::vec_len(p.pos_x - rx, p.pos_y - ry);
        assert!(f(d) < 8.0, "passenger {id} ejected far from the wreck");
    }
}

#[test]
fn gar10_no_healing_and_no_low_hp_rule_inside_a_ram() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::BatteringRam, OWNER_PLAYER, (58, 58));
    spawn(&mut sim, 910, EntityKind::Militia, OWNER_PLAYER, (50, 50));
    let mi = sim.entities.iter().position(|e| e.id == 910).unwrap();
    sim.entities[mi].hp = I16F16::from_num(10);
    stage_garrison(&mut sim, GarrisonRef::Unit(900), &[910]);
    // The Ram at 17% HP: GAR-03's ≤20% ejection and re-entry
    // block are building-scoped — neither applies to a Ram.
    let ri = sim.entities.iter().position(|e| e.id == 900).unwrap();
    sim.entities[ri].hp = I16F16::from_num(30);
    spawn(&mut sim, 911, EntityKind::Archer, OWNER_PLAYER, (56, 62));
    assert!(
        sim.can_garrison_unit(911, 900),
        "a sub-20% Ram refused a passenger (GAR-03 does not apply)"
    );
    for _ in 0..1200 {
        sim.tick();
    }
    assert_eq!(
        sim.garrison_count_unit(900),
        1,
        "the passenger was ejected at low HP"
    );
    // GAR-04 names no Ram rate: twenty seconds inside heal nothing.
    assert_eq!(f(ent(&sim, 910).hp), 10.0, "a passenger healed inside a Ram");
}

#[test]
fn gar10_a_moving_ram_still_loads_and_ungarrisons_where_it_stands() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::BatteringRam, OWNER_PLAYER, (58, 58));
    spawn(&mut sim, 910, EntityKind::Militia, OWNER_PLAYER, (52, 58));
    sim.push_command(Command::GarrisonUnit {
        entity: 910,
        host: 900,
    });
    // Mid-walk, the Ram drives ten cells east: the passenger must
    // still load (the approach reads the live position).
    for _ in 0..30 {
        sim.tick();
    }
    let (rx, ry) = (ent(&sim, 900).pos_x, ent(&sim, 900).pos_y);
    sim.push_command(Command::Move {
        entity: 900,
        x: rx + I16F16::from_num(40),
        y: ry,
    });
    let loaded = run_until(&mut sim, 40_000, |s| s.garrison_count_unit(900) == 1);
    assert!(loaded, "the passenger never loaded into the moved Ram");
    // Voluntary ungarrison: out at the ring around the Ram's
    // CURRENT position, not where it loaded.
    let (rx, ry) = (ent(&sim, 900).pos_x, ent(&sim, 900).pos_y);
    sim.push_command(Command::UngarrisonUnit { host: 900 });
    sim.tick();
    assert_eq!(sim.garrison_count_unit(900), 0);
    let p = ent(&sim, 910);
    assert!(p.garrisoned_in.is_none());
    let d = crate::steer::vec_len(p.pos_x - rx, p.pos_y - ry);
    assert!(f(d) < 8.0, "ungarrisoned far from the Ram's position");
}

// ------------------------------------------------------------------
// GAR-07 defect hunt (live report 2026-10-30): "ring the bell to go
// into the Town Center — not all of them come back out afterwards."
// Scenarios (a)–(f): over-capacity crowd, mixed tasks, a villager
// garrisoned manually before the ring, a villager produced during
// the bell, a fast double ring, and a rally set on the TC itself.

fn player_villager_ids(sim: &Sim) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER && !e.dead)
        .map(|e| e.id)
        .collect()
}

/// Spawn `n` extra player villagers on free cells in the ring
/// around the player's Town Center (inside the 23-tile bell
/// radius), with hand-picked ids from `first_id` up.
fn spawn_villagers_near_tc(sim: &mut Sim, n: usize, first_id: u32) -> Vec<u32> {
    let tc = sim.town_center().id;
    let (tx, ty) = {
        let e = ent(sim, tc);
        (e.pos_x, e.pos_y)
    };
    let (ccx, ccy) = sim.grid().world_to_cell(tx, ty);
    let mut out = Vec::new();
    let mut id = first_id;
    'outer: for ring in 3i32..=6 {
        for dx in -ring..=ring {
            for dy in -ring..=ring {
                if dx.abs() != ring && dy.abs() != ring {
                    continue;
                }
                if out.len() >= n {
                    break 'outer;
                }
                let cx = ccx as i32 + dx;
                let cy = ccy as i32 + dy;
                if cx < 0 || cy < 0 || cx >= crate::GRID_DIM as i32 || cy >= crate::GRID_DIM as i32
                {
                    continue;
                }
                let (cx, cy) = (cx as u32, cy as u32);
                if sim.grid().is_blocked(cx, cy) {
                    continue;
                }
                // Keep one villager per cell.
                let (wx, wy) = sim.grid().cell_center(cx, cy);
                if sim
                    .entities()
                    .iter()
                    .any(|e| !e.dead && e.pos_x == wx && e.pos_y == wy)
                {
                    continue;
                }
                spawn(sim, id, EntityKind::Villager, OWNER_PLAYER, (cx, cy));
                out.push(id);
                id += 1;
            }
        }
    }
    assert_eq!(out.len(), n, "could not place {n} villagers near the TC");
    out
}

fn nearest_node(sim: &Sim, from: (I16F16, I16F16)) -> u32 {
    sim.nodes()
        .iter()
        .filter(|n| n.amount_left > I16F16::ZERO)
        .min_by_key(|n| {
            let d = crate::steer::vec_len(n.pos_x - from.0, n.pos_y - from.1);
            d.to_bits()
        })
        .unwrap()
        .id
}

// (a) More villagers inside the radius than the TC holds (15).
// Only 15 can shelter; the rest never get a snapshot and keep
// working. On the second ring every one of the 15 must come out.
#[test]
fn gar07_bell_over_capacity_crowd_all_sheltered_come_back_out() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    spawn_villagers_near_tc(&mut sim, 17, 500);
    assert_eq!(player_villager_ids(&sim).len(), 20);
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    // Exactly 15 hold snapshots (the TC's capacity); 5 never got
    // an order and stay at work.
    let snapped = player_villager_ids(&sim)
        .into_iter()
        .filter(|&id| ent(&sim, id).bell_snapshot.is_some())
        .count();
    assert_eq!(snapped, 15, "expected exactly 15 bell snapshots, got {snapped}");
    // Every snapshot holder must actually get inside — nobody may
    // be stranded outside with a garrison walk that died.
    let all_in = run_until(&mut sim, 60_000, |s| {
        player_villager_ids(s)
            .into_iter()
            .filter(|&id| ent(s, id).bell_snapshot.is_some())
            .all(|id| ent(s, id).garrisoned_in == Some(GarrisonRef::TownCenter(tc)))
    });
    assert!(
        all_in,
        "some bell villagers never entered the TC: {:?}",
        player_villager_ids(&sim)
            .into_iter()
            .filter(|&id| ent(&sim, id).bell_snapshot.is_some())
            .map(|id| {
                let e = ent(&sim, id);
                (id, e.garrisoned_in, e.garrison_target, e.order.is_some())
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(sim.garrison_count_tc(tc), 15);
    // Back to work: all 15 come out, snapshots consumed, the TC
    // is empty again and nobody keeps a garrison walk.
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    assert_eq!(sim.garrison_count_tc(tc), 0, "the TC did not empty on back-to-work");
    for id in player_villager_ids(&sim) {
        let e = ent(&sim, id);
        assert!(e.garrisoned_in.is_none(), "villager {id} still garrisoned");
        assert!(e.bell_snapshot.is_none(), "villager {id} kept its snapshot");
        assert!(e.garrison_target.is_none(), "villager {id} kept a garrison walk");
    }
}

// (b) Mixed tasks at ring time: wood gatherer, food gatherer,
// builder, repairer, idle. All shelter; on back-to-work all come
// out AND resume their snapshotted task.
#[test]
fn gar07_bell_mixed_tasks_all_come_out_and_resume() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let extra = spawn_villagers_near_tc(&mut sim, 3, 500);
    let starters = player_villager_ids(&sim)
        .into_iter()
        .filter(|id| !extra.contains(id))
        .collect::<Vec<_>>();
    let (v_wood, v_food, v_build) = (starters[0], starters[1], extra[0]);
    let (v_repair, v_idle) = (extra[1], extra[2]);
    // Wood: nearest node to the villager.
    let wpos = {
        let e = ent(&sim, v_wood);
        (e.pos_x, e.pos_y)
    };
    let wood_node = nearest_node(&sim, wpos);
    sim.push_command(Command::Gather {
        entity: v_wood,
        node: wood_node,
    });
    // Food: the villager next to a berry bush if one is in reach,
    // else a second gatherer on another node.
    let fpos = {
        let e = ent(&sim, v_food);
        (e.pos_x, e.pos_y)
    };
    let food_node = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == crate::econ::NodeKind::BerryBush && n.amount_left > I16F16::ZERO)
        .min_by_key(|n| crate::steer::vec_len(n.pos_x - fpos.0, n.pos_y - fpos.1).to_bits())
        .map(|n| n.id)
        .unwrap_or_else(|| nearest_node(&sim, fpos));
    sim.push_command(Command::Gather {
        entity: v_food,
        node: food_node,
    });
    // Build: a House foundation a few cells from the TC.
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    let (tcx, tcy) = {
        let e = ent(&sim, tc);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    let (hx, hy) = sim.grid().cell_center(tcx + 6, tcy + 6);
    sim.push_command(Command::Build {
        entity: v_build,
        kind: BuildingKind::House,
        x: hx,
        y: hy,
    });
    // Repair: a damaged own House record beside the TC.
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::House, OWNER_PLAYER, (tcx + 5, tcy)));
    let hi = sim.buildings.iter().position(|b| b.id == 900).unwrap();
    sim.buildings[hi].hp = I16F16::from_num(50);
    sim.push_command(Command::Repair {
        entity: v_repair,
        building: 900,
    });
    // Let the tasks take hold.
    let tasked = run_until(&mut sim, 30_000, |s| {
        matches!(ent(s, v_wood).task, Task::ToNode | Task::Gathering)
            && matches!(ent(s, v_food).task, Task::ToNode | Task::Gathering)
            && matches!(ent(s, v_build).task, Task::ToBuild | Task::Building)
            && matches!(ent(s, v_repair).task, Task::Repair { .. })
    });
    assert!(tasked, "the mixed tasks never took hold");
    let house_bid = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::House && !b.complete)
        .map(|b| b.id)
        .expect("the House foundation exists");
    // Ring: everyone shelters in the TC.
    sim.push_command(Command::TownBell { tc });
    let all_in = run_until(&mut sim, 60_000, |s| {
        player_villager_ids(s)
            .into_iter()
            .all(|id| ent(s, id).garrisoned_in == Some(GarrisonRef::TownCenter(tc)))
    });
    assert!(all_in, "not every villager sheltered");
    // Back to work: everyone out, and each tasked villager resumes
    // its snapshot task.
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    assert_eq!(sim.garrison_count_tc(tc), 0);
    for id in player_villager_ids(&sim) {
        assert!(ent(&sim, id).garrisoned_in.is_none(), "villager {id} still in");
        assert!(ent(&sim, id).bell_snapshot.is_none(), "villager {id} kept snapshot");
    }
    let resumed = run_until(&mut sim, 30_000, |s| {
        matches!(ent(s, v_wood).task, Task::ToNode | Task::Gathering | Task::Returning)
            && matches!(ent(s, v_food).task, Task::ToNode | Task::Gathering | Task::Returning)
            && matches!(ent(s, v_build).task, Task::ToBuild | Task::Building)
            && matches!(ent(s, v_repair).task, Task::Repair { building } if building == 900)
    });
    assert!(
        resumed,
        "tasks did not resume: wood={:?} food={:?} build={:?} repair={:?} (foundation {house_bid})",
        ent(&sim, v_wood).task,
        ent(&sim, v_food).task,
        ent(&sim, v_build).task,
        ent(&sim, v_repair).task
    );
    let _ = v_idle;
}

// (g) A developed base at real-game scale: 24 villagers working
// scattered tasks inside the radius, a Watch Tower beside the far
// group (nearer to them than the TC), a Castle on the other side.
// Full cycle: everyone the bell takes must shelter somewhere, and
// on back-to-work EVERY shelter must empty of bell villagers.
#[test]
fn gar07_bell_developed_base_full_cycle_empties_every_shelter() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let (tcx, tcy) = {
        let e = ent(&sim, tc);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    // Tower 14 tiles east of the TC; Castle 14 tiles west.
    sim.buildings.push(placed(
        &sim,
        900,
        BuildingKind::WatchTower,
        OWNER_PLAYER,
        (tcx + 14, tcy),
    ));
    sim.buildings.push(placed(
        &sim,
        901,
        BuildingKind::Castle,
        OWNER_PLAYER,
        (tcx.saturating_sub(16), tcy),
    ));
    // 21 extra villagers in three clusters: east (tower side),
    // west (Castle side), and around the TC.
    let mut spawned = Vec::new();
    for i in 0..7 {
        spawned.push((tcx + 11 + (i % 3), tcy + (i / 3) as u32 % 3));
    }
    for i in 0..7 {
        spawned.push((tcx.saturating_sub(13 + (i % 3)), tcy + (i / 3) as u32 % 3));
    }
    for i in 0..7 {
        spawned.push((tcx + (i % 3), tcy + 4 + (i / 3) as u32 % 3));
    }
    let mut ids = Vec::new();
    for (n, cell) in spawned.into_iter().enumerate() {
        let id = 500 + n as u32;
        // Skip cells the fixtures or the map already block/occupy.
        if sim.grid().is_blocked(cell.0, cell.1) {
            continue;
        }
        let (wx, wy) = sim.grid().cell_center(cell.0, cell.1);
        if sim
            .entities()
            .iter()
            .any(|e| !e.dead && e.pos_x == wx && e.pos_y == wy)
        {
            continue;
        }
        spawn(&mut sim, id, EntityKind::Villager, OWNER_PLAYER, cell);
        ids.push(id);
    }
    assert!(ids.len() >= 15, "fixture placed only {} villagers", ids.len());
    // Send half of them to work on the nearest nodes so the ring
    // catches a mix of walking / gathering / idle villagers.
    let workers: Vec<u32> = ids.iter().step_by(2).cloned().collect();
    for id in &workers {
        let pos = {
            let e = ent(&sim, *id);
            (e.pos_x, e.pos_y)
        };
        let node = nearest_node(&sim, pos);
        sim.push_command(Command::Gather {
            entity: *id,
            node,
        });
    }
    for _ in 0..600 {
        sim.tick();
    }
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    let snapped: Vec<u32> = player_villager_ids(&sim)
        .into_iter()
        .filter(|&id| ent(&sim, id).bell_snapshot.is_some())
        .collect();
    assert!(
        snapped.len() >= 18,
        "the bell snapshotted only {} villagers",
        snapped.len()
    );
    // Everyone snapshotted must end up inside SOME shelter.
    let all_in = run_until(&mut sim, 90_000, |s| {
        snapped
            .iter()
            .all(|&id| ent(s, id).garrisoned_in.is_some())
    });
    assert!(
        all_in,
        "bell villagers stranded outside: {:?}",
        snapped
            .iter()
            .filter(|&id| ent(&sim, *id).garrisoned_in.is_none())
            .map(|&id| {
                let e = ent(&sim, id);
                (id, e.garrison_target, e.order.is_some(), e.pos_x.to_num::<f64>(), e.pos_y.to_num::<f64>())
            })
            .collect::<Vec<_>>()
    );
    let in_tower = sim.garrison_count_building(900);
    let in_castle = sim.garrison_count_building(901);
    assert!(
        in_tower > 0,
        "nobody sheltered in the nearer tower (tower {in_tower}, castle {in_castle}, tc {})",
        sim.garrison_count_tc(tc)
    );
    // Back to work: every shelter empties of bell villagers.
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    assert_eq!(sim.garrison_count_tc(tc), 0, "TC still holds bell villagers");
    assert_eq!(sim.garrison_count_building(900), 0, "tower still holds bell villagers");
    assert_eq!(sim.garrison_count_building(901), 0, "castle still holds bell villagers");
    for id in &snapped {
        let e = ent(&sim, *id);
        assert!(e.garrisoned_in.is_none(), "villager {id} still garrisoned");
        assert!(e.bell_snapshot.is_none(), "villager {id} kept its snapshot");
        assert!(e.garrison_target.is_none(), "villager {id} kept a garrison walk");
    }
}

// (h) TWO Town Centers: each TC's bell is its own toggle. TC2's
// bell shelters the villagers around TC2; TC1's full ring cycle
// must NOT release TC2's bell villagers (they answer to TC2's
// bell, which is still rung), and TC2's own second ring must
// release them.
#[test]
fn gar07_bell_two_town_centers_do_not_cross_release() {
    let mut sim = Sim::new(7);
    let tc1 = sim.town_center().id;
    let (c1x, c1y) = {
        let e = ent(&sim, tc1);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    // Second TC fixture, 40 tiles east — its villagers are far
    // outside TC1's bell radius.
    let tc2: u32 = 950;
    let (t2x, t2y) = sim.grid().cell_center(c1x + 40, c1y);
    sim.entities
        .push(Entity::spawned(tc2, EntityKind::TownCenter, OWNER_PLAYER, t2x, t2y));
    sim.tc_states.push(crate::TcState {
        tc_id: tc2,
        queue: Vec::new(),
        progress: 0,
        rally: None,
        rally_node: None,
        garrison: Vec::new(),
        garrison_arrows: 0,
        weapon_target: None,
        weapon_target_building: None,
        weapon_strike_tick: None,
        weapon_next_tick: 0,
        weapon_engaged: false,
        bell_active: false,
        repair_hp_remainder: 0,
        repair_cost_remainder: 0,
    });
    // Four villagers around TC2.
    let mut near2 = Vec::new();
    for i in 0..4u32 {
        let cell = (c1x + 38 + (i % 2), c1y + 2 + (i / 2));
        spawn(&mut sim, 500 + i, EntityKind::Villager, OWNER_PLAYER, cell);
        near2.push(500 + i);
    }
    // Ring TC2: its four shelter inside TC2.
    sim.push_command(Command::TownBell { tc: tc2 });
    let in2 = run_until(&mut sim, 60_000, |s| s.garrison_count_tc(tc2) == 4);
    assert!(in2, "TC2's bell did not shelter its villagers");
    assert!(sim.tc_bell_active(tc2));
    // TC1 runs a full ring cycle of its own (its starters shelter
    // and come back). TC2's bell villagers must stay inside —
    // TC2's bell is still rung.
    sim.push_command(Command::TownBell { tc: tc1 });
    let in1 = run_until(&mut sim, 60_000, |s| {
        s.garrison_count_tc(tc1) == 3
    });
    assert!(in1, "TC1's bell did not shelter the starters");
    sim.push_command(Command::TownBell { tc: tc1 });
    sim.tick();
    assert_eq!(sim.garrison_count_tc(tc1), 0, "TC1's own villagers did not come out");
    assert_eq!(
        sim.garrison_count_tc(tc2),
        4,
        "TC1's back-to-work released TC2's bell villagers"
    );
    assert!(sim.tc_bell_active(tc2), "TC2's bell state was disturbed");
    for id in &near2 {
        assert!(
            ent(&sim, *id).bell_snapshot.is_some(),
            "villager {id} lost its TC2 snapshot to TC1's ring cycle"
        );
    }
    // TC2's own second ring releases them.
    sim.push_command(Command::TownBell { tc: tc2 });
    sim.tick();
    assert_eq!(sim.garrison_count_tc(tc2), 0, "TC2's back-to-work did not release its villagers");
    assert!(!sim.tc_bell_active(tc2));
}

// (i) A villager already walking to shelter for TC1's bell holds
// TC1's snapshot of its REAL task. TC2's ring (its first) must
// not steal the walker and overwrite the snapshot with the
// walker's zeroed task — on back-to-work the villager must
// return to the task it had before ANY bell.
#[test]
fn gar07_bell_second_tc_ring_does_not_steal_snapshot() {
    let mut sim = Sim::new(7);
    let tc1 = sim.town_center().id;
    let (c1x, c1y) = {
        let e = ent(&sim, tc1);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    // Second TC 18 tiles east: a villager between them is inside
    // both bells' 23-tile radii.
    let tc2: u32 = 950;
    let (t2x, t2y) = sim.grid().cell_center(c1x + 18, c1y);
    sim.entities
        .push(Entity::spawned(tc2, EntityKind::TownCenter, OWNER_PLAYER, t2x, t2y));
    sim.tc_states.push(crate::TcState {
        tc_id: tc2,
        queue: Vec::new(),
        progress: 0,
        rally: None,
        rally_node: None,
        garrison: Vec::new(),
        garrison_arrows: 0,
        weapon_target: None,
        weapon_target_building: None,
        weapon_strike_tick: None,
        weapon_next_tick: 0,
        weapon_engaged: false,
        bell_active: false,
        repair_hp_remainder: 0,
        repair_cost_remainder: 0,
    });
    // One villager 5 tiles east of TC1, put to work gathering the
    // nearest node so its pre-bell task is real.
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    // Move it to a known spot between the TCs first: 6 tiles east.
    let (mx, my) = sim.grid().cell_center(c1x + 6, c1y + 2);
    sim.push_command(Command::Move {
        entity: vil,
        x: mx,
        y: my,
    });
    let _ = run_until(&mut sim, 30_000, |s| {
        let e = ent(s, vil);
        crate::steer::vec_len(e.pos_x - mx, e.pos_y - my) < I16F16::from_num(2)
    });
    let pos = {
        let e = ent(&sim, vil);
        (e.pos_x, e.pos_y)
    };
    let node = nearest_node(&sim, pos);
    sim.push_command(Command::Gather {
        entity: vil,
        node,
    });
    let working = run_until(&mut sim, 30_000, |s| {
        matches!(ent(s, vil).task, Task::ToNode | Task::Gathering)
    });
    assert!(working, "the villager never started gathering");
    // Ring TC1: snapshot of the gather task, walk to shelter.
    sim.push_command(Command::TownBell { tc: tc1 });
    sim.tick();
    assert!(ent(&sim, vil).bell_snapshot.is_some(), "TC1's ring took no snapshot");
    // Ring TC2 at once, while the villager is still walking: the
    // snapshot must survive untouched.
    sim.push_command(Command::TownBell { tc: tc2 });
    sim.tick();
    let snap = ent(&sim, vil)
        .bell_snapshot
        .expect("TC2's ring consumed the snapshot");
    assert!(
        matches!(snap.task, Task::ToNode | Task::Gathering | Task::Returning),
        "TC2's ring overwrote the snapshot task with {:?}",
        snap.task
    );
    assert_eq!(snap.task_node, Some(node), "the snapshot lost its gather node");
}

// (c) A villager garrisoned MANUALLY before the ring is not a
// bell villager: the bell shelters the others around it, and
// back-to-work releases only the bell's own — the manual one
// stays inside.
#[test]
fn gar07_bell_leaves_manually_garrisoned_villager_inside() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let manual = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    sim.push_command(Command::GarrisonTc {
        entity: manual,
        tc,
    });
    let inside = run_until(&mut sim, 30_000, |s| s.garrison_count_tc(tc) == 1);
    assert!(inside, "the manual garrison never completed");
    assert!(ent(&sim, manual).bell_snapshot.is_none());
    sim.push_command(Command::TownBell { tc });
    let all_in = run_until(&mut sim, 60_000, |s| {
        player_villager_ids(s)
            .into_iter()
            .all(|id| ent(s, id).garrisoned_in == Some(GarrisonRef::TownCenter(tc)))
    });
    assert!(all_in, "the bell did not shelter the remaining villagers");
    assert_eq!(sim.garrison_count_tc(tc), 4);
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    // The 3 bell villagers are out; the manual one stays.
    assert_eq!(
        sim.garrison_count_tc(tc),
        1,
        "back-to-work must leave exactly the manual garrison inside"
    );
    assert_eq!(
        ent(&sim, manual).garrisoned_in,
        Some(GarrisonRef::TownCenter(tc))
    );
    for id in player_villager_ids(&sim) {
        if id != manual {
            assert!(ent(&sim, id).garrisoned_in.is_none(), "bell villager {id} still in");
        }
    }
}

// (d) A villager PRODUCED while the bell is active is not a bell
// villager: it spawns outside, snapshot-free, and back-to-work
// does not touch it.
#[test]
fn gar07_bell_villager_produced_during_bell_is_untouched() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    sim.push_command(Command::TownBell { tc });
    let all_in = run_until(&mut sim, 60_000, |s| {
        player_villager_ids(s)
            .into_iter()
            .all(|id| ent(s, id).garrisoned_in == Some(GarrisonRef::TownCenter(tc)))
    });
    assert!(all_in, "the bell did not shelter the starting villagers");
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    let before = player_villager_ids(&sim).len();
    sim.push_command(Command::TrainVillagerAt { tc });
    let born = run_until(&mut sim, 60_000, |s| player_villager_ids(s).len() == before + 1);
    assert!(born, "no villager was produced during the bell");
    let new_vil = player_villager_ids(&sim)
        .into_iter()
        .find(|id| ent(&sim, *id).bell_snapshot.is_none() && ent(&sim, *id).garrisoned_in.is_none())
        .expect("the new villager stands outside, snapshot-free");
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    assert_eq!(sim.garrison_count_tc(tc), 0, "the bell villagers did not all come out");
    let e = ent(&sim, new_vil);
    assert!(e.garrisoned_in.is_none() && e.bell_snapshot.is_none());
}

// (e) Fast double ring: ring again while everyone is still
// walking to shelter. Nobody may be stranded: no snapshots, no
// garrison walks, nobody inside — and a later full ring cycle
// still works.
#[test]
fn gar07_bell_fast_double_ring_strands_nobody() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    sim.push_command(Command::TownBell { tc });
    for _ in 0..5 {
        sim.tick();
    }
    sim.push_command(Command::TownBell { tc });
    for _ in 0..5 {
        sim.tick();
    }
    assert!(!sim.tc_bell_active(tc));
    for id in player_villager_ids(&sim) {
        let e = ent(&sim, id);
        assert!(e.bell_snapshot.is_none(), "villager {id} kept its snapshot");
        assert!(e.garrison_target.is_none(), "villager {id} kept a garrison walk");
        assert!(e.garrisoned_in.is_none(), "villager {id} is garrisoned");
    }
    // A fresh full cycle still works afterwards.
    sim.push_command(Command::TownBell { tc });
    let all_in = run_until(&mut sim, 60_000, |s| {
        player_villager_ids(s)
            .into_iter()
            .all(|id| ent(s, id).garrisoned_in == Some(GarrisonRef::TownCenter(tc)))
    });
    assert!(all_in, "the bell no longer shelters after a fast double ring");
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    assert_eq!(sim.garrison_count_tc(tc), 0);
}

// (f) The TC's rally point sits ON the TC itself, and a villager
// is produced while the bell is active: the rally sends the new
// villager inside as a production garrison (GAR-08) — it holds
// no bell snapshot, so back-to-work releases the bell's own and
// leaves the produced one inside.
#[test]
fn gar07_bell_with_rally_on_tc_keeps_production_garrison() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let (tx, ty) = {
        let e = ent(&sim, tc);
        (e.pos_x, e.pos_y)
    };
    sim.push_command(Command::SetRallyTc { tc, x: tx, y: ty });
    sim.tick();
    sim.push_command(Command::TownBell { tc });
    let all_in = run_until(&mut sim, 60_000, |s| {
        player_villager_ids(s)
            .into_iter()
            .all(|id| ent(s, id).garrisoned_in == Some(GarrisonRef::TownCenter(tc)))
    });
    assert!(all_in, "the bell did not shelter the starting villagers");
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    sim.push_command(Command::TrainVillagerAt { tc });
    // The produced villager rallies into the TC (production entry)
    // or, if the rally does not take it inside, stands outside —
    // either way it holds no snapshot and the bell villagers are
    // unaffected. Wait for training to finish either way.
    let born = run_until(&mut sim, 60_000, |s| {
        player_villager_ids(s).len() == 4
            && player_villager_ids(s).into_iter().any(|id| {
                let e = ent(s, id);
                e.bell_snapshot.is_none()
                    && (e.garrisoned_in == Some(GarrisonRef::TownCenter(tc))
                        || (e.garrisoned_in.is_none() && e.garrison_target.is_none()))
            })
    });
    assert!(born, "the villager was never produced during the bell");
    let produced = player_villager_ids(&sim)
        .into_iter()
        .find(|id| ent(&sim, *id).bell_snapshot.is_none())
        .unwrap();
    let produced_inside = ent(&sim, produced).garrisoned_in.is_some();
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    // Every bell villager is out; the produced one is exactly
    // where it was (inside as production garrison, or outside).
    for id in player_villager_ids(&sim) {
        if id != produced {
            assert!(
                ent(&sim, id).garrisoned_in.is_none(),
                "bell villager {id} still inside after back-to-work"
            );
        }
    }
    assert_eq!(
        ent(&sim, produced).garrisoned_in.is_some(),
        produced_inside,
        "back-to-work moved the production villager"
    );
    assert_eq!(
        sim.garrison_count_tc(tc),
        if produced_inside { 1 } else { 0 }
    );
}
