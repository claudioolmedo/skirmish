//! MOV-07 tests: formations assign slots by class layer.
//!
//! Two halves: the pure assignment function
//! (`formation::assign_slots` — layering per formation kind,
//! purity in id order, facing rotation, the Box's center-back
//! shelter, the Flank's wings) and the sim integration (a
//! FormationMove batch re-aims its members at their slots,
//! the group arrives and holds shape, queued formation moves
//! slot on execution, saves round-trip, plain moves are
//! untouched).
//!
//! Fixture style follows `group_speed_tests`: in-crate
//! staging on a `Sim::new(7)` map with a scanned open row as
//! the travel corridor.

use crate::entity::{Entity, EntityKind, OWNER_PLAYER};
use crate::formation::{assign_slots, layer_of, FormationKind, FormationMember};
use crate::{Command, QueuedOrder, Sim};
use fixed::types::I16F16;

fn fx(v: f64) -> I16F16 {
    I16F16::from_num(v)
}

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn member(id: u32, kind: EntityKind) -> FormationMember {
    FormationMember { id, kind }
}

/// Project a slot onto the facing basis: (depth along the
/// facing, lateral along its right), in world units, for an
/// EAST facing from the origin — the pure tests all face
/// east unless they say otherwise.
fn depth_lat(slot: (I16F16, I16F16)) -> (f64, f64) {
    (f(slot.0), f(slot.1))
}

fn slot_of(slots: &[(u32, (I16F16, I16F16))], id: u32) -> (I16F16, I16F16) {
    slots.iter().find(|(i, _)| *i == id).unwrap().1
}

// --- The layer table -------------------------------------------------

#[test]
fn layers_classify_by_the_unit_table() {
    // Melee front.
    assert_eq!(layer_of(EntityKind::Militia), 0);
    assert_eq!(layer_of(EntityKind::ManAtArms), 0);
    assert_eq!(layer_of(EntityKind::Spearman), 0);
    assert_eq!(layer_of(EntityKind::Knight), 0);
    assert_eq!(layer_of(EntityKind::ScoutCavalry), 0);
    assert_eq!(layer_of(EntityKind::LightCavalry), 0);
    assert_eq!(layer_of(EntityKind::Petard), 0);
    // Civilians shelter behind the front.
    assert_eq!(layer_of(EntityKind::Villager), 1);
    assert_eq!(layer_of(EntityKind::TradeCart), 1);
    assert_eq!(layer_of(EntityKind::King), 1);
    // Ranged behind the melee.
    assert_eq!(layer_of(EntityKind::Archer), 2);
    assert_eq!(layer_of(EntityKind::Skirmisher), 2);
    assert_eq!(layer_of(EntityKind::CavalryArcher), 2);
    // Siege and monks at the back.
    assert_eq!(layer_of(EntityKind::BatteringRam), 3);
    assert_eq!(layer_of(EntityKind::Mangonel), 3);
    assert_eq!(layer_of(EntityKind::Scorpion), 3);
    assert_eq!(layer_of(EntityKind::Monk), 3);
}

// --- Pure assignment -------------------------------------------------

#[test]
fn line_puts_melee_ahead_of_ranged_and_centers_on_the_destination() {
    let members = [
        member(12, EntityKind::Archer),
        member(10, EntityKind::Militia),
        member(13, EntityKind::Archer),
        member(11, EntityKind::Militia),
    ];
    let slots = assign_slots(&members, FormationKind::Line, (fx(0.0), fx(0.0)), (fx(1.0), fx(0.0)));
    let (m0, _) = depth_lat(slot_of(&slots, 10));
    let (m1, _) = depth_lat(slot_of(&slots, 11));
    let (a0, _) = depth_lat(slot_of(&slots, 12));
    let (a1, _) = depth_lat(slot_of(&slots, 13));
    assert!(m0 > a0 && m0 > a1 && m1 > a0 && m1 > a1, "melee must stand ahead of ranged: melee {m0}/{m1}, archers {a0}/{a1}");
    // Every slot distinct, and the block centered on the anchor.
    let mut pts: Vec<(i32, i32)> = slots.iter().map(|(_, s)| (s.0.to_bits(), s.1.to_bits())).collect();
    pts.sort();
    pts.dedup();
    assert_eq!(pts.len(), 4, "slots overlap: {slots:?}");
    let mean_x: f64 = slots.iter().map(|(_, s)| f(s.0)).sum::<f64>() / 4.0;
    let mean_y: f64 = slots.iter().map(|(_, s)| f(s.1)).sum::<f64>() / 4.0;
    assert!(mean_x.abs() <= 1.0 && mean_y.abs() <= 1.0, "formation not centered: ({mean_x}, {mean_y})");
}

