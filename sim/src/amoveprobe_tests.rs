//! AMOVEPROBE (2026-11-18): diagnosis probe for the occorder4
//! handoff's attempt 3 — an Attack Move ordered PAST an enemy
//! unit standing inside a friendly Town Center footprint did
//! not engage in live play: the moving unit passed by and idled
//! at the destination. The client issues the correct
//! `attackmove` ground order (occorder4, client side closed);
//! the open question is sim-side auto-acquisition: does
//! `acquire_targets` / the `attack_auto` chase engage a target
//! standing on building-footprint / obstacle cells, the way an
//! explicit Attack order does?
//!
//! Staged variants (all Militia vs Militia, Aggressive stance,
//! the sim's spawn default):
//!   1. attack-move, target on open ground (control);
//!   2. attack-move, target inside the player's real Town
//!      Center footprint (the live scene — a standing TC never
//!      blocks its ground, `convert_to_town_center`);
//!   3. attack-move, target inside a BLOCKING building
//!      footprint (a Barracks record: its cells are
//!      dynamic-blocked, so the target stands in a footprint
//!      pocket no path can target);
//!   4. explicit Attack, target inside the blocking footprint
//!      (does the explicit-order chase do better?);
//!   5. explicit Attack, target inside the TC footprint
//!      (control for 2).
//!
//! Every test asserts the DE-expected outcome (the target takes
//! damage) and reports the measured numbers in its assert
//! messages: ticks with the target acquired, acquire→clear
//! churn, minimum approach distance, target HP, and where the
//! attacker ended up. A red test here IS the diagnosis.

use crate::build::{Building, BuildingKind};
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

/// A clear land cell near the map centre (5×5 unblocked, dry) —
/// same helper shape as mov03_tests.
fn clear_land_spot(sim: &Sim) -> (u32, u32) {
    let g = sim.grid();
    for cy in 40..80 {
        for cx in 40..80 {
            let ok = (0..5).all(|dy| {
                (0..5).all(|dx| {
                    let (x, y) = (cx + dx, cy + dy);
                    !g.is_blocked(x, y) && !g.is_water(x, y) && !g.is_shallows(x, y)
                })
            });
            if ok {
                return (cx + 2, cy + 2);
            }
        }
    }
    panic!("seed 42: no clear land spot near the centre");
}

/// A placed, complete Barracks record (garrison_tests' `placed`
/// shape) — the caller marks its footprint cells dynamic-blocked.
fn placed_barracks(sim: &Sim, id: u32, owner: u8, cell: (u32, u32)) -> Building {
    let kind = BuildingKind::Barracks;
    let size = kind.footprint().w;
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    Building {
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
    }
}

const ATK: u32 = 9_700;
const FOE: u32 = 9_701;
const TICKS: u64 = 3600; // 60 game-seconds at 60 ticks/s

#[derive(Default, Debug)]
struct Measure {
    acquired_ticks: u64,
    acquire_edges: u64,
    clears_while_amove: u64,
    min_dist_bits: i32,
    foe_hp_end: f64,
    foe_dead: bool,
    atk_hp_end: f64,
    atk_end: (f64, f64),
    foe_end: (f64, f64),
    amove_open_at_end: bool,
}

