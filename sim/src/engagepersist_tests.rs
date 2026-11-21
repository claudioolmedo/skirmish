//! ENGAGEPERSIST (2026-11-21): independent staging of the
//! EXECUTION layer of the hidden-attacker defect — bot-loop
//! runs #448/#451, on build 43a16e5 (FIX #20 live, selection
//! layer closed). The live capture proves the pick and the
//! issue: `pick {picked:71, fromOutline:true}` ->
//! `issue {attack, ok:true}` — and `ok:true` is, by the
//! client's own source, only `applyLocalOrder`'s return:
//! `sim.push_attack(...)` accepted the command (the
//! `can_attack` gate passed and the order was stored). It
//! says nothing about persistence. Within ~a minute the
//! ordered Militia is idle again — in #451 idle AT THE
//! BARRACKS, where it spawned (travel ~0) — while the foe
//! under the TC roof keeps draining it.
//!
//! RELATION TO OCCORDER23: a parallel builder (occorder23,
//! commit 4ee7500 on the same base) staged this same layer
//! with bare `grid.set_dynamic` rings and proved the chase's
//! clear-on-path-failure. This suite is the independent
//! cross-check with REAL game objects, in the
//! castle_p3/garrison fixture idiom: enclosures are placed
//! Building records (PalisadeWall segments, Barracks, Houses)
//! whose footprint cells are set dynamic — the exact two
//! effects a completed building has in this sim (record +
//! dynamic cells, per `place_building` / `complete_building`).
//! If the mechanism were an artifact of bare grid staging,
//! these scenes would engage; if it is the sim's real chase
//! behaviour, they reproduce the live signature exactly.
//!
//! DISCRIMINATION (all orders on the path under test are real
//! `Command`s — the foe's Attack on the TC, the attacker's
//! SetStance and Attack; positions/kinds/building records are
//! scene selection):
//!
//!   - (control) Foe settled on the TC's south porch draining
//!     it, a real Barracks standing adjacent to the TC, the
//!     Militia spawning south of that Barracks: ENGAGES.
//!     Building adjacency is not an enclosure.
//!   - (control) A real palisade ring around the TC with a
//!     one-cell gap: ENGAGES (first damage ~400 ticks).
//!   - (PIN) The same ring complete (24 real PalisadeWall
//!     records, Chebyshev radius 3 cells around the TC): on
//!     pristine the attack target drops at t=0, the attacker
//!     never moves (travel 0.0), the foe's HP is frozen and
//!     the TC drains for the full window — the #451 shape.
//!   - (PIN) The enclosure built from PRODUCTION buildings
//!     instead of walls (4 Barracks + 4 Houses whose
//!     footprints cover the same ring cells — a base built
//!     tight around its own TC): identical pristine signature.
//!   - (PIN) The ATTACKER enclosed instead (real wall ring
//!     around its spawn pocket, foe at the TC outside):
//!     identical pristine signature.
//!
//! The pins assert the DE-expected behaviour (order retained,
//! attacker closes to the barrier face), so they FAIL on
//! pristine 43a16e5 and PASS with the chase fix (occorder23's
//! fallback + retry, cherry-picked verbatim as the second
//! commit of this package and verified against this suite).

use crate::build::{Building, BuildingKind};
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

/// A completed building record at anchor cell `(cx, cy)`,
/// garrison_tests' `placed` idiom, plus the dynamic grid
/// block its footprint carries in the live sim.
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

