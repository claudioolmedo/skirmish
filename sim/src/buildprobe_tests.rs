//! BUILDPROBE (2026-11-19): diagnosis probe for two live
//! observations from bot-loop run #431 (live client, solo vs
//! AI·Normal):
//!
//! (A) A House was placed and its foundation sat at HP 1/550 —
//! the assigned builder did NOT walk to the site on placement;
//! construction only started after the player right-clicked the
//! foundation with a villager. In the SAME session a Barracks
//! builder DID walk over on his own. Earlier loop history also
//! had a "builder never reaches foundation" sighting.
//!
//! (B) A dragged Palisade wall (4 segments, −12 wood exact)
//! rendered as THREE separate fence runs with grass gaps
//! between them. Other sessions (#426, #430) produced
//! continuous walls.
//!
//! Method (the bellprobe/retprobe discipline): every scene is
//! staged through the REAL `Command` path — `Build`,
//! `BuildLine`, `Move`, `Gather`, `Queue`, `Stop`,
//! `DeleteBuilding` — with the match's own starting villagers
//! (ids 1..3). Nothing is injected: no entity is placed by
//! hand, no stockpile is topped up, no task field is written.
//! Anchor cells and drag endpoints are CHOSEN by reading the
//! map (the suites' ring-scan idiom) and fog is opened by
//! riding the Scout through `Command::Move` (the construction
//! suite's `explore_cells` idiom) — scene selection, not state
//! on the path under test. What is measured is what a player
//! sees: the builder's task + order + position over ticks, the
//! first tick construction progress accrues, the founded wall
//! cells vs the drag's expansion vs the wood actually charged.
//!
//! The DE behaviour probed against: a builder auto-walks to a
//! foundation he was assigned at placement and construction
//! starts on arrival with no further order; a wall drag founds
//! a continuous line along the drag and charges exactly for
//! the segments it founds (a segment the ground refuses — a
//! tree, a node, reserved or unexplored ground — is skipped
//! and never charged: BLD-13/BLD-20's own letter).
//!
//! OUTCOME (2026-11-19): the standard sweep passes in full —
//! assignment, auto-walk and completion hold for every kind,
//! distance and direction probed, busy/idle/queued/on-site/
//! crowded builders included — and every wall-drag accounting
//! case is exact. ONE defect was proven, in the seal scenes:
//! when a builder's stand cell is blocked AFTER his move
//! order was issued (a wall segment starts on it), the order
//! can never complete and the work step's retry/give-up only
//! ran for order-less builders — he froze mid-field forever
//! (measured: 30,000 ticks at 10.60 wu from the site, task
//! ToBuild, progress 0). The minimal fix lives in `work_tick`'s
//! ToBuild arm (`build_order_target_blocked`): a stale order
//! whose target cell is now blocked is dropped into the
//! existing retry/give-up path. The two seal tests are its
//! regression pins; everything else here pins behaviour that
//! was already correct.

use crate::build::BuildingKind;
use crate::entity::{Entity, EntityKind, Task};
use crate::{Command, QueuedOrder, ResourceKind, Sim, OWNER_PLAYER};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn wood(sim: &Sim) -> i32 {
    sim.stockpile_of(OWNER_PLAYER)
        .amount(ResourceKind::Wood)
        .to_num::<i32>()
}

/// A world point inside anchor cell `(cx, cy)` (its center),
/// the way a client reports a placement click.
fn anchor_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn build(sim: &mut Sim, entity: u32, kind: BuildingKind, cx: u32, cy: u32) {
    let (x, y) = anchor_point(sim, cx, cy);
    sim.push_command(Command::Build { entity, kind, x, y });
}

fn build_line(sim: &mut Sim, entity: u32, a: (u32, u32), b: (u32, u32)) {
    let (x0, y0) = anchor_point(sim, a.0, a.1);
    let (x1, y1) = anchor_point(sim, b.0, b.1);
    sim.push_command(Command::BuildLine {
        entity,
        kind: BuildingKind::PalisadeWall,
        x0,
        y0,
        x1,
        y1,
    });
}

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

/// The first anchor cell near a center cell where `kind` may
/// be built by `entity` right now, scanning rings outward
/// (the construction suite's M7 idiom).
fn open_anchor_near_from(
    sim: &Sim,
    entity: u32,
    kind: BuildingKind,
    cx: u32,
    cy: u32,
    r0: i32,
) -> (u32, u32) {
    for r in r0..=16i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (cx as i32 + dx, cy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let (x, y) = anchor_point(sim, ax, ay);
                if sim.can_build(entity, kind, x, y) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no buildable ground for {kind:?} near ({cx}, {cy})");
}

fn open_anchor_near_tc(sim: &Sim, entity: u32, kind: BuildingKind) -> (u32, u32) {
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    open_anchor_near_from(sim, entity, kind, tcx, tcy, 2)
}

fn tc_cell(sim: &Sim) -> (u32, u32) {
    let tc = sim.town_center();
    sim.grid().world_to_cell(tc.pos_x, tc.pos_y)
}

/// Ride the Scout through the given cells (settling the fog
/// at each) and back home — BLD-01 refuses unexplored ground,
/// and staged sites outside the explored disc need the ride
/// first (the construction suite's idiom, verbatim shape).
fn explore_cells(sim: &mut Sim, cells: &[(u32, u32)]) {
    let scout = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::ScoutCavalry)
        .unwrap()
        .id;
    let tc = sim.town_center();
    let home = (tc.pos_x, tc.pos_y);
    let ride = |sim: &mut Sim, x: I16F16, y: I16F16| {
        sim.push_command(Command::Move { entity: scout, x, y });
        let there = run_until(sim, 90_000, |s| {
            let e = s.entities().iter().find(|e| e.id == scout).unwrap();
            (e.pos_x - x).abs() <= I16F16::from_num(8)
                && (e.pos_y - y).abs() <= I16F16::from_num(8)
        });
        assert!(there, "scout never reached a staging cell");
        for _ in 0..30 {
            sim.tick();
        }
    };
    for &(cx, cy) in cells {
        let (x, y) = anchor_point(sim, cx, cy);
        ride(sim, x, y);
    }
    ride(sim, home.0, home.1);
}

// ---------------------------------------------------------------------
// (A) measurement: one builder, one foundation, watched per tick.
// ---------------------------------------------------------------------

#[derive(Default, Debug)]
struct BuildMeasure {
    /// Task was ToBuild on the named building right after the
    /// placement tick.
    assigned: bool,
    /// The placement tick's own work step already flipped the
    /// builder to Building — he was standing inside the build
    /// range when he placed (Barracks/Farm near the TC land
    /// here: their 12.5 range covers the villagers' start).
    /// Assignment took effect exactly the same; only the
    /// travel phase is empty.
    already_building: bool,
    /// A move order was installed on the placement tick.
    order_at_assign: bool,
    first_building_tick: Option<u64>,
    first_progress_tick: Option<u64>,
    completed_tick: Option<u64>,
    /// First tick the builder stands Idle with the building
    /// still incomplete, having once been assigned (give-up).
    gave_up_tick: Option<u64>,
    /// Ticks spent in ToBuild with a live order. During a
    /// normal walk these are ordinary travel ticks; the limbo
    /// signature is this count running to the whole window
    /// with no completion and no give-up (the seal scenes).
    limbo_ticks: u64,
    min_dist: f64,
    end_dist: f64,
    end_task: Option<Task>,
    progress_end: u64,
    complete_end: bool,
    builder_dead: bool,
}

impl BuildMeasure {
    /// The placement's assignment took effect: the builder is
    /// travelling to the site or already building it.
    fn took_effect(&self) -> bool {
        self.assigned || self.already_building
    }
}

fn build_report(tag: &str, kind: BuildingKind, m: &BuildMeasure) -> String {
    format!(
        "{tag} kind={kind:?} assigned={} already_building={} order_at_assign={} first_building={:?} first_progress={:?} completed={:?} gave_up={:?} limbo_ticks={} min_dist={:.2} end_dist={:.2} end_task={:?} progress_end={} complete_end={} dead={}",
        m.assigned,
        m.already_building,
        m.order_at_assign,
        m.first_building_tick,
        m.first_progress_tick,
        m.completed_tick,
        m.gave_up_tick,
        m.limbo_ticks,
        m.min_dist,
        m.end_dist,
        m.end_task,
        m.progress_end,
        m.complete_end,
        m.builder_dead,
    )
}

