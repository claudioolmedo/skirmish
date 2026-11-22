//! ENGAGEUNDER (2026-11-22): the two UNTESTED live differences
//! of the hidden-attacker defect, run #463 (live client, twice).
//!
//! Prior narrowing (engagefoe / engageexec2 / engageexec3 /
//! engagegroup / engagepersist) staged only SPAWNED or PARKED
//! defenders: a foe at the TC centre engages when the defender
//! is spawned nearby or group-ordered in staging geometry, the
//! clear-on-A*-fail order drop is fixed (engagepersist /
//! occorder23), and Branch B (fallback hold on a blocked foe
//! cell) is unreachable because the TC footprint cells are NOT
//! grid-blocked (BLD-16 pass-under).
//!
//! What none of them staged — the exact live #463 path:
//!   (a) the defender is TRAINED at a real Barracks EAST of the
//!       TC through the real production queue (`TrainAt`),
//!       walks to a RALLY point (real `SetRally`) SOUTH of the
//!       TC, and only THEN is given the Attack order — from
//!       OUTSIDE the TC footprint, against a Scout standing at
//!       the TC CENTRE draining the TC. Its chase path must
//!       cross into the footprint and end beside the foe; live
//!       it ended at the TC EAST EDGE / PILLAR and the Militia
//!       stood idle 121–157 s, foe HP frozen at 45/45.
//!       Variants: order lands after rally arrival, order lands
//!       MID-rally-walk, no rally at all (unit idles at its
//!       Barracks ring spawn — itself at the TC east edge), and
//!       rally points on the E / S / W approach sides.
//!   (b) the villager face: a villager ALREADY ADJACENT
//!       (<= 1 cell) to a 1-HP enemy Spearman standing at the
//!       TC EAST PILLAR cell, Attack order accepted — live,
//!       BOTH stood idle 60+ s and the finishing blow never
//!       landed, while open-ground combat worked. Variants:
//!       foe idle / foe ordered on the TC, distances 1.5 / 2.9
//!       / 3.0, foe at pillar (+6,0) / (+4,0) / centre, and a
//!       real starting villager walked to adjacency by a real
//!       Move order before the Attack.
//!   (c) trained Militia vs a foe standing AT the pillar /
//!       edge (not the centre), from the Barracks + rally path.
//!
//! Every order on the path under test is a real `Command`
//! (foe Attack on the TC, SetRally, TrainAt, SetStance,
//! Move, Attack — `can_attack` asserted where the live issue
//! returned ok:true). Positions / building records / the
//! Spearman's 1 HP are scene selection only (the
//! engagepersist `place_record` idiom: record + dynamic grid
//! block, the exact two effects of a completed building).
//!
//! Per-tick trace per scene: attacker position (TC-relative),
//! distance to foe, attack_target, move order, chase_retry,
//! strike state, foe HP, TC HP — plus the idle-moment capture
//! (first tick: no move order, foe alive, no damage yet) and
//! the worst out-of-reach / in-reach no-damage stall.
//!
//! OUTCOME (measured on base f677f43, see handoff
//! `engageunder-build.md`): NO branch reproduces. The trained
//! Militia (dynamic id 30 in these scenes) spawns at the
//! Barracks ring (TC-relative (4,-8)), walks its rally, and on
//! the Attack order closes INTO the footprint to dist
//! 2.95–3.00 in every timing/side variant (first damage
//! t=16–558 after the order, foe dead t=1336–1878,
//! max_stall_out = 0, drop = None everywhere). The villager
//! face kills the 1-HP pillar Spearman with the first landed
//! hit (t=38 at gaps 1.5/2.9/3.0; t=158 when the foe walks
//! itself into the TC disc first). Two staging facts were
//! measured along the way: an un-pinned idle opponent unit
//! takes AI orders and walks off its cell, and a foe spawned
//! on a cell a neighbouring building's footprint covers is
//! displacement-ejected — both are scene-selection traps, not
//! the live defect. Per package discipline NO production fix
//! is made; these pins are the deliverable, and the remaining
//! live-only territory is the client order path / the exact
//! live state at the idle moment (see the handoff).

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

