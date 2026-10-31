//! Spearman + Skirmisher tests: training routes and age gates,
//! the documented bonus tables measured in-sim, the Fletching /
//! armor tech patches on the new kinds, and PRJ-08's minimum
//! range (dead zone, repositioning, building stand-off).
//!
//! In-crate staging (direct entity/building records) follows the
//! lib.rs fixture style; combat expectations are literal table
//! values from the unit rows, not recomputed ones.

use crate::build::{Building, BuildingKind};
use crate::combat::{building_spec, compute_damage};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::grid::TERRAIN_LAND;
use crate::tech::{Age, TechId};
use crate::units::spec;
use crate::{Command, Sim};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn dist(sim: &Sim, a: u32, b: u32) -> f64 {
    let (x, y) = (ent(sim, a), ent(sim, b));
    f(crate::steer::vec_len(x.pos_x - y.pos_x, x.pos_y - y.pos_y))
}

/// Two horizontally adjacent land cells at the same height level
/// near the map center — flat ground keeps the elevation factor
/// at exactly ×1, so measured strike damage equals the table.
fn flat_pair(sim: &Sim) -> ((u32, u32), (u32, u32)) {
    for cy in 50..70u32 {
        for cx in 50..70u32 {
            let g = sim.grid();
            if g.terrain_at(cx, cy) == TERRAIN_LAND
                && g.terrain_at(cx + 1, cy) == TERRAIN_LAND
                && g.height_at(cx, cy) == g.height_at(cx + 1, cy)
            {
                return ((cx, cy), (cx + 1, cy));
            }
        }
    }
    panic!("no flat adjacent land pair near the map center");
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

/// A complete, idle production building record at a cell anchor
/// (the placed_house fixture's shape, completed).
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
        progress_thirds: 3,
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

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

// ------------------------------------------------------------------
// Training routes: building, age gate, exact cost.

#[test]
fn spearman_trains_at_the_barracks_only_from_the_feudal_age() {
    let mut sim = Sim::new(7);
    let rax = placed(&sim, 900, BuildingKind::Barracks, OWNER_PLAYER, (58, 58));
    sim.buildings.push(rax);
    let range = placed(&sim, 901, BuildingKind::ArcheryRange, OWNER_PLAYER, (64, 58));
    sim.buildings.push(range);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));

    // Dark Age: the Barracks stands but its Spearman is a Feudal
    // unit — refused, while the Militia on the same panel trains.
    assert!(!sim.can_train_at(900, EntityKind::Spearman));
    assert!(sim.can_train_at(900, EntityKind::Militia));
    // Wrong building at any age.
    assert!(!sim.can_train_at(901, EntityKind::Spearman));
    assert!(!sim.can_train_at(900, EntityKind::Skirmisher));

    sim.players[0].age = Age::Feudal;
    assert!(sim.can_train_at(900, EntityKind::Spearman));
    let (f0, w0) = (
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food),
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
    );
    sim.push_command(Command::TrainAt {
        building: 900,
        kind: EntityKind::Spearman,
    });
    let trained = run_until(&mut sim, 5_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Spearman && e.owner == OWNER_PLAYER)
    });
    assert!(trained, "spearman never trained");
    // Exact DE cost: 35 food / 25 wood, no gold.
    assert_eq!(
        f0 - sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food),
        I16F16::from_num(35)
    );
    assert_eq!(
        w0 - sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        I16F16::from_num(25)
    );
}

#[test]
fn skirmisher_trains_at_the_archery_range_for_25_food_35_wood() {
    let mut sim = Sim::new(7);
    let range = placed(&sim, 900, BuildingKind::ArcheryRange, OWNER_PLAYER, (58, 58));
    sim.buildings.push(range);
    sim.players[0].age = Age::Feudal;
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    assert!(sim.can_train_at(900, EntityKind::Skirmisher));
    assert!(sim.can_train_at(900, EntityKind::Archer));
    let (f0, w0) = (
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food),
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
    );
    sim.push_command(Command::TrainAt {
        building: 900,
        kind: EntityKind::Skirmisher,
    });
    let trained = run_until(&mut sim, 5_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Skirmisher && e.owner == OWNER_PLAYER)
    });
    assert!(trained, "skirmisher never trained");
    assert_eq!(
        f0 - sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food),
        I16F16::from_num(25)
    );
    assert_eq!(
        w0 - sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        I16F16::from_num(35)
    );
}

// ------------------------------------------------------------------
// The bonus tables: pure values, then measured in-sim.

