//! OCCORDER5 probe (bot-loop, 2026-11-18) — auxiliary measurement
//! harness, NOT a regression pin.
//!
//! Live defect, still open after client FIX #2/#2b/#2c/#3/#4/#5
//! (run #428 session 2, published build 2b3f113): an enemy Scout
//! Cavalry stands outline-only under the player's Town Center
//! roof — at the EAST porch — draining the TC. Left-click
//! selects it (45/45). The player's Scout (Aggressive) is
//! right-clicked onto the outline: it walks to the enemy's exact
//! point and stands IDLE; the enemy's HP never drops; when the
//! enemy later flees east (TC-arrow damage) our Scout does not
//! pursue.
//!
//! What the earlier probes staged, and what they did not:
//! - stanceatk: explicit Attack engages in every stance — but
//!   the victim was always staged from (-14,-14) (the NW side)
//!   and settled wherever TC-attack stops a NW approach.
//! - amoveprobe: attack-move / explicit Attack engage against a
//!   victim inside a real TC footprint — staged at the footprint
//!   centre area; the footprint itself is not a sim obstacle.
//! Neither staged the victim settled from an EAST approach (the
//! live side), nor measured the chase's own A* goal test at the
//! victim's settled cell.
//!
//! The chase mechanism that can silently kill an explicit
//! Attack (combat.rs attack_step): out of range, the attacker
//! refreshes its internal move order via set_move_order(attacker
//! -> victim pos); set_move_order returns false when
//! find_path_through_for refuses the GOAL cell
//! (grid.rs: a blocked goal returns None before A* runs), and
//! attack_step then calls clear_combat — the attack order is
//! dropped without a sound. The same refusal loop applies to an
//! auto-acquired target after a degraded plain Move completes:
//! acquire -> chase fails -> clear -> re-acquire -> ... the
//! attacker stands at the point, idle, forever.
//!
//! This probe stages the EXACT live geometry as closely as the
//! handoffs describe: the victim is walked to a staging offset
//! on each of the 8 compass sides of the player's TC, ordered
//! to Attack the TC (the client's enemy does exactly this), and
//! settles where TC-attack stops it — verifiably draining the
//! TC. Per settle point it records: the victim's cell and its
//! blocked status, the chase A*'s verdict on the victim's exact
//! position as a goal, a plain Move onto that exact position
//! (the degraded client order's shape), and an explicit Attack
//! on the entity id (the correct client order's shape) with the
//! attacker approaching from outside.

