//! Castle P3c — the University and its research set: Ballistics
//! (PRJ-05), Murder Holes (BLD-18's remainder), Treadmill Crane
//! (TEC-09's construction half), Masonry, Fortified Wall
//! (BLD-20's upgrade half + the foundation melee-armor-0 half).
//! Heated Shot is DEFERRED by the scope — no test, no button.
//!
//! Fixture style follows `castle_p3_tests`: buildings and units
//! are placed directly (the placement/queue gates have their own
//! pins below), tech bits are set or researched through the real
//! command path where the flow itself is the subject.

use crate::build::{Building, BuildingKind};
use crate::classes;
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::{Age, ResearchBlock, TechId, TechHost};
use crate::{player_slot, Command, Sim};
use fixed::types::I16F16;

fn sim_in_castle() -> Sim {
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Castle;
    sim.players[1].age = Age::Castle;
    sim
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
    });
    id
}

fn spawn(sim: &mut Sim, kind: EntityKind, owner: u8, x: I16F16, y: I16F16) -> u32 {
    let id = 7000 + sim.entities.len() as u32;
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
    id
}

fn wu(n: i32) -> I16F16 {
    I16F16::from_num(n)
}

fn run_until(sim: &mut Sim, ticks: u64) {
    for _ in 0..ticks {
        sim.tick();
    }
}

fn grant(sim: &mut Sim, owner: u8, tech: TechId) {
    let slot = player_slot(owner).expect("owner slot");
    sim.players[slot].techs |= tech.bit();
}

fn building_hp(sim: &Sim, id: u32) -> i32 {
    sim.buildings
        .iter()
        .find(|b| b.id == id)
        .expect("building stands")
        .hp
        .to_num::<i32>()
}

// ----------------------------------------------------------------
// The building row and the research hosts.

#[test]
fn university_row_matches_the_dossier() {
    let k = BuildingKind::University;
    assert_eq!(k.tag(), 16);
    assert_eq!(BuildingKind::from_tag(16), Some(k));
    assert_eq!(k.wood_cost().to_num::<i32>(), 200);
    assert_eq!(k.build_ticks(), 3600, "60 s at 60 Hz");
    assert_eq!(k.footprint_size(), 4);
    assert_eq!(k.max_hp_at(Age::Castle), 2100);
    assert_eq!(k.armor_melee_at(Age::Castle), 2);
    assert_eq!(k.armor_pierce_at(Age::Castle), 9);
    assert_eq!(k.sight_tiles_at(Age::Castle), 6);
    assert_eq!(k.age_required(), Age::Castle);
    assert!(k.counts_toward_imperial());
    assert!(!k.is_production());
}