/// A completed building record at anchor cell `(cx, cy)` plus
/// the dynamic grid block its footprint carries in the live
/// sim (engagepersist `place_record` idiom).
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
    order_target: Option<(f64, f64)>,
    chase_retry_tick: u64,
    suppress: bool,
    strike_tick: Option<u64>,
    next_attack_tick: u64,
    attack_engaged: bool,
    contained: bool,
    dist: f64,
    atk_off: (f64, f64),
    foe_off: (f64, f64),
    foe_cell: (u32, u32),
    foe_cell_blocked: bool,
    foe_hp: f64,
    tc_hp: f64,
}

fn capture(sim: &Sim, atk: u32) -> Capture {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let a = ent(sim, atk);
    let fo = ent(sim, FOE);
    let cell = sim.grid.world_to_cell(fo.pos_x, fo.pos_y);
    Capture {
        tick: sim.tick,
        attack_target: a.attack_target,
        has_order: a.order.is_some(),
        order_target: a.order.as_ref().map(|o| (f(o.target_x), f(o.target_y))),
        chase_retry_tick: a.chase_retry_tick,
        suppress: a.suppress_separation,
        strike_tick: a.strike_tick,
        next_attack_tick: a.next_attack_tick,
        attack_engaged: a.attack_engaged,
        contained: a.is_contained(),
        dist: f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y)),
        atk_off: (f(a.pos_x - tcx), f(a.pos_y - tcy)),
        foe_off: (f(fo.pos_x - tcx), f(fo.pos_y - tcy)),
        foe_cell: cell,
        foe_cell_blocked: sim.grid.is_blocked(cell.0, cell.1),
        foe_hp: f(fo.hp),
        tc_hp: f(ent(sim, 0).hp),
    }
}

fn print_capture(tag: &str, when: &str, c: &Capture) {
    eprintln!(
        "ENGAGEUNDER CAPTURE {tag} {when}: tick={} attack_target={:?} has_order={} order_target={:?} chase_retry_tick={} suppress={} strike_tick={:?} next_attack_tick={} engaged={} contained={} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_cell={:?} foe_cell_blocked={} foe_hp={:.0} tc_hp={:.0}",
        c.tick, c.attack_target, c.has_order, c.order_target, c.chase_retry_tick,
        c.suppress, c.strike_tick, c.next_attack_tick, c.attack_engaged, c.contained,
        c.dist, c.atk_off.0, c.atk_off.1, c.foe_off.0, c.foe_off.1,
        c.foe_cell, c.foe_cell_blocked, c.foe_hp, c.tc_hp
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
    /// The live #463 shape: foe never dies, or a >= 600-tick
    /// idle stall (out of reach OR in reach without a strike),
    /// or the target dropped.
    fn live_shape(&self) -> bool {
        self.dead_tick.is_none()
            || self.max_stall_out >= 600
            || self.max_stall_in >= 600
            || self.drop_tick.is_some()
    }
}

/// Run the window after the attack order on attacker `atk`.
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
        "ENGAGEUNDER {tag} first_dmg={:?} drop={:?} dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} live_shape={}",
        tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.min_dist,
        tr.max_stall_out, tr.max_stall_in, tr.live_shape()
    );
    tr
}

/// Common setup: foe Scout at the TC centre ordered on the TC
/// by real command, settled 300 ticks, TC verifiably draining;
/// a real Barracks placed EAST of the TC (anchor cc+2, cc-1:
/// its footprint starts on the first cell east of the TC
/// footprint). Returns (tcx, tcy).
fn setup_foe_and_rax(sim: &mut Sim) -> (I16F16, I16F16) {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tcx, tcy);
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..300 {
        sim.tick();
    }
    assert!(
        f(ent(sim, 0).hp) < 2400.0,
        "setup: foe at the TC centre is not draining the TC (hp {:.0})",
        f(ent(sim, 0).hp)
    );
    let cc = sim.grid.world_to_cell(tcx, tcy);
    place_record(sim, RAX, BuildingKind::Barracks, OWNER_PLAYER, (cc.0 + 2, cc.1 - 1));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(500));
    (tcx, tcy)
}

