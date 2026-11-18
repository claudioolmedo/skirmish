//! A2 (bot-loop, 2026-11-18): re-issuing an Attack order naming
//! the attacker's CURRENT target must not restart its strike
//! state (`strike_tick` / `next_attack_tick` / `attack_engaged`).
//!
//! Measured defect (idlecount probe 3, handoff
//! `skirmish-study/handoffs/idlecount-build.md`): `apply_attack`
//! reset all three fields on every issue, so re-issuing every
//! 30 ticks — faster than the 41-tick CMB-04 strike delay —
//! postponed the strike forever: zero damage in 18,000 ticks.
//! A human re-clicking the same enemy is the ordinary case this
//! starved. The fix preserves the strike state on a same-target
//! re-issue (unit and building targets); a re-issue naming a
//! DIFFERENT target resets it exactly as before.
//!
//! Pins:
//! 1. Behavioural: re-issue every 30 ticks at the same sheep —
//!    the victim dies on the normal cadence (3 hits of 3 damage,
//!    120 ticks apart). Fails on the pristine tree (no damage
//!    at all in 18,000 ticks).
//! 2. State: in the reload window after a landed strike, a
//!    same-target re-issue keeps `attack_engaged` and the exact
//!    `next_attack_tick`, and schedules no new strike. Fails
//!    pristine (engaged cleared, reload zeroed, a fresh delayed
//!    strike scheduled).
//! 3. State: a re-issue at a DIFFERENT target still resets —
//!    the fix is same-target only.
//! 4. State: the building counterpart — a same-building
//!    `AttackBuilding` re-issue in the reload window keeps the
//!    strike state too.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, Entity, EntityKind, MatchMode, Sim, OWNER_OPPONENT, OWNER_PLAYER,
};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}
fn fx(v: f64) -> I16F16 {
    I16F16::from_num(v)
}
fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}
fn pos(sim: &Sim, id: u32) -> (f64, f64) {
    let e = ent(sim, id);
    (f(e.pos_x), f(e.pos_y))
}
fn find_id(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind && !e.dead)
        .unwrap()
        .id
}

/// Drive Scout 4 onto sheep 5 until the first strike lands, and
/// return the attacker's `next_attack_tick` in the reload window
/// that follows (engaged, no strike pending, reload in future).
fn scout_in_reload_window(sim: &mut Sim) -> u64 {
    sim.push_command(Command::Attack {
        entity: 4,
        target: 5,
    });
    let mut prev_hp = f(ent(sim, 5).hp);
    for _ in 0..9000 {
        sim.tick();
        let hp = f(ent(sim, 5).hp);
        if hp < prev_hp - 1e-9 {
            let a = ent(sim, 4);
            assert!(
                a.attack_engaged && a.strike_tick.is_none(),
                "staging: attacker not in the reload window after a landed strike: \
                 engaged={} strike_tick={:?}",
                a.attack_engaged, a.strike_tick
            );
            assert!(
                a.next_attack_tick > sim.tick_count(),
                "staging: reload tick {} not in the future (now {})",
                a.next_attack_tick,
                sim.tick_count()
            );
            return a.next_attack_tick;
        }
        prev_hp = hp;
    }
    panic!("staging: scout never landed a first strike on the sheep");
}

#[test]
fn reissue_same_target_every_30_ticks_kills_on_normal_cadence() {
    let mut sim = Sim::new(7);
    let (scout, sheep) = (4u32, 5u32);
    // Walk the scout adjacent first (plain move to the sheep),
    // so the cadence is measured on an ENGAGED fight, not travel
    // (the idlecount probe-3 staging).
    let sp = pos(&sim, sheep);
    sim.push_command(Command::Move {
        entity: scout,
        x: fx(sp.0),
        y: fx(sp.1),
    });
    for _ in 0..6000 {
        sim.tick();
        if ent(&sim, scout).order.is_none() {
            break;
        }
    }
    let mut events: Vec<(u64, f64, f64)> = Vec::new();
    let mut prev_hp = f(ent(&sim, sheep).hp);
    let mut kill = None;
    for t in 0..18000u64 {
        if t % 30 == 0 {
            sim.push_command(Command::Attack {
                entity: scout,
                target: sheep,
            });
        }
        sim.tick();
        let hp = f(ent(&sim, sheep).hp);
        if hp < prev_hp - 1e-9 {
            events.push((t + 1, prev_hp, hp));
            prev_hp = hp;
        }
        if ent(&sim, sheep).dead {
            kill = Some(t + 1);
            break;
        }
    }
    assert!(
        kill.is_some(),
        "sheep survived 18,000 ticks of 30-tick re-issues (events: {events:?})"
    );
    assert_eq!(events.len(), 3, "sheep must take exactly 3 hits: {events:?}");
    for (_, before, after) in &events {
        assert_eq!(before - after, 3.0, "each hit deals exactly 3: {events:?}");
    }
    let gaps: Vec<u64> = events.windows(2).map(|w| w[1].0 - w[0].0).collect();
    assert_eq!(gaps, vec![120, 120], "hit cadence must stay 120 ticks: {events:?}");
}

