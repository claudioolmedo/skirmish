//! Castle P3a tests (2026-10-29): the Castle building (650 S,
//! 200 s, 4×4, 4800 HP, 8/11, LoS 11, +20 pop), its BLD-18
//! weapon with the base volley of 5, and the Petard (CMB-17) —
//! Castle-only training, the documented bonus table measured
//! in-sim, and the suicide detonation (blast 0.5, CMB-12's
//! everyone-in-the-area rule).
//!
//! In-crate staging (direct entity/building records) follows
//! the `new_units_tests` fixture style; expectations are the
//! dossier's literal DE values (Castle Age dossier §2, fetched
//! 2026-10-22), not recomputed ones.

use crate::build::{Building, BuildingKind};
use crate::classes as cls;
use crate::combat::{building_spec, compute_damage};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::Age;
use crate::units::spec;
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

/// A complete, idle building record at a cell anchor (the
/// `new_units_tests::placed` fixture's shape, completed).
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
    }
}

// ------------------------------------------------------------------
// The Castle's table row (dossier §2).

#[test]
fn castle_table_row_matches_the_dossier() {
    let k = BuildingKind::Castle;
    assert_eq!(k.tag(), 15);
    assert_eq!(BuildingKind::from_tag(15), Some(k));
    assert_eq!(f(k.stone_cost()), 650.0);
    assert_eq!(f(k.wood_cost()), 0.0);
    assert_eq!(k.build_ticks(), 12_000); // 200 s
    assert_eq!(k.footprint_size(), 4);
    assert_eq!(k.max_hp(), 4800);
    assert_eq!(k.max_hp_at(Age::Castle), 4800);
    assert_eq!(k.armor_melee_at(Age::Castle), 8);
    assert_eq!(k.armor_pierce_at(Age::Castle), 11);
    assert_eq!(k.sight_tiles_at(Age::Castle), 11);
    assert_eq!(k.pop_support(), 20);
    assert_eq!(k.age_required(), Age::Castle);
    assert_eq!(k.requires_building(), None);
    assert!(k.is_production());
    assert!(k.produces_units());
    assert!(k.trains(EntityKind::Petard));
    assert!(!k.trains(EntityKind::Knight));

    // The weapon profile (BLD-18's second user): 11 pierce
    // +2 vs Spearmen, range 8 (min 1), RoF 2.0 s, 100%, volley 5.
    let w = k.weapon().expect("the Castle is armed");
    assert_eq!(w.attacks, &[(cls::PIERCE, 11), (cls::SPEARMAN, 2)]);
    assert_eq!(f(w.range_max), 32.0);
    assert_eq!(f(w.range_min), 4.0);
    assert_eq!(w.reload_ticks, 120);
    assert_eq!(w.accuracy, 100);
    assert_eq!(w.volley, 5);
    // The tower's profile is unchanged: a single shot.
    assert_eq!(BuildingKind::WatchTower.weapon().unwrap().volley, 1);
}

// ------------------------------------------------------------------
// Placement gates, the exact stone charge, population support.

#[test]
fn castle_placement_is_castle_age_only_and_supports_twenty() {
    let mut sim = Sim::new(7);
    let vil = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
        .unwrap()
        .id;
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    // First anchor the sim itself accepts, on rings around the TC.
    let mut anchor = None;
    'scan: for r in 3i32..=16 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (cx, cy) = (tcx as i32 + dx, tcy as i32 + dy);
                if cx < 0 || cy < 0 {
                    continue;
                }
                let (x, y) = sim.grid().cell_center(cx as u32, cy as u32);
                if sim.can_build(vil, BuildingKind::Castle, x, y) {
                    anchor = Some((cx as u32, cy as u32));
                    break 'scan;
                }
            }
        }
    }
    // Dark Age (even with the stone banked): refused.
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Stone, I16F16::from_num(1000));
    assert!(anchor.is_none(), "a Dark Age Castle must not place");
    // Castle Age: the same scan finds a home.
    sim.players[0].age = Age::Castle;
    let mut anchor = None;
    'scan2: for r in 3i32..=16 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (cx, cy) = (tcx as i32 + dx, tcy as i32 + dy);
                if cx < 0 || cy < 0 {
                    continue;
                }
                let (x, y) = sim.grid().cell_center(cx as u32, cy as u32);
                if sim.can_build(vil, BuildingKind::Castle, x, y) {
                    anchor = Some((cx as u32, cy as u32, x, y));
                    break 'scan2;
                }
            }
        }
    }
    let (_, _, x, y) = anchor.expect("no Castle anchor in Castle Age");
    let stone0 = f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Stone));
    let cap0 = sim.population_cap_of(OWNER_PLAYER);
    sim.push_command(Command::Build {
        entity: vil,
        kind: BuildingKind::Castle,
        x,
        y,
    });
    let up = run_until(&mut sim, 40_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::Castle && b.complete)
    });
    assert!(up, "the Castle never completed");
    // Exactly 650 stone left the stockpile at placement.
    let stone1 = f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Stone));
    assert_eq!(stone0 - stone1, 650.0);
    // +20 population room once it stands.
    assert_eq!(sim.population_cap_of(OWNER_PLAYER), cap0 + 20);
    let castle = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Castle)
        .unwrap();
    assert_eq!(f(castle.hp), 4800.0);
}

