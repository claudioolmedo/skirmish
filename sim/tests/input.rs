//! Input & command completion tests (M4): queued orders (SEL-12),
//! rally points (SEL-13), gatherer retargeting (ECO-09), and the
//! idle queries behind the idle keys and indicators (SEL-08/09,
//! INF-03).
//!
//! Entity ids in the standard start: 0 Town Center, 1–3 villagers,
//! 4 Scout Cavalry, 5–8 near sheep, 9–12 far sheep, 13–14 boars,
//! 15–17 deer (see `ANIMAL_SPECS` in lib.rs). Node ids follow
//! `NODE_SPECS` in econ.rs: 0–7 west-forest trees, 8–10 east-copse
//! trees, 11–14 berries, 15–17 gold, 18–19 stone.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, Entity, EntityKind, QueuedOrder, ResourceKind, Sim, Task,
};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn cell_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn dist_to(sim: &Sim, id: u32, x: f64, y: f64) -> f64 {
    let e = ent(sim, id);
    let dx = f(e.pos_x) - x;
    let dy = f(e.pos_y) - y;
    (dx * dx + dy * dy).sqrt()
}

fn node_amount(sim: &Sim, node: u32) -> I16F16 {
    sim.nodes().iter().find(|n| n.id == node).unwrap().amount_left
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

/// Build a Barracks east of the wall (the combat tests' proven
/// spot) with villager 1 and return its building id.
fn build_barracks(sim: &mut Sim) -> u32 {
    let (x, y) = cell_point(sim, 27, 20);
    sim.push_command(Command::Build {
        entity: 1,
        kind: BuildingKind::Barracks,
        x,
        y,
    });
    let built = run_until(sim, 30_000, |s| {
        s.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::Barracks && b.complete)
    });
    assert!(built, "barracks never completed");
    sim.buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Barracks)
        .unwrap()
        .id
}

// ---------------------------------------------------------------------
// SEL-12: queued orders
// ---------------------------------------------------------------------

#[test]
fn queued_moves_execute_in_fifo_order() {
    let mut sim = Sim::new(7);
    // Three open-ground points, all west of the wall.
    let a = cell_point(&sim, 15, 23); // (-38, -6)
    let b = cell_point(&sim, 15, 30); // (-38, 22)
    let c = cell_point(&sim, 12, 15); // (-50, -38)
    sim.push_command(Command::Move {
        entity: 1,
        x: a.0,
        y: a.1,
    });
    sim.push_command(Command::Queue {
        entity: 1,
        order: QueuedOrder::Move { x: b.0, y: b.1 },
    });
    sim.push_command(Command::Queue {
        entity: 1,
        order: QueuedOrder::Move { x: c.0, y: c.1 },
    });

    let (mut ta, mut tb, mut tc) = (None, None, None);
    for tick in 0..6000u64 {
        sim.tick();
        if ta.is_none() && dist_to(&sim, 1, f(a.0), f(a.1)) < 1.5 {
            ta = Some(tick);
        }
        if tb.is_none() && dist_to(&sim, 1, f(b.0), f(b.1)) < 1.5 {
            tb = Some(tick);
        }
        if tc.is_none() && dist_to(&sim, 1, f(c.0), f(c.1)) < 1.5 {
            tc = Some(tick);
        }
        if tc.is_some() {
            break;
        }
    }
    let (ta, tb, tc) = (ta.expect("never reached A"), tb.expect("never reached B"), tc.expect("never reached C"));
    assert!(ta < tb && tb < tc, "visits out of order: A@{ta} B@{tb} C@{tc}");
    // The chain is spent: the final order completes and nothing is
    // queued behind it. (Arrival within 1.5 can precede the order's
    // own completion by a few ticks — settle first.)
    let settled = run_until(&mut sim, 600, |s| {
        let v = ent(s, 1);
        v.order.is_none() && v.order_queue.is_empty()
    });
    assert!(settled, "the last queued move never completed");
}

