//! ENGAGESE (2026-11-23): hidden-attacker staging scene (e) —
//! the exact geometry of the LIVE defect in bot-loop runs
//! #481–#484, never staged by any prior suite.
//!
//! Live (real client, #481–#484): the enemy Scout Cavalry
//! camps at the Town Center's SOUTH-EAST pillar (screen SE),
//! actively attacking the TC, and the player's Militia —
//! ordered to Attack by right-click — walks to the TC's EAST
//! edge and goes idle there forever (60s+, both HP frozen,
//! TC draining).
//!
//! Prior harness work staged: foe settled at the TC NORTH
//! edge vs spawned defender from every face (tcsouth) and
//! vs a trained rally-south defender (tcrally); trained /
//! rally defenders vs centre and east foes (engagefrozen /
//! engageunder); a foe ARRIVING from E/SE/NE settling on
//! the east sector vs a trained defender whose Barracks sat
//! south-east (engagepillar — Scout-SE settled (2.11,2.11),
//! and every face there engaged). What none staged: the
//! foe settled at the SE pillar under a real Attack on the
//! TC, attacked by a defender that starts EAST of the TC —
//! the live Militia's own approach, whose chase ends at the
//! TC's east edge in the live client — nor the same SE foe
//! vs a defender from the SOUTH, nor an east-Barracks
//! trained defender against this foe.
//!
//! Scene construction: the foe is NOT teleported. It spawns
//! 24 wu out on the SE diagonal under a real
//! `Command::Attack` on the TC (entity 0) and settles
//! wherever the TC-attack stand actually is (recorded;
//! `attack_target=Some(TC)` and TC draining asserted).
//! Defenders: (1) MAIN — a spawned Militia (scene selection
//! only) EAST of the TC; (2) TRAINED — a Militia trained
//! through the real queue at a Barracks east of the TC
//! (tcrally's record placement, cc+(2,-1)) with rally set
//! EAST before it spawns, Attack issued from the rally-end
//! state; (3) MIRROR — a spawned Militia SOUTH of the TC.
//! Every defender gets Aggressive stance and a real
//! `Command::Attack` on the settled foe (`can_attack`
//! asserted). Metrics mirror tcsouth/tcrally
//! (Capture/Trace/run_window): first damage tick, kill
//! tick, order-drop tick, min distance, worst out-of-reach
//! / in-reach no-damage stall, window-end stand.
//!
//! OUTCOME (measured on base ce215e1, see handoff
//! `engagese-build.md`): NOT REPRODUCED on any face. The
//! Scout arriving from the SE settles at exactly
//! (2.12,2.12) — the SE pillar stand, centre distance
//! 3.00 — with attack_target=Some(TC) and the TC draining
//! (2400 -> 2397 over the settle; foe HP 45 -> 42 from TC
//! fire). Every defender engages and kills it:
//!   MAIN (spawned, from EAST) — first damage t=368, foe
//!   dead t=1568, drop=None, min_dist=3.00,
//!   max_stall_out=0, end stand (4.88,3.28): the Militia
//!   does NOT stop at the TC's east edge; it closes to
//!   melee distance beside the pillar and fights there.
//!   TRAINED (Barracks east, rally EAST; spawn
//!   (4.00,-8.00), rally arrived t=422 at stand
//!   (18.89,-0.43), dist 16.97) — first damage t=471,
//!   dead t=1671, drop=None, min_dist=2.96, end stand
//!   (3.89,4.50).
//!   MIRROR (spawned, from SOUTH) — first damage t=268,
//!   dead t=1468, drop=None, min_dist=3.00, end stand
//!   (3.28,4.88).
//! The max_stall_in=30 on every face is the in-reach
//! strike cadence (strike_tick set, damage progressing),
//! as in tcsouth/tcrally — not a stall. The exact live
//! #481–#484 geometry (SE-pillar foe × east-approach
//! defender, spawned and trained) therefore has NO sim
//! counterpart either: in the harness the east-edge stop
//! never happens. The remaining live-only territory is
//! unchanged — the client order path / the exact live
//! state at the idle moment. Per package discipline NO
//! production fix is made; these pins are the deliverable.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Command, Sim, Stance};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}
fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}
fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, x: I16F16, y: I16F16) {
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

const FOE: u32 = 9_801;
const ATK: u32 = 9_802;
const RAX: u32 = 9_950;
const MELEE: f64 = 3.0;
/// Consecutive position-identical ticks (order held) that
/// confirm a frozen rally stand (tcrally's measure).
const FREEZE_CONFIRM: u64 = 120;

fn place_record(sim: &mut Sim, id: u32, kind: BuildingKind, owner: u8, cell: (u32, u32)) {
    let size = kind.footprint().w;
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    let b = Building {
        id,
        kind,
        owner,
        hp: I16F16::from_num(kind.max_hp()),
        cell_x: cell.0,
        cell_y: cell.1,
        pos_x: x0 + half - I16F16::from_num(2),
        pos_y: y0 + half - I16F16::from_num(2),
        progress_thirds: kind.build_thirds(),
        repair_hp_remainder: 0,
        repair_cost_remainder: 0,
        repair_stone_remainder: 0,
        repair_gold_remainder: 0,
        build_rate_remainder: 0,
        gate_along_x: true,
        queue_rate_remainder: 0,
        complete: true,
        node_id: None,
        queue: Vec::new(),
        train_progress: 0,
        rally: None,
        rally_node: None,
        weapon_target: None,
        weapon_target_building: None,
        weapon_strike_tick: None,
        weapon_next_tick: 0,
        weapon_engaged: false,
        garrison: Vec::new(),
        garrison_arrows: 0,
        relics: Vec::new(),
        relic_gold_remainder: 0,
    };
    if kind.blocks_footprint() {
        for (cx, cy) in b.footprint_cells() {
            sim.grid.set_dynamic(cx, cy, true);
        }
    }
    sim.buildings.push(b);
}

#[derive(Debug, Clone)]
struct Capture {
    tick: u64,
    attack_target: Option<u32>,
    has_order: bool,
    chase_retry_tick: u64,
    strike_tick: Option<u64>,
    dist: f64,
    atk_off: (f64, f64),
    foe_off: (f64, f64),
    foe_hp: f64,
    atk_hp: f64,
    tc_hp: f64,
}

fn capture(sim: &Sim, atk: u32) -> Capture {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let a = ent(sim, atk);
    let fo = ent(sim, FOE);
    Capture {
        tick: sim.tick,
        attack_target: a.attack_target,
        has_order: a.order.is_some(),
        chase_retry_tick: a.chase_retry_tick,
        strike_tick: a.strike_tick,
        dist: f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y)),
        atk_off: (f(a.pos_x - tcx), f(a.pos_y - tcy)),
        foe_off: (f(fo.pos_x - tcx), f(fo.pos_y - tcy)),
        foe_hp: f(fo.hp),
        atk_hp: f(a.hp),
        tc_hp: f(ent(sim, 0).hp),
    }
}

