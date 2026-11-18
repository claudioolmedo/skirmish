//! MOV-03 tests (2026-11-17): no pushing — bump, stop, repath.
//!
//! The documented DE rule (sk-research sk-04, closed 2026-11-16;
//! Claudio's decision the same day: full DE target, phased —
//! repath first, displacement removal second): units bump into
//! others, stop and repath around; nobody is displaced by
//! another unit's passage. Sources: Klingers (Engineering
//! Director, Forgotten Empires), Meeting C++ 2025 — "Units bump
//! into others, stop and repath around. No pushing allowed";
//! official Update 153015 (herdables DO push villagers — the
//! one documented displacement exception — and a villager
//! trapped by enemy units cannot escape); official Update
//! 81058 (collision → a correct re-path, never a displacement).
//!
//! Phase 1 (this file's first test): the bump-stop-repath
//! machinery lands additively while displacement still exists —
//! the wait/retry half is staged by presetting the bump state
//! (the detection half fires naturally only once Phase 2
//! removes displacement; its tests arrive with that commit).

use crate::entity::{Entity, EntityKind, OWNER_PLAYER};
use crate::steer;
use crate::Sim;
use fixed::types::I16F16;

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn index_of(sim: &Sim, id: u32) -> usize {
    sim.entities().iter().position(|e| e.id == id).unwrap()
}

/// A clear land cell near the map centre: land domain unblocked
/// in the whole 5×5, so a staged scenario plays out in the open
/// (same helper shape as mov05_tests).
fn clear_land_spot(sim: &Sim) -> (I16F16, I16F16) {
    let g = sim.grid();
    for cy in 40..80 {
        for cx in 40..80 {
            let ok = (0..5).all(|dy| {
                (0..5).all(|dx| {
                    let (x, y) = (cx + dx, cy + dy);
                    !g.is_blocked(x, y) && !g.is_water(x, y) && !g.is_shallows(x, y)
                })
            });
            if ok {
                return g.cell_center(cx + 2, cy + 2);
            }
        }
    }
    panic!("seed 42: no clear land spot near the centre");
}

#[test]
fn bump_wait_holds_the_unit_and_the_retry_resumes_it() {
    // Phase 1: a unit whose repath found no route waits —
    // steering zeroed, order standing — and the deterministic
    // retry resumes it once a route exists. Staged by presetting
    // the bump state on a real order, then sealing the unit's
    // cell with a ring of tree cells so every repath fails on
    // terrain alone (the blockers here are the grid, not units:
    // the wait is the same).
    let mut sim = Sim::new(42);
    let c = clear_land_spot(&sim);
    sim.entities.push(Entity::spawned(
        9_501,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0,
        c.1,
    ));
    let idx = index_of(&sim, 9_501);
    let target = (c.0 + I16F16::from_num(30), c.1);
    assert!(sim.set_move_order(idx, target.0, target.1));
    // Preset the wait, one tick short of the retry schedule:
    // retries fire at blocked_ticks = WINDOW + k·INTERVAL.
    {
        let o = sim.entities[idx].order.as_mut().unwrap();
        o.bump_wait = true;
        o.blocked_ticks = steer::BUMP_WINDOW_TICKS + steer::BUMP_REPATH_INTERVAL - 1;
    }
    // Seal the cell: the 8 surrounding cells become tree cells.
    let (ccx, ccy) = sim.grid().world_to_cell(c.0, c.1);
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx != 0 || dy != 0 {
                sim.grid
                    .set_node_block((ccx as i32 + dx) as u32, (ccy as i32 + dy) as u32, true);
            }
        }
    }
    // 300 ticks sealed: several retries fire and fail; the unit
    // never moves and the order (and its wait) stands.
    for _ in 0..300 {
        sim.tick();
    }
    let e = ent(&sim, 9_501);
    assert_eq!(
        (e.pos_x, e.pos_y),
        (c.0, c.1),
        "a waiting unit moved from its cell"
    );
    let o = e.order.as_ref().expect("the order must stand while waiting");
    assert!(o.bump_wait, "the wait must persist while no route exists");
    // Unseal: the next retry (≤ INTERVAL ticks) finds the route,
    // clears the wait, and the unit walks out and arrives.
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx != 0 || dy != 0 {
                sim.grid
                    .set_node_block((ccx as i32 + dx) as u32, (ccy as i32 + dy) as u32, false);
            }
        }
    }
    let mut arrived = false;
    for _ in 0..2400 {
        sim.tick();
        if ent(&sim, 9_501).order.is_none() {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "the unit never resumed after the route reopened");
    let e = ent(&sim, 9_501);
    let d = steer::vec_len(e.pos_x - target.0, e.pos_y - target.1);
    assert!(
        d < I16F16::from_num(1.5),
        "resumed unit stopped {d} from its target"
    );
}


