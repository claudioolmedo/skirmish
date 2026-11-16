//! PRJ-02 / PRJ-04 convergence pins (2026-11-16).
//!
//! PRJ-02: the generic arrow speed is the DOCUMENTED 7 tiles/s
//! (AoE wiki projectile data via the Age of Empires forum's
//! projectile-speed discussion — the speed of archers', arrow
//! towers' and Spanish cannon galleons' projectiles), and Town
//! Center arrows carry their own documented 8 tiles/s (wiki,
//! Update 153015 changelog: "Town Center arrows projectile speed
//! increased from 7 → 8"). The Mangonel's documented 3.5 tiles/s
//! was already exact.
//!
//! PRJ-04: a miss scatters uniformly in the SQUARE whose side is
//! the weapon's dispersion × the shot's distance (the AoE2 info
//! sheet's accuracy-error-width model: "the projectile can land
//! in a square with a side length of the error width times the
//! range"; archer-type dispersion 0.33, gunpowder/camel-archer
//! 0.75, Arambai 0.5, Trebuchet 0.2, cannon galleons 0.1).
//!
//! Fixture style follows `university_p3c_tests`: units and
//! buildings are placed directly.

use crate::build::{
    Building, BuildingKind, CASTLE_WEAPON, DOCK_WEAPON, TOWN_CENTER_WEAPON, WATCH_TOWER_WEAPON,
};
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::units::spec;
use crate::{Command, Sim};
use fixed::types::I16F16;

fn wu(n: i32) -> I16F16 {
    I16F16::from_num(n)
}

fn spawn(sim: &mut Sim, kind: EntityKind, owner: u8, x: I16F16, y: I16F16) -> u32 {
    let id = 7000 + sim.entities.len() as u32;
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
    id
}

fn placed(sim: &mut Sim, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> u32 {
    let id = 900 + sim.buildings.len() as u32;
    let size = kind.footprint_size();
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    let hp = sim.building_max_hp(kind, owner);
    sim.buildings.push(Building {
        id,
        kind,
        owner,
        hp: I16F16::from_num(hp),
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
    });
    id
}

fn building_pos(sim: &Sim, id: u32) -> (I16F16, I16F16) {
    let b = sim
        .buildings
        .iter()
        .find(|b| b.id == id)
        .expect("building stands");
    (b.pos_x, b.pos_y)
}

// ----------------------------------------------------------------
// PRJ-02 — documented projectile speeds.

#[test]
fn prj02_projectile_speeds_are_the_documented_values() {
    // 7 tiles/s at the sim's scale (1 tile = 4 wu, 60 ticks/s):
    // floor(7 × 4 × 65536 / 60) = 30_583 bits. The Archer carries
    // it; every row whose docs record adopting the Archer's /
    // generic constants follows it.
    for kind in [
        EntityKind::Archer,
        EntityKind::Skirmisher,
        EntityKind::CavalryArcher,
        EntityKind::Galley,
        EntityKind::WarGalley,
        EntityKind::FireGalley,
        EntityKind::FireShip,
        EntityKind::Scorpion,
    ] {
        assert_eq!(
            spec(kind).projectile_speed.to_bits(),
            30_583,
            "{kind:?} should fly at the documented 7 tiles/s"
        );
    }
    // The Mangonel's own documented figure (3.5 tiles/s) was
    // already exact and does not move.
    assert_eq!(spec(EntityKind::Mangonel).projectile_speed.to_bits(), 15_291);
    // Arrow buildings fly at the generic documented 7; the Town
    // Center at its own documented 8 (update 153015):
    // floor(8 × 4 × 65536 / 60) = 34_952 bits.
    assert_eq!(WATCH_TOWER_WEAPON.projectile_speed.to_bits(), 30_583);
    assert_eq!(CASTLE_WEAPON.projectile_speed.to_bits(), 30_583);
    assert_eq!(DOCK_WEAPON.projectile_speed.to_bits(), 30_583);
    assert_eq!(TOWN_CENTER_WEAPON.projectile_speed.to_bits(), 34_952);
}

#[test]
fn prj02_archer_flight_at_four_tiles_is_35_ticks() {
    let mut sim = Sim::new(7);
    // Two archers exactly 4 tiles (16 wu) apart — both in range
    // at spawn, so neither moves. The first projectile's flight
    // is ceil(1_048_576 / 30_583) = 35 ticks (the adopted
    // 7.5 tiles/s gave exactly 32).
    let a = spawn(&mut sim, EntityKind::Archer, OWNER_PLAYER, wu(40), wu(40));
    let b = spawn(&mut sim, EntityKind::Archer, OWNER_OPPONENT, wu(56), wu(40));
    sim.push_command(Command::Attack { entity: a, target: b });
    sim.push_command(Command::Attack { entity: b, target: a });
    for _ in 0..600 {
        sim.tick();
        if let Some(p) = sim.projectiles().first() {
            assert_eq!(p.impact_tick - p.spawn_tick, 35);
            return;
        }
    }
    panic!("no projectile was released");
}

#[test]
fn prj02_archer_flight_to_a_building_center_uses_the_same_speed() {
    let mut sim = Sim::new(7);
    let house = placed(&mut sim, BuildingKind::House, OWNER_OPPONENT, (40, 40));
    let (bx, by) = building_pos(&sim, house);
    // The Archer stands exactly 16 wu west of the center (the
    // footprint-edge distance is inside its band, so it stands
    // and fires); flight is measured to the center.
    let a = spawn(&mut sim, EntityKind::Archer, OWNER_PLAYER, bx - wu(16), by);
    sim.push_command(Command::AttackBuilding { entity: a, building: house });
    for _ in 0..600 {
        sim.tick();
        if let Some(p) = sim.projectiles().first() {
            assert_eq!(p.intended_building, Some(house));
            assert_eq!(p.impact_tick - p.spawn_tick, 35);
            return;
        }
    }
    panic!("no projectile was released");
}
