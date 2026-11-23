//! TCSOUTH (2026-11-23): arrival geometry at the TC footprint —
//! the live runs #480/#481 geometry that no prior engage*
//! suite staged.
//!
//! Live (real client, #480/#481): an enemy Scout Cavalry
//! settled UNDER/BEHIND the player's Town Center roof on the
//! NORTH side (partially occluded). The player selected it by
//! clicking low at its feet and right-clicked Attack with a
//! Militia. In #481 the Militia walked to the SOUTH edge of
//! the TC footprint and went idle — enemy unharmed (45/45)
//! across 3 attempts in 2 matches, while the TC drained HP.
//! In #480 the same gesture engaged once.
//!
//! Prior harness work excluded: Attack/AttackMove from the
//! rally-frozen state (engagefrozen, base a7a62cb), the east
//! pillar geometry (engagepillar: a foe arriving from E/SE/NE
//! settles at centre distance ~2.9–3.0 on the EAST sector and
//! every trained/rally defender face engages), and the
//! TC-centre foe (engageunder). What none staged: a foe that
//! arrives from the NORTH and settles at the NORTH edge of
//! the footprint, attacked by a melee unit starting SOUTH of
//! the TC — the attacker's chase must cross the whole
//! footprint (BLD-16 pass-under: TC footprint cells are not
//! grid-blocked) and end beside the foe on the far side.
//!
//! Scene construction: the foe is NOT teleported to its
//! settled spot. It spawns 24 wu out on the approach face
//! under a real `Command::Attack` on the TC (entity 0) and
//! settles wherever the TC-attack stand actually is (recorded,
//! TC draining asserted) — the exact arrival an attacking
//! Scout makes. The defender is a spawned Militia (scene
//! selection only) placed 18–24 wu out on its approach face,
//! given Aggressive stance and a real `Command::Attack` on
//! the foe (`can_attack` asserted). Per-face trace: settled
//! foe offset, first-damage tick, kill tick, drop tick,
//! min distance, worst out-of-reach / in-reach no-damage
//! stall, and the window-end capture (attacker offset,
//! distance, order state, foe HP, TC HP).
//!
//! Faces: (1) MAIN — foe settles north, Militia from south
//! (the live #481 gesture); (2) mirrors — same north-settled
//! foe, Militia from east / west / north; (3) MIRROR TARGET
//! — foe arrives from south and settles at the south edge,
//! Militia from north.
//!
//! OUTCOME (measured on base a7a62cb, see handoff
//! `tcsouth-build.md`): NOT REPRODUCED on any face. The
//! Scout arriving from the north settles at exactly
//! (0.00,-2.96) — centre distance 2.96, TC draining
//! (2400 -> 2397 over the settle) — and the Militia from
//! the south walks THROUGH the footprint (BLD-16
//! pass-under), ends at the TC centre (0.00,0.00) at dist
//! 2.96, first damage t=330, foe dead t=1650, drop=None,
//! max_stall_out=0. Every mirror engages the same way
//! (E/W: first dmg 395, dead 1715; N: 331/1651; south-edge
//! foe vs north attacker: 330/1650; NE/NW diagonal settles
//! (2.07,-2.07)/(-2.07,-2.07): 326/1646; placed foe at the
//! settled point: 330/1650). Arrival geometry at the TC
//! footprint is therefore NOT the defect in the harness:
//! the south-edge idle of live #481 has no sim counterpart
//! here, and the remaining live-only territory is the same
//! as after engagefrozen — the client order path / the
//! exact live state at the idle moment. Per package
//! discipline NO production fix is made; these pins are
//! the deliverable.

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
const MELEE: f64 = 3.0;

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

fn capture(sim: &Sim) -> Capture {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let a = ent(sim, ATK);
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
        "TCSOUTH CAPTURE {tag} {when}: tick={} attack_target={:?} has_order={} chase_retry_tick={} strike_tick={:?} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_hp={:.0} atk_hp={:.0} tc_hp={:.0}",
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

fn run_window(tag: &str, sim: &mut Sim, window: u64) -> Trace {
    let foe_hp0 = f(ent(sim, FOE).hp);
    let mut tr = Trace { min_dist: f64::MAX, ..Default::default() };
    let mut stall_out = 0u64;
    let mut stall_in = 0u64;
    for t in 0..window {
        sim.tick();
        let a = ent(sim, ATK);
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
            let c = capture(sim);
            print_capture(tag, "idle-moment", &c);
            tr.idle = Some(c);
        }
        if fo.dead {
            tr.dead_tick = Some(t);
            break;
        }
        if a.dead {
            eprintln!("TCSOUTH {tag} attacker DIED at window t={t}");
            break;
        }
    }
    let c = capture(sim);
    print_capture(tag, "window-end", &c);
    tr.end = Some(c);
    eprintln!(
        "TCSOUTH {tag} first_dmg={:?} drop={:?} dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} live_shape={}",
        tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.min_dist,
        tr.max_stall_out, tr.max_stall_in, tr.live_shape()
    );
    tr
}