#[test]
fn university_research_gates_and_flow() {
    let mut sim = sim_in_castle();
    // The sandbox stockpile is the real opening one — top up so
    // the gates below read Ok rather than NoFunds.
    sim.stockpiles[0].add(ResourceKind::Wood, I16F16::from_num(5000));
    sim.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(5000));
    sim.stockpiles[0].add(ResourceKind::Food, I16F16::from_num(5000));
    sim.stockpiles[0].add(ResourceKind::Stone, I16F16::from_num(5000));
    let uni = placed(&mut sim, BuildingKind::University, OWNER_PLAYER, (20, 20));
    let rax = placed(&mut sim, BuildingKind::Barracks, OWNER_PLAYER, (30, 20));
    // Host law: the five techs live at the University only.
    for t in [
        TechId::Ballistics,
        TechId::MurderHoles,
        TechId::TreadmillCrane,
        TechId::Masonry,
        TechId::FortifiedWall,
    ] {
        assert_eq!(t.host(), TechHost::Building(BuildingKind::University));
        assert_eq!(
            sim.research_block_at(rax, t),
            ResearchBlock::Locked,
            "foreign host refuses {:?}",
            t
        );
        assert_eq!(sim.research_block_at(uni, t), ResearchBlock::Ok, "{:?} open", t);
    }
    // Age law: Feudal owners are Locked everywhere.
    sim.players[0].age = Age::Feudal;
    assert_eq!(
        sim.research_block_at(uni, TechId::Ballistics),
        ResearchBlock::Locked
    );
    sim.players[0].age = Age::Castle;
    // The real flow: enqueue Ballistics, pay exactly 300 W /
    // 175 G, and the bit lands after 3600 ticks of research.
    let amt = |sim: &Sim, k: ResourceKind| {
        sim.stockpile_of(OWNER_PLAYER).amount(k).to_num::<i32>()
    };
    let wood_before = amt(&sim, ResourceKind::Wood);
    let gold_before = amt(&sim, ResourceKind::Gold);
    sim.push_command(Command::ResearchAt {
        building: uni,
        tech: TechId::Ballistics,
    });
    // Commands land on the next tick: the charge happens then.
    run_until(&mut sim, 1);
    assert_eq!(amt(&sim, ResourceKind::Wood), wood_before - 300);
    assert_eq!(amt(&sim, ResourceKind::Gold), gold_before - 175);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::Ballistics));
    // The 3600-tick research time is pinned exactly in the
    // tech table test; here the flow: not done well before,
    // done shortly after.
    run_until(&mut sim, 3400);
    assert!(!sim.has_tech(OWNER_PLAYER, TechId::Ballistics));
    run_until(&mut sim, 300);
    assert!(sim.has_tech(OWNER_PLAYER, TechId::Ballistics));
    assert_eq!(
        sim.research_block_at(uni, TechId::Ballistics),
        ResearchBlock::AlreadyDone
    );
}

// ----------------------------------------------------------------
// PRJ-05 — Ballistics lead.

#[test]
fn ballistics_aim_math_is_exact() {
    // Shooter at origin; target 32 wu east (8 tiles), moving
    // east at Scout Cavalry speed (6772 bits/tick); arrow speed
    // 0.5 wu/tick (32768 bits) → flight 64 ticks → lead
    // 6772 × 64 = 433408 bits = 6.615 wu.
    let (ax, ay) = Sim::ballistics_aim(
        wu(0),
        wu(0),
        wu(32),
        wu(0),
        I16F16::from_bits(6772),
        wu(0),
        I16F16::from_bits(32768),
    );
    assert_eq!(ax.to_bits(), 32 * 65536 + 433408);
    assert_eq!(ay.to_bits(), 0);
    // A stationary target is never led.
    let (ax, _) = Sim::ballistics_aim(
        wu(0),
        wu(0),
        wu(32),
        wu(0),
        wu(0),
        wu(0),
        I16F16::from_bits(32768),
    );
    assert_eq!(ax, wu(32));
}

/// One tower, one scout crossing east through its band. Returns
/// the first projectile's impact point and the scout's position
/// on the tick the projectile appeared.
fn building_pos(sim: &Sim, id: u32) -> (I16F16, I16F16) {
    let b = sim
        .buildings
        .iter()
        .find(|b| b.id == id)
        .expect("building stands");
    (b.pos_x, b.pos_y)
}

fn tower_shot(ballistics: bool) -> (I16F16, I16F16, I16F16, I16F16, I16F16) {
    let mut sim = sim_in_castle();
    if ballistics {
        grant(&mut sim, OWNER_PLAYER, TechId::Ballistics);
    }
    let tw = placed(&mut sim, BuildingKind::WatchTower, OWNER_PLAYER, (40, 40));
    let (bx, by) = building_pos(&sim, tw);
    // The scout crosses 3.5 tiles north of the tower's center,
    // moving east.
    let scout = spawn(
        &mut sim,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        bx - wu(12),
        by - wu(14),
    );
    sim.push_command(Command::Move {
        entity: scout,
        x: bx + wu(38),
        y: by - wu(14),
    });
    for _ in 0..1200 {
        sim.tick();
        if let Some(p) = sim.projectiles().first() {
            let si = sim.entity_index(scout).expect("scout alive");
            let e = &sim.entities[si];
            return (p.impact_x, p.impact_y, e.pos_x, e.pos_y, by - wu(14));
        }
    }
    panic!("the tower never released");
}

