//! VILLAGEREDGE (2026-11-23): the NEXT face of the hidden-
//! attacker defect, staged after villagerattack (d5b1733)
//! excluded the plain combination.
//!
//! Live #488 (occlog-verified, real client, TWICE) and #487:
//! a VILLAGER who is IDLE immediately before the order (not
//! gathering, not carrying, not returning — proven twice in
//! #488) is right-click-ordered to attack an enemy unit
//! standing under / inside the player's Town Center roof that
//! is actively attacking the TC. The order issues cleanly
//! ({t:'attack', ok:true}) but the villager walks only to the
//! TC's SOUTH EDGE and stands idle there; the enemy stays at
//! full HP and keeps draining the TC. #488 instances: Scout
//! Cavalry id 28 under the roof, and a target picked by
//! outline (tg:74). #487: a Militia under the roof.
//!
//! villagerattack already excluded: villager vs full-HP
//! Militia inside the TC footprint attacking TC entity 0,
//! from all sides / interior cells / pillars / adjacency.
//! This suite stages the combinations that match the LIVE
//! shape and were still untested:
//!   (a) foe = SCOUT CAVALRY (not Militia) under the roof
//!       attacking the TC, villager attacking from the SOUTH
//!       side specifically; foe settled from the south, and
//!       foe at the centre and at deep interior cells;
//!   (b) foe at FOOTPRINT-EDGE stand positions — the stand a
//!       BUILDING-shaped attacker takes (range measured from
//!       the footprint edge; engagepillar measured that a
//!       TC-entity attacker never settles there): pinned
//!       StandGround Scout at the SOUTH edge matrix, plus a
//!       TRUE `AttackBuilding` face — a Scout ordered on a
//!       House record whose footprint abuts / overlaps the
//!       TC's south edge, settling at that building's edge
//!       stand while its HP drains;
//!   (c) CROWD: 3 other friendly villagers already inside /
//!       near the footprint (adjacent to the foe) when the
//!       attack order lands — separation pressure — against
//!       both a Scout foe and a Militia foe;
//!   (d) foe RE-ORDERED mid-approach by an AI-style re-issue
//!       of its TC attack (single re-issue at fixed ticks,
//!       periodic re-issue, and re-issue at the moment the
//!       villager crosses the TC south edge).
//!
//! Geometry: TC footprint 4x4 cells / 16 wu wide centred on
//! the TC position; interior cell centres at TC-relative
//! offsets in {-6,-2,+2,+6} per axis, centre (0,0), the
//! footprint edge at |offset| = 8, south = +y. Every order on
//! the path under test is a real `Command` (foe Attack on the
//! TC / AttackBuilding / SetStance, villager Attack —
//! `can_attack` asserted where the live issue returned
//! ok:true). Spawn positions are scene selection only.
//!
//! Per scene the trace records: villager final position /
//! distance to foe, foe HP over ticks (first-damage tick,
//! death tick, HP samples in the capture lines), first-strike
//! tick (the first-damage tick), and attack_target /
//! strike_tick / next_attack_tick / attack_engaged at the
//! idle moment and at window end.
//!
//! OUTCOME (measured on base b33f9d7, see handoff
//! `villageredge-build.md`): NO scene reproduces the
//! arrive-and-idle live shape — 21 scenes, max_stall_out = 0
//! and max_stall_in = 38 (the first-strike windup) in every
//! scene where the villager survives. Measured facts:
//!   * (a) A Scout under a real Attack order on the TC
//!     settles at (0,2.96) from the south / (2.96,0) from the
//!     east, HOLDS (0,0) at the centre, and walks inward
//!     from the (±6,*) cells to stands (2.96,0)/(0,2.96)/
//!     (±2.10,±2.10) — always inside the footprint, never at
//!     an edge. The south villager closes PAST the south
//!     edge to (0,5.92), dist 2.96, first damage t=302–491,
//!     Scout dead t=1929–2171, villager 25/25 (the
//!     TC-attacking Scout never retargets).
//!   * (b) At the pinned south edge stands (0,8)–(6,8) and
//!     at the true AttackBuilding stands — House abutting
//!     the TC south edge: Scout settles (0,16.96); House
//!     overlapping the southern row: (0,12.96), House HP
//!     verifiably draining in both — the villager closes to
//!     dist 2.95–2.99 and FIGHTS; these foes (no TC-entity
//!     order running) retarget the villager, and in 7 of 8
//!     scenes the Scout kills the villager (foe left at
//!     18/45) — an engagement, the opposite of the live
//!     shape. (The `drop` tick in those scenes is the
//!     attack_target clearing on the villager's death tick,
//!     after damage both ways.)
//!   * (c) Crowds change nothing: bystanders are never
//!     displaced or hurt; the attacker closes to dist
//!     2.96–3.00 and kills the foe (t=1933–1982) at 25/25.
//!   * (d) Re-issuing the foe's TC attack mid-approach
//!     (once at t=+90, every 300 ticks, or at the south-edge
//!     crossing — all verifiably fired) changes nothing:
//!     identical first damage t=302, foe dead t=1982.
//! Per package discipline NO production fix is made; these
//! pins are the deliverable, and the remaining untested
//! territory is named precisely in the handoff.

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