/// Watch builder `vil` and building `bid` for up to `cap`
/// ticks, sampling after every tick. The placement command
/// must already have been applied (one tick run by the
/// caller), so `assigned`/`order_at_assign` read the state
/// the placement itself produced.
fn measure_build(sim: &mut Sim, vil: u32, bid: u32, cap: u64) -> BuildMeasure {
    let mut m = BuildMeasure {
        min_dist: f64::MAX,
        ..Default::default()
    };
    {
        let e = ent(sim, vil);
        let b = sim.buildings().iter().find(|b| b.id == bid).unwrap();
        m.assigned = e.task == Task::ToBuild && e.task_building == Some(bid);
        m.already_building = e.task == Task::Building && e.task_building == Some(bid);
        m.order_at_assign = e.order.is_some();
        m.min_dist = f(crate::steer::vec_len(e.pos_x - b.pos_x, e.pos_y - b.pos_y));
    }
    let t0 = sim.tick_count();
    for _ in 0..cap {
        sim.tick();
        let t = sim.tick_count() - t0;
        let e = ent(sim, vil);
        let b = sim.buildings().iter().find(|b| b.id == bid).unwrap();
        let d = f(crate::steer::vec_len(e.pos_x - b.pos_x, e.pos_y - b.pos_y));
        if d < m.min_dist {
            m.min_dist = d;
        }
        m.end_dist = d;
        if e.task == Task::Building && m.first_building_tick.is_none() {
            m.first_building_tick = Some(t);
        }
        if b.progress_thirds > 0 && m.first_progress_tick.is_none() {
            m.first_progress_tick = Some(t);
        }
        if b.complete && m.completed_tick.is_none() {
            m.completed_tick = Some(t);
        }
        if m.took_effect()
            && e.task == Task::Idle
            && !b.complete
            && m.gave_up_tick.is_none()
        {
            m.gave_up_tick = Some(t);
        }
        if e.task == Task::ToBuild && e.order.is_some() && !b.complete {
            m.limbo_ticks += 1;
        }
        if b.complete {
            break;
        }
    }
    let e = ent(sim, vil);
    let b = sim.buildings().iter().find(|b| b.id == bid).unwrap();
    m.end_task = Some(e.task);
    m.progress_end = b.progress_thirds;
    m.complete_end = b.complete;
    m.builder_dead = e.dead;
    m
}

/// Place `kind` at `(ax, ay)` with villager `vil` in a fresh
/// sim on `seed`, apply one tick, and measure the build to
/// completion (or the cap). Returns the measure.
fn place_and_measure(
    seed: u64,
    vil: u32,
    kind: BuildingKind,
    anchor: (u32, u32),
    cap: u64,
) -> (Sim, BuildMeasure) {
    let mut sim = Sim::new(seed);
    build(&mut sim, vil, kind, anchor.0, anchor.1);
    sim.tick();
    assert_eq!(
        sim.buildings().len(),
        1,
        "staging broken: the {kind:?} placement at {anchor:?} was refused on seed {seed}"
    );
    let bid = sim.buildings()[0].id;
    let m = measure_build(&mut sim, vil, bid, cap);
    (sim, m)
}

// ---------------------------------------------------------------------
// (A) the standard sweep: kinds × distances, builder idle at the TC.
// ---------------------------------------------------------------------

/// The live shape, part 1: a House near the Town Center. The
/// builder must be assigned WITH a move order on the placement
/// tick, walk, and construction must start on arrival and run
/// to completion with no further order.
#[test]
fn a_house_near_tc_auto_walks_and_completes() {
    let mut sim = Sim::new(42);
    let a = open_anchor_near_tc(&sim, 1, BuildingKind::House);
    build(&mut sim, 1, BuildingKind::House, a.0, a.1);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "house placement refused");
    let bid = sim.buildings()[0].id;
    let m = measure_build(&mut sim, 1, bid, 20_000);
    eprintln!("BUILDPROBE {}", build_report("A-house-near", BuildingKind::House, &m));
    assert!(m.took_effect(), "builder not assigned at placement: {}", build_report("A", BuildingKind::House, &m));
    assert!(m.order_at_assign || m.already_building, "no move order installed at placement: {}", build_report("A", BuildingKind::House, &m));
    assert!(m.first_progress_tick.is_some(), "construction never started: {}", build_report("A", BuildingKind::House, &m));
    assert!(m.complete_end, "house never completed: {}", build_report("A", BuildingKind::House, &m));
    assert!(m.gave_up_tick.is_none(), "builder gave up mid-task: {}", build_report("A", BuildingKind::House, &m));
}

/// The live shape, part 2: the SAME session's Barracks — the
/// kind that worked live. 3×3, build range 12.5.
#[test]
fn a_barracks_near_tc_auto_walks_and_completes() {
    let mut sim = Sim::new(42);
    let a = open_anchor_near_tc(&sim, 1, BuildingKind::Barracks);
    build(&mut sim, 1, BuildingKind::Barracks, a.0, a.1);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "barracks placement refused");
    let bid = sim.buildings()[0].id;
    let m = measure_build(&mut sim, 1, bid, 30_000);
    eprintln!("BUILDPROBE {}", build_report("A-barracks-near", BuildingKind::Barracks, &m));
    assert!(m.took_effect() && (m.order_at_assign || m.already_building), "barracks builder not auto-walking: {}", build_report("A", BuildingKind::Barracks, &m));
    assert!(m.complete_end, "barracks never completed: {}", build_report("A", BuildingKind::Barracks, &m));
    assert!(m.gave_up_tick.is_none(), "barracks builder gave up: {}", build_report("A", BuildingKind::Barracks, &m));
}

/// The kind sweep at near-TC distance: every Dark-Age kind a
/// bot-loop opening places — the three 2×2 camps (build range
/// 10.0, the House's geometry), the 1×1 Outpost (SMALL range
/// 5.8, stand points pulled to 4.6 — the Watch Tower shares
/// this 1×1 geometry but is Feudal-gated in DE, so no Dark-Age
/// scene can stage it), and a single Palisade segment ordered
/// as a plain Build.
#[test]
fn a_kind_sweep_near_tc() {
    let mut failures: Vec<String> = Vec::new();
    for kind in [
        BuildingKind::LumberCamp,
        BuildingKind::Mill,
        BuildingKind::MiningCamp,
        BuildingKind::Outpost,
        BuildingKind::PalisadeWall,
    ] {
        let mut sim = Sim::new(42);
        let a = open_anchor_near_tc(&sim, 1, kind);
        build(&mut sim, 1, kind, a.0, a.1);
        sim.tick();
        assert_eq!(sim.buildings().len(), 1, "{kind:?} placement refused near the TC");
        let bid = sim.buildings()[0].id;
        let m = measure_build(&mut sim, 1, bid, 30_000);
        eprintln!("BUILDPROBE {}", build_report("A-sweep-near", kind, &m));
        if !(m.took_effect() && (m.order_at_assign || m.already_building) && m.complete_end && m.gave_up_tick.is_none()) {
            failures.push(build_report("A-sweep-near", kind, &m));
        }
    }
    assert!(
        failures.is_empty(),
        "kinds whose builder did not auto-walk and complete near the TC:\n{}",
        failures.join("\n")
    );
}

/// Distance sweep: a House placed ~28 cells from the Town
/// Center (fog opened by the Scout's ride first — the real
/// player's path too), on three seeds. The builder's walk is
/// ~110 world units; assignment, arrival and completion are
/// measured exactly as at near range.
#[test]
fn a_house_far_from_tc_auto_walks() {
    let mut failures: Vec<String> = Vec::new();
    for seed in [42u64, 7, 123] {
        let mut sim = Sim::new(seed);
        let (tcx, tcy) = tc_cell(&sim);
        let target = (tcx + 28, tcy);
        explore_cells(&mut sim, &[target, (target.0 + 2, target.1 + 2)]);
        let a = open_anchor_near_from(&sim, 1, BuildingKind::House, target.0, target.1, 0);
        build(&mut sim, 1, BuildingKind::House, a.0, a.1);
        sim.tick();
        assert_eq!(sim.buildings().len(), 1, "far house refused on seed {seed}");
        let bid = sim.buildings()[0].id;
        let m = measure_build(&mut sim, 1, bid, 40_000);
        eprintln!("BUILDPROBE {}", build_report(&format!("A-house-far-seed{seed}"), BuildingKind::House, &m));
        if !(m.assigned && m.order_at_assign && m.complete_end && m.gave_up_tick.is_none() && !m.builder_dead) {
            failures.push(build_report(&format!("A-house-far-seed{seed}"), BuildingKind::House, &m));
        }
    }
    assert!(
        failures.is_empty(),
        "far House placements whose builder did not auto-walk and complete:\n{}",
        failures.join("\n")
    );
}

/// The Barracks at distance (the 3×3 geometry on a long walk),
/// and a House in the opposite compass direction — placement
/// direction varies which ring corner the fixed-order stand
/// search picks, so the sweep covers both.
#[test]
fn a_barracks_far_and_house_west() {
    // Barracks far east.
    let mut sim = Sim::new(42);
    let (tcx, tcy) = tc_cell(&sim);
    let target = (tcx + 26, tcy + 6);
    explore_cells(&mut sim, &[target, (target.0 + 3, target.1 + 3)]);
    let a = open_anchor_near_from(&sim, 1, BuildingKind::Barracks, target.0, target.1, 0);
    build(&mut sim, 1, BuildingKind::Barracks, a.0, a.1);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "far barracks refused");
    let bid = sim.buildings()[0].id;
    let m = measure_build(&mut sim, 1, bid, 40_000);
    eprintln!("BUILDPROBE {}", build_report("A-barracks-far", BuildingKind::Barracks, &m));
    assert!(m.assigned && m.order_at_assign && m.complete_end && m.gave_up_tick.is_none(),
        "far barracks builder did not auto-walk and complete: {}", build_report("A", BuildingKind::Barracks, &m));

    // House far west (the stand search's first ring cell is the
    // footprint's north-west corner — from a western approach
    // the geometry differs from the eastern scenes above).
    // Seed 42's TC sits at (25, 81): ~20 cells of room west.
    let mut sim = Sim::new(42);
    let (tcx, tcy) = tc_cell(&sim);
    assert!(tcx >= 21 && tcy >= 13, "staging: seed 42's TC moved");
    let target = (tcx - 20, tcy - 12);
    explore_cells(&mut sim, &[target, (target.0 + 2, target.1 + 2)]);
    let a = open_anchor_near_from(&sim, 1, BuildingKind::House, target.0, target.1, 0);
    build(&mut sim, 1, BuildingKind::House, a.0, a.1);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "west house refused");
    let bid = sim.buildings()[0].id;
    let m = measure_build(&mut sim, 1, bid, 40_000);
    eprintln!("BUILDPROBE {}", build_report("A-house-west", BuildingKind::House, &m));
    assert!(m.assigned && m.order_at_assign && m.complete_end && m.gave_up_tick.is_none(),
        "west house builder did not auto-walk and complete: {}", build_report("A", BuildingKind::House, &m));
}

