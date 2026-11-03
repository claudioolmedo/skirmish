//! Castle P4c tests (2026-11-03): the Scorpion (pass-through
//! weapon, PRJ-09), the Ram armor class its +1 row keys on,
//! and the scripted AI's scorpion line.
//!
//! Pass-through pins live here rather than `sim/tests/combat.rs`
//! (following the P4a/P4b split: slice tests beside the code,
//! the standing suites keep the cross-cutting pins). Fixture
//! style follows `siege_p4b_tests` (direct records; ages set
//! on the player slots). Line scenarios stage victims by world
//! coordinates on the bolt's north line: staged units never
//! share a sim across scenarios, and victims stand outside
//! their own line-of-sight of the shooter (a Militia's LoS is
//! 4 tiles) so nobody closes or retaliates mid-measurement.

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

fn spawn_at(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, x: I16F16, y: I16F16) {
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
        gate_along_x: false,
        queue_rate_remainder: 0,
    }
}

fn sim_in_castle() -> Sim {
    let mut sim = Sim::new(7);
    sim.players[0].age = Age::Castle;
    sim.players[1].age = Age::Castle;
    sim
}

/// A Scorpion of `owner` staged at cell (60, 66); its bolt
/// scenarios play out on the north line from its position.
const SCORP_ID: u32 = 990;
const SCORP_CELL: (u32, u32) = (60, 66);

fn staged_scorpion(sim: &mut Sim, owner: u8) -> (I16F16, I16F16) {
    spawn(sim, SCORP_ID, EntityKind::Scorpion, owner, SCORP_CELL);
    let s = ent(sim, SCORP_ID);
    (s.pos_x, s.pos_y)
}

// ------------------------------------------------------------------
// The Scorpion's data row, the Ram class, and training.

#[test]
fn p4c_scorpion_row() {
    let s = crate::units::spec(EntityKind::Scorpion);
    assert_eq!(EntityKind::Scorpion.tag(), 19);
    assert_eq!(EntityKind::from_tag(19), Some(EntityKind::Scorpion));
    assert_eq!(s.max_hp, 40);
    assert_eq!(
        s.attacks,
        &[
            (cls::PIERCE, 11),
            (cls::MELEE, 0),
            (cls::ELEPHANT, 7),
            (cls::STANDARD_BUILDINGS, 3),
            (cls::RAM, 1),
            (cls::INFANTRY, 1),
        ]
    );
    assert_eq!(s.armor_melee, 0);
    assert_eq!(s.armor_pierce, 7);
    assert_eq!(s.classes, cls::bit(cls::SIEGE));
    assert_eq!(s.range_max.to_bits(), 1_835_008); // 7 tiles
    assert_eq!(s.range_min.to_bits(), 524_288); // 2 tiles (PRJ-08)
    assert_eq!(s.reload_ticks, 216); // 3.6 s
    assert_eq!(s.attack_delay_ticks, 13); // 0.21 s
    assert_eq!(s.accuracy, 100);
    assert_eq!(s.projectile_speed.to_bits(), 32_768); // 0.5 — design constant
    // PRJ-09's pass-through data: no blast at all, the flag instead.
    assert_eq!(s.blast_radius, I16F16::ZERO);
    assert!(s.pass_through);
    assert_eq!(s.blast_secondaries, 0);
    assert_eq!(s.speed.to_bits(), 2_840); // 0.65 tiles/s
    assert_eq!(s.los.to_bits(), 2_359_296); // 9 tiles
    assert_eq!(s.radius.to_bits(), 78_643); // 1.2 — adopted constant
    assert_eq!(s.food_cost, 0);
    assert_eq!(s.wood_cost, 75);
    assert_eq!(s.gold_cost, 75);
    assert_eq!(s.train_ticks, 1800); // 30 s
    assert_eq!(EntityKind::Scorpion.train_age(), Age::Castle);
    assert!(EntityKind::Scorpion.counts_population());
    assert!(EntityKind::Scorpion.is_trainable());
    // Siege never garrisons (GAR-01), like the Ram and Mangonel.
    assert!(!EntityKind::Scorpion.is_foot_garrisonable());
    assert!(!EntityKind::Scorpion.is_mounted_garrisonable());
    // The roster: the Siege Workshop trains it, nothing else does.
    assert!(BuildingKind::SiegeWorkshop.trains(EntityKind::Scorpion));
    assert!(!BuildingKind::Barracks.trains(EntityKind::Scorpion));
    assert!(!BuildingKind::Castle.trains(EntityKind::Scorpion));
    // Passo 0.3: the Battering Ram carries the distinct RAM
    // class beside SIEGE — the membership the +1 row keys on.
    let ram = crate::units::spec(EntityKind::BatteringRam);
    assert_eq!(ram.classes, cls::mask(&[cls::SIEGE, cls::RAM]));
}