/// Stage the foe: spawn a Scout Cavalry 24 wu out on face
/// `foe_from` (TC-relative offset), order a real Attack on
/// the TC, settle 600 ticks. Asserts the TC is draining and
/// returns the settled TC-relative offset.
fn stage_foe(sim: &mut Sim, tag: &str, foe_from: (f64, f64)) -> (f64, f64) {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        sim,
        FOE,
        EntityKind::ScoutCavalry,
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
        "TCSOUTH {tag} foe settled off=({:.2},{:.2}) tc_hp={:.0} foe_attack_target={:?} foe_hp={:.0}",
        settled.0, settled.1, tc_hp, fo.attack_target, f(fo.hp)
    );
    assert!(tc_hp < 2400.0, "{tag}: foe is not draining the TC (hp {tc_hp:.0})");
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
    run_window(tag, sim, 7_200)
}

/// (1) MAIN: foe arrives from the NORTH, settles at the north
/// edge; Militia starts SOUTH of the TC and attacks it — the
/// live #481 gesture that idled at the south footprint edge.
#[test]
fn scout_settled_north_militia_from_south() {
    let tag = "main-Nfoe-Satk";
    let mut sim = Sim::new(42);
    let settled = stage_foe(&mut sim, tag, (0.0, -24.0));
    assert!(
        settled.1 < -2.0 && settled.0.abs() < 1.5,
        "{tag}: foe did not settle at the north edge: {settled:?}"
    );
    let tr = attack_from(tag, &mut sim, (0.0, 18.0));
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (2) MIRRORS: the same north-settled foe, Militia arriving
/// from east / west / north.
#[test]
fn scout_settled_north_militia_from_east_west_north() {
    let mut failures = Vec::new();
    for (name, atk_from) in [
        ("E", (24.0, 0.0)),
        ("W", (-24.0, 0.0)),
        ("N", (0.0, -24.0)),
    ] {
        let tag = format!("mirror-Nfoe-{name}atk");
        let mut sim = Sim::new(42);
        let settled = stage_foe(&mut sim, &tag, (0.0, -24.0));
        if !(settled.1 < -2.0 && settled.0.abs() < 1.5) {
            failures.push(format!("{tag}: foe did not settle north: {settled:?}"));
            continue;
        }
        let tr = attack_from(&tag, &mut sim, atk_from);
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "mirror failures:\n{}", failures.join("\n"));
}

/// (3) MIRROR TARGET: foe arrives from the SOUTH, settles at
/// the south edge; Militia starts NORTH and attacks it.
#[test]
fn scout_settled_south_militia_from_north() {
    let tag = "mirror-Sfoe-Natk";
    let mut sim = Sim::new(42);
    let settled = stage_foe(&mut sim, tag, (0.0, 24.0));
    assert!(
        settled.1 > 2.0 && settled.0.abs() < 1.5,
        "{tag}: foe did not settle at the south edge: {settled:?}"
    );
    let tr = attack_from(tag, &mut sim, (0.0, -18.0));
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}

/// (3b) DIAGONAL SETTLES: foe arrives from NE / NW and
/// settles on the north diagonal (the pillar suite measured
/// (2.10,-2.09) for NE), Militia from the south — the live
/// foe's exact diagonal under the roof is not recorded, so
/// both north diagonals are covered.
#[test]
fn scout_settled_north_diagonals_militia_from_south() {
    let mut failures = Vec::new();
    for (name, foe_from) in [("NE", (17.0, -17.0)), ("NW", (-17.0, -17.0))] {
        let tag = format!("diag-{name}foe-Satk");
        let mut sim = Sim::new(42);
        let settled = stage_foe(&mut sim, &tag, foe_from);
        if !(settled.1 < -1.5 && settled.0.abs() > 1.5) {
            failures.push(format!("{tag}: foe did not settle on the north diagonal: {settled:?}"));
            continue;
        }
        let tr = attack_from(&tag, &mut sim, (0.0, 18.0));
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "diagonal failures:\n{}", failures.join("\n"));
}

/// (4) PLACED VARIANT: the foe pinned (StandGround) at the
/// exact settled north-edge coordinates the arrival scene
/// measured, (0,-2.96) — the closest position to the
/// footprint the sim allows a TC attacker — attacked by a
/// Militia from the south. Separates "settled position"
/// from "arrival state" (attack order held on the TC).
#[test]
fn scout_placed_at_north_settled_point_militia_from_south() {
    let tag = "placed-Nfoe-Satk";
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        FOE,
        EntityKind::ScoutCavalry,
        OWNER_OPPONENT,
        tcx,
        tcy + I16F16::from_num(-2.96),
    );
    sim.push_command(Command::SetStance { entity: FOE, stance: Stance::StandGround });
    sim.tick();
    let fo = ent(&sim, FOE);
    eprintln!(
        "TCSOUTH {tag} placed foe off=({:.2},{:.2})",
        f(fo.pos_x - tcx),
        f(fo.pos_y - tcy)
    );
    let tr = attack_from(tag, &mut sim, (0.0, 18.0));
    assert!(!tr.live_shape(), "{tag}: LIVE SHAPE REPRODUCED: {tr:?}");
}
