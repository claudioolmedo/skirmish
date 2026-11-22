//! CLS-06 tests: kings and heroes as special entities — the
//! hero regeneration rate (30 HP/min, hero-mode bit 4), the
//! conversion immunity read (bit 2, via MNK-08's existing gate),
//! the one documented hero subject (Liu Bei, tag 34,
//! test/scenario-only), and the King's garrison eligibility
//! (TC / towers / Castle yes, Ram no — the GAR-01 column plus
//! the CLS-06 Ram exclusion).
//!
//! Fixture style follows `garrison_tests` (direct records);
//! expectations are the documented DE values (wiki, Hero /
//! King / Garrison; Counterforge, Liu Bei), not recomputed ones.

use crate::build::{Building, BuildingKind};
use crate::entity::{Entity, EntityKind, GarrisonRef, OWNER_OPPONENT, OWNER_PLAYER};
use crate::{Command, MatchMode, MatchState, Sim, VictorySettings};
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

fn wound(sim: &mut Sim, id: u32, hp: i32) {
    let ei = sim.entities.iter().position(|e| e.id == id).unwrap();
    sim.entities[ei].hp = I16F16::from_num(hp);
}

fn placed(sim: &Sim, id: u32, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> Building {
    let size = kind.footprint().w;
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

// ------------------------------------------------------------------
// The hero subject: one documented named hero, scenario-only.

#[test]
fn cls06_hero_row_is_the_documented_liu_bei() {
    let row = crate::units::spec(EntityKind::LiuBei);
    // Wiki, Hero + Liu Bei; Counterforge unit page (fetched
    // 2026-11-22): HP 425, armor 3/3, attack 15 melee +3 vs
    // Shock Infantry, RoF 2.0 s, accuracy 100%, speed 0.9, LoS 5.
    assert_eq!(row.max_hp, 425);
    assert_eq!(row.armor_melee, 3);
    assert_eq!(row.armor_pierce, 3);
    assert_eq!(
        row.attacks,
        &[(crate::classes::MELEE, 15), (crate::classes::SHOCK_INFANTRY, 3)]
    );
    assert_eq!(row.reload_ticks, 120);
    assert_eq!(row.accuracy, 100);
    assert_eq!(row.speed, I16F16::from_bits(3_932)); // 0.90 tiles/s
    assert_eq!(row.los, I16F16::from_bits(1_310_720)); // 20 (5 tiles)
    // Classes: Infantry + Heroes & Kings + Unique Units.
    let mask = crate::classes::mask(&[
        crate::classes::INFANTRY,
        crate::classes::HEROES_AND_KINGS,
        crate::classes::UNIQUE_UNIT,
    ]);
    assert_eq!(row.classes, mask);
    // The CLS-06 hero flags: bit 2 (immune) + bit 4 (regen
    // 30 HP/min); NOT a regicide unit.
    assert!(row.immune_conversion);
    assert_eq!(row.regenerates, 30);
    assert!(!row.regicide_unit);
    // Scenario-only: tag 34, never trainable, no train time.
    assert_eq!(EntityKind::LiuBei.tag(), 34);
    assert_eq!(EntityKind::from_tag(34), Some(EntityKind::LiuBei));
    assert!(!EntityKind::LiuBei.is_trainable());
    assert_eq!(row.train_ticks, 0);
    // The King keeps his documented distinction: no regeneration.
    let king = crate::units::spec(EntityKind::King);
    assert_eq!(king.regenerates, 0);
    assert!(king.regicide_unit);
    assert!(king.immune_conversion);
}

// ------------------------------------------------------------------
// Regeneration: exactly 30 HP in exactly 3 600 ticks.

#[test]
fn cls06_hero_regenerates_exactly_30_hp_in_sixty_seconds() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::LiuBei, OWNER_PLAYER, (58, 58));
    wound(&mut sim, 900, 385);
    // 120 ticks = 2 s at 0.5 HP/s: exactly +1 HP, no remainder.
    for _ in 0..120 {
        sim.tick();
    }
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(386));
    assert_eq!(ent(&sim, 900).regen_remainder, 0);
    // The full minute: exactly +30 HP from the wounded value.
    for _ in 0..(3600 - 120) {
        sim.tick();
    }
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(415));
}

#[test]
fn cls06_hero_regeneration_caps_at_max_hp() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::LiuBei, OWNER_PLAYER, (58, 58));
    wound(&mut sim, 900, 424);
    for _ in 0..3600 {
        sim.tick();
    }
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(425));
    assert_eq!(ent(&sim, 900).regen_remainder, 0);
}