// ------------------------------------------------------------------
// The Petard's table row and its training route.

#[test]
fn petard_table_row_matches_the_dossier() {
    let p = spec(EntityKind::Petard);
    assert_eq!(EntityKind::Petard.tag(), 15);
    assert_eq!(EntityKind::from_tag(15), Some(EntityKind::Petard));
    assert_eq!(EntityKind::Petard.train_age(), Age::Castle);
    assert_eq!(p.max_hp, 50);
    assert_eq!(
        p.attacks,
        &[
            (cls::MELEE, 25),
            (cls::WALLS_AND_GATES, 900),
            (cls::STANDARD_BUILDINGS, 500),
            (cls::CASTLE, 100),
            (cls::SIEGE, 60),
        ]
    );
    assert_eq!((p.armor_melee, p.armor_pierce), (0, 2));
    assert_eq!(p.speed.to_bits(), 3_495); // 0.8 tiles/s
    assert_eq!(f(p.los), 16.0); // 4 tiles
    assert_eq!((p.food_cost, p.gold_cost), (65, 20));
    assert_eq!(p.train_ticks, 1500); // 25 s
    assert!(EntityKind::Petard.counts_population());
    assert!(EntityKind::Petard.is_trainable());
    assert!(EntityKind::Petard.is_military());
}

#[test]
fn petard_trains_at_the_castle_only() {
    let mut sim = Sim::new(7);
    let castle = placed(&sim, 900, BuildingKind::Castle, OWNER_PLAYER, (58, 58));
    sim.buildings.push(castle);
    let rax = placed(&sim, 901, BuildingKind::Barracks, OWNER_PLAYER, (66, 58));
    sim.buildings.push(rax);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(500));

    // Feudal: the Castle stands but the Petard is a Castle unit.
    sim.players[0].age = Age::Feudal;
    assert!(!sim.can_train_at(900, EntityKind::Petard));
    // Castle Age: trainable at the Castle — and nowhere else.
    sim.players[0].age = Age::Castle;
    assert!(sim.can_train_at(900, EntityKind::Petard));
    assert!(!sim.can_train_at(901, EntityKind::Petard));
    assert!(!sim.can_train_at(900, EntityKind::Knight));

    let (food0, gold0) = (
        f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)),
        f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Gold)),
    );
    sim.push_command(Command::TrainAt {
        building: 900,
        kind: EntityKind::Petard,
    });
    let born = run_until(&mut sim, 5_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Petard && !e.dead)
    });
    assert!(born, "the Petard never trained");
    // Exact charges: 65 food, 20 gold.
    assert_eq!(
        food0 - f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)),
        65.0
    );
    assert_eq!(
        gold0 - f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Gold)),
        20.0
    );
}

// ------------------------------------------------------------------
// The bonus table, measured through the damage formula.

#[test]
fn petard_bonus_table_measures_as_documented() {
    let p = spec(EntityKind::Petard);
    // Vs a Castle: (25 − 8 melee armor) + 500 buildings + 100 castles.
    assert_eq!(
        compute_damage(p.attacks, &building_spec(BuildingKind::Castle, Age::Castle)),
        617
    );
    // Vs a Stone Wall (Feudal): (25 − 8) + 900 walls + 500 buildings.
    assert_eq!(
        compute_damage(
            p.attacks,
            &building_spec(BuildingKind::StoneWall, Age::Feudal)
        ),
        1417
    );
    // Vs an ordinary building (Barracks, Castle column 2 melee):
    // (25 − 2) + 500 — no walls/castle rows participate.
    assert_eq!(
        compute_damage(
            p.attacks,
            &building_spec(BuildingKind::Barracks, Age::Castle)
        ),
        523
    );
    // Vs units the bonus rows stay silent: a Knight takes the
    // plain melee pair through its 2 melee armor; the +60 vs
    // Siege row has no member kind yet and never participates.
    assert_eq!(compute_damage(p.attacks, spec(EntityKind::Knight)), 23);
    assert_eq!(compute_damage(p.attacks, spec(EntityKind::Militia)), 25);
}

