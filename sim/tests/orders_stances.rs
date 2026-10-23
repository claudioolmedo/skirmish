//! Orders & stances (CMB-09/10/11): unit stances, attack-move,
//! and patrol.
//!
//! Pins the package's contract against the documented DE
//! behaviour (AoE wiki "Unit stance" / "Hotkey", the official
//! forums' attack-move thread, the GameFAQs AoK guide — all
//! fetched 2026-10-22; see docs/MECHANICS-RULEBOOK.md CMB-09/10/11):
//!
//! - Defaults: every military unit is Aggressive, except each
//!   seat's starting Scout Cavalry, which stands at Stand Ground.
//! - Stance matrix: Aggressive acquires in LoS and chases without
//!   limit; Defensive acquires in LoS, chases only within its
//!   leash of the post, then returns; Stand Ground acquires only
//!   inside attack range and never chases; No Attack never
//!   acquires and never retaliates — while explicit orders
//!   override every stance.
//! - Attack-move engages what enters sight en route (CMB-08
//!   chain), then resumes and holds at the destination.
//! - Patrol loops between its points, engages en route, resumes
//!   the loop, and grows by appended points.
//! - Queue interplay: a queued attack-move executes when the
//!   unit goes idle; a patrol point for a patrolling unit appends
//!   at once; Stop and plain orders cancel standing orders;
//!   attack-move/patrol cancel AutoExplore like any manual order.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, Entity, EntityKind, MatchMode, QueuedOrder, Sim, Stance,
    DEFENSIVE_LEASH, OWNER_OPPONENT, OWNER_PLAYER,
};

fn fx(v: f64) -> I16F16 {
    I16F16::from_num(v)
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    &sim.entities()[id as usize]
}

fn pos(sim: &Sim, id: u32) -> (f64, f64) {
    let e = ent(sim, id);
    (e.pos_x.to_num::<f64>(), e.pos_y.to_num::<f64>())
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

fn unit_of(sim: &Sim, kind: EntityKind, owner: u8) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == kind && e.owner == owner && !e.dead)
        .map(|e| e.id)
        .unwrap_or_else(|| panic!("no living {kind:?} for owner {owner}"))
}

fn villager_of(sim: &Sim, owner: u8) -> u32 {
    unit_of(sim, EntityKind::Villager, owner)
}

fn scout_of(sim: &Sim, owner: u8) -> u32 {
    unit_of(sim, EntityKind::ScoutCavalry, owner)
}

fn cell_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn cell_of(sim: &Sim, x: I16F16, y: I16F16) -> (u32, u32) {
    sim.grid().world_to_cell(x, y)
}

/// The first anchor cell for `kind` that `entity` may build, in
/// expanding rings around a center cell (the combat.rs pattern).
fn anchor_near(sim: &Sim, entity: u32, kind: BuildingKind, center: (u32, u32)) -> (u32, u32) {
    for r in 1..=14i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (center.0 as i32 + dx, center.1 as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let (x, y) = cell_point(sim, ax, ay);
                if sim.can_build(entity, kind, x, y) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no buildable anchor near {center:?} for {kind:?}");
}

/// Build a House and a Barracks for `owner` (in parallel, two
/// villagers), then train `n` Militia at the Barracks. Returns
/// the militia ids in train order. The starting stockpile covers
/// it exactly (200 W = 175 Barracks + 25 House; n×50 F, n×20 G).
fn train_militia(sim: &mut Sim, owner: u8, n: usize) -> Vec<u32> {
    let tc = unit_of(sim, EntityKind::TownCenter, owner);
    let tce = ent(sim, tc);
    let tc_cell = cell_of(sim, tce.pos_x, tce.pos_y);
    let vils: Vec<u32> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager && e.owner == owner)
        .map(|e| e.id)
        .collect();
    let (hx, hy) = anchor_near(sim, vils[0], BuildingKind::House, tc_cell);
    let (bx, by) = anchor_near(sim, vils[1], BuildingKind::Barracks, tc_cell);
    for (v, kind, cx, cy) in [
        (vils[0], BuildingKind::House, hx, hy),
        (vils[1], BuildingKind::Barracks, bx, by),
    ] {
        let (x, y) = cell_point(sim, cx, cy);
        sim.push_command(Command::Build {
            entity: v,
            kind,
            x,
            y,
        });
    }
    let up = run_until(sim, 30_000, |s| {
        s.buildings()
            .iter()
            .filter(|b| b.owner == owner && b.complete)
            .count()
            >= 2
    });
    assert!(up, "house/barracks never completed for owner {owner}");
    let barracks = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Barracks && b.owner == owner)
        .unwrap()
        .id;
    let mut out = Vec::new();
    for _ in 0..n {
        sim.push_command(Command::TrainAt {
            building: barracks,
            kind: EntityKind::Militia,
        });
        let before: Vec<u32> = sim
            .entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Militia && e.owner == owner)
            .map(|e| e.id)
            .collect();
        let trained = run_until(sim, 10_000, |s| {
            s.entities()
                .iter()
                .filter(|e| e.kind == EntityKind::Militia && e.owner == owner)
                .count()
                > before.len()
        });
        assert!(trained, "militia never trained for owner {owner}");
        let id = sim
            .entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Militia && e.owner == owner && !before.contains(&e.id))
            .map(|e| e.id)
            .next()
            .unwrap();
        out.push(id);
    }
    out
}