#[test]
fn plain_command_replaces_the_whole_queue() {
    let mut sim = Sim::new(7);
    let a = cell_point(&sim, 15, 23);
    let b = cell_point(&sim, 15, 30);
    let c = cell_point(&sim, 19, 19); // (-22, -22)
    sim.push_command(Command::Move {
        entity: 1,
        x: a.0,
        y: a.1,
    });
    sim.push_command(Command::Queue {
        entity: 1,
        order: QueuedOrder::Move { x: b.0, y: b.1 },
    });
    sim.tick(); // walking to A, B queued behind it
    assert_eq!(ent(&sim, 1).order_queue.len(), 1);

    // A fresh, non-queued Move supersedes everything.
    sim.push_command(Command::Move {
        entity: 1,
        x: c.0,
        y: c.1,
    });
    sim.tick();
    assert!(ent(&sim, 1).order_queue.is_empty(), "queue not cleared by a plain command");

    let arrived = run_until(&mut sim, 6000, |s| dist_to(s, 1, f(c.0), f(c.1)) < 1.5);
    assert!(arrived, "villager never reached the replacement target");
    // And it stays there: the queued trip to B is gone for good.
    for _ in 0..600 {
        sim.tick();
    }
    assert!(dist_to(&sim, 1, f(c.0), f(c.1)) < 1.5, "villager left for the cleared queue target");
    assert!(ent(&sim, 1).order.is_none());
}

#[test]
fn queued_gather_runs_after_the_current_move() {
    let mut sim = Sim::new(7);
    let near = cell_point(&sim, 18, 23); // (-26, -6), open ground
    sim.push_command(Command::Move {
        entity: 2,
        x: near.0,
        y: near.1,
    });
    sim.push_command(Command::Queue {
        entity: 2,
        order: QueuedOrder::Gather { node: 0 }, // west-forest tree
    });
    // The gather must not start until the move lands: mid-walk the
    // villager has no task assignment yet.
    for _ in 0..10 {
        sim.tick();
    }
    assert_eq!(ent(&sim, 2).task_node, None);

    let started = run_until(&mut sim, 20_000, |s| ent(s, 2).task_node == Some(0));
    assert!(started, "queued gather never started after the move");
    // …and it produces: the first deposit raises the wood stockpile.
    let start_wood = sim.stockpile().amount(ResourceKind::Wood);
    let produced = run_until(&mut sim, 30_000, |s| {
        s.stockpile().amount(ResourceKind::Wood) > start_wood
    });
    assert!(produced, "queued gather never deposited wood");
}

#[test]
fn queued_order_for_an_idle_unit_starts_immediately() {
    let mut sim = Sim::new(7);
    // Villager 3 stands idle; a queued gather is its first order.
    assert!(sim.can_queue(3, QueuedOrder::Gather { node: 11 }));
    sim.push_command(Command::Queue {
        entity: 3,
        order: QueuedOrder::Gather { node: 11 }, // berry bush
    });
    sim.tick();
    let v = ent(&sim, 3);
    assert_eq!(v.task_node, Some(11), "queued gather did not start on the same tick");
    assert!(v.order_queue.is_empty());
}

#[test]
fn queued_attack_executes_after_the_current_move() {
    let mut sim = Sim::new(11);
    let bid = build_barracks(&mut sim);
    sim.push_command(Command::TrainAt {
        building: bid,
        kind: EntityKind::Militia,
    });
    let trained = run_until(&mut sim, 10_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Militia)
    });
    assert!(trained, "militia never trained");
    let militia = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Militia)
        .unwrap()
        .id;
    let boar = 13; // north-west boar, cell (8, 10)
    let boar_hp = |s: &Sim| f(ent(s, boar).hp);

    let p = cell_point(&sim, 18, 20); // (-26, -18), on the way west
    sim.push_command(Command::Move {
        entity: militia,
        x: p.0,
        y: p.1,
    });
    sim.push_command(Command::Queue {
        entity: militia,
        order: QueuedOrder::Attack { target: boar },
    });

    let (mut tp, mut thit) = (None, None);
    for tick in 0..60_000u64 {
        sim.tick();
        if tp.is_none() && dist_to(&sim, militia, f(p.0), f(p.1)) < 1.5 {
            tp = Some(tick);
        }
        if thit.is_none() && boar_hp(&sim) < 75.0 {
            thit = Some(tick);
        }
        if tp.is_some() && thit.is_some() {
            break;
        }
    }
    let tp = tp.expect("militia never reached the waypoint");
    let thit = thit.expect("boar was never hit — queued attack never ran");
    assert!(tp < thit, "boar hit at {thit} before the move finished at {tp}");
    assert_eq!(ent(&sim, militia).attack_target, Some(boar));
}