#[test]
fn cls06_dead_hero_does_not_regenerate() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::LiuBei, OWNER_PLAYER, (58, 58));
    wound(&mut sim, 900, 100);
    let ei = sim.entities.iter().position(|e| e.id == 900).unwrap();
    sim.entities[ei].dead = true;
    for _ in 0..600 {
        sim.tick();
    }
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(100));
    assert_eq!(ent(&sim, 900).regen_remainder, 0);
}

#[test]
fn cls06_king_regenerates_zero_in_sixty_seconds() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::King, OWNER_PLAYER, (58, 58));
    wound(&mut sim, 900, 45);
    for _ in 0..3600 {
        sim.tick();
    }
    // "Kings, unlike heroes, do not regenerate hit points."
    assert_eq!(ent(&sim, 900).hp, I16F16::from_num(45));
    assert_eq!(ent(&sim, 900).regen_remainder, 0);
}

// ------------------------------------------------------------------
// Conversion immunity through the existing MNK-08 gate.

#[test]
fn cls06_hero_is_refused_by_the_conversion_gate() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 900, EntityKind::Monk, OWNER_OPPONENT, (58, 58));
    spawn(&mut sim, 901, EntityKind::LiuBei, OWNER_PLAYER, (59, 58));
    spawn(&mut sim, 902, EntityKind::Militia, OWNER_PLAYER, (58, 59));
    assert!(!sim.can_convert_target(900, 901), "hero converted");
    // Control: an ordinary military unit IS eligible, so the
    // refusal above is the hero flag, not a broken gate.
    assert!(sim.can_convert_target(900, 902), "control refused");
}

// ------------------------------------------------------------------
// King garrison: TC / tower / Castle yes, Ram no.

#[test]
fn cls06_king_garrison_eligibility_matches_de() {
    let mut sim = Sim::new(7);
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::WatchTower, OWNER_PLAYER, (58, 58)));
    sim.buildings
        .push(placed(&sim, 901, BuildingKind::Castle, OWNER_PLAYER, (70, 58)));
    spawn(&mut sim, 910, EntityKind::King, OWNER_PLAYER, (60, 62));
    spawn(&mut sim, 911, EntityKind::BatteringRam, OWNER_PLAYER, (62, 62));
    spawn(&mut sim, 912, EntityKind::Militia, OWNER_PLAYER, (60, 64));
    let tc = sim.town_center().id;
    assert!(sim.can_garrison_tc(910, tc), "King refused by TC");
    assert!(sim.can_garrison_building(910, 900), "King refused by tower");
    assert!(sim.can_garrison_building(910, 901), "King refused by Castle");
    // The Ram exclusion: the King is foot-garrisonable in the
    // data column, yet the Ram arm refuses him (wiki, Garrison).
    assert!(EntityKind::King.is_foot_garrisonable());
    assert!(!sim.can_garrison_unit(910, 911), "King admitted by Ram");
    // Control: an ordinary foot unit IS admitted by the Ram.
    assert!(sim.can_garrison_unit(912, 911), "control refused by Ram");
}

#[test]
fn cls06_king_walks_into_the_town_center() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let (tx, ty) = (ent(&sim, tc).pos_x, ent(&sim, tc).pos_y);
    let (cx, cy) = sim.grid().world_to_cell(tx, ty);
    spawn(&mut sim, 900, EntityKind::King, OWNER_PLAYER, (cx + 3, cy));
    sim.push_command(Command::GarrisonTc {
        entity: 900,
        tc,
    });
    let inside = run_until(&mut sim, 20_000, |s| s.garrison_count_tc(tc) == 1);
    assert!(inside, "the King never garrisoned in the TC");
    assert_eq!(
        ent(&sim, 900).garrisoned_in,
        Some(GarrisonRef::TownCenter(tc))
    );
}

// ------------------------------------------------------------------
// VIC-06 regression: the Regicide death sweep still eliminates.

#[test]
fn cls06_regicide_king_death_still_eliminates_the_owner() {
    let mut sim = Sim::new_match_victory(
        7,
        MatchMode::TwoPlayer,
        VictorySettings {
            regicide: true,
            ..VictorySettings::conquest()
        },
    );
    let king = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::King && e.owner == OWNER_PLAYER)
        .unwrap()
        .id;
    sim.push_command(Command::DeleteEntity { entity: king });
    for _ in 0..5 {
        sim.tick();
    }
    assert!(ent(&sim, king).dead);
    assert_eq!(sim.match_state(), MatchState::Defeat);
}
