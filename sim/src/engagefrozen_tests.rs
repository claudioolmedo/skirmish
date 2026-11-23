//! ENGAGEFROZEN (2026-11-23): the engagepillar rally stall,
//! one step further along the live path. engagepillar pinned
//! that a Barracks-trained Militia rallying SOUTH past a
//! Spearman settled at the TC (stand (2.93,0), draining)
//! freezes at (3.97,-1.20) — dist 1.59 from the foe, rally
//! order HELD, both HP untouched — and that a real Attack
//! issued AFTER a completed 600-tick probe engages from that
//! stand (first damage t=30, foe dead t=1350). What no suite
//! combined before this one:
//!   (A) the settled foe is a SCOUT CAVALRY (the #478 kind)
//!       under a real Attack on the TC, and the defender's
//!       Attack is issued AT THE FIRST CONFIRMED FROZEN TICK
//!       (position identical for 120 consecutive ticks, order
//!       held) — mid-freeze, not after a full probe;
//!   (B) from the same frozen state (Spearman foe), an
//!       AttackMove toward the foe instead of a targeted
//!       Attack — does the frozen unit pick the foe up and
//!       fight, or stay frozen / walk past?
//!   (C) CONTROL: Spearman foe, identical mid-freeze Attack
//!       timing as (A) — separates "mid-freeze timing" from
//!       "Scout foe kind" as the variable.
//!
//! Every order on the path under test is a real `Command`
//! (foe Attack on the TC, SetRally, TrainAt, SetStance,
//! Attack / AttackMove — `can_attack` / `can_attack_move`
//! asserted where the live issue would return ok). Positions
//! / building records are scene selection only. Helpers are
//! reused verbatim from engagepillar_tests where possible;
//! the Capture here adds velocity magnitude (the frozen
//! state's discriminant: order held + velocity zero).
//!
//! Per-scene trace: first damage tick, foe death tick,
//! attack_target drop tick, worst position-frozen no-damage
//! stall after the order, attacker/foe positions, foe HP,
//! TC HP.
//!
//! OUTCOME (measured on base f4122f7, see handoff
//! `engagefrozen2-build.md`): NO stall or order drop
//! reproduces from the frozen state — pins only, no fix.
//!   (A) Scout settles (2.96,0), TC draining (2397); the
//!       Militia does NOT freeze against the Scout face —
//!       the rally completes (stand (0.45,18.89), order
//!       cleared) — and the Attack from the post-rally stand
//!       engages: first damage t=310, foe dead t=1630,
//!       drop=None.
//!   (C) Spearman control: the freeze reproduces EXACTLY
//!       (stand (3.97,-1.20), dist 1.58, vel 0.000, order
//!       held, confirmed 233 ticks into the watch); Attack
//!       AT THE FIRST FROZEN TICK engages identically to
//!       the pillar post-probe Attack: first damage t=30,
//!       foe dead t=1350, drop=None. Mid-freeze timing is
//!       not a variable.
//!   (B) AttackMove from the same frozen stand: acquired
//!       t=0, first damage t=30, foe dead t=1350 — the
//!       frozen unit picks the foe up immediately.
//! The remaining max_stall_frozen=119 in every window is
//! the in-reach strike cadence (position static, damage
//! progressing), not a stall. The rally freeze itself
//! remains the pillar-pinned MOVE-layer defect; attack
//! execution out of it is clean on every face tried.

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
/// confirm the frozen/bump state.
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
    suppress: bool,
    strike_tick: Option<u64>,
    vel: f64,
    dist: f64,
    atk_off: (f64, f64),
    foe_off: (f64, f64),
    foe_hp: f64,
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
        suppress: a.suppress_separation,
        strike_tick: a.strike_tick,
        vel: f(steer::vec_len(a.vel_x, a.vel_y)),
        dist: f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y)),
        atk_off: (f(a.pos_x - tcx), f(a.pos_y - tcy)),
        foe_off: (f(fo.pos_x - tcx), f(fo.pos_y - tcy)),
        foe_hp: f(fo.hp),
        tc_hp: f(ent(sim, 0).hp),
    }
}

