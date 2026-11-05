//! Castle P4b tests (2026-10-31): the Mangonel (blast weapon,
//! PRJ-07), Attack Ground (CMB-16), tree felling by blast
//! (MAP-04), and the scripted AI's mangonel line.
//!
//! Blast pins live here rather than `sim/tests/combat.rs`
//! (following the P4a split: slice tests beside the code, the
//! standing suites keep the cross-cutting pins). Fixture style
//! follows `siege_p4a_tests` (direct records; ages set on the
//! player slots). One victim per blast scenario: separation
//! between staged units would move them off their pinned
//! distances, so taper measurements never share a sim.

use crate::build::{Building, BuildingKind};
use crate::classes as cls;
use crate::econ::{NodeKind, ResourceKind};
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::Age;
use crate::{Command, MatchMode, QueuedOrder, Sim};
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
        repair_gold_remainder: 0,
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
        relics: Vec::new(),
        relic_gold_remainder: 0,
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

/// A Mangonel of `owner` staged at cell (60, 66) — six tiles
/// south of the (60, 60) impact cell, inside its [3, 7] band,
/// so it fires from where it stands.
const MANGO_ID: u32 = 990;
const IMPACT_CELL: (u32, u32) = (60, 60);
const MANGO_CELL: (u32, u32) = (60, 66);

fn staged_mangonel(sim: &mut Sim, owner: u8) -> (I16F16, I16F16) {
    spawn(sim, MANGO_ID, EntityKind::Mangonel, owner, MANGO_CELL);
    sim.grid().cell_center(IMPACT_CELL.0, IMPACT_CELL.1)
}

// ------------------------------------------------------------------
// The Mangonel's data row and training.

#[test]
fn p4b_mangonel_row() {
    let s = crate::units::spec(EntityKind::Mangonel);
    assert_eq!(EntityKind::Mangonel.tag(), 18);
    assert_eq!(EntityKind::from_tag(18), Some(EntityKind::Mangonel));
    assert_eq!(s.max_hp, 50);
    assert_eq!(
        s.attacks,
        &[
            (cls::MELEE, 40),
            (cls::STANDARD_BUILDINGS, 35),
            (cls::SIEGE, 12)
        ]
    );
    assert_eq!(s.armor_melee, 0);
    assert_eq!(s.armor_pierce, 6);
    assert_eq!(s.classes, cls::bit(cls::SIEGE));
    assert_eq!(s.range_max.to_bits(), 1_835_008); // 7 tiles
    assert_eq!(s.range_min.to_bits(), 786_432); // 3 tiles (PRJ-08)
    assert_eq!(s.reload_ticks, 360); // 6.0 s
    assert_eq!(s.attack_delay_ticks, 0);
    assert_eq!(s.accuracy, 100);
    assert_eq!(s.projectile_speed.to_bits(), 15_291); // 3.5 tiles/s
    // PRJ-07's blast data: 1-tile radius, 5 secondary stones.
    assert_eq!(s.blast_radius.to_bits(), 262_144); // 1 tile
    assert_eq!(s.blast_secondaries, 5);
    assert!(!s.pass_through); // PRJ-09 is the Scorpion's (P4c)
    assert_eq!(s.speed.to_bits(), 2_621); // 0.6 tiles/s
    assert_eq!(s.los.to_bits(), 2_359_296); // 9 tiles
    assert_eq!(s.food_cost, 0);
    assert_eq!(s.wood_cost, 160);
    assert_eq!(s.gold_cost, 135);
    assert_eq!(s.train_ticks, 2760); // 46 s
    assert_eq!(EntityKind::Mangonel.train_age(), Age::Castle);
    assert!(EntityKind::Mangonel.counts_population());
    assert!(EntityKind::Mangonel.is_trainable());
    // Siege never garrisons (GAR-01), like the Ram.
    assert!(!EntityKind::Mangonel.is_foot_garrisonable());
    assert!(!EntityKind::Mangonel.is_mounted_garrisonable());
    // The roster: the Siege Workshop trains it, nothing else does.
    assert!(BuildingKind::SiegeWorkshop.trains(EntityKind::Mangonel));
    assert!(!BuildingKind::Barracks.trains(EntityKind::Mangonel));
    assert!(!BuildingKind::Castle.trains(EntityKind::Mangonel));
}

#[test]
fn p4b_mangonel_trains_at_the_workshop_only() {
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
    assert!(sim.can_train_at(900, EntityKind::Mangonel));
    assert!(!sim.can_train_at(901, EntityKind::Mangonel));
    sim.push_command(Command::TrainAt {
        building: 900,
        kind: EntityKind::Mangonel,
    });
    let trained = run_until(&mut sim, 20_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Mangonel && e.owner == OWNER_PLAYER)
    });
    assert!(trained, "the workshop never produced a Mangonel");
}