/// Two builders, one House: the second villager's order on the
/// standing foundation is a Resume (no second charge) and both
/// must walk and build.
#[test]
fn a_two_builders_one_house_both_walk() {
    let mut sim = Sim::new(42);
    let a = open_anchor_near_tc(&sim, 1, BuildingKind::House);
    let w0 = wood(&sim);
    build(&mut sim, 1, BuildingKind::House, a.0, a.1);
    sim.tick();
    build(&mut sim, 2, BuildingKind::House, a.0, a.1);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "resume placed a duplicate");
    assert_eq!(wood(&sim), w0 - 25, "resume charged twice");
    let bid = sim.buildings()[0].id;
    let both_building = run_until(&mut sim, 20_000, |s| {
        ent(s, 1).task == Task::Building && ent(s, 2).task == Task::Building
    });
    eprintln!(
        "BUILDPROBE A-two-builders both_building={} vil1_task={:?} vil2_task={:?}",
        both_building,
        ent(&sim, 1).task,
        ent(&sim, 2).task
    );
    assert!(both_building, "the two builders never worked side by side");
    let done = run_until(&mut sim, 20_000, |s| s.buildings()[0].complete);
    assert!(done, "two-builder house never completed");
    assert_eq!(bid, sim.buildings()[0].id);
}

/// The builder is BUSY when the placement lands: villager 1 is
/// out gathering wood; the Build order re-tasks him — he must
/// leave the tree, walk to the site and build (the bot-loop's
/// usual mid-opening shape).
#[test]
fn a_builder_retasked_from_gathering_walks() {
    let mut sim = Sim::new(42);
    let tree = sim
        .nodes()
        .iter()
        .find(|n| n.kind == crate::NodeKind::Tree)
        .unwrap()
        .id;
    sim.push_command(Command::Gather { entity: 1, node: tree });
    let working = run_until(&mut sim, 20_000, |s| {
        matches!(ent(s, 1).task, Task::Gathering | Task::Returning)
    });
    assert!(working, "staging broken: villager never started gathering");
    let a = open_anchor_near_tc(&sim, 1, BuildingKind::House);
    build(&mut sim, 1, BuildingKind::House, a.0, a.1);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "house placement refused");
    let bid = sim.buildings()[0].id;
    let m = measure_build(&mut sim, 1, bid, 30_000);
    eprintln!("BUILDPROBE {}", build_report("A-retask-from-gather", BuildingKind::House, &m));
    assert!(m.took_effect() && (m.order_at_assign || m.already_building), "gathering builder not re-tasked at placement: {}", build_report("A", BuildingKind::House, &m));
    assert!(m.complete_end, "re-tasked builder never completed the house: {}", build_report("A", BuildingKind::House, &m));
}

/// The Farm chain: a Mill raised by villager 2, then a Farm
/// placed by villager 1 — the farmer must walk, raise the
/// field, and start farming it on completion (the finished-
/// site path), with no further order.
#[test]
fn a_farm_after_mill_builder_walks_and_farms() {
    let mut sim = Sim::new(42);
    let ma = open_anchor_near_tc(&sim, 2, BuildingKind::Mill);
    build(&mut sim, 2, BuildingKind::Mill, ma.0, ma.1);
    let mill_up = run_until(&mut sim, 30_000, |s| {
        s.buildings().iter().any(|b| b.kind == BuildingKind::Mill && b.complete)
    });
    assert!(mill_up, "staging broken: mill never completed");
    let fa = open_anchor_near_from(&sim, 1, BuildingKind::Farm, ma.0 + 4, ma.1, 0);
    build(&mut sim, 1, BuildingKind::Farm, fa.0, fa.1);
    sim.tick();
    let fb = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Farm)
        .unwrap()
        .id;
    let m = measure_build(&mut sim, 1, fb, 30_000);
    eprintln!("BUILDPROBE {}", build_report("A-farm", BuildingKind::Farm, &m));
    assert!(m.took_effect() && (m.order_at_assign || m.already_building) && m.complete_end,
        "farm builder did not auto-walk and complete: {}", build_report("A", BuildingKind::Farm, &m));
    // On completion the builder starts farming its own field.
    let v = ent(&sim, 1);
    eprintln!(
        "BUILDPROBE A-farm end_task={:?} task_node={:?}",
        v.task, v.task_node
    );
    assert!(
        v.task_node.is_some(),
        "the farm's builder did not start farming on completion"
    );
}

/// The ordering builder stands ON the footprint when he places
/// (the placement validation exempts him): he must still end
/// up building — displaced off the footprint by the grid-
/// integrity pass, re-approached, and put to work.
#[test]
fn a_builder_standing_on_his_own_site() {
    let mut sim = Sim::new(42);
    let a = open_anchor_near_tc(&sim, 1, BuildingKind::House);
    // Walk villager 1 onto the anchor cell first (a plain Move).
    let (px, py) = anchor_point(&sim, a.0, a.1);
    sim.push_command(Command::Move { entity: 1, x: px, y: py });
    let there = run_until(&mut sim, 20_000, |s| {
        let e = ent(s, 1);
        e.order.is_none()
            && s.grid().world_to_cell(e.pos_x, e.pos_y) == (a.0, a.1)
    });
    assert!(there, "staging broken: villager never stood on the site");
    assert!(
        sim.can_build(1, BuildingKind::House, px, py),
        "staging broken: the ordering builder cannot place on his own cell"
    );
    build(&mut sim, 1, BuildingKind::House, a.0, a.1);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "placement from on-site refused");
    let bid = sim.buildings()[0].id;
    let m = measure_build(&mut sim, 1, bid, 30_000);
    eprintln!("BUILDPROBE {}", build_report("A-on-site", BuildingKind::House, &m));
    assert!(m.complete_end, "on-site builder never completed his house: {}", build_report("A", BuildingKind::House, &m));
}

/// A Shift-queued Build (SEL-12): the build order waits behind
/// a Move and executes when the villager goes idle — the
/// foundation appears only then, and the builder walks from
/// the move's destination to the site and builds.
#[test]
fn a_queued_build_executes_and_walks() {
    let mut sim = Sim::new(42);
    let a = open_anchor_near_tc(&sim, 1, BuildingKind::House);
    let (tcx, tcy) = tc_cell(&sim);
    let (mx, my) = anchor_point(&sim, tcx + 6, tcy + 6);
    sim.push_command(Command::Move { entity: 1, x: mx, y: my });
    let (bx, by) = anchor_point(&sim, a.0, a.1);
    sim.push_command(Command::Queue {
        entity: 1,
        order: QueuedOrder::Build { kind: BuildingKind::House, x: bx, y: by },
    });
    // While the move runs, no foundation exists.
    for _ in 0..30 {
        sim.tick();
        assert!(sim.buildings().is_empty(), "queued build placed early");
    }
    let placed = run_until(&mut sim, 30_000, |s| !s.buildings().is_empty());
    assert!(placed, "queued build never executed");
    let bid = sim.buildings()[0].id;
    let m = measure_build(&mut sim, 1, bid, 30_000);
    eprintln!("BUILDPROBE {}", build_report("A-queued", BuildingKind::House, &m));
    assert!(m.complete_end, "queued build never completed: {}", build_report("A", BuildingKind::House, &m));
}

