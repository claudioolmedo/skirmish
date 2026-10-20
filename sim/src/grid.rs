//! Deterministic coarse grid + A* pathfinding (M1, reshaped by M7).
//!
//! The world is a fixed square spanning `[-WORLD_HALF, WORLD_HALF)` on each
//! axis, divided into a coarse grid of square cells. A* runs on cell indices
//! with integer costs and fully deterministic tie-breaking, so the same
//! query always returns the same path on every platform, including WASM.
//!
//! M7 reshape: the fixed 50×50 test arena (compile-time wall layout) is
//! gone. The grid is now the rulebook's 2-player map size — Tiny,
//! 120×120 tiles (MAP-02) — at the sim's long-standing scale of one
//! cell = one tile = 4 world units, and its contents are *generated*
//! per match by `mapgen`: a terrain layer (land/water, MAP-03/08), a
//! per-cell height layer (MAP-05), and a node-block layer for live
//! trees (MAP-04: a tree is its cell's block until depleted). The
//! dynamic building layer from M2.5 stays as it was. A bare `Grid::new`
//! is flat, empty land — the generator fills it.
//!
//! Determinism notes: the open list is a `BinaryHeap` with a *total*
//! deterministic ordering `(f, h, cell)` — never a `HashMap`. Neighbor
//! expansion follows one fixed order. All costs are integers.

use fixed::types::I16F16;
use std::collections::BinaryHeap;

/// World half-extent in world units: the world spans [-240, 240).
/// (const via raw bits: 240 × 2^16 = 15728640.)
pub const WORLD_HALF: I16F16 = I16F16::from_bits(15_728_640);
/// Cell edge length in world units (coarse grid): 4 × 2^16 = 262144.
/// One cell is one rulebook tile (MAP-01's convention, M3).
pub const CELL_SIZE: I16F16 = I16F16::from_bits(262_144);
/// Grid dimension, in cells per axis: (2 * 240) / 4 = 120 — the
/// rulebook's Tiny map, the 2-player size (MAP-02).
pub const GRID_DIM: u32 = 120;

/// Terrain tag: ordinary land — every land unit may stand here.
pub const TERRAIN_LAND: u8 = 0;
/// Terrain tag: water — impassable to land units (MAP-08: no wading;
/// ships do not exist yet, so water is a pure obstacle in M7).
pub const TERRAIN_WATER: u8 = 1;
/// Highest elevation level the generator produces (MAP-05: the
/// reference's per-tile height byte has 7 levels; Skirmish's Arabia
/// hills use 0..=4 — see `mapgen`).
pub const MAX_HEIGHT: u8 = 4;

/// A* step costs: orthogonal = 10, diagonal = 14 (≈ 10·√2, integer).
const COST_ORTHO: i64 = 10;
const COST_DIAG: i64 = 14;

fn cell_count() -> usize {
    (GRID_DIM * GRID_DIM) as usize
}

/// The coarse navigation grid. All layers are explicit state and part
/// of the state hash: terrain and heights from the map generator,
/// the node-block layer tracking live trees, and the dynamic layer
/// of cells blocked by placed buildings (M2.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grid {
    /// Per-cell terrain tag (`TERRAIN_LAND` / `TERRAIN_WATER`),
    /// row-major. Set once by the generator; never changes in play.
    terrain: Vec<u8>,
    /// Per-cell elevation level, 0..=`MAX_HEIGHT`, row-major.
    /// Data only (MAP-06's combat factor reads it); it never gates
    /// movement — AoE elevation does not slow or block land units.
    height: Vec<u8>,
    /// Cells blocked by a live tree node (MAP-04), row-major 0/1.
    /// Set at generation / farm growth, cleared the moment the tree
    /// depletes: a harvested tree stops blocking its cell.
    node_block: Vec<u8>,
    /// Cells blocked at runtime by buildings (M2.5), row-major 0/1.
    dynamic: Vec<u8>,
}

impl Grid {
    /// An empty map: flat land everywhere, no water, no trees, no
    /// buildings. The match generator starts from this and carves
    /// the match's Arabia into it.
    pub fn new() -> Self {
        Grid {
            terrain: vec![TERRAIN_LAND; cell_count()],
            height: vec![0; cell_count()],
            node_block: vec![0; cell_count()],
            dynamic: vec![0; cell_count()],
        }
    }

    pub fn width(&self) -> u32 {
        GRID_DIM
    }

    pub fn height(&self) -> u32 {
        GRID_DIM
    }

    fn idx(x: u32, y: u32) -> usize {
        (y * GRID_DIM + x) as usize
    }