#[test]
fn queued_build_executes_after_the_current_move() {
    let mut sim = Sim::new(7);
    let near = cell_point(&sim, 18, 23);
    // House anchor east of the wall: the old (20, 27) spot has a
    // sheep standing on it, and placement refuses to cover a
    // standing unit (M6 rule).
    let site = cell_point(&sim, 28, 30);
    let start_wood = sim.stockpile().amount(ResourceKind::Wood);
    sim.push_command(Command::Move {
        entity: 1,
        x: near.0,
        y: near.1,
    });
    sim.push_command(Command::Queue {
        entity: 1,
        order: QueuedOrder::Build {
            kind: BuildingKind::House,
            x: site.0,
            y: site.1,
        },
    });
    sim.tick();
    // Nothing placed yet — the build waits behind the move, and its
    // wood is NOT charged until it executes.
    assert!(sim.buildings().is_empty());
    assert_eq!(sim.stockpile().amount(ResourceKind::Wood), start_wood);

    let placed = run_until(&mut sim, 20_000, |s| {
        s.buildings().iter().any(|b| b.kind == BuildingKind::House)
    });
    assert!(placed, "queued build never placed the house");
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        start_wood - I16F16::from_num(25),
        "house wood must be charged at execution, exactly once"
    );
    let done = run_until(&mut sim, 20_000, |s| s.buildings()[0].complete);
    assert!(done, "queued-build house never completed");
}

#[test]
fn queue_validation_and_cap() {
    let mut sim = Sim::new(7);
    // Shape rules: the Town Center takes no orders; a Scout cannot
    // gather or build; a boar is not a legal attack target for…
    // (a boar IS legal — use the Town Center as an illegal target.)
    assert!(!sim.can_queue(0, QueuedOrder::Move { x: I16F16::ZERO, y: I16F16::ZERO }));
    assert!(!sim.can_queue(4, QueuedOrder::Gather { node: 0 }));
    assert!(!sim.can_queue(4, QueuedOrder::Build {
        kind: BuildingKind::House,
        x: I16F16::ZERO,
        y: I16F16::ZERO,
    }));
    assert!(!sim.can_queue(1, QueuedOrder::Attack { target: 0 }));
    assert!(sim.can_queue(1, QueuedOrder::Attack { target: 13 }));
    // Rally validation: Town Center always; unknown buildings never;
    // a House is not a production building.
    assert!(sim.can_set_rally(None));
    assert!(!sim.can_set_rally(Some(999)));

    // The cap: far more Queue commands than ORDER_QUEUE_CAP are
    // pushed in one tick; only the cap's worth are appended, and the
    // queue-advance step starts the first one immediately (the
    // villager was idle), leaving CAP-1 waiting behind the walk.
    let far = cell_point(&sim, 15, 23);
    for _ in 0..(skirmish_sim::ORDER_QUEUE_CAP + 4) {
        sim.push_command(Command::Queue {
            entity: 1,
            order: QueuedOrder::Move { x: far.0, y: far.1 },
        });
    }
    sim.tick();
    assert_eq!(
        ent(&sim, 1).order_queue.len(),
        skirmish_sim::ORDER_QUEUE_CAP - 1,
        "queue cap not enforced at append time"
    );
    // While it walks (busy, CAP-1 queued), exactly one more fits;
    // then can_queue reports the cap.
    assert!(sim.can_queue(1, QueuedOrder::Move { x: far.0, y: far.1 }));
    sim.push_command(Command::Queue {
        entity: 1,
        order: QueuedOrder::Move { x: far.0, y: far.1 },
    });
    sim.tick(); // appends (queue now at cap); the walk continues
    assert_eq!(ent(&sim, 1).order_queue.len(), skirmish_sim::ORDER_QUEUE_CAP);
    assert!(!sim.can_queue(1, QueuedOrder::Move { x: far.0, y: far.1 }));
}

