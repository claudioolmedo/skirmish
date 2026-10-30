//! Castle P4a tests (2026-10-30): the Siege Workshop (200 W,
//! 40 s, 4×4, requires a completed Blacksmith, Castle Age) and
//! the Battering Ram spec row — placement gates through the
//! real Build command path, training through the real TrainAt
//! path, and the scripted AI's siege slice (build chain,
//! production, wave targeting).
//!
//! GAR-10's garrison behavior lives in `garrison_tests.rs`;
//! the negative-armor formula pins live in `sim/tests/combat.rs`.
//! Fixture style follows `university_p3c_tests` (direct records;
//! ages set on the player slots).

use crate::build::{Building, BuildingKind};
use crate::classes as cls;
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::Age;
use crate::{Command, MatchMode, Sim};
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

fn sim_in_castle() -> Sim {
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Castle;
    sim.players[1].age = Age::Castle;
    sim
}

/// A legal build anchor for `kind` near the player's Town
/// Center, found with the sim's own placement validation (the
/// AI's `find_anchor` discipline), or None.
fn legal_anchor(sim: &Sim, builder: u32, kind: BuildingKind) -> Option<(u32, u32)> {
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    for r in 3..=10i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let (x, y) = sim.grid().cell_center(ax, ay);
                if sim.can_build(builder, kind, x, y) {
                    return Some((ax, ay));
                }
            }
        }
    }
    None
}

fn own_villager(sim: &Sim) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
        .unwrap()
        .id
}

// ------------------------------------------------------------------
// The Siege Workshop's data row and gates.

#[test]
fn p4a_siege_workshop_row() {
    let k = BuildingKind::SiegeWorkshop;
    assert_eq!(k.tag(), 17);
    assert_eq!(BuildingKind::from_tag(17), Some(k));
    assert_eq!(f(k.wood_cost()), 200.0);
    assert_eq!(f(k.stone_cost()), 0.0);
    assert_eq!(k.build_ticks(), 2400); // 40 s
    assert_eq!(k.footprint_size(), 4); // RISKS R8: 4×4, not 3×3
    assert_eq!(k.max_hp(), 1800); // Castle column
    assert_eq!(k.max_hp_at(Age::Castle), 1800);
    assert_eq!(k.max_hp_at(Age::Imperial), 2100);
    assert_eq!(k.armor_melee(), 2);
    assert_eq!(k.armor_pierce(), 9);
    assert_eq!(k.sight_tiles_at(Age::Castle), 6);
    assert_eq!(k.age_required(), Age::Castle);
    assert_eq!(k.requires_building(), Some(BuildingKind::Blacksmith));
    assert!(k.is_production());
    assert!(k.produces_units());
    assert!(k.counts_toward_imperial());
    assert!(k.trains(EntityKind::BatteringRam));
    assert!(!k.trains(EntityKind::Militia));
    assert!(!k.trains(EntityKind::Villager));
    // No weapon of its own — the workshop does not fire.
    assert!(k.weapon().is_none());
    assert_eq!(k.pop_support(), 0);
}

#[test]
fn p4a_workshop_placement_gates() {
    // Feudal Age: refused even with a Blacksmith standing.
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Feudal;
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::Blacksmith, OWNER_PLAYER, (70, 70)));
    let vil = own_villager(&sim);
    let wood_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood);
    assert!(legal_anchor(&sim, vil, BuildingKind::SiegeWorkshop).is_none());
    let (x, y) = {
        let tc = sim.town_center();
        (tc.pos_x + I16F16::from_num(24), tc.pos_y)
    };
    sim.push_command(Command::Build {
        entity: vil,
        kind: BuildingKind::SiegeWorkshop,
        x,
        y,
    });
    for _ in 0..300 {
        sim.tick();
    }
    assert!(
        sim.buildings()
            .iter()
            .all(|b| b.kind != BuildingKind::SiegeWorkshop),
        "a Feudal player raised a Siege Workshop"
    );
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        wood_before,
        "a refused placement spent wood"
    );

    // Castle Age without a Blacksmith: refused (BLD-15's chain).
    let mut sim = sim_in_castle();
    let vil = own_villager(&sim);
    assert!(legal_anchor(&sim, vil, BuildingKind::SiegeWorkshop).is_none());

    // Castle Age with a completed Blacksmith: builds to
    // completion through the ordinary command path.
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::Blacksmith, OWNER_PLAYER, (70, 70)));
    let vil = own_villager(&sim);
    let (ax, ay) = legal_anchor(&sim, vil, BuildingKind::SiegeWorkshop)
        .expect("no legal Siege Workshop anchor beside the Blacksmith");
    let (x, y) = sim.grid().cell_center(ax, ay);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    sim.push_command(Command::Build {
        entity: vil,
        kind: BuildingKind::SiegeWorkshop,
        x,
        y,
    });
    let built = run_until(&mut sim, 40_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::SiegeWorkshop && b.complete)
    });
    assert!(built, "the Siege Workshop never completed");
}