#[test]
fn ballistics_leads_a_moving_target_towers() {
    // Without Ballistics the aim is the scout's release-time
    // position: impact == its position on the spawn tick (the
    // tower's accuracy is 100 — no scatter).
    let (ix, iy, sx, _sy, lane) = tower_shot(false);
    assert!(
        (ix - sx).abs() < I16F16::from_num(1) / I16F16::from_num(2),
        "no-Ballistics impact {} should sit on the scout at {}",
        ix,
        sx
    );
    assert!(
        (iy - lane).abs() < I16F16::from_num(3) / I16F16::from_num(2),
        "impact stays on the scout's lane ({} vs {})",
        iy,
        lane
    );
    // With Ballistics the impact leads the scout east by the
    // flight-time lead — several world units at this range.
    let (ix, iy, sx, sy, _lane) = tower_shot(true);
    let lead = ix - sx;
    assert!(
        lead > wu(2),
        "Ballistics lead {} should exceed 2 wu (scout at {})",
        lead,
        sx
    );
    // The led point tracks the scout's own position plus its
    // velocity over the flight: on this lane the scout drifts
    // slightly north as it runs (its measured velocity has a
    // small northward component, and the lead extrapolates it
    // faithfully), so the height pin is a loose 3 wu — the
    // strict pin is the eastward lead above.
    assert!(
        (iy - sy).abs() < wu(3),
        "led impact {} should track the scout's height {}",
        iy,
        sy
    );
}

// ----------------------------------------------------------------
// BLD-18 remainder — Murder Holes.

#[test]
fn murder_holes_removes_defensive_min_range() {
    let mut sim = sim_in_castle();
    for kind in [BuildingKind::WatchTower, BuildingKind::Castle] {
        let w = sim.building_weapon(OWNER_PLAYER, kind).expect("armed");
        assert_eq!(w.range_min, wu(4), "{:?} keeps its band", kind);
    }
    grant(&mut sim, OWNER_PLAYER, TechId::MurderHoles);
    for kind in [BuildingKind::WatchTower, BuildingKind::Castle] {
        let w = sim.building_weapon(OWNER_PLAYER, kind).expect("armed");
        assert_eq!(w.range_min, wu(0), "{:?} band removed", kind);
    }
}

#[test]
fn murder_holes_tower_shoots_a_hugging_attacker() {
    // A militia stands half a tile from the tower's edge — deep
    // inside the 1-tile minimum-range band.
    let scenario = |murder_holes: bool| -> i32 {
        let mut sim = sim_in_castle();
        if murder_holes {
            grant(&mut sim, OWNER_PLAYER, TechId::MurderHoles);
        }
        let tw = placed(&mut sim, BuildingKind::WatchTower, OWNER_PLAYER, (40, 40));
        let (bx, by) = building_pos(&sim, tw);
        // Half a tile south of the tower's edge, centered in x:
        // edge distance 2 wu, deep inside the 1-tile band.
        let v = spawn(
            &mut sim,
            EntityKind::Militia,
            OWNER_OPPONENT,
            bx,
            by + wu(4),
        );
        run_until(&mut sim, 300);
        let vi = sim.entity_index(v).expect("militia placed");
        sim.entities[vi].hp.to_num::<i32>()
    };
    assert_eq!(scenario(false), 40, "inside the band: never shot");
    assert!(
        scenario(true) < 40,
        "Murder Holes: the tower shoots the hugging militia"
    );
}

// ----------------------------------------------------------------
// TEC-09 construction half — Treadmill Crane.