const FOE: u32 = 9_711;
const VIL: u32 = 9_712;
const BY1: u32 = 9_713;
const BY2: u32 = 9_714;
const BY3: u32 = 9_715;
const ANNEX: u32 = 9_751;
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
    attack_building: Option<u32>,
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
    foe_attack_building: Option<u32>,
}

fn capture(sim: &Sim, atk: u32) -> Capture {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    let a = ent(sim, atk);
    let fo = ent(sim, FOE);
    Capture {
        tick: sim.tick,
        attack_target: a.attack_target,
        attack_building: a.attack_building,
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
        foe_attack_building: fo.attack_building,
    }
}

fn print_capture(tag: &str, when: &str, c: &Capture) {
    eprintln!(
        "VILLAGEREDGE CAPTURE {tag} {when}: tick={} attack_target={:?} attack_building={:?} has_order={} strike_tick={:?} next_attack_tick={} engaged={} dist={:.2} atk_off=({:.2},{:.2}) foe_off=({:.2},{:.2}) foe_hp={:.0} atk_hp={:.0} tc_hp={:.0} foe_attack_target={:?} foe_attack_building={:?}",
        c.tick, c.attack_target, c.attack_building, c.has_order, c.strike_tick,
        c.next_attack_tick, c.attack_engaged, c.dist, c.atk_off.0, c.atk_off.1,
        c.foe_off.0, c.foe_off.1, c.foe_hp, c.atk_hp, c.tc_hp,
        c.foe_attack_target, c.foe_attack_building
    );
}

#[derive(Default, Debug)]
struct Trace {
    first_dmg: Option<u64>,
    first_hurt: Option<u64>,
    drop_tick: Option<u64>,
    dead_tick: Option<u64>,
    atk_dead_tick: Option<u64>,
    min_dist: f64,
    max_stall_out: u64,
    max_stall_in: u64,
    idle: Option<Capture>,
    end: Option<Capture>,
}