/// The crowd scene: the other villagers and the Scout are
/// ordered to stand on and around the House's stand corner
/// before the builder arrives. DE builders push in and build
/// from contact; the sim's bump/arrival machinery must land
/// him in build range — he may not freeze outside it.
#[test]
fn a_crowd_at_the_stand_point_does_not_strand_the_builder() {
    let mut sim = Sim::new(42);
    let a = open_anchor_near_tc(&sim, 1, BuildingKind::House);
    // The stand cell the fixed-order search picks first for a
    // builder approaching from the TC side: read it off the
    // sim's own approach search (measurement, not staging —
    // the crowd is then ordered there by plain Move commands).
    let idx = sim.entities().iter().position(|e| e.id == 1).unwrap();
    let stand = sim
        .approach_point(idx, a.0, a.1, 2, 2)
        .expect("a stand point must exist for a valid anchor");
    let stand_cell = sim.grid().world_to_cell(stand.0, stand.1);
    // Crowd: villager 2 onto the stand cell, villager 3 and the
    // Scout onto its outer neighbours.
    let (sx, sy) = anchor_point(&sim, stand_cell.0, stand_cell.1);
    sim.push_command(Command::Move { entity: 2, x: sx, y: sy });
    let out = (
        stand_cell.0 as i32 + (stand_cell.0 as i32 - a.0 as i32 - 1).signum(),
        stand_cell.1 as i32 + (stand_cell.1 as i32 - a.1 as i32 - 1).signum(),
    );
    let (ox, oy) = anchor_point(&sim, out.0 as u32, out.1 as u32);
    sim.push_command(Command::Move { entity: 3, x: ox, y: oy });
    let scout = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::ScoutCavalry)
        .unwrap()
        .id;
    sim.push_command(Command::Move { entity: scout, x: sx, y: sy });
    let settled = run_until(&mut sim, 30_000, |s| {
        ent(s, 2).order.is_none() && ent(s, 3).order.is_none() && ent(s, scout).order.is_none()
    });
    assert!(settled, "staging broken: the crowd never settled");
    build(&mut sim, 1, BuildingKind::House, a.0, a.1);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "house placement refused");
    let bid = sim.buildings()[0].id;
    let m = measure_build(&mut sim, 1, bid, 30_000);
    eprintln!(
        "BUILDPROBE {} stand_cell={stand_cell:?}",
        build_report("A-crowd", BuildingKind::House, &m)
    );
    assert!(
        m.complete_end,
        "crowded stand point stranded the builder: {}",
        build_report("A-crowd", BuildingKind::House, &m)
    );
}

// ---------------------------------------------------------------------
// (A) the seal scenes: the stand ring closes AFTER assignment.
// ---------------------------------------------------------------------

/// Two distinct open cells near the Town Center, outside the
/// House anchor's footprint AND its whole ring, for parking
/// the staging villagers. (Parking ON the TC's own cell does
/// not work: a Move order to the Town Center entity's center
/// never certifies arrival — contact with the TC body holds
/// the villager outside the arrival radius — so the first
/// staging draft's clear step never completed. Open ground
/// beside the TC completes normally.)
fn park_cells(sim: &Sim, a: (u32, u32)) -> [(u32, u32); 2] {
    let (tcx, tcy) = tc_cell(sim);
    let ring = crate::build::rect_ring_cells(a.0, a.1, 2, 2);
    let mut out: Vec<(u32, u32)> = Vec::new();
    for r in 3..=7i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (tcx as i32 + dx, tcy as i32 + dy);
                if x < 1 || y < 1 {
                    continue;
                }
                let (x, y) = (x as u32, y as u32);
                let on_footprint =
                    (a.0..a.0 + 2).contains(&x) && (a.1..a.1 + 2).contains(&y);
                if on_footprint || ring.contains(&(x, y)) || sim.grid().is_blocked(x, y) {
                    continue;
                }
                if !out.contains(&(x, y)) {
                    out.push((x, y));
                }
                if out.len() == 2 {
                    return [out[0], out[1]];
                }
            }
        }
    }
    panic!("no park cells near the TC");
}

/// Build completed Palisade segments on every ring cell of
/// the House anchor `a` except `keep`, using villager 2 alone
/// through the real Build path, one segment at a time (a
/// completed segment blocks its cell). Returns when the seal
/// stands and both staging villagers are parked clear of the
/// footprint and the kept cell. Scene staging only — the path
/// under test (villager 1's assignment) has not been touched
/// yet when this returns.
///
/// The seal order matters (learned the hard way): a diagonal
/// step is barred when a building stands on EITHER orthogonal
/// neighbour (the grid's hard-corner rule), so once the ring
/// closes, a sealer standing on the footprint can be trapped
/// inside — his only exit would be the diagonal into `keep`,
/// barred by the completed segments flanking it. The segments
/// therefore go up in ROTATIONAL order around the ring,
/// starting beside `keep`, and the sealer parks outside after
/// each one; the last segment (the "door", `keep`'s other
/// neighbour) is approached from its own outside ring cell —
/// the approach search's first candidate — so he is never
/// inside when the ring closes.
fn seal_ring_except(sim: &mut Sim, a: (u32, u32), keep: (u32, u32)) {
    // The ring in clockwise order from its north-west cell.
    let cw: Vec<(u32, u32)> = vec![
        (a.0 - 1, a.1 - 1),
        (a.0, a.1 - 1),
        (a.0 + 1, a.1 - 1),
        (a.0 + 2, a.1 - 1),
        (a.0 + 2, a.1),
        (a.0 + 2, a.1 + 1),
        (a.0 + 2, a.1 + 2),
        (a.0 + 1, a.1 + 2),
        (a.0, a.1 + 2),
        (a.0 - 1, a.1 + 2),
        (a.0 - 1, a.1 + 1),
        (a.0 - 1, a.1),
    ];
    let ring = crate::build::rect_ring_cells(a.0, a.1, 2, 2);
    assert_eq!(ring.len(), 12, "a 2x2 ring has 12 cells");
    for c in &cw {
        assert!(ring.contains(c), "staging broken: {c:?} is not a ring cell");
    }
    let k = cw.iter().position(|&c| c == keep).expect("keep must be a ring cell");
    // Rotational order from just after `keep`; the door (last)
    // is `keep`'s counterclockwise neighbour. Its approach
    // search tries its own north-west ring cell first — that
    // cell must be open ground, else run the ring the other
    // way (door = the clockwise neighbour, same property).
    let door_nw = |door: (u32, u32)| (door.0 - 1, door.1 - 1);
    let mut order: Vec<(u32, u32)> = (1..12).map(|i| cw[(k + i) % 12]).collect();
    if sim.grid().is_blocked(door_nw(*order.last().unwrap()).0, door_nw(*order.last().unwrap()).1) {
        order = (1..12).map(|i| cw[(k + 12 - i) % 12]).collect();
        assert!(
            !sim.grid().is_blocked(door_nw(*order.last().unwrap()).0, door_nw(*order.last().unwrap()).1),
            "staging broken: both door approaches are blocked"
        );
    }
    assert_eq!(order.len(), 11, "sealing 11 of the 12 ring cells");
    let parks = park_cells(sim, a);
    let on_footprint = |s: &Sim, v: u32| {
        let e = ent(s, v);
        let c = s.grid().world_to_cell(e.pos_x, e.pos_y);
        (a.0..a.0 + 2).contains(&c.0) && (a.1..a.1 + 2).contains(&c.1)
    };
    let last = order.len() - 1;
    for (i, cell) in order.into_iter().enumerate() {
        assert_eq!(ent(sim, 2).task, Task::Idle, "staging broken: sealer busy");
        build(sim, 2, BuildingKind::PalisadeWall, cell.0, cell.1);
        let done = run_until(sim, 30_000, |s| {
            s.buildings().iter().any(|b| {
                b.kind == BuildingKind::PalisadeWall
                    && (b.cell_x, b.cell_y) == cell
                    && b.complete
            })
        });
        assert!(done, "staging broken: seal segment at {cell:?} never completed");
        let idle = run_until(sim, 30_000, |s| ent(s, 2).task == Task::Idle);
        assert!(idle, "staging broken: sealer never stood down");
        if i != last {
            // Park the sealer outside before the next segment:
            // while the ring is still open he can always step
            // out through an unbuilt cell; after the door he
            // never needs to.
            let (hx, hy) = anchor_point(sim, parks[0].0, parks[0].1);
            sim.push_command(Command::Move { entity: 2, x: hx, y: hy });
            let out = run_until(sim, 30_000, |s| {
                ent(s, 2).order.is_none() && !on_footprint(s, 2)
            });
            assert!(out, "staging broken: sealer trapped inside the open ring");
        }
    }
    // Park both staging villagers clear of the footprint and
    // the kept cell, so the placement validation sees no unit
    // on the footprint and the kept cell stays the only stand
    // point. (`parks` was computed before the seal loop.)
    for (i, v) in [2u32, 3].iter().enumerate() {
        let (hx, hy) = anchor_point(sim, parks[i].0, parks[i].1);
        sim.push_command(Command::Move { entity: *v, x: hx, y: hy });
    }
    let clear = run_until(sim, 30_000, |s| {
        [2u32, 3].iter().all(|&v| {
            let e = ent(s, v);
            let c = s.grid().world_to_cell(e.pos_x, e.pos_y);
            let on_footprint =
                (a.0..a.0 + 2).contains(&c.0) && (a.1..a.1 + 2).contains(&c.1);
            e.order.is_none() && c != keep && !on_footprint
        })
    });
    assert!(clear, "staging broken: sealers never cleared the site");
}

