//! VILLAGERATTACK (2026-11-23): the NEW face of the hidden-
//! attacker defect proven live in run #487.
//!
//! Live #487 (occlog, match vs AI Normal): a VILLAGER (entity
//! 2) was ordered to attack an enemy MILITIA (entity 70)
//! standing UNDER the player's Town Center roof / INSIDE the
//! TC footprint, attacking the TC. The client pick was CORRECT
//! (picked=70 via outline, tFromOutline=true) and the issue was
//! a clean attack: {t:'attack', e:2, tg:70}, ok:true. Yet the
//! villager walked in under the roof and stood IDLE beside the
//! Militia: both HPs unchanged (villager 25/25, Militia 40/40)
//! for minutes, no damage either way, TC kept draining and died.
//!
//! Previous staging (engagefoe / engageunder / engagese /
//! engagefrozen / tcrally / engagepillar) proved MILITIA
//! attackers engage foes at TC edges/pillars, and a villager
//! adjacent to a 1-HP Spearman at the EAST pillar lands the
//! final blow. What was NEVER staged, and what this suite
//! stages in combination:
//!   (a) the ATTACKER is a VILLAGER (not Militia) against a
//!       FULL-HP foe (40/40 Militia, not a 1-HP Spearman);
//!   (b) the foe is INSIDE the TC footprint (centre / interior
//!       cells / pillars), not only at a pillar/edge;
//!   (c) the foe is ACTIVELY ATTACKING the TC (its own real
//!       Attack order on the TC running) while being attacked.
//!
//! Geometry: the TC footprint is 4x4 cells, 16 wu wide, centred
//! on the TC position — interior cell centres sit at TC-relative
//! offsets in {-6,-2,+2,+6} on each axis, the centre at (0,0),
//! the pillars at the corners/edges. Every order on the path
//! under test is a real `Command` (foe Attack on the TC,
//! villager Move, villager Attack — `can_attack` asserted where
//! the live issue returned ok:true). Spawn positions are scene
//! selection only.
//!
//! Per-tick trace per scene: attacker final position / distance
//! to foe, foe HP over the window, attacker HP, whether any
//! strike lands, attack_target / strike_tick / next_attack_tick
//! / attack_engaged at the idle moment and at window end.
//!
//! OUTCOME (measured on base b33f9d7, see handoff
//! `villagerattack-build.md`): NO scene reproduces. Across all
//! 25 scenes (5 settled-approach + 13 interior + 5 adjacent +
//! 2 starting-villager) the villager closes INTO the footprint
//! to dist 2.95–3.00, first damage lands t=11–490 after the
//! order, the full-HP Militia dies t=1593–2050, the villager
//! finishes at 25/25 (the foe never retargets off the TC),
//! max_stall_out = 0, max_stall_in = 38 (the first-strike
//! windup), drop = None everywhere. Two staging facts: a foe
//! Militia ordered on the TC HOLDS the exact centre (0,0) and
//! the (±2,±2) interior cells while draining the TC, and a foe
//! staged at (±6,0)/(0,±6)/(±6,±6) walks inward to its TC stand
//! (2.94,0)/(0,±2.94)/(±2.10,±2.10) — still inside the
//! footprint — before the villager arrives. Per package
//! discipline NO production fix is made; these pins are the
//! deliverable, and the remaining untested territory is named
//! in the handoff (live foe striking the TC as a building
//! record rather than entity 0; villager with carry/work-task
//! state at the order moment; shift-queued orders; multi-unit
//! separation crowds; full-match AI state).

use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::steer;
use crate::{Command, Sim};
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

const FOE: u32 = 9_701;
const VIL: u32 = 9_702;
const MELEE: f64 = 3.0;

#[derive(Debug, Clone)]
struct Capture {
    tick: u64,
    attack_target: Option<u32>,
    has_order: bool,
    strike_tick: Option<u64>,
    next_attack_tick: u64,
    attack_engaged: bool,
    dist: f64,
    atk_off: (f64, f64),
    foe_off: (f64, f64),
    foe_hp: f64,
    atk_hp: f64,
    tc_hp: f64,
    foe_attack_target: Option<u32>,
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
        strike_tick: a.strike_tick,
        next_attack_tick: a.next_attack_tick,
        attack_engaged: a.attack_engaged,
        dist: f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y)),
        atk_off: (f(a.pos_x - tcx), f(a.pos_y - tcy)),
        foe_off: (f(fo.pos_x - tcx), f(fo.pos_y - tcy)),
        foe_hp: f(fo.hp),
        atk_hp: f(a.hp),
        tc_hp: f(ent(sim, 0).hp),
        foe_attack_target: fo.attack_target,
    }
}