#[test]
fn reissue_same_target_preserves_strike_state_in_reload_window() {
    let mut sim = Sim::new(7);
    let next0 = scout_in_reload_window(&mut sim);
    sim.push_command(Command::Attack {
        entity: 4,
        target: 5,
    });
    sim.tick();
    let a = ent(&sim, 4);
    assert_eq!(a.attack_target, Some(5));
    assert!(a.attack_engaged, "same-target re-issue cleared attack_engaged");
    assert_eq!(
        a.next_attack_tick, next0,
        "same-target re-issue moved the reload tick"
    );
    assert_eq!(
        a.strike_tick, None,
        "same-target re-issue scheduled a fresh delayed strike"
    );
}

#[test]
fn reissue_different_target_still_resets_strike_state() {
    let mut sim = Sim::new(7);
    let _next0 = scout_in_reload_window(&mut sim);
    // Sheep 6 is a different (Gaia, orderable) target.
    sim.push_command(Command::Attack {
        entity: 4,
        target: 6,
    });
    sim.tick();
    let a = ent(&sim, 4);
    assert_eq!(a.attack_target, Some(6));
    assert!(
        !a.attack_engaged,
        "different-target re-issue must clear attack_engaged (fresh engagement)"
    );
    assert_eq!(
        a.next_attack_tick, 0,
        "different-target re-issue must reset the reload tick"
    );
}

#[test]
fn reissue_same_building_preserves_strike_state_in_reload_window() {
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let attacker = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let builder = find_id(&sim, OWNER_OPPONENT, EntityKind::Villager);
    let tc_pos = pos(&sim, tc);
    // Stage the opponent builder east of the player's TC and
    // have it build a House there (the idlecount probe staging).
    let site = (tc_pos.0 + 20.0, tc_pos.1);
    sim.push_command(Command::Move {
        entity: builder,
        x: fx(site.0 - 6.0),
        y: fx(site.1),
    });
    for _ in 0..9000 {
        sim.tick();
        if ent(&sim, builder).order.is_none() {
            break;
        }
    }
    sim.push_command(Command::Build {
        entity: builder,
        kind: BuildingKind::House,
        x: fx(site.0),
        y: fx(site.1),
    });
    let mut built = false;
    for _ in 0..30000 {
        sim.tick();
        if sim
            .buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::House && b.complete)
        {
            built = true;
            break;
        }
    }
    assert!(built, "staging: opponent house never completed");
    let house = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::House && b.complete)
        .unwrap()
        .id;
    let house_hp = |s: &Sim| {
        f(s.buildings()
            .iter()
            .find(|b| b.id == house)
            .map(|b| b.hp)
            .unwrap_or_else(|| fx(0.0)))
    };
    // Attack until the first strike lands, then read the reload
    // window exactly as in the unit-target pin.
    sim.push_command(Command::AttackBuilding {
        entity: attacker,
        building: house,
    });
    let mut prev_hp = house_hp(&sim);
    let mut next0 = None;
    for _ in 0..9000 {
        sim.tick();
        let hp = house_hp(&sim);
        if hp < prev_hp - 1e-9 {
            let a = ent(&sim, attacker);
            assert!(
                a.attack_engaged && a.strike_tick.is_none() && a.next_attack_tick > sim.tick_count(),
                "staging: building attacker not in the reload window"
            );
            next0 = Some(a.next_attack_tick);
            break;
        }
        prev_hp = hp;
    }
    let next0 = next0.expect("staging: scout never landed a strike on the house");
    sim.push_command(Command::AttackBuilding {
        entity: attacker,
        building: house,
    });
    sim.tick();
    let a = ent(&sim, attacker);
    assert_eq!(a.attack_building, Some(house));
    assert!(a.attack_engaged, "same-building re-issue cleared attack_engaged");
    assert_eq!(
        a.next_attack_tick, next0,
        "same-building re-issue moved the reload tick"
    );
    assert_eq!(
        a.strike_tick, None,
        "same-building re-issue scheduled a fresh delayed strike"
    );
}
