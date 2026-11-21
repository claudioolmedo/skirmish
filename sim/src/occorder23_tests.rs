//! OCCORDER23 (2026-11-20): the EXECUTION layer of the
//! hidden-attacker defect — bot-loop runs #448/#451, on build
//! 43a16e5 (FIX #20 live). The pick layer is closed (FIX #20
//! proved live in #451, classes R/G/A) and the issue layer is
//! proven (`pick {picked:71, fromOutline:true}` →
//! `issue attack ok:true`). The failure is downstream of the
//! issue: within ~a minute the ordered Militia is IDLE again —
//! in #451 idle AT THE BARRACKS, i.e. where it spawned — while
//! the enemy under the TC roof keeps draining it (2400→1713 in
//! #448; TC destroyed ~31:08 in #451).
//!
//! DISCRIMINATION (all scenes staged through the real Command
//! path — the foe's Attack on the TC, the attacker's SetStance
//! and Attack are real `Command`s; positions/kinds are scene
//! selection in the occorder9/occorder15 discipline):
//!
//!   - (sparse) A Militia ordered onto a Spearman that walked
//!     in from the south and settled on the TC's south porch
//!     (dist 2.93 from the centre, draining it), from S-far
//!     (28 wu), S-near, N-far and the diagonal, at full HP and
//!     at the live 21 HP: engages every time (first damage
//!     98–496 ticks, foe dead ≤1,816, target never dropped).
//!     The same holds for a foe at the TC centre, with and
//!     without occorder15's r=5 dropoff ring. The sparse and
//!     ring geometries of occorder9/15 stay closed for the
//!     Militia/Spearman pairing too.
//!   - (traffic) Four villagers endlessly walking dropoff
//!     routes through the fight zone do not break the
//!     engagement (foe dead at 1,718, no drop).
//!   - (multi-foe) With THREE foes settled on the TC, the
//!     Militia kills its ordered foe (t=1,719), auto-acquires
//!     the next one tick later, and kills all three by 4,658 —
//!     hypothesis (b), an acquisition re-scan dropping or
//!     failing to chain targets at the footprint, does not
//!     reproduce: an Aggressive Militia never stands idle
//!     while a foe drains its TC in LoS.
//!   - (d) TC arrows / garrison: INVERTED. A TC garrisoned
//!     (after the foe settled) kills the hidden foe ITSELF —
//!     the foe died at t=447 of the attack window with
//!     `last_hit_by` = the building-arrow sentinel; garrisoned
//!     before the settle, the foe was already at 25 HP after
//!     300 ticks. A firing TC cannot be the reason a foe
//!     survives for minutes, and a target cleared by the foe's
//!     death to arrows is a correct idle, with the drainage
//!     stopped — not the live shape (drainage continued).
//!   - (c) separation/press ejection: no scene shows an
//!     in-range attacker displaced (MOV-03 has no
//!     displacement; the in-range arm suppresses separation),
//!     and every contact scene engages — the occorder15 press
//!     holds for this pairing.
//!   - (a) THE MECHANISM — attack order cleared on the chase's
//!     first path failure. With the foe+TC enclosed by a
//!     completed palisade ring (Chebyshev radius 3 cells, the
//!     grid dynamic cells a PalisadeWall segment sets; the
//!     #448/#451 sessions both built complete palisades) and
//!     the Militia OUTSIDE it, the pristine chase discards the
//!     order at tick 0: `set_move_order` to the victim returns
//!     false (A* cannot reach the enclosed goal), and
//!     `attack_step` answered with `clear_combat()`. Measured
//!     on pristine: drop at t=0, attacker travel 0.0 (it never
//!     leaves its spawn — the #451 "idle at the Barracks"),
//!     foe HP frozen at 45, TC draining 2396→2316 over 14,400
//!     ticks and counting. The identical scene with a one-cell
//!     gap in the ring engages (first damage 398, foe dead
//!     1,718, travel 22.1). The ring is the whole difference.
//!
//! THE FIX (combat.rs, attack_step's chase arm only): a failed
//! exact path no longer completes the order. The attacker
//! installs a fallback walk to the closest stand point it CAN
//! path to (expanding free-cell rings around the victim,
//! `chase_fallback_order`), keeps its target, and retries the
//! exact path on a 600-tick cadence (`chase_retry_tick`, the
//! M6 no-A*-per-tick discipline kept by the gate). If the
//! enclosure opens mid-hold (a razed segment, a gate), the
//! retry resumes the chase and the foe dies. Only a victim no
//! fallback point within 12 cells can reach at all (a land
//! unit ordered onto a ship in deep water) still ends the
//! chase, as before. No rule, balance or design change: an
//! ordered unit closes as far as the ground allows and holds
//! its order there — the DE observable — instead of silently
//! idling at spawn from the first tick.
//!
//! Post-fix measurements (same scenes): enclosed — target
//! retained for the full 14,400 ticks, attacker closes 11.0
//! wu to the ring's south face and holds there (end distance
//! to the foe 14.0, just outside the wall), foe keeps
//! draining (the honest outcome: it still cannot be reached
//! until the ring opens); enclosure breached at t=1,200 —
//! chase resumes at the retry, foe dead ~2,400 ticks later;
//! gap control and every sparse/traffic/multi scene unchanged
//! (first-damage and kill ticks identical to pristine, since
//! their exact paths never fail). Golden hash: unchanged —
//! the golden scenario issues no commands and no chase ever
//! fails a path, so `chase_retry_tick` never leaves 0, its
//! conditional hash block (marker 0xCA) never appears, and
//! the fallback code never runs.