/// A House anchor near the TC whose ENTIRE ring can be
/// sealed: the House validates, and every one of its 12 ring
/// cells individually validates as a Palisade segment (clean,
/// explored, node-free, unreserved ground).
fn sealable_anchor(sim: &Sim) -> (u32, u32) {
    let (tcx, tcy) = tc_cell(sim);
    for r in 2..=12i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 2 || ay < 2 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let (hx, hy) = anchor_point(sim, ax, ay);
                if !sim.can_build(1, BuildingKind::House, hx, hy) {
                    continue;
                }
                let ring = crate::build::rect_ring_cells(ax, ay, 2, 2);
                let ok = ring.iter().all(|&(cx, cy)| {
                    let (x, y) = anchor_point(sim, cx, cy);
                    sim.can_build(2, BuildingKind::PalisadeWall, x, y)
                });
                if ok {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no sealable house anchor near the TC on this seed");
}

/// Stage villager 2 beside the kept ring cell `keep`, ready
/// to wall it shut the moment villager 1 is assigned: he
/// stands on the open cell diagonally outside the corner.
fn stage_closer(sim: &mut Sim, a: (u32, u32), keep: (u32, u32)) {
    // The cell outside `keep`, away from the footprint: one
    // step further along the diagonal/edge from the anchor.
    let dx = (keep.0 as i32 - (a.0 as i32 + 1)).signum();
    let dy = (keep.1 as i32 - (a.1 as i32 + 1)).signum();
    let out = ((keep.0 as i32 + dx) as u32, (keep.1 as i32 + dy) as u32);
    let (ox, oy) = anchor_point(sim, out.0, out.1);
    sim.push_command(Command::Move { entity: 2, x: ox, y: oy });
    let there = run_until(sim, 30_000, |s| ent(s, 2).order.is_none());
    assert!(there, "staging broken: the closer never took position");
}

/// The corner seal: the House's only open stand cell is its
/// north-west ring CORNER (the fixed-order search's first
/// cell). Villager 1 is assigned and walks; villager 2 then
/// walls the corner shut behind him. A corner stand point is
/// 8.49 wu from the House's center — outside it, sealed, the
/// builder stands ~10.5+ wu away, beyond the 10.0 build range:
/// he can neither build from outside nor stand on the cell.
/// What he may NOT do is freeze mid-field forever with a live
/// stale order (invisible to the idle-villager query, never
/// re-approached, never released): he must build, or be
/// released to Idle where the player can see and re-task him.
#[test]
fn a_corner_stand_cell_sealed_after_assignment() {
    let mut sim = Sim::new(42);
    let a = sealable_anchor(&sim);
    let keep = (a.0 - 1, a.1 - 1); // the ring's north-west corner
    seal_ring_except(&mut sim, a, keep);
    stage_closer(&mut sim, a, keep);
    // The placement still validates: the kept corner is a
    // stand point with a path.
    let (hx, hy) = anchor_point(&sim, a.0, a.1);
    assert!(sim.can_build(1, BuildingKind::House, hx, hy), "staging broken: sealed-but-one house does not validate");
    build(&mut sim, 1, BuildingKind::House, a.0, a.1);
    sim.tick();
    assert_eq!(
        sim.buildings().iter().filter(|b| b.kind == BuildingKind::House).count(),
        1,
        "house placement refused"
    );
    let bid = sim.buildings().iter().find(|b| b.kind == BuildingKind::House).unwrap().id;
    {
        let e = ent(&sim, 1);
        assert_eq!(e.task, Task::ToBuild, "builder not assigned at placement");
        assert!(e.order.is_some(), "builder has no order at placement");
    }
    // The closer walls the corner shut.
    build(&mut sim, 2, BuildingKind::PalisadeWall, keep.0, keep.1);
    let sealed = run_until(&mut sim, 30_000, |s| s.grid().is_dynamic(keep.0, keep.1));
    assert!(sealed, "staging broken: the corner segment never started (cell never blocked)");
    // Watch the builder for a long window. (The release to
    // Idle may land during the seal wait above, before this
    // window opens — so release is read from the end state:
    // Idle with the house unfinished, having verifiably been
    // assigned and ordered above.)
    let m = measure_build(&mut sim, 1, bid, 12_000);
    eprintln!("BUILDPROBE {}", build_report("A-seal-corner", BuildingKind::House, &m));
    let released = m.gave_up_tick.is_some() || m.end_task == Some(Task::Idle);
    assert!(
        m.complete_end || released,
        "builder froze with a stale order outside a sealed site (limbo): {}",
        build_report("A-seal-corner", BuildingKind::House, &m)
    );
    // The player's recovery, exactly the live one: right-click
    // the foundation with a fresh villager. With the corner
    // deleted first (the seal segment removed), the resumer
    // must walk and finish the house.
    let seg = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::PalisadeWall && (b.cell_x, b.cell_y) == keep)
        .unwrap()
        .id;
    sim.push_command(Command::DeleteBuilding { building: seg });
    sim.tick();
    build(&mut sim, 3, BuildingKind::House, a.0, a.1);
    sim.tick();
    let m3 = measure_build(&mut sim, 3, bid, 30_000);
    eprintln!("BUILDPROBE {}", build_report("A-seal-corner-resume", BuildingKind::House, &m3));
    assert!(m3.complete_end, "right-click resume after unsealing never completed: {}", build_report("A-seal-corner-resume", BuildingKind::House, &m3));
}

/// The ortho seal: the only open stand cell is the footprint's
/// north EDGE cell (6.0 wu from the center). Sealed after
/// assignment the same way, the builder's order into the cell
/// can never complete. MEASURED on the pristine base
/// (2026-11-19): he froze at 10.60 wu from the center — 0.6 wu
/// OUTSIDE the 10.0 build range, held off by the started
/// segment and the closer's body — for the full 30,000-tick
/// window: task ToBuild, live order, progress 0, never
/// released. That freeze is the live run #431 sighting's
/// mechanism (foundation at 1 HP, builder never builds,
/// right-click resume revives him because a fresh order
/// re-runs the approach). With the stale-order fix he must
/// build the house or be released to Idle — never freeze.
#[test]
fn a_ortho_stand_cell_sealed_after_assignment() {
    let mut sim = Sim::new(42);
    let a = sealable_anchor(&sim);
    let keep = (a.0, a.1 - 1); // the ring's north edge cell
    seal_ring_except(&mut sim, a, keep);
    stage_closer(&mut sim, a, keep);
    let (hx, hy) = anchor_point(&sim, a.0, a.1);
    assert!(sim.can_build(1, BuildingKind::House, hx, hy), "staging broken: sealed-but-one house does not validate");
    build(&mut sim, 1, BuildingKind::House, a.0, a.1);
    sim.tick();
    let bid = sim.buildings().iter().find(|b| b.kind == BuildingKind::House).unwrap().id;
    {
        let e = ent(&sim, 1);
        assert_eq!(e.task, Task::ToBuild, "builder not assigned at placement");
        assert!(e.order.is_some(), "builder has no order at placement");
    }
    build(&mut sim, 2, BuildingKind::PalisadeWall, keep.0, keep.1);
    let sealed = run_until(&mut sim, 30_000, |s| s.grid().is_dynamic(keep.0, keep.1));
    assert!(sealed, "staging broken: the edge segment never started");
    let m = measure_build(&mut sim, 1, bid, 30_000);
    eprintln!("BUILDPROBE {}", build_report("A-seal-ortho", BuildingKind::House, &m));
    let released = m.gave_up_tick.is_some() || m.end_task == Some(Task::Idle);
    assert!(
        m.complete_end || released,
        "builder froze with a stale order outside a sealed site (limbo): {}",
        build_report("A-seal-ortho", BuildingKind::House, &m)
    );
}

// ---------------------------------------------------------------------
// (B) wall drags: expansion vs founded vs charged.
// ---------------------------------------------------------------------

#[derive(Default, Debug)]
struct DragMeasure {
    expansion: Vec<(u32, u32)>,
    founded: Vec<(u32, u32)>,
    wood_spent: i32,
    primary: Option<u32>,
    queue_len: usize,
}

fn drag_report(tag: &str, m: &DragMeasure) -> String {
    format!(
        "{tag} expansion={} founded={} wood_spent={} primary={:?} queue_len={} founded_cells={:?}",
        m.expansion.len(),
        m.founded.len(),
        m.wood_spent,
        m.primary,
        m.queue_len,
        m.founded,
    )
}

/// Issue one BuildLine and measure the same tick: the drag's
/// expansion (the sim's own `wall_line_cells`), the segments
/// actually founded, and the exact wood charged.
fn measure_drag(sim: &mut Sim, vil: u32, a: (u32, u32), b: (u32, u32)) -> DragMeasure {
    let expansion = crate::wall_line_cells(a.0, a.1, b.0, b.1);
    let w0 = wood(sim);
    let before: Vec<u32> = sim.buildings().iter().map(|x| x.id).collect();
    build_line(sim, vil, a, b);
    sim.tick();
    let founded: Vec<(u32, u32)> = sim
        .buildings()
        .iter()
        .filter(|x| x.kind == BuildingKind::PalisadeWall && !before.contains(&x.id))
        .map(|x| (x.cell_x, x.cell_y))
        .collect();
    let e = ent(sim, vil);
    DragMeasure {
        expansion,
        founded,
        wood_spent: w0 - wood(sim),
        primary: e.task_building,
        queue_len: e.order_queue.len(),
    }
}