fn print_capture(tag: &str, when: &str, c: &Capture) {
    eprintln!(
        "VILLAGERATTACK CAPTURE {tag} {when}: tick={} attack_target={:?} has_order={} strike_tick={:?} next_attack_tick={} engaged={} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_hp={:.0} atk_hp={:.0} tc_hp={:.0} foe_attack_target={:?}",
        c.tick, c.attack_target, c.has_order, c.strike_tick, c.next_attack_tick,
        c.attack_engaged, c.dist, c.atk_off.0, c.atk_off.1, c.foe_off.0, c.foe_off.1,
        c.foe_hp, c.atk_hp, c.tc_hp, c.foe_attack_target
    );
}

#[derive(Default, Debug)]
struct Trace {
    first_dmg: Option<u64>,      // first tick foe HP drops
    first_hurt: Option<u64>,     // first tick attacker HP drops
    drop_tick: Option<u64>,      // attack_target lost while foe alive
    dead_tick: Option<u64>,      // foe died
    atk_dead_tick: Option<u64>,  // attacker died (engagement happened)
    min_dist: f64,
    max_stall_out: u64,
    max_stall_in: u64,
    idle: Option<Capture>,
    end: Option<Capture>,
}

impl Trace {
    /// The live #487 shape: no damage EITHER WAY while both
    /// live, the attacker idle (no move order) for >= 600 ticks
    /// (out of reach OR in reach without a strike), or the foe
    /// never dies while the attacker stands idle beside it.
    /// An attacker that DIES fighting is an engagement, not
    /// the live shape.
    fn live_shape(&self) -> bool {
        if self.atk_dead_tick.is_some() {
            return false;
        }
        self.first_dmg.is_none()
            && self.first_hurt.is_none()
            && (self.dead_tick.is_none()
                || self.max_stall_out >= 600
                || self.max_stall_in >= 600
                || self.drop_tick.is_some())
    }
}

fn run_window(tag: &str, sim: &mut Sim, atk: u32, window: u64) -> Trace {
    let foe_hp0 = f(ent(sim, FOE).hp);
    let atk_hp0 = f(ent(sim, atk).hp);
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
        if tr.first_hurt.is_none() && f(a.hp) < atk_hp0 {
            tr.first_hurt = Some(t);
        }
        if tr.drop_tick.is_none() && !fo.dead && a.attack_target != Some(FOE) {
            tr.drop_tick = Some(t);
        }
        if !fo.dead && !a.dead && a.order.is_none() && tr.first_dmg.is_none() {
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
            tr.atk_dead_tick = Some(t);
            break;
        }
    }
    let c = capture(sim, atk);
    print_capture(tag, "window-end", &c);
    tr.end = Some(c);
    eprintln!(
        "VILLAGERATTACK {tag} first_dmg={:?} first_hurt={:?} drop={:?} dead={:?} atk_dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} live_shape={}",
        tr.first_dmg, tr.first_hurt, tr.drop_tick, tr.dead_tick, tr.atk_dead_tick,
        tr.min_dist, tr.max_stall_out, tr.max_stall_in, tr.live_shape()
    );
    tr
}

fn order_attack(sim: &mut Sim, tag: &str, atk: u32) {
    assert!(sim.can_attack(atk, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: atk, target: FOE });
}

