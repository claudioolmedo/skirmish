//! ENGAGEEXEC3 (2026-11-21): the LIVE hidden-attacker geometry.
//! engageexec2 staged its foe at the TC EDGE (settled off
//! (0.00, 2.93) after walking in) on a grid-blocked cell and
//! proved Branch B unreachable there because
//! `displace_out_of_blocked` ejects foes off blocked cells.
//! But the live defect (bot-loop run #460) is different: an
//! enemy Scout stands INSIDE the Town Center footprint —
//! units pass under the TC per BLD-16, so footprint cells are
//! NOT grid-blocked — draining the TC, and a player Militia
//! ordered to attack it (issue ok:true) walks to the TC edge
//! and goes idle without fighting (HP 40/40, no blow).
//!
//! Staging (documented per the package): the foe is SPAWNED
//! at the TC centre / an interior footprint offset (scene
//! selection only, the engagepersist idiom) and then ordered
//! by a REAL `Command::Attack` on the TC entity (id 0). Where
//! the spawn offset is within MELEE_REACH of the TC centre the
//! foe stands there and drains from that exact interior cell;
//! the suite records the settled offset so any inward drift
//! is visible. The Militia is spawned outside, set Aggressive
//! and ordered by REAL `Command::Attack` on the foe.
//!
//! Capture at the idle moment (first tick the attacker holds
//! no move order, foe alive, no damage yet): attacker move
//! order, attack_target, distance to foe, foe cell, foe-cell
//! blocked flag, chase_retry_tick, suppress_separation — then
//! damage is watched for a further 600 ticks minimum (the
//! window runs far longer; first-damage tick is recorded).
//!
//! OUTCOME (measured on base 8c912d0, see handoff
//! `engageexec3-build.md`): NO branch confirms in this
//! geometry either. In EVERY scene — Scout at the exact TC
//! centre from all four sides, Spearman/Scout at seven
//! interior footprint offsets, default-stance Militia, and a
//! dropoff-ring villager crowd — the Militia walks INTO the
//! footprint, stands at dist 2.94..2.99 from the foe (inside
//! MELEE_REACH), the idle-moment capture is the ordinary
//! fighting stand (attack_target retained, chase_retry_tick
//! 0, foe cell unblocked), first damage lands t=347..440 and
//! the foe dies t=1667..1760. The sim's chase has no defect
//! at the TC interior for a lone foe: the live #460 shape
//! (Militia idle at the TC EAST EDGE, dist >> reach) is not
//! reproduced by centre/interior staging, so the live
//! capture's distinguishing ingredient is something this
//! staging does not contain (e.g. the foe's exact live
//! position/state or live-only client/order context) — a
//! live vigia capture remains the discriminant. Per package
//! discipline NO production fix is made; these pins are the
//! deliverable.

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

#[derive(Debug, Clone)]
struct Capture {
    tick: u64,
    attack_target: Option<u32>,
    order_target: Option<(f64, f64)>,
    chase_retry_tick: u64,
    suppress: bool,
    dist: f64,
    atk_off: (f64, f64),
    foe_off: (f64, f64),
    foe_cell: (u32, u32),
    foe_cell_blocked: bool,
    foe_hp: f64,
    tc_hp: f64,
}

fn capture(sim: &Sim) -> Capture {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let a = ent(sim, ATK);
    let fo = ent(sim, FOE);
    let cell = sim.grid.world_to_cell(fo.pos_x, fo.pos_y);
    Capture {
        tick: sim.tick,
        attack_target: a.attack_target,
        order_target: a.order.as_ref().map(|o| (f(o.target_x), f(o.target_y))),
        chase_retry_tick: a.chase_retry_tick,
        suppress: a.suppress_separation,
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
        "ENGAGEEXEC3 CAPTURE {tag} {when}: tick={} attack_target={:?} order_target={:?} chase_retry_tick={} suppress={} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_cell={:?} foe_cell_blocked={} foe_hp={:.0} tc_hp={:.0}",
        c.tick, c.attack_target, c.order_target, c.chase_retry_tick, c.suppress,
        c.dist, c.atk_off.0, c.atk_off.1, c.foe_off.0, c.foe_off.1,
        c.foe_cell, c.foe_cell_blocked, c.foe_hp, c.tc_hp
    );
}