fn print_capture(tag: &str, when: &str, c: &Capture) {
    eprintln!(
        "ENGAGEFROZEN CAPTURE {tag} {when}: tick={} attack_target={:?} has_order={} chase_retry_tick={} suppress={} strike_tick={:?} vel={:.3} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0}",
        c.tick, c.attack_target, c.has_order, c.chase_retry_tick, c.suppress,
        c.strike_tick, c.vel, c.dist, c.atk_off.0, c.atk_off.1, c.foe_off.0,
        c.foe_off.1, c.foe_hp, c.tc_hp
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
    /// Worst streak of ticks with the attacker's position
    /// identical AND the foe's HP unchanged (foe alive) —
    /// the frozen-state stall measure after the order.
    max_stall_frozen: u64,
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
    let mut stall_frozen = 0u64;
    let mut prev_pos: Option<(f64, f64)> = None;
    let mut prev_foe_hp = foe_hp0;
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
        let pos = (f(a.pos_x), f(a.pos_y));
        if !fo.dead && prev_pos == Some(pos) && f(fo.hp) == prev_foe_hp {
            stall_frozen += 1;
            tr.max_stall_frozen = tr.max_stall_frozen.max(stall_frozen);
        } else {
            stall_frozen = 0;
        }
        prev_pos = Some(pos);
        prev_foe_hp = f(fo.hp);
        if tr.idle.is_none() && t >= 30 && a.order.is_none() && !fo.dead && tr.first_dmg.is_none() {
            let c = capture(sim, atk);
            print_capture(tag, "idle-moment", &c);
            tr.idle = Some(c);
        }
        if fo.dead {
            tr.dead_tick = Some(t);
            break;
        }
    }
    let c = capture(sim, atk);
    print_capture(tag, "window-end", &c);
    tr.end = Some(c);
    eprintln!(
        "ENGAGEFROZEN {tag} first_dmg={:?} drop={:?} dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} max_stall_frozen={} live_shape={}",
        tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.min_dist,
        tr.max_stall_out, tr.max_stall_in, tr.max_stall_frozen, tr.live_shape()
    );
    tr
}

fn stock(sim: &mut Sim) {
    sim.stockpile_mut(OWNER_PLAYER).add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER).add(ResourceKind::Gold, I16F16::from_num(500));
}

fn train_militia(sim: &mut Sim, tcx: I16F16, tcy: I16F16) -> (u32, (f64, f64)) {
    let before: Vec<u32> = sim.entities().iter().map(|e| e.id).collect();
    assert!(sim.can_train_at(RAX, EntityKind::Militia), "setup: can_train_at refused");
    sim.push_command(Command::TrainAt { building: RAX, kind: EntityKind::Militia });
    for _ in 0..5_000 {
        sim.tick();
        if let Some(e) = sim.entities().iter()
            .find(|e| !before.contains(&e.id) && e.kind == EntityKind::Militia && !e.dead)
        {
            return (e.id, (f(e.pos_x - tcx), f(e.pos_y - tcy)));
        }
    }
    panic!("the trained Militia never spawned");
}

/// The shared staging: foe spawns EAST at TC+(24,0) under a
/// real Attack on the TC, settles over 600 ticks (TC draining
/// asserted); Barracks at cc+(2,-1), rally SOUTH TC+(0,18);
/// a Militia is trained. Returns (sim, atk, settle offset).
fn stage_settled_foe(tag: &str, kind: EntityKind) -> (Sim, u32, (f64, f64)) {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, kind, OWNER_OPPONENT,
          tcx + I16F16::from_num(24), tcy);
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..600 { sim.tick(); }
    let fo = ent(&sim, FOE);
    let settled = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
    let tc_hp = f(ent(&sim, 0).hp);
    eprintln!("ENGAGEFROZEN {tag} foe settled off=({:.2},{:.2}) tc_hp={:.0} foe_attack_target={:?}",
              settled.0, settled.1, tc_hp, fo.attack_target);
    assert!(tc_hp < 2400.0, "{tag}: foe not draining TC (hp {tc_hp:.0})");
    let cc = sim.grid.world_to_cell(tcx, tcy);
    place_record(&mut sim, RAX, BuildingKind::Barracks, OWNER_PLAYER, (cc.0 + 2, cc.1 - 1));
    stock(&mut sim);
    sim.push_command(Command::SetRally { building: Some(RAX), x: tcx, y: tcy + I16F16::from_num(18) });
    let (atk, spawn_off) = train_militia(&mut sim, tcx, tcy);
    eprintln!("ENGAGEFROZEN {tag} trained id={atk} spawn_off=({:.2},{:.2})", spawn_off.0, spawn_off.1);
    (sim, atk, settled)
}