/// Run a staged scene: attacker ATK, target FOE, measuring the
/// engagement over TICKS ticks. `dest` is where the attacker was
/// headed (attack-move destination, or the foe for Attack).
fn measure(sim: &mut Sim, dest: (I16F16, I16F16)) -> Measure {
    let mut m = Measure {
        min_dist_bits: i32::MAX,
        ..Default::default()
    };
    let mut was_acquired = false;
    for _ in 0..TICKS {
        sim.tick();
        let a = ent(sim, ATK);
        let t = ent(sim, FOE);
        let d = steer::vec_len(t.pos_x - a.pos_x, t.pos_y - a.pos_y);
        if d.to_bits() < m.min_dist_bits {
            m.min_dist_bits = d.to_bits();
        }
        let acquired = a.attack_target == Some(FOE);
        if acquired {
            m.acquired_ticks += 1;
            if !was_acquired {
                m.acquire_edges += 1;
            }
        } else if was_acquired && a.attack_move.is_some() {
            m.clears_while_amove += 1;
        }
        was_acquired = acquired;
        if t.dead {
            break;
        }
    }
    let a = ent(sim, ATK);
    let t = ent(sim, FOE);
    m.foe_hp_end = f(t.hp);
    m.foe_dead = t.dead;
    m.atk_hp_end = f(a.hp);
    m.atk_end = (f(a.pos_x), f(a.pos_y));
    m.foe_end = (f(t.pos_x), f(t.pos_y));
    m.amove_open_at_end = a.attack_move.is_some();
    let _ = dest;
    eprintln!("PROBE {}", report(&m));
    m
}

fn report(m: &Measure) -> String {
    format!(
        "acquired_ticks={} acquire_edges={} clears_while_amove={} min_dist={:.2} foe_hp_end={:.0} foe_dead={} atk_hp_end={:.0} atk_end=({:.1},{:.1}) foe_end=({:.1},{:.1}) amove_open_at_end={}",
        m.acquired_ticks,
        m.acquire_edges,
        m.clears_while_amove,
        f(I16F16::from_bits(m.min_dist_bits)),
        m.foe_hp_end,
        m.foe_dead,
        m.atk_hp_end,
        m.atk_end.0,
        m.atk_end.1,
        m.foe_end.0,
        m.foe_end.1,
        m.amove_open_at_end,
    )
}

/// Variant 1 (control): attack-move past an enemy on open
/// ground must engage.
#[test]
fn amove_open_ground_control_engages() {
    let mut sim = Sim::new(42);
    let (cx, cy) = clear_land_spot(&sim);
    let (ox, oy) = sim.grid().cell_center(cx, cy);
    spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, ox, oy);
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        ox - I16F16::from_num(24),
        oy,
    );
    let dest = (ox + I16F16::from_num(24), oy);
    sim.push_command(Command::AttackMove {
        entity: ATK,
        x: dest.0,
        y: dest.1,
    });
    let m = measure(&mut sim, dest);
    assert!(
        m.foe_dead || m.foe_hp_end < 40.0,
        "CONTROL FAILED — attack-move did not engage on open ground: {}",
        report(&m)
    );
}

/// Variant 2 (the live scene): attack-move past an enemy
/// standing inside the player's own Town Center footprint.
#[test]
fn amove_tc_footprint_engages() {
    let mut sim = Sim::new(42);
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == OWNER_PLAYER)
        .map(|e| (e.pos_x, e.pos_y))
        .expect("seed 42 has a player Town Center");
    // Inside the 4×4 footprint (16 wu wide; the TC entity stands
    // on its (1,1) cell centre, so the footprint spans
    // tc−6 .. tc+10 on each axis): the foe stands 4 wu east of
    // the TC centre, well inside.
    let foe_pos = (tc.0 + I16F16::from_num(4), tc.1);
    assert!(
        sim.grid().is_blocked(
            sim.grid().world_to_cell(foe_pos.0, foe_pos.1).0,
            sim.grid().world_to_cell(foe_pos.0, foe_pos.1).1
        ) == false,
        "staging note: a standing TC's ground is expected unblocked"
    );
    spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, foe_pos.0, foe_pos.1);
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        foe_pos.0 - I16F16::from_num(28),
        foe_pos.1,
    );
    let dest = (foe_pos.0 + I16F16::from_num(28), foe_pos.1);
    sim.push_command(Command::AttackMove {
        entity: ATK,
        x: dest.0,
        y: dest.1,
    });
    let m = measure(&mut sim, dest);
    assert!(
        m.foe_dead || m.foe_hp_end < 40.0,
        "TC-FOOTPRINT attack-move did not engage: {}",
        report(&m)
    );
}