// ------------------------------------------------------------------
// The Battering Ram's data row and training.

#[test]
fn p4a_battering_ram_row() {
    let s = crate::units::spec(EntityKind::BatteringRam);
    assert_eq!(EntityKind::BatteringRam.tag(), 17);
    assert_eq!(EntityKind::from_tag(17), Some(EntityKind::BatteringRam));
    assert_eq!(s.max_hp, 175);
    assert_eq!(
        s.attacks,
        &[(cls::MELEE, 2), (cls::STANDARD_BUILDINGS, 150), (cls::SIEGE, 40)]
    );
    // The negative melee armor is data, not a typo (CMB-01's
    // formula amplifies through it — see the combat pins).
    assert_eq!(s.armor_melee, -3);
    assert_eq!(s.armor_pierce, 180);
    assert_eq!(s.classes, cls::bit(cls::SIEGE));
    assert_eq!(s.reload_ticks, 300); // 5.0 s
    assert_eq!(s.speed.to_bits(), 2_621); // 0.6 tiles/s (empty)
    assert_eq!(s.los.to_bits(), 1_310_720); // 5 tiles
    assert_eq!(s.food_cost, 0);
    assert_eq!(s.wood_cost, 160);
    assert_eq!(s.gold_cost, 75);
    assert_eq!(s.train_ticks, 2160); // 36 s
    assert_eq!(EntityKind::BatteringRam.train_age(), Age::Castle);
    assert!(EntityKind::BatteringRam.counts_population());
    assert!(EntityKind::BatteringRam.is_trainable());
    // Siege never garrisons (GAR-01).
    assert!(!EntityKind::BatteringRam.is_foot_garrisonable());
    assert!(!EntityKind::BatteringRam.is_mounted_garrisonable());
}

#[test]
fn p4a_ram_trains_at_the_workshop_only() {
    let mut sim = sim_in_castle();
    sim.buildings.push(placed(
        &sim,
        900,
        BuildingKind::SiegeWorkshop,
        OWNER_PLAYER,
        (70, 70),
    ));
    sim.buildings
        .push(placed(&sim, 901, BuildingKind::Barracks, OWNER_PLAYER, (80, 70)));
    // Population room for the trainee (the HOUSED rule).
    sim.buildings
        .push(placed(&sim, 902, BuildingKind::House, OWNER_PLAYER, (60, 80)));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(500));
    assert!(sim.can_train_at(900, EntityKind::BatteringRam));
    assert!(!sim.can_train_at(901, EntityKind::BatteringRam));
    sim.push_command(Command::TrainAt {
        building: 900,
        kind: EntityKind::BatteringRam,
    });
    let trained = run_until(&mut sim, 20_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::BatteringRam && e.owner == OWNER_PLAYER)
    });
    assert!(trained, "the workshop never produced a Battering Ram");
}

// ------------------------------------------------------------------
// The AI slice (AI-04 extension): build chain, production, waves.

#[test]
fn p4a_ai_builds_blacksmith_then_siege_workshop_in_castle() {
    let mut sim = Sim::new_match(7, MatchMode::AiNormal);
    sim.players[1].age = Age::Castle;
    sim.stockpile_mut(OWNER_OPPONENT)
        .add(ResourceKind::Wood, I16F16::from_num(5000));
    sim.stockpile_mut(OWNER_OPPONENT)
        .add(ResourceKind::Food, I16F16::from_num(5000));
    sim.stockpile_mut(OWNER_OPPONENT)
        .add(ResourceKind::Gold, I16F16::from_num(5000));
    let blacksmith = run_until(&mut sim, 120_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.owner == OWNER_OPPONENT && b.kind == BuildingKind::Blacksmith && b.complete)
    });
    assert!(blacksmith, "the Castle-Age AI never raised a Blacksmith");
    let workshop = run_until(&mut sim, 120_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.owner == OWNER_OPPONENT && b.kind == BuildingKind::SiegeWorkshop)
    });
    assert!(
        workshop,
        "the AI never placed a Siege Workshop behind its Blacksmith"
    );
}

