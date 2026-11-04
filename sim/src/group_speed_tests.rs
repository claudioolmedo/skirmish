//! MOV-06 tests: groups travel at their slowest member's speed.
//!
//! Fixture style: in-crate staging (direct entity records, the
//! `castle_p3_tests` pattern) on a `Sim::new(7)` map, with a
//! scanned open row (uniform height, nothing blocked) as the
//! travel corridor — so the geometry is exact and the speeds are
//! the unit table's own: Knight 1.35 tiles/s (5898 bits),
//! Scout Cavalry 1.20 (5243), Militia 0.90 (3932).
//!
//! The sim-level contract under test (docs/MECHANICS-RULEBOOK.md
//! MOV-06 + its Impl note): at order issue, orders that arrive
//! in one application batch from one owner, of one move kind,
//! with destinations within the client's group-offset spread,
//! form a group; the faster members' orders are stamped with a
//! group id and a speed cap = the slowest member's speed, and
//! steering clamps to the cap. The slowest member is unstamped
//! (it needs no clamp), a single-unit order is unstamped, and
//! the cap ends when the order ends or is replaced.

use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::{Command, QueuedOrder, Sim};
use fixed::types::I16F16;

const KNIGHT_SPEED: i32 = 5_898; // 1.35 tiles/s
const MILITIA_SPEED: i32 = 3_932; // 0.90 tiles/s

fn fx(v: f64) -> I16F16 {
    I16F16::from_num(v)
}

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn pos(sim: &Sim, id: u32) -> (f64, f64) {
    let e = ent(sim, id);
    (f(e.pos_x), f(e.pos_y))
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// A run of `len` consecutive unblocked, uniform-height cells in
/// one row: returns `(row_y, first_x)`. The corridor for every
/// travel test — a straight flat run, so a path along it is a
/// straight line and cruise velocities are exact.
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

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

/// Stage a Knight (900) and a Militia (901) two cells apart on
/// an open row and return the row's far-end destination point
/// (22 cells east of the Knight).
fn stage_pair(sim: &mut Sim) -> (I16F16, I16F16) {
    let (y, x0) = free_row(sim, 26);
    spawn(sim, 900, EntityKind::Knight, OWNER_PLAYER, (x0, y));
    spawn(sim, 901, EntityKind::Militia, OWNER_PLAYER, (x0 + 2, y));
    sim.grid().cell_center(x0 + 24, y)
}

fn move_cmd(entity: u32, x: I16F16, y: I16F16) -> Command {
    Command::Move { entity, x, y }
}

#[test]
fn mixed_group_stamps_the_fast_member_with_the_slowest_speed() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    sim.push_command(move_cmd(900, dx, dy));
    sim.push_command(move_cmd(901, dx, dy));
    sim.tick();
    let knight = ent(&sim, 900).order.clone().expect("knight has no order");
    assert_ne!(knight.group_id, 0, "the fast member was not group-stamped");
    assert_eq!(
        knight.group_cap.to_bits(),
        MILITIA_SPEED,
        "the cap must be the slowest member's speed"
    );
    // The slowest member needs no clamp and stays unstamped.
    let militia = ent(&sim, 901).order.clone().expect("militia has no order");
    assert_eq!(militia.group_id, 0, "the slowest member must not be stamped");
    // And the clamp is live in steering from the very first tick:
    // the Knight cruises at the Militia's speed, not its own.
    let k = ent(&sim, 900);
    assert_eq!(k.vel_x.to_bits(), MILITIA_SPEED, "knight not clamped");
    assert_eq!(k.vel_y, I16F16::ZERO);
    // The Militia cruises at its own (full) speed.
    assert_eq!(ent(&sim, 901).vel_x.to_bits(), MILITIA_SPEED);
}

