//! TCRALLY (2026-11-23): hidden-attacker scene (c) — the ONE
//! combination tcsouth §3c left untested.
//!
//! Live defect: an enemy Scout Cavalry settles under the
//! Town Center roof attacking the TC; a player Militia
//! ordered to attack it (right-click, attack ok:true)
//! sometimes arrives and idles without damage.
//!
//! Prior harness packages excluded: spawned defender vs
//! settled foe (tcsouth — every face engages, incl. the
//! north-settled Scout at exactly (0.00,-2.96) vs a spawned
//! Militia from the south, first damage t=330, dead t=1650),
//! and trained+rally defenders vs centre/east foes
//! (engageunder / engagefrozen — the engagefrozen Scout-E
//! face rallying south does not even freeze, and its Attack
//! engages, first damage t=310, dead t=1630).
//!
//! What none staged: the defender is TRAINED at a Barracks
//! with its rally point set SOUTH (set before the unit
//! spawns, through the real training queue), while the foe
//! is a Scout that ARRIVED from the north and settled at
//! the NORTH edge of the TC under a real `Command::Attack`
//! on the TC (settle ~600 ticks, foe at approx (0,-2.96),
//! attack_target=Some(TC), TC draining asserted) — then a
//! real `Command::Attack` on the settled Scout.
//!
//! Staging mirrors tcsouth (foe arrival, Capture/Trace
//! stall metrics, live_shape) and engagefrozen (Barracks
//! record at cc+(2,-1), stock, SetRally before TrainAt,
//! trained-id discovery, rally watch to arrival / freeze /
//! timeout). The Attack is issued from exactly the state
//! the rally walk ended in (rally arrival, confirmed
//! freeze, timeout, or — on the north-rally face — an
//! engagement the rally walk itself started). Per-face
//! trace: settled foe offset, rally outcome + stand,
//! first-damage tick, kill tick, drop tick, min distance,
//! worst out-of-reach / in-reach no-damage stall, and the
//! window-end capture.
//!
//! Faces: (1) MAIN — rally SOUTH vs north-settled Scout;
//! (2) MIRROR — rally NORTH vs the same north-settled
//! Scout; (3) ARCHER FOE — an Archer arriving from the
//! south and settling at the south side under Attack on
//! the TC, defender trained with rally SOUTH.
//!
//! OUTCOME (measured on base 7dc32a9, see handoff
//! `tcrally-build.md`): NOT REPRODUCED on any face.
//!   MAIN — Scout settles exactly (0.00,-2.96), TC
//!   draining (2400 -> 2397), attack_target=Some(TC).
//!   The trained Militia (spawn (4.00,-8.00)) completes
//!   the south rally (arrived t=521, stand (0.45,18.89),
//!   dist 21.85) and its Attack engages: first damage
//!   t=345, foe dead t=1665, drop=None, min_dist=2.98,
//!   max_stall_out=0 — ending at the TC centre
//!   (0.00,0.02), the same stand as tcsouth's spawned
//!   defender.
//!   MIRROR (rally north) — the rally walk itself
//!   auto-acquires the foe (attack_target=Some(FOE) at
//!   rally end, stand (0.21,-17.03)); the explicit
//!   Attack then kills: first damage t=214, dead
//!   t=1534, drop=None, min_dist=2.98.
//!   ARCHER — an Archer arriving from the south settles
//!   at (0.00,15.94) (its ranged TC stand), TC draining
//!   (2396); the south rally brings the Militia beside
//!   it (stand (0.45,18.89), dist 2.99, already
//!   auto-acquired); Attack: first damage t=28, dead
//!   t=868, drop=None, min_dist=2.99, attacker unhurt.
//! The trained + rally-south + north-settled combination
//! is therefore NOT the defect in the harness either:
//! with tcsouth and engagefrozen, every staged ordering
//! of defender provenance, rally direction and foe
//! arrival face engages. The remaining live-only
//! territory is unchanged — the client order path / the
//! exact live state at the idle moment. Per package
//! discipline NO production fix is made; these pins are
//! the deliverable.

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
const RAX: u32 = 9_950;
const MELEE: f64 = 3.0;
/// Consecutive position-identical ticks (order held) that
/// confirm a frozen rally stand (engagefrozen's measure).
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
        "TCRALLY CAPTURE {tag} {when}: tick={} attack_target={:?} has_order={} chase_retry_tick={} strike_tick={:?} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_hp={:.0} atk_hp={:.0} tc_hp={:.0}",
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
            eprintln!("TCRALLY {tag} attacker DIED at window t={t}");
            break;
        }
    }
    let c = capture(sim, atk);
    print_capture(tag, "window-end", &c);
    tr.end = Some(c);
    eprintln!(
        "TCRALLY {tag} first_dmg={:?} drop={:?} dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} live_shape={}",
        tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.min_dist,
        tr.max_stall_out, tr.max_stall_in, tr.live_shape()
    );
    tr
}