/// The first candidate offset from `from` whose cell is not
/// blocked — scripted destinations must be standable ground, or
/// the sim (correctly) snaps/skips them and the script's geometry
/// breaks.
fn free_point(sim: &Sim, from: (f64, f64), offsets: &[(f64, f64)]) -> (f64, f64) {
    for &(dx, dy) in offsets {
        let p = (from.0 + dx, from.1 + dy);
        let (cx, cy) = sim.grid().world_to_cell(fx(p.0), fx(p.1));
        if !sim.grid().is_blocked(cx, cy) {
            return p;
        }
    }
    panic!("no free point near {from:?}");
}

/// The boar nearest to a world point.
fn boar_near(sim: &Sim, p: (f64, f64)) -> u32 {
    sim.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Boar && !e.dead)
        .min_by_key(|e| {
            let d = dist((e.pos_x.to_num::<f64>(), e.pos_y.to_num::<f64>()), p);
            (d * 1000.0) as i64
        })
        .map(|e| e.id)
        .expect("a boar")
}

fn two_player() -> Sim {
    Sim::with_mode(42, MatchMode::TwoPlayer)
}

// ---------------------------------------------------------------------
// Defaults and the SetStance command.

#[test]
fn stance_defaults_starting_scout_stand_ground_militia_aggressive() {
    let mut sim = two_player();
    assert_eq!(
        sim.stance_of(scout_of(&sim, OWNER_PLAYER)),
        Some(Stance::StandGround),
        "the player's starting scout must default to Stand Ground (CMB-09)"
    );
    assert_eq!(
        sim.stance_of(scout_of(&sim, OWNER_OPPONENT)),
        Some(Stance::StandGround),
        "the opponent's starting scout must default to Stand Ground too"
    );
    let militia = train_militia(&mut sim, OWNER_PLAYER, 1)[0];
    assert_eq!(
        sim.stance_of(militia),
        Some(Stance::Aggressive),
        "a trained military unit defaults to Aggressive (the wiki's default)"
    );
}

#[test]
fn set_stance_validates_and_persists_through_orders() {
    let mut sim = two_player();
    let scout = scout_of(&sim, OWNER_PLAYER);
    let vil = villager_of(&sim, OWNER_PLAYER);
    // Villagers, sheep and the dead hold no stance row.
    assert!(!sim.can_set_stance(vil));
    let sheep = unit_of(&sim, EntityKind::Sheep, skirmish_sim::OWNER_GAIA);
    assert!(!sim.can_set_stance(sheep));
    assert!(sim.can_set_stance(scout));
    sim.push_command(Command::SetStance {
        entity: vil,
        stance: Stance::NoAttack,
    });
    sim.push_command(Command::SetStance {
        entity: scout,
        stance: Stance::Defensive,
    });
    sim.tick();
    assert_eq!(sim.stance_of(scout), Some(Stance::Defensive));
    // A stance is a setting, not an order: a Move does not clear it.
    let (sx, sy) = pos(&sim, scout);
    sim.push_command(Command::Move {
        entity: scout,
        x: fx(sx + 20.0),
        y: fx(sy),
    });
    for _ in 0..600 {
        sim.tick();
    }
    assert_eq!(
        sim.stance_of(scout),
        Some(Stance::Defensive),
        "the stance must survive ordinary orders"
    );
    // Attack-move / patrol validation: military only.
    assert!(sim.can_attack_move(scout));
    assert!(!sim.can_attack_move(vil));
    assert!(!sim.can_attack_move(sheep));
}