use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Command, Sim, Stance};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 { v.to_num::<f64>() }
fn ent(sim: &Sim, id: u32) -> &Entity { sim.entities().iter().find(|e| e.id == id).unwrap() }
fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, x: I16F16, y: I16F16) {
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

const FOE: u32 = 9_901;
const FOE2: u32 = 9_903;
const FOE3: u32 = 9_904;
const ATK: u32 = 9_902;

#[derive(Default, Debug)]
struct Trace {
    first_dmg: Option<u64>,
    drop_tick: Option<u64>,
    dead_tick: Option<u64>,
    max_travel: f64,
    end_target: Option<u32>,
    end_dist: f64,
    tc_end: f64,
    foe_hp_end: f64,
}

/// Foe Spearman walks in from `foe_spawn_off` (TC-relative) and
/// attacks the TC by real command; settles `settle` ticks.
/// Attacker Militia spawned at `atk_off`, Aggressive, Attack
/// foe by real command. Optional r=5 villager ring.
fn scene(foe_spawn_off: (f64, f64), settle: u64, foe_hp: Option<i32>,
         ring5: bool, atk_off: (f64, f64), window: u64) -> Trace {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT,
          tcx + I16F16::from_num(foe_spawn_off.0), tcy + I16F16::from_num(foe_spawn_off.1));
    if let Some(hp) = foe_hp {
        let i = sim.entities.iter().position(|e| e.id == FOE).unwrap();
        sim.entities[i].hp = I16F16::from_num(hp);
    }
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    if ring5 {
        for (i, (dx, dy)) in [(5.0, 0.0), (-5.0, 0.0), (0.0, 5.0), (0.0, -5.0),
                              (3.54, 3.54), (3.54, -3.54), (-3.54, 3.54), (-3.54, -3.54)]
            .iter().enumerate() {
            spawn(&mut sim, 9_910 + i as u32, EntityKind::Villager, OWNER_PLAYER,
                  tcx + I16F16::from_num(*dx), tcy + I16F16::from_num(*dy));
        }
    }
    for _ in 0..settle { sim.tick(); }
    spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
          tcx + I16F16::from_num(atk_off.0), tcy + I16F16::from_num(atk_off.1));
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(ATK, FOE), "can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let foe_hp0 = f(ent(&sim, FOE).hp);
    let (ax0, ay0) = (ent(&sim, ATK).pos_x, ent(&sim, ATK).pos_y);
    let mut tr = Trace::default();
    for t in 0..window {
        sim.tick();
        let a = ent(&sim, ATK);
        let fo = ent(&sim, FOE);
        let travel = f(steer::vec_len(a.pos_x - ax0, a.pos_y - ay0));
        if travel > tr.max_travel { tr.max_travel = travel; }
        if tr.first_dmg.is_none() && f(fo.hp) < foe_hp0 { tr.first_dmg = Some(t); }
        if tr.drop_tick.is_none() && !fo.dead && a.attack_target != Some(FOE) {
            tr.drop_tick = Some(t);
        }
        if fo.dead { tr.dead_tick = Some(t); break; }
    }
    let a = ent(&sim, ATK);
    let fo = ent(&sim, FOE);
    tr.end_target = a.attack_target;
    tr.end_dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
    tr.tc_end = f(ent(&sim, 0).hp);
    tr.foe_hp_end = f(fo.hp);
    tr
}

