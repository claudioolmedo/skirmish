//! Wolf regression suite (AI-08, 2026-10-22).
//!
//! DE's Grey Wolf (AoE wiki, fetched 2026-10-22): 25 HP, 3 melee,
//! no food — a kill leaves NOTHING behind. Wolves are aggressive
//! predators: unlike the boar (which only retaliates) a wolf
//! INITIATES, attacking the nearest eligible unit on sight —
//! Villagers above all — but it never hunts Scout Cavalry on
//! sight. Arabia placement (adopted, generator-symmetric): one
//! pack of 3 per start in the 26–34 tile band; the wiki publishes
//! no per-player wolf count, and the pack must never spawn next
//! to a starting TC.

use fixed::types::I16F16;
use skirmish_sim::{Command, EntityKind, MatchMode, Sim, OWNER_GAIA, OWNER_PLAYER};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &skirmish_sim::Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn alive(sim: &Sim, id: u32) -> bool {
    sim.entities().iter().any(|e| e.id == id && !e.dead)
}

fn wolves(sim: &Sim) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Wolf && e.owner == OWNER_GAIA)
        .map(|e| e.id)
        .collect()
}

fn first_of(sim: &Sim, kind: EntityKind, owner: u8) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == kind && e.owner == owner)
        .map(|e| e.id)
        .expect("entity exists")
}

fn dist_xy(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
}

#[test]
fn packs_stand_far_from_every_town_center() {
    // Sandbox = one start: one pack of 3. Match modes: two starts,
    // two packs. Every wolf at least 24 tiles (96 wu) from every TC.
    for (mode, want) in [(MatchMode::Sandbox, 3), (MatchMode::AiNormal, 6)] {
        let sim = Sim::with_mode(42, mode);
        let ws = wolves(&sim);
        assert_eq!(ws.len(), want, "wolf count in {mode:?}");
        let tcs: Vec<(f64, f64)> = sim
            .entities()
            .iter()
            .filter(|e| e.kind == EntityKind::TownCenter)
            .map(|e| (f(e.pos_x), f(e.pos_y)))
            .collect();
        assert!(!tcs.is_empty());
        for w in &ws {
            let e = ent(&sim, *w);
            assert_eq!(e.hp.to_num::<i32>(), 25, "DE wolf HP");
            for (tx, ty) in &tcs {
                let d = dist_xy(f(e.pos_x), f(e.pos_y), *tx, *ty);
                assert!(d >= 96.0, "wolf {w} only {d:.1} wu from a TC");
            }
        }
    }
}

#[test]
fn idle_wolves_hold_their_posts() {
    // Nothing comes near: over 60 s of sim time no wolf moves,
    // none dies, none starves into the starting area.
    let mut sim = Sim::with_mode(42, MatchMode::Sandbox);
    let posts: Vec<(u32, f64, f64)> = wolves(&sim)
        .iter()
        .map(|w| {
            let e = ent(&sim, *w);
            (*w, f(e.pos_x), f(e.pos_y))
        })
        .collect();
    for _ in 0..3600 {
        sim.tick();
    }
    for (w, px, py) in posts {
        assert!(alive(&sim, w), "idle wolf died");
        let e = ent(&sim, w);
        let d = dist_xy(f(e.pos_x), f(e.pos_y), px, py);
        assert!(d < 0.01, "idle wolf left its post ({d:.2} wu)");
    }
}

#[test]
fn wolf_hunts_down_a_villager_and_leaves_nothing() {
    let mut sim = Sim::with_mode(42, MatchMode::Sandbox);
    let wolf = wolves(&sim)[0];
    let post = ent(&sim, wolf);
    let (px, py) = (post.pos_x, post.pos_y);
    let vil = first_of(&sim, EntityKind::Villager, OWNER_PLAYER);
    // Walk the villager straight onto the wolf's post.
    sim.push_command(Command::Move { entity: vil, x: px, y: py });
    let nodes_before = sim.nodes().len();
    let mut chased = false;
    let mut killed = false;
    for _ in 0..7200 {
        sim.tick();
        let w = ent(&sim, wolf);
        if dist_xy(f(w.pos_x), f(w.pos_y), f(px), f(py)) > 8.0 {
            chased = true;
        }
        if ent(&sim, vil).dead {
            killed = true;
            break;
        }
    }
    assert!(killed, "the wolf never killed the villager who came to it");
    assert!(chased, "the wolf never left its post to chase");
    assert!(alive(&sim, wolf), "an unarmed villager must not kill the wolf");
    assert_eq!(
        sim.nodes().len(),
        nodes_before,
        "a wolf kill must leave no carcass node (wolves hold no food)"
    );
}

