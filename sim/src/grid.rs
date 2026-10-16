//! Deterministic coarse grid + A* pathfinding (M1).
//!
//! The world is a fixed square spanning `[-WORLD_HALF, WORLD_HALF)` on each
//! axis, divided into a coarse grid of square cells. A* runs on cell indices
//! with integer costs and fully deterministic tie-breaking, so the same
//! query always returns the same path on every platform, including WASM.
//!
//! Determinism notes: the open list is a `BinaryHeap` with a *total*
//! deterministic ordering `(f, h, cell)` — never a `HashMap`. Neighbor
//! expansion follows one fixed order. All costs are integers.

use fixed::types::I16F16;
use std::collections::BinaryHeap;

/// World half-extent in world units: the world spans [-100, 100).
/// (const via raw bits: 100 × 2^16 = 6553600.)
pub const WORLD_HALF: I16F16 = I16F16::from_bits(6_553_600);
/// Cell edge length in world units (coarse grid): 4 × 2^16 = 262144.
pub const CELL_SIZE: I16F16 = I16F16::from_bits(262_144);
/// Grid dimension, in cells per axis: (2 * 100) / 4.
pub const GRID_DIM: u32 = 50;

/// A* step costs: orthogonal = 10, diagonal = 14 (≈ 10·√2, integer).
const COST_ORTHO: i64 = 10;
const COST_DIAG: i64 = 14;

/// Static obstacle layout. Walls are compile-time constants, so the layout
/// is identical for every seed — deterministic by construction:
///
/// - vertical wall at column x = 25, rows 8..=38, with a 3-cell gap at
///   rows 22..=24 (the only crossing);
/// - horizontal wall at row y = 32, columns 6..=20.
///
/// The map stays fully connected: everything is reachable through the gap
/// or around the wall ends.
fn is_wall_cell(x: u32, y: u32) -> bool {
    if x == 25 && (8..=38).contains(&y) && !(22..=24).contains(&y) {
        return true;
    }
    if y == 32 && (6..=20).contains(&x) {
        return true;
    }
    false
}

/// The coarse navigation grid. Static obstacles (walls) are compile-time
/// constants; M2.5 adds a second, dynamic layer for cells blocked by
/// placed buildings (houses). The struct exists so both layers are
/// explicit state (and part of the state hash).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    blocked: Vec<bool>, // row-major, GRID_DIM * GRID_DIM — static walls
    /// Cells blocked at runtime by buildings (M2.5). Kept separate from
    /// the static layer: `blocked_bytes` (state hash, debug preview)
    /// stays wall-only, and the sim hashes this layer on its own.
    dynamic: Vec<bool>, // row-major, GRID_DIM * GRID_DIM
}

impl Grid {
    pub fn new() -> Self {
        let mut blocked = vec![false; (GRID_DIM * GRID_DIM) as usize];
        for y in 0..GRID_DIM {
            for x in 0..GRID_DIM {
                if is_wall_cell(x, y) {
                    blocked[(y * GRID_DIM + x) as usize] = true;
                }
            }
        }
        Grid {
            blocked,
            dynamic: vec![false; (GRID_DIM * GRID_DIM) as usize],
        }
    }

    pub fn width(&self) -> u32 {
        GRID_DIM
    }

    pub fn height(&self) -> u32 {
        GRID_DIM
    }