// ---------------------------------------------------------------------
// The stance matrix against a boar (the chain's Gaia target).

/// Post `militia` on free ground near where it stands and wait
/// until it stands there. Returns the post.
fn post_militia(sim: &mut Sim, militia: u32) -> (f64, f64) {
    post_group(sim, &[militia])
}

/// Post a group of militia on one free point near the first of
/// them and wait until all stand there. (The lure scenarios use
/// groups: one Militia loses a 1v1 against a full-HP boar — the
/// boar out-damages it — so a lone posted unit dies mid-script
/// and the stance behaviour under test never plays out.)
fn post_group(sim: &mut Sim, militias: &[u32]) -> (f64, f64) {
    let mp = pos(sim, militias[0]);
    let post = free_point(sim, mp, &[(6.0, 0.0), (-6.0, 0.0), (0.0, 6.0), (0.0, -6.0), (12.0, 0.0)]);
    for &m in militias {
        sim.push_command(Command::Move {
            entity: m,
            x: fx(post.0),
            y: fx(post.1),
        });
    }
    let group: Vec<u32> = militias.to_vec();
    let arrived = run_until(sim, 20_000, |s| {
        group.iter().all(|&m| dist(pos(s, m), post) <= 1.5)
    });
    assert!(arrived, "the group never reached its post");
    post
}

/// Poke `boar` with the villager (locking its aggro), then send
/// the villager fleeing through `post` to a point 6 units beyond
/// it — close enough that the chase corridor genuinely crosses
/// the militia's position (a far endpoint lets pathfinding swing
/// the chase wide around obstacles), and the boar catches the
/// villager just past the post, in the militia's sight. Returns
/// the moment the flee order is issued: observation starts here,
/// because one villager strike may still be in flight.
fn lure_boar_past(sim: &mut Sim, vil: u32, boar: u32, post: (f64, f64)) {
    let bp = pos(sim, boar);
    sim.push_command(Command::Attack {
        entity: vil,
        target: boar,
    });
    let aggroed = run_until(sim, 10_000, |s| ent(s, boar).attack_target == Some(vil));
    assert!(aggroed, "the boar never aggroed the villager");
    let dir = (post.0 - bp.0, post.1 - bp.1);
    let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1.0);
    let (ux, uy) = (dir.0 / len, dir.1 / len);
    let raw = (post.0 + ux * 6.0, post.1 + uy * 6.0);
    // Snap the endpoint to free ground: along the chase axis
    // first (the corridor through the post is preserved), then
    // small perpendicular steps.
    let mut cands: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    for k in [2.0, 4.0, -2.0, 6.0, -4.0, 8.0, -6.0, 10.0] {
        cands.push((ux * k, uy * k));
    }
    for k in [2.0, -2.0, 4.0, -4.0] {
        cands.push((-uy * k, ux * k));
    }
    let through = free_point(sim, raw, &cands);
    sim.push_command(Command::Move {
        entity: vil,
        x: fx(through.0),
        y: fx(through.1),
    });
}