fn print_capture(tag: &str, when: &str, c: &Capture) {
    eprintln!(
        "ENGAGESE CAPTURE {tag} {when}: tick={} attack_target={:?} has_order={} chase_retry_tick={} strike_tick={:?} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_hp={:.0} atk_hp={:.0} tc_hp={:.0}",
        c.tick, c.attack_target, c.has_order, c.chase_retry_tick, c.strike_tick,
        c.dist, c.atk_off.0, c.atk_off.1, c.foe_off.0, c.foe_off.1,
        c.foe_hp, c.atk_hp, c.tc_hp
    );
}

#[derive(Default, Debug)]
struct Trace {
    first_dmg: Option<u64>,
    drop_tick: Option<u64>,
    dead_tick: Option<u64>,
    min_dist: f64,
    max_stall_out: u64,
    max_stall_in: u64,
    idle: Option<Capture>,
    end: Option<Capture>,
}

impl Trace {
    fn live_shape(&self) -> bool {
        self.dead_tick.is_none()
            || self.max_stall_out >= 600
            || self.max_stall_in >= 600
            || self.drop_tick.is_some()
    }
}

fn run_window(tag: &str, sim: &mut Sim, atk: u32, window: u64) -> Trace {
    let foe_hp0 = f(ent(sim, FOE).hp);
    let mut tr = Trace { min_dist: f64::MAX, ..Default::default() };
    let mut stall_out = 0u64;
    let mut stall_in = 0u64;
    for t in 0..window {
        sim.tick();
        let a = ent(sim, atk);
        let fo = ent(sim, FOE);
        let dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
        if dist < tr.min_dist {
            tr.min_dist = dist;
        }
        if tr.first_dmg.is_none() && f(fo.hp) < foe_hp0 {
            tr.first_dmg = Some(t);
        }
        if tr.drop_tick.is_none() && !fo.dead && a.attack_target != Some(FOE) {
            tr.drop_tick = Some(t);
        }
        if !fo.dead && a.order.is_none() && tr.first_dmg.is_none() {
            if dist > MELEE {
                stall_out += 1;
                tr.max_stall_out = tr.max_stall_out.max(stall_out);
            } else {
                stall_in += 1;
                tr.max_stall_in = tr.max_stall_in.max(stall_in);
            }
        } else {
            stall_out = 0;
            stall_in = 0;
        }
        if tr.idle.is_none() && t >= 30 && a.order.is_none() && !fo.dead && tr.first_dmg.is_none() {
            let c = capture(sim, atk);
            print_capture(tag, "idle-moment", &c);
            tr.idle = Some(c);
        }
        if fo.dead {
            tr.dead_tick = Some(t);
            break;
        }
        if a.dead {
            eprintln!("ENGAGESE {tag} attacker DIED at window t={t}");
            break;
        }
    }
    let c = capture(sim, atk);
    print_capture(tag, "window-end", &c);
    tr.end = Some(c);
    eprintln!(
        "ENGAGESE {tag} first_dmg={:?} drop={:?} dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} live_shape={}",
        tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.min_dist,
        tr.max_stall_out, tr.max_stall_in, tr.live_shape()
    );
    tr
}

