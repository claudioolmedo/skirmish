//! Movement tests (M1, adapted for M2): A* orders, arrival, separation,
//! determinism.
//!
//! These prove the movement contract on top of the M0 harness: move
//! commands produce deterministic, wall-avoiding motion; entities arrive;
//! separation keeps them from stacking; and the whole thing replays
//! bit-identically.
//!
//! M2 adaptation: entity 0 is now the static Town Center (it ignores
//! `Move`), and the moving units are the villagers, ids 1..=3 at start.

use fixed::types::I16F16;
use skirmish_sim::{Command, EntityKind, Sim};

fn f64x(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn dist_to(e: &skirmish_sim::Entity, tx: f64, ty: f64) -> f64 {
    let dx = f64x(e.pos_x) - tx;
    let dy = f64x(e.pos_y) - ty;
    (dx * dx + dy * dy).sqrt()
}

fn by_id(sim: &Sim, id: u32) -> &skirmish_sim::Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

#[test]
fn move_order_arrives_at_target() {
    let mut sim = Sim::new(7);
    // Far corner, open field: cell (45, 5) — forces a real A* route.
    sim.push_command(Command::Move {
        entity: 1,
        x: I16F16::from_num(80),
        y: I16F16::from_num(-80),
    });
    // The order must install a waypoint trail immediately.
    sim.tick();
    let order = by_id(&sim, 1)
        .order
        .clone()
        .expect("move order was not installed");
    assert!(order.waypoints.len() >= 2, "expected a real path");

    let mut arrived_at = None;
    // Pace correction (2026-10): the ~254-unit cross-map leg at the
    // corrected 0.8 tiles/s takes ≈4,800 cruise ticks (plus forest
    // detours and the arrival slowdown) — the old 2,400-tick budget
    // encoded the prototype pace. The arrival assertion below is
    // unchanged.
    for tick in 0..12_000 {
        sim.tick();
        if by_id(&sim, 1).order.is_none() {
            arrived_at = Some(tick);
            break;
        }
    }
    assert!(arrived_at.is_some(), "villager 1 never arrived");
    let d = dist_to(by_id(&sim, 1), 80.0, -80.0);
    assert!(
        d < 1.5,
        "villager stopped {d:.2} units from target — arrival broken"
    );
}

#[test]
fn move_commands_replay_bit_identically() {
    fn run() -> Vec<u64> {
        let mut sim = Sim::new(1234);
        let mut hashes = Vec::new();
        for tick in 0..900 {
            if tick == 10 {
                sim.push_command(Command::Move {
                    entity: 1,
                    x: I16F16::from_num(50),
                    y: I16F16::from_num(50),
                });
                sim.push_command(Command::Move {
                    entity: 2,
                    x: I16F16::from_num(-50),
                    y: I16F16::from_num(-50),
                });
            }
            if tick == 400 {
                // Re-target mid-route: the path recompute must be
                // deterministic too.
                sim.push_command(Command::Move {
                    entity: 1,
                    x: I16F16::from_num(-70),
                    y: I16F16::from_num(70),
                });
            }
            hashes.push(sim.tick());
        }
        hashes
    }
    assert_eq!(run(), run());
}

#[test]
fn move_orders_keep_different_seeds_diverged() {
    let mut a = Sim::new(101);
    let mut b = Sim::new(202);
    for e in 1..=3u32 {
        let cmd = Command::Move {
            entity: e,
            x: I16F16::from_num(60),
            y: I16F16::from_num(-60),
        };
        a.push_command(cmd);
        b.push_command(cmd);
    }
    let mut diverged = false;
    for _ in 0..600 {
        if a.tick() != b.tick() {
            diverged = true;
            break;
        }
    }
    assert!(diverged, "different seeds stayed identical under orders");
}

#[test]
fn separation_keeps_entities_from_stacking() {
    let mut sim = Sim::new(99);
    // Send every villager to the same open point: cell (40, 40).
    let ids: Vec<u32> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager)
        .map(|e| e.id)
        .collect();
    assert_eq!(ids.len(), 3, "M2 starts with 3 villagers");
    for id in ids {
        sim.push_command(Command::Move {
            entity: id,
            x: I16F16::from_num(60),
            y: I16F16::from_num(60),
        });
    }
    for _ in 0..3000 {
        sim.tick();
    }
    let ents: Vec<&skirmish_sim::Entity> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager)
        .collect();
    let mut min_d2 = f64::MAX;
    for i in 0..ents.len() {
        for j in (i + 1)..ents.len() {
            let dx = f64x(ents[i].pos_x) - f64x(ents[j].pos_x);
            let dy = f64x(ents[i].pos_y) - f64x(ents[j].pos_y);
            min_d2 = min_d2.min(dx * dx + dy * dy);
        }
    }
    let min_d = min_d2.sqrt();
    assert!(
        min_d > 0.2,
        "villagers stacked: min pairwise distance {min_d:.3}"
    );
}

#[test]
fn order_survives_serialization_shape() {
    // Commands are plain data: a move order round-trips through a manual
    // field copy exactly (this is the shape lockstep will send on the wire).
    let cmd = Command::Move {
        entity: 3,
        x: I16F16::from_num(12),
        y: I16F16::from_num(-34),
    };
    let Command::Move { entity, x, y } = cmd else {
        panic!("pattern shape changed");
    };
    assert_eq!(
        (entity, x, y),
        (3, I16F16::from_num(12), I16F16::from_num(-34))
    );

    let mut sim = Sim::new(5);
    sim.push_command(cmd);
    sim.tick();
    let o = by_id(&sim, 3).order.clone().expect("order missing");
    assert_eq!((o.target_x, o.target_y), (x, y));
}