#[test]
fn stand_ground_holds_post_while_boar_passes() {
    let mut sim = two_player();
    let militias = train_militia(&mut sim, OWNER_PLAYER, 3);
    let vil = villager_of(&sim, OWNER_PLAYER);
    for &m in &militias {
        sim.push_command(Command::SetStance {
            entity: m,
            stance: Stance::StandGround,
        });
    }
    sim.tick(); // the stances land before the lure
    let post = post_group(&mut sim, &militias);
    let settled: Vec<(f64, f64)> = militias.iter().map(|&m| pos(&sim, m)).collect();
    let boar = boar_near(&sim, post);
    lure_boar_past(&mut sim, vil, boar, post);
    // Watch the whole chase from the flee order on: whenever the
    // boar is outside a militia's melee range that militia must
    // hold NO target; in range, the group must engage; and
    // through it all no militia may leave its post (separation
    // shoves of a few units are physics, not a chase — a chase
    // after the fleeing villager's line would show 6+).
    let mut saw_band = false;
    let mut engaged = false;
    for _ in 0..40_000 {
        sim.tick();
        for (i, &m) in militias.iter().enumerate() {
            let d = dist(pos(&sim, m), pos(&sim, boar));
            if d > 4.5 {
                assert!(
                    ent(&sim, m).attack_target.is_none(),
                    "Stand Ground held a target outside its range (d={d:.1})"
                );
            }
            if d < 12.0 && d > 4.5 {
                saw_band = true;
            }
            if ent(&sim, m).attack_target == Some(boar) {
                engaged = true;
            }
            let _ = i;
        }
        if ent(&sim, boar).dead {
            break;
        }
    }
    assert!(saw_band, "the lure never crossed the group's sight band");
    assert!(engaged, "the boar passed in range and was never engaged");
    for (i, &m) in militias.iter().enumerate() {
        assert!(
            !ent(&sim, m).dead,
            "a posted militia died — the stance script needs the group alive"
        );
        assert!(
            dist(pos(&sim, m), settled[i]) < 3.0,
            "Stand Ground left its post (moved {:.1})",
            dist(pos(&sim, m), settled[i])
        );
        let d_end = dist(pos(&sim, m), pos(&sim, boar));
        assert!(
            ent(&sim, m).attack_target.is_none() || d_end <= 3.5,
            "Stand Ground kept an out-of-range target at the end"
        );
    }
}

#[test]
fn defensive_leashes_and_returns_aggressive_does_not() {
    // The same script twice: once Defensive, once Aggressive.
    // The quarry is the FOE's villager, ordered from its own
    // start through the group's post to a stop 40 units beyond:
    // it survives the whole approach (a boar dies at the post
    // against three militia, ending the chase before the leash
    // can speak), the group acquires it as it enters LoS, and
    // then the stances diverge — Defensive breaks off at the
    // leash and walks back; Aggressive follows it to its stop
    // and kills it there. Returns (max distance any militia got
    // from the post, the group engaged at all, every militia
    // back at the post afterwards).
    let run = |stance: Stance| -> (f64, bool, bool) {
        let mut sim = two_player();
        let militias = train_militia(&mut sim, OWNER_PLAYER, 3);
        for &m in &militias {
            sim.push_command(Command::SetStance {
                entity: m,
                stance,
            });
        }
        sim.tick();
        let post = post_group(&mut sim, &militias);
        let quarry = villager_of(&sim, OWNER_OPPONENT);
        let qp = pos(&sim, quarry);
        let dir = (post.0 - qp.0, post.1 - qp.1);
        let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1.0);
        let (ux, uy) = (dir.0 / len, dir.1 / len);
        let raw = (post.0 + ux * 40.0, post.1 + uy * 40.0);
        let mut cands: Vec<(f64, f64)> = vec![(0.0, 0.0)];
        for k in [3.0, -3.0, 6.0, -6.0, 9.0] {
            cands.push((ux * k, uy * k));
        }
        for k in [3.0, -3.0, 6.0] {
            cands.push((-uy * k, ux * k));
        }
        let dest = free_point(&sim, raw, &cands);
        sim.push_command(Command::Move {
            entity: quarry,
            x: fx(dest.0),
            y: fx(dest.1),
        });
        let mut max_from_post = 0.0f64;
        let mut acquired = false;
        // Phase 1: the approach and the chase, until the quarry
        // is dead or the script has fully played out.
        for _ in 0..45_000 {
            sim.tick();
            for &m in &militias {
                if ent(&sim, m).attack_target == Some(quarry) {
                    acquired = true;
                }
                max_from_post = max_from_post.max(dist(pos(&sim, m), post));
            }
            if ent(&sim, quarry).dead {
                break;
            }
        }
        // Phase 2: with the chase over, do the militia come back
        // to the post (targets dropped)? A Defensive group must;
        // an Aggressive one stays out at the kill.
        let group: Vec<u32> = militias.clone();
        let back = run_until(&mut sim, 20_000, |s| {
            group
                .iter()
                .all(|&m| dist(pos(s, m), post) <= 2.5 && ent(s, m).attack_target.is_none())
        });
        (max_from_post, acquired, back)
    };
    let leash = DEFENSIVE_LEASH.to_num::<f64>();
    let (max_d, acquired_d, back_d) = run(Stance::Defensive);
    assert!(acquired_d, "the Defensive militia never engaged the boar");
    assert!(
        max_d > 4.0,
        "the Defensive militia never chased at all ({max_d:.1})"
    );
    assert!(
        max_d <= leash + 3.0,
        "the Defensive militia chased {max_d:.1} from its post — beyond its leash {leash}"
    );
    assert!(
        back_d,
        "the Defensive militia did not return to its post after the leash dropped its target"
    );
    let (max_a, acquired_a, _) = run(Stance::Aggressive);
    assert!(acquired_a, "the Aggressive militia never engaged the boar");
    assert!(
        max_a > leash + 6.0,
        "the Aggressive militia stopped at {max_a:.1} — it should chase without limit"
    );
}

