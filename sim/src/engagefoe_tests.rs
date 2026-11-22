//! ENGAGEFOE (2026-11-22): candidates (1) and (3) from the
//! engageexec3 handoff — the foe's exact live position/state at
//! the idle moment, and a moving crowd denser than staged rings.
//!
//! engageexec3 excluded a SETTLED foe at the TC centre/interior;
//! engageexec2 excluded edge + blocked cell; engagegroup excluded
//! the multi-unit order context. What none of them staged:
//!   (1a) a foe at TC footprint EDGE cells (half-extent 8 wu —
//!        offsets ±4..±8), settled or with the attack order
//!        landing while it still stands there;
//!   (1b) a foe MID-PATH through the footprint — it walked in
//!        from outside under a real Attack order on the TC and
//!        the player's attack order lands while the foe is
//!        still moving (vel != 0) inside the footprint. A foe
//!        attacking the TC entity strikes from centre distance
//!        <= MELEE_REACH, so an edge foe is necessarily a
//!        moving foe until it settles;
//!   (1c) a foe JUST DISPLACED — stacked spawns shoved apart by
//!        separation, or shoved by late-arriving allies — with
//!        the attack order landing mid-displacement;
//!   (1d) an OSCILLATING foe — Move orders ping-ponging it
//!        across the footprint (the microed / bumped-around
//!        state) while the attacker chases;
//!   (3)  CONTINUOUS dropoff traffic — 8 villagers looping
//!        Move orders across the footprint through the TC
//!        centre lanes for the whole window while the attack
//!        order lands mid-crossing.
//!
//! Attackers are the three kinds the live runs used: lone
//! Militia, lone Villager, lone own Scout Cavalry — each
//! ordered by a real `Command::Attack` (can_attack asserted).
//!
//! The LIVE SHAPE hunted (run #462): attacker holds no move
//! order, out of reach (dist > MELEE_REACH), foe alive, for a
//! sustained stretch (>= 600 ticks) with no damage — or the
//! target dropped — or in reach >= 600 ticks without a strike.
//! Every scene records the idle-moment capture (attack_target,
//! order, dist, chase_retry_tick, foe cell + blocked, foe vel)
//! and the worst out-of-reach stall.
//!
//! OUTCOME (measured on base f677f43, see handoff
//! `engagefoe-build.md`): NO branch confirms. Edge-spawned
//! foes never settle at the edge — a foe striking the TC
//! entity stands at centre distance 2.94..3.00, so every
//! settled edge scene collapses to the centre geometry. The
//! mid-path (foe moving, vel -0.080 at band entry), displaced
//! (stacked spawn / ally shove), oscillating (ping-pong Move
//! orders across the footprint — caught and killed by all
//! three attacker kinds) and continuous-traffic (8 villagers
//! looping across the footprint; the Militia's kill ticks are
//! IDENTICAL to the no-traffic centre baseline) scenes all
//! engage: max_stall_out = 0 and drop = None in all 30
//! scenes. Per package discipline NO production fix is made;
//! these pins are the deliverable.

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
    suppress: bool,
    dist: f64,
    atk_off: (f64, f64),
    foe_off: (f64, f64),
    foe_vel: (f64, f64),
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
        has_order: a.order.is_some(),
        chase_retry_tick: a.chase_retry_tick,
        suppress: a.suppress_separation,
        dist: f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y)),
        atk_off: (f(a.pos_x - tcx), f(a.pos_y - tcy)),
        foe_off: (f(fo.pos_x - tcx), f(fo.pos_y - tcy)),
        foe_vel: (f(fo.vel_x), f(fo.vel_y)),
        foe_cell: cell,
        foe_cell_blocked: sim.grid.is_blocked(cell.0, cell.1),
        foe_hp: f(fo.hp),
        tc_hp: f(ent(sim, 0).hp),
    }
}