/// Stage the foe: spawn a Scout Cavalry 24 wu out on the
/// SE diagonal under a real Attack on the TC (entity 0),
/// settle 600 ticks. Asserts the foe still holds the TC
/// attack and the TC is draining; returns the settled
/// TC-relative offset.
fn stage_foe_se(sim: &mut Sim, tag: &str) -> (f64, f64) {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        sim,
        FOE,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(17.0),
        tcy + I16F16::from_num(17.0),
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..600 {
        sim.tick();
    }
    let fo = ent(sim, FOE);
    let settled = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
    let tc_hp = f(ent(sim, 0).hp);
    eprintln!(
        "ENGAGESE {tag} foe settled off=({:.2},{:.2}) tc_hp={:.0} foe_attack_target={:?} foe_hp={:.0}",
        settled.0, settled.1, tc_hp, fo.attack_target, f(fo.hp)
    );
    assert_eq!(fo.attack_target, Some(0), "{tag}: foe no longer holds the TC attack");
    assert!(tc_hp < 2400.0, "{tag}: foe is not draining the TC (hp {tc_hp:.0})");
    assert!(
        settled.0 > 1.5 && settled.1 > 1.5,
        "{tag}: foe did not settle at the SE pillar: {settled:?}"
    );
    settled
}

/// Spawn the Militia on face `atk_from`, Aggressive, real
/// Attack on the settled foe; run the window.
fn attack_from(tag: &str, sim: &mut Sim, atk_from: (f64, f64)) -> Trace {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        tcx + I16F16::from_num(atk_from.0),
        tcy + I16F16::from_num(atk_from.1),
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    run_window(tag, sim, ATK, 7_200)
}

fn stock(sim: &mut Sim) {
    sim.stockpile_mut(OWNER_PLAYER).add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER).add(ResourceKind::Gold, I16F16::from_num(500));
}

/// Place the Barracks east of the TC (tcrally's record,
/// cc+(2,-1)), set the rally to TC+`rally_off` BEFORE the
/// defender spawns, then train the Militia through the
/// real queue. Returns (defender id, spawn offset).
fn train_with_rally(sim: &mut Sim, tag: &str, rally_off: (f64, f64)) -> (u32, (f64, f64)) {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let cc = sim.grid.world_to_cell(tcx, tcy);
    place_record(sim, RAX, BuildingKind::Barracks, OWNER_PLAYER, (cc.0 + 2, cc.1 - 1));
    stock(sim);
    sim.push_command(Command::SetRally {
        building: Some(RAX),
        x: tcx + I16F16::from_num(rally_off.0),
        y: tcy + I16F16::from_num(rally_off.1),
    });
    let before: Vec<u32> = sim.entities().iter().map(|e| e.id).collect();
    assert!(sim.can_train_at(RAX, EntityKind::Militia), "{tag}: can_train_at refused");
    sim.push_command(Command::TrainAt { building: RAX, kind: EntityKind::Militia });
    for _ in 0..5_000 {
        sim.tick();
        if let Some(e) = sim.entities().iter()
            .find(|e| !before.contains(&e.id) && e.kind == EntityKind::Militia && !e.dead)
        {
            let off = (f(e.pos_x - tcx), f(e.pos_y - tcy));
            eprintln!("ENGAGESE {tag} trained id={} spawn_off=({:.2},{:.2})", e.id, off.0, off.1);
            return (e.id, off);
        }
    }
    panic!("{tag}: the trained Militia never spawned");
}

enum RallyOutcome {
    Arrived { tick: u64, off: (f64, f64) },
    Frozen { tick: u64, off: (f64, f64) },
    Engaged { tick: u64, off: (f64, f64) },
    Timeout { tick: u64, off: (f64, f64) },
}

