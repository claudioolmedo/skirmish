//! Wall-drag2 staging pins (bot-loop walldrag2 builder, 2026-11-21 —
//! live run #459 open question).
//!
//! Live #459: a ~5-tile Palisade drag south of the TC stood only
//! ONE segment (-3 wood, exact for 1) on the published build
//! (b274be5), while run #455's watch on the same build passed with
//! 5 segments (-15) and 4 segments (-12). The question: were the
//! other cells rejected by terrain validation (correct sim
//! behaviour), or did the client commit collapse the line to one
//! cell, or did the ghost preview promise more than was placed?
//!
//! This suite pins the SIM half of the verdict (the client half is
//! web-preview/play/walldrag2.test.mjs):
//!
//!   * On verified-open ground a 5-cell BuildLine drag stands
//!     exactly 5 segments and charges exactly 15 wood.
//!   * Across obstructed ground (the Town Center reservation, a
//!     live resource node) the line stands EXACTLY the cells that
//!     individually validate (`can_build` per cell, measured before
//!     the drag) and charges exactly 3 wood per standing segment —
//!     no more, no fewer. Terrain rejection is per-cell and total:
//!     a drag can legitimately stand any subset, including one.
//!   * A 5-cell drag near the TC that stands exactly 1 segment
//!     with a -3 charge EXISTS in the starting position (found by
//!     scan on seed 31): four of its five cells are individually
//!     invalid (reserved / unexplored / node ground). One standing
//!     segment with a -3 charge is therefore, by itself, NOT
//!     evidence of a client collapse — the discriminating evidence
//!     is the ghost's per-cell colours and the individually-valid
//!     count of the exact cells dragged, which is what a live
//!     re-test must record.

use fixed::types::I16F16;
use skirmish_sim::{
    wall_line_cells, BuildingKind, Command, EntityKind, NodeKind, ResourceKind, Sim,
    OWNER_PLAYER,
};

const WALL: BuildingKind = BuildingKind::PalisadeWall;

fn cell_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn vil(sim: &Sim) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::Villager)
        .unwrap()
        .id
}

fn tc_cell(sim: &Sim) -> (u32, u32) {
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::TownCenter)
        .unwrap();
    sim.grid().world_to_cell(tc.pos_x, tc.pos_y)
}

fn wall_count(sim: &Sim) -> usize {
    sim.buildings().iter().filter(|b| b.kind == WALL).count()
}

/// Per-cell individual validity of a drag's expansion, measured
/// BEFORE the drag is issued.
fn valid_cells(sim: &Sim, a: (u32, u32), b: (u32, u32)) -> Vec<bool> {
    let v = vil(sim);
    wall_line_cells(a.0, a.1, b.0, b.1)
        .iter()
        .map(|&(cx, cy)| {
            let (x, y) = cell_point(sim, cx, cy);
            sim.can_build(v, WALL, x, y)
        })
        .collect()
}

/// Issue the drag; return (segments standing, wood spent).
fn drag(sim: &mut Sim, a: (u32, u32), b: (u32, u32)) -> (usize, i32) {
    let v = vil(sim);
    let wood0 = sim.stockpile().amount(ResourceKind::Wood);
    let (x0, y0) = cell_point(sim, a.0, a.1);
    let (x1, y1) = cell_point(sim, b.0, b.1);
    sim.push_command(Command::BuildLine { entity: v, kind: WALL, x0, y0, x1, y1 });
    sim.tick();
    let spent = (wood0 - sim.stockpile().amount(ResourceKind::Wood)).to_num::<i32>();
    (wall_count(sim), spent)
}

/// The start of a horizontal run of `len` cells where every cell
/// individually validates (the walls.rs open-ground scan).
fn open_run(sim: &Sim, len: u32) -> (u32, u32) {
    let v = vil(sim);
    let (tcx, tcy) = tc_cell(sim);
    for r in 3..=40i32 {
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
                let ok = (0..len).all(|i| {
                    let (x, y) = cell_point(sim, ax + i, ay);
                    sim.can_build(v, WALL, x, y)
                });
                if ok {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no open wall run of {len} near the Town Center");
}

#[test]
fn open_ground_five_cell_drag_stands_five_and_charges_15() {
    let mut sim = Sim::new(31);
    let (ax, ay) = open_run(&sim, 5);
    let valid = valid_cells(&sim, (ax, ay), (ax + 4, ay));
    assert!(valid.iter().all(|v| *v), "staging: the run is verified open");
    let (stood, spent) = drag(&mut sim, (ax, ay), (ax + 4, ay));
    assert_eq!(stood, 5, "open ground: every segment of the 5-cell drag stands");
    assert_eq!(spent, 15, "open ground: exactly 5 x 3 wood is charged");
}

#[test]
fn drag_through_tc_reservation_stands_only_valid_cells() {
    let mut sim = Sim::new(31);
    let (tcx, tcy) = tc_cell(&sim);
    // Straight through the TC along its row: the reserved 4x4 must
    // stay empty, the outer cells must stand.
    let a = (tcx - 6, tcy);
    let b = (tcx + 6, tcy);
    let valid = valid_cells(&sim, a, b);
    let valid_n = valid.iter().filter(|v| **v).count();
    assert!(valid_n < valid.len(), "staging: the reservation obstructs this drag");
    assert!(valid_n > 1, "staging: the outer cells are valid");
    let (stood, spent) = drag(&mut sim, a, b);
    assert_eq!(stood, valid_n, "the line stands exactly the individually-valid cells");
    assert_eq!(spent, 3 * stood as i32, "charge is exactly 3 per standing segment");
    for bd in sim.buildings().iter().filter(|b| b.kind == WALL) {
        let in_reserve = bd.cell_x >= tcx - 1 && bd.cell_x <= tcx + 2
            && bd.cell_y >= tcy - 1 && bd.cell_y <= tcy + 2;
        assert!(!in_reserve, "no segment stands on reserved ground");
    }
}

#[test]
fn drag_across_a_live_node_stands_the_rest_and_charges_per_segment() {
    // Find (on a fresh sim, by scan) a 5-cell horizontal drag whose
    // expansion crosses a live Tree node's cell, with the node cell
    // individually invalid and at least 3 other cells valid; then
    // issue exactly that drag on a fresh sim.
    let probe = Sim::new(31);
    let node_cells: Vec<(u32, u32)> = probe
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Tree && n.amount_left > I16F16::ZERO)
        .map(|n| probe.grid().world_to_cell(n.pos_x, n.pos_y))
        .collect();
    assert!(!node_cells.is_empty(), "staging: the map has live trees");
    let mut staged = None;
    'scan: for &(nx, ny) in &node_cells {
        for back in 0..5u32 {
            if nx < back {
                continue;
            }
            let a = (nx - back, ny);
            let b = (a.0 + 4, ny);
            let valid = valid_cells(&probe, a, b);
            let idx = back as usize;
            if !valid[idx] && valid.iter().filter(|v| **v).count() >= 3 {
                staged = Some((a, b, (nx, ny), valid));
                break 'scan;
            }
        }
    }
    let (a, b, node_cell, valid) = staged.expect("a tree-crossing drag exists near the start");
    let valid_n = valid.iter().filter(|v| **v).count();
    let mut sim = Sim::new(31);
    assert_eq!(valid_cells(&sim, a, b), valid, "validity is deterministic across fresh sims");
    let (stood, spent) = drag(&mut sim, a, b);
    assert_eq!(stood, valid_n, "the line stands exactly the valid cells around the tree");
    assert_eq!(spent, 3 * stood as i32);
    assert!(
        !sim.buildings().iter().any(|bd| bd.kind == WALL && (bd.cell_x, bd.cell_y) == node_cell),
        "no segment stands on the live tree's cell"
    );
}