/// Run `ticks` ticks; return once `done` holds. Shared driver
/// for the scenario tests below.
fn run_until(sim: &mut Sim, ticks: u32, done: impl Fn(&Sim) -> bool) -> bool {
    for _ in 0..ticks {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

#[test]
fn a_standing_unit_is_never_displaced_and_the_traveller_repaths() {
    // The core MOV-03 scenario (sk-04 §2): a traveller ordered
    // straight through a standing unit. The standing unit's
    // position must stay BIT-IDENTICAL — under the old shove it
    // was displaced 12.6 world units on this exact run (probe
    // P2, 2026-11-17) — and the traveller must still arrive:
    // bump, stop, repath around.
    let mut sim = Sim::new(42);
    let c = clear_land_spot(&sim);
    let six = I16F16::from_num(6);
    sim.entities.push(Entity::spawned(
        9_511,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0 - six,
        c.1,
    ));
    sim.entities.push(Entity::spawned(
        9_512,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0,
        c.1,
    ));
    let ti = index_of(&sim, 9_511);
    assert!(sim.set_move_order(ti, c.0 + six, c.1));
    let arrived = run_until(&mut sim, 2400, |s| ent(s, 9_511).order.is_none());
    assert!(arrived, "traveller never arrived past the standing unit");
    let b = ent(&sim, 9_512);
    assert_eq!(
        (b.pos_x, b.pos_y),
        (c.0, c.1),
        "the standing unit was displaced (MOV-03 forbids it)"
    );
}

#[test]
fn head_on_units_pass_without_either_being_pushed() {
    // sk-04 §2 again, both moving: the pair meets, neither is
    // displaced off its own steering (their closest approach
    // stays at the pair's diameter), and both complete their
    // orders — by sliding and by the bump repath, never by
    // pushing through.
    let mut sim = Sim::new(42);
    let c = clear_land_spot(&sim);
    let six = I16F16::from_num(6);
    sim.entities.push(Entity::spawned(
        9_521,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0 - six,
        c.1,
    ));
    sim.entities.push(Entity::spawned(
        9_522,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0 + six,
        c.1,
    ));
    let (ai, bi) = (index_of(&sim, 9_521), index_of(&sim, 9_522));
    assert!(sim.set_move_order(ai, c.0 + six, c.1));
    assert!(sim.set_move_order(bi, c.0 - six, c.1));
    let mut min_d = i32::MAX;
    let mut both_done = false;
    for _ in 0..2400 {
        sim.tick();
        let (a, b) = (ent(&sim, 9_521), ent(&sim, 9_522));
        min_d = min_d.min(steer::vec_len(b.pos_x - a.pos_x, b.pos_y - a.pos_y).to_bits());
        if a.order.is_none() && b.order.is_none() {
            both_done = true;
            break;
        }
    }
    assert!(both_done, "the head-on pair never completed their orders");
    assert!(
        min_d >= 104_858 - 8_192,
        "the pair closed to {min_d} bits, inside the 1.6-unit foot diameter"
    );
    assert!(
        min_d <= 104_858 + 32_768,
        "the pair never met ({min_d} bits) — the scenario did not happen"
    );
}

#[test]
fn a_unit_ringed_by_standing_units_is_trapped_until_a_gap_opens() {
    // Update 153015's documented case: a villager trapped by
    // units cannot escape — nobody moves aside for him. The
    // ring members are orderless, which is also exactly the
    // state of a villager at work (order complete, task
    // running), so this pins "working villagers are not pushed
    // by military" with the same mechanism. When one ring
    // member dies, the gap opens and the trapped unit walks
    // out and completes its order.
    let mut sim = Sim::new(42);
    let c = clear_land_spot(&sim);
    sim.entities.push(Entity::spawned(
        9_531,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0,
        c.1,
    ));
    // 8 ring members at 1.3 world units (inside the 1.6
    // diameter: the trap is already closed at spawn).
    let r = I16F16::from_num(1.3);
    let diag = I16F16::from_bits(60_207); // 1.3 / sqrt(2)
    let ring: [(i32, i32); 8] = [
        (1, 0),
        (-1, 0),
        (0, 1),
        (0, -1),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ];
    let mut ring_ids = Vec::new();
    for (k, (dx, dy)) in ring.iter().enumerate() {
        let id = 9_540 + k as u32;
        let (ox, oy) = if dx.abs() + dy.abs() == 2 {
            (diag * I16F16::from_num(*dx), diag * I16F16::from_num(*dy))
        } else {
            (r * I16F16::from_num(*dx), r * I16F16::from_num(*dy))
        };
        sim.entities.push(Entity::spawned(
            id,
            EntityKind::Villager,
            OWNER_PLAYER,
            c.0 + ox,
            c.1 + oy,
        ));
        ring_ids.push((id, c.0 + ox, c.1 + oy));
    }
    let ti = index_of(&sim, 9_531);
    assert!(sim.set_move_order(ti, c.0 + I16F16::from_num(20), c.1));
    // 900 ticks penned: the traveller stays at the centre and
    // every ring member is bit-identical at its spawn.
    for _ in 0..900 {
        sim.tick();
    }
    let t = ent(&sim, 9_531);
    let d = steer::vec_len(t.pos_x - c.0, t.pos_y - c.1);
    assert!(
        d < I16F16::from_num(2.5),
        "the trapped unit escaped its ring (moved {d})"
    );
    assert!(t.order.is_some(), "the trapped unit's order was dropped");
    for (id, x, y) in &ring_ids {
        let e = ent(&sim, *id);
        assert_eq!(
            (e.pos_x, e.pos_y),
            (*x, *y),
            "ring member {id} was displaced from its post"
        );
    }
    // Kill the eastern side (E, NE, SE — ids 9_540, 9_544,
    // 9_545 in the ring table's order): the gap opens. One
    // member would not be enough — its neighbours stand 1.84
    // apart, and two 1.6-diameter discs leave no channel
    // between them; the eastern side is the honest gap.
    for id in [9_540, 9_544, 9_545] {
        let ei = index_of(&sim, id);
        sim.entities[ei].dead = true;
    }
    let escaped = run_until(&mut sim, 3600, |s| ent(s, 9_531).order.is_none());
    assert!(escaped, "the unit never walked out through the gap");
}

#[test]
fn sheep_displace_villagers_and_units_herd_sheep() {
    // The documented herdable system (Update 153015), both
    // halves. A sheep ordered through a standing villager
    // pushes him ahead of it (the pre-MOV-03 system displaced
    // him 8.8 world units on this run — probe P2; DE publishes
    // no magnitude, so the pin is > 2, not an equality). A
    // villager ordered through a standing sheep HERDS it: the
    // sheep yields out of his ground and is displaced ahead of
    // him (the yield — a sheep that never yielded would block
    // a one-cell chokepoint forever, contradicting the
    // documented collision outcome of Update 81058, a correct
    // path; measured in the wild: a Scout stalled 120,000
    // ticks behind one sheep in a forest corridor).
    let mut sim = Sim::new(42);
    let c = clear_land_spot(&sim);
    let six = I16F16::from_num(6);
    sim.entities.push(Entity::spawned(
        9_551,
        EntityKind::Sheep,
        OWNER_PLAYER,
        c.0 - six,
        c.1,
    ));
    sim.entities.push(Entity::spawned(
        9_552,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0,
        c.1,
    ));
    let si = index_of(&sim, 9_551);
    assert!(sim.set_move_order(si, c.0 + six, c.1));
    let arrived = run_until(&mut sim, 2400, |s| ent(s, 9_551).order.is_none());
    assert!(arrived, "the sheep never completed its crossing");
    let v = ent(&sim, 9_552);
    let disp = steer::vec_len(v.pos_x - c.0, v.pos_y - c.1);
    assert!(
        disp > I16F16::from_num(2),
        "the sheep displaced the villager by only {disp} (exception lost)"
    );

    // The mirror: villager through a standing sheep.
    let mut sim = Sim::new(42);
    let c = clear_land_spot(&sim);
    sim.entities.push(Entity::spawned(
        9_561,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0 - six,
        c.1,
    ));
    sim.entities.push(Entity::spawned(
        9_562,
        EntityKind::Sheep,
        OWNER_PLAYER,
        c.0,
        c.1,
    ));
    let vi = index_of(&sim, 9_561);
    assert!(sim.set_move_order(vi, c.0 + six, c.1));
    let arrived = run_until(&mut sim, 2400, |s| ent(s, 9_561).order.is_none());
    assert!(arrived, "the villager never arrived past the sheep");
    let sh = ent(&sim, 9_562);
    let disp = steer::vec_len(sh.pos_x - c.0, sh.pos_y - c.1);
    assert!(
        disp > I16F16::from_num(1),
        "the standing sheep was not herded (displaced only {disp})"
    );
    assert!(
        sh.pos_x > c.0,
        "the sheep yielded backward ({}) — herding pushes it ahead",
        sh.pos_x
    );
}

#[test]
fn same_formation_units_do_not_collide_with_each_other() {
    // sk-04 §2: "Units that are in the same formation do not
    // collide with each other at all in DE." Two villagers
    // carrying one formation stamp cross head-on THROUGH each
    // other — their closest approach is far inside the foot
    // diameter that stops an unstamped pair — and both arrive.
    let mut sim = Sim::new(42);
    let c = clear_land_spot(&sim);
    let six = I16F16::from_num(6);
    sim.entities.push(Entity::spawned(
        9_571,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0 - six,
        c.1,
    ));
    sim.entities.push(Entity::spawned(
        9_572,
        EntityKind::Villager,
        OWNER_PLAYER,
        c.0 + six,
        c.1,
    ));
    let (ai, bi) = (index_of(&sim, 9_571), index_of(&sim, 9_572));
    assert!(sim.set_move_order(ai, c.0 + six, c.1));
    assert!(sim.set_move_order(bi, c.0 - six, c.1));
    sim.entities[ai].order.as_mut().unwrap().formation_id = 77;
    sim.entities[bi].order.as_mut().unwrap().formation_id = 77;
    let mut min_d = i32::MAX;
    let mut both_done = false;
    for _ in 0..2400 {
        sim.tick();
        let (a, b) = (ent(&sim, 9_571), ent(&sim, 9_572));
        min_d = min_d.min(steer::vec_len(b.pos_x - a.pos_x, b.pos_y - a.pos_y).to_bits());
        if a.order.is_none() && b.order.is_none() {
            both_done = true;
            break;
        }
    }
    assert!(both_done, "the formation pair never completed their orders");
    assert!(
        min_d < 32_768,
        "the formation pair kept {min_d} bits apart — they collided after all"
    );
}

