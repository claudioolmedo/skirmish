//! FIX #2b probe (bot-loop runs #417–#419) — auxiliary harness.
//!
//! Live defect: an enemy unit hidden behind/under the player's
//! Town Center (visible only as its red INF-12 outline) drains
//! the TC ~0.5 HP/s. Left-click on the outline selects it
//! (FIX #2, works live), but a right-click attack order on the
//! outline degrades: the ordered Scout/Militia walks to the
//! outline point and stands IDLE, the enemy's HP never moves.
//! In run #415 one such order DID kill the hidden unit, so the
//! failure is position/context dependent.
//!
//! This harness stages the sim half of that geometry with the
//! public command API only: the opponent Scout is walked to a
//! staging point on a chosen side of the player's TC, ordered
//! to attack the TC (so it settles exactly where attack_step
//! stops a TC attacker — within MELEE_REACH of the TC centre,
//! under the TC sprite), and once the TC is verifiably draining
//! the player's Scout is staged on a chosen approach side and
//! ordered to attack the hidden unit. Per case we measure:
//! does the attacker keep its attack target, close, deal
//! damage, and kill?

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

fn find_id(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind && !e.dead)
        .unwrap()
        .id
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn pos(sim: &Sim, id: u32) -> (f64, f64) {
    let e = ent(sim, id);
    (f(e.pos_x), f(e.pos_y))
}

struct CaseResult {
    name: String,
    victim_settled: (f64, f64),
    victim_cell: (u32, u32),
    victim_cell_blocked: bool,
    tc_drained: f64,
    attacker_final_dist: f64,
    attacker_target_kept: bool,
    victim_hp_end: f64,
    victim_dead: bool,
    ticks_used: u64,
}