fn print_capture(tag: &str, when: &str, c: &Capture) {
    eprintln!(
        "ENGAGEFOE CAPTURE {tag} {when}: tick={} attack_target={:?} has_order={} chase_retry_tick={} suppress={} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_vel=({:.3},{:.3}) foe_cell={:?} foe_cell_blocked={} foe_hp={:.0} tc_hp={:.0}",
        c.tick, c.attack_target, c.has_order, c.chase_retry_tick, c.suppress,
        c.dist, c.atk_off.0, c.atk_off.1, c.foe_off.0, c.foe_off.1,
        c.foe_vel.0, c.foe_vel.1, c.foe_cell, c.foe_cell_blocked, c.foe_hp, c.tc_hp
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
    /// The live #462 shape: foe never dies, or a >=600-tick
    /// out-of-reach idle stall, or a target drop.
    fn live_shape(&self) -> bool {
        self.dead_tick.is_none() || self.max_stall_out >= 600 || self.drop_tick.is_some()
    }
}

/// Run the window after the attack order, applying the
/// per-tick `driver` first (traffic / oscillation re-orders).
fn run_window(
    tag: &str,
    sim: &mut Sim,
    window: u64,
    mut driver: impl FnMut(&mut Sim),
) -> Trace {
    let foe_hp0 = f(ent(sim, FOE).hp);
    let mut tr = Trace { min_dist: f64::MAX, ..Default::default() };
    let mut stall_out = 0u64;
    let mut stall_in = 0u64;
    for t in 0..window {
        driver(sim);
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
    }
    let c = capture(sim);
    print_capture(tag, "window-end", &c);
    tr.end = Some(c);
    eprintln!(
        "ENGAGEFOE {tag} first_dmg={:?} drop={:?} dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} live_shape={}",
        tr.first_dmg, tr.drop_tick, tr.dead_tick, tr.min_dist, tr.max_stall_out,
        tr.max_stall_in, tr.live_shape()
    );
    tr
}

fn order_attack(sim: &mut Sim, tag: &str, atk_kind: EntityKind) {
    if atk_kind != EntityKind::Villager {
        sim.push_command(Command::SetStance { entity: ATK, stance: Stance::Aggressive });
        sim.tick();
    }
    assert!(sim.can_attack(ATK, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: ATK, target: FOE });
}

/// (1a) Foe spawned at footprint EDGE offsets, ordered Attack
/// on the TC, settled 300 ticks — record where it settles —
/// then a Militia attacks from the east. Plus the no-settle
/// variant: the Militia's order lands the tick the foe spawns
/// at the edge (foe still standing/moving at the edge).
#[test]
fn edge_cells_settled_and_immediate() {
    let mut failures = Vec::new();
    for (i, foe_off) in [
        (6.0, 0.0), (-6.0, 0.0), (0.0, 6.0), (0.0, -6.0),
        (6.0, 6.0), (-6.0, -6.0), (7.0, 0.0), (4.0, 4.0),
    ]
    .iter()
    .enumerate()
    {
        // settled variant
        let tag = format!("edge-settled-{i}-({},{})", foe_off.0, foe_off.1);
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(foe_off.1));
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..300 { sim.tick(); }
        let foe = ent(&sim, FOE);
        let settled = (f(foe.pos_x - tcx), f(foe.pos_y - tcy));
        eprintln!("ENGAGEFOE {tag} foe settled off=({:.2},{:.2}) tc_hp={:.0}",
                  settled.0, settled.1, f(ent(&sim, 0).hp));
        spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
              tcx + I16F16::from_num(24), tcy);
        order_attack(&mut sim, &tag, EntityKind::Militia);
        let tr = run_window(&tag, &mut sim, 7_200, |_| {});
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
        // immediate variant: no settle at all
        let tag = format!("edge-immediate-{i}-({},{})", foe_off.0, foe_off.1);
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(foe_off.1));
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
              tcx + I16F16::from_num(24), tcy);
        order_attack(&mut sim, &tag, EntityKind::Militia);
        let tr = run_window(&tag, &mut sim, 7_200, |_| {});
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "edge failures:\n{}", failures.join("\n"));
}