#[test]
fn assignment_is_a_pure_function_of_the_composition_in_id_order() {
    let a = [
        member(10, EntityKind::Militia),
        member(11, EntityKind::Archer),
        member(12, EntityKind::Monk),
        member(13, EntityKind::Knight),
    ];
    let mut b = a;
    b.reverse();
    let dest = (fx(20.0), fx(-8.0));
    let facing = (fx(1.0), fx(0.0));
    let sa = assign_slots(&a, FormationKind::Box, dest, facing);
    let sb = assign_slots(&b, FormationKind::Box, dest, facing);
    for id in [10, 11, 12, 13] {
        assert_eq!(slot_of(&sa, id), slot_of(&sb, id), "input order moved id {id}'s slot");
    }
    let sa2 = assign_slots(&a, FormationKind::Box, dest, facing);
    assert_eq!(sa, sa2, "the same inputs must give the same slots");
}

#[test]
fn facing_rotates_the_formation() {
    let members = [member(10, EntityKind::Militia), member(11, EntityKind::Archer)];
    let east = assign_slots(&members, FormationKind::Line, (fx(0.0), fx(0.0)), (fx(1.0), fx(0.0)));
    let north = assign_slots(&members, FormationKind::Line, (fx(0.0), fx(0.0)), (fx(0.0), fx(-1.0)));
    // East: the melee stands at greater x. North (y-down
    // world: north is -y): the melee stands at smaller y.
    assert!(f(slot_of(&east, 10).0) > f(slot_of(&east, 11).0));
    assert!(f(slot_of(&north, 10).1) < f(slot_of(&north, 11).1));
    assert_ne!(slot_of(&east, 10), slot_of(&north, 10));
}

#[test]
fn box_shelters_the_monk_at_the_center_back() {
    let mut members: Vec<FormationMember> = (0..6)
        .map(|i| member(10 + i, EntityKind::Militia))
        .collect();
    members.push(member(20, EntityKind::Archer));
    members.push(member(21, EntityKind::Archer));
    members.push(member(22, EntityKind::Archer));
    members.push(member(30, EntityKind::Monk));
    members.push(member(31, EntityKind::Mangonel));
    members.push(member(32, EntityKind::Scorpion));
    let slots = assign_slots(&members, FormationKind::Box, (fx(0.0), fx(0.0)), (fx(1.0), fx(0.0)));
    let (monk_depth, monk_lat) = depth_lat(slot_of(&slots, 30));
    // The monk stands in the deepest row, with the melee ahead.
    for i in 0..6 {
        let (d, _) = depth_lat(slot_of(&slots, 10 + i));
        assert!(d > monk_depth, "militia {i} at depth {d} is not ahead of the monk at {monk_depth}");
    }
    // ...and centered: the monk's lateral offset is the
    // innermost of the back row's (the back row spans ±3).
    assert!(monk_lat.abs() <= 1.0, "monk not sheltered at the center: lat {monk_lat}");
    let (siege_depth, _) = depth_lat(slot_of(&slots, 31));
    assert_eq!(siege_depth, monk_depth, "siege shares the back row");
}

#[test]
fn staggered_offsets_the_back_rank_by_half_a_step() {
    let members = [
        member(10, EntityKind::Militia),
        member(11, EntityKind::Militia),
        member(12, EntityKind::Militia),
        member(13, EntityKind::Archer),
        member(14, EntityKind::Archer),
    ];
    let line = assign_slots(&members, FormationKind::Line, (fx(0.0), fx(0.0)), (fx(1.0), fx(0.0)));
    let stag = assign_slots(&members, FormationKind::Staggered, (fx(0.0), fx(0.0)), (fx(1.0), fx(0.0)));
    // Layering holds the same...
    assert!(f(slot_of(&stag, 10).0) > f(slot_of(&stag, 13).0));
    // ...but the back rank (the archers' row) sits half a
    // spacing (1.0 world unit) to the side of its Line slot.
    for id in [13, 14] {
        let d = f(slot_of(&stag, id).1) - f(slot_of(&line, id).1);
        assert!((d - 1.0).abs() < 0.01, "back-rank stagger is {d}, want 1.0");
    }
    // The front rank is unshifted.
    for id in [10, 11, 12] {
        assert_eq!(slot_of(&stag, id), slot_of(&line, id));
    }
}