#[test]
fn bonus_tables_are_the_documented_values() {
    let spearman = spec(EntityKind::Spearman);
    let skirmisher = spec(EntityKind::Skirmisher);
    // Spearman vs Scout Cavalry: 3 melee + 15 vs Cavalry.
    assert_eq!(
        compute_damage(spearman.attacks, spec(EntityKind::ScoutCavalry)),
        18
    );
    // No membership, no bonus: vs Militia and vs Archer only the
    // base 3 lands (both carry 0 melee armor).
    assert_eq!(compute_damage(spearman.attacks, spec(EntityKind::Militia)), 3);
    assert_eq!(compute_damage(spearman.attacks, spec(EntityKind::Archer)), 3);
    // The +1 vs Standard Buildings row participates against the
    // building view: the total is the melee part (priced against
    // the building's melee armor) plus exactly the bonus 1.
    let house = building_spec(BuildingKind::House, Age::Dark);
    assert_eq!(
        compute_damage(spearman.attacks, &house),
        (3 - house.armor_melee).max(0) + 1
    );
    // Skirmisher vs Archer: 2 pierce + 3 vs Archers.
    assert_eq!(compute_damage(skirmisher.attacks, spec(EntityKind::Archer)), 5);
    // Skirmisher vs Spearman: 2 pierce + 3 vs Spearmen.
    assert_eq!(
        compute_damage(skirmisher.attacks, spec(EntityKind::Spearman)),
        5
    );
    // Skirmisher vs Militia: no bonus, 2 − 1 pierce armor = 1.
    assert_eq!(compute_damage(skirmisher.attacks, spec(EntityKind::Militia)), 1);
    // The counter-duel: an Archer shooting a Skirmisher gets
    // 4 − 3 pierce armor = 1 — the Skirmisher's high pierce
    // armor is its second identity half. (The Archer's +3 row
    // targets the Spearman class, which the Skirmisher lacks.)
    assert_eq!(
        compute_damage(spec(EntityKind::Archer).attacks, spec(EntityKind::Skirmisher)),
        1
    );
}

/// The first landed strike of an ordered duel, as an HP delta.
fn first_strike_delta(
    attacker: EntityKind,
    victim: EntityKind,
) -> f64 {
    let mut sim = Sim::new(7);
    let (a, b) = flat_pair(&sim);
    spawn(&mut sim, 9001, attacker, OWNER_PLAYER, a);
    spawn(&mut sim, 9002, victim, OWNER_OPPONENT, b);
    let hp0 = f(ent(&sim, 9002).hp);
    sim.issue_attack(9001, 9002);
    let mut delta = 0.0;
    run_until(&mut sim, 3_000, |s| {
        let d = hp0 - f(ent(s, 9002).hp);
        if d > 0.0 {
            delta = d;
            true
        } else {
            false
        }
    });
    assert!(delta > 0.0, "{attacker:?} never landed a strike on {victim:?}");
    delta
}

#[test]
fn spearman_bonus_is_measured_in_sim_not_just_tabulated() {
    // 3 + 15 vs the Cavalry class, exactly, on flat ground.
    assert_eq!(
        first_strike_delta(EntityKind::Spearman, EntityKind::ScoutCavalry),
        18.0
    );
    // The same spear against a Militia: the base 3 only.
    assert_eq!(
        first_strike_delta(EntityKind::Spearman, EntityKind::Militia),
        3.0
    );
}

#[test]
fn skirmisher_wins_the_archer_duel_it_exists_to_win() {
    // Both shoot on the same reload; the Skirmisher deals 5 a
    // hit to the Archer's 30 HP while taking 1 a hit back — the
    // Archer falls with the Skirmisher barely scratched.
    let mut sim = Sim::new(7);
    let (a, b) = flat_pair(&sim);
    spawn(&mut sim, 9001, EntityKind::Skirmisher, OWNER_PLAYER, a);
    spawn(&mut sim, 9002, EntityKind::Archer, OWNER_OPPONENT, b);
    // Start inside comfortable range of each other (4 tiles).
    sim.issue_attack(9001, 9002);
    let done = run_until(&mut sim, 6_000, |s| ent(s, 9002).dead);
    assert!(done, "the archer survived the skirmisher duel");
    assert!(!ent(&sim, 9001).dead);
    assert!(
        f(ent(&sim, 9001).hp) >= 20.0,
        "skirmisher took {} damage in its own counter-duel",
        30.0 - f(ent(&sim, 9001).hp)
    );
}

// ------------------------------------------------------------------
// PRJ-08: minimum range.