/// (1b) MID-PATH: foe spawns outside (28,0), walks in under a
/// real Attack order on the TC; the moment its offset length
/// first drops to <= 8.5 (footprint edge band, still moving),
/// the attacker's real Attack order lands. Attacker kinds:
/// Militia, Villager, own Scout.
#[test]
fn midpath_through_footprint() {
    let mut failures = Vec::new();
    for (name, kind) in [
        ("militia", EntityKind::Militia),
        ("villager", EntityKind::Villager),
        ("scout", EntityKind::ScoutCavalry),
    ] {
        let tag = format!("midpath-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT,
              tcx + I16F16::from_num(28), tcy);
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        spawn(&mut sim, ATK, kind, OWNER_PLAYER,
              tcx, tcy + I16F16::from_num(32));
        // Walk the foe in until it reaches the edge band.
        let mut entry = None;
        for t in 0..600u64 {
            sim.tick();
            let fo = ent(&sim, FOE);
            let d = f(steer::vec_len(fo.pos_x - tcx, fo.pos_y - tcy));
            if d <= 8.5 {
                entry = Some((t, d, f(fo.vel_x), f(fo.vel_y)));
                break;
            }
        }
        let (et, ed, evx, evy) = entry.expect("foe never entered the footprint band");
        eprintln!("ENGAGEFOE {tag} foe entered band at t={et} dist={ed:.2} vel=({evx:.3},{evy:.3})");
        order_attack(&mut sim, &tag, kind);
        let tr = run_window(&tag, &mut sim, 7_200, |_| {});
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "midpath failures:\n{}", failures.join("\n"));
}

/// (1c) JUST DISPLACED: foe spawned at the centre stacked with
/// 4 opponent villagers on the exact same point (separation
/// shoves everyone apart over the first ticks); the attack
/// order lands at tick 5, mid-displacement. Second variant:
/// foe settled draining, then 3 opponent Scouts spawn stacked
/// ON the foe and attack the TC too — the shove lands while
/// the Militia is already chasing.
#[test]
fn just_displaced_foe() {
    let mut failures = Vec::new();
    // variant 1: stacked spawn
    {
        let tag = "displaced-stack";
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tcx, tcy);
        for i in 0..4u32 {
            spawn(&mut sim, 9_900 + i, EntityKind::Villager, OWNER_OPPONENT, tcx, tcy);
        }
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
              tcx + I16F16::from_num(24), tcy);
        for _ in 0..5 { sim.tick(); }
        let c = capture(&sim);
        print_capture(tag, "order-moment", &c);
        order_attack(&mut sim, tag, EntityKind::Militia);
        let tr = run_window(tag, &mut sim, 7_200, |_| {});
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    // variant 2: late shove by allies
    {
        let tag = "displaced-shove";
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tcx, tcy);
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..300 { sim.tick(); }
        assert!(f(ent(&sim, 0).hp) < 2400.0, "{tag}: TC not draining");
        spawn(&mut sim, ATK, EntityKind::Militia, OWNER_PLAYER,
              tcx + I16F16::from_num(24), tcy);
        order_attack(&mut sim, tag, EntityKind::Militia);
        let mut shoved = false;
        let tr = run_window(tag, &mut sim, 7_200, |s| {
            if !shoved && s.tick > 340 {
                shoved = true;
                let tc2 = s.town_center();
                let (x, y) = (tc2.pos_x, tc2.pos_y);
                for i in 0..3u32 {
                    let id = 9_910 + i;
                    s.entities.push(Entity::spawned(id, EntityKind::ScoutCavalry, OWNER_OPPONENT, x, y));
                    s.push_command(Command::Attack { entity: id, target: 0 });
                }
            }
        });
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "displaced failures:\n{}", failures.join("\n"));
}