/// (1) THE SETTLED LIVE GEOMETRY: foe Militia walks in from
/// the east under a real Attack order on the TC, settles at
/// its TC stand INSIDE the footprint (recorded), TC
/// verifiably draining; then a villager spawned OUTSIDE the
/// footprint on five approach sides is ordered to attack it.
#[test]
fn villager_outside_attacks_settled_militia_attacking_tc() {
    let mut failures = Vec::new();
    for (name, vil_off) in [
        ("E", (20.0, 0.0)),
        ("S", (0.0, 20.0)),
        ("W", (-20.0, 0.0)),
        ("N", (0.0, -20.0)),
        ("SE", (16.0, 16.0)),
    ] {
        let tag = format!("settled-vil{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT,
              tcx + I16F16::from_num(24), tcy);
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..600 {
            sim.tick();
        }
        let fo = ent(&sim, FOE);
        let settled = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
        let tc_hp = f(ent(&sim, 0).hp);
        eprintln!(
            "VILLAGERATTACK {tag} foe settled off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0} foe_attack_target={:?}",
            settled.0, settled.1, f(fo.hp), tc_hp, fo.attack_target
        );
        if tc_hp >= 2400.0 {
            failures.push(format!("{tag}: foe not draining TC (hp {tc_hp:.0})"));
            continue;
        }
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tcx + I16F16::from_num(vil_off.0), tcy + I16F16::from_num(vil_off.1));
        order_attack(&mut sim, &tag, VIL);
        let tr = run_window(&tag, &mut sim, VIL, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: settled={settled:?} {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "settled failures:\n{}", failures.join("\n"));
}

/// (2) THE STAGED INTERIOR MATRIX: foe Militia spawned AT
/// each interior cell centre / centre / pillar of the TC
/// footprint with its real Attack order on the TC issued at
/// spawn; after 30 ticks (order running; the foe holds its
/// cell if in TC-stand range, else walks inward — position
/// recorded at the order moment) a villager outside the
/// footprint to the EAST attacks it.
#[test]
fn villager_outside_attacks_interior_militia_attacking_tc() {
    let cells: [(f64, f64); 13] = [
        (0.0, 0.0),
        (2.0, 2.0), (-2.0, -2.0), (2.0, -2.0), (-2.0, 2.0),
        (6.0, 0.0), (0.0, 6.0), (-6.0, 0.0), (0.0, -6.0),
        (6.0, 6.0), (-6.0, -6.0), (6.0, -6.0), (-6.0, 6.0),
    ];
    let mut failures = Vec::new();
    for foe_off in cells {
        let tag = format!("interior-({},{})", foe_off.0, foe_off.1);
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(foe_off.1));
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..30 {
            sim.tick();
        }
        let fo = ent(&sim, FOE);
        let foe_now = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
        eprintln!(
            "VILLAGERATTACK {tag} foe at order moment off=({:.2},{:.2}) (staged {:?}) foe_hp={:.0} tc_hp={:.0}",
            foe_now.0, foe_now.1, foe_off, f(fo.hp), f(ent(&sim, 0).hp)
        );
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tcx + I16F16::from_num(20), tcy + I16F16::from_num(foe_off.1));
        order_attack(&mut sim, &tag, VIL);
        let tr = run_window(&tag, &mut sim, VIL, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: foe_now={foe_now:?} {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "interior failures:\n{}", failures.join("\n"));
}

/// (3) ADJACENT INSIDE: villager already inside the footprint,
/// adjacent (gap 1.5 / 2.9) to a full-HP Militia at the centre
/// / an interior cell, the Militia actively attacking the TC;
/// the villager's Attack order lands from adjacency — the
/// closest staging of the live idle moment itself.
#[test]
fn villager_adjacent_inside_to_full_hp_militia_attacking_tc() {
    let mut failures = Vec::new();
    for (name, foe_off, gap) in [
        ("centre-gap1.5", (0.0, 0.0), 1.5),
        ("centre-gap2.9", (0.0, 0.0), 2.9),
        ("cell(2,2)-gap1.5", (2.0, 2.0), 1.5),
        ("cell(-2,2)-gap2.9", (-2.0, 2.0), 2.9),
        ("pillar(6,6)-gap1.5", (6.0, 6.0), 1.5),
    ] {
        let tag = format!("adjacent-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(foe_off.1));
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tcx + I16F16::from_num(foe_off.0 + gap), tcy + I16F16::from_num(foe_off.1));
        for _ in 0..30 {
            sim.tick();
        }
        order_attack(&mut sim, &tag, VIL);
        let tr = run_window(&tag, &mut sim, VIL, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "adjacent failures:\n{}", failures.join("\n"));
}

/// (4) A REAL STARTING VILLAGER (from `Sim::new`, the live
/// unit's provenance) attacks the settled, TC-draining
/// Militia — first directly, and in a second scene after a
/// real Move order that parks it just OUTSIDE the footprint
/// east edge (the approach/rally face available to a villager).
#[test]
fn starting_villager_attacks_settled_militia_attacking_tc() {
    let mut failures = Vec::new();
    for move_first in [false, true] {
        let tag = if move_first { "starting-vil-move-then-attack" } else { "starting-vil-direct" };
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT,
              tcx + I16F16::from_num(24), tcy);
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..600 {
            sim.tick();
        }
        if f(ent(&sim, 0).hp) >= 2400.0 {
            failures.push(format!("{tag}: foe not draining TC"));
            continue;
        }
        let vil = sim
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
            .unwrap()
            .id;
        if move_first {
            sim.push_command(Command::Move {
                entity: vil,
                x: tcx + I16F16::from_num(12),
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
            eprintln!("VILLAGERATTACK {tag} move arrived={arrived}");
            if !arrived {
                failures.push(format!("{tag}: villager Move never arrived"));
                continue;
            }
        }
        order_attack(&mut sim, tag, vil);
        let tr = run_window(tag, &mut sim, vil, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "starting-villager failures:\n{}", failures.join("\n"));
}