/// Build a House with one villager, wait for construction to
/// start, then measure the thirds accrued over exactly 100
/// ticks.
fn construction_delta(treadmill: bool) -> u64 {
    let mut sim = sim_in_castle();
    if treadmill {
        grant(&mut sim, OWNER_PLAYER, TechId::TreadmillCrane);
    }
    let (cx, cy) = sim.grid().cell_center(21, 21);
    let v = spawn(&mut sim, EntityKind::Villager, OWNER_PLAYER, cx, cy);
    let (tx, ty) = sim.grid().cell_center(22, 20);
    sim.push_command(Command::Build {
        entity: v,
        kind: BuildingKind::House,
        x: tx,
        y: ty,
    });
    // Wait for the builder to arrive and start accruing.
    let mut started = None;
    for _ in 0..3000 {
        sim.tick();
        if let Some(b) = sim.buildings.iter().find(|b| b.kind == BuildingKind::House) {
            if b.progress_thirds > 0 {
                started = Some(b.progress_thirds);
                break;
            }
        }
    }
    let before = started.expect("construction started");
    run_until(&mut sim, 100);
    let b = sim
        .buildings
        .iter()
        .find(|b| b.kind == BuildingKind::House)
        .expect("house stands");
    b.progress_thirds - before
}

#[test]
fn treadmill_crane_builds_twenty_percent_faster() {
    // One builder accrues 3 thirds/tick: 300 over 100 ticks.
    assert_eq!(construction_delta(false), 300);
    // Treadmill: ×6/5 exactly — 360 over the same 100 ticks.
    assert_eq!(construction_delta(true), 360);
}

#[test]
fn treadmill_crane_never_speeds_repair() {
    let scenario = |treadmill: bool| -> i32 {
        let mut sim = sim_in_castle();
        if treadmill {
            grant(&mut sim, OWNER_PLAYER, TechId::TreadmillCrane);
        }
        let house = placed(&mut sim, BuildingKind::House, OWNER_PLAYER, (20, 20));
        let bi = sim
            .buildings
            .iter()
            .position(|b| b.id == house)
            .expect("house");
        sim.buildings[bi].hp = wu(100); // of 550 — heavily damaged
        let (hx, hy) = building_pos(&sim, house);
        let v = spawn(&mut sim, EntityKind::Villager, OWNER_PLAYER, hx, hy + wu(8));
        sim.push_command(Command::Repair {
            entity: v,
            building: house,
        });
        run_until(&mut sim, 600);
        sim.buildings[bi].hp.to_num::<i32>()
    };
    let plain = scenario(false);
    let crane = scenario(true);
    assert!(plain > 100, "the repair actually ran ({})", plain);
    assert_eq!(plain, crane, "repair rate is Treadmill-free (BLD-07)");
}

// ----------------------------------------------------------------
// Masonry.

#[test]
fn masonry_patches_building_stats() {
    let mut sim = sim_in_castle();
    // Bare rows (Castle Age columns).
    assert_eq!(
        sim.building_max_hp(BuildingKind::Barracks, OWNER_PLAYER),
        1800
    );
    assert_eq!(
        sim.building_armor(BuildingKind::Barracks, OWNER_PLAYER),
        (2, 9, 0)
    );
    grant(&mut sim, OWNER_PLAYER, TechId::Masonry);
    // +10% HP, +1/+1 armor, +3 building armor.
    assert_eq!(
        sim.building_max_hp(BuildingKind::Barracks, OWNER_PLAYER),
        1980
    );
    assert_eq!(
        sim.building_armor(BuildingKind::Barracks, OWNER_PLAYER),
        (3, 10, 3)
    );
    // The University itself is a building like any other here.
    assert_eq!(
        sim.building_max_hp(BuildingKind::University, OWNER_PLAYER),
        2310
    );
    // Exclusions: walls and farms never take Masonry.
    assert_eq!(
        sim.building_max_hp(BuildingKind::StoneWall, OWNER_PLAYER),
        1800
    );
    assert_eq!(
        sim.building_armor(BuildingKind::StoneWall, OWNER_PLAYER),
        (8, 10, 0)
    );
    assert_eq!(sim.building_max_hp(BuildingKind::Farm, OWNER_PLAYER), 480);
    // The Town Center entity takes the patch in its spec.
    let tc = sim.stats_for(OWNER_PLAYER, EntityKind::TownCenter);
    assert_eq!(tc.max_hp, 2640);
    assert_eq!(
        (tc.armor_melee, tc.armor_pierce, tc.armor_building),
        (6, 8, 3)
    );
}