/// Foe Spearman walks in from 20 wu south of the TC centre and
/// attacks the TC by real command; settles 900 ticks (measured
/// in the engageprobe/occorder23 line: it ends in the r<=3
/// disc, draining at ~0.5 HP/s). `enclose` then places the
/// scene's real buildings. Attacker Militia spawns at
/// `atk_off` (TC-relative), SetStance Aggressive + Attack by
/// real commands, and the window runs.
fn scene(
    enclose: &mut dyn FnMut(&mut Sim, (u32, u32)),
    atk_off: (f64, f64),
    window: u64,
) -> Trace {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(
        &mut sim,
        FOE,
        EntityKind::Spearman,
        OWNER_OPPONENT,
        tcx,
        tcy + I16F16::from_num(20),
    );
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..900 {
        sim.tick();
    }
    // The foe must genuinely be the live shape before the
    // enclosure lands: settled in the disc, TC already draining.
    let foe = ent(&sim, FOE);
    let d = f(steer::vec_len(foe.pos_x - tcx, foe.pos_y - tcy));
    assert!(d <= 3.5, "foe did not settle in the TC disc: dist {d:.2}");
    assert!(
        f(ent(&sim, 0).hp) < 2400.0,
        "foe settled but the TC is not draining"
    );
    let cc = sim.grid.world_to_cell(tcx, tcy);
    enclose(&mut sim, cc);
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
        if travel > tr.max_travel {
            tr.max_travel = travel;
        }
        if tr.first_dmg.is_none() && f(fo.hp) < foe_hp0 {
            tr.first_dmg = Some(t);
        }
        if tr.drop_tick.is_none() && !fo.dead && a.attack_target != Some(FOE) {
            tr.drop_tick = Some(t);
        }
        if fo.dead {
            tr.dead_tick = Some(t);
            break;
        }
    }
    let a = ent(&sim, ATK);
    let fo = ent(&sim, FOE);
    tr.end_target = a.attack_target;
    tr.end_dist = f(steer::vec_len(fo.pos_x - a.pos_x, fo.pos_y - a.pos_y));
    tr.tc_end = f(ent(&sim, 0).hp);
    tr.foe_hp_end = f(fo.hp);
    tr
}

fn ring_cells(cc: (u32, u32), r: i32) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for dy in -r..=r {
        for dx in -r..=r {
            if dx.abs() != r && dy.abs() != r {
                continue;
            }
            out.push(((cc.0 as i32 + dx) as u32, (cc.1 as i32 + dy) as u32));
        }
    }
    out
}

/// A complete ring of real PalisadeWall records at Chebyshev
/// radius 3 cells around the TC centre cell; `gap_south`
/// leaves the due-south cell unbuilt.
fn wall_ring(gap_south: bool) -> impl FnMut(&mut Sim, (u32, u32)) {
    move |sim: &mut Sim, cc: (u32, u32)| {
        for (i, cell) in ring_cells(cc, 3).into_iter().enumerate() {
            if gap_south && cell == (cc.0, cc.1 + 3) {
                continue;
            }
            place_record(sim, 9_900 + i as u32, BuildingKind::PalisadeWall, OWNER_PLAYER, cell);
        }
    }
}

/// The same ring cells covered by production buildings:
/// Barracks (3x3) on the four cardinal stretches, Houses
/// (2x2) on the four corners — a base built tight around its
/// own Town Center.
fn building_ring() -> impl FnMut(&mut Sim, (u32, u32)) {
    move |sim: &mut Sim, cc: (u32, u32)| {
        let (x, y) = (cc.0 as i32, cc.1 as i32);
        let at = |dx: i32, dy: i32| ((x + dx) as u32, (y + dy) as u32);
        place_record(sim, 9_950, BuildingKind::Barracks, OWNER_PLAYER, at(-1, -5));
        place_record(sim, 9_951, BuildingKind::Barracks, OWNER_PLAYER, at(-1, 3));
        place_record(sim, 9_952, BuildingKind::Barracks, OWNER_PLAYER, at(3, -1));
        place_record(sim, 9_953, BuildingKind::Barracks, OWNER_PLAYER, at(-5, -1));
        place_record(sim, 9_954, BuildingKind::House, OWNER_PLAYER, at(-3, -3));
        place_record(sim, 9_955, BuildingKind::House, OWNER_PLAYER, at(2, -3));
        place_record(sim, 9_956, BuildingKind::House, OWNER_PLAYER, at(-3, 2));
        place_record(sim, 9_957, BuildingKind::House, OWNER_PLAYER, at(2, 2));
    }
}

fn assert_engages(tag: &str, tr: &Trace) {
    assert!(
        tr.drop_tick.is_none(),
        "{tag}: attack target dropped at {:?} while foe alive: {tr:?}",
        tr.drop_tick
    );
    assert!(
        tr.first_dmg.is_some() && tr.first_dmg.unwrap() <= 600,
        "{tag}: first damage too late: {tr:?}"
    );
    assert!(tr.dead_tick.is_some(), "{tag}: foe never died: {tr:?}");
}