// ------------------------------------------------------------------
// Blast resolution (PRJ-07): taper, friendly fire, buildings.

/// One blast scenario: a player Mangonel in band, one victim at
/// the given offset (world units) from the impact point; the
/// victim's HP after the first stone lands.
fn blast_victim_hp(owner: u8, offset_x: I16F16) -> I16F16 {
    let mut sim = Sim::new(7);
    let (px, py) = staged_mangonel(&mut sim, OWNER_PLAYER);
    spawn_at(&mut sim, 991, EntityKind::Knight, owner, px + offset_x, py);
    sim.push_command(Command::AttackGround {
        entity: MANGO_ID,
        x: px,
        y: py,
    });
    let landed = run_until(&mut sim, 2_000, |s| ent(s, 991).hp < I16F16::from_num(100));
    assert!(landed, "the stone never landed on the victim");
    ent(&sim, 991).hp
}

#[test]
fn p4b_blast_center_takes_full_damage() {
    // Knight at the impact center: factor 1 — the full table
    // (40 melee − 2 armor = 38) plus 5 secondary minimum damage.
    let hp = blast_victim_hp(OWNER_OPPONENT, I16F16::ZERO);
    assert_eq!(f(hp), 100.0 - 38.0 - 5.0);
}

#[test]
fn p4b_blast_tapers_with_distance() {
    // Knight at half the radius (0.5 tiles = 2 world units):
    // factor 1 − 2/4 = 0.5 → 38 × 0.5 = 19, plus the 5 flat
    // secondary damage (the extra stones do not taper).
    let hp = blast_victim_hp(OWNER_OPPONENT, I16F16::from_num(2));
    assert_eq!(f(hp), 100.0 - 19.0 - 5.0);
}

#[test]
fn p4b_blast_friendly_fire() {
    // CMB-12: the shooter's OWN knight at the center takes the
    // same full blast — blast-flagged weapons hurt everyone.
    let hp = blast_victim_hp(OWNER_PLAYER, I16F16::ZERO);
    assert_eq!(f(hp), 100.0 - 38.0 - 5.0);
}

#[test]
fn p4b_blast_spares_units_outside_the_radius() {
    let mut sim = Sim::new(7);
    let (px, py) = staged_mangonel(&mut sim, OWNER_PLAYER);
    // 1.5 tiles east of the impact point: outside the 1-tile radius.
    spawn_at(
        &mut sim,
        991,
        EntityKind::Knight,
        OWNER_OPPONENT,
        px + I16F16::from_num(6),
        py,
    );
    sim.push_command(Command::AttackGround {
        entity: MANGO_ID,
        x: px,
        y: py,
    });
    // Two full reload cycles: the bystander is never touched —
    // no table damage, no secondaries.
    for _ in 0..1_000 {
        sim.tick();
    }
    assert_eq!(f(ent(&sim, 991).hp), 100.0);
}

#[test]
fn p4b_blast_building_takes_the_building_pair() {
    let mut sim = Sim::new(7);
    // An enemy Barracks (Dark row: 0 melee armor) whose center
    // is the impact point — its footprint edge is at distance 0,
    // so the taper factor is 1 and the primary-building rule
    // also applies to an AttackBuilding shot.
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::Barracks, OWNER_OPPONENT, IMPACT_CELL));
    let (bx, by) = {
        let b = &sim.buildings()[0];
        (b.pos_x, b.pos_y)
    };
    spawn(&mut sim, MANGO_ID, EntityKind::Mangonel, OWNER_PLAYER, MANGO_CELL);
    sim.push_command(Command::AttackBuilding {
        entity: MANGO_ID,
        building: 900,
    });
    let _ = (bx, by);
    let hit = run_until(&mut sim, 2_000, |s| {
        s.buildings().iter().find(|b| b.id == 900).unwrap().hp < I16F16::from_num(1200)
    });
    assert!(hit, "the stone never reached the Barracks");
    let b = sim.buildings().iter().find(|b| b.id == 900).unwrap();
    // 40 melee (vs 0 melee armor) + 35 vs Buildings (vs 0
    // building armor) + 5 secondaries = 80.
    assert_eq!(f(b.hp), 1200.0 - 80.0);
}