    /// Out-of-bounds counts as blocked: the world edge is a wall
    /// (MAP-11). A cell is blocked when it is water, holds a live
    /// tree, or a placed building occupies it.
    pub fn is_blocked(&self, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return true;
        }
        let i = Self::idx(x, y);
        self.terrain[i] == TERRAIN_WATER || self.node_block[i] != 0 || self.dynamic[i] != 0
    }

    /// Whether the terrain itself blocks this cell (water). Out of
    /// bounds counts as terrain-blocked (the edge is a wall).
    pub fn is_water(&self, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return true;
        }
        self.terrain[Self::idx(x, y)] == TERRAIN_WATER
    }

    /// The terrain tag of a cell (out of bounds reads as water —
    /// the wall at the world's edge).
    pub fn terrain_at(&self, x: u32, y: u32) -> u8 {
        if x >= GRID_DIM || y >= GRID_DIM {
            return TERRAIN_WATER;
        }
        self.terrain[Self::idx(x, y)]
    }

    /// The nearest unblocked cell to (x, y), searching expanding
    /// Chebyshev rings (radius 1..=max_ring) in a fixed scan order
    /// (dy outer, dx inner — fully deterministic). `None` when no
    /// cell in range is free. Used to land a move order whose
    /// target cell is itself blocked next to the obstacle instead
    /// of dropping the order (the reference game's behavior: a
    /// click on a tree or a building walks the unit up to it).
    pub fn nearest_free_cell(&self, x: u32, y: u32, max_ring: u32) -> Option<(u32, u32)> {
        if !self.is_blocked(x, y) {
            return Some((x, y));
        }
        for r in 1..=max_ring as i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue; // ring cells only
                    }
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 {
                        continue;
                    }
                    if !self.is_blocked(nx as u32, ny as u32) {
                        return Some((nx as u32, ny as u32));
                    }
                }
            }
        }
        None
    }

    /// Set a cell's terrain tag (generator use).
    pub fn set_terrain(&mut self, x: u32, y: u32, tag: u8) {
        if x >= GRID_DIM || y >= GRID_DIM {
            return;
        }
        let i = Self::idx(x, y);
        self.terrain[i] = tag;
    }

    /// The elevation level of a cell, 0..=`MAX_HEIGHT` (out of
    /// bounds reads as 0).
    pub fn height_at(&self, x: u32, y: u32) -> u8 {
        if x >= GRID_DIM || y >= GRID_DIM {
            return 0;
        }
        self.height[Self::idx(x, y)]
    }

    /// Set a cell's elevation level (generator use).
    pub fn set_height(&mut self, x: u32, y: u32, level: u8) {
        if x >= GRID_DIM || y >= GRID_DIM {
            return;
        }
        let i = Self::idx(x, y);
        self.height[i] = level;
    }

    /// Whether a live tree blocks this cell (the node-block layer
    /// alone — water and buildings do not count).
    pub fn is_node_blocked(&self, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return false;
        }
        self.node_block[Self::idx(x, y)] != 0
    }

    /// Set or clear the tree block on one cell (generation, and the
    /// economy phase the tick a tree depletes — MAP-04).
    pub fn set_node_block(&mut self, x: u32, y: u32, value: bool) {
        if x >= GRID_DIM || y >= GRID_DIM {
            return;
        }
        let i = Self::idx(x, y);
        self.node_block[i] = u8::from(value);
    }

    /// Whether a building (dynamic layer) blocks this cell. Terrain
    /// and trees alone do not count.
    pub fn is_dynamic(&self, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return false;
        }
        self.dynamic[Self::idx(x, y)] != 0
    }

    /// Set or clear the dynamic (building) block on one cell.
    pub fn set_dynamic(&mut self, x: u32, y: u32, value: bool) {
        if x >= GRID_DIM || y >= GRID_DIM {
            return;
        }
        let i = Self::idx(x, y);
        self.dynamic[i] = u8::from(value);
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

    /// Terrain tags as row-major bytes — for the state hash and the
    /// WASM terrain array the renderer paints from.
    pub fn terrain_bytes(&self) -> &[u8] {
        &self.terrain
    }

    /// Elevation levels as row-major bytes — for the state hash and
    /// the WASM heights array (rendering lift/shading).
    pub fn height_bytes(&self) -> &[u8] {
        &self.height
    }

    /// Tree-blocked cells as row-major bytes (0/1) — for the state
    /// hash, so a falling tree is replay-visible.
    pub fn node_block_bytes(&self) -> &[u8] {
        &self.node_block
    }

    /// Dynamic (building-blocked) cells as row-major bytes (0/1) —
    /// for the state hash, so building placement is replay-visible.
    pub fn dynamic_bytes(&self) -> &[u8] {
        &self.dynamic
    }

    /// Statically impassable cells as row-major bytes (0/1): water
    /// or a live tree. This is the WASM `obstacles()` view the client
    /// mirrors for its placement ghost — terrain truth, without the
    /// building layer (the client reads buildings separately).
    pub fn blocked_bytes(&self) -> Vec<u8> {
        (0..cell_count())
            .map(|i| {
                u8::from(self.terrain[i] == TERRAIN_WATER || self.node_block[i] != 0)
            })
            .collect()
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
                // Corner cutting: a diagonal step is barred by a
                // HARD corner — water or a building on either
                // orthogonal neighbor. A tree is not a wall, it is
                // a trunk standing in its cell: diagonal passage
                // past a tree corner is legal (M7, MAP-04). The
                // strict both-free rule made a felled tree's freed
                // cell a pocket no woodcutter could step into —
                // ringed orthogonally by standing trunks — and the
                // whole forest block behind it unworkable; the
                // long tech tests caught what the arena never had.
                if dx != 0 && dy != 0 {
                    let hard = |x: u32, y: u32| self.is_water(x, y) || self.is_dynamic(x, y);
                    if hard((cx as i32 + dx) as u32, cy) || hard(cx, (cy as i32 + dy) as u32) {
                        continue;
                    }
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
            g.world_to_cell(I16F16::from_num(-240), I16F16::from_num(-240)),
            (0, 0)
        );
        assert_eq!(
            g.world_to_cell(I16F16::from_num(239), I16F16::from_num(239)),
            (119, 119)
        );
        // Cell (0,0) spans [-240, -236); its center is (-238, -238).
        let (wx, wy) = g.cell_center(0, 0);
        assert_eq!(
            (wx, wy),
            (I16F16::from_num(-238), I16F16::from_num(-238))
        );
        // The grid is the rulebook's 2-player size: Tiny, 120×120.
        assert_eq!((g.width(), g.height()), (120, 120));
    }

    #[test]
    fn terrain_layers_block_as_designed() {
        let mut g = Grid::new();
        // Open flat land blocks nothing.
        assert!(!g.is_blocked(60, 60));
        assert_eq!(g.height_at(60, 60), 0);
        // Water blocks (MAP-08) and reads back as water.
        g.set_terrain(60, 60, TERRAIN_WATER);
        assert!(g.is_blocked(60, 60));
        assert!(g.is_water(60, 60));
        // A tree blocks until it is cleared (MAP-04).
        g.set_node_block(61, 60, true);
        assert!(g.is_blocked(61, 60));
        assert!(g.is_node_blocked(61, 60));
        g.set_node_block(61, 60, false);
        assert!(!g.is_blocked(61, 60));
        // Buildings block through their own layer.
        g.set_dynamic(62, 60, true);
        assert!(g.is_blocked(62, 60));
        assert!(g.is_dynamic(62, 60));
        // Height is data, never a block.
        g.set_height(63, 60, 4);
        assert_eq!(g.height_at(63, 60), 4);
        assert!(!g.is_blocked(63, 60));
        // Out of bounds is a wall (MAP-11).
        assert!(g.is_blocked(120, 0));
        assert!(g.is_blocked(0, 120));
    }

    #[test]
    fn astar_routes_around_water() {
        let mut g = Grid::new();
        // A water column at x = 60, rows 20..=99, with a 3-cell gap
        // at rows 58..=60 — the only crossing.
        for y in 20..=99u32 {
            if !(58..=60).contains(&y) {
                g.set_terrain(60, y, TERRAIN_WATER);
            }
        }
        let from = g.cell_center(30, 40);
        let to = g.cell_center(90, 40);
        let path = g.find_path(from, to).expect("a path must exist through the gap");
        for &(wx, wy) in &path[..path.len() - 1] {
            let (cx, cy) = g.world_to_cell(wx, wy);
            assert!(!g.is_blocked(cx, cy), "waypoint inside water");
        }
        // Detour is real: longer than the straight 60-cell run.
        assert!(path.len() > 60, "path did not detour: {}", path.len());
        assert_eq!(*path.last().unwrap(), to);
    }

    #[test]
    fn astar_rejects_blocked_target() {
        let mut g = Grid::new();
        g.set_terrain(60, 40, TERRAIN_WATER);
        let to = g.cell_center(60, 40); // inside the pond
        assert!(g.find_path(g.cell_center(0, 0), to).is_none());
    }

    #[test]
    fn astar_is_deterministic() {
        let g = Grid::new();
        let from = g.cell_center(5, 110);
        let to = g.cell_center(115, 10);
        let a = g.find_path(from, to);
        let b = g.find_path(from, to);
        assert_eq!(a, b);
        assert!(a.is_some());
    }
}