#[test]
fn p4a_ai_trains_rams_and_sends_them_at_buildings() {
    let mut sim = Sim::new_match(7, MatchMode::AiNormal);
    sim.players[1].age = Age::Castle;
    sim.stockpile_mut(OWNER_OPPONENT)
        .add(ResourceKind::Wood, I16F16::from_num(5000));
    sim.stockpile_mut(OWNER_OPPONENT)
        .add(ResourceKind::Food, I16F16::from_num(5000));
    sim.stockpile_mut(OWNER_OPPONENT)
        .add(ResourceKind::Gold, I16F16::from_num(5000));
    // Housing for the AI's growth, a completed Blacksmith and
    // Siege Workshop near the opponent's Town Center.
    let (tcx, tcy) = {
        let tc = sim.tc_entity_of(OWNER_OPPONENT).unwrap();
        sim.grid().world_to_cell(tc.pos_x, tc.pos_y)
    };
    sim.buildings.push(placed(
        &sim,
        900,
        BuildingKind::House,
        OWNER_OPPONENT,
        (tcx + 8, tcy + 8),
    ));
    sim.buildings.push(placed(
        &sim,
        901,
        BuildingKind::House,
        OWNER_OPPONENT,
        (tcx + 11, tcy + 8),
    ));
    sim.buildings.push(placed(
        &sim,
        902,
        BuildingKind::Blacksmith,
        OWNER_OPPONENT,
        (tcx + 8, tcy + 12),
    ));
    sim.buildings.push(placed(
        &sim,
        903,
        BuildingKind::SiegeWorkshop,
        OWNER_OPPONENT,
        (tcx + 13, tcy + 12),
    ));
    // The AI keeps a ram corps: its own TrainAt path produces one.
    let trained = run_until(&mut sim, 60_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::BatteringRam && e.owner == OWNER_OPPONENT)
    });
    assert!(trained, "the AI's Siege Workshop never produced a Ram");

    // Waves: a Ram staged with a full wave beside an explored
    // player House joins the wave and is sent at a BUILDING
    // through the ordinary paths — never at a unit. The wave's
    // shipped target order (which the Ram follows like every
    // member; only its unit hunt is skipped) is visible units,
    // then the Town Center once explored, then the nearest
    // explored building: by staging time the AI's scout has
    // explored the player's Town Center, so the Ram's order
    // lands on the TC entity (Attack) — the AttackBuilding form
    // against the staged House is accepted too, and a unit
    // target that is not the TC fails the test.
    let ram = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::BatteringRam && e.owner == OWNER_OPPONENT)
        .unwrap()
        .id;
    let (rx, ry) = {
        let r = ent(&sim, ram);
        sim.grid().world_to_cell(r.pos_x, r.pos_y)
    };
    for i in 0..5u32 {
        spawn(
            &mut sim,
            950 + i,
            EntityKind::Militia,
            OWNER_OPPONENT,
            (rx + 2 + i, ry + 3),
        );
    }
    sim.buildings.push(placed(
        &sim,
        980,
        BuildingKind::House,
        OWNER_PLAYER,
        (rx + 4, ry),
    ));
    let player_tc = sim.tc_entity_of(OWNER_PLAYER).unwrap().id;
    sim.ai.next_wave_tick = 0;
    let sent = run_until(&mut sim, 30_000, |s| {
        let r = ent(s, ram);
        r.attack_building == Some(980) || r.attack_target == Some(player_tc)
    });
    assert!(
        sent,
        "the wave never sent the Ram at a building (TC or House)"
    );
    let r = ent(&sim, ram);
    assert!(
        r.attack_building == Some(980) || r.attack_target == Some(player_tc),
        "the Ram was ordered onto a unit hunt instead of a building: \
         attack_target={:?} attack_building={:?}",
        r.attack_target, r.attack_building
    );
}