#[test]
fn queued_script_replays_bit_identically() {
    fn run() -> (Vec<u64>, u32) {
        let mut sim = Sim::new(1234);
        let a = cell_point(&sim, 15, 23);
        let b = cell_point(&sim, 15, 30);
        let berry = cell_point(&sim, 21, 24);
        sim.push_command(Command::Move {
            entity: 1,
            x: a.0,
            y: a.1,
        });
        sim.push_command(Command::Queue {
            entity: 1,
            order: QueuedOrder::Move { x: b.0, y: b.1 },
        });
        sim.push_command(Command::Queue {
            entity: 1,
            order: QueuedOrder::Gather { node: 0 },
        });
        sim.push_command(Command::Queue {
            entity: 4,
            order: QueuedOrder::Move { x: b.0, y: b.1 },
        });
        // Rally the Town Center onto the berries and train: the new
        // villager goes straight to work (SEL-13 in the same replay).
        sim.push_command(Command::SetRally {
            building: None,
            x: berry.0,
            y: berry.1,
        });
        sim.push_command(Command::TrainVillager);
        let mut hashes = Vec::new();
        for _ in 0..6000 {
            hashes.push(sim.tick());
        }
        (hashes, sim.villager_count())
    }
    let (h1, pop1) = run();
    let (h2, pop2) = run();
    assert_eq!(h1, h2, "queued/rally script diverged between runs");
    assert_eq!(pop1, 4, "the rallied villager never trained");
    assert_eq!(pop1, pop2);
}

// ---------------------------------------------------------------------
// SEL-13: rally points
// ---------------------------------------------------------------------

#[test]
fn tc_rally_point_walks_the_new_villager_there() {
    let mut sim = Sim::new(7);
    let point = cell_point(&sim, 18, 23); // (-26, -6), open ground
    sim.push_command(Command::SetRally {
        building: None,
        x: point.0,
        y: point.1,
    });
    sim.tick();
    let (rx, ry, node) = sim.tc_rally().expect("rally not stored");
    assert_eq!((rx, ry), (point.0, point.1));
    assert_eq!(node, None, "open ground must not snap to a node");

    sim.push_command(Command::TrainVillager);
    let arrived = run_until(&mut sim, 10_000, |s| {
        s.villager_count() == 4 && dist_to(s, 18, f(point.0), f(point.1)) < 1.5
    });
    assert!(arrived, "new villager never walked to the rally point");
    assert_eq!(ent(&sim, 18).task, Task::Idle, "a point rally is a walk, not a task");
}

#[test]
fn tc_rally_on_a_node_sends_the_new_villager_to_work() {
    let mut sim = Sim::new(7);
    let berry = cell_point(&sim, 21, 24); // node 11's center
    sim.push_command(Command::SetRally {
        building: None,
        x: berry.0,
        y: berry.1,
    });
    sim.tick();
    let (_, _, node) = sim.tc_rally().expect("rally not stored");
    assert_eq!(node, Some(11), "rally on the bush must snap to it");

    sim.push_command(Command::TrainVillager);
    let working = run_until(&mut sim, 10_000, |s| {
        s.villager_count() == 4 && ent(s, 18).task_node == Some(11)
    });
    assert!(working, "new villager never went to work on the rallied node");
    // And it produces: berries land in the food stockpile.
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    let produced = run_until(&mut sim, 30_000, |s| {
        s.stockpile().amount(ResourceKind::Food) > start_food
    });
    assert!(produced, "rallied villager never deposited food");
}

#[test]
fn barracks_rally_walks_the_new_unit_there() {
    let mut sim = Sim::new(11);
    let bid = build_barracks(&mut sim);
    let point = cell_point(&sim, 30, 20); // (22, -18), east open ground
    sim.push_command(Command::SetRally {
        building: Some(bid),
        x: point.0,
        y: point.1,
    });
    sim.tick();
    let (rx, ry, node) = sim.building_rally(bid).expect("barracks rally not stored");
    assert_eq!((rx, ry), (point.0, point.1));
    assert_eq!(node, None);

    sim.push_command(Command::TrainAt {
        building: bid,
        kind: EntityKind::Militia,
    });
    let arrived = run_until(&mut sim, 20_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::Militia)
            && s
                .entities()
                .iter()
                .filter(|e| e.kind == EntityKind::Militia)
                .all(|m| dist_to(s, m.id, f(point.0), f(point.1)) < 1.5)
    });
    assert!(arrived, "new militia never walked to the barracks rally point");
}