/// THE PIN, real walls. On pristine this fails at the first
/// assertion: the target drops at t=0 and the attacker never
/// moves (travel 0.0 — the #451 "idle at the Barracks").
#[test]
fn real_wall_ring_enclosed_foe_order_is_retained() {
    let tr = scene(&mut wall_ring(false), (0.0, 28.0), 14_400);
    assert!(
        tr.drop_tick.is_none(),
        "attack target dropped at {:?} while the enclosed foe lived: {tr:?}",
        tr.drop_tick
    );
    assert_eq!(tr.end_target, Some(FOE), "order not retained at window end: {tr:?}");
    assert!(
        tr.max_travel > 8.0,
        "attacker never closed toward the ring (travel {:.1}): {tr:?}",
        tr.max_travel
    );
    // Honest outcome while the ring stands: still unreachable,
    // so the foe keeps draining — but the order persists.
    assert!(
        tr.dead_tick.is_none() && tr.tc_end < 2396.0,
        "enclosed foe should still be draining: {tr:?}"
    );
}

/// THE PIN, production buildings. Same pristine signature:
/// the enclosure's composition (walls vs buildings) is not
/// the mechanism — any closed barrier of real buildings is.
#[test]
fn building_ring_enclosed_foe_order_is_retained() {
    let tr = scene(&mut building_ring(), (0.0, 28.0), 7_200);
    assert!(
        tr.drop_tick.is_none(),
        "attack target dropped at {:?} while the enclosed foe lived: {tr:?}",
        tr.drop_tick
    );
    assert_eq!(tr.end_target, Some(FOE), "order not retained at window end: {tr:?}");
    assert!(
        tr.max_travel > 8.0,
        "attacker never closed toward the buildings (travel {:.1}): {tr:?}",
        tr.max_travel
    );
}

/// Control: the one-cell gap in the real wall ring engages on
/// pristine and fixed alike.
#[test]
fn real_wall_ring_with_gap_engages() {
    let tr = scene(&mut wall_ring(true), (0.0, 28.0), 7_200);
    assert_engages("gap", &tr);
}

/// Control: a real Barracks adjacent to the TC (its footprint
/// south of the TC, the attacker spawning south of IT) is not
/// an enclosure — the Militia paths around it and engages.
#[test]
fn barracks_adjacent_to_tc_does_not_block_engagement() {
    let mut place = |sim: &mut Sim, cc: (u32, u32)| {
        // TC footprint spans cc-2..cc+1 cells per axis (4x4
        // around the centre cell); the Barracks stands due
        // south, clear of it.
        place_record(
            sim,
            9_960,
            BuildingKind::Barracks,
            OWNER_PLAYER,
            (cc.0 - 1, cc.1 + 2),
        );
    };
    let tr = scene(&mut place, (0.0, 28.0), 7_200);
    assert_engages("barracks-adjacent", &tr);
}

/// THE PIN, attacker side: the Militia spawns inside a real
/// wall pocket (ring radius 2 cells around its spawn cell),
/// the foe drains the TC outside it. Pristine: drop at t=0,
/// travel 0.0. Fixed: order retained, unit closes to its own
/// wall face.
#[test]
fn attacker_enclosed_in_real_wall_pocket_retains_order() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(&mut sim, FOE, EntityKind::Spearman, OWNER_OPPONENT, tcx, tcy + I16F16::from_num(20));
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..900 {
        sim.tick();
    }
    let atk_pos = (tcx, tcy + I16F16::from_num(28));
    let ac = sim.grid.world_to_cell(atk_pos.0, atk_pos.1);
    for (i, cell) in ring_cells(ac, 2).into_iter().enumerate() {
        place_record(&mut sim, 9_970 + i as u32, BuildingKind::PalisadeWall, OWNER_PLAYER, cell);
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
        if travel > max_travel {
            max_travel = travel;
        }
        if drop.is_none() && ent(&sim, FOE).dead == false && a.attack_target != Some(FOE) {
            drop = Some(t);
        }
    }
    assert!(
        drop.is_none(),
        "attack target dropped at {drop:?} while the foe lived (travel {max_travel:.1})"
    );
    assert_eq!(ent(&sim, ATK).attack_target, Some(FOE), "order not retained");
    assert!(
        max_travel > 1.0,
        "attacker never closed to its own wall face (travel {max_travel:.1})"
    );
}