/// A clean straight drag's ground: a row of `len` cells at
/// Chebyshev distance >= 3 from the TC, every cell explored,
/// unblocked, node-free and outside the TC reserve. Returns
/// the row's first cell.
fn clean_row(sim: &Sim, len: u32) -> (u32, u32) {
    let (tcx, tcy) = tc_cell(sim);
    let reserved = |x: u32, y: u32| {
        (tcx as i32 - 1..tcx as i32 + 3).contains(&(x as i32))
            && (tcy as i32 - 1..tcy as i32 + 3).contains(&(y as i32))
    };
    let node_cells: Vec<(u32, u32)> = sim
        .nodes()
        .iter()
        .filter(|n| !n.is_depleted())
        .map(|n| sim.grid().world_to_cell(n.pos_x, n.pos_y))
        .collect();
    let building_cells: Vec<(u32, u32)> = sim
        .buildings()
        .iter()
        .flat_map(|b| b.footprint_cells())
        .collect();
    for cy in (tcy + 4)..(tcy + 30) {
        for cx in 4..100u32 {
            let ok = (0..len).all(|dx| {
                let (x, y) = (cx + dx, cy);
                !sim.grid().is_blocked(x, y)
                    && !reserved(x, y)
                    && !node_cells.contains(&(x, y))
                    && !building_cells.contains(&(x, y))
                    && sim.cell_explored_by(OWNER_PLAYER, x, y)
            });
            if ok {
                return (cx, cy);
            }
        }
    }
    panic!("no clean row of {len} cells on this seed");
}

/// The live shape, exactly: a 4-segment horizontal drag on
/// clean ground. Founded must equal the expansion, the charge
/// exactly 12, and the line must be one continuous run.
#[test]
fn b_straight_drag_four_segments_exact() {
    let mut sim = Sim::new(42);
    let a = clean_row(&sim, 4);
    let m = measure_drag(&mut sim, 1, a, (a.0 + 3, a.1));
    eprintln!("BUILDPROBE {}", drag_report("B-straight4", &m));
    assert_eq!(m.founded.len(), 4, "clean 4-drag founded {} segments: {}", m.founded.len(), drag_report("B", &m));
    assert_eq!(m.founded, m.expansion, "founded cells differ from the drag expansion: {}", drag_report("B", &m));
    assert_eq!(m.wood_spent, 12, "4 segments must charge exactly 12 wood: {}", drag_report("B", &m));
    // Continuity: consecutive founded cells are king-adjacent.
    for w in m.founded.windows(2) {
        let d = (w[0].0 as i32 - w[1].0 as i32).abs().max((w[0].1 as i32 - w[1].1 as i32).abs());
        assert_eq!(d, 1, "gap in a clean drag between {:?} and {:?}", w[0], w[1]);
    }
    assert!(m.primary.is_some(), "the drag assigned no primary segment");
    assert_eq!(m.queue_len, 3, "the remaining segments must queue behind the primary");
}

/// Diagonal and sloped drags on clean ground: the expansion
/// is a king-adjacent staircase with no gaps BY CONSTRUCTION
/// (`wall_line_cells`), and the founded set must equal it
/// cell-for-cell, charged 3 per segment.
#[test]
fn b_diagonal_and_sloped_drags_continuous() {
    let mut failures: Vec<String> = Vec::new();
    for (tag, dx, dy) in [
        ("diag44", 4i32, 4i32),
        ("slope72", 7, 2),
        ("slope27", 2, 7),
        ("slope83", 8, 3),
        ("diag-neg", -5, -5),
    ] {
        let mut sim = Sim::new(42);
        let (tcx, tcy) = tc_cell(&sim);
        let node_cells: Vec<(u32, u32)> = sim
            .nodes()
            .iter()
            .filter(|nd| !nd.is_depleted())
            .map(|nd| sim.grid().world_to_cell(nd.pos_x, nd.pos_y))
            .collect();
        let cell_clean = |sim: &Sim, x: u32, y: u32| {
            !sim.grid().is_blocked(x, y)
                && !node_cells.contains(&(x, y))
                && sim.cell_explored_by(OWNER_PLAYER, x, y)
                && !((tcx as i32 - 1..tcx as i32 + 3).contains(&(x as i32))
                    && (tcy as i32 - 1..tcy as i32 + 3).contains(&(y as i32)))
        };
        // Scan for a start cell whose whole STAIRCASE is clean
        // ground (the first draft anchored on a clean ROW and
        // then found most staircases crossing trees).
        let mut start: Option<(u32, u32)> = None;
        'scan: for cy in (tcy as i32 - 14)..(tcy as i32 + 20) {
            for cx in 4..110i32 {
                if cx < 1 || cy < 1 {
                    continue;
                }
                let (ax, ay) = (cx as u32, cy as u32);
                let (bx, by) = (cx + dx, cy + dy);
                if bx < 1 || by < 1 {
                    continue;
                }
                let cells = crate::wall_line_cells(ax, ay, bx as u32, by as u32);
                if cells.iter().all(|&(x, y)| cell_clean(&sim, x, y)) {
                    start = Some((ax, ay));
                    break 'scan;
                }
            }
        }
        let Some(a) = start else {
            eprintln!("BUILDPROBE B-{tag} skipped: no clean staircase on seed 42");
            continue;
        };
        let b = ((a.0 as i32 + dx) as u32, (a.1 as i32 + dy) as u32);
        let m = measure_drag(&mut sim, 1, a, b);
        eprintln!("BUILDPROBE {}", drag_report(&format!("B-{tag}"), &m));
        if !(m.founded == m.expansion && m.wood_spent == 3 * m.founded.len() as i32) {
            failures.push(drag_report(&format!("B-{tag}"), &m));
        }
    }
    assert!(
        failures.is_empty(),
        "clean diagonal/sloped drags that dropped or overcharged segments:\n{}",
        failures.join("\n")
    );
}

/// Stage a horizontal 7-cell row whose middle cell holds a
/// live node of `kind`, the other six cells clean. Node lines
/// sit at the edge of the explored disc, so the row is first
/// opened by the Scout's ride (the real player's path too);
/// candidates are tried nearest the TC first. Returns the
/// drag endpoints and the node cell.
fn row_crossing_node(
    sim: &mut Sim,
    kind: crate::NodeKind,
) -> ((u32, u32), (u32, u32), (u32, u32)) {
    let (tcx, tcy) = tc_cell(sim);
    let node_at = |sim: &Sim, x: u32, y: u32| {
        sim.nodes().iter().find(|n| {
            !n.is_depleted() && sim.grid().world_to_cell(n.pos_x, n.pos_y) == (x, y)
        }).map(|n| n.kind)
    };
    let row_ok = |sim: &Sim, cx: u32, cy: u32| {
        (0..7u32).all(|dx| {
            let (x, y) = (cx + dx, cy);
            if dx == 3 {
                return true; // the node cell itself
            }
            !sim.grid().is_blocked(x, y)
                && node_at(sim, x, y).is_none()
                && sim.cell_explored_by(OWNER_PLAYER, x, y)
                && !((tcx as i32 - 1..tcx as i32 + 3).contains(&(x as i32))
                    && (tcy as i32 - 1..tcy as i32 + 3).contains(&(y as i32)))
        })
    };
    // Candidate node cells of the wanted kind, nearest first.
    let mut candidates: Vec<(u32, u32)> = sim
        .nodes()
        .iter()
        .filter(|n| !n.is_depleted() && n.kind == kind)
        .map(|n| sim.grid().world_to_cell(n.pos_x, n.pos_y))
        .collect();
    candidates.sort_by_key(|&(x, y)| {
        (x as i32 - tcx as i32).abs() + (y as i32 - tcy as i32).abs()
    });
    candidates.dedup();
    for mid in candidates.into_iter().take(16) {
        if mid.0 < 4 || mid.1 < 1 {
            continue;
        }
        let (cx, cy) = (mid.0 - 3, mid.1);
        if row_ok(sim, cx, cy) {
            return ((cx, cy), (cx + 6, cy), mid);
        }
        // Open the row's fog with the Scout, then re-check.
        explore_cells(sim, &[(cx, cy), mid, (cx + 6, cy)]);
        if node_at(sim, mid.0, mid.1) == Some(kind) && row_ok(sim, cx, cy) {
            return ((cx, cy), (cx + 6, cy), mid);
        }
    }
    panic!("no {kind:?}-crossing row on seed 42");
}

/// A drag whose line crosses a standing tree: that segment
/// is REFUSED (a footprint may not swallow a live node) — it
/// leaves a gap and is never charged. The charge is exactly
/// 3 × founded. (This is the sim's documented skip: a gap in
/// the founded line, never a partial charge — and it is the
/// shape a live drag takes when its path crosses a woodline.)
#[test]
fn b_drag_crossing_tree_skips_without_charge() {
    let mut sim = Sim::new(42);
    let (a, b, tree_cell) = row_crossing_node(&mut sim, crate::NodeKind::Tree);
    let m = measure_drag(&mut sim, 1, a, b);
    eprintln!("BUILDPROBE {} tree_cell={tree_cell:?}", drag_report("B-tree", &m));
    assert_eq!(m.expansion.len(), 7);
    assert!(!m.founded.contains(&tree_cell), "a segment was founded on the tree cell");
    assert_eq!(m.founded.len(), 6, "exactly the tree segment must be skipped: {}", drag_report("B-tree", &m));
    assert_eq!(m.wood_spent, 18, "charge must be 3 x founded (18), never the full line: {}", drag_report("B-tree", &m));
}

