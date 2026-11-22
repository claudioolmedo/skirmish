//! ENGAGEPILLAR (2026-11-22): the run #471 geometry — a foe
//! settled at the TC EAST PILLAR / ANNEX EDGE cell (foe at the
//! footprint EDGE, not the centre), draining the TC, and a
//! TRAINED defender walking FROM ITS RALLY/SPAWN point to a
//! real `Command::Attack` on that foe. Live #471 (real client,
//! twice): enemy Scout id 28 and Spearman id 77 under the TC
//! roof / east annex draining the TC (~2.5 HP/s in instance
//! 2); the player's Militia (Aggressive) was right-clicked on
//! the foe — occlog PERFECT pick (picked=foe, afterGate=foe,
//! fromOutline:true), issue {"t":"attack",...} ok:true in BOTH
//! instances — yet the Militia walked only to the TC EAST
//! EDGE/PILLAR and went idle, HP 40/40 unchanged, never
//! entered under the roof, never fought, while the foe stayed
//! adjacent under the east annex.
//!
//! Prior staging (engageunder branch (c)) pinned a StandGround
//! foe at (6,0)/(4,0)/(0,6) with ONE Barracks placement
//! (anchor cc+4) and ONE rally (south) and it engaged. What it
//! never combined, and what this suite stages systematically:
//!   (A) a foe that ARRIVES from the east under a real Attack
//!       order on the TC and SETTLES wherever the TC-attack
//!       stand actually is (recorded, not assumed), TC
//!       verifiably draining, then the trained/rally defender
//!       attacks it — Scout and Spearman faces of #471;
//!   (B) a foe PINNED (StandGround) at the east pillar/annex
//!       EDGE cells across a systematic offset matrix —
//!       x in {6..10}, y in {-4..+4} on the east sector —
//!       with the trained defender arriving from rally SOUTH
//!       and rally EAST (the east rally makes the defender's
//!       path END at the footprint edge from outside, the
//!       exact live approach), Barracks at cc+4;
//!   (C) an ANNEX building east of the TC (a House record +
//!       dynamic footprint block, the engagepersist idiom):
//!       its blocked footprint abuts the TC's east edge, the
//!       foe stands at the pillar cells west/east of it, and
//!       the trained defender (Barracks south-east, rally
//!       south) attacks — the defender's path must round or
//!       cross the annex block to reach the foe.
//!
//! Every order on the path under test is a real `Command`
//! (foe Attack on the TC, SetRally, TrainAt, SetStance,
//! Attack — `can_attack` asserted where the live issue
//! returned ok:true). Positions / building records are scene
//! selection only.
//!
//! Per-tick trace per scene: attacker position (TC-relative),
//! distance to foe, attack_target, move order, chase_retry,
//! strike state, foe HP, TC HP — plus the idle-moment capture
//! (first tick: no move order, foe alive, no damage yet) and
//! the worst out-of-reach / in-reach no-damage stall.
//!
//! OUTCOME (measured on base b38ccfe, see handoff
//! `engagepillar-build.md`): the explicit Attack execution
//! at the pillar does NOT reproduce — all 10 pinned-matrix
//! offsets, all 3 annex scenes and all 4 draining scenes
//! engage (max_stall_out = 0, drop = None everywhere). Two
//! measured facts reshape the hunt:
//!   1. A foe under a real Attack order on the TC NEVER
//!      settles at the east edge: Scout-E settles (2.96,0),
//!      Spearman-E (2.93,0), Scout-SE (2.11,2.11),
//!      Spearman-NE (2.10,-2.09) — centre distance ~2.9–3.0
//!      in every approach. The live foe "at the east annex
//!      draining the TC" has no sim counterpart as a
//!      TC-entity attacker; in the live client it must be
//!      striking a building-shaped target (range from the
//!      footprint edge) or standing in a state the harness
//!      cannot spawn.
//!   2. A REAL STALL DID REPRODUCE, one step earlier in the
//!      live path: with a Spearman settled at (2.93,0)
//!      draining the TC, the Barracks-trained Militia's
//!      RALLY walk south freezes at (3.97,-1.20) — dist
//!      1.58 from the foe, rally order HELD, both HP
//!      untouched (40/40, 45/45) — position identical over
//!      a 600-tick probe and over a full 7,200-tick wait.
//!      The Scout-E face of the same scene walks through
//!      and arrives. From the frozen stand, a real Attack
//!      order engages immediately (first damage t=30, foe
//!      dead t=1350): the stall is in the MOVE layer passing
//!      beside a settled Spearman at the TC's east side,
//!      not in attack execution. `rally_stall_beside_
//!      settled_spearman_then_attack` pins both halves.

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
const ANNEX: u32 = 9_951;
const MELEE: f64 = 3.0;

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
    dist: f64,
    atk_off: (f64, f64),
    foe_off: (f64, f64),
    foe_cell: (u32, u32),
    foe_cell_blocked: bool,
    atk_cell_blocked: bool,
    foe_hp: f64,
    tc_hp: f64,
}

