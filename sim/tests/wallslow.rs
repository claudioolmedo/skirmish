//! WALLSLOW regression (run #442, from the live observation of
//! run #441): a Palisade Wall line's second segment stayed a flat
//! 0-progress foundation for ~5 minutes of live play while its
//! builder stood idle beside it, completing only when the area
//! around it changed.
//!
//! Reproduced in the sim (probe staging, seed 31): segment 2 sits
//! in a pocket — its N/E/S ring cells are completed wall segments
//! and its W ring cell is segment 1 — so its only stand cells are
//! the four diagonals. The builder's assigned stand point is the
//! NW diagonal's pulled point, and the plain A* route to it
//! threads the corridor cell west of it — where the pocket's own
//! builder stands IDLE. Standing units are solid (MOV-03), so
//! steering halts at separation from him, one waypoint short of
//! the stand cell, ~8 world units from the segment — outside the
//! 5.8 build range, so construction never starts.
//!
//! Root cause, in `Sim::repath_around` (MOV-03's bump repath):
//! the avoid list EXCLUDED a blocker's cell whenever the
//! traveller was standing in it. Pressed against the idle
//! villager inside that villager's own cell, the builder's
//! avoid list came up empty, so every "avoiding" route was the
//! plain shortest route — threading the cell, whose centre
//! waypoint sits inside the villager's separation radius and
//! can never be reached. (The exclusion was also unnecessary
//! for escaping the cell: `find_path_avoiding_for` never blocks
//! the querier's own start cell by `avoid`, its documented
//! rule.) Each bump window (83 ticks in the probe) re-ran the
//! repath, got the same route, and pushed again — while a
//! genuine avoiding route around the north existed the whole
//! time. The task layer never intervened: the order existed
//! and its target cell was unblocked, so neither stale-order
//! retry fired. Measured pre-fix: segment 2 at progress 0
//! after 60,000 ticks, builder oscillating in `ToBuild` at
//! the corridor mouth.
//!
//! Fix (in `repath_around`, scoped by the caller to build
//! travel — task ToBuild/Building): a blocker's cell stays in
//! the avoid list even when the traveller shares it, so the
//! avoiding search returns the real detour around a stationary
//! blocker. The scoping is measured: the same inclusion applied
//! to all movement regressed the bellprobe resume tests (the
//! Town Center doorstep crowd relies on the coincident
//! exclusion), and removing the detour legs' through-path
//! fallback as well regressed five engageprobe crowd-ring
//! tests — so gather/military movement keeps the base
//! construction bit-for-bit and the legs keep their fallback.
//! No rule, cost, stat or design change: the same DE behaviour
//! (a builder walks around a bystander to his stand point),
//! reached by the sim's own documented route.
//!
//! WALLSLOW2 addendum (2026-11-20): the strict list is now a
//! FALLBACK, invoked by the caller only when the base repath
//! produced no route or a degenerate one (a waypoint inside a
//! blocker's separation radius — this pocket's through-route
//! qualifies; its (20,66) centre waypoint is unreachable by
//! construction). Run #442's unconditional version re-timed a
//! Town Center doorstep bump whose base route was already
//! executable and deadlocked the auto_explore woodline gather
//! downstream; the fallback keeps that bump bit-identical to
//! base while this pocket still gets the strict detour.

use fixed::types::I16F16;
use skirmish_sim::{BuildingKind, Command, Entity, EntityKind, Sim, OWNER_PLAYER};

const WALL: BuildingKind = BuildingKind::PalisadeWall;

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn villagers(sim: &Sim) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == OWNER_PLAYER && e.kind == EntityKind::Villager)
        .map(|e| e.id)
        .collect()
}

fn find_id(sim: &Sim, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == OWNER_PLAYER && e.kind == kind)
        .unwrap()
        .id
}

fn cell_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn cell_of(sim: &Sim, id: u32) -> (u32, u32) {
    let e = ent(sim, id);
    sim.grid().world_to_cell(e.pos_x, e.pos_y)
}

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> u64 {
    for t in 0..cap {
        if done(sim) {
            return t;
        }
        sim.tick();
    }
    cap
}