#[test]
fn no_attack_never_engages_but_explicit_orders_still_work() {
    let mut sim = two_player();
    let militia = train_militia(&mut sim, OWNER_PLAYER, 1)[0];
    let vil = villager_of(&sim, OWNER_PLAYER);
    sim.push_command(Command::SetStance {
        entity: militia,
        stance: Stance::NoAttack,
    });
    sim.tick();
    let post = post_militia(&mut sim, militia);
    let boar = boar_near(&sim, post);
    lure_boar_past(&mut sim, vil, boar, post);
    // The boar chases the villager right past the militia: a No
    // Attack unit neither acquires nor strikes. Target-None on
    // every tick IS the complete proof: strikes only ever come
    // from a unit holding a target. (The boar's HP is not a
    // usable signal here — the fleeing villager itself retaliates
    // when the boar catches it, far past the post.)
    let mut saw_pass = false;
    for _ in 0..20_000 {
        sim.tick();
        assert!(
            ent(&sim, militia).attack_target.is_none(),
            "a No Attack unit acquired a target"
        );
        let d = dist(pos(&sim, boar), pos(&sim, militia));
        if d < 10.0 {
            saw_pass = true;
        }
        if saw_pass && d > 30.0 {
            break; // the pass is complete
        }
        if ent(&sim, boar).dead {
            break;
        }
    }
    assert!(saw_pass, "the lure never passed the No Attack militia");
    // But an explicit Attack order overrides the stance: the same
    // unit, ordered onto the boar, chases and fights it.
    let hp_now = ent(&sim, boar).hp;
    sim.push_command(Command::Attack {
        entity: militia,
        target: boar,
    });
    let engaged = run_until(&mut sim, 20_000, |s| ent(s, boar).hp < hp_now);
    assert!(engaged, "an explicit Attack order was ignored under No Attack stance");
}

// ---------------------------------------------------------------------
// The starting scouts: Stand Ground on both seats.

#[test]
fn starting_scouts_fight_at_point_blank_and_hold_position() {
    let mut sim = two_player();
    let mine = scout_of(&sim, OWNER_PLAYER);
    let foe = scout_of(&sim, OWNER_OPPONENT);
    let post = pos(&sim, mine);
    let foe_full = ent(&sim, foe).hp;
    // Approach along the ray from my Town Center through my
    // scout — the start area's cleared side.
    let tc = ent(&sim, unit_of(&sim, EntityKind::TownCenter, OWNER_PLAYER));
    let (tx, ty) = (tc.pos_x.to_num::<f64>(), tc.pos_y.to_num::<f64>());
    let dir = (post.0 - tx, post.1 - ty);
    let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1.0);
    let (ux, uy) = (dir.0 / len, dir.1 / len);
    // Ride the foe scout to a point a few units from mine: outside
    // melee range, inside LoS. Stand Ground must NOT react.
    let near = (post.0 + ux * 8.0, post.1 + uy * 8.0);
    sim.push_command(Command::Move {
        entity: foe,
        x: fx(near.0),
        y: fx(near.1),
    });
    let arrived = run_until(&mut sim, 30_000, |s| dist(pos(s, foe), near) <= 1.5);
    assert!(arrived, "the foe scout never arrived");
    for _ in 0..600 {
        sim.tick();
        assert!(
            ent(&sim, mine).attack_target.is_none(),
            "the starting scout acquired a target outside its range"
        );
        assert_eq!(pos(&sim, mine), post, "the starting scout left its post");
    }
    // Now step inside melee range: both scouts are Stand Ground,
    // so both answer — and neither leaves its ground to do it.
    let close = (post.0 + ux * 2.0, post.1 + uy * 2.0);
    sim.push_command(Command::Move {
        entity: foe,
        x: fx(close.0),
        y: fx(close.1),
    });
    let fought = run_until(&mut sim, 10_000, |s| ent(s, foe).hp < foe_full);
    assert!(fought, "the starting scout never fought an in-range enemy");
    assert!(
        dist(pos(&sim, mine), post) < 1.5,
        "the starting scout chased off its post"
    );
}