/// (1d) OSCILLATING foe: settled draining first, then Move
/// orders ping-pong it between the E and W footprint edges
/// (re-issued on arrival) for the whole window while each
/// attacker kind chases. The foe is a moving target inside
/// the footprint — the press gate's stationary-victim premise
/// never holds.
#[test]
fn oscillating_foe() {
    let mut failures = Vec::new();
    for (name, kind) in [
        ("militia", EntityKind::Militia),
        ("villager", EntityKind::Villager),
        ("scout", EntityKind::ScoutCavalry),
    ] {
        let tag = format!("oscillating-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tcx, tcy);
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..300 { sim.tick(); }
        assert!(f(ent(&sim, 0).hp) < 2400.0, "{tag}: TC not draining");
        spawn(&mut sim, ATK, kind, OWNER_PLAYER,
              tcx + I16F16::from_num(24), tcy);
        order_attack(&mut sim, &tag, kind);
        let mut leg = 0i32; // 0 -> heading W edge, 1 -> heading E edge
        let tr = run_window(&tag, &mut sim, 7_200, |s| {
            let (cx2, cy2) = {
                let tc2 = s.town_center();
                (tc2.pos_x, tc2.pos_y)
            };
            let foe_idle = {
                let fo = s.entities().iter().find(|e| e.id == FOE).unwrap();
                fo.order.is_none() && !fo.dead
            };
            if foe_idle {
                let dest = if leg == 0 { -6.0 } else { 6.0 };
                leg = 1 - leg;
                s.push_command(Command::Move {
                    entity: FOE,
                    x: cx2 + I16F16::from_num(dest),
                    y: cy2,
                });
            }
        });
        // An endlessly microed foe may legitimately outrun a
        // Militia/Villager; the live shape here is narrower:
        // a >=600-tick OUT-OF-REACH IDLE stall or a drop. A foe
        // that keeps moving and is never caught while the
        // attacker keeps its order and keeps moving is a
        // pursuit, not the defect.
        if tr.max_stall_out >= 600 || tr.drop_tick.is_some() {
            failures.push(format!("{tag}: stall/drop {tr:?}"));
        }
        eprintln!("ENGAGEFOE {tag} oscillating verdict: dead={:?} first_dmg={:?}",
                  tr.dead_tick, tr.first_dmg);
    }
    assert!(failures.is_empty(), "oscillating failures:\n{}", failures.join("\n"));
}

/// (3) CONTINUOUS DROPOFF TRAFFIC: foe settled at the centre
/// draining; 8 player villagers loop Move orders E<->W across
/// the footprint on lanes y = -6..+6 (re-issued on arrival, so
/// the crossing never stops) while the attack order lands.
/// Attacker kinds: Militia, Villager, own Scout.
#[test]
fn continuous_dropoff_traffic() {
    let mut failures = Vec::new();
    for (name, kind) in [
        ("militia", EntityKind::Militia),
        ("villager", EntityKind::Villager),
        ("scout", EntityKind::ScoutCavalry),
    ] {
        let tag = format!("traffic-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT, tcx, tcy);
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..300 { sim.tick(); }
        assert!(f(ent(&sim, 0).hp) < 2400.0, "{tag}: TC not draining");
        // Traffic: 8 villagers starting on alternating sides.
        let lanes = [-6.0, -4.0, -2.0, 0.0, 0.0, 2.0, 4.0, 6.0];
        let mut heading: Vec<i32> = Vec::new();
        for (i, lane) in lanes.iter().enumerate() {
            let id = 9_920 + i as u32;
            let start_east = i % 2 == 0;
            let sx = if start_east { 14.0 } else { -14.0 };
            spawn(&mut sim, id, EntityKind::Villager, OWNER_PLAYER,
                  tcx + I16F16::from_num(sx), tcy + I16F16::from_num(*lane));
            heading.push(if start_east { -1 } else { 1 });
            sim.push_command(Command::Move {
                entity: id,
                x: tcx + I16F16::from_num(if start_east { -14.0 } else { 14.0 }),
                y: tcy + I16F16::from_num(*lane),
            });
        }
        // Let the traffic get mid-crossing before the order.
        for _ in 0..60 { sim.tick(); }
        spawn(&mut sim, ATK, kind, OWNER_PLAYER,
              tcx + I16F16::from_num(24), tcy);
        order_attack(&mut sim, &tag, kind);
        let lanes2 = lanes;
        let tr = run_window(&tag, &mut sim, 7_200, |s| {
            let (cx2, cy2) = {
                let tc2 = s.town_center();
                (tc2.pos_x, tc2.pos_y)
            };
            for (i, lane) in lanes2.iter().enumerate() {
                let id = 9_920 + i as u32;
                let idle = {
                    let v = s.entities().iter().find(|e| e.id == id).unwrap();
                    v.order.is_none() && !v.dead
                };
                if idle {
                    heading[i] = -heading[i];
                    let dest = if heading[i] > 0 { 14.0 } else { -14.0 };
                    s.push_command(Command::Move {
                        entity: id,
                        x: cx2 + I16F16::from_num(dest),
                        y: cy2 + I16F16::from_num(*lane),
                    });
                }
            }
        });
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "traffic failures:\n{}", failures.join("\n"));
}