#[test]
fn skirmisher_never_fires_from_inside_its_dead_zone_and_backs_out() {
    let mut sim = Sim::new(7);
    let (a, b) = flat_pair(&sim);
    spawn(&mut sim, 9001, EntityKind::Skirmisher, OWNER_PLAYER, a);
    spawn(&mut sim, 9002, EntityKind::Militia, OWNER_OPPONENT, b);
    // Teleport the militia to half a tile away — deep inside the
    // 1-tile dead zone (positions are plain state in a fixture).
    {
        let (sx, sy) = (ent(&sim, 9001).pos_x, ent(&sim, 9001).pos_y);
        let m = sim.entities.iter_mut().find(|e| e.id == 9002).unwrap();
        m.pos_x = sx + I16F16::from_num(2);
        m.pos_y = sy;
    }
    assert!(dist(&sim, 9001, 9002) < 4.0, "staging must start in the dead zone");
    sim.issue_attack(9001, 9002);

    // A projectile is measured only on the tick it spawns (first
    // sighting): positions then are exactly the release-time
    // positions, since combat runs after movement in a tick.
    let mut seen: std::collections::HashSet<(u64, i32, i32)> = std::collections::HashSet::new();
    let mut min_spawn_dist = f64::MAX;
    let mut spawns = 0u32;
    let mut backed_out = false;
    for _ in 0..4_000 {
        sim.tick();
        if dist(&sim, 9001, 9002) >= 4.0 {
            backed_out = true;
        }
        for p in sim.projectiles() {
            if p.attacker == 9001
                && seen.insert((p.spawn_tick, p.spawn_x.to_bits(), p.spawn_y.to_bits()))
            {
                spawns += 1;
                let d = f(crate::steer::vec_len(
                    p.spawn_x - ent(&sim, 9002).pos_x,
                    p.spawn_y - ent(&sim, 9002).pos_y,
                ));
                min_spawn_dist = min_spawn_dist.min(d);
            }
        }
        if ent(&sim, 9002).dead {
            break;
        }
    }
    assert!(backed_out, "the skirmisher never left its dead zone");
    assert!(spawns > 0, "the skirmisher never fired at all");
    assert!(
        min_spawn_dist >= 3.9,
        "a shot was released from {min_spawn_dist} world units — inside the dead zone"
    );
}

#[test]
fn skirmisher_stands_off_from_buildings_and_still_damages_them() {
    let mut sim = Sim::new(7);
    // An opponent House at the map center; the skirmisher starts
    // one tile from its south edge — inside the dead zone
    // measured from the footprint's edge.
    let house = placed(&sim, 900, BuildingKind::House, OWNER_OPPONENT, (60, 60));
    let (hx, hy) = (house.pos_x, house.pos_y);
    sim.buildings.push(house);
    // Cell (60, 62) sits 2 world units south of the footprint's
    // edge — inside the dead zone measured from the wall.
    let (sx, sy) = sim.grid().cell_center(60, 62);
    sim.entities.push(Entity::spawned(
        9001,
        EntityKind::Skirmisher,
        OWNER_PLAYER,
        sx,
        sy,
    ));
    let hp0 = f(sim.buildings()[0].hp);
    sim.issue_attack_building(9001, 900);
    let mut min_edge = f64::MAX;
    for _ in 0..6_000 {
        sim.tick();
        let e = ent(&sim, 9001);
        let qx = (e.pos_x - hx).clamp(I16F16::from_num(-4), I16F16::from_num(4)) + hx;
        let qy = (e.pos_y - hy).clamp(I16F16::from_num(-4), I16F16::from_num(4)) + hy;
        let edge = f(crate::steer::vec_len(e.pos_x - qx, e.pos_y - qy));
        if sim.tick_count() > 3_000 {
            min_edge = min_edge.min(edge);
        }
    }
    let dealt = hp0 - f(sim.buildings()[0].hp);
    assert!(dealt >= 5.0, "the house took only {dealt} damage");
    assert!(
        min_edge >= 3.9,
        "the skirmisher settled {min_edge} from the wall — inside its dead zone"
    );
}

// ------------------------------------------------------------------
// Tech patches cover the new kinds (TEC family).

#[test]
fn fletching_and_armor_patches_cover_spearman_and_skirmisher() {
    let mut sim = Sim::new(7);
    // Fletching: the Skirmisher's table swaps and its range grows
    // a tile, exactly like the Archer's.
    sim.players[0].techs |= TechId::Fletching.bit();
    let s = sim.stats_for(OWNER_PLAYER, EntityKind::Skirmisher);
    assert_eq!(
        s.attacks,
        crate::units::SKIRMISHER_FLETCHING_ATTACKS,
    );
    assert_eq!(
        s.range_max,
        spec(EntityKind::Skirmisher).range_max + I16F16::from_bits(262_144)
    );
    // Its dead zone does NOT grow — minimum range is not part of
    // the tech in DE.
    assert_eq!(s.range_min, spec(EntityKind::Skirmisher).range_min);
    // Scale Mail: the Spearman is infantry.
    sim.players[0].techs |= TechId::ScaleMail.bit();
    let s = sim.stats_for(OWNER_PLAYER, EntityKind::Spearman);
    assert_eq!((s.armor_melee, s.armor_pierce), (1, 1));
    // Padded Archer Armor: the Skirmisher is a foot archer —
    // 0/3 becomes 1/4.
    sim.players[0].techs |= TechId::PaddedArcherArmor.bit();
    let s = sim.stats_for(OWNER_PLAYER, EntityKind::Skirmisher);
    assert_eq!((s.armor_melee, s.armor_pierce), (1, 4));
    // And an unpatched opponent keeps the base rows (patches are
    // per-player state, never global).
    let s = sim.stats_for(OWNER_OPPONENT, EntityKind::Skirmisher);
    assert_eq!((s.armor_melee, s.armor_pierce), (0, 3));
}