use fixed::types::I16F16;
use skirmish_sim::{
    Command, Domain, Entity, EntityKind, MatchMode, Sim, Stance, OWNER_OPPONENT, OWNER_PLAYER,
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

fn pos(sim: &Sim, id: u32) -> (f64, f64) {
    let e = ent(sim, id);
    (f(e.pos_x), f(e.pos_y))
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Settle the enemy Scout onto the player's TC from a staging
/// offset (walk to the offset, Attack the TC, wait until the TC
/// has verifiably drained >= 3 HP). Returns (tc, victim, tc_pos,
/// victim_settle_pos).
fn settle_victim_on_tc(sim: &mut Sim, victim_stage: (f64, f64)) -> (u32, u32, (f64, f64), (f64, f64)) {
    let tc = find_id(sim, OWNER_PLAYER, EntityKind::TownCenter);
    let victim = find_id(sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    let tc_pos = pos(sim, tc);
    let tc_hp0 = f(ent(sim, tc).hp);
    sim.push_command(Command::Move {
        entity: victim,
        x: fx(tc_pos.0 + victim_stage.0),
        y: fx(tc_pos.1 + victim_stage.1),
    });
    for _ in 0..6000 {
        sim.tick();
        if ent(sim, victim).order.is_none() {
            break;
        }
    }
    sim.push_command(Command::Attack {
        entity: victim,
        target: tc,
    });
    let mut drained = 0.0;
    for _ in 0..12000 {
        sim.tick();
        drained = tc_hp0 - f(ent(sim, tc).hp);
        if drained >= 3.0 {
            break;
        }
    }
    assert!(drained >= 3.0, "victim never engaged the TC (drained {drained})");
    let vpos = pos(sim, victim);
    (tc, victim, tc_pos, vpos)
}

fn stage_attacker(sim: &mut Sim, attacker: u32, at: (f64, f64)) {
    sim.push_command(Command::Move {
        entity: attacker,
        x: fx(at.0),
        y: fx(at.1),
    });
    for _ in 0..6000 {
        sim.tick();
        if ent(sim, attacker).order.is_none() {
            break;
        }
    }
}

/// Diagnostics at the victim's settled cell: blocked status and
/// the chase A*'s own verdict on the victim's exact position as
/// a goal from the attacker's staging point.
fn cell_diag(sim: &Sim, victim: u32, from: (f64, f64)) -> String {
    let v = ent(sim, victim);
    let cell = sim.grid().world_to_cell(v.pos_x, v.pos_y);
    let blocked = sim.grid().is_blocked(cell.0, cell.1);
    let blocked_land = sim.grid().is_blocked_for(Domain::Land, cell.0, cell.1);
    let path = sim.grid().find_path_through_for(
        Domain::Land,
        (fx(from.0), fx(from.1)),
        (v.pos_x, v.pos_y),
        &[],
    );
    format!(
        "victim_cell=({},{}) is_blocked={} is_blocked_land={} chase_astar_goal={}",
        cell.0,
        cell.1,
        blocked,
        blocked_land,
        if path.is_some() { "PATH" } else { "REFUSED" }
    )
}

#[derive(Default)]
struct Measure {
    hp_at_order: f64,
    hp_end: f64,
    first_damage_tick: Option<u64>,
    damage_events: usize,
    idle_ticks: u64,
    target_lost_tick: Option<u64>,
    kill_tick: Option<u64>,
    min_dist: f64,
    final_dist: f64,
    chase_order_seen: bool,
}

fn measure_attack(sim: &mut Sim, attacker: u32, victim: u32, ticks: u64) -> Measure {
    let mut m = Measure {
        hp_at_order: f(ent(sim, victim).hp),
        min_dist: f64::MAX,
        ..Default::default()
    };
    let mut prev_hp = m.hp_at_order;
    for t in 1..=ticks {
        sim.tick();
        let v = ent(sim, victim);
        let hp = f(v.hp);
        if hp < prev_hp - 1e-9 {
            m.damage_events += 1;
            if m.first_damage_tick.is_none() {
                m.first_damage_tick = Some(t);
            }
        }
        prev_hp = hp;
        if v.dead && m.kill_tick.is_none() {
            m.kill_tick = Some(t);
        }
        let a = ent(sim, attacker);
        if a.order.is_some() {
            m.chase_order_seen = true;
        }
        if !v.dead && a.attack_target.is_none() && a.order.is_none() {
            m.idle_ticks += 1;
        }
        if !v.dead && a.attack_target != Some(victim) && m.target_lost_tick.is_none() {
            m.target_lost_tick = Some(t);
        }
        let d = dist(pos(sim, attacker), pos(sim, victim));
        if d < m.min_dist {
            m.min_dist = d;
        }
    }
    m.hp_end = f(ent(sim, victim).hp);
    m.final_dist = dist(pos(sim, attacker), pos(sim, victim));
    m
}

/// One approach direction: settle the victim from `stage`, then
/// (a) a plain Move by the attacker onto the victim's EXACT
/// position (the degraded order's shape — and the chase's own
/// goal), in a fresh identically-staged sim; (b) an explicit
/// Attack on the entity id, attacker approaching from outside.
fn run_approach(label: &str, stage: (f64, f64), stance: Stance) {
    // (b) explicit Attack.
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let (_tc, victim, tc_pos, vpos) = settle_victim_on_tc(&mut sim, stage);
    let attacker = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let astage = (tc_pos.0 + 16.0, tc_pos.1 + 16.0);
    stage_attacker(&mut sim, attacker, astage);
    let diag = cell_diag(&sim, victim, astage);
    sim.push_command(Command::SetStance {
        entity: attacker,
        stance,
    });
    sim.push_command(Command::Attack {
        entity: attacker,
        target: victim,
    });
    let m = measure_attack(&mut sim, attacker, victim, 3600);
    println!(
        "RESULT attack [{label} {stance:?}]: victim_settled=({:.2},{:.2}) dist_to_tc={:.2} {} \
         hp {:.1}->{:.1} first_dmg={:?} dmg_events={} kill={:?} target_lost={:?} \
         idle_ticks={} chase_order_seen={} min_dist={:.2} final_dist={:.2}",
        vpos.0,
        vpos.1,
        dist(vpos, tc_pos),
        diag,
        m.hp_at_order,
        m.hp_end,
        m.first_damage_tick,
        m.damage_events,
        m.kill_tick,
        m.target_lost_tick,
        m.idle_ticks,
        m.chase_order_seen,
        m.min_dist,
        m.final_dist,
    );

    // (a) plain Move onto the victim's exact position.
    let mut sim = Sim::with_mode(7, MatchMode::TwoPlayer);
    let (_tc, victim, _tc_pos, vpos2) = settle_victim_on_tc(&mut sim, stage);
    assert!(
        dist(vpos, vpos2) < 0.01,
        "staging is deterministic: settle points agree ({vpos:?} vs {vpos2:?})"
    );
    let attacker = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    stage_attacker(&mut sim, attacker, astage);
    // NoAttack so the measurement is the move alone, never an
    // auto-acquired engagement.
    sim.push_command(Command::SetStance {
        entity: attacker,
        stance: Stance::NoAttack,
    });
    sim.push_command(Command::Move {
        entity: attacker,
        x: fx(vpos2.0),
        y: fx(vpos2.1),
    });
    let mut min_dist = f64::MAX;
    let mut order_alive_ticks = 0u64;
    for _ in 1..=3600 {
        sim.tick();
        let a = ent(&sim, attacker);
        if a.order.is_some() {
            order_alive_ticks += 1;
        }
        let d = dist(pos(&sim, attacker), pos(&sim, victim));
        if d < min_dist {
            min_dist = d;
        }
    }
    println!(
        "RESULT move-goal [{label}]: attacker staged at ({:.2},{:.2}); move_order_alive_ticks={} \
         min_dist_to_victim={:.2} final_dist={:.2}",
        astage.0,
        astage.1,
        order_alive_ticks,
        min_dist,
        dist(pos(&sim, attacker), pos(&sim, victim)),
    );
}

#[test]
fn probe_tc_ring_all_approaches() {
    // The 8 compass approaches. The live victim held the EAST
    // porch; stanceatk staged only NW (-14,-14).
    let approaches: [(&str, (f64, f64)); 8] = [
        ("NW", (-14.0, -14.0)),
        ("N", (0.0, -16.0)),
        ("NE", (14.0, -14.0)),
        ("E", (16.0, 0.0)),
        ("SE", (14.0, 14.0)),
        ("S", (0.0, 16.0)),
        ("SW", (-14.0, 14.0)),
        ("W", (-16.0, 0.0)),
    ];
    for (label, stage) in approaches {
        run_approach(label, stage, Stance::Aggressive);
    }
}

#[test]
fn probe_tc_east_standground() {
    // The parent task's second live shape: the same east-porch
    // geometry with the attacker in Stand Ground (run #429's
    // report). An explicit order must chase in any stance
    // (stanceatk's doctrine) — measured here for the east settle.
    run_approach("E", (16.0, 0.0), Stance::StandGround);
    run_approach("NE", (14.0, -14.0), Stance::StandGround);
}