// ---------------------------------------------------------------------
// Attack-move.

#[test]
fn attack_move_engages_en_route_then_resumes_and_holds() {
    let mut sim = two_player();
    let militia = train_militia(&mut sim, OWNER_PLAYER, 3);
    let start = pos(&sim, militia[0]);
    let boar = boar_near(&sim, start);
    let bp = pos(&sim, boar);
    // Destination: straight past the boar, well beyond it.
    let dir = (bp.0 - start.0, bp.1 - start.1);
    let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt().max(1.0);
    // Past the boar, on standable ground (the sim would snap an
    // obstructed destination and the arrival check must measure
    // against where the units were actually sent).
    let dest = [24.0, 16.0, 10.0]
        .into_iter()
        .map(|k| (bp.0 + dir.0 / len * k, bp.1 + dir.1 / len * k))
        .find(|p| {
            let (cx, cy) = sim.grid().world_to_cell(fx(p.0), fx(p.1));
            !sim.grid().is_blocked(cx, cy)
        })
        .expect("no free destination past the boar");
    for &m in &militia {
        sim.push_command(Command::AttackMove {
            entity: m,
            x: fx(dest.0),
            y: fx(dest.1),
        });
    }
    sim.tick();
    assert!(
        militia.iter().all(|&m| sim.attack_moving(m)),
        "the attack-move standing order was not installed"
    );
    // Completion is judged on the SURVIVORS: a group walking
    // into a boar can lose a unit on the way in (the boar focuses
    // the first it meets), and in DE the rest still finish the
    // order. The dead keep their state frozen, standing order
    // included — they are not "still walking".
    let mut engaged = false;
    let done = run_until(&mut sim, 60_000, |s| {
        if militia.iter().any(|&m| ent(s, m).attack_target == Some(boar)) {
            engaged = true;
        }
        ent(s, boar).dead
            && militia
                .iter()
                .all(|&m| ent(s, m).dead || !s.attack_moving(m))
    });
    assert!(done, "the attack-move never completed");
    assert!(engaged, "the group never engaged the boar en route");
    assert!(ent(&sim, boar).dead, "the boar survived the attack-move");
    let survivors: Vec<u32> = militia
        .iter()
        .copied()
        .filter(|&m| !ent(&sim, m).dead)
        .collect();
    assert!(
        survivors.len() >= 2,
        "the group was mauled: only {} survived a single boar",
        survivors.len()
    );
    for &m in &survivors {
        assert!(
            dist(pos(&sim, m), dest) < 6.5,
            "a surviving unit did not hold at the attack-move destination"
        );
        assert!(ent(&sim, m).attack_target.is_none());
    }
}