#[test]
fn p4b_blast_fells_trees_and_opens_the_cell() {
    let mut sim = Sim::new(7);
    // A live tree somewhere in the map's middle band.
    let (ni, cx, cy) = sim
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == NodeKind::Tree && !n.is_depleted())
        .map(|(i, n)| {
            let (cx, cy) = sim.grid().world_to_cell(n.pos_x, n.pos_y);
            (i, cx, cy)
        })
        .find(|(_, cx, cy)| (30..90).contains(cx) && (30..90).contains(cy))
        .expect("the standard layout has a mid-map tree");
    let (tx, ty) = (sim.nodes()[ni].pos_x, sim.nodes()[ni].pos_y);
    assert!(sim.grid().is_node_blocked(cx, cy));
    // Five tiles east: inside the band, no pathing needed.
    spawn_at(
        &mut sim,
        MANGO_ID,
        EntityKind::Mangonel,
        OWNER_PLAYER,
        tx + I16F16::from_num(20),
        ty,
    );
    let wood_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood);
    sim.push_command(Command::AttackGround {
        entity: MANGO_ID,
        x: tx,
        y: ty,
    });
    let felled = run_until(&mut sim, 2_000, |s| s.nodes()[ni].is_depleted());
    assert!(felled, "the blast never felled the tree (MAP-04)");
    assert!(
        !sim.grid().is_node_blocked(cx, cy),
        "the felled tree's cell stayed blocked"
    );
    // Felled, not harvested: no wood reaches any stockpile.
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        wood_before
    );
}

// ------------------------------------------------------------------
// Attack Ground (CMB-16): standing bombardment, the band, queues.

#[test]
fn p4b_attack_ground_on_bare_ground_is_a_standing_bombardment() {
    let mut sim = Sim::new(7);
    let (px, py) = staged_mangonel(&mut sim, OWNER_PLAYER);
    assert!(sim.can_attack_ground(MANGO_ID));
    sim.push_command(Command::AttackGround {
        entity: MANGO_ID,
        x: px,
        y: py,
    });
    sim.tick();
    assert_eq!(ent(&sim, MANGO_ID).attack_point, Some((px, py)));
    // First stone flies; after it lands, a second follows one
    // reload later — the bombardment stands until replaced.
    let mut stones = 0;
    let mut in_flight = false;
    for _ in 0..1_600 {
        sim.tick();
        let flying = !sim.projectiles().is_empty();
        if flying && !in_flight {
            stones += 1;
        }
        in_flight = flying;
    }
    assert!(stones >= 2, "only {stones} stone(s) flew in 1600 ticks");
    assert_eq!(ent(&sim, MANGO_ID).attack_point, Some((px, py)));
}

#[test]
fn p4b_attack_ground_rejected_for_non_blast_weapons() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, 991, EntityKind::Archer, OWNER_PLAYER, (60, 66));
    assert!(!sim.can_attack_ground(991));
    let (px, py) = sim.grid().cell_center(60, 60);
    sim.push_command(Command::AttackGround {
        entity: 991,
        x: px,
        y: py,
    });
    sim.tick();
    assert_eq!(ent(&sim, 991).attack_point, None);
}

#[test]
fn p4b_both_seats_can_attack_ground() {
    // The opponent's Mangonel validates and bombards identically
    // (the lockstep surface is seat-blind but for ownership).
    let mut sim = Sim::new(7);
    let (px, py) = staged_mangonel(&mut sim, OWNER_OPPONENT);
    assert!(sim.can_attack_ground(MANGO_ID));
    sim.push_command(Command::AttackGround {
        entity: MANGO_ID,
        x: px,
        y: py,
    });
    let flying = run_until(&mut sim, 500, |s| !s.projectiles().is_empty());
    assert!(flying, "the opponent's Mangonel never released a stone");
}

#[test]
fn p4b_min_range_target_is_not_engaged_point_blank() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, MANGO_ID, EntityKind::Mangonel, OWNER_PLAYER, MANGO_CELL);
    // An enemy Knight two tiles north — inside the 3-tile dead
    // zone (PRJ-08).
    spawn(&mut sim, 991, EntityKind::Knight, OWNER_OPPONENT, (60, 64));
    sim.push_command(Command::Attack {
        entity: MANGO_ID,
        target: 991,
    });
    for _ in 0..150 {
        sim.tick();
    }
    assert_eq!(
        f(ent(&sim, 991).hp),
        100.0,
        "the Mangonel fired from inside its dead zone"
    );
    // It repositions instead: the gap grows past the dead zone…
    let (mx, my) = (ent(&sim, MANGO_ID).pos_x, ent(&sim, MANGO_ID).pos_y);
    let (kx, ky) = (ent(&sim, 991).pos_x, ent(&sim, 991).pos_y);
    let gap = f((mx - kx).abs() + (my - ky).abs());
    assert!(gap > 8.0, "the Mangonel never backed out of the dead zone");
    // …and from the band, the stones land.
    let hit = run_until(&mut sim, 6_000, |s| ent(s, 991).hp < I16F16::from_num(100));
    assert!(hit, "the repositioned Mangonel never fired");
}