#[test]
fn p4c_scorpion_damage_table_pins() {
    let scorp = crate::units::spec(EntityKind::Scorpion);
    // Vs the Battering Ram: pierce 11 is eaten by its 180
    // pierce armor; the (MELEE, 0) pair lands 0 − (−3) = 3
    // through the ram's negative melee armor; the RAM bonus
    // adds its 1 — 4 damage a bolt, the soft anti-ram bite.
    let ram = crate::units::spec(EntityKind::BatteringRam);
    assert_eq!(crate::combat::compute_damage(scorp.attacks, &ram), 4);
    // Vs a Knight: 11 pierce − 2 armor = 9 (full application).
    let knight = crate::units::spec(EntityKind::Knight);
    assert_eq!(crate::combat::compute_damage(scorp.attacks, &knight), 9);
    // Vs a Militia: 11 pierce − 1 armor + 1 vs Infantry = 11
    // (full application; the line tests pin its half at 5.5).
    let militia = crate::units::spec(EntityKind::Militia);
    assert_eq!(crate::combat::compute_damage(scorp.attacks, &militia), 11);
}

#[test]
fn p4c_scorpion_trains_at_the_workshop_only() {
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
    assert!(sim.can_train_at(900, EntityKind::Scorpion));
    assert!(!sim.can_train_at(901, EntityKind::Scorpion));
    sim.push_command(Command::TrainAt {
        building: 900,
        kind: EntityKind::Scorpion,
    });
    let trained = run_until(&mut sim, 20_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Scorpion && e.owner == OWNER_PLAYER)
    });
    assert!(trained, "the workshop never produced a Scorpion");
}

// ------------------------------------------------------------------
// Pass-through resolution (PRJ-09): the line, the half rule,
// friendly immunity, travel past max range, buildings.

#[test]
fn p4c_line_primary_full_secondary_half() {
    let mut sim = Sim::new(7);
    let (sx, sy) = staged_scorpion(&mut sim, OWNER_PLAYER);
    // The primary: an enemy Militia 6 tiles north (in band).
    spawn_at(
        &mut sim,
        991,
        EntityKind::Militia,
        OWNER_OPPONENT,
        sx,
        sy - I16F16::from_num(24),
    );
    // A second enemy Militia directly behind it, 7 tiles out.
    spawn_at(
        &mut sim,
        992,
        EntityKind::Militia,
        OWNER_OPPONENT,
        sx,
        sy - I16F16::from_num(28),
    );
    sim.push_command(Command::Attack {
        entity: SCORP_ID,
        target: 991,
    });
    let landed = run_until(&mut sim, 2_000, |s| ent(s, 991).hp < I16F16::from_num(40));
    assert!(landed, "the bolt never reached the primary");
    // Full application on the primary, half on the secondary —
    // both read from the shipped tables' own arithmetic (the
    // damage-table test pins that arithmetic literally: 11
    // full, 5.5 half against a Militia).
    let full = crate::combat::compute_damage(
        crate::units::spec(EntityKind::Scorpion).attacks,
        &crate::units::spec(EntityKind::Militia),
    );
    assert_eq!(f(ent(&sim, 991).hp), 40.0 - full as f64);
    assert_eq!(f(ent(&sim, 992).hp), 40.0 - full as f64 / 2.0);
}

