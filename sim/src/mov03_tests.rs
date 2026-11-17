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