#[test]
fn group_arrives_together_within_a_stated_tolerance() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    let dest = (f(dx), f(dy));
    sim.push_command(move_cmd(900, dx, dy));
    sim.push_command(move_cmd(901, dx, dy));
    // Travel ~88 world units at the Militia's 0.06 wu/tick.
    let mut arrived: [Option<u64>; 2] = [None, None];
    let mut max_gap = 0.0f64;
    for t in 0..6_000 {
        sim.tick();
        let gap = dist(pos(&sim, 900), pos(&sim, 901));
        if arrived.iter().any(|a| a.is_none()) {
            max_gap = max_gap.max(gap);
        }
        for (i, id) in [900u32, 901].iter().enumerate() {
            if arrived[i].is_none() && ent(&sim, *id).order.is_none() {
                arrived[i] = Some(t);
            }
        }
        if arrived.iter().all(|a| a.is_some()) {
            break;
        }
    }
    let [ka, ma] = arrived;
    let ka = ka.expect("the knight never arrived");
    let ma = ma.expect("the militia never arrived");
    // Arrival tolerance, stated: the Knight staged 8 wu BEHIND
    // the Militia, which at the capped 0.06 wu/tick is ~133
    // ticks of extra road, plus arrival-slowdown jitter — so
    // the pair may land up to 240 ticks apart with the Knight
    // LAST. The stringing-out failure this rule exists to kill
    // is the other direction: unclamped, the Knight (0.09
    // wu/tick) would arrive ~470 ticks BEFORE the Militia.
    assert!(
        ka + 30 >= ma,
        "the fast member arrived early: knight at {ka}, militia at {ma} — the cap is not holding"
    );
    assert!(
        ka <= ma + 240,
        "group arrivals strung out: knight at {ka}, militia at {ma} (tolerance: start offset + jitter)"
    );
    assert!(
        max_gap <= 10.0,
        "the group strung out en route: max separation {max_gap:.1} wu"
    );
    assert!(dist(pos(&sim, 900), dest) < 1.5);
    assert!(dist(pos(&sim, 901), dest) < 1.5);
}

#[test]
fn single_unit_order_is_unstamped_and_full_speed() {
    let mut sim = Sim::new(7);
    let (y, x0) = free_row(&sim, 26);
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (x0, y));
    let (dx, dy) = sim.grid().cell_center(x0 + 24, y);
    sim.push_command(move_cmd(900, dx, dy));
    sim.tick();
    let o = ent(&sim, 900).order.clone().expect("no order");
    assert_eq!(o.group_id, 0, "a single-unit order was group-stamped");
    assert_eq!(ent(&sim, 900).vel_x.to_bits(), KNIGHT_SPEED);
}

#[test]
fn equal_speed_group_stamps_nobody() {
    // Two Militia share an order: the cap would equal their own
    // speed (a trivial cap), so no stamp is written and the
    // stream stays byte-identical to the pre-MOV-06 one.
    let mut sim = Sim::new(7);
    let (y, x0) = free_row(&sim, 26);
    spawn(&mut sim, 901, EntityKind::Militia, OWNER_PLAYER, (x0, y));
    spawn(&mut sim, 902, EntityKind::Militia, OWNER_PLAYER, (x0 + 2, y));
    let (dx, dy) = sim.grid().cell_center(x0 + 24, y);
    sim.push_command(move_cmd(901, dx, dy));
    sim.push_command(move_cmd(902, dx, dy));
    sim.tick();
    for id in [901, 902] {
        let o = ent(&sim, id).order.clone().expect("no order");
        assert_eq!(o.group_id, 0, "an equal-speed member was stamped");
        assert_eq!(ent(&sim, id).vel_x.to_bits(), MILITIA_SPEED);
    }
}

#[test]
fn far_apart_orders_in_one_batch_do_not_merge() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    // The Knight east, the Militia to a point far west of it:
    // destinations > GROUP_LINK_DIST apart are separate orders
    // that merely arrived in the same tick.
    let knight_pos = pos(&sim, 900);
    sim.push_command(move_cmd(900, dx, dy));
    sim.push_command(move_cmd(901, fx(knight_pos.0 - 60.0), fx(knight_pos.1)));
    sim.tick();
    // (The west point may snap if blocked; what matters is that
    // neither order carries the other's stamp.)
    if let Some(o) = &ent(&sim, 900).order {
        assert_eq!(o.group_id, 0, "separate orders merged into a group");
    }
    assert_eq!(ent(&sim, 900).vel_x.to_bits(), KNIGHT_SPEED);
}

#[test]
fn other_owners_orders_do_not_merge() {
    let mut sim = Sim::new(7);
    let (y, x0) = free_row(&sim, 26);
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (x0, y));
    spawn(&mut sim, 910, EntityKind::Militia, OWNER_OPPONENT, (x0 + 2, y));
    let (dx, dy) = sim.grid().cell_center(x0 + 24, y);
    sim.push_command(move_cmd(900, dx, dy));
    sim.push_command(move_cmd(910, dx, dy));
    sim.tick();
    let o = ent(&sim, 900).order.clone().expect("no order");
    assert_eq!(o.group_id, 0, "another owner's order joined the group");
    assert_eq!(ent(&sim, 900).vel_x.to_bits(), KNIGHT_SPEED);
}