#[test]
fn attack_move_and_patrol_cancel_auto_explore_and_each_other() {
    let mut sim = two_player();
    let scout = scout_of(&sim, OWNER_PLAYER);
    let (sx, sy) = pos(&sim, scout);
    let east = free_point(&sim, (sx, sy), &[(30.0, 0.0), (20.0, 0.0), (0.0, 30.0)]);
    let west = free_point(&sim, (sx, sy), &[(-30.0, 0.0), (-20.0, 0.0), (0.0, -30.0)]);
    sim.push_command(Command::SetAutoExplore {
        entity: scout,
        enabled: true,
    });
    sim.tick();
    assert!(sim.auto_explore_on(scout));
    // A patrol is a manual order: it takes the scout back.
    sim.push_command(Command::Patrol {
        entity: scout,
        x: fx(east.0),
        y: fx(east.1),
    });
    sim.tick();
    assert!(!sim.auto_explore_on(scout), "patrol did not cancel AutoExplore");
    assert!(sim.patrolling(scout));
    // An attack-move replaces the patrol.
    sim.push_command(Command::AttackMove {
        entity: scout,
        x: fx(west.0),
        y: fx(west.1),
    });
    sim.tick();
    assert!(!sim.patrolling(scout), "attack-move did not end the patrol");
    assert!(sim.attack_moving(scout));
    // A plain move ends the attack-move; a Stop ends a patrol.
    sim.push_command(Command::Move {
        entity: scout,
        x: fx(sx),
        y: fx(sy),
    });
    sim.tick();
    assert!(!sim.attack_moving(scout), "a plain move did not end the attack-move");
    sim.push_command(Command::Patrol {
        entity: scout,
        x: fx(east.0),
        y: fx(east.1),
    });
    sim.tick();
    assert!(sim.patrolling(scout));
    sim.push_command(Command::Stop { entity: scout });
    sim.tick();
    assert!(!sim.patrolling(scout), "Stop did not end the patrol");
}

// ---------------------------------------------------------------------
// Patrol.

#[test]
fn patrol_loops_engages_and_appends() {
    let mut sim = two_player();
    let militia = train_militia(&mut sim, OWNER_PLAYER, 3);
    let m = militia[0];
    // The engagement quarry is a lone FOE militia posted on
    // Stand Ground at the route's midpoint: a fair 3v1 the
    // patrol wins. (A boar is not usable here — in this sim's
    // balance a boar out-fights staggered militia arrivals; and
    // a fleeing deer can drag a chaser across the map.)
    let foe = train_militia(&mut sim, OWNER_OPPONENT, 1)[0];
    let a = pos(&sim, m);
    let b = free_point(&sim, a, &[(36.0, 0.0), (28.0, 0.0), (-36.0, 0.0), (0.0, 36.0), (0.0, -36.0)]);
    let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    sim.push_command(Command::Move {
        entity: foe,
        x: fx(mid.0),
        y: fx(mid.1),
    });
    let posted = run_until(&mut sim, 30_000, |s| dist(pos(s, foe), mid) < 2.0);
    assert!(posted, "the foe militia never reached the route midpoint");
    sim.push_command(Command::SetStance {
        entity: foe,
        stance: Stance::StandGround,
    });
    sim.tick();
    for &u in &militia {
        sim.push_command(Command::Patrol {
            entity: u,
            x: fx(b.0),
            y: fx(b.1),
        });
    }
    sim.tick();
    assert!(sim.patrolling(m));
    // The pennant route carries both legs (INF-09).
    let route = sim.order_route(m);
    assert_eq!(route.len(), 2, "the patrol route should hold its two legs");
    // One observation window for the whole loop story, judged on
    // ANY surviving patroller: the group walks to B, engages the
    // posted foe on the way, kills it, and comes back to A.
    let mut engaged = false;
    let mut reached_b = false;
    let mut back_a = false;
    let mut killed = false;
    for _ in 0..90_000 {
        sim.tick();
        if militia.iter().any(|&u| ent(&sim, u).attack_target == Some(foe)) {
            engaged = true;
        }
        if militia
            .iter()
            .any(|&u| !ent(&sim, u).dead && dist(pos(&sim, u), b) < 3.0)
        {
            reached_b = true;
        }
        if reached_b
            && militia
                .iter()
                .any(|&u| !ent(&sim, u).dead && dist(pos(&sim, u), a) < 3.0)
        {
            back_a = true;
        }
        if ent(&sim, foe).dead {
            killed = true;
            if reached_b && back_a {
                break;
            }
        }
    }
    assert!(reached_b, "the patrol never reached its far leg");
    assert!(engaged, "the patrol never engaged the posted foe on its loop");
    assert!(killed, "the patrol group never killed the posted foe");
    assert!(back_a, "the patrol never returned to its first leg");
    // Append a third point to a surviving patroller: the loop
    // grows, the patrol continues.
    let p = *militia
        .iter()
        .find(|&&u| !ent(&sim, u).dead && sim.patrolling(u))
        .expect("no surviving patroller");
    let c = free_point(&sim, a, &[(0.0, 30.0), (0.0, 22.0), (30.0, 30.0), (-30.0, 30.0)]);
    sim.push_command(Command::Patrol {
        entity: p,
        x: fx(c.0),
        y: fx(c.1),
    });
    sim.tick();
    assert_eq!(ent(&sim, p).patrol.len(), 3, "the patrol point was not appended");
    assert!(sim.patrolling(p));
    // After the kill the loop resumes: a leg endpoint is visited.
    let resumed = run_until(&mut sim, 40_000, |s| {
        dist(pos(s, p), a) < 3.5 || dist(pos(s, p), b) < 3.5 || dist(pos(s, p), c) < 3.5
    });
    assert!(resumed, "the patrol did not resume its loop after the fight");
    assert!(sim.patrolling(p));
}