#[derive(Default, Debug)]
struct Trace {
    settled_off: (f64, f64),
    first_dmg: Option<u64>,
    dmg_after_idle_600: Option<bool>,
    drop_tick: Option<u64>,
    dead_tick: Option<u64>,
    min_dist: f64,
    idle: Option<Capture>,
    end: Option<Capture>,
}

/// Foe of `foe_kind` spawned at TC-relative `foe_off`, ordered
/// Attack on the TC by real command, settled 300 ticks (TC
/// must be draining). Militia spawned at `atk_off`, ordered
/// Attack on the foe by real commands. Runs `window` ticks.
fn interior_scene(
    tag: &str,
    foe_kind: EntityKind,
    foe_off: (f64, f64),
    atk_off: (f64, f64),
    window: u64,
) -> Trace {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        FOE,
        foe_kind,
        OWNER_OPPONENT,
        tcx + I16F16::from_num(foe_off.0),
        tcy + I16F16::from_num(foe_off.1),
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..300 {
        sim.tick();
    }
    let foe = ent(&sim, FOE);
    let settled = (f(foe.pos_x - tcx), f(foe.pos_y - tcy));
    assert!(
        f(ent(&sim, 0).hp) < 2400.0,
        "{tag}: foe at {settled:?} is not draining the TC (hp {:.0})",
        f(ent(&sim, 0).hp)
    );
    eprintln!(
        "ENGAGEEXEC3 {tag} foe settled off=({:.2},{:.2}) cell={:?} cell_blocked={}",
        settled.0,
        settled.1,
        sim.grid.world_to_cell(foe.pos_x, foe.pos_y),
        sim.grid.is_blocked(
            sim.grid.world_to_cell(foe.pos_x, foe.pos_y).0,
            sim.grid.world_to_cell(foe.pos_x, foe.pos_y).1
        )
    );
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        tcx + I16F16::from_num(atk_off.0),
        tcy + I16F16::from_num(atk_off.1),
    );
    sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
    sim.tick();
    assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
    let foe_hp0 = f(ent(&sim, FOE).hp);
    let mut tr = Trace {
        settled_off: settled,
        min_dist: f64::MAX,
        ..Default::default()
    };
    for t in 0..window {
        sim.tick();
        let a = ent(&sim, ATK);
        let fo = ent(&sim, FOE);
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
        if tr.idle.is_none() && t >= 30 && a.order.is_none() && !fo.dead && tr.first_dmg.is_none() {
            let c = capture(&sim);
            print_capture(tag, "idle-moment", &c);
            tr.idle = Some(c);
        }
        // 600 ticks after the idle moment: has damage landed?
        if tr.dmg_after_idle_600.is_none() {
            if let Some(idle) = &tr.idle {
                if sim.tick >= idle.tick + 600 {
                    tr.dmg_after_idle_600 = Some(tr.first_dmg.is_some());
                }
            }
        }
        if fo.dead {
            tr.dead_tick = Some(t);
            break;
        }
    }
    if tr.dmg_after_idle_600.is_none() {
        tr.dmg_after_idle_600 = Some(tr.first_dmg.is_some());
    }
    let c = capture(&sim);
    print_capture(tag, "window-end", &c);
    tr.end = Some(c);
    eprintln!(
        "ENGAGEEXEC3 {tag} first_dmg={:?} drop={:?} dead={:?} min_dist={:.2} dmg600={:?}",
        tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.min_dist, tr.dmg_after_idle_600
    );
    tr
}

/// THE LIVE GEOMETRY: enemy Scout at the TC CENTRE, Militia
/// ordered in from the east (run #460's approach side), plus
/// the other three sides.
#[test]
fn centre_scout_militia_from_each_side() {
    let mut failures = Vec::new();
    for (name, off) in [
        ("E", (24.0, 0.0)),
        ("W", (-24.0, 0.0)),
        ("N", (0.0, -24.0)),
        ("S", (0.0, 24.0)),
    ] {
        let tag = format!("centre-scout-{name}");
        let tr = interior_scene(&tag, EntityKind::ScoutCavalry, (0.0, 0.0), off, 7_200);
        if tr.first_dmg.is_none() || tr.dead_tick.is_none() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "centre failures:\n{}", failures.join("\n"));
}