#[test]
fn client_offset_destinations_still_form_one_group() {
    // The client's group-move shape: each member's destination
    // keeps its offset from the centroid (here ±4 wu in y), so
    // the raw destinations differ — within the link distance
    // they are still one order.
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    sim.push_command(move_cmd(900, dx, dy + fx(4.0)));
    sim.push_command(move_cmd(901, dx, dy - fx(4.0)));
    sim.tick();
    let knight = ent(&sim, 900).order.clone().expect("no order");
    assert_ne!(knight.group_id, 0, "offset destinations did not group");
    assert_eq!(knight.group_cap.to_bits(), MILITIA_SPEED);
}

#[test]
fn reissue_inside_the_same_batch_leaves_the_group() {
    // Batch: Knight + Militia group east, then — still in the
    // same drain — the Knight is re-issued west alone. The
    // superseded group order must not stamp him, and with him
    // gone the Militia's order is a solo one too.
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    let kp = pos(&sim, 900);
    sim.push_command(move_cmd(900, dx, dy));
    sim.push_command(move_cmd(901, dx, dy));
    sim.push_command(move_cmd(900, fx(kp.0 - 60.0), fx(kp.1)));
    sim.tick();
    let o = ent(&sim, 900).order.clone().expect("no order");
    assert_eq!(o.group_id, 0, "a superseded group order stamped the knight");
    assert_eq!(ent(&sim, 900).vel_x.to_bits(), -KNIGHT_SPEED);
    assert_eq!(
        ent(&sim, 901).order.as_ref().unwrap().group_id,
        0,
        "the abandoned member kept a group of one"
    );
}

#[test]
fn cap_releases_when_the_order_is_replaced() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    sim.push_command(move_cmd(900, dx, dy));
    sim.push_command(move_cmd(901, dx, dy));
    sim.tick();
    assert_ne!(ent(&sim, 900).order.as_ref().unwrap().group_id, 0);
    // A fresh solo order for the Knight: the cap ends with the
    // group order it was stamped on.
    let back = pos(&sim, 901);
    sim.push_command(move_cmd(900, fx(back.0 - 60.0), fx(back.1)));
    sim.tick();
    let o = ent(&sim, 900).order.clone().expect("no order");
    assert_eq!(o.group_id, 0, "the cap survived a replacement order");
    // Cruise speed is the Knight's own again (heading back west).
    assert_eq!(ent(&sim, 900).vel_x.to_bits(), -KNIGHT_SPEED);
}

#[test]
fn cap_releases_after_arrival() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    sim.push_command(move_cmd(900, dx, dy));
    sim.push_command(move_cmd(901, dx, dy));
    let done = (0..6_000).any(|_| {
        sim.tick();
        ent(&sim, 900).order.is_none() && ent(&sim, 901).order.is_none()
    });
    assert!(done, "the group never arrived");
    // The Knight alone again, back west: full speed, no stamp.
    let kp = pos(&sim, 900);
    sim.push_command(move_cmd(900, fx(kp.0 - 60.0), fx(kp.1)));
    sim.tick();
    let o = ent(&sim, 900).order.clone().expect("no order");
    assert_eq!(o.group_id, 0, "the cap survived arrival");
    assert_eq!(ent(&sim, 900).vel_x.to_bits(), -KNIGHT_SPEED);
}

#[test]
fn slowest_member_dying_keeps_the_issue_time_stamp() {
    // The rulebook's Impl note stamps the cap at issue; it is
    // not recomputed when the group changes mid-travel. A dead
    // slowest member leaves the survivors' cap where it was
    // stamped until their orders end.
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    sim.push_command(move_cmd(900, dx, dy));
    sim.push_command(move_cmd(901, dx, dy));
    sim.tick();
    assert_ne!(ent(&sim, 900).order.as_ref().unwrap().group_id, 0);
    // Kill the Militia outright (the death sweep processes it).
    let idx = sim.entities.iter().position(|e| e.id == 901).unwrap();
    sim.entities[idx].hp = I16F16::ZERO;
    sim.tick();
    assert!(ent(&sim, 901).dead, "the militia did not die");
    let o = ent(&sim, 900).order.clone().expect("knight lost its order");
    assert_ne!(o.group_id, 0, "the stamp was recomputed away on a death");
    assert_eq!(o.group_cap.to_bits(), MILITIA_SPEED);
    assert_eq!(ent(&sim, 900).vel_x.to_bits(), MILITIA_SPEED);
}

#[test]
fn queued_group_move_groups_on_execution() {
    // A Shift+click group queues one Move per unit in one
    // batch; the orders execute together in the queue advance
    // and group there.
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    for id in [900, 901] {
        sim.push_command(Command::Queue {
            entity: id,
            order: QueuedOrder::Move { x: dx, y: dy },
        });
    }
    sim.tick();
    let knight = ent(&sim, 900).order.clone().expect("queued order never ran");
    assert_ne!(knight.group_id, 0, "the queued group was not stamped");
    assert_eq!(knight.group_cap.to_bits(), MILITIA_SPEED);
    assert_eq!(ent(&sim, 901).order.as_ref().unwrap().group_id, 0);
}