#[test]
fn flank_splits_the_melee_into_wings_around_the_shooters() {
    let members = [
        member(10, EntityKind::Militia),
        member(11, EntityKind::Militia),
        member(12, EntityKind::Militia),
        member(13, EntityKind::Militia),
        member(20, EntityKind::Archer),
        member(21, EntityKind::Archer),
    ];
    let slots = assign_slots(&members, FormationKind::Flank, (fx(0.0), fx(0.0)), (fx(1.0), fx(0.0)));
    let mut wing_lats = Vec::new();
    for id in [10, 11, 12, 13] {
        let (depth, lat) = depth_lat(slot_of(&slots, id));
        wing_lats.push(lat);
        assert!(lat.abs() >= 3.0, "wing member {id} too close to the center: lat {lat}");
        // The wings stand ahead of the shooters.
        for a in [20, 21] {
            let (ad, _) = depth_lat(slot_of(&slots, a));
            assert!(depth > ad, "wing member {id} not ahead of archer {a}");
        }
    }
    assert!(wing_lats.iter().any(|l| *l < 0.0) && wing_lats.iter().any(|l| *l > 0.0),
        "the melee must split to BOTH wings: {wing_lats:?}");
    for a in [20, 21] {
        let (_, lat) = depth_lat(slot_of(&slots, a));
        assert!(lat.abs() <= 1.0, "archer {a} not held in the center: lat {lat}");
    }
}

#[test]
fn a_single_member_gets_the_destination_itself() {
    for kind in [FormationKind::Line, FormationKind::Box, FormationKind::Staggered, FormationKind::Flank] {
        let slots = assign_slots(
            &[member(7, EntityKind::Knight)],
            kind,
            (fx(12.0), fx(4.0)),
            (fx(1.0), fx(0.0)),
        );
        assert_eq!(slot_of(&slots, 7), (fx(12.0), fx(4.0)), "{kind:?} moved a lone unit");
    }
}

// --- Sim integration ---------------------------------------------------

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn free_row(sim: &Sim, len: u32) -> (u32, u32) {
    let g = sim.grid();
    for y in 8..112 {
        let mut run = 0;
        let mut h0 = 0;
        for x in 8..112 {
            if g.is_blocked(x, y) {
                run = 0;
                continue;
            }
            if run == 0 {
                h0 = g.height_at(x, y);
            }
            if g.height_at(x, y) != h0 {
                run = 0;
                continue;
            }
            run += 1;
            if run >= len {
                return (y, x + 1 - len);
            }
        }
    }
    panic!("no free row of {len} cells");
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, OWNER_PLAYER, x, y));
}

/// Stage Militia 900, Archer 901, Monk 902 on an open row —
/// already layered along the eastward travel direction
/// (melee front/east, monk back/west), as a group that had
/// been travelling in shape would stand — and return a
/// destination 22 cells east of the Monk.
fn stage_trio(sim: &mut Sim) -> (I16F16, I16F16) {
    let (y, x0) = free_row(sim, 30);
    spawn(sim, 900, EntityKind::Militia, (x0 + 4, y));
    spawn(sim, 901, EntityKind::Archer, (x0 + 2, y));
    spawn(sim, 902, EntityKind::Monk, (x0, y));
    sim.grid().cell_center(x0 + 24, y)
}

fn formation_cmd(entity: u32, x: I16F16, y: I16F16, formation: FormationKind) -> Command {
    Command::FormationMove { entity, x, y, formation }
}

#[test]
fn formation_move_batch_assigns_layered_slot_targets() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_trio(&mut sim);
    for id in [900, 901, 902] {
        sim.push_command(formation_cmd(id, dx, dy, FormationKind::Line));
    }
    sim.tick();
    let t = |id: u32| {
        let o = ent(&sim, id).order.as_ref().expect("no order");
        (f(o.target_x), f(o.target_y))
    };
    let (mx, _) = t(900);
    let (ax, _) = t(901);
    let (kx, _) = t(902);
    assert!(mx > ax && ax > kx, "slot targets not layered: militia {mx}, archer {ax}, monk {kx}");
    // The slots sit around the shared destination, not on it.
    assert!((mx - f(dx)).abs() <= 3.0 && (kx - f(dx)).abs() <= 3.0);
}