enum FreezeOutcome {
    /// Position identical for FREEZE_CONFIRM consecutive
    /// ticks with the rally order still held.
    Frozen { tick: u64, off: (f64, f64) },
    /// The rally order completed (unit arrived) before any
    /// freeze was confirmed.
    Arrived { tick: u64, off: (f64, f64) },
    /// Neither within the watch window (still walking, or
    /// the unit died).
    Timeout { tick: u64, off: (f64, f64) },
}

/// Watch the rally walk tick by tick and stop AT THE FIRST
/// CONFIRMED FROZEN TICK (or at rally arrival), so the next
/// order is issued mid-freeze exactly as staged.
fn await_freeze_or_arrival(sim: &mut Sim, atk: u32, watch: u64) -> FreezeOutcome {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let mut prev: Option<(f64, f64)> = None;
    let mut still: u64 = 0;
    for t in 0..watch {
        sim.tick();
        let a = ent(sim, atk);
        let off = (f(a.pos_x - tcx), f(a.pos_y - tcy));
        if a.dead {
            return FreezeOutcome::Timeout { tick: t, off };
        }
        if a.order.is_none() {
            return FreezeOutcome::Arrived { tick: t, off };
        }
        if prev == Some(off) {
            still += 1;
        } else {
            still = 0;
        }
        prev = Some(off);
        if still >= FREEZE_CONFIRM {
            return FreezeOutcome::Frozen { tick: t, off };
        }
    }
    let a = ent(sim, atk);
    FreezeOutcome::Timeout {
        tick: watch,
        off: (f(a.pos_x - tcx), f(a.pos_y - tcy)),
    }
}

fn describe(outcome: &FreezeOutcome) -> (bool, u64, (f64, f64)) {
    match outcome {
        FreezeOutcome::Frozen { tick, off } => (true, *tick, *off),
        FreezeOutcome::Arrived { tick, off } => (false, *tick, *off),
        FreezeOutcome::Timeout { tick, off } => (false, *tick, *off),
    }
}