#[test]
fn p4c_line_friendly_untouched_and_does_not_stop_the_bolt() {
    let mut sim = Sim::new(7);
    let (sx, sy) = staged_scorpion(&mut sim, OWNER_PLAYER);
    // The shooter's own Knight stands on the line, 3 tiles out.
    spawn_at(
        &mut sim,
        993,
        EntityKind::Knight,
        OWNER_PLAYER,
        sx,
        sy - I16F16::from_num(12),
    );
    // An enemy Militia behind the Knight (non-primary)…
    spawn_at(
        &mut sim,
        992,
        EntityKind::Militia,
        OWNER_OPPONENT,
        sx,
        sy - I16F16::from_num(18),
    );
    // …and the primary, 6 tiles out.
    spawn_at(
        &mut sim,
        991,
        EntityKind::Militia,
        OWNER_OPPONENT,
        sx,
        sy - I16F16::from_num(24),
    );
    sim.push_command(Command::Attack {
        entity: SCORP_ID,
        target: 991,
    });
    let landed = run_until(&mut sim, 2_000, |s| ent(s, 991).hp < I16F16::from_num(40));
    assert!(landed, "the bolt never reached the primary");
    // CMB-12's exclusion, the anti-blast: the friendly Knight is
    // entirely untouched — no damage, and the bolt flew through
    // him to both enemies behind.
    assert_eq!(f(ent(&sim, 993).hp), 100.0);
    let full = crate::combat::compute_damage(
        crate::units::spec(EntityKind::Scorpion).attacks,
        &crate::units::spec(EntityKind::Militia),
    );
    assert_eq!(f(ent(&sim, 991).hp), 40.0 - full as f64);
    assert_eq!(f(ent(&sim, 992).hp), 40.0 - full as f64 / 2.0);
}

#[test]
fn p4c_bolt_travels_past_max_range_up_to_three_tiles() {
    let mut sim = Sim::new(7);
    let (sx, sy) = staged_scorpion(&mut sim, OWNER_PLAYER);
    // The primary at max range's inner band: 6 tiles.
    spawn_at(
        &mut sim,
        991,
        EntityKind::Militia,
        OWNER_OPPONENT,
        sx,
        sy - I16F16::from_num(24),
    );
    // A second enemy 9 tiles out — 2 tiles PAST the 7-tile max
    // range, inside the bolt's 3-tile overtravel: hit at half.
    spawn_at(
        &mut sim,
        992,
        EntityKind::Militia,
        OWNER_OPPONENT,
        sx,
        sy - I16F16::from_num(36),
    );
    // A third enemy 10.5 tiles out — 3.5 tiles past max range,
    // beyond the bolt's travel (segment ends at 10 tiles; the
    // gap to its circle is 2 world units > radius + bolt size):
    // never touched.
    spawn_at(
        &mut sim,
        994,
        EntityKind::Militia,
        OWNER_OPPONENT,
        sx,
        sy - I16F16::from_num(42),
    );
    sim.push_command(Command::Attack {
        entity: SCORP_ID,
        target: 991,
    });
    let landed = run_until(&mut sim, 2_000, |s| ent(s, 991).hp < I16F16::from_num(40));
    assert!(landed, "the bolt never reached the primary");
    let full = crate::combat::compute_damage(
        crate::units::spec(EntityKind::Scorpion).attacks,
        &crate::units::spec(EntityKind::Militia),
    );
    assert_eq!(f(ent(&sim, 992).hp), 40.0 - full as f64 / 2.0);
    assert_eq!(f(ent(&sim, 994).hp), 40.0);
}

