//! AutoExplore (SEL-17): the starting Scout Cavalry explores its
//! owner's fog frontier on its own.
//!
//! Pins the feature's contract: only the starting scout is
//! eligible (eligibility is derived from spawn order — the
//! lowest-id Scout Cavalry of each owner — so a trained scout,
//! always a later id, can never carry the flag; the rejection side
//! is exercised here on every other kind, the acceptance side on
//! both players' starting scouts), the walker actually explores,
//! any manual order cancels the mode, toggling off stops the
//! scout where it stands, and two runs fed the same commands stay
//! bit-identical (the mode is hashed state, not a client trick).

use skirmish_sim::{Command, EntityKind, QueuedOrder, Sim, OWNER_OPPONENT, OWNER_PLAYER};

fn starting_scout(sim: &Sim, owner: u8) -> u32 {
    sim.starting_scout_of(owner).expect("a starting scout")
}

fn explored_cells(sim: &Sim, owner: u8) -> usize {
    sim.visibility(owner).iter().filter(|&&v| v >= 1).count()
}

fn pos_of(sim: &Sim, id: u32) -> (f64, f64) {
    let e = &sim.entities()[id as usize];
    (e.pos_x.to_num::<f64>(), e.pos_y.to_num::<f64>())
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

#[test]
fn auto_explore_rides_and_the_frontier_grows() {
    let seed = 42;
    let mut auto = Sim::new(seed);
    let mut control = Sim::new(seed);
    let scout = starting_scout(&auto, OWNER_PLAYER);
    let spawn = pos_of(&auto, scout);
    auto.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: true,
    });
    // Three minutes of game time, no further orders.
    for _ in 0..10_800 {
        auto.tick();
        control.tick();
    }
    assert!(auto.auto_explore_on(scout), "the mode switched itself off");
    let travelled = dist(pos_of(&auto, scout), spawn);
    assert!(
        travelled > 40.0,
        "the scout barely moved ({travelled:.1} world units)"
    );
    let (grown, base) = (
        explored_cells(&auto, OWNER_PLAYER),
        explored_cells(&control, OWNER_PLAYER),
    );
    assert!(
        grown > base + 500,
        "exploration did not grow the frontier: {grown} vs control {base}"
    );
}

#[test]
fn auto_explore_is_bit_deterministic() {
    let run = |seed: u64| -> Vec<u64> {
        let mut sim = Sim::new(seed);
        let scout = starting_scout(&sim, OWNER_PLAYER);
        sim.push_command(Command::SetAutoExplore {
            entity: scout,
            enabled: true,
        });
        let mut hashes = Vec::new();
        for i in 0..7_200 {
            let h = sim.tick();
            if i % 600 == 0 {
                hashes.push(h);
            }
        }
        hashes
    };
    assert_eq!(run(99), run(99), "identical auto-explore runs diverged");
    // And the mode is real hashed state: the same seed without the
    // toggle produces a different stream.
    let mut plain = Sim::new(99);
    let mut plain_hashes = Vec::new();
    for i in 0..7_200 {
        let h = plain.tick();
        if i % 600 == 0 {
            plain_hashes.push(h);
        }
    }
    assert_ne!(run(99), plain_hashes, "the toggle changed nothing");
}

#[test]
fn a_manual_move_cancels_auto_explore() {
    let mut sim = Sim::new(7);
    let scout = starting_scout(&sim, OWNER_PLAYER);
    sim.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: true,
    });
    for _ in 0..600 {
        sim.tick();
    }
    assert!(sim.auto_explore_on(scout));
    // Ride home: any manual order takes the scout back.
    let tc = &sim.entities()[0];
    let (tx, ty) = (tc.pos_x, tc.pos_y);
    sim.push_command(Command::Move {
        entity: scout,
        x: tx,
        y: ty,
    });
    sim.tick();
    assert!(
        !sim.auto_explore_on(scout),
        "a manual Move left AutoExplore on"
    );
    // And the scout obeys the order (ends up by the Town Center).
    for _ in 0..12_000 {
        sim.tick();
    }
    let d = dist(pos_of(&sim, scout), (tx.to_num::<f64>(), ty.to_num::<f64>()));
    assert!(d < 12.0, "the scout never rode home (still {d:.1} away)");
}

#[test]
fn a_queued_order_cancels_auto_explore() {
    let mut sim = Sim::new(11);
    let scout = starting_scout(&sim, OWNER_PLAYER);
    sim.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: true,
    });
    for _ in 0..300 {
        sim.tick();
    }
    assert!(sim.auto_explore_on(scout));
    sim.push_command(Command::Queue {
        entity: scout,
        order: QueuedOrder::Move {
            x: sim.entities()[scout as usize].pos_x,
            y: sim.entities()[scout as usize].pos_y,
        },
    });
    sim.tick();
    assert!(
        !sim.auto_explore_on(scout),
        "a queued order left AutoExplore on"
    );
}

#[test]
fn toggling_off_stops_the_scout_where_it_stands() {
    let mut sim = Sim::new(13);
    let scout = starting_scout(&sim, OWNER_PLAYER);
    sim.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: true,
    });
    for _ in 0..1_200 {
        sim.tick();
    }
    assert!(sim.auto_explore_on(scout));
    sim.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: false,
    });
    sim.tick();
    assert!(!sim.auto_explore_on(scout));
    let stopped_at = pos_of(&sim, scout);
    for _ in 0..1_200 {
        sim.tick();
    }
    let drift = dist(pos_of(&sim, scout), stopped_at);
    assert!(
        drift < 3.0,
        "the scout kept riding after toggle-off ({drift:.1} units)"
    );
}

#[test]
fn only_the_starting_scout_is_eligible() {
    let mut sim = Sim::new(17);
    let scout = starting_scout(&sim, OWNER_PLAYER);
    assert!(sim.can_auto_explore(scout));
    assert!(sim.is_starting_scout(scout));
    // Every other kind rejects the toggle: Town Center, villagers,
    // and the Gaia herd.
    for e in sim.entities().iter() {
        if e.id == scout {
            continue;
        }
        assert!(
            !sim.can_auto_explore(e.id),
            "entity {} ({:?}) must not be auto-explore eligible",
            e.id,
            e.kind
        );
        assert!(!sim.is_starting_scout(e.id));
    }
    let villager = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager)
        .map(|e| e.id)
        .expect("a villager");
    sim.push_command(Command::SetAutoExplore {
        entity: villager,
        enabled: true,
    });
    sim.tick();
    assert!(!sim.auto_explore_on(villager));

    // Both seats' starting scouts are eligible in a match.
    let m = Sim::new_match(19, skirmish_sim::MatchMode::TwoPlayer);
    let mine = starting_scout(&m, OWNER_PLAYER);
    let theirs = starting_scout(&m, OWNER_OPPONENT);
    assert!(m.can_auto_explore(mine));
    assert!(m.can_auto_explore(theirs));
    assert_ne!(mine, theirs);
}