// ------------------------------------------------------------------
// The Castle's volley: five projectiles per release.

#[test]
fn castle_volley_looses_five_projectiles() {
    let mut sim = Sim::new(7);
    let castle = placed(&sim, 900, BuildingKind::Castle, OWNER_PLAYER, (58, 58));
    sim.buildings.push(castle);
    // An enemy villager standing in the band, ~4 tiles east of
    // the footprint's edge (inside 8, outside the 1-tile minimum).
    spawn(
        &mut sim,
        900,
        EntityKind::Villager,
        OWNER_OPPONENT,
        (68, 60),
    );
    let mut max_volley = 0usize;
    for _ in 0..900 {
        sim.tick();
        let n = sim
            .projectiles()
            .iter()
            .filter(|p| p.attacker_building == Some(900))
            .count();
        max_volley = max_volley.max(n);
        if ent(&sim, 900).dead {
            break;
        }
    }
    assert_eq!(
        max_volley, 5,
        "the Castle never loosed its base volley of 5"
    );
    // Five 11-pierce arrows kill a 25 HP villager (0 pierce armor).
    assert!(ent(&sim, 900).dead, "the volley's target survived");
}

// ------------------------------------------------------------------
// CMB-17: detonation on a building strike and on a unit strike.

#[test]
fn petard_detonates_on_a_wall_and_dies() {
    let mut sim = Sim::new(7);
    let wall = placed(&sim, 900, BuildingKind::StoneWall, OWNER_OPPONENT, (60, 58));
    sim.buildings.push(wall);
    spawn(&mut sim, 900, EntityKind::Petard, OWNER_PLAYER, (58, 58));
    sim.push_command(Command::AttackBuilding {
        entity: 900,
        building: 900,
    });
    let done = run_until(&mut sim, 10_000, |s| {
        s.buildings().iter().all(|b| b.id != 900) && ent(s, 900).dead
    });
    assert!(done, "wall standing or Petard alive after 10 000 ticks");
    // One strike: 1417 damage against the wall's 1080 HP.
    assert!(sim.buildings().iter().all(|b| b.id != 900));
    assert!(ent(&sim, 900).dead, "the Petard survived its own strike");
}

#[test]
fn petard_blast_hits_everyone_in_the_radius_but_not_the_primary_twice() {
    let mut sim = Sim::new(7);
    // Staged in world coordinates around one open cell: the
    // Petard starts at striking distance (2.0) of the primary
    // target; an enemy bystander and a FRIENDLY villager stand
    // pressed against the primary, well inside the 0.5-tile
    // (2.0 wu) blast radius of its position. CMB-12: a
    // blast-flagged weapon hurts friend and foe alike.
    let (bx, by) = sim.grid().cell_center(58, 58);
    let put = |sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, dx: i32, dy: i32| {
        sim.entities.push(Entity::spawned(
            id,
            kind,
            owner,
            bx + I16F16::from_num(dx),
            by + I16F16::from_num(dy),
        ));
    };
    put(&mut sim, 900, EntityKind::Petard, OWNER_PLAYER, -2, 0);
    put(&mut sim, 901, EntityKind::Militia, OWNER_OPPONENT, 0, 0);
    put(&mut sim, 902, EntityKind::Militia, OWNER_OPPONENT, 1, 1);
    put(&mut sim, 903, EntityKind::Villager, OWNER_PLAYER, 1, -1);
    sim.push_command(Command::Attack {
        entity: 900,
        target: 901,
    });
    let done = run_until(&mut sim, 10_000, |s| ent(s, 900).dead);
    assert!(done, "the Petard never detonated");
    // Militia: 40 HP, 0 melee armor — one 25-melee application
    // each (the primary via the strike, the bystander via the
    // blast), never two.
    assert_eq!(f(ent(&sim, 901).hp), 15.0, "primary target");
    assert_eq!(f(ent(&sim, 902).hp), 15.0, "enemy bystander");
    // The friendly villager (25 HP, 0 armor) takes the blast's
    // full 25 and dies — friendly fire, as CMB-12 flags it.
    assert!(ent(&sim, 903).dead, "friendly bystander took no blast");
}