impl Trace {
    /// The live #487/#488 shape: no damage EITHER WAY while
    /// both live, the attacker idle (no move order) for
    /// >= 600 ticks (out of reach OR in reach without a
    /// strike), or the target dropped while the foe lives.
    /// An attacker that DIES fighting is an engagement.
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

/// Run the observation window. `hook` runs BEFORE each tick
/// with the number of ticks already elapsed in the window —
/// scene (d) uses it to re-issue the foe's TC attack
/// mid-approach.
fn run_window_hook(
    tag: &str,
    sim: &mut Sim,
    atk: u32,
    window: u64,
    mut hook: impl FnMut(&mut Sim, u64),
) -> Trace {
    let foe_hp0 = f(ent(sim, FOE).hp);
    let atk_hp0 = f(ent(sim, atk).hp);
    let mut tr = Trace { min_dist: f64::MAX, ..Default::default() };
    let mut stall_out = 0u64;
    let mut stall_in = 0u64;
    for t in 0..window {
        hook(sim, t);
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
        if t % 1200 == 1199 {
            let c = capture(sim, atk);
            print_capture(tag, "sample", &c);
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
        "VILLAGEREDGE {tag} first_dmg={:?} first_hurt={:?} drop={:?} dead={:?} atk_dead={:?} min_dist={:.2} max_stall_out={} max_stall_in={} live_shape={}",
        tr.first_dmg, tr.first_hurt, tr.drop_tick, tr.dead_tick, tr.atk_dead_tick,
        tr.min_dist, tr.max_stall_out, tr.max_stall_in, tr.live_shape()
    );
    tr
}

fn run_window(tag: &str, sim: &mut Sim, atk: u32, window: u64) -> Trace {
    run_window_hook(tag, sim, atk, window, |_, _| {})
}

fn order_attack(sim: &mut Sim, tag: &str, atk: u32) {
    assert!(sim.can_attack(atk, FOE), "{tag}: can_attack refused the live order");
    sim.push_command(Command::Attack { entity: atk, target: FOE });
}

/// Settle a foe of `kind` walking in from `spawn_off` under a
/// real Attack order on the TC (entity 0); assert the TC is
/// verifiably draining; return the settled TC-relative offset.
fn settle_foe(sim: &mut Sim, tag: &str, kind: EntityKind, spawn_off: (f64, f64)) -> Option<(f64, f64)> {
    let tc = sim.town_center();
    let (tcx, tcy) = (tc.pos_x, tc.pos_y);
    spawn(sim, FOE, kind, OWNER_OPPONENT,
          tcx + I16F16::from_num(spawn_off.0), tcy + I16F16::from_num(spawn_off.1));
    sim.push_command(Command::Attack { entity: FOE, target: 0 });
    for _ in 0..600 {
        sim.tick();
    }
    let fo = ent(sim, FOE);
    let settled = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
    let tc_hp = f(ent(sim, 0).hp);
    eprintln!(
        "VILLAGEREDGE {tag} foe settled off=({:.2},{:.2}) foe_hp={:.0} tc_hp={:.0} foe_attack_target={:?}",
        settled.0, settled.1, f(fo.hp), tc_hp, fo.attack_target
    );
    if tc_hp >= 2400.0 {
        return None;
    }
    Some(settled)
}

/// (a1) THE LIVE #488 GEOMETRY: foe SCOUT CAVALRY walks in
/// from the SOUTH under a real Attack order on the TC,
/// settles under the roof (recorded), TC verifiably draining;
/// an IDLE villager south of the TC (the live approach —
/// the villager stops at the SOUTH EDGE in the live runs) is
/// ordered to attack it. East-settled Scout is the control.
#[test]
fn scout_settled_villager_from_south() {
    let mut failures = Vec::new();
    for (name, foe_spawn, vil_off) in [
        ("scoutS-vilS", (0.0, 24.0), (0.0, 20.0)),
        ("scoutS-vilSE", (0.0, 24.0), (14.0, 16.0)),
        ("scoutE-vilS", (24.0, 0.0), (0.0, 20.0)),
    ] {
        let tag = format!("{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        let Some(settled) = settle_foe(&mut sim, &tag, EntityKind::ScoutCavalry, foe_spawn) else {
            failures.push(format!("{tag}: foe not draining TC"));
            continue;
        };
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tcx + I16F16::from_num(vil_off.0), tcy + I16F16::from_num(vil_off.1));
        // The live precondition: the villager is IDLE at the
        // order moment (no order, no task target).
        {
            let v = ent(&sim, VIL);
            assert!(v.order.is_none() && v.attack_target.is_none(),
                    "{tag}: staged villager is not idle at the order moment");
        }
        order_attack(&mut sim, &tag, VIL);
        let tr = run_window(&tag, &mut sim, VIL, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: settled={settled:?} {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "scout-settled failures:\n{}", failures.join("\n"));
}

/// (a2) SCOUT AT THE CENTRE AND DEEP INTERIOR CELLS: foe
/// Scout spawned AT (0,0), (0,6), (6,0), (6,6), (6,-6),
/// (-6,6) with its real Attack order on the TC issued at
/// spawn; after 30 ticks a villager due SOUTH of the foe
/// (the live approach side) attacks it.
#[test]
fn scout_interior_villager_from_south() {
    let cells: [(f64, f64); 6] = [
        (0.0, 0.0),
        (0.0, 6.0),
        (6.0, 0.0),
        (6.0, 6.0),
        (6.0, -6.0),
        (-6.0, 6.0),
    ];
    let mut failures = Vec::new();
    for foe_off in cells {
        let tag = format!("scout-interior-({},{})", foe_off.0, foe_off.1);
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(foe_off.1));
        sim.push_command(Command::Attack { entity: FOE, target: 0 });
        for _ in 0..30 {
            sim.tick();
        }
        let fo = ent(&sim, FOE);
        let foe_now = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
        eprintln!(
            "VILLAGEREDGE {tag} foe at order moment off=({:.2},{:.2}) (staged {:?}) foe_hp={:.0} tc_hp={:.0}",
            foe_now.0, foe_now.1, foe_off, f(fo.hp), f(ent(&sim, 0).hp)
        );
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(20));
        order_attack(&mut sim, &tag, VIL);
        let tr = run_window(&tag, &mut sim, VIL, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: foe_now={foe_now:?} {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "scout-interior failures:\n{}", failures.join("\n"));
}

/// (b1) THE EDGE-STAND FOE, SOUTH MATRIX: a Scout pinned
/// (StandGround) at the TC's SOUTH footprint-edge stands —
/// the positions a building-shaped attacker settles at,
/// which a TC-entity attacker never takes (engagepillar) —
/// and an idle villager due south attacks it. The pinned foe
/// carries no TC order: its POSITION is the staged variable
/// (the building-stand shape), exactly the engagepillar
/// pinned-matrix idiom.
#[test]
fn edge_stand_scout_south_matrix_villager() {
    let stands: [(f64, f64); 6] = [
        (0.0, 8.0),
        (0.0, 9.0),
        (0.0, 10.0),
        (2.0, 8.0),
        (-2.0, 8.0),
        (6.0, 8.0),
    ];
    let mut failures = Vec::new();
    for foe_off in stands {
        let tag = format!("edgeS-({},{})", foe_off.0, foe_off.1);
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(foe_off.1));
        sim.push_command(Command::SetStance { entity: FOE, stance: Stance::StandGround });
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tcx + I16F16::from_num(foe_off.0), tcy + I16F16::from_num(20));
        for _ in 0..30 {
            sim.tick();
        }
        let fo = ent(&sim, FOE);
        let foe_now = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
        eprintln!(
            "VILLAGEREDGE {tag} foe at order moment off=({:.2},{:.2}) (staged {:?}) foe_hp={:.0}",
            foe_now.0, foe_now.1, foe_off, f(fo.hp)
        );
        if (foe_now.0 - foe_off.0).abs() > 0.6 || (foe_now.1 - foe_off.1).abs() > 0.6 {
            failures.push(format!("{tag}: pinned foe left its stand {foe_now:?} vs {foe_off:?}"));
            continue;
        }
        order_attack(&mut sim, &tag, VIL);
        let tr = run_window(&tag, &mut sim, VIL, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: foe_now={foe_now:?} {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "edge-stand failures:\n{}", failures.join("\n"));
}

/// (b2) THE TRUE BUILDING-SHAPED FACE: a Scout under a real
/// `AttackBuilding` order on a House record whose footprint
/// abuts the TC's south edge (anchor row cc+2) or overlaps
/// its southern row (anchor row cc+1). The Scout settles at
/// the BUILDING's edge stand — measured, with the House HP
/// verifiably draining — at / beyond the TC south edge; an
/// idle villager due south then attacks the Scout.
#[test]
fn scout_attackbuilding_house_at_tc_south_edge() {
    let mut failures = Vec::new();
    for (name, row_delta) in [("abut-cc+2", 2i32), ("overlap-cc+1", 1i32)] {
        let tag = format!("bldg-{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        let cc = sim.grid.world_to_cell(tcx, tcy);
        place_record(&mut sim, ANNEX, BuildingKind::House, OWNER_PLAYER,
                     (cc.0 - 1, (cc.1 as i32 + row_delta) as u32));
        let house_hp0 = sim.buildings().iter().find(|b| b.id == ANNEX).unwrap().hp;
        spawn(&mut sim, FOE, EntityKind::ScoutCavalry, OWNER_OPPONENT,
              tcx, tcy + I16F16::from_num(26));
        assert!(sim.can_attack_building(FOE, ANNEX),
                "{tag}: can_attack_building refused the staged order");
        sim.push_command(Command::AttackBuilding { entity: FOE, building: ANNEX });
        for _ in 0..900 {
            sim.tick();
        }
        let fo = ent(&sim, FOE);
        let settled = (f(fo.pos_x - tcx), f(fo.pos_y - tcy));
        let house_hp = sim.buildings().iter().find(|b| b.id == ANNEX).map(|b| f(b.hp));
        eprintln!(
            "VILLAGEREDGE {tag} foe settled off=({:.2},{:.2}) foe_hp={:.0} house_hp={:?} (from {:.0}) foe_attack_building={:?} tc_hp={:.0}",
            settled.0, settled.1, f(fo.hp), house_hp, f(house_hp0), fo.attack_building,
            f(ent(&sim, 0).hp)
        );
        match house_hp {
            Some(hp) if hp < f(house_hp0) => {}
            other => {
                failures.push(format!("{tag}: foe not draining the building (hp {other:?})"));
                continue;
            }
        }
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tcx + I16F16::from_num(settled.0), tcy + I16F16::from_num(24));
        order_attack(&mut sim, &tag, VIL);
        let tr = run_window(&tag, &mut sim, VIL, 7_200);
        if tr.live_shape() {
            failures.push(format!("{tag}: settled={settled:?} {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "attackbuilding failures:\n{}", failures.join("\n"));
}

/// (c) CROWD / SEPARATION PRESSURE: the foe (Scout settled
/// from the south; Militia settled from the east as the
/// #487 control) is draining the TC when THREE other
/// friendly villagers are placed inside / near the footprint
/// — two adjacent to the foe, one at the south interior —
/// all idle; then the idle attacker villager due south is
/// ordered onto the foe.
#[test]
fn crowd_inside_footprint_villager_attacks() {
    let mut failures = Vec::new();
    for (name, kind, foe_spawn) in [
        ("crowd-scout", EntityKind::ScoutCavalry, (0.0, 24.0)),
        ("crowd-militia", EntityKind::Militia, (24.0, 0.0)),
    ] {
        let tag = format!("{name}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        let Some(settled) = settle_foe(&mut sim, &tag, kind, foe_spawn) else {
            failures.push(format!("{tag}: foe not draining TC"));
            continue;
        };
        // Bystanders: adjacent west/east of the settled foe
        // and at the south interior cell — all idle villagers.
        let by: [(u32, (f64, f64)); 3] = [
            (BY1, (settled.0 - 2.0, settled.1)),
            (BY2, (settled.0 + 2.0, settled.1)),
            (BY3, (settled.0, 6.0)),
        ];
        for (id, off) in by {
            spawn(&mut sim, id, EntityKind::Villager, OWNER_PLAYER,
                  tcx + I16F16::from_num(off.0), tcy + I16F16::from_num(off.1));
        }
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tcx, tcy + I16F16::from_num(20));
        for _ in 0..30 {
            sim.tick();
        }
        let c0 = capture(&sim, VIL);
        print_capture(&tag, "order-moment", &c0);
        order_attack(&mut sim, &tag, VIL);
        let tr = run_window(&tag, &mut sim, VIL, 7_200);
        // Record where the bystanders ended up (separation
        // evidence) regardless of outcome.
        for (id, _) in by {
            let b = ent(&sim, id);
            eprintln!(
                "VILLAGEREDGE {tag} bystander {id} end off=({:.2},{:.2}) hp={:.0} dead={}",
                f(b.pos_x - tcx), f(b.pos_y - tcy), f(b.hp), b.dead
            );
        }
        if tr.live_shape() {
            failures.push(format!("{tag}: settled={settled:?} {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "crowd failures:\n{}", failures.join("\n"));
}

/// (d) FOE RE-ORDERED MID-APPROACH: Scout settled draining
/// from the south; the villager attacks from the south; the
/// foe's TC attack is RE-ISSUED the way an AI loop re-issues
/// it — (d1) once at t=+90 (mid-approach), (d2) every 300
/// ticks through the whole window, (d3) once at the tick
/// the villager first crosses the TC south edge
/// (|atk_off.y| <= 9 while approaching).
#[test]
fn foe_reordered_mid_approach_villager_from_south() {
    let mut failures = Vec::new();
    for mode in ["once-90", "periodic-300", "at-edge"] {
        let tag = format!("reorder-{mode}");
        let mut sim = Sim::new(42);
        let tc = sim.town_center();
        let (tcx, tcy) = (tc.pos_x, tc.pos_y);
        let Some(settled) = settle_foe(&mut sim, &tag, EntityKind::ScoutCavalry, (0.0, 24.0)) else {
            failures.push(format!("{tag}: foe not draining TC"));
            continue;
        };
        spawn(&mut sim, VIL, EntityKind::Villager, OWNER_PLAYER,
              tcx, tcy + I16F16::from_num(20));
        order_attack(&mut sim, &tag, VIL);
        let mut reissued = 0u32;
        let mut crossed = false;
        let tr = run_window_hook(&tag, &mut sim, VIL, 7_200, |s, t| {
            let due = match mode {
                "once-90" => t == 90,
                "periodic-300" => t > 0 && t % 300 == 0,
                "at-edge" => {
                    if crossed {
                        false
                    } else {
                        let v = ent(s, VIL);
                        let inside = f(v.pos_y - tcy) <= 9.0 && f(v.pos_y - tcy) > 0.0;
                        if inside {
                            crossed = true;
                            true
                        } else {
                            false
                        }
                    }
                }
                _ => false,
            };
            if due && !ent(s, FOE).dead {
                s.push_command(Command::Attack { entity: FOE, target: 0 });
                reissued += 1;
            }
        });
        eprintln!("VILLAGEREDGE {tag} reissues_fired={reissued} crossed_edge={crossed}");
        if mode == "at-edge" && !crossed {
            failures.push(format!("{tag}: villager never crossed the south edge (re-issue never fired)"));
        }
        if reissued == 0 {
            failures.push(format!("{tag}: no re-issue fired"));
        }
        if tr.live_shape() {
            failures.push(format!("{tag}: settled={settled:?} {tr:?}"));
        }
    }
    assert!(failures.is_empty(), "reorder failures:\n{}", failures.join("\n"));
}