#[test]
fn p4c_bolt_against_a_building_is_an_ordinary_building_shot() {
    let mut sim = Sim::new(7);
    staged_scorpion(&mut sim, OWNER_PLAYER);
    // An enemy Barracks 6 tiles north; an enemy Militia stands
    // directly behind it on the bolt's line.
    sim.buildings.push(placed(
        &sim,
        900,
        BuildingKind::Barracks,
        OWNER_OPPONENT,
        (60, 60),
    ));
    let (sx, sy) = {
        let s = ent(&sim, SCORP_ID);
        (s.pos_x, s.pos_y)
    };
    spawn_at(
        &mut sim,
        991,
        EntityKind::Militia,
        OWNER_OPPONENT,
        sx,
        sy - I16F16::from_num(30),
    );
    sim.push_command(Command::AttackBuilding {
        entity: SCORP_ID,
        building: 900,
    });
    let hit = run_until(&mut sim, 2_000, |s| {
        s.buildings().iter().find(|b| b.id == 900).unwrap().hp
            < I16F16::from_num(BuildingKind::Barracks.max_hp())
    });
    assert!(hit, "the bolt never reached the Barracks");
    // The building took the ordinary building application of
    // the Scorpion's table (the +3 vs Standard Buildings pair
    // included), computed from the shipped tables.
    let expected = crate::combat::compute_damage(
        crate::units::spec(EntityKind::Scorpion).attacks,
        &crate::combat::building_spec(BuildingKind::Barracks, Age::Dark),
    );
    let b = sim.buildings().iter().find(|b| b.id == 900).unwrap();
    assert_eq!(
        f(b.hp),
        BuildingKind::Barracks.max_hp() as f64 - expected as f64
    );
    // Buildings are not pierced: the Militia behind the
    // Barracks on the same line is untouched.
    assert_eq!(f(ent(&sim, 991).hp), 40.0);
}

#[test]
fn p4c_min_range_target_is_not_engaged_point_blank() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, SCORP_ID, EntityKind::Scorpion, OWNER_PLAYER, SCORP_CELL);
    // An enemy Knight one tile north — inside the 2-tile dead
    // zone (PRJ-08).
    spawn(&mut sim, 991, EntityKind::Knight, OWNER_OPPONENT, (60, 65));
    sim.push_command(Command::Attack {
        entity: SCORP_ID,
        target: 991,
    });
    // While the gap is inside the dead zone, no bolt flies:
    // the Scorpion retreats instead (its gap grows from 4 world
    // units toward the band; the first band shot cannot land
    // before the gap reaches 8 — measured dynamics put that
    // past tick ~100, so 60 ticks is safely inside the zone).
    for _ in 0..60 {
        sim.tick();
    }
    assert_eq!(
        f(ent(&sim, 991).hp),
        100.0,
        "the Scorpion fired from inside its dead zone"
    );
    let gap_now = |s: &Sim| {
        let (mx, my) = (ent(s, SCORP_ID).pos_x, ent(s, SCORP_ID).pos_y);
        let (kx, ky) = (ent(s, 991).pos_x, ent(s, 991).pos_y);
        f(crate::steer::vec_len(mx - kx, my - ky))
    };
    assert!(gap_now(&sim) > 4.5, "the Scorpion never backed away");
    // The retreat carries the gap out of the dead zone…
    let out = run_until(&mut sim, 6_000, |s| gap_now(s) >= 8.0);
    assert!(out, "the Scorpion never left its dead zone");
    assert_eq!(
        f(ent(&sim, 991).hp),
        100.0,
        "the Knight was hit before the gap left the dead zone"
    );
    // …and from the band, the bolts land (9 a hit vs a Knight).
    let hit = run_until(&mut sim, 6_000, |s| ent(s, 991).hp < I16F16::from_num(100));
    assert!(hit, "the repositioned Scorpion never fired");
}

