//! A2-ground (bot-loop, 2026-11-18): re-issuing an AttackGround
//! order naming the point ALREADY under bombardment must not
//! restart the attacker's strike state (`strike_tick` /
//! `next_attack_tick` / `attack_engaged`) — the ground twin of
//! the A2 defect fixed for `apply_attack` / `issue_attack_building`.
//!
//! Measured on the pristine tree (probe, this package): a
//! Mangonel staged in its firing band fires 10 stones in 3,600
//! ticks on its 360-tick reload. Re-issuing the identical
//! AttackGround command every 30 ticks fired 120 stones in the
//! same span (every 100 ticks: 36) — `apply_attack_ground` reset
//! the strike state on every issue, wiping the reload window, so
//! a player re-clicking the same ground point (a common human
//! habit) multiplied the weapon's rate of fire ~12x. The mirror
//! image of A2: there the 41-tick strike delay starved the fight;
//! here the Mangonel's zero attack delay makes the reset bypass
//! the reload instead. The fix preserves the strike state on a
//! same-point re-issue; a different point resets it as before.
//!
//! Pins:
//! 1. Behavioural: re-issue every 30 ticks at the same point —
//!    the stone count over 3,600 ticks equals the no-reissue
//!    baseline (10). Fails pristine (120).
//! 2. State: in the reload window after a released stone, a
//!    same-point re-issue keeps `attack_engaged` and the exact
//!    `next_attack_tick`, and schedules no new strike. Fails
//!    pristine (engaged cleared, reload zeroed, fresh strike
//!    scheduled).
//! 3. State: a re-issue at a DIFFERENT point still resets —
//!    the fix is same-point only. Passes on both trees.

use crate::entity::{Entity, EntityKind, OWNER_PLAYER};
use crate::{Command, Sim};
use fixed::types::I16F16;

const MANGO_ID: u32 = 990;

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

/// A Mangonel staged at cell (60, 66) — six tiles south of the
/// (60, 60) impact cell, inside its [3, 7] band (the P4b staging).
fn staged(sim: &mut Sim) -> (I16F16, I16F16) {
    let (x, y) = sim.grid().cell_center(60, 66);
    sim.entities
        .push(Entity::spawned(MANGO_ID, EntityKind::Mangonel, OWNER_PLAYER, x, y));
    sim.grid().cell_center(60, 60)
}

/// Count stone spawns over `ticks` ticks; when `reissue_every`
/// is Some(n), re-issue the identical AttackGround command every
/// n ticks. Spawns are positive deltas of the projectile list
/// (stones overlap in flight, so an empty->non-empty edge
/// counter would undercount a fast cadence).
fn count_stones(reissue_every: Option<u64>, ticks: u64) -> usize {
    let mut sim = Sim::new(7);
    let (px, py) = staged(&mut sim);
    sim.push_command(Command::AttackGround { entity: MANGO_ID, x: px, y: py });
    let mut stones = 0usize;
    let mut prev_len = 0usize;
    for t in 0..ticks {
        sim.tick();
        if let Some(n) = reissue_every {
            if t > 0 && t % n == 0 {
                sim.push_command(Command::AttackGround { entity: MANGO_ID, x: px, y: py });
            }
        }
        let len = sim.projectiles().len();
        if len > prev_len {
            stones += len - prev_len;
        }
        prev_len = len;
    }
    stones
}

#[test]
fn a2ground_reissue_same_point_every_30_ticks_keeps_the_reload_cadence() {
    let baseline = count_stones(None, 3_600);
    assert_eq!(baseline, 10, "staging: baseline cadence off (reload 360)");
    let reissued = count_stones(Some(30), 3_600);
    assert_eq!(
        reissued, baseline,
        "same-point re-issues every 30 ticks changed the cadence: \
         {reissued} stones vs baseline {baseline}"
    );
}

/// Drive the staged Mangonel to its first release and return
/// (sim, point, next_attack_tick) in the reload window that
/// follows (engaged, no strike pending, reload in future).
fn mangonel_in_reload_window() -> (Sim, (I16F16, I16F16), u64) {
    let mut sim = Sim::new(7);
    let (px, py) = staged(&mut sim);
    sim.push_command(Command::AttackGround { entity: MANGO_ID, x: px, y: py });
    for _ in 0..600 {
        sim.tick();
        let e = ent(&sim, MANGO_ID);
        if e.attack_engaged {
            let (strike, next) = (e.strike_tick, e.next_attack_tick);
            assert!(
                strike.is_none() && next > sim.tick,
                "staging: not a clean reload window: strike={strike:?} next={next} now={}",
                sim.tick
            );
            return (sim, (px, py), next);
        }
    }
    panic!("staging: mangonel never released a first stone");
}

#[test]
fn a2ground_reissue_same_point_preserves_strike_state_in_reload_window() {
    let (mut sim, (px, py), saved_next) = mangonel_in_reload_window();
    sim.push_command(Command::AttackGround { entity: MANGO_ID, x: px, y: py });
    sim.tick();
    let e = ent(&sim, MANGO_ID);
    assert!(e.attack_engaged, "same-point re-issue cleared attack_engaged");
    assert_eq!(
        e.next_attack_tick, saved_next,
        "same-point re-issue moved the reload tick"
    );
    assert_eq!(
        e.strike_tick, None,
        "same-point re-issue scheduled a fresh strike inside the reload"
    );
    assert_eq!(e.attack_point, Some((px, py)));
}

#[test]
fn a2ground_reissue_different_point_still_resets_strike_state() {
    let (mut sim, _point, _saved_next) = mangonel_in_reload_window();
    // A different point, still inside the band from (60, 66).
    let (qx, qy) = sim.grid().cell_center(60, 61);
    sim.push_command(Command::AttackGround { entity: MANGO_ID, x: qx, y: qy });
    sim.tick();
    let e = ent(&sim, MANGO_ID);
    assert_eq!(e.attack_point, Some((qx, qy)));
    assert!(
        !e.attack_engaged,
        "different-point re-issue must reset attack_engaged"
    );
    assert_eq!(
        e.next_attack_tick, 0,
        "different-point re-issue must reset the reload tick"
    );
}
