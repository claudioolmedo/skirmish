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

// --- Herdables done properly (2026-10-20, Claudio play-test) -----
// The live report: sheep "don't move on their own", take no
// visible owner, and can't be herded. The sim half of that: Gaia
// sheep never wandered (animal AI had no sheep arm), and an owned
// sheep could never be stolen. Both are DE rules (wiki, Sheep /
// Herdable animal / Gaia) and are pinned here.

#[test]
fn gaia_sheep_wander_on_their_own_deterministically() {
    let mut a = Sim::with_mode(42, MatchMode::Sandbox);
    let mut b = Sim::with_mode(42, MatchMode::Sandbox);
    let spawn: Vec<(u32, f64, f64)> = a
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Sheep)
        .map(|e| (e.id, f(e.pos_x), f(e.pos_y)))
        .collect();
    assert!(!spawn.is_empty(), "the standard start has sheep");
    // Two minutes of game time, hash-compared every 10 seconds:
    // the wander draws from the seeded RNG in entity order, so the
    // two instances must stay bit-identical throughout.
    for step in 0..12 {
        for _ in 0..600 {
            a.tick();
            b.tick();
        }
        assert_eq!(a.state_hash(), b.state_hash(), "wander diverged at step {step}");
    }
    let mut max_moved = 0.0f64;
    for (id, sx, sy) in &spawn {
        let e = ent(&a, *id);
        let d = ((f(e.pos_x) - sx).powi(2) + (f(e.pos_y) - sy).powi(2)).sqrt();
        max_moved = max_moved.max(d);
        assert!(
            d < 16.0,
            "sheep {id} drifted {d:.1} wu from its post — the wander leash failed"
        );
    }
    assert!(
        max_moved > 2.0,
        "no Gaia sheep moved on its own in 7200 ticks (max {max_moved:.2} wu)"
    );
}

#[test]
fn owned_sheep_hold_position_until_ordered() {
    let mut sim = Sim::with_mode(42, MatchMode::Sandbox);
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
    assert!(captured, "sheep never captured");
    // Send the villager home; an owned sheep must then simply
    // stand for a full minute — wander is a Gaia behavior, and a
    // herded sheep that drifts off is the complaint inverted.
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == 0)
        .map(|e| e.id)
        .expect("human TC");
    let t = ent(&sim, tc);
    let (tx, ty) = (t.pos_x, t.pos_y);
    sim.push_command(Command::Move { entity: vil, x: tx, y: ty });
    let before = ent(&sim, sheep);
    let (bx, by) = (f(before.pos_x), f(before.pos_y));
    for _ in 0..3600 {
        sim.tick();
    }
    let after = ent(&sim, sheep);
    let d = ((f(after.pos_x) - bx).powi(2) + (f(after.pos_y) - by).powi(2)).sqrt();
    assert_eq!(after.owner, 0, "the sheep left its owner with no enemy near");
    assert!(d < 1.5, "owned sheep drifted {d:.2} wu on its own in 3600 ticks");
}

#[test]
fn owned_sheep_left_alone_can_be_stolen() {
    use skirmish_sim::OWNER_OPPONENT;
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
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
    assert!(captured, "sheep never captured by the human");
    // The whole point of the steal rule: the sheep is only held
    // while friendly units stand near. Send the villager home and
    // confirm it is well outside capture range (1 tile = 4 wu).
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == 0)
        .map(|e| e.id)
        .expect("human TC");
    let t = ent(&sim, tc);
    let (tx, ty) = (t.pos_x, t.pos_y);
    sim.push_command(Command::Move { entity: vil, x: tx, y: ty });
    for _ in 0..1800 {
        sim.tick();
    }
    assert!(
        dist(&sim, vil, sheep) > 8.0,
        "villager still within guard range of the sheep ({} wu)",
        dist(&sim, vil, sheep)
    );
    assert_eq!(ent(&sim, sheep).owner, 0, "sheep flipped with no enemy near");
    // The enemy Scout crosses the map to the lone sheep: with no
    // friendly unit nearby, ownership must transfer (wiki, Sheep:
    // "if no friendly units are nearby and another team's unit
    // happens to pass by, the Sheep will fall under the control of
    // that player").
    let scout = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::ScoutCavalry && e.owner == OWNER_OPPONENT)
        .map(|e| e.id)
        .expect("opponent scout");
    let s = ent(&sim, sheep);
    let (sx, sy) = (s.pos_x, s.pos_y);
    sim.push_command(Command::Move { entity: scout, x: sx, y: sy });
    let mut stolen = false;
    for _ in 0..60_000 {
        sim.tick();
        if ent(&sim, sheep).owner == OWNER_OPPONENT {
            stolen = true;
            break;
        }
    }
    assert!(stolen, "enemy scout reached the lone sheep but never stole it");
}

#[test]
fn owned_sheep_grants_two_tiles_of_sight() {
    let mut sim = Sim::with_mode(42, MatchMode::Sandbox);
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
    assert!(captured, "sheep never captured");
    // Park the sheep ten tiles east of the Town Center — far
    // beyond every other unit's sight (villager 4, Scout 6, TC 6)
    // — and send the villager home so only the sheep sees there.
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == 0)
        .map(|e| e.id)
        .expect("human TC");
    let t = ent(&sim, tc);
    let (tx, ty) = (t.pos_x, t.pos_y);
    sim.push_command(Command::Move {
        entity: sheep,
        x: tx + I16F16::from_num(40),
        y: ty,
    });
    sim.push_command(Command::Move { entity: vil, x: tx, y: ty });
    for _ in 0..3600 {
        sim.tick();
    }
    let d_tc = dist(&sim, sheep, tc);
    assert!(
        d_tc > 32.0,
        "sheep only reached {d_tc:.1} wu from the TC — the probe cells would be covered by the TC's own sight"
    );
    let s = ent(&sim, sheep);
    let (cx, cy) = sim.grid().world_to_cell(s.pos_x, s.pos_y);
    assert!(
        sim.cell_visible_to(0, cx + 2, cy),
        "cell 2 tiles from an owned sheep is not visible — the sheep grants no (or less) sight"
    );
    assert!(
        !sim.cell_visible_to(0, cx + 4, cy),
        "cell 4 tiles from an owned sheep is visible — sheep sight exceeds 2 tiles"
    );
}