#[test]
fn p4c_pass_through_streams_replay_bit_identically() {
    // Two identical setups with the same command stream hash
    // identically at every checkpoint: pass-through consumes
    // no RNG draws (PRJ-03) — the line resolution is a pure
    // function of who stood on the line.
    let setup = || {
        let mut sim = Sim::new(11);
        let (sx, sy) = staged_scorpion(&mut sim, OWNER_PLAYER);
        spawn_at(
            &mut sim,
            991,
            EntityKind::Militia,
            OWNER_OPPONENT,
            sx,
            sy - I16F16::from_num(24),
        );
        spawn_at(
            &mut sim,
            992,
            EntityKind::Militia,
            OWNER_OPPONENT,
            sx,
            sy - I16F16::from_num(28),
        );
        sim.push_command(Command::Attack {
            entity: SCORP_ID,
            target: 991,
        });
        sim
    };
    let (mut a, mut b) = (setup(), setup());
    for step in 0..15 {
        for _ in 0..100 {
            a.tick();
            b.tick();
        }
        assert_eq!(
            a.state_hash(),
            b.state_hash(),
            "pass-through streams diverged at checkpoint {step}"
        );
    }
    // And the line really resolved in both.
    assert!(ent(&a, 991).hp < I16F16::from_num(40));
    assert!(ent(&a, 992).hp < I16F16::from_num(40));
}

// ------------------------------------------------------------------
// The AI slice: the opponent fields Scorpions (plan §3.7).

#[test]
fn p4c_ai_trains_scorpions_and_sends_them_with_the_wave() {
    let mut sim = Sim::new_match(7, MatchMode::AiNormal);
    sim.players[1].age = Age::Castle;
    sim.stockpile_mut(OWNER_OPPONENT)
        .add(ResourceKind::Wood, I16F16::from_num(8000));
    sim.stockpile_mut(OWNER_OPPONENT)
        .add(ResourceKind::Food, I16F16::from_num(5000));
    sim.stockpile_mut(OWNER_OPPONENT)
        .add(ResourceKind::Gold, I16F16::from_num(8000));
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
    // Behind the Ram and Mangonel lines, the workshop produces
    // a Scorpion through the AI's own TrainAt path (no AI-only
    // orders) — the cap is generous because the queue serves
    // the whole roster ahead of it first.
    let trained = run_until(&mut sim, 480_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Scorpion && e.owner == OWNER_OPPONENT)
    });
    assert!(trained, "the AI's Siege Workshop never produced a Scorpion");

    // Waves: staged with a full wave beside an explored player
    // House (the P4a/P4b staging), the Scorpion is sent at a
    // target through the ordinary paths — the shipped target
    // order: a visible unit, the Town Center once explored, or
    // the nearest explored building. Any of the three proves
    // the wave claimed it: the Scorpion is not in
    // `acquire_targets`, so an attack order on it can only
    // have come from the wave.
    let scorp = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Scorpion && e.owner == OWNER_OPPONENT)
        .unwrap()
        .id;
    let (mx, my) = {
        let m = ent(&sim, scorp);
        sim.grid().world_to_cell(m.pos_x, m.pos_y)
    };
    for i in 0..5u32 {
        spawn(
            &mut sim,
            950 + i,
            EntityKind::Militia,
            OWNER_OPPONENT,
            (mx + 2 + i, my + 3),
        );
    }
    sim.buildings.push(placed(
        &sim,
        980,
        BuildingKind::House,
        OWNER_PLAYER,
        (mx + 4, my),
    ));
    sim.ai.next_wave_tick = 0;
    let sent = run_until(&mut sim, 30_000, |s| {
        let m = ent(s, scorp);
        m.attack_target.is_some() || m.attack_building == Some(980)
    });
    assert!(sent, "the wave never gave the Scorpion a target");
}