// ---------------------------------------------------------------------
// ECO-09: gatherer retargeting
// ---------------------------------------------------------------------

#[test]
fn retarget_continues_at_the_same_resource_neighbor() {
    let mut sim = Sim::new(9);
    // All three villagers on the first west-forest tree (node 0).
    for v in 1..=3 {
        sim.push_command(Command::Gather {
            entity: v,
            node: 0,
        });
    }
    let dried = run_until(&mut sim, 90_000, |s| node_amount(s, 0) == I16F16::ZERO);
    assert!(dried, "the shared tree never depleted");
    // Nobody goes idle while the woodline stands: within a few
    // thousand ticks (deposit trips included) every villager's
    // assignment is the neighboring tree — node 1, the nearest
    // same-resource node to node 0 (ties at 4 units break low).
    let moved_on = run_until(&mut sim, 6_000, |s| {
        (1..=3).all(|v| ent(s, v).task_node == Some(1))
    });
    assert!(moved_on, "villagers went idle instead of retargeting to the next tree");
    // And the work is real: the stockpile keeps growing past the
    // first tree's 100 wood.
    let wood = sim.stockpile().amount(ResourceKind::Wood);
    let grew = run_until(&mut sim, 30_000, |s| {
        s.stockpile().amount(ResourceKind::Wood) > wood
    });
    assert!(grew, "retargeted villagers produced nothing");
}

#[test]
fn retarget_goes_idle_when_no_neighbor_is_in_reach() {
    let mut sim = Sim::new(13);
    // Sheep 12 grazes south of the horizontal wall at (-34, 34) —
    // far from every other food node (the berries are ~38 units
    // away, far outside the retarget radius).
    sim.push_command(Command::Attack {
        entity: 1,
        target: 12,
    });
    // The killer harvests the carcass unprompted (M3); it decays as
    // it is worked, and between the two it runs dry. ("Dry" requires
    // a carcass to exist first — an empty filter would pass at tick
    // zero — and the hunter fully stood down: not fighting, not
    // hauling.)
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    let dry = run_until(&mut sim, 120_000, |s| {
        let carcasses: Vec<_> = s
            .nodes()
            .iter()
            .filter(|n| n.decay_per_tick > I16F16::ZERO)
            .collect();
        let v = ent(s, 1);
        !carcasses.is_empty()
            && carcasses.iter().all(|n| n.amount_left == I16F16::ZERO)
            && v.task == Task::Idle
            && v.attack_target.is_none()
            && v.carry == I16F16::ZERO
    });
    assert!(dry, "carcass never ran dry, or the hunter never stood down");
    let v = ent(&sim, 1);
    assert_eq!(v.task, Task::Idle);
    assert_eq!(v.task_node, None, "no retarget should exist out there");
    assert_eq!(v.carry, I16F16::ZERO);
    assert!(
        sim.stockpile().amount(ResourceKind::Food) > start_food,
        "the hunter banked nothing from the carcass"
    );
}

// ---------------------------------------------------------------------
// SEL-08/09: idle queries
// ---------------------------------------------------------------------

#[test]
fn idle_queries_track_who_is_free() {
    let mut sim = Sim::new(7);
    assert_eq!(sim.idle_villagers(), vec![1, 2, 3]);
    assert_eq!(sim.idle_military(), vec![4], "the Scout starts idle");

    // A moving villager is not idle; a gathering one is not either.
    let a = cell_point(&sim, 15, 23);
    sim.push_command(Command::Move {
        entity: 1,
        x: a.0,
        y: a.1,
    });
    sim.push_command(Command::Gather {
        entity: 2,
        node: 11,
    });
    // The Scout, ordered onto a sheep, is busy military.
    sim.push_command(Command::Attack {
        entity: 4,
        target: 5,
    });
    sim.tick();
    assert_eq!(sim.idle_villagers(), vec![3]);
    assert!(sim.idle_military().is_empty(), "attacking Scout counted as idle");

    // When the move lands, the villager is idle again.
    let back = run_until(&mut sim, 6000, |s| s.idle_villagers().contains(&1));
    assert!(back, "villager never became idle after arriving");
}