/// Variant 3: attack-move past an enemy standing inside a
/// BLOCKING building footprint (Barracks 3×3, cells
/// dynamic-blocked — a footprint pocket).
#[test]
fn amove_building_footprint_pocket_engages() {
    let mut sim = Sim::new(42);
    let (cx, cy) = clear_land_spot(&sim);
    // Barracks anchor two cells NW of the spot so its 3×3
    // footprint centres on the spot.
    let anchor = (cx - 1, cy - 1);
    let b = placed_barracks(&sim, 9_710, OWNER_PLAYER, anchor);
    let (bx, by) = (b.pos_x, b.pos_y);
    sim.buildings.push(b);
    for dy in 0..3u32 {
        for dx in 0..3u32 {
            sim.grid.set_dynamic(anchor.0 + dx, anchor.1 + dy, true);
        }
    }
    // The foe stands at the building's centre — inside blocked
    // cells, unreachable as a pathing goal.
    spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, bx, by);
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        bx - I16F16::from_num(28),
        by,
    );
    let dest = (bx + I16F16::from_num(28), by);
    sim.push_command(Command::AttackMove {
        entity: ATK,
        x: dest.0,
        y: dest.1,
    });
    let m = measure(&mut sim, dest);
    assert!(
        m.foe_dead || m.foe_hp_end < 40.0,
        "BUILDING-POCKET attack-move did not engage: {}",
        report(&m)
    );
}

/// Variant 4: explicit Attack on the pocket target — the
/// stanceatk family's claim, staged on footprint cells.
#[test]
fn explicit_attack_building_footprint_pocket_engages() {
    let mut sim = Sim::new(42);
    let (cx, cy) = clear_land_spot(&sim);
    let anchor = (cx - 1, cy - 1);
    let b = placed_barracks(&sim, 9_710, OWNER_PLAYER, anchor);
    let (bx, by) = (b.pos_x, b.pos_y);
    sim.buildings.push(b);
    for dy in 0..3u32 {
        for dx in 0..3u32 {
            sim.grid.set_dynamic(anchor.0 + dx, anchor.1 + dy, true);
        }
    }
    spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, bx, by);
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        bx - I16F16::from_num(28),
        by,
    );
    sim.push_command(Command::Attack {
        entity: ATK,
        target: FOE,
    });
    let m = measure(&mut sim, (bx, by));
    assert!(
        m.foe_dead || m.foe_hp_end < 40.0,
        "BUILDING-POCKET explicit Attack did not engage: {}",
        report(&m)
    );
}

/// Stage the blocking-pocket scene with the foe PINNED inside
/// (Stand Ground: it fights what reaches it, never chases out)
/// — the discriminating form of variants 3/4, where the foe
/// cannot rescue the engagement by walking out of the pocket.
/// Returns (sim, building centre).
fn pocket_scene_pinned() -> (Sim, (I16F16, I16F16)) {
    let mut sim = Sim::new(42);
    let (cx, cy) = clear_land_spot(&sim);
    let anchor = (cx - 1, cy - 1);
    let b = placed_barracks(&sim, 9_710, OWNER_PLAYER, anchor);
    let centre = (b.pos_x, b.pos_y);
    sim.buildings.push(b);
    for dy in 0..3u32 {
        for dx in 0..3u32 {
            sim.grid.set_dynamic(anchor.0 + dx, anchor.1 + dy, true);
        }
    }
    spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, centre.0, centre.1);
    let fi = sim.entities().iter().position(|e| e.id == FOE).unwrap();
    sim.entities[fi].stance = crate::Stance::StandGround;
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        centre.0 - I16F16::from_num(28),
        centre.1,
    );
    (sim, centre)
}