/// The same refusal at the berry patch: a drag driven
/// straight through the patch crosses several bushes (the
/// patch is a cluster — no 3-clean-cells-either-side row
/// through a single bush exists on this map). Every cell the
/// line crosses is accounted by reading the ground first:
/// a cell founds iff it is explored, unblocked, node-free
/// and unreserved; every other cell is skipped and never
/// charged. The founded set must equal that read exactly,
/// and the charge exactly 3 × founded. (The bush's cell is
/// not even blocked — the refusal is the live-node check,
/// not the terrain.)
#[test]
fn b_drag_crossing_berry_bush_skips_without_charge() {
    let mut sim = Sim::new(42);
    let (tcx, tcy) = tc_cell(&sim);
    // The patch's centroid, from the live bushes themselves.
    let bushes: Vec<(u32, u32)> = sim
        .nodes()
        .iter()
        .filter(|n| !n.is_depleted() && n.kind == crate::NodeKind::BerryBush)
        .map(|n| sim.grid().world_to_cell(n.pos_x, n.pos_y))
        .collect();
    assert!(!bushes.is_empty(), "staging: no berry patch on seed 42");
    let cy = bushes.iter().map(|b| b.1).sum::<u32>() / bushes.len() as u32;
    let minx = bushes.iter().map(|b| b.0).min().unwrap();
    let maxx = bushes.iter().map(|b| b.0).max().unwrap();
    let a = (minx - 3, cy);
    let b = (maxx + 3, cy);
    explore_cells(&mut sim, &[a, b]);
    // Expected founded set, read off the ground.
    let node_cells: Vec<(u32, u32)> = sim
        .nodes()
        .iter()
        .filter(|n| !n.is_depleted())
        .map(|n| sim.grid().world_to_cell(n.pos_x, n.pos_y))
        .collect();
    let expected: Vec<(u32, u32)> = crate::wall_line_cells(a.0, a.1, b.0, b.1)
        .into_iter()
        .filter(|&(x, y)| {
            sim.cell_explored_by(OWNER_PLAYER, x, y)
                && !sim.grid().is_blocked(x, y)
                && !node_cells.contains(&(x, y))
                && !((tcx as i32 - 1..tcx as i32 + 3).contains(&(x as i32))
                    && (tcy as i32 - 1..tcy as i32 + 3).contains(&(y as i32)))
        })
        .collect();
    let crossed_bushes = crate::wall_line_cells(a.0, a.1, b.0, b.1)
        .iter()
        .filter(|c| bushes.contains(c))
        .count();
    assert!(crossed_bushes >= 1, "staging: the drag must cross the patch");
    let m = measure_drag(&mut sim, 1, a, b);
    eprintln!(
        "BUILDPROBE {} crossed_bushes={crossed_bushes} expected={expected:?}",
        drag_report("B-berry", &m)
    );
    assert_eq!(
        m.founded, expected,
        "founded must be exactly the placeable cells of the line: {}",
        drag_report("B-berry", &m)
    );
    assert_eq!(
        m.wood_spent,
        3 * expected.len() as i32,
        "charge must be 3 x founded, never the full line: {}",
        drag_report("B-berry", &m)
    );
}

/// A drag along the Town Center's reserved 4×4: the reserved
/// cells are refused and uncharged; the cells outside the
/// reserve found normally.
#[test]
fn b_drag_crossing_tc_reserve_skips_without_charge() {
    let mut sim = Sim::new(42);
    let (tcx, tcy) = tc_cell(&sim);
    // The reserve's top row (tcy-1), dragged from 3 cells west
    // of the reserve to 3 cells east of it.
    let a = (tcx - 4, tcy - 1);
    let b = (tcx + 5, tcy - 1);
    let expansion = crate::wall_line_cells(a.0, a.1, b.0, b.1);
    let reserved: Vec<(u32, u32)> = expansion
        .iter()
        .copied()
        .filter(|&(x, y)| {
            (tcx as i32 - 1..tcx as i32 + 3).contains(&(x as i32))
                && (tcy as i32 - 1..tcy as i32 + 3).contains(&(y as i32))
        })
        .collect();
    assert_eq!(reserved.len(), 4, "staging: the drag must cross exactly the reserve's 4 cells");
    let m = measure_drag(&mut sim, 1, a, b);
    eprintln!("BUILDPROBE {}", drag_report("B-tc-reserve", &m));
    for cell in &reserved {
        assert!(!m.founded.contains(cell), "a segment was founded on reserved cell {cell:?}");
    }
    assert_eq!(
        m.founded.len(),
        m.expansion.len() - reserved.len(),
        "founded must be the expansion minus the reserve: {}",
        drag_report("B-tc-reserve", &m)
    );
    assert_eq!(m.wood_spent, 3 * m.founded.len() as i32, "charge must follow founded exactly: {}", drag_report("B-tc-reserve", &m));
}

/// A drag from explored ground into the black: BLD-01 refuses
/// the unexplored segments — skipped, uncharged — while the
/// explored part of the very same drag founds normally.
#[test]
fn b_drag_into_unexplored_skips_without_charge() {
    let mut sim = Sim::new(42);
    let (tcx, tcy) = tc_cell(&sim);
    // Walk east along a few rows to find the explored boundary
    // on clean ground (no trees/nodes/reserve in the window).
    let node_cells: Vec<(u32, u32)> = sim
        .nodes()
        .iter()
        .filter(|n| !n.is_depleted())
        .map(|n| sim.grid().world_to_cell(n.pos_x, n.pos_y))
        .collect();
    let mut staged: Option<((u32, u32), (u32, u32), usize)> = None;
    'rows: for cy in (tcy + 3)..(tcy + 20) {
        let mut bx: Option<u32> = None;
        for cx in tcx..110u32 {
            if !sim.cell_explored_by(OWNER_PLAYER, cx, cy) {
                bx = Some(cx);
                break;
            }
        }
        let Some(bx) = bx else { continue };
        if bx < tcx + 5 {
            continue;
        }
        let a = (bx - 3, cy);
        let b = (bx + 2, cy);
        let cells = crate::wall_line_cells(a.0, a.1, b.0, b.1);
        // Every EXPLORED cell of the window must be foundable
        // ground (unblocked, node-free); unexplored cells are
        // refused whatever they hold.
        let clean = cells.iter().all(|&(x, y)| {
            !sim.cell_explored_by(OWNER_PLAYER, x, y)
                || (!sim.grid().is_blocked(x, y) && !node_cells.contains(&(x, y)))
        });
        if clean {
            let explored_count = cells
                .iter()
                .filter(|&&(x, y)| sim.cell_explored_by(OWNER_PLAYER, x, y))
                .count();
            staged = Some((a, b, explored_count));
            break 'rows;
        }
    }
    let (a, b, explored_count) = staged.expect("no explored-boundary row on seed 42");
    assert!(explored_count >= 1 && explored_count < 6, "staging: the drag must straddle the boundary");
    let m = measure_drag(&mut sim, 1, a, b);
    eprintln!("BUILDPROBE {} explored_in_line={explored_count}", drag_report("B-unexplored", &m));
    assert_eq!(m.expansion.len(), 6);
    assert_eq!(
        m.founded.len(),
        explored_count,
        "only the explored segments may found: {}",
        drag_report("B-unexplored", &m)
    );
    assert_eq!(m.wood_spent, 3 * explored_count as i32, "unexplored segments must never be charged: {}", drag_report("B-unexplored", &m));
}

/// A drag across a cell another villager is standing on:
/// walls are exempt from the unit check (BLD-20) — the
/// segment founds under his feet and the charge is full.
#[test]
fn b_drag_over_a_standing_unit_founds() {
    let mut sim = Sim::new(42);
    let a = clean_row(&sim, 5);
    let mid = (a.0 + 2, a.1);
    let (mx, my) = anchor_point(&sim, mid.0, mid.1);
    sim.push_command(Command::Move { entity: 2, x: mx, y: my });
    let there = run_until(&mut sim, 20_000, |s| {
        let e = ent(s, 2);
        e.order.is_none() && s.grid().world_to_cell(e.pos_x, e.pos_y) == mid
    });
    assert!(there, "staging broken: villager 2 never stood on the line");
    let m = measure_drag(&mut sim, 1, a, (a.0 + 4, a.1));
    eprintln!("BUILDPROBE {}", drag_report("B-unit", &m));
    assert_eq!(m.founded.len(), 5, "a standing unit must not hole the line: {}", drag_report("B-unit", &m));
    assert!(m.founded.contains(&mid), "the segment under the standing villager is missing");
    assert_eq!(m.wood_spent, 15, "full line, full charge: {}", drag_report("B-unit", &m));
}