// ---------------------------------------------------------------------
// Queue interplay.

#[test]
fn queued_attack_move_runs_after_the_current_order() {
    let mut sim = two_player();
    let m = train_militia(&mut sim, OWNER_PLAYER, 1)[0];
    let start = pos(&sim, m);
    let waypoint = free_point(&sim, start, &[(40.0, 0.0), (32.0, 0.0), (-40.0, 0.0)]);
    let dest = free_point(&sim, waypoint, &[(0.0, 30.0), (0.0, -30.0), (30.0, 0.0)]);
    sim.push_command(Command::Move {
        entity: m,
        x: fx(waypoint.0),
        y: fx(waypoint.1),
    });
    sim.push_command(Command::Queue {
        entity: m,
        order: QueuedOrder::AttackMove {
            x: fx(dest.0),
            y: fx(dest.1),
        },
    });
    sim.tick();
    assert!(!sim.attack_moving(m), "the queued attack-move started early");
    let started = run_until(&mut sim, 40_000, |s| s.attack_moving(m));
    assert!(started, "the queued attack-move never executed");
    let arrived = run_until(&mut sim, 40_000, |s| {
        !s.attack_moving(m) && dist(pos(s, m), dest) < 2.0
    });
    assert!(arrived, "the queued attack-move never completed at its destination");
}

#[test]
fn queued_patrol_point_appends_to_an_active_patrol_at_once() {
    let mut sim = two_player();
    let m = train_militia(&mut sim, OWNER_PLAYER, 1)[0];
    let a = pos(&sim, m);
    sim.push_command(Command::Patrol {
        entity: m,
        x: fx(a.0 + 30.0),
        y: fx(a.1),
    });
    sim.tick();
    assert_eq!(ent(&sim, m).patrol.len(), 2);
    // A QUEUED patrol point on a patrolling unit cannot wait
    // behind an endless loop — it appends immediately.
    sim.push_command(Command::Queue {
        entity: m,
        order: QueuedOrder::Patrol {
            x: fx(a.0 + 30.0),
            y: fx(a.1 + 20.0),
        },
    });
    sim.tick();
    assert_eq!(
        ent(&sim, m).patrol.len(),
        3,
        "the queued patrol point did not append to the loop"
    );
    assert!(
        ent(&sim, m).order_queue.is_empty(),
        "the patrol point was queued behind the endless loop"
    );
}

// ---------------------------------------------------------------------
// Determinism: the whole package is hashed state.

#[test]
fn orders_and_stances_replay_bit_identically() {
    let script = |sim: &mut Sim| {
        let m = train_militia(sim, OWNER_PLAYER, 1)[0];
        let p = pos(sim, m);
        sim.push_command(Command::SetStance {
            entity: m,
            stance: Stance::Defensive,
        });
        sim.push_command(Command::Patrol {
            entity: m,
            x: fx(p.0 + 30.0),
            y: fx(p.1 + 10.0),
        });
        let scout = scout_of(sim, OWNER_PLAYER);
        let sp = pos(sim, scout);
        sim.push_command(Command::AttackMove {
            entity: scout,
            x: fx(sp.0 - 25.0),
            y: fx(sp.1),
        });
        m
    };
    let run = || -> Vec<u64> {
        let mut sim = two_player();
        script(&mut sim);
        (0..6_000).map(|_| sim.tick()).collect()
    };
    assert_eq!(run(), run(), "the orders package broke replay determinism");
}