/// Live variants on the centre geometry: (a) the Militia keeps
/// its DEFAULT stance (no SetStance — the live player issues
/// only the attack click); (b) the player's villagers stand at
/// the TC dropoff ring (r=5) while the Militia comes from E.
#[test]
fn centre_scout_live_variants() {
    // (a) default stance
    {
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tcx, tcy);
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..300 { sim.tick(); }
        assert!(f(ent(&sim, 0).hp) < 2400.0, "variant-a: TC not draining");
        spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
              tcx + I16F16::from_num(24), tcy);
        assert!(sim.can_attack(ATK, FOE), "variant-a: can_attack refused");
        sim.push_command(Command::Attack { entity: ATK, target: FOE });
        let hp0 = f(ent(&sim, FOE).hp);
        let mut first = None;
        let mut dead = None;
        for t in 0..7_200u64 {
            sim.tick();
            if first.is_none() && f(ent(&sim, FOE).hp) < hp0 { first = Some(t); }
            if ent(&sim, FOE).dead { dead = Some(t); break; }
        }
        eprintln!("ENGAGEEXEC3 variant-a-default-stance first_dmg={first:?} dead={dead:?}");
        assert!(first.is_some() && dead.is_some(), "variant-a: no engagement: first={first:?} dead={dead:?}");
    }
    // (b) dropoff ring crowd
    {
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tcx, tcy);
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..300 { sim.tick(); }
        assert!(f(ent(&sim, 0).hp) < 2400.0, "variant-b: TC not draining");
        for (i, (dx, dy)) in [(5.0, 0.0), (-5.0, 0.0), (0.0, 5.0), (0.0, -5.0)].iter().enumerate() {
            spawn(&mut sim, 9_960 + i as u32, EntityKind::Villager, OWNER_PLAYER,
                  tcx + I16F16::from_num(*dx), tcy + I16F16::from_num(*dy));
        }
        spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
              tcx + I16F16::from_num(24), tcy);
        sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
        sim.tick();
        assert!(sim.can_attack(ATK, FOE), "variant-b: can_attack refused");
        sim.push_command(Command::Attack { entity: ATK, target: FOE });
        let hp0 = f(ent(&sim, FOE).hp);
        let mut first = None;
        let mut dead = None;
        let mut idle_cap = None;
        for t in 0..7_200u64 {
            sim.tick();
            let a = ent(&sim, ATK);
            if idle_cap.is_none() && t >= 30 && a.order.is_none() && !ent(&sim, FOE).dead && first.is_none() {
                idle_cap = Some(capture(&sim));
            }
            if first.is_none() && f(ent(&sim, FOE).hp) < hp0 { first = Some(t); }
            if ent(&sim, FOE).dead { dead = Some(t); break; }
        }
        if let Some(c) = &idle_cap { print_capture("variant-b-crowd", "idle-moment", c); }
        eprintln!("ENGAGEEXEC3 variant-b-crowd first_dmg={first:?} dead={dead:?}");
        assert!(first.is_some() && dead.is_some(), "variant-b: no engagement: first={first:?} dead={dead:?}");
    }
}

/// Interior footprint cells: foe Spearman spawned at offsets
/// inside the 4x4 footprint (half-extent 8 wu) but within
/// MELEE_REACH of the centre so it drains standing there.
#[test]
fn interior_footprint_cells_spearman() {
    let mut failures = Vec::new();
    for (i, foe_off) in [
        (2.0, 0.0),
        (-2.0, 0.0),
        (0.0, 2.0),
        (0.0, -2.0),
        (2.0, 2.0),
        (-2.0, -2.0),
        (1.0, -1.0),
    ]
    .iter()
    .enumerate()
    {
        let atk_off = if i % 2 == 0 { (24.0, 0.0) } else { (-24.0, 0.0) };
        let tag = format!("interior-{i}-({},{})", foe_off.0, foe_off.1);
        let tr = interior_scene(&tag, EntityKind::Spearman, *foe_off, atk_off, 7_200);
        if tr.first_dmg.is_none() || tr.dead_tick.is_none() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "interior failures:\n{}", failures.join("\n"));
}

/// Interior Scout (the live kind) at off-centre interior
/// offsets, Militia from the east.
#[test]
fn interior_footprint_cells_scout() {
    let mut failures = Vec::new();
    for (i, foe_off) in [(2.0, 0.0), (0.0, 2.0), (-2.0, 2.0)].iter().enumerate() {
        let tag = format!("interior-scout-{i}-({},{})", foe_off.0, foe_off.1);
        let tr = interior_scene(&tag, EntityKind::ScoutCavalry, *foe_off, (24.0, 0.0), 7_200);
        if tr.first_dmg.is_none() || tr.dead_tick.is_none() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "interior scout failures:\n{}", failures.join("\n"));
}