/// A drag crossing a standing building's footprint: those
/// cells are refused and uncharged; the rest of the line
/// founds.
#[test]
fn b_drag_over_a_standing_building_skips() {
    let mut sim = Sim::new(42);
    let h = open_anchor_near_tc(&sim, 2, BuildingKind::House);
    build(&mut sim, 2, BuildingKind::House, h.0, h.1);
    let up = run_until(&mut sim, 30_000, |s| {
        s.buildings().iter().any(|b| b.kind == BuildingKind::House && b.complete)
    });
    assert!(up, "staging broken: the house never completed");
    // Drag a horizontal line through the house's top row,
    // extending 2 cells beyond it on both sides.
    let a = (h.0 - 2, h.1);
    let b = (h.0 + 3, h.1);
    let m = measure_drag(&mut sim, 1, a, b);
    eprintln!("BUILDPROBE {}", drag_report("B-building", &m));
    assert_eq!(m.expansion.len(), 6);
    assert_eq!(m.founded.len(), 4, "exactly the house's 2 cells must be skipped: {}", drag_report("B-building", &m));
    assert_eq!(m.wood_spent, 12, "charge follows founded: {}", drag_report("B-building", &m));
}

/// Wood exhaustion mid-drag: the line founds segments until
/// the stockpile cannot pay the next one, then stops — the
/// truncation is exact, never a partial charge, and the
/// leftover wood is the arithmetic remainder.
#[test]
fn b_wood_exhaustion_truncates_exactly() {
    let mut sim = Sim::new(42);
    // Burn the 200 starting wood down to 14 through the real
    // command path: six House placements (6 × 25 = 150,
    // charged at placement) and two 6-segment drags (36).
    // Each house needs a FRESH anchor: the plain ring scan
    // would re-pick a standing foundation's anchor, and a
    // Build there is a Resume — no charge — so the scan here
    // additionally requires the candidate's footprint to
    // clear every standing building, and each placement is
    // asserted to be a fresh, charged foundation.
    let (tcx, tcy) = tc_cell(&sim);
    for i in 0..6u32 {
        let vil = 1 + (i % 3);
        let mut anchor: Option<(u32, u32)> = None;
        'scan: for r in 2..=16i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                    if ax < 0 || ay < 0 {
                        continue;
                    }
                    let (ax, ay) = (ax as u32, ay as u32);
                    let cells: Vec<(u32, u32)> =
                        (0..2).flat_map(|ox| (0..2).map(move |oy| (ax + ox, ay + oy))).collect();
                    let clash = sim
                        .buildings()
                        .iter()
                        .flat_map(|b| b.footprint_cells())
                        .any(|c| cells.contains(&c));
                    if clash {
                        continue;
                    }
                    let (x, y) = anchor_point(&sim, ax, ay);
                    if sim.can_build(vil, BuildingKind::House, x, y) {
                        anchor = Some((ax, ay));
                        break 'scan;
                    }
                }
            }
        }
        let a = anchor.expect("no fresh house anchor near the TC");
        let (n0, w0) = (sim.buildings().len(), wood(&sim));
        build(&mut sim, vil, BuildingKind::House, a.0, a.1);
        sim.tick();
        assert_eq!(sim.buildings().len(), n0 + 1, "house {} was a resume, not a placement", i + 1);
        assert_eq!(wood(&sim), w0 - 25, "house {} did not charge 25", i + 1);
    }
    assert_eq!(wood(&sim), 50, "staging: six houses must leave 50 wood");
    // Two 6-segment burn drags (2 × 18 = 36): after the six
    // houses, no 12-cell clean row survives near the TC, but
    // 6-cell rows do (each scan excludes the standing
    // buildings, so the three drags land on distinct rows).
    for vil in [1u32, 2] {
        let a = clean_row(&sim, 6);
        let m1 = measure_drag(&mut sim, vil, a, (a.0 + 5, a.1));
        eprintln!("BUILDPROBE {}", drag_report("B-burn", &m1));
        assert_eq!(m1.founded.len(), 6, "the burn drag must found fully on clean ground: {}", drag_report("B-burn", &m1));
    }
    assert_eq!(wood(&sim), 14, "staging: 50 - 36 must leave 14");
    // A second drag of 6 on another clean row: 4 segments
    // found (12 wood), 2 refused, 2 wood left.
    let c = clean_row(&sim, 6);
    let m2 = measure_drag(&mut sim, 3, c, (c.0 + 5, c.1));
    eprintln!("BUILDPROBE {}", drag_report("B-exhaust", &m2));
    assert_eq!(m2.founded.len(), 4, "14 wood founds exactly 4 segments: {}", drag_report("B-exhaust", &m2));
    assert_eq!(m2.wood_spent, 12, "the truncated drag charges only what it founds: {}", drag_report("B-exhaust", &m2));
    assert_eq!(wood(&sim), 2, "the remainder must be exactly 2");
}

/// The multi-villager line (the client issues the same
/// BuildLine to every selected villager): the second order
/// finds every segment already founded — Resume verdicts, no
/// new buildings, NO second charge — and the two builders
/// distribute along the line instead of piling onto segment
/// one (BLD-20).
#[test]
fn b_second_villager_same_line_no_double_charge() {
    let mut sim = Sim::new(42);
    let a = clean_row(&sim, 5);
    let b = (a.0 + 4, a.1);
    let m1 = measure_drag(&mut sim, 1, a, b);
    assert_eq!(m1.founded.len(), 5);
    assert_eq!(m1.wood_spent, 15);
    let w_after_first = wood(&sim);
    let n_buildings = sim.buildings().len();
    build_line(&mut sim, 2, a, b);
    sim.tick();
    assert_eq!(sim.buildings().len(), n_buildings, "the second line placed duplicates");
    assert_eq!(wood(&sim), w_after_first, "the second line charged again");
    let p1 = ent(&sim, 1).task_building;
    let p2 = ent(&sim, 2).task_building;
    eprintln!(
        "BUILDPROBE B-two-villagers primary1={p1:?} primary2={p2:?} buildings={n_buildings}"
    );
    assert!(p1.is_some() && p2.is_some(), "both villagers must be tasked to the line");
    assert_ne!(p1, p2, "the builders must distribute, not pile onto one segment");
    // Both work; the whole line completes.
    let done = run_until(&mut sim, 60_000, |s| {
        s.buildings()
            .iter()
            .filter(|x| x.kind == BuildingKind::PalisadeWall)
            .all(|x| x.complete)
    });
    assert!(done, "the two-villager line never completed");
}

/// One villager walks the line: a lone builder raises the
/// segments in DRAG ORDER (primary first, then the queued
/// Builds), and the whole line completes.
#[test]
fn b_lone_builder_walks_the_line_in_drag_order() {
    let mut sim = Sim::new(42);
    let a = clean_row(&sim, 5);
    let m = measure_drag(&mut sim, 1, a, (a.0 + 4, a.1));
    assert_eq!(m.founded.len(), 5);
    let ids_in_drag_order: Vec<u32> = m
        .expansion
        .iter()
        .map(|cell| {
            sim.buildings()
                .iter()
                .find(|x| (x.cell_x, x.cell_y) == *cell)
                .unwrap()
                .id
        })
        .collect();
    let mut completion_order: Vec<u32> = Vec::new();
    let mut seen: Vec<u32> = Vec::new();
    let done = run_until(&mut sim, 60_000, |s| {
        for x in s.buildings().iter().filter(|x| x.kind == BuildingKind::PalisadeWall) {
            if x.complete && !seen.contains(&x.id) {
                seen.push(x.id);
                completion_order.push(x.id);
            }
        }
        completion_order.len() == 5
    });
    eprintln!(
        "BUILDPROBE B-line-walk drag_order={ids_in_drag_order:?} completion_order={completion_order:?}"
    );
    assert!(done, "the lone builder never finished the line");
    assert_eq!(
        completion_order, ids_in_drag_order,
        "segments must complete in drag order"
    );
    assert_eq!(ent(&sim, 1).task, Task::Idle, "the builder must stand down at the line's end");
}

/// The reverse drag founds the same cell set (the expansion
/// is reversal-exact), and a single-cell drag founds exactly
/// one segment for exactly 3 wood.
#[test]
fn b_reverse_drag_and_single_cell() {
    let mut sim = Sim::new(42);
    let a = clean_row(&sim, 6);
    let b = (a.0 + 5, a.1);
    let m1 = measure_drag(&mut sim, 1, a, b);
    assert_eq!(m1.founded.len(), 6);
    // Reverse: same endpoints swapped, by villager 2 — every
    // cell already stands, so all Resume: no new buildings,
    // no charge.
    let n = sim.buildings().len();
    let w = wood(&sim);
    build_line(&mut sim, 2, b, a);
    sim.tick();
    assert_eq!(sim.buildings().len(), n, "reverse drag placed duplicates");
    assert_eq!(wood(&sim), w, "reverse drag charged for standing segments");
    eprintln!("BUILDPROBE B-reverse primaries v1={:?} v2={:?}", m1.primary, ent(&sim, 2).task_building);

    // Single cell.
    let mut sim = Sim::new(42);
    let c = clean_row(&sim, 1);
    let m = measure_drag(&mut sim, 1, c, c);
    eprintln!("BUILDPROBE {}", drag_report("B-single", &m));
    assert_eq!(m.founded.len(), 1, "a single-cell drag must found one segment");
    assert_eq!(m.wood_spent, 3, "a single segment charges exactly 3");
}