/// Stage the foe exactly as tcsouth: spawn 24 wu out on
/// face `foe_from` under a real Attack on the TC (entity
/// 0), settle 600 ticks. Asserts the foe still holds the
/// TC attack and the TC is draining; returns the settled
/// TC-relative offset.
fn stage_foe(sim: &mut Sim, tag: &str, kind: EntityKind, foe_from: (f64, f64)) -> (f64, f64) {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        sim,
        FOE,
        kind,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(foe_from.0),
        tcy + I16F16::from_num(foe_from.1),
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..600 {
        sim.tick();
    }
    let fo = ent(sim, FOE);
    let settled = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
    let tc_hp = f(ent(sim, 0).hp);
    eprintln!(
        "TCRALLY {tag} foe settled off=({:.2},{:.2}) tc_hp={:.0} foe_attack_target={:?} foe_hp={:.0}",
        settled.0, settled.1, tc_hp, fo.attack_target, f(fo.hp)
    );
    assert_eq!(fo.attack_target, Some(0), "{tag}: foe no longer holds the TC attack");
    assert!(tc_hp < 2400.0, "{tag}: foe is not draining the TC (hp {tc_hp:.0})");
    settled
}

fn stock(sim: &mut Sim) {
    sim.stockpile_mut(OWNER_PLAYER).add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER).add(ResourceKind::Gold, I16F16::from_num(500));
}

/// Place the Barracks (engagefrozen's record, cc+(2,-1)),
/// set the rally to TC+`rally_off` BEFORE the defender
/// spawns, then train the Militia through the real queue.
/// Returns (defender id, spawn offset).
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
            eprintln!("TCRALLY {tag} trained id={} spawn_off=({:.2},{:.2})", e.id, off.0, off.1);
            return (e.id, off);
        }
    }
    panic!("{tag}: the trained Militia never spawned");
}

enum RallyOutcome {
    /// Rally order completed (unit arrived at the point).
    Arrived { tick: u64, off: (f64, f64) },
    /// Position identical for FREEZE_CONFIRM ticks with
    /// the rally order still held (the pillar freeze).
    Frozen { tick: u64, off: (f64, f64) },
    /// The rally walk itself engaged the foe (foe damaged
    /// or targeted) before any Attack was issued.
    Engaged { tick: u64, off: (f64, f64) },
    /// Neither within the watch window.
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

/// Shared tail: from the rally-end state, issue the real
/// Attack on the settled foe (Aggressive + can_attack
/// asserted, as the live right-click) and run the window.
/// If the rally walk already killed the foe, that is an
/// engagement outcome, recorded as such.
fn attack_settled_foe(tag: &str, sim: &mut Sim, atk: u32, outcome: &RallyOutcome) -> Trace {
    let (how, rtick, roff) = describe(outcome);
    let c = capture(sim, atk);
    print_capture(tag, "issue-moment", &c);
    eprintln!(
        "TCRALLY {tag} rally_end={how} rally_tick={rtick} stand_off=({:.2},{:.2}) dist={:.2} has_order={} foe_hp={:.0} atk_hp={:.0}",
        roff.0, roff.1, c.dist, c.has_order, c.foe_hp, c.atk_hp
    );
    if ent(sim, FOE).dead {
        eprintln!("TCRALLY {tag} foe already dead at rally end — rally walk engaged it");
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

/// (1) MAIN — the §3c combination: Scout ARRIVES from the
/// north and settles at the north edge; the defender is
/// TRAINED at the Barracks with rally set SOUTH before it
/// spawns; after the rally walk ends, a real Attack on the
/// settled Scout.
#[test]
fn trained_rally_south_vs_scout_settled_north() {
    let tag = "main-rallyS-Nscout";
    let mut sim = Sim::new(42);
    let settled = stage_foe(&mut sim, tag, EntityKind::ScoutCavalry, (0.0, -24.0));
    assert!(
        settled.1 < -2.0 && settled.0.abs() < 1.5,
        "{tag}: foe did not settle at the north edge: {settled:?}"
    );
    let (atk, _spawn_off) = train_with_rally(&mut sim, tag, (0.0, 18.0));
    let outcome = await_rally(&mut sim, atk, 7_200);
    let tr = attack_settled_foe(tag, &mut sim, atk, &outcome);
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (2) MIRROR — same north-settled Scout, defender trained
/// with rally set NORTH (toward the foe's side).
#[test]
fn trained_rally_north_vs_scout_settled_north() {
    let tag = "mirror-rallyN-Nscout";
    let mut sim = Sim::new(42);
    let settled = stage_foe(&mut sim, tag, EntityKind::ScoutCavalry, (0.0, -24.0));
    assert!(
        settled.1 < -2.0 && settled.0.abs() < 1.5,
        "{tag}: foe did not settle at the north edge: {settled:?}"
    );
    let (atk, _spawn_off) = train_with_rally(&mut sim, tag, (0.0, -18.0));
    let outcome = await_rally(&mut sim, atk, 7_200);
    let tr = attack_settled_foe(tag, &mut sim, atk, &outcome);
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (3) ARCHER FOE — an Archer arrives from the SOUTH under
/// Attack on the TC and settles at the south side; the
/// defender is trained with rally set SOUTH, then Attacks
/// the settled Archer.
#[test]
fn trained_rally_south_vs_archer_settled_south() {
    let tag = "archerS-rallyS";
    let mut sim = Sim::new(42);
    let settled = stage_foe(&mut sim, tag, EntityKind::Archer, (0.0, 24.0));
    assert!(
        settled.1 > 2.0 && settled.0.abs() < 1.5,
        "{tag}: archer did not settle at the south side: {settled:?}"
    );
    let (atk, _spawn_off) = train_with_rally(&mut sim, tag, (0.0, 18.0));
    let outcome = await_rally(&mut sim, atk, 7_200);
    let tr = attack_settled_foe(tag, &mut sim, atk, &outcome);
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}
