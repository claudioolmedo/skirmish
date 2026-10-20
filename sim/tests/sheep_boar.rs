//! Sheep + boar regression suite (2026-10-19, Claudio play-test).
//!
//! Sheep: the DE flow is approach -> capture (ownership flips by
//! proximity) -> select the sheep -> right-click -> it walks. This
//! suite walks a villager to a Gaia sheep in a live match sim,
//! asserts the capture, then issues the sheep a Move and asserts
//! it actually travels.
//!
//! Boar: both of a start's boars must exist, stand on dry open
//! ground, and sit in the DE-typical band around the Town Center
//! (mapgen picks 15–24 tiles; the assertion band leaves room for
//! the pick's cell rounding).

use fixed::types::I16F16;
use skirmish_sim::{Command, EntityKind, MatchMode, Sim};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &skirmish_sim::Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn dist(sim: &Sim, a: u32, b: u32) -> f64 {
    let (ea, eb) = (ent(sim, a), ent(sim, b));
    ((f(ea.pos_x) - f(eb.pos_x)).powi(2) + (f(ea.pos_y) - f(eb.pos_y)).powi(2)).sqrt()
}

#[test]
fn captured_sheep_takes_move_orders() {
    let mut sim = Sim::with_mode(42, MatchMode::Sandbox);
    // The human's first villager and the nearest Gaia sheep.
    let vil = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == 0)
        .map(|e| e.id)
        .expect("human villager");
    let sheep = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Sheep && e.owner == 1)
        .min_by_key(|e| {
            let v = ent(&sim, vil);
            ((f(e.pos_x) - f(v.pos_x)).powi(2) + (f(e.pos_y) - f(v.pos_y)).powi(2)) as i64
        })
        .map(|e| e.id)
        .expect("a Gaia sheep");
    // Walk the villager onto the sheep; capture is proximity-based
    // (<= 1 tile), evaluated in the combat step.
    let s = ent(&sim, sheep);
    let (sx, sy) = (s.pos_x, s.pos_y);
    sim.push_command(Command::Move { entity: vil, x: sx, y: sy });
    let mut captured = false;
    for _ in 0..1800 {
        sim.tick();
        if ent(&sim, sheep).owner == 0 {
            captured = true;
            break;
        }
    }
    assert!(captured, "sheep never captured (villager-sheep dist {})", dist(&sim, vil, sheep));
    // Order the sheep back toward the Town Center (open ground).
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == 0)
        .map(|e| e.id)
        .expect("human TC");
    let before = ent(&sim, sheep);
    let (bx, by) = (f(before.pos_x), f(before.pos_y));
    let t = ent(&sim, tc);
    sim.push_command(Command::Move { entity: sheep, x: t.pos_x, y: t.pos_y });
    for _ in 0..900 {
        sim.tick();
    }
    let after = ent(&sim, sheep);
    let moved = ((f(after.pos_x) - bx).powi(2) + (f(after.pos_y) - by).powi(2)).sqrt();
    assert!(
        moved > 6.0,
        "captured sheep barely moved after a Move order: {moved:.2} wu in 900 ticks"
    );
    // And the blocked-target case (the actual play-test break):
    // a Move aimed INTO a tree cell — the sheep's only order
    // type is the plain move, and the sim used to drop it
    // silently. It must now walk up to the tree and stop beside
    // it. (Before the fix this assertion fails with the sheep
    // standing exactly where it was.)
    let tree = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == skirmish_sim::NodeKind::Tree)
        .min_by_key(|n| {
            let s = ent(&sim, sheep);
            ((f(n.pos_x) - f(s.pos_x)).powi(2) + (f(n.pos_y) - f(s.pos_y)).powi(2)) as i64
        })
        .expect("a tree near the start");
    let (tx, ty) = (tree.pos_x, tree.pos_y);
    // Sanity: the tree's own cell really is pathing-blocked.
    let (tcx, tcy) = sim.grid().world_to_cell(tx, ty);
    assert!(sim.grid().is_blocked(tcx, tcy), "a standing tree blocks its cell");
    sim.push_command(Command::Move { entity: sheep, x: tx, y: ty });
    for _ in 0..1800 {
        sim.tick();
    }
    let s = ent(&sim, sheep);
    let near = ((f(s.pos_x) - f(tx)).powi(2) + (f(s.pos_y) - f(ty)).powi(2)).sqrt();
    assert!(
        near < 7.0,
        "sheep ordered onto a tree cell ended {near:.1} wu away — the order was dropped, not snapped"
    );
}

#[test]
fn boars_placed_on_dry_open_ground_in_band() {
    for seed in [1u64, 7, 42, 123, 2024, 55555] {
        let sim = Sim::with_mode(seed, MatchMode::Sandbox);
        let tc = sim
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::TownCenter && e.owner == 0)
            .expect("human TC");
        let (tcx, tcy) = (f(tc.pos_x), f(tc.pos_y));
        let boars: Vec<_> = sim
            .entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Boar)
            .collect();
        // Sandbox spawns only the human start's package: its 2 boars.
        assert_eq!(boars.len(), 2, "seed {seed}: expected the human start's 2 boars");
        // The human's pair: the two nearest the human TC.
        let mut by_dist: Vec<(f64, &skirmish_sim::Entity)> = boars
            .iter()
            .map(|b| {
                (
                    ((f(b.pos_x) - tcx).powi(2) + (f(b.pos_y) - tcy).powi(2)).sqrt(),
                    *b,
                )
            })
            .collect();
        by_dist.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for (d, b) in by_dist.iter() {
            let tiles = d / 4.0;
            assert!(
                (13.0..=26.0).contains(&tiles),
                "seed {seed}: boar at {tiles:.1} tiles from the TC, outside the 15-24 pick band (slack incl.)"
            );
            // Dry, unobstructed cell.
            let cx = ((f(b.pos_x) + 240.0) / 4.0).floor() as u32;
            let cy = ((f(b.pos_y) + 240.0) / 4.0).floor() as u32;
            assert!(!sim.grid().is_water(cx, cy), "seed {seed}: boar stands in water");
            assert!(!sim.grid().is_blocked(cx, cy), "seed {seed}: boar stands on a blocked cell");
        }
    }
}