#[test]
fn attack_move_issued_leg_is_grouped() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    sim.push_command(Command::AttackMove {
        entity: 900,
        x: dx,
        y: dy,
    });
    sim.push_command(Command::AttackMove {
        entity: 901,
        x: dx,
        y: dy,
    });
    sim.tick();
    assert!(sim.attack_moving(900) && sim.attack_moving(901));
    let knight = ent(&sim, 900).order.clone().expect("no order");
    assert_ne!(knight.group_id, 0, "the attack-move group was not stamped");
    assert_eq!(knight.group_cap.to_bits(), MILITIA_SPEED);
    assert_eq!(ent(&sim, 900).vel_x.to_bits(), MILITIA_SPEED);
}

#[test]
fn three_member_group_caps_at_the_slowest() {
    let mut sim = Sim::new(7);
    let (y, x0) = free_row(&sim, 28);
    spawn(&mut sim, 900, EntityKind::Knight, OWNER_PLAYER, (x0, y));
    spawn(&mut sim, 902, EntityKind::ScoutCavalry, OWNER_PLAYER, (x0 + 2, y));
    spawn(&mut sim, 901, EntityKind::Militia, OWNER_PLAYER, (x0 + 4, y));
    let (dx, dy) = sim.grid().cell_center(x0 + 26, y);
    for id in [900, 902, 901] {
        sim.push_command(move_cmd(id, dx, dy));
    }
    sim.tick();
    // Both faster members carry the same group id and the
    // Militia's speed as cap; the Militia is unstamped.
    let k = ent(&sim, 900).order.clone().unwrap();
    let s = ent(&sim, 902).order.clone().unwrap();
    assert_ne!(k.group_id, 0);
    assert_eq!(k.group_id, s.group_id, "one order, one group id");
    assert_eq!(k.group_cap.to_bits(), MILITIA_SPEED);
    assert_eq!(s.group_cap.to_bits(), MILITIA_SPEED);
    assert_eq!(ent(&sim, 902).vel_x.to_bits(), MILITIA_SPEED);
    assert_eq!(ent(&sim, 901).order.as_ref().unwrap().group_id, 0);
}

#[test]
fn group_state_round_trips_through_a_save() {
    let mut sim = Sim::new(7);
    let (dx, dy) = stage_pair(&mut sim);
    sim.push_command(move_cmd(900, dx, dy));
    sim.push_command(move_cmd(901, dx, dy));
    for _ in 0..30 {
        sim.tick();
    }
    let gid = ent(&sim, 900).order.as_ref().unwrap().group_id;
    assert_ne!(gid, 0);
    let bytes = sim.save_game();
    let mut restored = Sim::load_game(&bytes).expect("group save did not restore");
    assert_eq!(restored.state_hash(), sim.state_hash());
    let o = ent(&restored, 900).order.clone().unwrap();
    assert_eq!(o.group_id, gid, "the stamp did not survive the save");
    assert_eq!(o.group_cap.to_bits(), MILITIA_SPEED);
    // The allocator continues where it left off: a new group
    // (Scout + Villager speeds differ) gets a fresh id.
    let (y, x0) = free_row(&restored, 26);
    spawn(&mut restored, 920, EntityKind::ScoutCavalry, OWNER_PLAYER, (x0, y));
    spawn(&mut restored, 921, EntityKind::Villager, OWNER_PLAYER, (x0 + 2, y));
    let (dx2, dy2) = restored.grid().cell_center(x0 + 24, y);
    restored.push_command(move_cmd(920, dx2, dy2));
    restored.push_command(move_cmd(921, dx2, dy2));
    restored.tick();
    let o2 = ent(&restored, 920).order.clone().unwrap();
    assert_ne!(o2.group_id, 0, "the restored sim stopped grouping");
    assert_ne!(o2.group_id, gid, "the restored sim reissued a live group id");
}

#[test]
fn group_travel_replays_bit_identically() {
    let run = || -> Vec<u64> {
        let mut sim = Sim::new(7);
        let (dx, dy) = stage_pair(&mut sim);
        sim.push_command(move_cmd(900, dx, dy));
        sim.push_command(move_cmd(901, dx, dy));
        (0..2_500).map(|_| sim.tick()).collect()
    };
    assert_eq!(run(), run(), "group travel broke replay determinism");
}