/// Train one Militia at the Barracks through the real queue;
/// returns the new unit's (dynamic) id and its spawn offset.
fn train_militia(sim: &mut Sim, tcx: I16F16, tcy: I16F16) -> (u32, (f64, f64)) {
    let before: Vec<u32> = sim.entities().iter().map(|e| e.id).collect();
    assert!(
        sim.can_train_at(RAX, EntityKind::Militia),
        "setup: can_train_at refused the Militia"
    );
    sim.push_command(Command::TrainAt { building: RAX, kind: EntityKind::Militia });
    let mut born = None;
    for _ in 0..5_000 {
        sim.tick();
        if let Some(e) = sim
            .entities()
            .iter()
            .find(|e| !before.contains(&e.id) && e.kind == EntityKind::Militia && !e.dead)
        {
            born = Some((e.id, (f(e.pos_x - tcx), f(e.pos_y - tcy))));
            break;
        }
    }
    let (id, off) = born.expect("the trained Militia never spawned");
    eprintln!("ENGAGEUNDER trained militia id={id} spawn_off=({:.2},{:.2})", off.0, off.1);
    (id, off)
}

fn order_attack(sim: &mut Sim, tag: &str, atk: u32, aggressive: bool) {
    if aggressive {
        sim.push_command(Command::SetStance { entity: atk, stance: Stance::Aggressive });
        sim.tick();
    }
    assert!(sim.can_attack(atk, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: atk, target: FOE });
}