/// Stage the pocket on seed 31 and issue the two-segment line.
/// Returns (sim, line_builder, pocket_builder, seg2 cell). The
/// pocket builder is left standing where the staging leaves him —
/// in the corridor cell (20,66) on seed 31, the body that blocks
/// the line builder's route to segment 2's stand point.
fn stage_pocket_line() -> (Sim, u32, u32, (u32, u32)) {
    let mut sim = Sim::new(31);
    let vils = villagers(&sim);
    let tc = ent(&sim, find_id(&sim, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let mut pocket = None;
    'scan: for r in 4..=30i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (cx, cy) = (tcx as i32 + dx, tcy as i32 + dy);
                if cx < 2 || cy < 2 || cx > 115 || cy > 115 {
                    continue;
                }
                let (cx, cy) = (cx as u32, cy as u32);
                let cells = [(cx - 1, cy), (cx, cy), (cx, cy - 1), (cx + 1, cy), (cx, cy + 1)];
                let ok = cells.iter().all(|&(x, y)| {
                    let (wx, wy) = cell_point(&sim, x, y);
                    sim.can_build(vils[0], WALL, wx, wy)
                });
                if ok {
                    pocket = Some((cx, cy));
                    break 'scan;
                }
            }
        }
    }
    let (cx, cy) = pocket.expect("no pocket staging on seed 31");
    assert_eq!((cx, cy), (22, 67), "pocket staging drifted from the probed geometry");
    // The pocket's own builder raises the N/E/S walls first.
    let mut staged = 0usize;
    for c in [(cx, cy - 1), (cx + 1, cy), (cx, cy + 1)] {
        staged += 1;
        let (x, y) = cell_point(&sim, c.0, c.1);
        sim.push_command(Command::Build { entity: vils[1], kind: WALL, x, y });
        let done = run_until(&mut sim, 60_000, |s| {
            let ws: Vec<_> = s.buildings().iter().filter(|b| b.kind == WALL).collect();
            ws.len() == staged && ws.iter().all(|b| b.complete)
        });
        assert!(done < 60_000, "pocket wall at {c:?} never completed");
    }
    // The line: segment 1 at (cx-1, cy), segment 2 in the pocket.
    let (x0, y0) = cell_point(&sim, cx - 1, cy);
    let (x1, y1) = cell_point(&sim, cx, cy);
    sim.push_command(Command::BuildLine { entity: vils[0], kind: WALL, x0, y0, x1, y1 });
    sim.tick();
    assert_eq!(
        sim.buildings().iter().filter(|b| b.kind == WALL).count(),
        5,
        "the line did not found both segments"
    );
    (sim, vils[0], vils[1], (cx, cy))
}

#[test]
fn pocket_second_segment_completes_around_a_standing_villager() {
    let (mut sim, builder, blocker, seg2_cell) = stage_pocket_line();
    // Segment 1 completes first; by then the pocket builder has
    // been displaced into the corridor cell on the line builder's
    // route to segment 2 (seed-31 fact, probed in run #442 — if
    // staging ever drifts, this is the assertion that says so).
    let seg1 = run_until(&mut sim, 30_000, |s| {
        s.buildings()
            .iter()
            .filter(|b| b.kind == WALL && b.complete)
            .count()
            >= 4
    });
    assert!(seg1 < 30_000, "segment 1 never completed");
    assert_eq!(
        cell_of(&sim, blocker),
        (seg2_cell.0 - 2, seg2_cell.1 - 1),
        "the pocket builder no longer stands in the corridor cell"
    );
    let done = run_until(&mut sim, 30_000, |s| {
        s.buildings().iter().filter(|b| b.kind == WALL).all(|b| b.complete)
    });
    assert!(
        done < 30_000,
        "the pocket's second segment never completed: the builder \
         stayed frozen before a 0-progress foundation (WALLSLOW)"
    );
    let v = ent(&sim, builder);
    assert!(v.task_building.is_none(), "the line builder is free again");
}

#[test]
fn pocket_second_segment_completes_with_the_corridor_clear() {
    // Control: the same pocket with the corridor villager walked
    // away after staging — the line completes on the plain route,
    // before and after the WALLSLOW fix alike.
    let (mut sim, builder, blocker, seg2_cell) = stage_pocket_line();
    let tc = ent(&sim, find_id(&sim, EntityKind::TownCenter));
    let (tx, ty) = (tc.pos_x, tc.pos_y);
    sim.push_command(Command::Move { entity: blocker, x: tx, y: ty });
    let done = run_until(&mut sim, 30_000, |s| {
        s.buildings().iter().filter(|b| b.kind == WALL).all(|b| b.complete)
    });
    assert!(done < 30_000, "the pocket line never completed with a clear corridor");
    let _ = (builder, seg2_cell);
}