/// Watch the rally walk and stop at arrival / confirmed
/// freeze / self-engagement / timeout, so the Attack is
/// issued from exactly the state the walk ended in.
fn await_rally(sim: &mut Sim, atk: u32, watch: u64) -> RallyOutcome {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let foe_hp0 = f(ent(sim, FOE).hp);
    let mut prev: Option<(f64, f64)> = None;
    let mut still: u64 = 0;
    for t in 0..watch {
        sim.tick();
        let a = ent(sim, atk);
        let fo = ent(sim, FOE);
        let off = (f(a.pos_x - tcx), f(a.pos_y - tcy));
        if a.dead || fo.dead || f(fo.hp) < foe_hp0 || a.attack_target == Some(FOE) {
            return RallyOutcome::Engaged { tick: t, off };
        }
        if a.order.is_none() {
            return RallyOutcome::Arrived { tick: t, off };
        }
        if prev == Some(off) {
            still += 1;
        } else {
            still = 0;
        }
        prev = Some(off);
        if still >= FREEZE_CONFIRM {
            return RallyOutcome::Frozen { tick: t, off };
        }
    }
    let a = ent(sim, atk);
    RallyOutcome::Timeout {
        tick: watch,
        off: (f(a.pos_x - tcx), f(a.pos_y - tcy)),
    }
}

fn describe(outcome: &RallyOutcome) -> (&'static str, u64, (f64, f64)) {
    match outcome {
        RallyOutcome::Arrived { tick, off } => ("arrived", *tick, *off),
        RallyOutcome::Frozen { tick, off } => ("frozen", *tick, *off),
        RallyOutcome::Engaged { tick, off } => ("engaged", *tick, *off),
        RallyOutcome::Timeout { tick, off } => ("timeout", *tick, *off),
    }
}

/// From the rally-end state, issue the real Attack on the
/// settled foe (Aggressive + can_attack asserted, as the
/// live right-click) and run the window.
fn attack_settled_foe(tag: &str, sim: &mut Sim, atk: u32, outcome: &RallyOutcome) -> Trace {
    let (how, rtick, roff) = describe(outcome);
    let c = capture(sim, atk);
    print_capture(tag, "issue-moment", &c);
    eprintln!(
        "ENGAGESE {tag} rally_end={how} rally_tick={rtick} stand_off=({:.2},{:.2}) dist={:.2} has_order={} foe_hp={:.0} atk_hp={:.0}",
        roff.0, roff.1, c.dist, c.has_order, c.foe_hp, c.atk_hp
    );
    if ent(sim, FOE).dead {
        eprintln!("ENGAGESE {tag} foe already dead at rally end — rally walk engaged it");
        return Trace {
            first_dmg: Some(0),
            dead_tick: Some(0),
            min_dist: c.dist,
            end: Some(c),
            ..Default::default()
        };
    }
    sim.push_command(Command::SetStance { entity: atk, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(atk, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: atk, target: FOE });
    run_window(tag, sim, atk, 7_200)
}

/// (1) MAIN — the live #481–#484 gesture: Scout settles at
/// the SE pillar under Attack on the TC; a Militia starting
/// EAST of the TC is ordered to Attack it. In the live
/// client this Militia stops at the TC's EAST edge and
/// idles forever while the TC drains.
#[test]
fn scout_settled_se_pillar_militia_from_east() {
    let tag = "main-SEfoe-Eatk";
    let mut sim = Sim::new(42);
    stage_foe_se(&mut sim, tag);
    let tr = attack_from(tag, &mut sim, (24.0, 0.0));
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (2) TRAINED — same SE-settled Scout; the defender is
/// TRAINED at a Barracks east of the TC with rally set
/// EAST before it spawns; after the rally walk ends, a
/// real Attack on the settled Scout.
#[test]
fn scout_settled_se_pillar_trained_east_barracks_rally_east() {
    let tag = "trained-SEfoe-raxE-rallyE";
    let mut sim = Sim::new(42);
    stage_foe_se(&mut sim, tag);
    let (atk, _spawn_off) = train_with_rally(&mut sim, tag, (18.0, 0.0));
    let outcome = await_rally(&mut sim, atk, 7_200);
    let tr = attack_settled_foe(tag, &mut sim, atk, &outcome);
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (3) MIRROR — same SE-settled Scout; a spawned Militia
/// starting SOUTH of the TC attacks it.
#[test]
fn scout_settled_se_pillar_militia_from_south() {
    let tag = "mirror-SEfoe-Satk";
    let mut sim = Sim::new(42);
    stage_foe_se(&mut sim, tag);
    let tr = attack_from(tag, &mut sim, (0.0, 18.0));
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}