/// (a) THE LIVE PATH: trained at the east Barracks, rally
/// SOUTH of the TC, attack order after rally arrival.
#[test]
fn trained_rally_south_then_attack_centre_foe() {
    let tag = "trained-rallyS-arrived";
    let mut sim = Sim::new(42);
    let (tcx, tcy) = setup_foe_and_rax(&mut sim);
    // Rally south of the TC by real command, before training.
    sim.push_command(Command::SetRally {
        building: Some(RAX),
        x: tcx,
        y: tcy + I16F16::from_num(18),
    });
    let (atk, _spawn_off) = train_militia(&mut sim, tcx, tcy);
    // Walk the rally out: wait until the unit holds no order.
    let mut arrived = false;
    for _ in 0..7_200 {
        sim.tick();
        if ent(&sim, atk).order.is_none() {
            arrived = true;
            break;
        }
    }
    let a = ent(&sim, atk);
    eprintln!(
        "ENGAGEUNDER {tag} rally arrived={} atk_off=({:.2},{:.2}) has_order={}",
        arrived,
        f(a.pos_x - tcx),
        f(a.pos_y - tcy),
        a.order.is_some()
    );
    assert!(arrived, "{tag}: the trained Militia never finished its rally walk");
    order_attack(&mut sim, tag, atk, true);
    let tr = run_window(tag, &mut sim, atk, 7_200);
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (a2) The attack order lands MID-RALLY-WALK (the unit still
/// holds its rally move order, southbound around the TC /
/// Barracks footprints) — the impatient-player timing.
#[test]
fn trained_attack_order_mid_rally_walk() {
    let tag = "trained-rallyS-midwalk";
    let mut sim = Sim::new(42);
    let (tcx, tcy) = setup_foe_and_rax(&mut sim);
    sim.push_command(Command::SetRally {
        building: Some(RAX),
        x: tcx,
        y: tcy + I16F16::from_num(18),
    });
    let (atk, _spawn_off) = train_militia(&mut sim, tcx, tcy);
    // Let it walk a little, then interrupt with the attack.
    for _ in 0..60 {
        sim.tick();
    }
    let a = ent(&sim, atk);
    eprintln!(
        "ENGAGEUNDER {tag} at order moment: atk_off=({:.2},{:.2}) has_order={} order_target={:?}",
        f(a.pos_x - tcx),
        f(a.pos_y - tcy),
        a.order.is_some(),
        a.order.as_ref().map(|o| (f(o.target_x), f(o.target_y)))
    );
    order_attack(&mut sim, tag, atk, true);
    let tr = run_window(tag, &mut sim, atk, 7_200);
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (a3) No rally: the trained Militia idles at its Barracks
/// ring spawn (at the TC's east edge — the live idle spot),
/// then the attack order lands.
#[test]
fn trained_no_rally_then_attack_centre_foe() {
    let tag = "trained-norally";
    let mut sim = Sim::new(42);
    let (tcx, tcy) = setup_foe_and_rax(&mut sim);
    let (atk, spawn_off) = train_militia(&mut sim, tcx, tcy);
    for _ in 0..120 {
        sim.tick();
    }
    let a = ent(&sim, atk);
    eprintln!(
        "ENGAGEUNDER {tag} idle-at-spawn off=({:.2},{:.2}) spawn_off=({:.2},{:.2}) has_order={}",
        f(a.pos_x - tcx),
        f(a.pos_y - tcy),
        spawn_off.0,
        spawn_off.1,
        a.order.is_some()
    );
    order_attack(&mut sim, tag, atk, true);
    let tr = run_window(tag, &mut sim, atk, 7_200);
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (a4) Rally points on the E and W approach sides (the S
/// side is the main scene): the trained unit rallies out to
/// the side, arrives, then attacks the centre foe.
#[test]
fn trained_rally_east_and_west_then_attack() {
    let mut failures = Vec::new();
    for (name, rally_off) in [("E", (26.0, 0.0)), ("W", (-26.0, 0.0))] {
        let tag = format!("trained-rally{name}-arrived");
        let mut sim = Sim::new(42);
        let (tcx, tcy) = setup_foe_and_rax(&mut sim);
        sim.push_command(Command::SetRally {
            building: Some(RAX),
            x: tcx + I16F16::from_num(rally_off.0),
            y: tcy + I16F16::from_num(rally_off.1),
        });
        let (atk, _spawn_off) = train_militia(&mut sim, tcx, tcy);
        let mut arrived = false;
        for _ in 0..7_200 {
            sim.tick();
            if ent(&sim, atk).order.is_none() {
                arrived = true;
                break;
            }
        }
        eprintln!("ENGAGEUNDER {tag} rally arrived={arrived}");
        if !arrived {
            failures.push(format!("{tag}: rally never arrived"));
            continue;
        }
        order_attack(&mut sim, &tag, atk, true);
        let tr = run_window(&tag, &mut sim, atk, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "rally-side failures:\n{}", failures.join("\n"));
}

/// (c) Trained Militia (Barracks + south rally path) vs a foe
/// standing AT the east pillar / edge — the foe is NOT on the
/// TC (it stands at the pillar cell, full HP, no order), so
/// the chase target is a point inside the footprint near its
/// east edge rather than the centre.
#[test]
fn trained_attack_foe_at_pillar_and_edge() {
    let mut failures = Vec::new();
    for (name, foe_off) in [("pillar+6", (6.0, 0.0)), ("edge+4", (4.0, 0.0)), ("edgeS+6", (0.0, 6.0))] {
        let tag = format!("trained-vs-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        let cc = sim.grid.world_to_cell(tcx, tcy);
        // This scene's Barracks stands two cells further east
        // than scene (a)'s: at anchor cc+2 its footprint covers
        // the +6 pillar cell itself, and a foe spawned there is
        // displacement-ejected before the order (measured in
        // this suite's second run: staged (6,0) read (4,-4) at
        // the order moment). At cc+4 the pillar/edge cells are
        // clear ground between the TC and the Barracks.
        place_record(&mut sim, RAX, BuildingKind::Barracks, OWNER_PLAYER, (cc.0 + 4, cc.1 - 1));
        sim.stockpile_mut(OWNER_PLAYER)
            .add(ResourceKind::Food, I16F16::from_num(500));
        sim.stockpile_mut(OWNER_PLAYER)
            .add(ResourceKind::Gold, I16F16::from_num(500));
        spawn(
            &mut sim,
            FOE,
            EntityKind::Spearman,
            OWNER_OPPONENT,
            tcx + I16F16::from_num(foe_off.0),
            tcy + I16F16::from_num(foe_off.1),
        );
        // Pin the foe at the pillar/edge: Stand Ground by real
        // command (the engageexec2 scene-2 idiom) — an idle
        // opponent unit otherwise takes AI orders and walks
        // off the staged cell (measured in this suite's first
        // run: the un-pinned foe drifted ~16 wu south and the
        // fight happened at the rally point instead).
        sim.push_command(Command::SetStance { entity: FOE, stance: Stance::StandGround });
        sim.push_command(Command::SetRally {
            building: Some(RAX),
            x: tcx,
            y: tcy + I16F16::from_num(18),
        });
        let (atk, _spawn_off) = train_militia(&mut sim, tcx, tcy);
        let mut arrived = false;
        for _ in 0..7_200 {
            sim.tick();
            if ent(&sim, atk).order.is_none() {
                arrived = true;
                break;
            }
        }
        if !arrived {
            failures.push(format!("{tag}: rally never arrived"));
            continue;
        }
        // The geometry pin: the foe must still stand at its
        // staged pillar/edge offset when the order lands.
        let fo = ent(&sim, FOE);
        let foe_now = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
        eprintln!("ENGAGEUNDER {tag} foe at order moment off=({:.2},{:.2}) (staged {:?})", foe_now.0, foe_now.1, foe_off);
        if (foe_now.0 - foe_off.0).abs() > 0.6 || (foe_now.1 - foe_off.1).abs() > 0.6 {
            failures.push(format!("{tag}: foe left its staged cell: {foe_now:?} vs {foe_off:?}"));
            continue;
        }
        order_attack(&mut sim, &tag, atk, true);
        let tr = run_window(&tag, &mut sim, atk, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "pillar/edge failures:\n{}", failures.join("\n"));
}

/// (b) THE VILLAGER FACE: villager adjacent to a 1-HP Spearman
/// at the TC east pillar. One landed hit kills; the live
/// defect is that NO hit ever lands with both idle.
#[test]
fn villager_adjacent_to_one_hp_spearman_at_pillar() {
    let mut failures = Vec::new();
    // (foe_off, gap east of the foe, foe ordered on the TC?)
    for (name, foe_off, gap, foe_on_tc) in [
        ("pillar+6-gap1.5-idle", (6.0, 0.0), 1.5, false),
        ("pillar+6-gap1.5-ontc", (6.0, 0.0), 1.5, true),
        ("pillar+6-gap2.9-idle", (6.0, 0.0), 2.9, false),
        ("pillar+6-gap3.0-idle", (6.0, 0.0), 3.0, false),
        ("pillar+4-gap1.5-idle", (4.0, 0.0), 1.5, false),
        ("centre-gap1.5-idle", (0.0, 0.0), 1.5, false),
    ] {
        let tag = format!("villager-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(
            &mut sim,
            FOE,
            EntityKind::Spearman,
            OWNER_OPPONENT,
            tcx + I16F16::from_num(foe_off.0),
            tcy + I16F16::from_num(foe_off.1),
        );
        // Scene selection: the live Spearman sat at 1/45 HP.
        {
            let idx = sim.entity_index(FOE).unwrap();
            sim.entities[idx].hp = I16F16::from_num(1);
        }
        if foe_on_tc {
            sim.push_command(Command::Attack { entity: FOE, target: 0 });
        }
        const VIL: u32 = 9_802;
        spawn(
            &mut sim,
            VIL,
            EntityKind::Villager,
            OWNER_PLAYER,
            tcx + I16F16::from_num(foe_off.0 + gap),
            tcy + I16F16::from_num(foe_off.1),
        );
        let d0 = f(steer::vec_len(
            ent(&sim, FOE).pos_x - ent(&sim, VIL).pos_x,
            ent(&sim, FOE).pos_y - ent(&sim, VIL).pos_y,
        ));
        assert!(sim.can_attack(VIL, FOE), "{tag}: can_attack refused the live order");
        sim.push_command(Command::Attack { entity: VIL, target: FOE });
        eprintln!("ENGAGEUNDER {tag} initial dist={d0:.2}");
        // Trace with the villager as the attacker id.
        let foe_hp0 = f(ent(&sim, FOE).hp);
        let mut first_dmg = None;
        let mut dead = None;
        let mut drop = None;
        let mut stall_in = 0u64;
        let mut max_stall_in = 0u64;
        let mut idle_cap = None;
        for t in 0..3_600u64 {
            sim.tick();
            let v = ent(&sim, VIL);
            let fo = ent(&sim, FOE);
            if first_dmg.is_none() && f(fo.hp) < foe_hp0 {
                first_dmg = Some(t);
            }
            if drop.is_none() && !fo.dead && v.attack_target != Some(FOE) {
                drop = Some(t);
            }
            let dist = f(steer::vec_len(fo.pos_x - v.pos_x, fo.pos_y - v.pos_y));
            if !fo.dead && v.order.is_none() && first_dmg.is_none() && dist <= MELEE {
                stall_in += 1;
                max_stall_in = max_stall_in.max(stall_in);
            } else {
                stall_in = 0;
            }
            if idle_cap.is_none() && t >= 30 && v.order.is_none() && !fo.dead && first_dmg.is_none() {
                idle_cap = Some(capture(&sim, VIL));
            }
            if fo.dead {
                dead = Some(t);
                break;
            }
        }
        if let Some(c) = &idle_cap {
            print_capture(&tag, "idle-moment", c);
        }
        let c = capture(&sim, VIL);
        print_capture(&tag, "window-end", &c);
        eprintln!(
            "ENGAGEUNDER {tag} first_dmg={first_dmg:?} drop={drop:?} dead={dead:?} max_stall_in={max_stall_in} vil_hp={:.0}",
            f(ent(&sim, VIL).hp)
        );
        if dead.is_none() || drop.is_some() || max_stall_in >= 600 {
            failures.push(format!(
                "{tag}: first_dmg={first_dmg:?} drop={drop:?} dead={dead:?} max_stall_in={max_stall_in} end={c:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "villager-face failures:\n{}", failures.join("\n"));
}

/// (b2) A REAL starting villager (from `Sim::new`, not a
/// spawn) walks to adjacency at the pillar by a real Move
/// order, then receives the Attack order — the live unit's
/// actual provenance.
#[test]
fn starting_villager_walked_to_pillar_then_attack() {
    let tag = "villager-starting-walked";
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        FOE,
        EntityKind::Spearman,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(6),
        tcy,
    );
    {
        let idx = sim.entity_index(FOE).unwrap();
        sim.entities[idx].hp = I16F16::from_num(1);
    }
    let vil = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
        .unwrap()
        .id;
    sim.push_command(Command::Move {
        entity: vil,
        x: tcx + I16F16::from_num(7.5),
        y: tcy,
    });
    let mut arrived = false;
    for _ in 0..7_200 {
        sim.tick();
        if ent(&sim, vil).order.is_none() {
            arrived = true;
            break;
        }
    }
    let d0 = f(steer::vec_len(
        ent(&sim, FOE).pos_x - ent(&sim, vil).pos_x,
        ent(&sim, FOE).pos_y - ent(&sim, vil).pos_y,
    ));
    eprintln!("ENGAGEUNDER {tag} walked arrived={arrived} dist_at_order={d0:.2}");
    assert!(arrived, "{tag}: the starting villager never arrived beside the pillar");
    assert!(sim.can_attack(vil, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: vil, target: FOE });
    let mut dead = None;
    for t in 0..3_600u64 {
        sim.tick();
        if ent(&sim, FOE).dead {
            dead = Some(t);
            break;
        }
    }
    let c = capture(&sim, vil);
    print_capture(tag, "window-end", &c);
    eprintln!("ENGAGEUNDER {tag} dead={dead:?}");
    assert!(dead.is_some(), "{tag}: LIVE SHAPE REPRODUCED — no finishing blow: {c:?}");
}