fn assert_engages(tag: &str, tr: &Trace) {
    assert!(tr.drop_tick.is_none(), "{tag}: attack target dropped at {:?} while foe alive: {tr:?}", tr.drop_tick);
    assert!(tr.first_dmg.is_some() && tr.first_dmg.unwrap() <= 600,
        "{tag}: first damage too late: {tr:?}");
    assert!(tr.dead_tick.is_some(), "{tag}: foe never died: {tr:?}");
}

#[test]
fn settled_south_porch_militia_engages_from_every_side() {
    for (tag, atk) in [("S-far", (0.0, 28.0)), ("S-near", (0.0, 10.0)),
                       ("N-far", (0.0, -28.0)), ("diag", (20.0, 20.0))] {
        let tr = scene((0.0, 20.0), 900, None, false, atk, 7200);
        assert_engages(tag, &tr);
    }
    // The #448 live value: foe already at 21/45 when clicked.
    let tr = scene((0.0, 20.0), 900, Some(21), false, (0.0, 28.0), 7200);
    assert_engages("hp21", &tr);
}

#[test]
fn centre_and_dropoff_ring_militia_engages() {
    assert_engages("centre", &scene((0.0, 0.0), 120, None, false, (0.0, 28.0), 7200));
    assert_engages("centre+ring", &scene((0.0, 0.0), 120, None, true, (0.0, 28.0), 7200));
    assert_engages("settled+ring S", &scene((0.0, 20.0), 900, None, true, (0.0, 28.0), 7200));
    assert_engages("settled+ring N", &scene((0.0, 20.0), 900, None, true, (0.0, -28.0), 7200));
}

/// Enclosure staging: foe settled on the TC; a completed
/// palisade ring (grid dynamic cells — what a PalisadeWall
/// segment's placement sets) at Chebyshev radius 3 cells
/// around the TC, optionally with a one-cell south gap, or
/// breached at `breach_tick` (three south cells cleared, a
/// razed segment). Attacker Militia outside, south.
fn enclosure_scene(gap_south: bool, breach_tick: Option<u64>, window: u64) -> Trace {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT,
          tcx, tcy + I16F16::from_num(20));
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..900 { sim.tick(); }
    let (ccx, ccy) = sim.grid.world_to_cell(tcx, tcy);
    let ring = |sim: &mut Sim, on: bool| {
        for dy in -3i32..=3 {
            for dx in -3i32..=3 {
                if dx.abs() != 3 && dy.abs() != 3 { continue; }
                if gap_south && dx == 0 && dy == 3 { continue; }
                sim.grid.set_dynamic((ccx as i32 + dx) as u32, (ccy as i32 + dy) as u32, on);
            }
        }
    };
    ring(&mut sim, true);
    spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
          tcx, tcy + I16F16::from_num(28));
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(ATK, FOE), "can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let foe_hp0 = f(ent(&sim, FOE).hp);
    let (ax0, ay0) = (ent(&sim, ATK).pos_x, ent(&sim, ATK).pos_y);
    let mut tr = Trace::default();
    for t in 0..window {
        if breach_tick == Some(t) {
            for dx in -1i32..=1 {
                sim.grid.set_dynamic((ccx as i32 + dx) as u32, (ccy as i32 + 3) as u32, false);
            }
        }
        sim.tick();
        let a = ent(&sim, ATK);
        let fo = ent(&sim, FOE);
        let travel = f(steer::vec_len(a.pos_x - ax0, a.pos_y - ay0));
        if travel > tr.max_travel { tr.max_travel = travel; }
        if tr.first_dmg.is_none() && f(fo.hp) < foe_hp0 { tr.first_dmg = Some(t); }
        if tr.drop_tick.is_none() && !fo.dead && a.attack_target != Some(FOE) {
            tr.drop_tick = Some(t);
        }
        if fo.dead { tr.dead_tick = Some(t); break; }
    }
    let a = ent(&sim, ATK);
    let fo = ent(&sim, FOE);
    tr.end_target = a.attack_target;
    tr.end_dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
    tr.tc_end = f(ent(&sim, 0).hp);
    tr.foe_hp_end = f(fo.hp);
    tr
}