#[test]
fn south_of_tc_drags_stand_exactly_the_valid_subset() {
    // The #459 geometry class: 5-cell drags starting a few cells
    // south of the TC (cell-space south, cell-space east-west along
    // a southern row, and the screen-south diagonal), across
    // several seeds. In every case the sim's answer is the
    // individually-valid subset at 3 wood per standing segment.
    for seed in [1u64, 31, 1234] {
        for (a, b) in south_drags(seed) {
            let mut sim = Sim::new(seed);
            let valid = valid_cells(&sim, a, b);
            let valid_n = valid.iter().filter(|v| **v).count();
            let (stood, spent) = drag(&mut sim, a, b);
            assert_eq!(stood, valid_n, "seed {seed} drag {a:?}->{b:?}: stood == valid");
            assert_eq!(spent, 3 * stood as i32, "seed {seed} drag {a:?}->{b:?}: charge == 3 x stood");
        }
    }
}

fn south_drags(seed: u64) -> Vec<((u32, u32), (u32, u32))> {
    let sim = Sim::new(seed);
    let (tcx, tcy) = tc_cell(&sim);
    let s = |dx: i32, dy: i32| ((tcx as i32 + dx) as u32, (tcy as i32 + dy) as u32);
    vec![
        (s(0, 3), s(0, 7)),   // cell-space south, TC column
        (s(0, 2), s(0, 6)),   // starting at the reservation's edge
        (s(-2, 3), s(2, 3)),  // east-west along a southern row
        (s(0, 3), s(4, 7)),   // screen-south diagonal (+1,+1)
        (s(-1, 3), s(3, 7)),  // screen-south diagonal, offset
    ]
}

#[test]
fn a_one_segment_drag_near_the_tc_exists_and_is_terrain() {
    // Scan the starting position (seed 31) for a 5-cell drag that
    // stands exactly one segment: the scan reads ONLY per-cell
    // validity on one sim, then the drag itself is issued on a
    // fresh sim. Its existence proves the #459 signature (1
    // standing, -3) is reachable by terrain rejection alone.
    let probe = Sim::new(31);
    let (tcx, tcy) = tc_cell(&probe);
    let mut found = None;
    'scan: for dy in -6i32..=6 {
        for dx in -6i32..=6 {
            for (ddx, ddy) in [(1i32, 0i32), (0, 1), (1, 1), (1, -1), (-1, 1)] {
                let a = ((tcx as i32 + dx) as u32, (tcy as i32 + dy) as u32);
                let b = ((a.0 as i32 + 4 * ddx) as u32, (a.1 as i32 + 4 * ddy) as u32);
                let valid = valid_cells(&probe, a, b);
                if valid.iter().filter(|v| **v).count() == 1 {
                    found = Some((a, b, valid));
                    break 'scan;
                }
            }
        }
    }
    let (a, b, valid) = found.expect("a 1-valid-cell drag exists near the TC on seed 31");
    let mut sim = Sim::new(31);
    assert_eq!(valid_cells(&sim, a, b), valid);
    let (stood, spent) = drag(&mut sim, a, b);
    assert_eq!(stood, 1, "the drag stands exactly its one valid cell");
    assert_eq!(spent, 3, "and charges exactly one segment's 3 wood");
    // The standing segment is the valid cell, not an endpoint
    // artefact of the drag's direction.
    let cells = wall_line_cells(a.0, a.1, b.0, b.1);
    let want = cells[valid.iter().position(|v| *v).unwrap()];
    assert!(
        sim.buildings().iter().any(|bd| bd.kind == WALL && (bd.cell_x, bd.cell_y) == want),
        "the standing segment is the individually-valid cell {want:?}"
    );
}