    /// Out-of-bounds counts as blocked: the world edge is a wall. A cell
    /// is blocked when a static wall *or* a placed building occupies it.
    pub fn is_blocked(&self, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return true;
        }
        let i = (y * GRID_DIM + x) as usize;
        self.blocked[i] || self.dynamic[i]
    }

    /// Whether a building (dynamic layer) blocks this cell. Static
    /// walls alone do not count.
    pub fn is_dynamic(&self, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return false;
        }
        self.dynamic[(y * GRID_DIM + x) as usize]
    }

    /// Set or clear the dynamic (building) block on one cell.
    pub fn set_dynamic(&mut self, x: u32, y: u32, value: bool) {
        if x >= GRID_DIM || y >= GRID_DIM {
            return;
        }
        let i = (y * GRID_DIM + x) as usize;
        self.dynamic[i] = value;
    }

    /// World coordinates → cell indices, clamped into the grid.
    pub fn world_to_cell(&self, wx: I16F16, wy: I16F16) -> (u32, u32) {
        let cx = ((wx + WORLD_HALF) / CELL_SIZE)
            .floor()
            .to_num::<i32>()
            .clamp(0, GRID_DIM as i32 - 1) as u32;
        let cy = ((wy + WORLD_HALF) / CELL_SIZE)
            .floor()
            .to_num::<i32>()
            .clamp(0, GRID_DIM as i32 - 1) as u32;
        (cx, cy)
    }

    /// Cell indices → world coordinates of the cell center.
    pub fn cell_center(&self, cx: u32, cy: u32) -> (I16F16, I16F16) {
        let wx =
            I16F16::from_num(cx as i32) * CELL_SIZE + CELL_SIZE / I16F16::from_num(2) - WORLD_HALF;
        let wy =
            I16F16::from_num(cy as i32) * CELL_SIZE + CELL_SIZE / I16F16::from_num(2) - WORLD_HALF;
        (wx, wy)
    }

    /// Static wall cells as row-major bytes (0/1) — for the state hash
    /// and the debug preview. The dynamic building layer is *not*
    /// included (the preview draws buildings themselves); see
    /// `dynamic_bytes`.
    pub fn blocked_bytes(&self) -> Vec<u8> {
        self.blocked.iter().map(|&b| u8::from(b)).collect()
    }

    /// Dynamic (building-blocked) cells as row-major bytes (0/1) — for
    /// the state hash, so building placement is replay-visible.
    pub fn dynamic_bytes(&self) -> Vec<u8> {
        self.dynamic.iter().map(|&b| u8::from(b)).collect()
    }

    /// A* from one world point to another.
    ///
    /// Returns waypoints as world coordinates: the centers of the traversed
    /// cells (skipping the start cell), ending exactly on the target point.
    /// Returns `None` when the target cell is blocked or unreachable.
    pub fn find_path(
        &self,
        from: (I16F16, I16F16),
        to: (I16F16, I16F16),
    ) -> Option<Vec<(I16F16, I16F16)>> {
        let start = self.world_to_cell(from.0, from.1);
        let goal = self.world_to_cell(to.0, to.1);
        if self.is_blocked(goal.0, goal.1) {
            return None;
        }
        let cells = self.astar_cells(start, goal)?;
        let mut waypoints: Vec<(I16F16, I16F16)> = cells
            .iter()
            .skip(1)
            .map(|&(cx, cy)| self.cell_center(cx, cy))
            .collect();
        // Finish exactly on the commanded point, not the cell center.
        waypoints.push(to);
        Some(waypoints)
    }

    /// A* over cell indices. Returns the cell trail from start to goal.
    fn astar_cells(&self, start: (u32, u32), goal: (u32, u32)) -> Option<Vec<(u32, u32)>> {
        if start == goal {
            return Some(vec![start]);
        }
        let cell_index = |x: u32, y: u32| (y * GRID_DIM + x) as usize;
        const INF: i64 = i64::MAX / 4;
        let n = (GRID_DIM * GRID_DIM) as usize;
        let mut g_score = vec![INF; n];
        let mut came_from = vec![u32::MAX; n];
        let mut closed = vec![false; n];

        /// Heap node with a *total* deterministic ordering. `BinaryHeap` is a
        /// max-heap, so the comparison is reversed to pop the smallest
        /// (f, h, cell) first. Ties can never resolve nondeterministically:
        /// every field is an integer and `cell` is unique.
        #[derive(Clone, Copy, PartialEq, Eq)]
        struct Node {
            f: i64,
            h: i64,
            cell: u32,
        }
        impl Ord for Node {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                other
                    .f
                    .cmp(&self.f)
                    .then_with(|| other.h.cmp(&self.h))
                    .then_with(|| other.cell.cmp(&self.cell))
            }
        }
        impl PartialOrd for Node {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        // Octile-distance heuristic, integer, admissible.
        let heuristic = |x: u32, y: u32| -> i64 {
            let dx = x.abs_diff(goal.0) as i64;
            let dy = y.abs_diff(goal.1) as i64;
            let (lo, hi) = if dx < dy { (dx, dy) } else { (dy, dx) };
            COST_ORTHO * hi + (COST_DIAG - 2 * COST_ORTHO) * lo
        };

        // Fixed neighbor expansion order: E, W, N, S, NE, SE, NW, SW.
        const DIRS: [(i32, i32, i64); 8] = [
            (1, 0, COST_ORTHO),
            (-1, 0, COST_ORTHO),
            (0, 1, COST_ORTHO),
            (0, -1, COST_ORTHO),
            (1, 1, COST_DIAG),
            (1, -1, COST_DIAG),
            (-1, 1, COST_DIAG),
            (-1, -1, COST_DIAG),
        ];

        let start_idx = cell_index(start.0, start.1);
        let goal_idx = cell_index(goal.0, goal.1);
        g_score[start_idx] = 0;
        let h0 = heuristic(start.0, start.1);
        let mut open = BinaryHeap::new();
        open.push(Node {
            f: h0,
            h: h0,
            cell: start_idx as u32,
        });

        while let Some(Node { cell, .. }) = open.pop() {
            let ci = cell as usize;
            if closed[ci] {
                continue;
            }
            closed[ci] = true;
            if ci == goal_idx {
                let mut path = vec![goal];
                let mut c = ci;
                while c != start_idx {
                    c = came_from[c] as usize;
                    path.push(((c as u32) % GRID_DIM, (c as u32) / GRID_DIM));
                }
                path.reverse();
                return Some(path);
            }
            let cx = (ci as u32) % GRID_DIM;
            let cy = (ci as u32) / GRID_DIM;
            for &(dx, dy, cost) in &DIRS {
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if nx < 0 || ny < 0 || nx >= GRID_DIM as i32 || ny >= GRID_DIM as i32 {
                    continue;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                if self.is_blocked(nx, ny) {
                    continue;
                }
                // No corner cutting: a diagonal step requires both
                // orthogonal neighbors to be free.
                if dx != 0
                    && dy != 0
                    && (self.is_blocked((cx as i32 + dx) as u32, cy)
                        || self.is_blocked(cx, (cy as i32 + dy) as u32))
                {
                    continue;
                }
                let ni = cell_index(nx, ny);
                let tentative = g_score[ci] + cost;
                if tentative < g_score[ni] {
                    g_score[ni] = tentative;
                    came_from[ni] = ci as u32;
                    let h = heuristic(nx, ny);
                    open.push(Node {
                        f: tentative + h,
                        h,
                        cell: ni as u32,
                    });
                }
            }
        }
        None
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_to_cell_mapping() {
        let g = Grid::new();
        assert_eq!(
            g.world_to_cell(I16F16::from_num(-100), I16F16::from_num(-100)),
            (0, 0)
        );
        assert_eq!(
            g.world_to_cell(I16F16::from_num(99), I16F16::from_num(99)),
            (49, 49)
        );
        // Cell (0,0) spans [-100, -96); its center is (-98, -98).
        let (wx, wy) = g.cell_center(0, 0);
        assert_eq!((wx, wy), (I16F16::from_num(-98), I16F16::from_num(-98)));
    }

    #[test]
    fn wall_layout_as_designed() {
        let g = Grid::new();
        // Vertical wall body is blocked…
        assert!(g.is_blocked(25, 10));
        assert!(g.is_blocked(25, 38));
        // …but the gap rows are open.
        assert!(!g.is_blocked(25, 22));
        assert!(!g.is_blocked(25, 23));
        assert!(!g.is_blocked(25, 24));
        // Horizontal wall body is blocked.
        assert!(g.is_blocked(10, 32));
        // Open field stays open.
        assert!(!g.is_blocked(0, 0));
        assert!(!g.is_blocked(49, 49));
    }

    #[test]
    fn astar_routes_around_wall() {
        let g = Grid::new();
        // Straight line at row 10 crosses the vertical wall (x = 25, rows
        // 8..=38, gap only at 22..=24): the path must detour.
        let from = g.cell_center(10, 10);
        let to = g.cell_center(40, 10);
        let path = g.find_path(from, to).expect("a path must exist");
        // No waypoint (except the exact final target) may sit in a wall.
        for &(wx, wy) in &path[..path.len() - 1] {
            let (cx, cy) = g.world_to_cell(wx, wy);
            assert!(!g.is_blocked(cx, cy), "waypoint inside a wall");
        }
        // Detour is real: the cell trail is longer than the straight
        // Manhattan distance (30 cells).
        assert!(path.len() > 30, "path did not detour: {}", path.len());
        // And it ends exactly on the target.
        assert_eq!(*path.last().unwrap(), to);
    }

    #[test]
    fn astar_rejects_blocked_target() {
        let g = Grid::new();
        let to = g.cell_center(25, 10); // inside the vertical wall
        assert!(g.find_path(g.cell_center(0, 0), to).is_none());
    }

    #[test]
    fn astar_is_deterministic() {
        let g = Grid::new();
        let from = g.cell_center(5, 45);
        let to = g.cell_center(45, 5);
        let a = g.find_path(from, to);
        let b = g.find_path(from, to);
        assert_eq!(a, b);
        assert!(a.is_some());
    }
}