#[test]
fn wolf_ignores_a_scout_on_sight() {
    // DE: wolves do not attack Scout Cavalry on sight — the scout
    // can ride onto the post and stand there unmolested.
    let mut sim = Sim::with_mode(42, MatchMode::Sandbox);
    let wolf = wolves(&sim)[0];
    let post = ent(&sim, wolf);
    let (px, py) = (post.pos_x, post.pos_y);
    let scout = first_of(&sim, EntityKind::ScoutCavalry, OWNER_PLAYER);
    sim.push_command(Command::Move { entity: scout, x: px, y: py });
    let mut arrived = false;
    for _ in 0..3600 {
        sim.tick();
        let s = ent(&sim, scout);
        if dist_xy(f(s.pos_x), f(s.pos_y), f(px), f(py)) < 8.0 {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "scout never reached the wolf's post");
    for _ in 0..600 {
        sim.tick();
    }
    let s = ent(&sim, scout);
    assert!(!s.dead, "the wolf hunted a scout on sight");
    assert_eq!(s.hp.to_num::<i32>(), 45, "the wolf bit the scout");
    let w = ent(&sim, wolf);
    assert_eq!(w.hp.to_num::<i32>(), 25, "the wolf was hurt unprovoked");
}

#[test]
fn villagers_can_kill_a_wolf_and_it_drops_nothing() {
    let mut sim = Sim::with_mode(42, MatchMode::Sandbox);
    let wolf = wolves(&sim)[0];
    let vils: Vec<u32> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
        .take(3)
        .map(|e| e.id)
        .collect();
    assert_eq!(vils.len(), 3);
    for v in &vils {
        sim.push_command(Command::Attack { entity: *v, target: wolf });
    }
    let nodes_before = sim.nodes().len();
    let mut dead = false;
    for _ in 0..7200 {
        sim.tick();
        if ent(&sim, wolf).dead {
            dead = true;
            break;
        }
    }
    assert!(dead, "three villagers could not kill one wolf");
    assert_eq!(
        sim.nodes().len(),
        nodes_before,
        "a dead wolf must leave no carcass node"
    );
    assert!(
        vils.iter().filter(|v| alive(&sim, **v)).count() >= 2,
        "the wolf should not wipe out three villagers"
    );
}

#[test]
fn chased_prey_escapes_the_leash_and_the_wolf_comes_home() {
    // A scout that BITES a wolf is hunted (retaliation overrides
    // the on-sight exemption) — but the scout outruns the wolf,
    // the chase breaks at the leash, and the wolf walks home.
    let mut sim = Sim::with_mode(42, MatchMode::Sandbox);
    let wolf = wolves(&sim)[0];
    let post = ent(&sim, wolf);
    let (px, py) = (f(post.pos_x), f(post.pos_y));
    let scout = first_of(&sim, EntityKind::ScoutCavalry, OWNER_PLAYER);
    let tc = first_of(&sim, EntityKind::TownCenter, OWNER_PLAYER);
    let home = ent(&sim, tc);
    let (hx, hy) = (home.pos_x, home.pos_y);
    sim.push_command(Command::Attack { entity: scout, target: wolf });
    let mut bitten = false;
    for _ in 0..3600 {
        sim.tick();
        if ent(&sim, wolf).hp.to_num::<i32>() < 25 {
            bitten = true;
            break;
        }
    }
    assert!(bitten, "the scout never landed a hit on the wolf");
    // Run home.
    sim.push_command(Command::Move { entity: scout, x: hx, y: hy });
    let mut ranged_out = false;
    for _ in 0..7200 {
        sim.tick();
        let w = ent(&sim, wolf);
        if dist_xy(f(w.pos_x), f(w.pos_y), px, py) > 40.0 {
            ranged_out = true;
        }
        if ent(&sim, scout).dead {
            break;
        }
    }
    assert!(alive(&sim, scout), "the faster scout must escape the wolf");
    assert!(ranged_out, "the wolf never chased its attacker out");
    // Let it walk home.
    let mut home_again = false;
    for _ in 0..7200 {
        sim.tick();
        let w = ent(&sim, wolf);
        if dist_xy(f(w.pos_x), f(w.pos_y), px, py) < 10.0 {
            home_again = true;
            break;
        }
    }
    assert!(home_again, "the wolf never returned to its post");
}

#[test]
fn wolf_match_is_deterministic() {
    // Same seed + same lure script in two sims: identical hashes
    // throughout, wolf hunt included.
    let run = |sim: &mut Sim| {
        let wolf = wolves(sim)[0];
        let post = ent(sim, wolf);
        let (px, py) = (post.pos_x, post.pos_y);
        let vil = first_of(sim, EntityKind::Villager, OWNER_PLAYER);
        sim.push_command(Command::Move { entity: vil, x: px, y: py });
    };
    let mut a = Sim::with_mode(42, MatchMode::Sandbox);
    let mut b = Sim::with_mode(42, MatchMode::Sandbox);
    run(&mut a);
    run(&mut b);
    for step in 0..3600 {
        a.tick();
        b.tick();
        if step % 300 == 0 {
            assert_eq!(a.state_hash(), b.state_hash(), "diverged at step {step}");
        }
    }
    assert_eq!(a.state_hash(), b.state_hash(), "final hashes diverged");
}