/// THE PIN. On pristine this fails at the first assertion:
/// the target drops at t=0 and the attacker never moves.
#[test]
fn enclosed_foe_order_is_retained_and_attacker_closes_to_the_wall() {
    let tr = enclosure_scene(false, None, 14_400);
    assert!(tr.drop_tick.is_none(),
        "attack target dropped at {:?} while the enclosed foe lived: {tr:?}", tr.drop_tick);
    assert_eq!(tr.end_target, Some(FOE), "order not retained at window end: {tr:?}");
    assert!(tr.max_travel > 8.0,
        "attacker never closed toward the ring (travel {:.1}): {tr:?}", tr.max_travel);
    assert!(tr.end_dist < 18.0,
        "attacker does not hold at the ring face (end dist {:.1}): {tr:?}", tr.end_dist);
    // The honest outcome while the ring stands: the foe still
    // cannot be reached, so it keeps draining — but the order
    // and the pressure at the wall persist.
    assert!(tr.dead_tick.is_none() && tr.tc_end < 2396.0,
        "enclosed foe should still be draining: {tr:?}");
}

/// The retry: breach the ring mid-hold and the held chase
/// resumes and completes. On pristine the order is long gone
/// (dropped at t=0), so the foe never dies.
#[test]
fn enclosure_breached_mid_hold_resumes_chase_and_kills() {
    let tr = enclosure_scene(false, Some(1200), 9600);
    assert!(tr.drop_tick.is_none(), "target dropped at {:?}: {tr:?}", tr.drop_tick);
    assert!(tr.dead_tick.is_some(), "foe never died after the breach: {tr:?}");
}

/// Control: the one-cell gap engages on pristine and fixed
/// alike (first damage ~398, foe dead ~1,718 measured).
#[test]
fn gap_in_ring_engages() {
    let tr = enclosure_scene(true, None, 7200);
    assert_engages("gap", &tr);
}

/// The attacker enclosed instead: a Militia boxed in its own
/// palisade pocket, foe draining the TC outside. Same pristine
/// signature (drop at t=0, travel 0); post-fix the target is
/// retained and the unit closes to its own wall face.
#[test]
fn enclosed_attacker_also_retains_its_order() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT,
          tcx, tcy + I16F16::from_num(20));
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..900 { sim.tick(); }
    let atk_pos = (tcx, tcy + I16F16::from_num(28));
    let (acx, acy) = sim.grid.world_to_cell(atk_pos.0, atk_pos.1);
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            if dx.abs() != 2 && dy.abs() != 2 { continue; }
            sim.grid.set_dynamic((acx as i32 + dx) as u32, (acy as i32 + dy) as u32, true);
        }
    }
    spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER, atk_pos.0, atk_pos.1);
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let mut drop = None;
    let mut max_travel = 0.0f64;
    for t in 0..3600u64 {
        sim.tick();
        let a = ent(&sim, ATK);
        let travel = f(steer::vec_len(a.pos_x - atk_pos.0, a.pos_y - atk_pos.1));
        if travel > max_travel { max_travel = travel; }
        if drop.is_none() && ent(&sim, FOE).dead == false && a.attack_target != Some(FOE) {
            drop = Some(t);
        }
    }
    assert!(drop.is_none(), "enclosed attacker's target dropped at {drop:?}");
    assert_eq!(ent(&sim, ATK).attack_target, Some(FOE));
    assert!(max_travel > 3.0, "enclosed attacker never closed to its wall (travel {max_travel:.1})");
}