/// Variant 6: attack-move past a PINNED pocket target.
#[test]
fn amove_building_footprint_pocket_pinned_engages() {
    let (mut sim, (bx, by)) = pocket_scene_pinned();
    let dest = (bx + I16F16::from_num(28), by);
    sim.push_command(Command::AttackMove {
        entity: ATK,
        x: dest.0,
        y: dest.1,
    });
    let m = measure(&mut sim, dest);
    assert!(
        m.foe_dead || m.foe_hp_end < 40.0,
        "PINNED BUILDING-POCKET attack-move did not engage: {}",
        report(&m)
    );
}

/// Variant 7: explicit Attack on a PINNED pocket target — does
/// the explicit-order chase reach a target no path can target,
/// when the target cannot walk out to meet it?
#[test]
fn explicit_attack_building_footprint_pocket_pinned_engages() {
    let (mut sim, (bx, by)) = pocket_scene_pinned();
    sim.push_command(Command::Attack {
        entity: ATK,
        target: FOE,
    });
    let m = measure(&mut sim, (bx, by));
    assert!(
        m.foe_dead || m.foe_hp_end < 40.0,
        "PINNED BUILDING-POCKET explicit Attack did not engage: {}",
        report(&m)
    );
}

/// Variant 8: attack-move past a PINNED target inside the TC
/// footprint (the foe cannot walk out to meet the attacker).
#[test]
fn amove_tc_footprint_pinned_engages() {
    let mut sim = Sim::new(42);
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == OWNER_PLAYER)
        .map(|e| (e.pos_x, e.pos_y))
        .expect("seed 42 has a player Town Center");
    let foe_pos = (tc.0 + I16F16::from_num(4), tc.1);
    spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, foe_pos.0, foe_pos.1);
    let fi = sim.entities().iter().position(|e| e.id == FOE).unwrap();
    sim.entities[fi].stance = crate::Stance::StandGround;
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        foe_pos.0 - I16F16::from_num(28),
        foe_pos.1,
    );
    let dest = (foe_pos.0 + I16F16::from_num(28), foe_pos.1);
    sim.push_command(Command::AttackMove {
        entity: ATK,
        x: dest.0,
        y: dest.1,
    });
    let m = measure(&mut sim, dest);
    assert!(
        m.foe_dead || m.foe_hp_end < 40.0,
        "PINNED TC-FOOTPRINT attack-move did not engage: {}",
        report(&m)
    );
}

/// Variant 9: attack-move past an open-ground enemy, for each
/// military kind the roster now fields — CMB-08 promises the
/// chain to "a military unit", but `acquire_targets` whitelists
/// Militia | ManAtArms | Archer | ScoutCavalry. Kinds outside
/// the list are measured here on OPEN ground (no footprint
/// anywhere), so a failure is the kind list, not the terrain.
#[test]
fn amove_open_ground_all_military_kinds_engage() {
    let mut failures: Vec<String> = Vec::new();
    for kind in [
        EntityKind::Militia,
        EntityKind::ManAtArms,
        EntityKind::Archer,
        EntityKind::ScoutCavalry,
        EntityKind::Spearman,
        EntityKind::Skirmisher,
        EntityKind::LightCavalry,
        EntityKind::Knight,
        EntityKind::CavalryArcher,
    ] {
        let mut sim = Sim::new(42);
        let (cx, cy) = clear_land_spot(&sim);
        let (ox, oy) = sim.grid().cell_center(cx, cy);
        spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, ox, oy);
        spawn(
            &mut sim,
            ATK,
            kind,
            OWNER_PLAYER,
            ox - I16F16::from_num(24),
            oy,
        );
        let dest = (ox + I16F16::from_num(24), oy);
        sim.push_command(Command::AttackMove {
            entity: ATK,
            x: dest.0,
            y: dest.1,
        });
        let m = measure(&mut sim, dest);
        eprintln!("PROBE kind={:?} {}", kind, report(&m));
        if !(m.foe_dead || m.foe_hp_end < 40.0) {
            failures.push(format!("{:?}: {}", kind, report(&m)));
        }
    }
    assert!(
        failures.is_empty(),
        "kinds that never engaged under attack-move on open ground:\n{}",
        failures.join("\n")
    );
}

