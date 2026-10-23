//! Deer pushing by proximity (AI-07's "or approaches" half,
//! 2026-10-22 — Study #3 §c.1, its top HIGH-impact finding).
//!
//! In DE, deer flee from a unit that wounds OR approaches them
//! (wiki, Hunt: regular huntables "flee when approached by any
//! non-Gaia unit and continue to do so if pursued"), which is what
//! lets a scout push deer toward the Town Center by pure
//! positioning — Hera spends minutes 1:44–5:30 of the studied
//! match doing exactly that, no shots fired. Before this package
//! the flee timer was set only on damage, so walking a scout
//! behind a deer did nothing.
//!
//! Standard start ids (Sim::new): 0 TC, 1–3 villagers, 4 Scout,
//! 5–8 near sheep, 9–12 far sheep, 13–14 boars, 15–20 deer.

use fixed::types::I16F16;
use skirmish_sim::{Command, Entity, Sim};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn dist(a: &Entity, b: &Entity) -> f64 {
    let dx = f(a.pos_x) - f(b.pos_x);
    let dy = f(a.pos_y) - f(b.pos_y);
    (dx * dx + dy * dy).sqrt()
}

fn displacement(sim: &Sim, id: u32, from: (f64, f64)) -> f64 {
    let e = ent(sim, id);
    ((f(e.pos_x) - from.0).powi(2) + (f(e.pos_y) - from.1).powi(2)).sqrt()
}

fn pos(sim: &Sim, id: u32) -> (f64, f64) {
    let e = ent(sim, id);
    (f(e.pos_x), f(e.pos_y))
}

#[test]
fn scout_pushes_deer_by_proximity_without_a_shot() {
    let mut sim = Sim::new(7);
    let deer_start = pos(&sim, 15);
    let deer_hp = ent(&sim, 15).hp;
    // Walk the Scout onto the deer's grazing spot. The Scout
    // never attacks (a plain Move; deer are never auto-targeted).
    let (dx, dy) = (ent(&sim, 15).pos_x, ent(&sim, 15).pos_y);
    sim.push_command(Command::Move { entity: 4, x: dx, y: dy });
    for _ in 0..6000 {
        sim.tick();
    }
    // The Scout is standing where the deer grazed…
    let scout = ent(&sim, 4);
    let scout_from_start =
        ((f(scout.pos_x) - deer_start.0).powi(2) + (f(scout.pos_y) - deer_start.1).powi(2)).sqrt();
    assert!(
        scout_from_start < 2.0,
        "scout never reached the deer's spot ({scout_from_start:.1} wu away)"
    );
    // …and the deer is gone from there, untouched: it fled the
    // approach itself, out of the push radius, at full HP.
    let deer = ent(&sim, 15);
    assert!(!deer.dead, "the deer died with nobody attacking it");
    assert_eq!(deer.hp, deer_hp, "the deer was damaged — the push must be shot-free");
    let moved = displacement(&sim, 15, deer_start);
    assert!(moved > 5.0, "deer barely moved from its grazing spot ({moved:.1} wu)");
    let separation = dist(ent(&sim, 15), ent(&sim, 4));
    assert!(
        separation > 8.0,
        "deer settled only {separation:.1} wu from the scout — still inside the push radius"
    );
}

#[test]
fn damage_flee_still_works() {
    let mut sim = Sim::new(7);
    let deer_start = pos(&sim, 15);
    let deer_hp = ent(&sim, 15).hp;
    // The Scout (faster than a deer) runs it down and hits it.
    sim.push_command(Command::Attack { entity: 4, target: 15 });
    let mut wounded = false;
    for _ in 0..12000 {
        sim.tick();
        if ent(&sim, 15).hp < deer_hp {
            wounded = true;
            break;
        }
    }
    assert!(wounded, "the scout never landed a hit on the deer");
    // Call the Scout off: from here the only thing that can move
    // the deer is the flee the wound started.
    sim.push_command(Command::Stop { entity: 4 });
    for _ in 0..900 {
        sim.tick();
    }
    let moved = displacement(&sim, 15, deer_start);
    assert!(
        moved > 4.0,
        "a wounded deer did not flee (moved {moved:.1} wu from its spot)"
    );
}

#[test]
fn deer_beside_an_idle_unit_settles_no_flee_loop() {
    let mut sim = Sim::new(7);
    // Push one deer off its spot, then leave the Scout standing
    // idle on that spot for minutes of game time.
    let (dx, dy) = (ent(&sim, 15).pos_x, ent(&sim, 15).pos_y);
    sim.push_command(Command::Move { entity: 4, x: dx, y: dy });
    for _ in 0..3000 {
        sim.tick();
    }
    // The deer must have settled: over a long quiet window it does
    // not move, and no flee window is active at the end — a unit
    // standing still nearby cannot keep it running forever (it
    // stepped out of the push radius once and grazes there).
    let settled_at = pos(&sim, 15);
    for _ in 0..2400 {
        sim.tick();
    }
    let drift = displacement(&sim, 15, settled_at);
    assert!(
        drift < 1.0,
        "deer kept moving beside an idle scout (drifted {drift:.1} wu in 2400 ticks)"
    );
    let deer = ent(&sim, 15);
    assert!(
        deer.flee_until <= sim.tick_count(),
        "deer still has an active flee window beside an idle unit (flee_until {} > tick {})",
        deer.flee_until,
        sim.tick_count()
    );
}

#[test]
fn boar_ignores_proximity_no_flee_no_aggro() {
    let mut sim = Sim::new(7);
    let boar_start = pos(&sim, 13);
    // Stand the Scout right beside the boar's post (offset, so
    // unit separation never shoves either of them). A boar is
    // passive until DAMAGED (AI-07's other half) — proximity must
    // not scare it off and must not aggro it either.
    let (bx, by) = (ent(&sim, 13).pos_x, ent(&sim, 13).pos_y);
    sim.push_command(Command::Move {
        entity: 4,
        x: bx + I16F16::from_num(4),
        y: by,
    });
    for _ in 0..1800 {
        sim.tick();
    }
    let boar = ent(&sim, 13);
    assert!(!boar.dead, "the boar died with nobody attacking it");
    assert_eq!(boar.hp, I16F16::from_num(75), "the boar took damage from mere proximity");
    let moved = displacement(&sim, 13, boar_start);
    assert!(moved < 0.5, "the boar left its post on proximity alone ({moved:.1} wu)");
    // Prove the proximity actually happened (else the test is
    // vacuous): the Scout is standing by the boar, unharmed.
    let separation = dist(ent(&sim, 13), ent(&sim, 4));
    assert!(separation < 6.0, "the scout never got near the boar ({separation:.1} wu)");
    assert_eq!(
        ent(&sim, 4).hp,
        I16F16::from_num(45),
        "the boar aggroed on proximity alone and hit the scout"
    );
}