fn run_case(seed: u64, name: &str, victim_stage: (f64, f64), attacker_stage: (f64, f64)) -> CaseResult {
    let mut sim = Sim::with_mode(seed, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let victim = find_id(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    let attacker = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let tc_pos = pos(&sim, tc);
    let tc_hp0 = f(ent(&sim, tc).hp);

    // Phase 1: walk the victim to its staging point (relative to
    // the TC centre), then order it onto the TC.
    sim.push_command(Command::Move {
        entity: victim,
        x: fx(tc_pos.0 + victim_stage.0),
        y: fx(tc_pos.1 + victim_stage.1),
    });
    for _ in 0..6000 {
        sim.tick();
        if ent(&sim, victim).order.is_none() {
            break;
        }
    }
    sim.push_command(Command::Attack {
        entity: victim,
        target: tc,
    });
    // Wait until the TC is verifiably draining (victim engaged
    // and standing at its attack post).
    let mut drained = 0.0;
    for _ in 0..12000 {
        sim.tick();
        drained = tc_hp0 - f(ent(&sim, tc).hp);
        if drained >= 3.0 {
            break;
        }
    }
    assert!(
        drained >= 3.0,
        "{name}: victim never engaged the TC (drained {drained})"
    );
    let victim_settled = pos(&sim, victim);
    let vcell = sim
        .grid()
        .world_to_cell(fx(victim_settled.0), fx(victim_settled.1));
    let victim_cell_blocked = sim.grid().is_blocked(vcell.0, vcell.1);

    // Phase 2: stage the attacker on its approach side and wait
    // for it to arrive and idle.
    sim.push_command(Command::Move {
        entity: attacker,
        x: fx(tc_pos.0 + attacker_stage.0),
        y: fx(tc_pos.1 + attacker_stage.1),
    });
    for _ in 0..6000 {
        sim.tick();
        if ent(&sim, attacker).order.is_none() {
            break;
        }
    }

    // Phase 3: the counter-attack order — the live right-click.
    sim.push_command(Command::Attack {
        entity: attacker,
        target: victim,
    });
    let mut ticks_used = 0u64;
    let mut target_kept = true;
    for t in 0..9000u64 {
        sim.tick();
        ticks_used = t + 1;
        let a = ent(&sim, attacker);
        if a.attack_target != Some(victim) && !ent(&sim, victim).dead {
            target_kept = false;
            break;
        }
        if ent(&sim, victim).dead {
            break;
        }
    }
    let v = ent(&sim, victim);
    CaseResult {
        name: name.to_string(),
        victim_settled,
        victim_cell: vcell,
        victim_cell_blocked,
        tc_drained: drained,
        attacker_final_dist: dist(pos(&sim, attacker), pos(&sim, victim)),
        attacker_target_kept: target_kept,
        victim_hp_end: f(v.hp),
        victim_dead: v.dead,
        ticks_used,
    }
}

#[test]
fn probe_hidden_tc_attacker_counterattack() {
    // Victim approach sides (staging offsets from the TC centre)
    // x attacker approach sides. "Back/north" in screen space is
    // the (-,-) world direction — the side the TC sprite hides.
    let cases = [
        ("victim from back(-,-), attacker from front(+,+)", (-14.0, -14.0), (16.0, 16.0)),
        ("victim from back(-,-), attacker from back(-,-)", (-14.0, -14.0), (-16.0, -16.0)),
        ("victim from front(+,+), attacker from back(-,-)", (14.0, 14.0), (-16.0, -16.0)),
        ("victim from east(+,-), attacker from west(-,+)", (14.0, -14.0), (-16.0, 16.0)),
        ("victim from west(-,+), attacker from east(+,-)", (-14.0, 14.0), (16.0, -16.0)),
    ];
    let mut failures = Vec::new();
    for (name, vstage, astage) in cases {
        let r = run_case(7, name, vstage, astage);
        println!(
            "CASE {name}: victim settled at ({:.2},{:.2}) cell {:?} blocked={} \
             (TC drained {:.1}); attacker end dist {:.2}, target_kept={}, \
             victim hp {:.1} dead={} after {} ticks",
            r.victim_settled.0,
            r.victim_settled.1,
            r.victim_cell,
            r.victim_cell_blocked,
            r.tc_drained,
            r.attacker_final_dist,
            r.attacker_target_kept,
            r.victim_hp_end,
            r.victim_dead,
            r.ticks_used
        );
        if !r.victim_dead {
            failures.push(r.name.clone());
        }
    }
    assert!(
        failures.is_empty(),
        "counter-attack failed to kill in: {failures:?}"
    );
}

/// The victim stands exactly ON the TC centre (a plain move, no
/// attack) — the purest form of "unit under the TC sprite".
#[test]
fn probe_victim_standing_on_tc_centre() {
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let victim = find_id(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    let attacker = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let tc_pos = pos(&sim, tc);
    sim.push_command(Command::Move {
        entity: victim,
        x: fx(tc_pos.0),
        y: fx(tc_pos.1),
    });
    for _ in 0..6000 {
        sim.tick();
        if ent(&sim, victim).order.is_none() {
            break;
        }
    }
    let vp = pos(&sim, victim);
    println!(
        "victim ordered to TC centre settled at ({:.2},{:.2}), dist to centre {:.2}",
        vp.0,
        vp.1,
        dist(vp, tc_pos)
    );
    sim.push_command(Command::Attack {
        entity: attacker,
        target: victim,
    });
    let mut killed = false;
    for _ in 0..9000 {
        sim.tick();
        if ent(&sim, victim).dead {
            killed = true;
            break;
        }
        if ent(&sim, attacker).attack_target != Some(victim) {
            println!(
                "attacker LOST its attack target: pos {:?} victim pos {:?} victim hp {:.1}",
                pos(&sim, attacker),
                pos(&sim, victim),
                f(ent(&sim, victim).hp)
            );
            break;
        }
    }
    println!(
        "centre case: killed={killed} attacker pos {:?} victim hp {:.1}",
        pos(&sim, attacker),
        f(ent(&sim, victim).hp)
    );
    assert!(killed, "attacker failed to kill a victim standing on the TC centre");
}

/// Hypothesis B's hard form: the victim stands on a cell the
/// nav grid BLOCKS, so `find_path` to its exact centre must
/// refuse and `attack_step`'s chase calls `clear_combat()`.
/// Staged deterministically with the one delayed-block kind:
/// a Palisade Wall foundation is walkable at progress 0
/// (BLD-20) and blocks its cell once construction first
/// advances — the victim walks onto the fresh foundation and
/// is standing there when the block lands. (This geometry
/// cannot occur at a Town Center — the TC never blocks — so
/// this probe documents the chase's behaviour on blocked goal
/// cells generally; it is NOT the live TC geometry.)
#[test]
fn probe_victim_on_blocked_cell() {
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let otc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let victim = find_id(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    let attacker = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let otc_pos = pos(&sim, otc);
    let otc_cell = sim.grid().world_to_cell(fx(otc_pos.0), fx(otc_pos.1));

    // Foundation cell: 8 cells east of the opponent TC centre
    // (open Arabia ground beside the start; scan east/north for
    // the first unblocked cell with an unblocked neighbour).
    let mut cell = None;
    'scan: for r in 6..14u32 {
        for (dx, dy) in [(r, 0), (0, r), (r, r)] {
            let c = (otc_cell.0 + dx, otc_cell.1 + dy);
            let west = (c.0 - 1, c.1);
            if !sim.grid().is_blocked(c.0, c.1) && !sim.grid().is_blocked(west.0, west.1) {
                cell = Some((c, west));
                break 'scan;
            }
        }
    }
    let (fc, west) = cell.expect("an open foundation cell near the opponent TC");
    let fpos = {
        let (x, y) = sim.grid().cell_center(fc.0, fc.1);
        (f(x), f(y))
    };
    let wpos = {
        let (x, y) = sim.grid().cell_center(west.0, west.1);
        (f(x), f(y))
    };

    // Victim stages on the cell WEST of the foundation cell;
    // the opponent builder stages far north so its walk gives
    // the victim time to step onto the foundation.
    let builder = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_OPPONENT && e.kind == EntityKind::Villager && !e.dead)
        .unwrap()
        .id;
    sim.push_command(Command::Move {
        entity: victim,
        x: fx(wpos.0),
        y: fx(wpos.1),
    });
    let far = {
        let (x, y) = sim.grid().cell_center(fc.0, fc.1 - 12);
        (f(x), f(y))
    };
    sim.push_command(Command::Move {
        entity: builder,
        x: fx(far.0),
        y: fx(far.1),
    });
    for _ in 0..9000 {
        sim.tick();
        if ent(&sim, victim).order.is_none() && ent(&sim, builder).order.is_none() {
            break;
        }
    }
    // Place the wall (foundation, progress 0, walkable), then
    // step the victim onto it while the builder walks back.
    sim.push_command(Command::Build {
        entity: builder,
        kind: BuildingKind::PalisadeWall,
        x: fx(fpos.0),
        y: fx(fpos.1),
    });
    sim.tick();
    assert!(
        sim.buildings()
            .iter()
            .any(|b| b.kind == BuildingKind::PalisadeWall && !b.complete),
        "wall foundation was placed"
    );
    assert!(!sim.grid().is_blocked(fc.0, fc.1), "fresh foundation is walkable");
    sim.push_command(Command::Move {
        entity: victim,
        x: fx(fpos.0),
        y: fx(fpos.1),
    });
    // Wait for the victim to arrive AND the builder's first
    // construction advance to block the cell under it.
    let mut blocked_under_victim = false;
    for _ in 0..12000 {
        sim.tick();
        let vc = sim.grid().world_to_cell(ent(&sim, victim).pos_x, ent(&sim, victim).pos_y);
        if vc == fc && sim.grid().is_blocked(fc.0, fc.1) {
            blocked_under_victim = true;
            break;
        }
    }
    println!(
        "blocked-cell staging: victim cell {:?} target cell {:?} blocked_under_victim={}",
        sim.grid().world_to_cell(ent(&sim, victim).pos_x, ent(&sim, victim).pos_y),
        fc,
        blocked_under_victim
    );
    assert!(blocked_under_victim, "staging failed: cell never blocked under the victim");
    // Measurement (this probe pins a MECHANISM, not the live
    // geometry): stop the builder (wall stays partial and
    // blocked) and issue the attack at once, while the victim
    // still stands on the blocked cell. `attack_step`'s chase
    // paths to the victim's exact centre; `find_path` refuses
    // a blocked goal cell, so the chase's first refresh must
    // `clear_combat()` — unless the victim is displaced off
    // the cell first. Record which happens, and when.
    let away = {
        let (x, y) = sim.grid().cell_center(fc.0, fc.1 - 3);
        (f(x), f(y))
    };
    sim.push_command(Command::Move {
        entity: builder,
        x: fx(away.0),
        y: fx(away.1),
    });
    let hp0 = f(ent(&sim, victim).hp);
    sim.push_command(Command::Attack {
        entity: attacker,
        target: victim,
    });
    let mut lost_at = None;
    let mut displaced_at = None;
    for t in 0..240u64 {
        sim.tick();
        let vc = sim.grid().world_to_cell(ent(&sim, victim).pos_x, ent(&sim, victim).pos_y);
        if vc != fc && displaced_at.is_none() {
            displaced_at = Some((t, vc));
        }
        if ent(&sim, attacker).attack_target != Some(victim) && lost_at.is_none() {
            lost_at = Some(t);
        }
        if lost_at.is_some() && displaced_at.is_some() {
            break;
        }
    }
    println!(
        "blocked-cell attack: target lost at {:?}, victim displaced at {:?}, \
         victim hp {:.1} (from {:.1}), victim cell now {:?} blocked={}",
        lost_at,
        displaced_at,
        f(ent(&sim, victim).hp),
        hp0,
        sim.grid().world_to_cell(ent(&sim, victim).pos_x, ent(&sim, victim).pos_y),
        sim.grid().is_blocked(fc.0, fc.1)
    );
    // The pinned signature (measured on 0ce201d): the blocked
    // state does NOT persist — the victim is displaced off the
    // blocked cell immediately (tick 0 of this window) and the
    // chase target is never lost, because by the first chase
    // refresh the goal is the victim's new, unblocked cell.
    // (A full-run variant of this staging killed the victim at
    // dist 2.93, target kept throughout.) So the chase's
    // clear_combat-on-blocked-goal path, while real code, is
    // not reachable as a persistent failure here — and the TC
    // geometry above never blocks at all. Hypothesis B is not
    // the live FIX #2b mechanism; the client fog gate is.
    assert!(
        displaced_at.is_some(),
        "victim was never displaced off the blocked cell"
    );
    assert!(
        lost_at.is_none(),
        "chase target was lost at {lost_at:?} despite the displacement"
    );
}
