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
//! per match by `mapgen`: a terrain layer (land/deep water, plus
//! the shallows tag and the two movement domains of the W1 grid
//! half — MAP-03/08, MOV-11), a
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
/// Terrain tag: deep water — impassable to land units, passable to
/// ships (MAP-08).
pub const TERRAIN_WATER: u8 = 1;
/// Terrain tag: shallows — the amphibious terrain (W1 grid half):
/// passable to BOTH land units and ships. Pinned at Passo 0 from
/// WATER-SPEC §1: the AoE2 manual's Terrain list ("Shallows —
/// Water passable by land units and ships"), the wiki Ship article
/// lead (ships "can also pass shallow water and beach terrain"),
/// and the amphibious map articles (Bog Islands / Rivers / Salt
/// Marsh: shallows "traversed by both domains"). This supersedes
/// MAP-08's study-era "no wading" parenthetical; the rulebook
/// text amendment rides the complete-W1 commit, not this one.
pub const TERRAIN_SHALLOWS: u8 = 2;

/// The movement domain a path query runs for (W1 grid half). The
/// domain is a *parameter* of the passability/A* functions, never
/// a fork of them: one grid, one A*, two passability readings.
/// Land is the domain every pre-water caller means; Water is the
/// ships' domain (no ship kinds exist yet — W2 adds the first
/// consumer; the grid half ships the domain itself plus tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    Land,
    Water,
}
/// Highest elevation level the generator produces (MAP-05: the
/// reference's per-tile height byte has 7 levels; Skirmish's Arabia
/// hills use 0..=4 — see `mapgen`).
pub const MAX_HEIGHT: u8 = 4;
/// MAP-07: a height step of this many levels or more between two
/// cells is a *cliff* — a discontinuous elevation break that land
/// units can never cross, whatever the heights on either side.
/// Generator hills step by at most 2 between neighbors, so every
/// step of `CLIFF_STEP` or more is an authored cliff border (see
/// `mapgen::author_cliffs`); the sim derives cliffness from the
/// height layer alone, which is already hashed state, so a cliff
/// needs no layer of its own.
pub const CLIFF_STEP: u8 = 3;

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
    /// Per-cell terrain tag (`TERRAIN_LAND` / `TERRAIN_WATER` /
    /// `TERRAIN_SHALLOWS`), row-major. Set once by the generator;
    /// never changes in play.
    terrain: Vec<u8>,
    /// Per-cell elevation level, 0..=`MAX_HEIGHT`, row-major.
    /// MAP-06's combat factor reads it; gentle elevation never
    /// slows or blocks land units — only a discontinuous step
    /// (MAP-07's cliff, `CLIFF_STEP` or more) gates movement.
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
    /// (MAP-11). Passability is domain-relative (W1 grid half):
    ///
    /// - `Land`: blocked by deep water, a live tree (node block),
    ///   or a placed building (dynamic). Shallows are passable —
    ///   land units wade (WATER-SPEC §1).
    /// - `Water`: blocked by land or a placed building (a Dock
    ///   footprint blocks its water cells — ships do not sail
    ///   through Docks). The node-block layer never blocks water:
    ///   trees never stand on water, and fish nodes are economy
    ///   state, not grid blocks. Shallows and deep water are both
    ///   passable.
    pub fn is_blocked_for(&self, domain: Domain, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return true;
        }
        let i = Self::idx(x, y);
        match domain {
            Domain::Land => {
                self.terrain[i] == TERRAIN_WATER || self.node_block[i] != 0 || self.dynamic[i] != 0
            }
            Domain::Water => self.terrain[i] == TERRAIN_LAND || self.dynamic[i] != 0,
        }
    }

    /// Whether a cell is blocked for a land unit — the pre-water
    /// reading every existing caller means. Exactly
    /// `is_blocked_for(Domain::Land, …)`.
    pub fn is_blocked(&self, x: u32, y: u32) -> bool {
        self.is_blocked_for(Domain::Land, x, y)
    }

    /// Whether the cell is deep water. Out of bounds counts as
    /// deep water (the edge is a wall). Shallows are NOT deep
    /// water: `is_water` stays the deep-water test the land
    /// flood, the corner rule and map validation already mean.
    pub fn is_water(&self, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return true;
        }
        self.terrain[Self::idx(x, y)] == TERRAIN_WATER
    }

    /// Whether the cell is shallows — the amphibious tag both
    /// domains enter. Out of bounds is never shallows.
    pub fn is_shallows(&self, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return false;
        }
        self.terrain[Self::idx(x, y)] == TERRAIN_SHALLOWS
    }

    /// Whether the cell is on the water side of a shoreline:
    /// deep water or shallows. The Dock placement predicate and
    /// the shoreline helpers of the complete W1 read this; the
    /// grid half ships the predicate, not the Dock.
    pub fn is_water_side(&self, x: u32, y: u32) -> bool {
        self.is_water(x, y) || self.is_shallows(x, y)
    }

    /// The terrain tag of a possibly out-of-bounds neighbour
    /// position: out of bounds reads as deep water, exactly as
    /// `terrain_at` reads the edge (the wall at the world's edge).
    fn terrain_at_signed(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= GRID_DIM as i32 || y >= GRID_DIM as i32 {
            return TERRAIN_WATER;
        }
        self.terrain_at(x as u32, y as u32)
    }

    /// Whether cell `(x, y)` has an orthogonal (4-neighbour) cell
    /// whose terrain tag is exactly `tag` (out-of-bounds neighbours
    /// read as deep water, per `terrain_at_signed`).
    fn has_terrain_neighbor4(&self, x: u32, y: u32, tag: u8) -> bool {
        [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .any(|&(dx, dy)| self.terrain_at_signed(x as i32 + dx, y as i32 + dy) == tag)
    }

    /// Whether a cell stands on a shoreline (complete W1): a land
    /// cell orthogonally adjacent to a water-side cell, or a
    /// water-side cell orthogonally adjacent to land — the mirror
    /// pair in one predicate. Orthogonal adjacency is the grid's
    /// standing shoreline reading (the same 4-neighbourhood the
    /// mapgen flood walks); a merely diagonal touch is not a
    /// shore. A shallows cell beside land is a shore cell (it is
    /// water-side, and it touches land).
    pub fn is_shore(&self, x: u32, y: u32) -> bool {
        if x >= GRID_DIM || y >= GRID_DIM {
            return false;
        }
        if self.terrain_at(x, y) == TERRAIN_LAND {
            [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                // Out of bounds is water-side (the edge wall reads
                // as deep water), so an edge land cell is a shore.
                (nx < 0 || ny < 0 || nx >= GRID_DIM as i32 || ny >= GRID_DIM as i32)
                    || self.is_water_side(nx as u32, ny as u32)
            })
        } else if self.is_water_side(x, y) {
            self.has_terrain_neighbor4(x, y, TERRAIN_LAND)
        } else {
            false
        }
    }

    /// Whether a cell counts toward the LAND side of the Dock
    /// predicate (complete W1, plan §0.3): a land cell, or a
    /// shallows cell orthogonally adjacent to land ("land-adjacent
    /// shallows" — a Dock may stand wholly in the shallows ring
    /// beside a coast, per the wiki's "in shallow water or on
    /// shallows, next to the coast"). Deep water never counts.
    fn is_dock_land_side(&self, x: u32, y: u32) -> bool {
        self.terrain_at(x, y) == TERRAIN_LAND
            || (self.is_shallows(x, y) && self.has_terrain_neighbor4(x, y, TERRAIN_LAND))
    }

    /// The terrain half of the Dock placement predicate (complete
    /// W1, plan §0.3 — the pinned contract): a `size`×`size`
    /// footprint anchored at `(ax, ay)` straddles a shoreline iff
    /// at least one footprint cell is water-side (deep water or
    /// shallows) AND at least one footprint cell is land-side
    /// (land, or land-adjacent shallows — `is_dock_land_side`).
    /// A shallows cell beside land counts toward BOTH sides (it
    /// is amphibious ground beside a coast). All-water in open
    /// sea refuses (no land side); all-land refuses (no water
    /// side); a footprint whose only land-side cell is one corner
    /// accepts — the predicate is the contract, corners included.
    /// Terrain only: the obstruction half (nodes, buildings,
    /// cliffs) is `dock_placement_ok`; mapgen validation reads
    /// this half, so a rimmed pond's Dock site is a terrain fact
    /// that package placement on the shore ring cannot revoke.
    pub fn dock_terrain_ok(&self, ax: u32, ay: u32, size: u32) -> bool {
        if ax + size > GRID_DIM || ay + size > GRID_DIM {
            return false;
        }
        let mut water_side = false;
        let mut land_side = false;
        for dy in 0..size {
            for dx in 0..size {
                let (cx, cy) = (ax + dx, ay + dy);
                if self.is_water_side(cx, cy) {
                    water_side = true;
                }
                if self.is_dock_land_side(cx, cy) {
                    land_side = true;
                }
            }
        }
        water_side && land_side
    }

    /// The Dock placement predicate (complete W1, plan §3.1.3):
    /// the terrain straddle (`dock_terrain_ok`) AND every
    /// footprint cell unobstructed — no live tree (node block) and
    /// no building (dynamic layer) on any cell, water-side cells
    /// included (a Dock may not swallow a tree on its land corner,
    /// and ships' water is building-free by the same layer). This
    /// is the one implementation `Sim::validate_build`, the
    /// client's ghost-mirror data and mapgen's soak all read;
    /// `validate_build` additionally runs BLD-01's standing
    /// checks (Town Center reserve, standing units, live resource
    /// nodes, the building registry's overlap, price, reach).
    pub fn dock_placement_ok(&self, ax: u32, ay: u32, size: u32) -> bool {
        if !self.dock_terrain_ok(ax, ay, size) {
            return false;
        }
        for dy in 0..size {
            for dx in 0..size {
                let (cx, cy) = (ax + dx, ay + dy);
                if self.is_node_blocked(cx, cy) || self.is_dynamic(cx, cy) {
                    return false;
                }
            }
        }
        true
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
        self.nearest_free_cell_for(Domain::Land, x, y, max_ring)
    }

    /// `nearest_free_cell` for an explicit domain (W1 grid half):
    /// "free" reads through `is_blocked_for`, so a water-domain
    /// search lands on water-side cells, a land search on land.
    pub fn nearest_free_cell_for(
        &self,
        domain: Domain,
        x: u32,
        y: u32,
        max_ring: u32,
    ) -> Option<(u32, u32)> {
        if !self.is_blocked_for(domain, x, y) {
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
                    if !self.is_blocked_for(domain, nx as u32, ny as u32) {
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

    /// MAP-07: whether the border between two cells is a cliff —
    /// the height changes discontinuously across it (a step of
    /// `CLIFF_STEP` levels or more). Symmetric in its arguments;
    /// meaningful for any pair, used with adjacent ones.
    pub fn is_cliff_between(&self, ax: u32, ay: u32, bx: u32, by: u32) -> bool {
        self.height_at(ax, ay).abs_diff(self.height_at(bx, by)) >= CLIFF_STEP
    }

    /// MAP-07: whether a direct step from cell `a` to an adjacent
    /// cell `b` (orthogonal or diagonal) crosses a cliff border.
    /// An orthogonal step crosses exactly when its own border is a
    /// cliff. A diagonal step is barred when the diagonal pair is
    /// itself a cliff or when any of the four side borders around
    /// the shared corner is one — the corner of a cliff face is
    /// still the face, so a path may not slip diagonally around
    /// the end cell of a cliff line.
    pub fn move_crosses_cliff(&self, a: (u32, u32), b: (u32, u32)) -> bool {
        if a == b {
            return false;
        }
        if self.is_cliff_between(a.0, a.1, b.0, b.1) {
            return true;
        }
        let dx = b.0 as i32 - a.0 as i32;
        let dy = b.1 as i32 - a.1 as i32;
        if dx != 0 && dy != 0 {
            let o1 = ((a.0 as i32 + dx) as u32, a.1);
            let o2 = (a.0, (a.1 as i32 + dy) as u32);
            return self.is_cliff_between(a.0, a.1, o1.0, o1.1)
                || self.is_cliff_between(a.0, a.1, o2.0, o2.1)
                || self.is_cliff_between(b.0, b.1, o1.0, o1.1)
                || self.is_cliff_between(b.0, b.1, o2.0, o2.1);
        }
        false
    }

    /// MAP-07: whether the straight segment between two world
    /// points crosses any cliff border. Sampled at one-world-unit
    /// steps (a quarter cell), so consecutive samples sit in the
    /// same or an adjacent cell and every border the segment
    /// crosses is tested with `move_crosses_cliff` — the same
    /// predicate pathing obeys. Fixed-point throughout, so the
    /// answer is identical on every platform. CMB-01's
    /// across-cliff default reads this (a shot or strike whose
    /// line crosses a cliff face).
    pub fn segment_crosses_cliff(&self, from: (I16F16, I16F16), to: (I16F16, I16F16)) -> bool {
        let mut prev = self.world_to_cell(from.0, from.1);
        if prev == self.world_to_cell(to.0, to.1) {
            return false;
        }
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        // One step per world unit of the dominant axis: each step
        // moves at most one unit per axis (a quarter cell), so no
        // cell border can be skipped over.
        let steps = dx.abs().max(dy.abs()).to_num::<i32>().max(1);
        let step_x = dx / I16F16::from_num(steps);
        let step_y = dy / I16F16::from_num(steps);
        for i in 1..=steps {
            let p = (
                from.0 + step_x * I16F16::from_num(i),
                from.1 + step_y * I16F16::from_num(i),
            );
            let cell = self.world_to_cell(p.0, p.1);
            if cell != prev {
                if self.move_crosses_cliff(prev, cell) {
                    return true;
                }
                prev = cell;
            }
        }
        false
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

    /// Statically impassable cells as row-major bytes (0/1): deep
    /// water or a live tree — the LAND domain's static view.
    /// Shallows are land-passable, so they are not obstacles here.
    /// This is the WASM `obstacles()` view the client mirrors for
    /// its placement ghost — terrain truth, without the building
    /// layer (the client reads buildings separately).
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
        self.find_path_through(from, to, &[])
    }

    /// A* from one world point to another for an explicit domain
    /// (W1 grid half). `find_path` is this with `Domain::Land`.
    pub fn find_path_for(
        &self,
        domain: Domain,
        from: (I16F16, I16F16),
        to: (I16F16, I16F16),
    ) -> Option<Vec<(I16F16, I16F16)>> {
        self.find_path_through_for(domain, from, to, &[])
    }

    /// A* from one world point to another, treating the cells of
    /// `passable` as unblocked wherever the grid's layers say
    /// blocked (Package J, BLD-13: the querying unit's OWN gate
    /// cells — a gate is conditionally blocked per querying
    /// player, so the owner's path runs through its span: the
    /// gate stands open for the owner's approach, and the derived
    /// open state lifts the dynamic block as the unit nears).
    /// Every other cell obeys the ordinary layers, and a
    /// `passable` cell that is water or a tree stays blocked —
    /// the exemption lifts only the building (dynamic) layer.
    pub fn find_path_through(
        &self,
        from: (I16F16, I16F16),
        to: (I16F16, I16F16),
        passable: &[(u32, u32)],
    ) -> Option<Vec<(I16F16, I16F16)>> {
        self.find_path_through_for(Domain::Land, from, to, passable)
    }

    /// `find_path_through` for an explicit domain (W1 grid half).
    /// The `passable` gate exemption is a land-domain mechanism
    /// (Package J): on the water domain it is ignored — ships own
    /// no gates, and a water A* must not inherit land's gate-free
    /// cell logic.
    pub fn find_path_through_for(
        &self,
        domain: Domain,
        from: (I16F16, I16F16),
        to: (I16F16, I16F16),
        passable: &[(u32, u32)],
    ) -> Option<Vec<(I16F16, I16F16)>> {
        let start = self.world_to_cell(from.0, from.1);
        let goal = self.world_to_cell(to.0, to.1);
        let goal_exempt = domain == Domain::Land && passable.contains(&goal);
        if self.is_blocked_for(domain, goal.0, goal.1) && !goal_exempt {
            return None;
        }
        let cells = self.astar_cells(domain, start, goal, passable)?;
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
    /// `passable` cells (Package J: the querier's own gates) count
    /// as unblocked for the building layer only — see
    /// `find_path_through`. The domain selects the passability
    /// reading (`is_blocked_for`); the land-only special cases
    /// inside (gate exemption, cliff seal, corner rule) are gated
    /// on `Domain::Land` explicitly, so a water A* inherits none
    /// of them — its corner rule is the mirror image (land or a
    /// building on either orthogonal neighbor bars the diagonal).
    fn astar_cells(
        &self,
        domain: Domain,
        start: (u32, u32),
        goal: (u32, u32),
        passable: &[(u32, u32)],
    ) -> Option<Vec<(u32, u32)>> {
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
                // Package J: the querier's own gate cells are
                // passable to it (they lift their building block
                // for the owner's approach) — every other layer
                // still applies to them. Land domain only (W1):
                // ships own no gates.
                let gate_free = domain == Domain::Land
                    && passable.contains(&(nx, ny))
                    && !self.is_water(nx, ny)
                    && !self.is_node_blocked(nx, ny);
                if self.is_blocked_for(domain, nx, ny) && !gate_free {
                    continue;
                }
                // MAP-07: a cliff border is impassable — the step
                // may not cross one, diagonally included (the
                // corner rule inside `move_crosses_cliff`). Land
                // domain only (W1): elevation is a land layer;
                // water movement never reads it.
                if domain == Domain::Land && self.move_crosses_cliff((cx, cy), (nx, ny)) {
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
                // Package J: the querier's own gate cell is not a
                // hard corner for it either (same exemption).
                if dx != 0 && dy != 0 {
                    // The corner rule is domain-mirrored (W1): for
                    // land a HARD corner is deep water or a building;
                    // for water it is land or a building. Shallows
                    // are never a hard corner (both domains enter
                    // them), and a tree never is one on either
                    // domain (land: the M7 trunk rule above; water:
                    // the node layer does not exist there).
                    let hard = |x: u32, y: u32| match domain {
                        Domain::Land => {
                            (self.is_water(x, y) || self.is_dynamic(x, y))
                                && !passable.contains(&(x, y))
                        }
                        Domain::Water => {
                            self.terrain_at(x, y) == TERRAIN_LAND || self.is_dynamic(x, y)
                        }
                    };
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

    /// A hand-built cliff shelf: row 40 at height 4, row 41 at
    /// height 1 (a step of 3 across the whole map width), gentle
    /// ramps elsewhere. MAP-07's predicates and pathing seals.
    fn cliff_grid() -> Grid {
        let mut g = Grid::new();
        for x in 0..GRID_DIM {
            g.set_height(x, 40, 4);
            g.set_height(x, 41, 1);
        }
        g
    }

    #[test]
    fn cliff_predicates_match_map_07() {
        let g = cliff_grid();
        // The 4|1 border is a cliff; gentle borders are not.
        assert!(g.is_cliff_between(60, 40, 60, 41));
        assert!(g.is_cliff_between(60, 41, 60, 40), "symmetric");
        assert!(!g.is_cliff_between(60, 41, 60, 42));
        // The fixture's default-0 row against the height-4 shelf
        // is a step of 4 — also a cliff.
        assert!(g.is_cliff_between(60, 39, 60, 40));
        // A step of exactly 2 never is; exactly 3 always is.
        let mut g2 = Grid::new();
        g2.set_height(10, 10, 2);
        assert!(!g2.is_cliff_between(10, 10, 11, 10));
        g2.set_height(10, 10, 3);
        assert!(g2.is_cliff_between(10, 10, 11, 10));
        // Orthogonal crossing predicate.
        assert!(g.move_crosses_cliff((60, 40), (60, 41)));
        assert!(!g.move_crosses_cliff((60, 41), (60, 42)));
        // Diagonal around the cliff corner: from the plateau cell
        // (60,40) diagonally to (61,41) — the side border into
        // (60,41) is the face, so the diagonal is barred.
        assert!(g.move_crosses_cliff((60, 40), (61, 41)));
        assert!(g.move_crosses_cliff((61, 41), (60, 40)), "both directions");
        // A diagonal wholly on gentle ground is free.
        assert!(!g.move_crosses_cliff((60, 41), (61, 42)));
    }

    #[test]
    fn astar_never_crosses_a_cliff() {
        let g = cliff_grid();
        // The shelf spans the whole width: no path from below to
        // above exists at all.
        let from = g.cell_center(60, 60);
        let to = g.cell_center(60, 30);
        assert!(g.find_path(from, to).is_none(), "cliff must seal the shelf");
        // Same-side travel is unaffected.
        let along = g.find_path(g.cell_center(20, 60), g.cell_center(100, 60));
        assert!(along.is_some());
        // A path right up to the face stops on its own side: order
        // the cell just across the border; A* must refuse (the
        // goal cell itself is open land — only the border seals).
        assert!(g.find_path(g.cell_center(60, 41), g.cell_center(60, 40)).is_none());
    }

    #[test]
    fn segment_crosses_cliff_detection() {
        let g = cliff_grid();
        let below = g.cell_center(60, 45);
        let above = g.cell_center(60, 35);
        assert!(g.segment_crosses_cliff(below, above));
        assert!(g.segment_crosses_cliff(above, below), "symmetric");
        // A segment running along one side never crosses.
        assert!(!g.segment_crosses_cliff(g.cell_center(20, 45), g.cell_center(100, 45)));
        // A segment inside one cell never crosses.
        let c = g.cell_center(60, 45);
        assert!(!g.segment_crosses_cliff(c, c));
    }

    // ---- W1 grid half: domains + shallows (MOV-11) ----

    #[test]
    fn domain_blocked_matrix_covers_shallows() {
        let mut g = Grid::new();
        // One probe cell per case, all on row 60.
        g.set_terrain(10, 60, TERRAIN_WATER); // deep water
        g.set_terrain(11, 60, TERRAIN_SHALLOWS); // shallows
        // (12,60) stays plain land.
        g.set_terrain(13, 60, TERRAIN_WATER);
        g.set_node_block(13, 60, true); // node flag on a water cell
        g.set_terrain(14, 60, TERRAIN_SHALLOWS);
        g.set_dynamic(14, 60, true); // building (Dock proxy) on shallows
        g.set_node_block(15, 60, true); // tree on land

        // Land domain: refuses deep water, crosses shallows.
        assert!(g.is_blocked_for(Domain::Land, 10, 60), "land in deep water");
        assert!(!g.is_blocked_for(Domain::Land, 11, 60), "land wades shallows");
        assert!(!g.is_blocked_for(Domain::Land, 12, 60), "land on land");
        assert!(g.is_blocked_for(Domain::Land, 13, 60), "land: water+node");
        assert!(g.is_blocked_for(Domain::Land, 14, 60), "land: building blocks");
        assert!(g.is_blocked_for(Domain::Land, 15, 60), "land: tree blocks");
        // Water domain: refuses land, crosses shallows and deep water.
        assert!(!g.is_blocked_for(Domain::Water, 10, 60), "ship in deep water");
        assert!(!g.is_blocked_for(Domain::Water, 11, 60), "ship in shallows");
        assert!(g.is_blocked_for(Domain::Water, 12, 60), "ship on land");
        assert!(
            !g.is_blocked_for(Domain::Water, 13, 60),
            "node layer never blocks water (fish are econ state)"
        );
        assert!(g.is_blocked_for(Domain::Water, 14, 60), "ship: building blocks");
        assert!(g.is_blocked_for(Domain::Water, 15, 60), "ship: land blocks");
        // The legacy reading is exactly the land domain, everywhere.
        for &(x, y) in &[(10, 60), (11, 60), (12, 60), (13, 60), (14, 60), (15, 60)] {
            assert_eq!(g.is_blocked(x, y), g.is_blocked_for(Domain::Land, x, y));
        }
        // Shallows predicates: shallows are water-side, never deep water.
        assert!(g.is_shallows(11, 60));
        assert!(!g.is_water(11, 60));
        assert!(g.is_water_side(11, 60));
        assert!(g.is_water_side(10, 60));
        assert!(!g.is_water_side(12, 60));
        // Out of bounds walls both domains.
        assert!(g.is_blocked_for(Domain::Land, 120, 0));
        assert!(g.is_blocked_for(Domain::Water, 120, 0));
    }

    #[test]
    fn land_astar_crosses_shallows_but_not_deep_water() {
        let mut g = Grid::new();
        // A deep-water column at x = 60, rows 20..=99, whose only
        // crossing is a shallows ford at rows 58..=60.
        for y in 20..=99u32 {
            g.set_terrain(60, y, TERRAIN_WATER);
        }
        for y in 58..=60u32 {
            g.set_terrain(60, y, TERRAIN_SHALLOWS);
        }
        let from = g.cell_center(30, 59);
        let to = g.cell_center(90, 59);
        let path = g
            .find_path_for(Domain::Land, from, to)
            .expect("land must cross at the shallows ford");
        let mut crossed_shallows = false;
        for &(wx, wy) in &path[..path.len() - 1] {
            let (cx, cy) = g.world_to_cell(wx, wy);
            assert!(!g.is_water(cx, cy), "land waypoint in deep water");
            assert!(
                !g.is_blocked_for(Domain::Land, cx, cy),
                "land waypoint on a blocked cell"
            );
            if g.is_shallows(cx, cy) {
                crossed_shallows = true;
            }
        }
        assert!(crossed_shallows, "the path must use the ford");
        // The legacy entry point agrees bit-for-bit on land queries.
        assert_eq!(g.find_path(from, to), g.find_path_for(Domain::Land, from, to));
        // Seal the ford and extend the column across the whole
        // map height: deep water everywhere — no land path at all.
        for y in 0..GRID_DIM {
            g.set_terrain(60, y, TERRAIN_WATER);
        }
        assert!(g.find_path_for(Domain::Land, from, to).is_none());
        assert!(g.find_path(from, to).is_none());
    }

    #[test]
    fn water_astar_crosses_shallows_and_deep_but_not_land() {
        let mut g = Grid::new();
        // Two deep-water pools joined by a shallows channel, in a
        // land sea: pool A x 30..=40, channel x 41..=43, pool B
        // x 44..=54, all rows 55..=65.
        for y in 55..=65u32 {
            for x in 30..=40u32 {
                g.set_terrain(x, y, TERRAIN_WATER);
            }
            for x in 41..=43u32 {
                g.set_terrain(x, y, TERRAIN_SHALLOWS);
            }
            for x in 44..=54u32 {
                g.set_terrain(x, y, TERRAIN_WATER);
            }
        }
        let from = g.cell_center(35, 60);
        let to = g.cell_center(49, 60);
        let path = g
            .find_path_for(Domain::Water, from, to)
            .expect("a ship must sail pool to pool through the shallows");
        let mut crossed_shallows = false;
        for &(wx, wy) in &path[..path.len() - 1] {
            let (cx, cy) = g.world_to_cell(wx, wy);
            assert!(
                !g.is_blocked_for(Domain::Water, cx, cy),
                "ship waypoint on a blocked cell"
            );
            assert!(
                g.terrain_at(cx, cy) != TERRAIN_LAND,
                "ship waypoint on land"
            );
            if g.is_shallows(cx, cy) {
                crossed_shallows = true;
            }
        }
        assert!(crossed_shallows, "the sailing must use the channel");
        // A ship ordered onto land finds no path (the goal itself
        // is land-blocked for the water domain).
        let ashore = g.cell_center(70, 60);
        assert!(g.find_path_for(Domain::Water, from, ashore).is_none());
        // Mirror pin: a land unit ordered into the pool's deep
        // water finds no path either.
        let bank = g.cell_center(20, 60);
        assert!(g.find_path_for(Domain::Land, bank, from).is_none());
        assert!(g.find_path(bank, from).is_none());
    }

    #[test]
    fn water_astar_respects_buildings_corners_and_ignores_land_layers() {
        // Buildings block water: a dynamic wall across a channel
        // with a one-cell gap sails through the gap; sealed, none.
        let mut g = Grid::new();
        for y in 40..=80u32 {
            for x in 55..=65u32 {
                g.set_terrain(x, y, TERRAIN_WATER);
            }
        }
        for y in 40..=80u32 {
            if y != 60 {
                g.set_dynamic(60, y, true);
            }
        }
        let west = g.cell_center(57, 60);
        let east = g.cell_center(63, 60);
        let path = g
            .find_path_for(Domain::Water, west, east)
            .expect("a ship must sail the gap in the building wall");
        for &(wx, wy) in &path[..path.len() - 1] {
            let (cx, cy) = g.world_to_cell(wx, wy);
            assert!(!g.is_dynamic(cx, cy), "ship waypoint inside a building");
        }
        g.set_dynamic(60, 60, true);
        assert!(g.find_path_for(Domain::Water, west, east).is_none());

        // Corner rule on water: a diagonal pinched between two
        // land cells is refused — with no other water around, the
        // pinched goal is unreachable.
        let mut g2 = Grid::new();
        g2.set_terrain(50, 50, TERRAIN_WATER);
        g2.set_terrain(51, 51, TERRAIN_WATER);
        // (51,50) and (50,51) stay land: both orthogonal neighbors.
        assert!(
            g2.find_path_for(Domain::Water, g2.cell_center(50, 50), g2.cell_center(51, 51))
                .is_none(),
            "a ship must not cut the land corner diagonally"
        );
        // Free one orthogonal neighbor to shallows: the diagonal opens.
        g2.set_terrain(51, 50, TERRAIN_SHALLOWS);
        assert!(
            g2.find_path_for(Domain::Water, g2.cell_center(50, 50), g2.cell_center(51, 51))
                .is_some(),
            "shallows are not a hard corner for ships"
        );

        // Cliffs are inert on water: a water column whose heights
        // step by 4 across the middle (a cliff for land) sails
        // straight through — elevation is a land layer.
        let mut g3 = Grid::new();
        for y in 30..=50u32 {
            g3.set_terrain(60, y, TERRAIN_WATER);
            if y <= 40 {
                g3.set_height(60, y, 4);
            }
        }
        assert!(g3.is_cliff_between(60, 40, 60, 41), "fixture is a cliff");
        assert!(
            g3.find_path_for(Domain::Water, g3.cell_center(60, 45), g3.cell_center(60, 35))
                .is_some(),
            "a cliff must not seal water movement"
        );
        // Node flags are inert on water: a fish-flagged cell (node
        // block on deep water) is sailed over, not routed around.
        let mut g4 = Grid::new();
        for y in 55..=65u32 {
            for x in 30..=50u32 {
                g4.set_terrain(x, y, TERRAIN_WATER);
            }
        }
        g4.set_node_block(40, 60, true);
        let straight = g4
            .find_path_for(Domain::Water, g4.cell_center(35, 60), g4.cell_center(45, 60))
            .expect("a ship sails over a fish cell");
        assert!(
            straight
                .iter()
                .any(|&p| g4.world_to_cell(p.0, p.1) == (40, 60)),
            "the ship must pass over the node-flagged cell"
        );
    }

    // ---- Complete W1: shoreline + Dock predicate (BLD-14) ----

    /// A pond fixture: deep-water 3×3 core at (50..=52, 50..=52)
    /// ringed by shallows, in open land.
    fn pond_grid() -> Grid {
        let mut g = Grid::new();
        for y in 49..=53u32 {
            for x in 49..=53u32 {
                g.set_terrain(x, y, TERRAIN_SHALLOWS);
            }
        }
        for y in 50..=52u32 {
            for x in 50..=52u32 {
                g.set_terrain(x, y, TERRAIN_WATER);
            }
        }
        g
    }

    #[test]
    fn shore_predicate_mirrors_both_sides() {
        let g = pond_grid();
        // Land beside shallows is shore; the shallows ring beside
        // land is shore; the deep core (no land neighbour) is not;
        // open land far away is not.
        assert!(g.is_shore(48, 51), "land beside the shallows ring");
        assert!(g.is_shore(49, 51), "shallows beside land");
        assert!(!g.is_shore(51, 51), "deep core touches no land");
        assert!(!g.is_shore(30, 30), "open land is not a shore");
        // A diagonal-only touch is not a shore (orthogonal rule):
        // (48,48) touches the ring's corner cell (49,49) diagonally.
        assert!(!g.is_shore(48, 48), "diagonal touch is not a shore");
    }

    #[test]
    fn dock_predicate_straddles_and_refuses_the_pure_sides() {
        let g = pond_grid();
        // Straddling the rim: land + shallows + deep in one 3×3.
        assert!(g.dock_placement_ok(48, 50, 3), "rim straddle accepts");
        // Corner-land case (plan §0.3: the predicate is the
        // contract): on a quadrant coast (land only for x ≤ 49 and
        // y ≤ 49, deep water beyond), the 3×3 anchored at (49,49)
        // has exactly ONE land cell — its corner — and accepts.
        let mut quad = Grid::new();
        for y in 0..GRID_DIM {
            for x in 0..GRID_DIM {
                if x > 49 || y > 49 {
                    quad.set_terrain(x, y, TERRAIN_WATER);
                }
            }
        }
        assert!(quad.dock_placement_ok(49, 49, 3), "corner-land accepts");
        assert!(!quad.dock_placement_ok(50, 50, 3), "past the corner: no land side");
        // All-water in the pond's deep core area refuses (no land
        // side): a 3×3 of pure deep water.
        let mut sea = Grid::new();
        for y in 40..=60u32 {
            for x in 40..=60u32 {
                sea.set_terrain(x, y, TERRAIN_WATER);
            }
        }
        assert!(!sea.dock_placement_ok(50, 50, 3), "open sea refuses");
        // All-land refuses (no water side).
        assert!(!g.dock_placement_ok(20, 20, 3), "dry land refuses");
        // A footprint of shallows + deep water whose shallows
        // touch land accepts with NO land cell at all: the
        // shallows cells adjacent to land are land-side (the
        // wiki's "on shallows, next to the coast" form). On the
        // pond fixture, anchor (49,49) covers only the shallows
        // ring and the deep core.
        assert!(g.dock_placement_ok(49, 49, 3), "rim-only footprint beside land accepts");
        let rim_only = {
            let mut g2 = Grid::new();
            // A shallows strip x 60..=62 beside land on both sides.
            for y in 55..=65u32 {
                for x in 60..=62u32 {
                    g2.set_terrain(x, y, TERRAIN_SHALLOWS);
                }
            }
            g2
        };
        assert!(
            rim_only.dock_placement_ok(60, 58, 3),
            "a 3x3 wholly in coast-adjacent shallows accepts"
        );
        // A shallows patch adrift in a deep-water sea (no land
        // within reach of any footprint cell) refuses: its
        // shallows are water-side only, never land-side.
        let mut float = Grid::new();
        for y in 50..=70u32 {
            for x in 55..=70u32 {
                float.set_terrain(x, y, TERRAIN_WATER);
            }
        }
        for y in 58..=60u32 {
            for x in 60..=62u32 {
                float.set_terrain(x, y, TERRAIN_SHALLOWS);
            }
        }
        assert!(
            !float.dock_placement_ok(60, 58, 3),
            "shallows adrift in deep water (no land adjacency) refuse"
        );
    }

    #[test]
    fn dock_predicate_refuses_obstructed_cells() {
        let mut g = pond_grid();
        // A tree on the land corner of a straddling footprint.
        g.set_node_block(48, 50, true);
        assert!(!g.dock_placement_ok(48, 50, 3), "tree on the footprint refuses");
        assert!(g.dock_terrain_ok(48, 50, 3), "the terrain half still reads true");
        g.set_node_block(48, 50, false);
        // A building on a shallows cell refuses too.
        g.set_dynamic(49, 51, true);
        assert!(!g.dock_placement_ok(48, 50, 3), "building on the footprint refuses");
        // Out-of-bounds anchors refuse.
        assert!(!g.dock_placement_ok(118, 50, 3));
        assert!(!g.dock_placement_ok(50, 118, 3));
    }
}