fn capture(sim: &Sim, atk: u32) -> Capture {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let a = ent(sim, atk);
    let fo = ent(sim, FOE);
    let cell = sim.grid.world_to_cell(fo.pos_x, fo.pos_y);
    let acell = sim.grid.world_to_cell(a.pos_x, a.pos_y);
    Capture {
        tick: sim.tick,
        attack_target: a.attack_target,
        has_order: a.order.is_some(),
        chase_retry_tick: a.chase_retry_tick,
        suppress: a.suppress_separation,
        strike_tick: a.strike_tick,
        dist: f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y)),
        atk_off: (f(a.pos_x - tcx), f(a.pos_y - tcy)),
        foe_off: (f(fo.pos_x - tcx), f(fo.pos_y - tcy)),
        foe_cell: cell,
        foe_cell_blocked: sim.grid.is_blocked(cell.0, cell.1),
        atk_cell_blocked: sim.grid.is_blocked(acell.0, acell.1),
        foe_hp: f(fo.hp),
        tc_hp: f(ent(sim, 0).hp),
    }
}

fn print_capture(tag: &str, when: &str, c: &Capture) {
    eprintln!(
        "ENGAGEPILLAR CAPTURE {tag} {when}: tick={} attack_target={:?} has_order={} chase_retry_tick={} suppress={} strike_tick={:?} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_cell={:?} foe_cell_blocked={} atk_cell_blocked={} foe_hp={:.0} tc_hp={:.0}",
        c.tick, c.attack_target, c.has_order, c.chase_retry_tick, c.suppress,
        c.strike_tick, c.dist, c.atk_off.0, c.atk_off.1, c.foe_off.0, c.foe_off.1,
        c.foe_cell, c.foe_cell_blocked, c.atk_cell_blocked, c.foe_hp, c.tc_hp
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
    }
    let c = capture(sim, atk);
    print_capture(tag, "window-end", &c);
    tr.end = Some(c);
    eprintln!(
        "ENGAGEPILLAR {tag} first_dmg={:?} drop={:?} dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} live_shape={}",
        tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.min_dist,
        tr.max_stall_out, tr.max_stall_in, tr.live_shape()
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

fn wait_rally_arrival(sim: &mut Sim, atk: u32) -> bool {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    for i in 0..7_200 {
        sim.tick();
        if i % 1200 == 1199 {
            let a = ent(sim, atk);
            let fo = ent(sim, FOE);
            eprintln!(
                "ENGAGEPILLAR RALLYWAIT t={} atk_off=({:.2},{:.2}) atk_hp={:.0} atk_dead={} attack_target={:?} has_order={} foe_off=({:.2},{:.2}) foe_hp={:.0} foe_dead={}",
                i + 1, f(a.pos_x - tcx), f(a.pos_y - tcy), f(a.hp), a.dead,
                a.attack_target, a.order.is_some(),
                f(fo.pos_x - tcx), f(fo.pos_y - tcy), f(fo.hp), fo.dead
            );
        }
        if ent(sim, atk).order.is_none() {
            return true;
        }
    }
    false
}

fn order_attack(sim: &mut Sim, tag: &str, atk: u32) {
    sim.push_command(Command::SetStance { entity: atk, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(atk, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: atk, target: FOE });
}

/// (A) THE #471 ARRIVAL: foe spawns EAST, walks in under a
/// real Attack order on the TC, settles (wherever the TC
/// stand is — recorded), TC draining asserted; Barracks east
/// (engageunder placement cc+2), rally SOUTH, trained
/// defender attacks the settled foe. Scout + Spearman.
#[test]
fn draining_foe_from_east_then_trained_defender() {
    let mut failures = Vec::new();
    for (name, kind, spawn_off) in [
        ("scout-E", EntityKind::ScoutCavalry, (24.0, 0.0)),
        ("spear-E", EntityKind::Spearman, (24.0, 0.0)),
        ("scout-SE", EntityKind::ScoutCavalry, (20.0, 14.0)),
        ("spear-NE", EntityKind::Spearman, (20.0, -14.0)),
    ] {
        let tag = format!("draining-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, kind, OWNER_OPPONENT,
              tcx + I16F16::from_num(spawn_off.0), tcy + I16F16::from_num(spawn_off.1));
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..600 { sim.tick(); }
        let fo = ent(&sim, FOE);
        let settled = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
        let tc_hp_settled = f(ent(&sim, 0).hp);
        eprintln!("ENGAGEPILLAR {tag} foe settled off=({:.2},{:.2}) tc_hp={:.0} foe_attack_target={:?}",
                  settled.0, settled.1, tc_hp_settled, fo.attack_target);
        if tc_hp_settled >= 2400.0 {
            failures.push(format!("{tag}: foe not draining TC (hp {tc_hp_settled:.0})"));
            continue;
        }
        let cc = sim.grid.world_to_cell(tcx, tcy);
        place_record(&mut sim, RAX, BuildingKind::Barracks, OWNER_PLAYER, (cc.0 + 2, cc.1 - 1));
        stock(&mut sim);
        sim.push_command(Command::SetRally { building: Some(RAX), x: tcx, y: tcy + I16F16::from_num(18) });
        let (atk, spawn) = train_militia(&mut sim, tcx, tcy);
        eprintln!("ENGAGEPILLAR {tag} trained id={atk} spawn_off=({:.2},{:.2})", spawn.0, spawn.1);
        if !wait_rally_arrival(&mut sim, atk) {
            // MEASURED (spear-E): the rally walk itself stalls
            // — the Militia freezes at (3.97,-1.20), 1.59 wu
            // from the foe, order held, HP 40/40, for the full
            // 7200-tick wait. Record it, then continue with
            // the live sequence: the player right-clicks the
            // foe from wherever the unit stands.
            let c = capture(&sim, atk);
            print_capture(&tag, "rally-stall", &c);
            eprintln!("ENGAGEPILLAR {tag} RALLY STALL atk_off=({:.2},{:.2}) dist={:.2} has_order={}",
                      c.atk_off.0, c.atk_off.1, c.dist, c.has_order);
        }
        // Record the foe's cell at the order moment (it may
        // have been displaced by the Barracks footprint).
        let c0 = capture(&sim, atk);
        print_capture(&tag, "order-moment", &c0);
        order_attack(&mut sim, &tag, atk);
        let tr = run_window(&tag, &mut sim, atk, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: settled={settled:?} order_foe_off={:?} {tr:?}", c0.foe_off));
        }
    }
    assert!(failures.is_empty(), "draining failures:\n{}", failures.join("\n"));
}

/// (B) THE PILLAR MATRIX: foe pinned at east-sector edge
/// cells, trained defender, rally SOUTH or EAST alternating.
/// Barracks at cc+4 so no staged cell is footprint-covered.
#[test]
fn pinned_east_pillar_matrix_trained_defender() {
    let offsets: [(f64, f64); 10] = [
        (8.0, 0.0), (8.0, 2.0), (8.0, -2.0), (8.0, 4.0), (8.0, -4.0),
        (9.0, 0.0), (10.0, 0.0), (7.0, 0.0), (6.0, 3.0), (6.0, -3.0),
    ];
    let mut failures = Vec::new();
    let mut results = Vec::new();
    for (i, foe_off) in offsets.iter().enumerate() {
        let rally_east = i % 2 == 1;
        let kind = if i == 0 || i == 3 { EntityKind::ScoutCavalry } else { EntityKind::Spearman };
        let tag = format!("pillar-({},{})-rally{}", foe_off.0, foe_off.1, if rally_east { "E" } else { "S" });
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        let cc = sim.grid.world_to_cell(tcx, tcy);
        place_record(&mut sim, RAX, BuildingKind::Barracks, OWNER_PLAYER, (cc.0 + 4, cc.1 - 1));
        stock(&mut sim);
        spawn(&mut sim, FOE, kind, OWNER_OPPONENT,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(foe_off.1));
        sim.push_command(Command::SetStance { entity: FOE, stance: Stance::StandGround });
        let rally = if rally_east { (26.0, 0.0) } else { (0.0, 18.0) };
        sim.push_command(Command::SetRally {
            building: Some(RAX),
            x: tcx + I16F16::from_num(rally.0),
            y: tcy + I16F16::from_num(rally.1),
        });
        let (atk, _spawn) = train_militia(&mut sim, tcx, tcy);
        if !wait_rally_arrival(&mut sim, atk) {
            failures.push(format!("{tag}: rally never arrived"));
            continue;
        }
        let fo = ent(&sim, FOE);
        let foe_now = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
        if (foe_now.0 - foe_off.0).abs() > 0.6 || (foe_now.1 - foe_off.1).abs() > 0.6 {
            failures.push(format!("{tag}: foe left staged cell {foe_now:?} vs {foe_off:?}"));
            continue;
        }
        order_attack(&mut sim, &tag, atk);
        let tr = run_window(&tag, &mut sim, atk, 7_200);
        results.push(format!("{tag}: first_dmg={:?} dead={:?} min_dist={:.2} stall_out={} stall_in={}",
                             tr.first_dmg, tr.dead_tick, tr.min_dist, tr.max_stall_out, tr.max_stall_in));
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    for r in &results { eprintln!("ENGAGEPILLAR MATRIX {r}"); }
    assert!(failures.is_empty(), "pillar matrix failures:\n{}", failures.join("\n"));
}

/// (A2) THE RALLY STALL, pinned: Spearman settled at the TC
/// (stand (2.93,0), draining), trained Militia rallies south.
/// Measured: the Militia freezes at (3.97,-1.20) — dist 1.59
/// from the foe — with its rally move order HELD and HP
/// 40/40, position identical over a 600-tick probe. Then the
/// live sequence completes: a real Attack order on the foe
/// from the stalled position. This test pins BOTH halves:
/// the rally stall (position frozen >= 600 ticks, order
/// held, no damage either way) and the post-Attack outcome
/// recorded in the trace line.
#[test]
fn rally_stall_beside_settled_spearman_then_attack() {
    let tag = "rallystall-spear-E";
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT,
          tcx + I16F16::from_num(24), tcy);
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..600 { sim.tick(); }
    assert!(f(ent(&sim, 0).hp) < 2400.0, "{tag}: TC not draining");
    let cc = sim.grid.world_to_cell(tcx, tcy);
    place_record(&mut sim, RAX, BuildingKind::Barracks, OWNER_PLAYER, (cc.0 + 2, cc.1 - 1));
    stock(&mut sim);
    sim.push_command(Command::SetRally { building: Some(RAX), x: tcx, y: tcy + I16F16::from_num(18) });
    let (atk, _spawn) = train_militia(&mut sim, tcx, tcy);
    // Probe: after the walk has had time to reach the TC,
    // is the unit frozen with its order held?
    for _ in 0..1_200 { sim.tick(); }
    let p0 = { let a = ent(&sim, atk); (f(a.pos_x - tcx), f(a.pos_y - tcy)) };
    let mut frozen = true;
    for _ in 0..600 {
        sim.tick();
        let a = ent(&sim, atk);
        let p = (f(a.pos_x - tcx), f(a.pos_y - tcy));
        if (p.0 - p0.0).abs() > 0.01 || (p.1 - p0.1).abs() > 0.01 {
            frozen = false;
        }
    }
    let c = capture(&sim, atk);
    print_capture(tag, "stall-probe", &c);
    eprintln!("ENGAGEPILLAR {tag} frozen={frozen} stall_off=({:.2},{:.2}) dist={:.2} has_order={} atk_hp={:.0} foe_hp={:.0}",
              c.atk_off.0, c.atk_off.1, c.dist, c.has_order,
              f(ent(&sim, atk).hp), f(ent(&sim, FOE).hp));
    assert!(frozen && c.has_order,
            "{tag}: rally stall did NOT reproduce (frozen={frozen}, has_order={}, off={:?})", c.has_order, c.atk_off);
    // The live sequence: right-click the foe from the stall.
    order_attack(&mut sim, tag, atk);
    let tr = run_window(tag, &mut sim, atk, 7_200);
    eprintln!("ENGAGEPILLAR {tag} post-attack live_shape={} first_dmg={:?} dead={:?}",
              tr.live_shape(), tr.first_dmg, tr.dead_tick);
    // Pin the measured post-attack outcome in the handoff;
    // the assertion is that the trace was recorded — the
    // stall itself is the reproduced defect half.
}

/// (C) ANNEX: a House record east of the TC (anchor cc+2,
/// cc-1: its 2x2 blocked footprint starts at the TC's east
/// edge cells). Foe pinned at the pillar cells on the
/// annex's west edge (6,0) and east edge (14,0); defender
/// trained at a Barracks south-east (cc+1, cc+3), rally
/// south, then Attack.
#[test]
fn annex_house_east_trained_defender() {
    let mut failures = Vec::new();
    for (name, foe_off) in [("annexW-(6,0)", (6.0, 0.0)), ("annexE-(14,0)", (14.0, 0.0)), ("annexW-(6,4)", (6.0, 4.0))] {
        let tag = format!("{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        let cc = sim.grid.world_to_cell(tcx, tcy);
        place_record(&mut sim, ANNEX, BuildingKind::House, OWNER_PLAYER, (cc.0 + 2, cc.1 - 1));
        place_record(&mut sim, RAX, BuildingKind::Barracks, OWNER_PLAYER, (cc.0 + 1, cc.1 + 3));
        stock(&mut sim);
        spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(foe_off.1));
        sim.push_command(Command::SetStance { entity: FOE, stance: Stance::StandGround });
        sim.push_command(Command::SetRally { building: Some(RAX), x: tcx, y: tcy + I16F16::from_num(18) });
        let (atk, _spawn) = train_militia(&mut sim, tcx, tcy);
        if !wait_rally_arrival(&mut sim, atk) {
            failures.push(format!("{tag}: rally never arrived"));
            continue;
        }
        let fo = ent(&sim, FOE);
        let foe_now = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
        let c0 = capture(&sim, atk);
        print_capture(&tag, "order-moment", &c0);
        eprintln!("ENGAGEPILLAR {tag} foe at order moment off=({:.2},{:.2}) staged={foe_off:?} foe_cell_blocked={}", foe_now.0, foe_now.1, c0.foe_cell_blocked);
        order_attack(&mut sim, &tag, atk);
        let tr = run_window(&tag, &mut sim, atk, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: foe_now={foe_now:?} {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "annex failures:\n{}", failures.join("\n"));
}