/// Hypothesis (b) control: three foes on the TC — the ordered
/// one dies, the chain auto-acquires, all die, TC survives.
#[test]
fn multi_foe_chain_engages() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    for (id, off) in [(FOE, (0.0, 20.0)), (FOE2, (-20.0, 0.0)), (FOE3, (0.0, -20.0))] {
        spawn(&mut sim, id, EntityKind::Spearman, OWNER_OPPONENT,
              tcx + I16F16::from_num(off.0), tcy + I16F16::from_num(off.1));
        sim.push_command(Command::Attack { entity: id, target: 0 });
    }
    for _ in 0..900 { sim.tick(); }
    spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
          tcx, tcy + I16F16::from_num(28));
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let mut all_dead_at = None;
    for t in 0..7200u64 {
        sim.tick();
        if ent(&sim, FOE).dead && ent(&sim, FOE2).dead && ent(&sim, FOE3).dead {
            all_dead_at = Some(t);
            break;
        }
        assert!(!ent(&sim, ATK).dead, "attacker died at {t}");
    }
    assert!(all_dead_at.is_some(), "multi-foe chain did not clear all foes in 7,200 ticks");
    assert!(f(ent(&sim, 0).hp) > 2000.0, "TC took implausible damage: {:.0}", f(ent(&sim, 0).hp));
}

/// Moving dropoff traffic through the fight does not break
/// the engagement (hypothesis (c) traffic form).
#[test]
fn moving_traffic_still_engages() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT,
          tcx, tcy + I16F16::from_num(20));
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    let mut vils = vec![];
    for i in 0..4 {
        let id = 9_920 + i;
        spawn(&mut sim, id, EntityKind::Villager, OWNER_PLAYER,
              tcx + I16F16::from_num(2.0 * i as f64 - 3.0), tcy + I16F16::from_num(14.0));
        vils.push(id);
    }
    for _ in 0..900 { sim.tick(); }
    spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
          tcx, tcy + I16F16::from_num(28));
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let mut dead_at = None;
    let mut drop = None;
    for t in 0..7200u64 {
        for (i, &v) in vils.iter().enumerate() {
            let e = ent(&sim, v);
            if e.order.is_none() {
                let south = f(e.pos_y - tcy) > 8.0;
                let (tx, ty) = if south {
                    (f(e.pos_x), f(tcy) + 4.5)
                } else {
                    (f(tcx) + 2.0 * i as f64 - 3.0, f(tcy) + 14.0)
                };
                sim.push_command(Command::Move { entity: v, x: I16F16::from_num(tx), y: I16F16::from_num(ty) });
            }
        }
        sim.tick();
        let a = ent(&sim, ATK);
        let fo = ent(&sim, FOE);
        if drop.is_none() && !fo.dead && a.attack_target != Some(FOE) { drop = Some(t); }
        if fo.dead { dead_at = Some(t); break; }
    }
    assert!(drop.is_none(), "traffic dropped the target at {drop:?}");
    assert!(dead_at.is_some(), "foe never died under traffic");
}

/// Hypothesis (d) discrimination (passes pre- and post-fix —
/// it is a measurement pin, not a fix pin): a TC garrisoned
/// AFTER the foe settles kills the hidden foe with its own
/// arrows (last_hit_by = the building sentinel), fast — so a
/// firing TC explains a foe dying, never a foe surviving.
#[test]
fn garrisoned_tc_kills_the_hidden_foe_itself() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT,
          tcx, tcy + I16F16::from_num(20));
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..900 { sim.tick(); }
    let vils: Vec<u32> = sim.entities().iter()
        .filter(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::Villager)
        .take(2).map(|e| e.id).collect();
    for v in &vils { sim.push_command(Command::GarrisonTc { entity: *v, tc: 0 }); }
    for _ in 0..60 { sim.tick(); }
    spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
          tcx, tcy + I16F16::from_num(28));
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let mut dead_at = None;
    for t in 0..2400u64 {
        sim.tick();
        if ent(&sim, FOE).dead { dead_at = Some(t); break; }
    }
    assert!(dead_at.is_some(), "garrisoned TC + Militia never killed the foe");
    assert_eq!(ent(&sim, FOE).last_hit_by, Some(u32::MAX),
        "expected the killing blow to be a TC arrow (building sentinel)");
}