#[test]
fn masonry_scales_standing_hp_and_petard_damage() {
    let mut sim = sim_in_castle();
    let rax = placed(&mut sim, BuildingKind::Barracks, OWNER_PLAYER, (20, 20));
    let bi = sim.buildings.iter().position(|b| b.id == rax).expect("rax");
    sim.buildings[bi].hp = wu(900); // exactly half of 1800
    sim.apply_research(OWNER_PLAYER, TechId::Masonry);
    assert_eq!(building_hp(&sim, rax), 990, "half of the new 1980 maximum");
    // A Petard-class strike now meets the +3 building armor:
    // melee (25 − 3) + building (500 − 3) = 519 through the real
    // damage path.
    let before = building_hp(&sim, rax);
    let (rx, ry) = building_pos(&sim, rax);
    let elev = sim.height_at_world(rx, ry);
    sim.damage_building(
        rax,
        &[(classes::MELEE, 25), (classes::STANDARD_BUILDINGS, 500)],
        elev,
        OWNER_OPPONENT,
    );
    assert_eq!(before - building_hp(&sim, rax), 519);
}

// ----------------------------------------------------------------
// BLD-20 — Fortified Wall + the foundation rule.

#[test]
fn fortified_wall_converts_standing_walls() {
    let mut sim = sim_in_castle();
    let wall = placed(&mut sim, BuildingKind::StoneWall, OWNER_PLAYER, (20, 20));
    let bi = sim.buildings.iter().position(|b| b.id == wall).expect("wall");
    sim.buildings[bi].hp = wu(900); // half of the standing 1800
    // Bare row first.
    assert_eq!(
        sim.building_max_hp(BuildingKind::StoneWall, OWNER_PLAYER),
        1800
    );
    sim.apply_research(OWNER_PLAYER, TechId::FortifiedWall);
    assert_eq!(
        sim.building_max_hp(BuildingKind::StoneWall, OWNER_PLAYER),
        3000
    );
    assert_eq!(
        sim.building_armor(BuildingKind::StoneWall, OWNER_PLAYER),
        (12, 12, 8)
    );
    assert_eq!(building_hp(&sim, wall), 1500, "half of the fortified 3000");
    // Palisade is untouched by the upgrade.
    assert_eq!(
        sim.building_max_hp(BuildingKind::PalisadeWall, OWNER_PLAYER),
        250
    );
}

#[test]
fn wall_foundations_have_zero_melee_armor() {
    let mut sim = sim_in_castle();
    let wall = placed(&mut sim, BuildingKind::StoneWall, OWNER_PLAYER, (20, 20));
    let bi = sim.buildings.iter().position(|b| b.id == wall).expect("wall");
    // Militia-class melee 4: through the real damage path.
    let hit = |sim: &mut Sim, id: u32, idx: usize| -> i32 {
        let before = sim.buildings[idx].hp.to_num::<i32>();
        let elev = sim.height_at_world(sim.buildings[idx].pos_x, sim.buildings[idx].pos_y);
        sim.damage_building(id, &[(classes::MELEE, 4)], elev, OWNER_OPPONENT);
        before - sim.buildings[idx].hp.to_num::<i32>()
    };
    assert_eq!(hit(&mut sim, wall, bi), 1, "standing wall: 4 − 8 clamps to 1");
    sim.buildings[bi].complete = false;
    assert_eq!(
        hit(&mut sim, wall, bi),
        4,
        "foundation: melee armor 0 (42848)"
    );
    // The palisade foundation follows the same rule.
    let pal = placed(&mut sim, BuildingKind::PalisadeWall, OWNER_PLAYER, (24, 20));
    let pi = sim.buildings.iter().position(|b| b.id == pal).expect("pal");
    sim.buildings[pi].complete = false;
    assert_eq!(hit(&mut sim, pal, pi), 4);
}