#[test]
fn p4b_queued_attack_ground_executes_after_the_current_order() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, MANGO_ID, EntityKind::Mangonel, OWNER_PLAYER, MANGO_CELL);
    let (px, py) = sim.grid().cell_center(IMPACT_CELL.0, IMPACT_CELL.1);
    let (fx, fy) = sim.grid().cell_center(60, 80);
    sim.push_command(Command::Move {
        entity: MANGO_ID,
        x: fx,
        y: fy,
    });
    sim.push_command(Command::Queue {
        entity: MANGO_ID,
        order: QueuedOrder::AttackGround { x: px, y: py },
    });
    sim.tick();
    assert_eq!(ent(&sim, MANGO_ID).attack_point, None);
    // The move completes; the queued bombardment takes over.
    let took = run_until(&mut sim, 30_000, |s| {
        ent(s, MANGO_ID).attack_point == Some((px, py))
    });
    assert!(took, "the queued Attack Ground never executed (SEL-12)");
}

#[test]
fn p4b_non_queued_replacement_clears_a_queued_attack_ground() {
    let mut sim = Sim::new(7);
    spawn(&mut sim, MANGO_ID, EntityKind::Mangonel, OWNER_PLAYER, MANGO_CELL);
    spawn(&mut sim, 991, EntityKind::Knight, OWNER_OPPONENT, (60, 63));
    let (px, py) = sim.grid().cell_center(IMPACT_CELL.0, IMPACT_CELL.1);
    let (fx, fy) = sim.grid().cell_center(60, 80);
    sim.push_command(Command::Move {
        entity: MANGO_ID,
        x: fx,
        y: fy,
    });
    sim.push_command(Command::Queue {
        entity: MANGO_ID,
        order: QueuedOrder::AttackGround { x: px, y: py },
    });
    // A fresh, non-queued order replaces everything (SEL-12).
    sim.push_command(Command::Attack {
        entity: MANGO_ID,
        target: 991,
    });
    sim.tick();
    let m = ent(&sim, MANGO_ID);
    assert!(m.order_queue.is_empty(), "the replacement left the queue standing");
    assert_eq!(m.attack_target, Some(991));
    assert_eq!(m.attack_point, None);
}

#[test]
fn p4b_blast_streams_replay_bit_identically() {
    // Two identical setups with the same command stream —
    // including per-unit blast rolls — hash identically at
    // every checkpoint (PRJ-07's draw discipline is a pure
    // function of who stood in the blast).
    let setup = || {
        let mut sim = Sim::new(11);
        let (px, py) = staged_mangonel(&mut sim, OWNER_PLAYER);
        spawn_at(&mut sim, 991, EntityKind::Knight, OWNER_OPPONENT, px, py);
        spawn_at(
            &mut sim,
            992,
            EntityKind::Knight,
            OWNER_PLAYER,
            px + I16F16::from_num(2),
            py,
        );
        sim.push_command(Command::AttackGround {
            entity: MANGO_ID,
            x: px,
            y: py,
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
            "blast streams diverged at checkpoint {step}"
        );
    }
    // And the blast really happened in both.
    assert!(ent(&a, 991).hp < I16F16::from_num(100));
    assert!(ent(&a, 992).hp < I16F16::from_num(100));
}

// ------------------------------------------------------------------
// The AI slice: the opponent fields Mangonels (plan §3.7).

#[test]
fn p4b_ai_trains_mangonels_and_sends_them_with_the_wave() {
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
    // Behind the Ram line, the workshop produces a Mangonel
    // through the AI's own TrainAt path (no AI-only orders).
    let trained = run_until(&mut sim, 240_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Mangonel && e.owner == OWNER_OPPONENT)
    });
    assert!(trained, "the AI's Siege Workshop never produced a Mangonel");

    // Waves: staged with a full wave beside an explored player
    // House (the P4a staging), the Mangonel is sent at a target
    // through the ordinary paths — the shipped target order:
    // a visible unit, the Town Center once explored, or the
    // nearest explored building. Any of the three proves the
    // wave claimed it: the Mangonel is not in `acquire_targets`,
    // so an attack order on it can only have come from the wave.
    // (By staging time the earlier waves may already have razed
    // the player's Town Center, so the TC is not required to
    // still stand — the staged House is the reliable target.)
    let mango = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Mangonel && e.owner == OWNER_OPPONENT)
        .unwrap()
        .id;
    let (mx, my) = {
        let m = ent(&sim, mango);
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
        let m = ent(s, mango);
        m.attack_target.is_some() || m.attack_building == Some(980)
    });
    assert!(sent, "the wave never gave the Mangonel a target");
}
