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

// ----------------------------------------------------------------
// PRJ-04 — the dispersion model.

#[test]
fn prj04_dispersion_constants_are_the_documented_values() {
    // 0.33 (any archer type) in bits: floor(0.33 × 65536) = 21_626.
    for kind in [
        EntityKind::Archer,
        EntityKind::Skirmisher,
        EntityKind::CavalryArcher,
        EntityKind::Galley,
        EntityKind::WarGalley,
        EntityKind::Scorpion,
    ] {
        assert_eq!(
            spec(kind).scatter.to_bits(),
            21_626,
            "{kind:?} should carry the documented archer dispersion 0.33"
        );
    }
    // The fire ships' spray never offsets — ZERO under both the
    // old fixed-radius model and the dispersion model.
    assert_eq!(spec(EntityKind::FireGalley).scatter.to_bits(), 0);
    assert_eq!(spec(EntityKind::FireShip).scatter.to_bits(), 0);
    // Arrow building weapons carry the archer-type 0.33 (inert
    // today: their accuracy is 100, so the roll never scatters).
    assert_eq!(WATCH_TOWER_WEAPON.scatter.to_bits(), 21_626);
    assert_eq!(CASTLE_WEAPON.scatter.to_bits(), 21_626);
    assert_eq!(TOWN_CENTER_WEAPON.scatter.to_bits(), 21_626);
    assert_eq!(DOCK_WEAPON.scatter.to_bits(), 21_626);
}

#[test]
fn prj04_half_side_is_dispersion_times_distance_over_two() {
    let mut sim = Sim::new(11);
    let aim = (wu(100), wu(100));
    // The info sheet's own worked example: dispersion 0.1 at
    // 10 tiles (40 wu) lands up to 0.5 tiles (2 wu) away per
    // axis: half-side = 6_554 × 2_621_440 / 131_072 = 131_080
    // bits ≈ 2 wu.
    let disp = I16F16::from_bits(6_554);
    let mut max_seen = 0;
    for _ in 0..500 {
        let (ix, iy) = sim.scattered_impact(disp, wu(40), aim);
        let dx = (ix - aim.0).to_bits().abs();
        let dy = (iy - aim.1).to_bits().abs();
        assert!(dx <= 131_080 && dy <= 131_080, "offset ({dx}, {dy}) outside the sheet's square");
        max_seen = max_seen.max(dx).max(dy);
    }
    assert!(max_seen > 100_000, "the draws should fill the square, max was {max_seen}");
    // The Archer's 0.33 at 4 tiles (16 wu): half-side =
    // 21_626 × 8 = 173_008 bits — WIDER than the old adopted
    // fixed half-side of 131_072 (0.5 tiles).
    let disp = I16F16::from_bits(21_626);
    let mut beyond_old = false;
    for _ in 0..500 {
        let (ix, iy) = sim.scattered_impact(disp, wu(16), aim);
        let dx = (ix - aim.0).to_bits().abs();
        let dy = (iy - aim.1).to_bits().abs();
        assert!(dx <= 173_008 && dy <= 173_008, "offset ({dx}, {dy}) outside the 0.33 square");
        beyond_old |= dx > 131_072 || dy > 131_072;
    }
    assert!(beyond_old, "at 4 tiles the documented square exceeds the old fixed radius");
    // At 2 tiles (8 wu) the same dispersion gives half-side
    // 86_504 bits — TIGHTER than the old fixed radius: the
    // square scales with the shot's distance.
    for _ in 0..500 {
        let (ix, iy) = sim.scattered_impact(disp, wu(8), aim);
        let dx = (ix - aim.0).to_bits().abs();
        let dy = (iy - aim.1).to_bits().abs();
        assert!(dx <= 86_504 && dy <= 86_504, "offset ({dx}, {dy}) outside the scaled square");
    }
    // Zero dispersion lands exactly on the aim.
    let (ix, iy) = sim.scattered_impact(I16F16::ZERO, wu(16), aim);
    assert_eq!((ix, iy), aim);
}

/// Shared behavioural pin for the two release paths that can
/// scatter today (unit → unit and unit → building): two shooters
/// (or one, against a building) at exactly 4 tiles, HP boosted
/// so the exchange runs long; every missed shot must land in
/// the 0.33 × distance square (half-side 173_008 bits), hits
/// land exactly on the aim, and at least one miss must land
/// beyond the old fixed 0.5-tile half-side.
fn assert_scatter_behaviour(unit_target: bool) {
    let mut sim = Sim::new(7);
    let house = if unit_target {
        None
    } else {
        let h = placed(&mut sim, BuildingKind::House, OWNER_OPPONENT, (40, 40));
        let bi = sim.buildings.iter().position(|b| b.id == h).unwrap();
        sim.buildings[bi].hp = I16F16::from_num(30_000);
        Some(h)
    };
    let (ax, ay, aim) = match house {
        Some(h) => {
            let (bx, by) = building_pos(&sim, h);
            (bx - wu(16), by, (bx, by))
        }
        None => (wu(40), wu(40), (wu(56), wu(40))),
    };
    let a = spawn(&mut sim, EntityKind::Archer, OWNER_PLAYER, ax, ay);
    let b = if unit_target {
        let id = spawn(&mut sim, EntityKind::Archer, OWNER_OPPONENT, aim.0, aim.1);
        let ei = sim.entity_index(id).unwrap();
        sim.entities[ei].hp = I16F16::from_num(30_000);
        Some(id)
    } else {
        None
    };
    let ei = sim.entity_index(a).unwrap();
    sim.entities[ei].hp = I16F16::from_num(30_000);
    match (b, house) {
        (Some(t), _) => {
            sim.push_command(Command::Attack { entity: a, target: t });
            sim.push_command(Command::Attack { entity: t, target: a });
        }
        (_, Some(h)) => sim.push_command(Command::AttackBuilding { entity: a, building: h }),
        _ => unreachable!(),
    }
    let mut seen = 0usize;
    let mut misses = 0usize;
    let mut beyond_old = false;
    let mut inspected: std::collections::HashSet<(u64, u32, i32, i32)> =
        std::collections::HashSet::new();
    for _ in 0..6000 {
        sim.tick();
        // Inspect every in-flight projectile exactly once
        // (keyed by spawn tick, attacker and impact point).
        for p in sim.projectiles() {
            let key = (p.spawn_tick, p.attacker, p.impact_x.to_bits(), p.impact_y.to_bits());
            if !inspected.insert(key) {
                continue;
            }
            seen += 1;
            // For the unit target the opponent's shots aim at
            // the OTHER archer; skip those here (their aim is
            // the first archer's position, not `aim`).
            if unit_target && p.owner == OWNER_OPPONENT {
                continue;
            }
            let dx = (p.impact_x - aim.0).to_bits().abs();
            let dy = (p.impact_y - aim.1).to_bits().abs();
            assert!(
                dx <= 173_008 && dy <= 173_008,
                "impact offset ({dx}, {dy}) outside the dispersion square"
            );
            if dx != 0 || dy != 0 {
                misses += 1;
                beyond_old |= dx > 131_072 || dy > 131_072;
            }
        }
    }
    assert!(seen >= 20, "expected a long exchange, saw {seen} shots");
    assert!(misses >= 3, "expected several misses, saw {misses}");
    assert!(beyond_old, "no miss landed beyond the old fixed half-side");
}

#[test]
fn prj04_misses_scatter_in_the_dispersion_square_unit_path() {
    assert_scatter_behaviour(true);
}

#[test]
fn prj04_misses_scatter_in_the_dispersion_square_building_path() {
    assert_scatter_behaviour(false);
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