/// (A) SCOUT FOE + ATTACK AT THE FIRST FROZEN TICK. The
/// Scout-E face of the pillar draining scene walked through
/// the rally (no freeze); whether it freezes or arrives is
/// recorded, and the Attack is issued from exactly the state
/// the walk ended in — frozen stand or rally arrival.
#[test]
fn scout_settled_attack_at_first_frozen_tick() {
    let tag = "frozenA-scout-attack";
    let (mut sim, atk, settled) = stage_settled_foe(tag, EntityKind::ScoutCavalry);
    let outcome = await_freeze_or_arrival(&mut sim, atk, 7_200);
    let (frozen, ftick, foff) = describe(&outcome);
    let c = capture(&sim, atk);
    print_capture(tag, "issue-moment", &c);
    eprintln!("ENGAGEFROZEN {tag} settled={settled:?} frozen={frozen} freeze_tick={ftick} stand_off=({:.2},{:.2}) vel={:.3} dist={:.2} has_order={} atk_hp={:.0} foe_hp={:.0}",
              foff.0, foff.1, c.vel, c.dist, c.has_order, f(ent(&sim, atk).hp), c.foe_hp);
    sim.push_command(Command::SetStance { entity: atk, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(atk, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: atk, target: FOE });
    let tr = run_window(tag, &mut sim, atk, 7_200);
    assert!(!tr.live_shape(),
            "{tag}: Attack from the {} state did not engage: {tr:?}",
            if frozen { "frozen" } else { "post-rally" });
}

/// (C) CONTROL: SPEARMAN FOE, IDENTICAL MID-FREEZE ATTACK
/// TIMING. The pillar suite pins this freeze (Spearman
/// settled (2.93,0), Militia frozen (3.97,-1.20)); here the
/// Attack goes out at the first confirmed frozen tick
/// instead of after a completed 600-tick probe.
#[test]
fn spearman_settled_attack_at_first_frozen_tick_control() {
    let tag = "frozenC-spear-attack";
    let (mut sim, atk, settled) = stage_settled_foe(tag, EntityKind::Spearman);
    let outcome = await_freeze_or_arrival(&mut sim, atk, 7_200);
    let (frozen, ftick, foff) = describe(&outcome);
    let c = capture(&sim, atk);
    print_capture(tag, "issue-moment", &c);
    eprintln!("ENGAGEFROZEN {tag} settled={settled:?} frozen={frozen} freeze_tick={ftick} stand_off=({:.2},{:.2}) vel={:.3} dist={:.2} has_order={} atk_hp={:.0} foe_hp={:.0}",
              foff.0, foff.1, c.vel, c.dist, c.has_order, f(ent(&sim, atk).hp), c.foe_hp);
    assert!(frozen && c.has_order,
            "{tag}: the pinned rally freeze did NOT reproduce (frozen={frozen}, has_order={}, off={:?})",
            c.has_order, c.atk_off);
    sim.push_command(Command::SetStance { entity: atk, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(atk, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: atk, target: FOE });
    let tr = run_window(tag, &mut sim, atk, 7_200);
    assert!(!tr.live_shape(),
            "{tag}: Attack at the first frozen tick did not engage: {tr:?}");
}

/// (B) ATTACKMOVE FROM THE FROZEN STATE: Spearman foe (the
/// freezing face), and at the first confirmed frozen tick an
/// AttackMove toward the foe's position replaces the rally
/// order. Records whether the unit acquires and kills the
/// foe, walks past it, or stays frozen. (attack_target is
/// legitimately None until acquisition under AttackMove, so
/// the drop measure does not apply here; engagement is foe
/// damage / death and the frozen-stall measure.)
#[test]
fn spearman_settled_attackmove_from_frozen() {
    let tag = "frozenB-spear-amove";
    let (mut sim, atk, settled) = stage_settled_foe(tag, EntityKind::Spearman);
    let outcome = await_freeze_or_arrival(&mut sim, atk, 7_200);
    let (frozen, ftick, foff) = describe(&outcome);
    let c = capture(&sim, atk);
    print_capture(tag, "issue-moment", &c);
    eprintln!("ENGAGEFROZEN {tag} settled={settled:?} frozen={frozen} freeze_tick={ftick} stand_off=({:.2},{:.2}) vel={:.3} dist={:.2} has_order={} atk_hp={:.0} foe_hp={:.0}",
              foff.0, foff.1, c.vel, c.dist, c.has_order, f(ent(&sim, atk).hp), c.foe_hp);
    assert!(frozen && c.has_order,
            "{tag}: the pinned rally freeze did NOT reproduce (frozen={frozen}, has_order={}, off={:?})",
            c.has_order, c.atk_off);
    sim.push_command(Command::SetStance { entity: atk, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack_move(atk), "{tag}: can_attack_move refused the live order");
    let fo = ent(&sim, FOE);
    let (fx, fy) = (fo.pos_x, fo.pos_y);
    sim.push_command(Command::AttackMove { entity: atk, x: fx, y: fy });
    // Window measured on foe damage/death + frozen stall
    // only (no targeted order to drop).
    let foe_hp0 = f(ent(&sim, FOE).hp);
    let mut first_dmg: Option<u64> = None;
    let mut dead: Option<u64> = None;
    let mut acquired: Option<u64> = None;
    let mut stall_frozen = 0u64;
    let mut max_stall_frozen = 0u64;
    let mut prev_pos: Option<(f64, f64)> = None;
    let mut prev_foe_hp = foe_hp0;
    for t in 0..7_200u64 {
        sim.tick();
        let a = ent(&sim, atk);
        let fo = ent(&sim, FOE);
        if first_dmg.is_none() && f(fo.hp) < foe_hp0 {
            first_dmg = Some(t);
        }
        if acquired.is_none() && a.attack_target == Some(FOE) {
            acquired = Some(t);
        }
        let pos = (f(a.pos_x), f(a.pos_y));
        if !fo.dead && prev_pos == Some(pos) && f(fo.hp) == prev_foe_hp {
            stall_frozen += 1;
            max_stall_frozen = max_stall_frozen.max(stall_frozen);
        } else {
            stall_frozen = 0;
        }
        prev_pos = Some(pos);
        prev_foe_hp = f(fo.hp);
        if fo.dead {
            dead = Some(t);
            break;
        }
    }
    let c = capture(&sim, atk);
    print_capture(tag, "window-end", &c);
    eprintln!("ENGAGEFROZEN {tag} amove first_dmg={first_dmg:?} acquired={acquired:?} dead={dead:?} max_stall_frozen={max_stall_frozen} end_atk_off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0}",
              c.atk_off.0, c.atk_off.1, c.foe_hp, c.tc_hp);
    assert!(dead.is_some(),
            "{tag}: AttackMove from the frozen state never killed the foe (first_dmg={first_dmg:?} acquired={acquired:?} max_stall_frozen={max_stall_frozen})");
}