#[test]
fn formation_group_arrives_and_holds_its_shape() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_trio(&mut sim);
    for id in [900, 901, 902] {
        sim.push_command(formation_cmd(id, dx, dy, FormationKind::Line));
    }
    let mut arrived = false;
    for _ in 0..3600 {
        sim.tick();
        if [900, 901, 902].iter().all(|&id| ent(&sim, id).order.is_none()) {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "the formation never arrived");
    let p = |id: u32| (f(ent(&sim, id).pos_x), f(ent(&sim, id).pos_y));
    let (mx, _) = p(900);
    let (ax, _) = p(901);
    let (kx, _) = p(902);
    assert!(mx > ax && ax > kx, "the arrived shape is not layered: {mx}, {ax}, {kx}");
    // Everyone holds near the destination anchor.
    for id in [900, 901, 902] {
        let (px, py) = p(id);
        assert!((px - f(dx)).hypot(py - f(dy)) <= 4.0, "unit {id} drifted from the formation: ({px}, {py}) vs ({}, {})", f(dx), f(dy));
    }
}

#[test]
fn a_lone_formation_move_is_a_plain_move() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_trio(&mut sim);
    sim.push_command(formation_cmd(900, dx, dy, FormationKind::Box));
    sim.tick();
    let o = ent(&sim, 900).order.as_ref().expect("no order");
    assert_eq!((o.target_x, o.target_y), (dx, dy), "a lone formation order must target the destination itself");
    assert_eq!(o.group_id, 0, "a lone formation order was group-stamped");
}

#[test]
fn queued_formation_moves_slot_together_on_execution() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_trio(&mut sim);
    for id in [900, 901] {
        sim.push_command(Command::Queue {
            entity: id,
            order: QueuedOrder::FormationMove { x: dx, y: dy, formation: FormationKind::Line },
        });
    }
    sim.tick();
    sim.tick();
    let t = |id: u32| {
        let o = ent(&sim, id).order.as_ref().expect("no order after queue execution");
        f(o.target_x)
    };
    assert!(t(900) > t(901), "queued formation pair not layered: {} vs {}", t(900), t(901));
}

#[test]
fn plain_move_batches_keep_their_issued_targets() {
    // MOV-07 must not leak into the plain move path: a plain
    // batch still targets exactly what was issued per unit.
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_trio(&mut sim);
    sim.push_command(Command::Move { entity: 900, x: dx, y: dy });
    sim.push_command(Command::Move { entity: 901, x: dx, y: dy });
    sim.tick();
    for id in [900, 901] {
        let o = ent(&sim, id).order.as_ref().expect("no order");
        assert_eq!((o.target_x, o.target_y), (dx, dy), "a plain move's target was re-slotted");
    }
}

#[test]
fn formation_travel_replays_bit_identically() {
    let run = || {
        let mut sim = Sim::new(7);
        let (dx, dy) = stage_trio(&mut sim);
        for id in [900, 901, 902] {
            sim.push_command(formation_cmd(id, dx, dy, FormationKind::Box));
        }
        for _ in 0..600 {
            sim.tick();
        }
        sim.state_hash()
    };
    assert_eq!(run(), run(), "formation travel broke replay determinism");
}

#[test]
fn formation_orders_round_trip_through_a_save() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_trio(&mut sim);
    for id in [900, 901, 902] {
        sim.push_command(formation_cmd(id, dx, dy, FormationKind::Line));
    }
    // A queued formation move rides the save too (its codec).
    sim.push_command(Command::Queue {
        entity: 900,
        order: QueuedOrder::FormationMove { x: dx, y: dy, formation: FormationKind::Flank },
    });
    sim.tick();
    let target_before = {
        let o = ent(&sim, 901).order.as_ref().expect("no order");
        (o.target_x, o.target_y)
    };
    let bytes = sim.save_game();
    let hash_before = sim.state_hash();
    let mut restored = Sim::load_game(&bytes).expect("formation save did not restore");
    assert_eq!(restored.state_hash(), hash_before, "restored hash differs");
    let o = ent(&restored, 901).order.as_ref().expect("order lost in save");
    assert_eq!((o.target_x, o.target_y), target_before, "the slot target did not survive the save");
    // And the restored sim keeps travelling identically.
    for _ in 0..120 {
        sim.tick();
        restored.tick();
    }
    assert_eq!(sim.state_hash(), restored.state_hash(), "restored formation travel diverged");
}