/// Diagnostic (always passes): trace the pinned-pocket
/// attack-move scene — can the chase even path to the foe, and
/// how do attacker/foe actually move tick by tick?
#[test]
fn probe_trace_pocket() {
    let (mut sim, (bx, by)) = pocket_scene_pinned();
    {
        let foe = ent(&sim, FOE);
        let atk = ent(&sim, ATK);
        let fc = sim.grid().world_to_cell(foe.pos_x, foe.pos_y);
        eprintln!(
            "TRACE staging: building centre=({:.1},{:.1}) foe at=({:.1},{:.1}) cell={:?} blocked={} dynamic={} atk at=({:.1},{:.1})",
            f(bx),
            f(by),
            f(foe.pos_x),
            f(foe.pos_y),
            fc,
            sim.grid().is_blocked(fc.0, fc.1),
            sim.grid().is_dynamic(fc.0, fc.1),
            f(atk.pos_x),
            f(atk.pos_y),
        );
    }
    // Direct pathing question first: chase-order to the foe's
    // exact position from the attacker's start.
    let ai = sim.entities().iter().position(|e| e.id == ATK).unwrap();
    let direct = sim.set_move_order(ai, bx, by);
    eprintln!(
        "TRACE set_move_order(attacker -> foe pos inside pocket) = {} ; foe cell blocked = {}",
        direct,
        sim.grid().is_blocked(
            sim.grid().world_to_cell(bx, by).0,
            sim.grid().world_to_cell(bx, by).1
        )
    );
    // Re-stage the order as the player would: attack-move past.
    sim.entities[ai].order = None;
    let dest = (bx + I16F16::from_num(28), by);
    sim.push_command(Command::AttackMove {
        entity: ATK,
        x: dest.0,
        y: dest.1,
    });
    for t in 0..TICKS {
        sim.tick();
        if t % 120 == 0 || ent(&sim, FOE).dead {
            let a = ent(&sim, ATK);
            let foe = ent(&sim, FOE);
            eprintln!(
                "TRACE t={} atk=({:.1},{:.1}) foe=({:.1},{:.1}) foe_hp={:.0} atk_target={:?} order_target={:?} amove={}",
                t,
                f(a.pos_x),
                f(a.pos_y),
                f(foe.pos_x),
                f(foe.pos_y),
                f(foe.hp),
                a.attack_target,
                a.order.as_ref().map(|o| (f(o.target_x), f(o.target_y))),
                a.attack_move.is_some(),
            );
            if foe.dead {
                break;
            }
        }
    }
}

/// Variant 5 (control for 2): explicit Attack on the enemy
/// inside the TC footprint.
#[test]
fn explicit_attack_tc_footprint_engages() {
    let mut sim = Sim::new(42);
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == OWNER_PLAYER)
        .map(|e| (e.pos_x, e.pos_y))
        .expect("seed 42 has a player Town Center");
    let foe_pos = (tc.0 + I16F16::from_num(4), tc.1);
    spawn(&mut sim, FOE, EntityKind::Militia, OWNER_OPPONENT, foe_pos.0, foe_pos.1);
    spawn(
        &mut sim,
        ATK,
        EntityKind::Militia,
        OWNER_PLAYER,
        foe_pos.0 - I16F16::from_num(28),
        foe_pos.1,
    );
    sim.push_command(Command::Attack {
        entity: ATK,
        target: FOE,
    });
    let m = measure(&mut sim, foe_pos);
    assert!(
        m.foe_dead || m.foe_hp_end < 40.0,
        "TC-FOOTPRINT explicit Attack did not engage: {}",
        report(&m)
    );
}
