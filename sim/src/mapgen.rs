//! Random map generation (M7) — the deterministic Arabia generator.
//!
//! Every match (Sandbox and AI modes alike) is played on a map this
//! module generates from the match seed, inside match creation, so
//! the map is ordinary hashed state (MAP-09): same seed ⇒
//! byte-identical map on every machine; different seeds ⇒ different
//! maps. Generation is integer-only (the in-sim PCG32), never floats
//! — float math is not bit-identical across platforms.
//!
//! The map is the rulebook's 2-player size — Tiny, 120×120 tiles
//! (MAP-02) — at the sim's scale of one grid cell = one tile. Two
//! starts sit opposite each other (180° rotational symmetry, the
//! Arabia convention): start B's package is start A's package
//! mirrored through the map center, so both starts are *exactly*
//! fair — equal resource counts at mirrored distances — while the
//! seed varies everything match to match. Terrain and heights are
//! symmetrized the same way.
//!
//! Per start, the OPENING-SPEC §2.3 package (bands in tiles from the
//! Town Center): 3 villagers + Scout beside the TC (spawned by
//! `Sim`, as always); 6 berry bushes at 5–8; 4 near sheep at 4–8 and
//! 2+2 far sheep at 10–16; 3 straggler trees at 4–8; a main woodline
//! (forest edge 10–15) and a second woodline; 7 main gold at 10–16
//! with two 4-tile secondaries farther out (one behind the main,
//! one at a split anchor — §2.3's "Back golds: 2 × 4"); 5 main
//! stone at 10–16 with 4 behind; 2
//! boars at 15–24 in split directions; deer herds of 3 at 12–18 and
//! 3 more at 20–28; one wolf pack of 3 at 26–34 (AI-08 — outside
//! every starting band, so no pack ever spawns beside a Town
//! Center). Heights are gentle value-noise hills, flattened
//! around each start and its packages (disc + per-node flattening),
//! so openings play identically on every seed. The generator also
//! places the map's five relics (MAP-13, Castle P5d): two mirrored
//! pairs plus one center relic at exactly equal Chebyshev distance
//! from both Town Centers, every relic at least 20 tiles from both
//! starts — the sorted distance multiset to start A equals the one
//! to start B, so the relic race is exactly fair like the starts. Water exists in the
//! vocabulary as small ponds (0–2 mirrored pairs), always ≥ 22
//! tiles from a start (MAP-08 terrain support; fishing is trimmed —
//! see ARCHITECTURE.md's M7 section). Complete W1 (2026-11-05):
//! every pond's outermost water ring — the water cells
//! orthogonally adjacent to land — becomes SHALLOWS, the
//! amphibious terrain both domains enter (MAP-08 as amended;
//! see `rim_ponds`), and validation requires every pond to
//! carry that rim and at least one terrain-legal Dock site
//! (BLD-14).
//!
//! Generation *validates* deterministically before a map is accepted
//! (see `validate`): a land path connects the two starts, every
//! package node and animal stands on land reachable from its start,
//! floods prove no start is enclosed, and no water touches a
//! start's ground. Heights are gentle (steps of at most 2) except
//! for one authored cliff line and its mirror (MAP-07, see
//! `author_cliffs`), whose sheer face the flood respects exactly
//! as A* does. A
//! failed candidate is discarded and the next deterministic
//! sub-seed (seed ⊕ attempt) generates the next; after
//! `MAX_ATTEMPTS` failures a fixed hand-authored template — built
//! to pass by construction — is used instead. Generation therefore
//! always terminates with a playable map, never a hang.

use crate::econ::NodeKind;
use crate::entity::EntityKind;
use crate::grid::{Grid, GRID_DIM, MAX_HEIGHT, TERRAIN_LAND, TERRAIN_SHALLOWS, TERRAIN_WATER};
use crate::rng::Pcg32;

/// Bounded regeneration attempts before the fallback template.
const MAX_ATTEMPTS: u32 = 64;
/// Placement margin: nothing generates within this many cells of
/// the world edge.
const MARGIN: i32 = 3;
/// Minimum land distance from a Town Center to any water cell.
const WATER_START_CLEARANCE: i32 = 22;

/// One player's start package: its Town Center cell and the
/// resources/animals generated for it, in deterministic order
/// (nodes: stragglers, forests, berries, gold, stone; animals:
/// sheep, boars, deer, wolves — `Sim` spawns the human's animals in
/// this order, preserving the historical entity-id layout; the
/// wolf pack appends after the deer, so pre-AI-08 ids are stable).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartPackage {
    pub tc_cell: (u32, u32),
    pub nodes: Vec<(NodeKind, u32, u32)>,
    pub animals: Vec<(EntityKind, u32, u32)>,
}

/// A generated map: the filled grid (terrain + heights + tree
/// blocks), both starts' packages, and neutral dressing nodes
/// (the mirrored mid-map forests — they belong to neither start).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedMap {
    pub grid: Grid,
    pub starts: [StartPackage; 2],
    pub neutral_nodes: Vec<(NodeKind, u32, u32)>,
    /// MAP-13 (Castle P5d): the five relic cells, in placement
    /// order (pair, pair, center). `Sim` spawns a Gaia relic
    /// entity on each at match creation.
    pub relics: Vec<(u32, u32)>,
}

fn idx(x: u32, y: u32) -> usize {
    (y * GRID_DIM + x) as usize
}

/// Rim every pond in a terrain field with shallows (complete W1):
/// each deep-water cell orthogonally adjacent to land becomes
/// `TERRAIN_SHALLOWS` — the pond's outermost ring, the amphibious
/// ground of MAP-08. Orthogonal adjacency is the shoreline
/// reading the grid's predicates use (`Grid::is_shore`); a
/// diagonal-only touch does not rim a cell, so even the smallest
/// pond (a radius-1 plus) keeps a deep core. The pass is a pure
/// function of the pre-pass field: the conversion test reads
/// LAND only, and a converted cell is no longer deep water, so
/// in-place application cannot cascade inward.
fn rim_ponds(terrain: &mut [u8]) {
    let mut rim: Vec<usize> = Vec::new();
    for y in 0..GRID_DIM {
        for x in 0..GRID_DIM {
            let i = idx(x, y);
            if terrain[i] != TERRAIN_WATER {
                continue;
            }
            let beside_land = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    nx >= 0
                        && ny >= 0
                        && nx < GRID_DIM as i32
                        && ny < GRID_DIM as i32
                        && terrain[idx(nx as u32, ny as u32)] == TERRAIN_LAND
                });
            if beside_land {
                rim.push(i);
            }
        }
    }
    for i in rim {
        terrain[i] = TERRAIN_SHALLOWS;
    }
}

/// Water W2a (ECO-15): the per-pond fish caps. An implementation
/// constant, recorded here — Arabia carries no documented DE
/// fish layout (WATER-SPEC §8.10: fish counts per pond are not
/// in the sources consulted), so the roster fields a bounded,
/// symmetric spread: up to 4 shore spots and 4 deep spots per
/// pond.
pub const MAX_SHORE_FISH_PER_POND: usize = 4;
pub const MAX_DEEP_FISH_PER_POND: usize = 4;

/// Water W2a (ECO-15): fish placement over a finished, rimmed
/// terrain grid — a pure function of the terrain (no RNG), so
/// dry seeds keep their hashes and pond seeds gain fish
/// deterministically. Ponds are the 4-connected water-region
/// components (shallows included); they are processed in
/// mirror pairs: the selection runs on one half and mirrors
/// through the map's own symmetry (`rot`), so both players'
/// ponds stock identically — the fairness discipline every
/// mirrored element of this generator already obeys. Per pond:
/// shore spots on shallows cells orthogonally adjacent to deep
/// water (the fisherman's bank and the ship's shallow ground in
/// one cell), deep spots on deep cells with no land
/// orthogonally adjacent (open water), each kind thinned to
/// its cap by an even stride over the pond's row-major order.
fn fish_placement(terrain: &[u8]) -> Vec<(NodeKind, u32, u32)> {
    let d = GRID_DIM as usize;
    let at = |x: usize, y: usize| terrain[y * d + x];
    let is_water = |t: u8| t == TERRAIN_WATER || t == TERRAIN_SHALLOWS;
    // 4-connected water components, discovered in scan order.
    let mut comp_of = vec![u32::MAX; d * d];
    let mut comps: Vec<Vec<(u32, u32)>> = Vec::new();
    for y in 0..d {
        for x in 0..d {
            if !is_water(at(x, y)) || comp_of[y * d + x] != u32::MAX {
                continue;
            }
            let id = comps.len() as u32;
            let mut cells = Vec::new();
            let mut stack = vec![(x, y)];
            comp_of[y * d + x] = id;
            while let Some((cx, cy)) = stack.pop() {
                cells.push((cx as u32, cy as u32));
                for (nx, ny) in [
                    (cx.wrapping_sub(1), cy),
                    (cx + 1, cy),
                    (cx, cy.wrapping_sub(1)),
                    (cx, cy + 1),
                ] {
                    if nx >= d || ny >= d || comp_of[ny * d + nx] != u32::MAX {
                        continue;
                    }
                    if is_water(at(nx, ny)) {
                        comp_of[ny * d + nx] = id;
                        stack.push((nx, ny));
                    }
                }
            }
            cells.sort_unstable();
            comps.push(cells);
        }
    }
    let mut out: Vec<(NodeKind, u32, u32)> = Vec::new();
    let mut done = vec![false; comps.len()];
    for ci in 0..comps.len() {
        if done[ci] {
            continue;
        }
        done[ci] = true;
        // The component's mirror twin (ponds are placed mirrored;
        // a self-mirrored component is its own twin).
        let twin_sorted: Vec<(u32, u32)> = {
            let mut t: Vec<(u32, u32)> = comps[ci].iter().map(|&(x, y)| rot(x, y)).collect();
            t.sort_unstable();
            t
        };
        for cj in 0..comps.len() {
            if !done[cj] && comps[cj] == twin_sorted {
                done[cj] = true;
                break;
            }
        }
        let select = |cells: &[(u32, u32)]| -> Vec<(NodeKind, u32, u32)> {
            let mut shore: Vec<(u32, u32)> = Vec::new();
            let mut deep: Vec<(u32, u32)> = Vec::new();
            for &(x, y) in cells {
                let (xu, yu) = (x as usize, y as usize);
                let adj: [(usize, usize); 4] = [
                    (xu.wrapping_sub(1), yu),
                    (xu + 1, yu),
                    (xu, yu.wrapping_sub(1)),
                    (xu, yu + 1),
                ];
                let adj_land = adj
                    .iter()
                    .any(|&(nx, ny)| nx < d && ny < d && !is_water(at(nx, ny)));
                let adj_deep = adj
                    .iter()
                    .any(|&(nx, ny)| nx < d && ny < d && at(nx, ny) == TERRAIN_WATER);
                if at(xu, yu) == TERRAIN_SHALLOWS && adj_deep {
                    shore.push((x, y));
                }
                if at(xu, yu) == TERRAIN_WATER && !adj_land {
                    deep.push((x, y));
                }
            }
            let thin = |v: &[(u32, u32)], cap: usize| -> Vec<(u32, u32)> {
                if v.len() <= cap {
                    return v.to_vec();
                }
                // Even stride over the row-major order: take
                // indices floor(i * len / cap), i in 0..cap.
                (0..cap).map(|i| v[i * v.len() / cap]).collect()
            };
            let mut picked: Vec<(NodeKind, u32, u32)> = thin(&shore, MAX_SHORE_FISH_PER_POND)
                .into_iter()
                .map(|(x, y)| (NodeKind::ShoreFish, x, y))
                .collect();
            picked.extend(
                thin(&deep, MAX_DEEP_FISH_PER_POND)
                    .into_iter()
                    .map(|(x, y)| (NodeKind::DeepFish, x, y)),
            );
            picked
        };
        let picked = select(&comps[ci]);
        for &(kind, x, y) in &picked {
            out.push((kind, x, y));
            let (mx, my) = rot(x, y);
            if (mx, my) != (x, y) {
                out.push((kind, mx, my));
            }
        }
    }
    out.sort_by_key(|&(kind, x, y)| (kind.tag(), x, y));
    out.dedup();
    out
}

/// The 180° rotational mirror of a cell through the map center.
fn rot(x: u32, y: u32) -> (u32, u32) {
    (GRID_DIM - 1 - x, GRID_DIM - 1 - y)
}

fn chebyshev(ax: u32, ay: u32, bx: u32, by: u32) -> u32 {
    ax.abs_diff(bx).max(ay.abs_diff(by))
}

/// Generate the map for a match seed: the first validated candidate
/// from the deterministic sub-seed stream, or the fallback template.
pub fn generate(seed: u64) -> GeneratedMap {
    for attempt in 0..MAX_ATTEMPTS {
        let sub_seed = seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (attempt as u64).wrapping_mul(0x2545_F491_4F6C_DD1D)
            ^ 0x4D37_2050_4745_4E21; // "M7 GEN!" — stream separator
        if let Some(map) = build(sub_seed) {
            if validate(&map) {
                return map;
            }
        }
    }
    fallback()
}

/// One candidate map from one sub-seed. `None` when any required
/// package element could not be placed (the caller retries with the
/// next sub-seed).
fn build(sub_seed: u64) -> Option<GeneratedMap> {
    let mut g = Gen {
        rng: Pcg32::new(sub_seed),
        terrain: vec![TERRAIN_LAND; (GRID_DIM * GRID_DIM) as usize],
        height: vec![0; (GRID_DIM * GRID_DIM) as usize],
        occ: vec![0; (GRID_DIM * GRID_DIM) as usize],
        nodes_a: Vec::new(),
        animals_a: Vec::new(),
        neutral: Vec::new(),
    };

    // Start A: west half of the map; start B is its mirror.
    let tc_a = (
        18 + g.rng.below(20),
        30 + g.rng.below(60),
    );
    let tc_b = rot(tc_a.0, tc_a.1);

    // Heights: white-noise levels, three box-blur passes, then
    // rotational symmetrization (copy the lower-indexed side).
    for h in g.height.iter_mut() {
        *h = g.rng.below(u32::from(MAX_HEIGHT) + 1) as u8;
    }
    for _ in 0..3 {
        g.height = blur(&g.height);
    }
    let hcopy = g.height.clone();
    for y in 0..GRID_DIM {
        for x in 0..GRID_DIM {
            let (rx, ry) = rot(x, y);
            if idx(rx, ry) < idx(x, y) {
                g.height[idx(x, y)] = hcopy[idx(rx, ry)];
            }
        }
    }

    // Water: 0–2 small ponds, placed as mirrored pairs, always far
    // from both starts. Arabia v1 uses water sparingly — some seeds
    // have none at all.
    let ponds = g.rng.below(3);
    for _ in 0..ponds {
        g.place_pond(tc_a, tc_b);
    }
    // Complete W1: rim every pond with shallows (the amphibious
    // ring a Dock straddles and land units wade). A pure terrain
    // rewrite of the placed ponds — it draws no RNG and claims no
    // cells, so the rest of generation is untouched, and dry
    // seeds (no pond) stay bit-identical by construction.
    rim_ponds(&mut g.terrain);

    // Claim the 3×3 around each Town Center: the start's villagers
    // and Scout spawn on its edge cells, so nothing may generate
    // there.
    g.claim_block(tc_a);
    g.claim_block(tc_b);

    // Start A's package, element by element (bands per §2.3).
    g.place_package(tc_a)?;

    // Mirror A's package into B's: identical counts, mirrored
    // distances — exact fairness by construction. A mirrored cell
    // already claimed (or water) fails the candidate: the terrain
    // is symmetric, so this can only be a neutral-placement clash
    // that placement order already prevents — a cheap assert.
    let mut pkg_b = StartPackage {
        tc_cell: tc_b,
        nodes: Vec::new(),
        animals: Vec::new(),
    };
    for i in 0..g.nodes_a.len() {
        let (kind, x, y) = g.nodes_a[i];
        let (mx, my) = rot(x, y);
        if !g.claim(mx, my) {
            return None;
        }
        pkg_b.nodes.push((kind, mx, my));
    }
    for i in 0..g.animals_a.len() {
        let (kind, x, y) = g.animals_a[i];
        let (mx, my) = rot(x, y);
        if !g.claim(mx, my) {
            return None;
        }
        pkg_b.animals.push((kind, mx, my));
    }

    // Neutral dressing: one mid-map forest, mirrored — extra wood
    // for the mid game, owned by neither start.
    g.place_neutral_forest();

    // Flatten heights around both starts and every package node, so
    // each opening plays on level ground (the start's own sampled
    // level — equal for both starts by symmetrization).
    let level_a = g.height[idx(tc_a.0, tc_a.1)];
    flatten_start(&mut g.height, tc_a, level_a, &g.nodes_a);
    flatten_start(&mut g.height, tc_b, level_a, &pkg_b.nodes);

    // MAP-07: author one cliff line (and its rotational mirror) on
    // the finished height field — a raised shelf with one sheer
    // face, placed clear of both starts and every package element.
    // Runs last among the height operations: nothing flattens or
    // blurs after this, so the authored face survives intact.
    author_cliffs(&mut g, tc_a, tc_b);

    // MAP-13 (Castle P5d): the five relics, placed last on the
    // finished candidate — two mirrored pairs and one center
    // relic (see `place_relics`). Placement draws from its OWN
    // stream (sub-seed ⊕ separator), never the base stream, so
    // the historical generation above is untouched: a candidate
    // that cannot place all five fairly is rejected like any
    // other invalid candidate.
    let relics = place_relics(&mut g, tc_a, tc_b, sub_seed)?;

    // Materialize the grid: terrain, heights, and the tree blocks
    // (MAP-04: every tree node blocks its cell).
    let mut grid = Grid::new();
    for y in 0..GRID_DIM {
        for x in 0..GRID_DIM {
            let i = idx(x, y);
            if g.terrain[i] != TERRAIN_LAND {
                // Deep water and (complete W1) the shallows rims.
                grid.set_terrain(x, y, g.terrain[i]);
            }
            grid.set_height(x, y, g.height[i]);
        }
    }
    let mut all_nodes: Vec<(NodeKind, u32, u32)> = Vec::new();
    all_nodes.extend_from_slice(&g.nodes_a);
    all_nodes.extend_from_slice(&pkg_b.nodes);
    all_nodes.extend_from_slice(&g.neutral);
    for &(kind, x, y) in &all_nodes {
        if kind == NodeKind::Tree {
            grid.set_node_block(x, y, true);
        }
    }

    // Water W2a (ECO-15): fish placement — over the finished,
    // rimmed terrain, a pure function of it (no RNG draws, so
    // the candidate stream and every dry seed stay untouched).
    g.neutral.extend(fish_placement(&g.terrain));

    let pkg_a = StartPackage {
        tc_cell: tc_a,
        nodes: std::mem::take(&mut g.nodes_a),
        animals: std::mem::take(&mut g.animals_a),
    };
    Some(GeneratedMap {
        grid,
        starts: [pkg_a, pkg_b],
        neutral_nodes: std::mem::take(&mut g.neutral),
        relics,
    })
}

/// Deterministic validation of a candidate map (see the module
/// docs): starts connected by a land path, every package element on
/// reachable land, no start enclosed, gentle height steps, dry
/// start ground. Pure function of the map — the same candidate
/// validates identically everywhere.

/// MAP-13's minimum Chebyshev distance from a relic to either
/// Town Center: no relic inside a start's immediate base area
/// (the plan's placement rule — contested ground only).
const RELIC_START_CLEARANCE: u32 = 20;

/// Place the five relics on a finished candidate (MAP-13):
/// two mirrored pairs drawn from a DEDICATED stream (the base
/// generation stream is never touched, so existing maps are
/// bit-identical apart from the relics themselves) and one
/// center relic placed by deterministic scan — the free land
/// cell nearest the map center whose Chebyshev distance to
/// start A equals its distance to start B exactly. Every relic
/// stands on free land at least `RELIC_START_CLEARANCE` tiles
/// from both Town Centers. The fairness predicate follows by
/// construction: rotation preserves Chebyshev distance, so a
/// mirrored pair contributes the same two distances to each
/// start, and the center relic contributes one equal distance
/// to both — the sorted distance multisets are identical.
/// `None` rejects the candidate (the generator retries with
/// the next sub-seed, like any validation failure).
fn place_relics(
    g: &mut Gen,
    tc_a: (u32, u32),
    tc_b: (u32, u32),
    sub_seed: u64,
) -> Option<Vec<(u32, u32)>> {
    let mut rng = Pcg32::new(sub_seed ^ 0x5245_4C49_4353_4D31); // "RELICM1"
    let mut relics: Vec<(u32, u32)> = Vec::with_capacity(5);
    let clear = |g: &Gen, x: u32, y: u32| {
        g.free(x, y)
            && chebyshev(x, y, tc_a.0, tc_a.1) >= RELIC_START_CLEARANCE
            && chebyshev(x, y, tc_b.0, tc_b.1) >= RELIC_START_CLEARANCE
    };
    // Two mirrored pairs.
    for _ in 0..2 {
        let mut placed = false;
        for _ in 0..200 {
            let x = 4 + rng.below(GRID_DIM - 8);
            let y = 4 + rng.below(GRID_DIM - 8);
            let (mx, my) = rot(x, y);
            if (mx, my) == (x, y) || !clear(g, x, y) || !clear(g, mx, my) {
                continue;
            }
            if g.claim(x, y) && g.claim(mx, my) {
                relics.push((x, y));
                relics.push((mx, my));
                placed = true;
                break;
            }
        }
        if !placed {
            return None;
        }
    }
    // The center relic: scan rings outward from the map center
    // (row-major inside a ring — fully deterministic, no draws).
    let center = (GRID_DIM / 2, GRID_DIM / 2);
    let mut found = false;
    'rings: for r in 0..GRID_DIM / 2 {
        for y in 0..GRID_DIM {
            for x in 0..GRID_DIM {
                if chebyshev(x, y, center.0, center.1) != r {
                    continue;
                }
                if chebyshev(x, y, tc_a.0, tc_a.1) != chebyshev(x, y, tc_b.0, tc_b.1) {
                    continue;
                }
                if clear(g, x, y) && g.claim(x, y) {
                    relics.push((x, y));
                    found = true;
                    break 'rings;
                }
            }
        }
    }
    if !found {
        return None;
    }
    debug_assert_eq!(relics.len(), 5);
    Some(relics)
}

pub fn validate(map: &GeneratedMap) -> bool {
    let grid = &map.grid;
    // Flood fill over land (terrain only: trees stand *on* land and
    // deplete — a path "through" a forest is a path a player can
    // chop; water never depletes, so it bounds the flood for real).
    let (ta, tb) = (map.starts[0].tc_cell, map.starts[1].tc_cell);
    let mut seen = vec![false; (GRID_DIM * GRID_DIM) as usize];
    let mut stack = vec![ta];
    seen[idx(ta.0, ta.1)] = true;
    let mut flood_size = 0usize;
    while let Some((x, y)) = stack.pop() {
        flood_size += 1;
        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx < 0 || ny < 0 || nx >= GRID_DIM as i32 || ny >= GRID_DIM as i32 {
                continue;
            }
            let (nx, ny) = (nx as u32, ny as u32);
            let i = idx(nx, ny);
            // MAP-07: the flood is land movement — a cliff border
            // seals it, exactly as it seals A*.
            if !seen[i] && !grid.is_water(nx, ny) && !grid.is_cliff_between(x, y, nx, ny) {
                seen[i] = true;
                stack.push((nx, ny));
            }
        }
    }
    // The starts connect, and the shared landmass is the map's bulk
    // (no start is enclosed in a pocket).
    if !seen[idx(tb.0, tb.1)] || flood_size < 5_000 {
        return false;
    }
    // Every package element stands on the shared landmass.
    for pkg in &map.starts {
        let cells: Vec<(u32, u32)> = pkg
            .nodes
            .iter()
            .map(|&(_, x, y)| (x, y))
            .chain(pkg.animals.iter().map(|&(_, x, y)| (x, y)))
            .collect();
        for &(x, y) in &cells {
            if !seen[idx(x, y)] {
                return false;
            }
        }
        // The Town Center's own cell is dry land.
        if grid.is_water(pkg.tc_cell.0, pkg.tc_cell.1) {
            return false;
        }
    }
    for &(kind, x, y) in &map.neutral_nodes {
        // Water W2a (ECO-15): fish spots are water nodes by
        // design — the land flood can never reach a deep-fish
        // cell, so the flood requirement above would reject
        // every pond candidate outright (measured: with the
        // fish extension live and this clause unamended, every
        // pond seed fell through to a dry candidate and Arabia
        // lost its ponds entirely). A fish cell's validation is
        // the water-side reading (plan §3.3.3): the cell is
        // water-side (deep water or shallows) — it stands in the
        // pond its placement read it from.
        if kind.is_fish() {
            if !grid.is_water_side(x, y) {
                return false;
            }
            continue;
        }
        if !seen[idx(x, y)] {
            return false;
        }
    }
    // MAP-13 (Castle P5d): every relic stands on the shared
    // landmass too — a relic sealed behind water or a cliff face
    // would be a relic no monk could ever fetch.
    if map.relics.len() != 5 {
        return false;
    }
    for &(x, y) in &map.relics {
        if !seen[idx(x, y)] {
            return false;
        }
    }
    // No water near either start. (Deep water only, as always:
    // `is_water` is the deep tag — a shallows rim cell is wadeable
    // ground, not the seal this clearance guards; the deep core
    // it rings stands at least as far out by construction.)
    for pkg in &map.starts {
        for y in 0..GRID_DIM {
            for x in 0..GRID_DIM {
                if grid.is_water(x, y)
                    && chebyshev(x, y, pkg.tc_cell.0, pkg.tc_cell.1)
                        <= WATER_START_CLEARANCE as u32
                {
                    return false;
                }
            }
        }
    }
    // Complete W1: every pond carries its shallows rim and offers
    // at least one terrain-legal Dock site (BLD-14) — a pond a
    // Dock could never straddle fails the candidate. The flood
    // above already treats shallows as land-passable (it seals on
    // deep water alone, `is_water`), which is the plan's
    // validation reading of the rim.
    if !ponds_have_rims_and_docks(grid) {
        return false;
    }
    // Height steps: the blurred hills step by at most 2 between
    // neighbors; a step of 3 or more is an authored cliff border
    // (MAP-07, see `author_cliffs`) — legal by design, so there
    // is nothing left to reject here. The flood above already
    // proved every package element stands on the shared landmass
    // under the cliff seal.
    true
}

/// Complete W1 validation helper: every pond (a 4-connected
/// water-side region containing deep water) carries a shallows
/// rim (at least one shallows cell) and is served by at least
/// one terrain-legal 3×3 Dock site (`Grid::dock_terrain_ok` —
/// the terrain half of the predicate, so shore-ring package
/// placement cannot revoke a pond's site). Maps with no pond
/// pass vacuously.
fn ponds_have_rims_and_docks(grid: &Grid) -> bool {
    // Label the water-side regions (deep water + shallows).
    let n = (GRID_DIM * GRID_DIM) as usize;
    let mut region = vec![u32::MAX; n];
    let mut regions: Vec<(bool, bool)> = Vec::new(); // (has deep, has shallows)
    for y in 0..GRID_DIM {
        for x in 0..GRID_DIM {
            let i = idx(x, y);
            if region[i] != u32::MAX || !grid.is_water_side(x, y) {
                continue;
            }
            let id = regions.len() as u32;
            let (mut deep, mut shal) = (false, false);
            let mut stack = vec![(x, y)];
            region[i] = id;
            while let Some((cx, cy)) = stack.pop() {
                if grid.is_water(cx, cy) {
                    deep = true;
                }
                if grid.is_shallows(cx, cy) {
                    shal = true;
                }
                for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                    let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= GRID_DIM as i32 || ny >= GRID_DIM as i32 {
                        continue;
                    }
                    let (nx, ny) = (nx as u32, ny as u32);
                    let ni = idx(nx, ny);
                    if region[ni] == u32::MAX && grid.is_water_side(nx, ny) {
                        region[ni] = id;
                        stack.push((nx, ny));
                    }
                }
            }
            regions.push((deep, shal));
        }
    }
    if regions.iter().all(|&(deep, _)| !deep) {
        return true; // no pond on the map
    }
    // A pond without a rim fails outright.
    if regions.iter().any(|&(deep, shal)| deep && !shal) {
        return false;
    }
    // Mark the regions any terrain-legal Dock site serves.
    let mut served = vec![false; regions.len()];
    for ay in 0..GRID_DIM - 2 {
        for ax in 0..GRID_DIM - 2 {
            if !grid.dock_terrain_ok(ax, ay, 3) {
                continue;
            }
            for dy in 0..3 {
                for dx in 0..3 {
                    let r = region[idx(ax + dx, ay + dy)];
                    if r != u32::MAX {
                        served[r as usize] = true;
                    }
                }
            }
        }
    }
    regions
        .iter()
        .enumerate()
        .all(|(i, &(deep, _))| !deep || served[i])
}

/// The fixed fallback template: used only if every randomized
/// candidate fails validation (a generator bug, by construction of
/// the retry loop). Hand-authored from the §2.3 band midpoints on
/// flat level-1 ground — valid by construction, and still passed
/// through `validate` by the caller path in tests. Complete W1:
/// the template gains one fixed pond-with-rim (and its rotational
/// mirror), so the fallback carries the water domain too.
fn fallback() -> GeneratedMap {
    let mut grid = Grid::new();
    for y in 0..GRID_DIM {
        for x in 0..GRID_DIM {
            grid.set_height(x, y, 1);
        }
    }
    // The fixed ponds (complete W1): radius-2 discs at (60, 20)
    // and its rotational mirror (59, 99) — mid-map ground clear
    // of both starts (Chebyshev ≥ 39 from each Town Center, past
    // the 22-tile water clearance with the deep core innermost),
    // of every package element and of the template's relics. The
    // rim rule is `rim_ponds`' own: a disc cell orthogonally
    // adjacent to land becomes shallows; the plus-shaped core
    // stays deep.
    for &(pcx, pcy) in &[(60i32, 20i32), (59, 99)] {
        let mut disc: Vec<(u32, u32)> = Vec::new();
        for dy in -2..=2i32 {
            for dx in -2..=2i32 {
                if dx * dx + dy * dy <= 4 {
                    disc.push(((pcx + dx) as u32, (pcy + dy) as u32));
                }
            }
        }
        for &(x, y) in &disc {
            let rim = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&(dx, dy)| !disc.contains(&((x as i32 + dx) as u32, (y as i32 + dy) as u32)));
            grid.set_terrain(x, y, if rim { TERRAIN_SHALLOWS } else { TERRAIN_WATER });
        }
    }
    let tc_a = (26u32, 60u32);
    // (kind, dx, dy) offsets from the Town Center cell — band
    // midpoints, fixed forever.
    let node_offsets: &[(NodeKind, i32, i32)] = &[
        // Berries: a 3×2 patch south-east at ~6 tiles.
        (NodeKind::BerryBush, 5, 3),
        (NodeKind::BerryBush, 6, 3),
        (NodeKind::BerryBush, 7, 3),
        (NodeKind::BerryBush, 5, 4),
        (NodeKind::BerryBush, 6, 4),
        (NodeKind::BerryBush, 7, 4),
        // Straggler trees.
        (NodeKind::Tree, -5, -4),
        (NodeKind::Tree, 4, -6),
        (NodeKind::Tree, -6, 3),
        // Main gold (7) north-east at ~12; back gold (4) behind
        // it, second secondary (4) split east.
        (NodeKind::GoldMine, 10, -6),
        (NodeKind::GoldMine, 11, -6),
        (NodeKind::GoldMine, 10, -5),
        (NodeKind::GoldMine, 11, -5),
        (NodeKind::GoldMine, 12, -6),
        (NodeKind::GoldMine, 10, -7),
        (NodeKind::GoldMine, 11, -7),
        (NodeKind::GoldMine, 16, -10),
        (NodeKind::GoldMine, 17, -10),
        (NodeKind::GoldMine, 16, -9),
        (NodeKind::GoldMine, 17, -9),
        // Second secondary gold (4) east at ~19–20 — the split
        // anchor, clear of the deer herd and the far sheep pair.
        (NodeKind::GoldMine, 19, 2),
        (NodeKind::GoldMine, 20, 2),
        (NodeKind::GoldMine, 19, 3),
        (NodeKind::GoldMine, 20, 3),
        // Main stone (5) south-west at ~12; back stone (4).
        (NodeKind::StoneMine, -10, 7),
        (NodeKind::StoneMine, -11, 7),
        (NodeKind::StoneMine, -10, 8),
        (NodeKind::StoneMine, -11, 8),
        (NodeKind::StoneMine, -10, 6),
        (NodeKind::StoneMine, -16, 11),
        (NodeKind::StoneMine, -17, 11),
        (NodeKind::StoneMine, -16, 12),
        (NodeKind::StoneMine, -17, 12),
    ];
    let mut nodes_a: Vec<(NodeKind, u32, u32)> = node_offsets
        .iter()
        .map(|&(k, dx, dy)| (k, (tc_a.0 as i32 + dx) as u32, (tc_a.1 as i32 + dy) as u32))
        .collect();
    // Forests as fixed rectangles: main woodline west, second
    // north — solid blocks (the readability pass removed the old
    // modulo holes; a template forest is a contiguous mass too).
    for x in 8..=12u32 {
        for y in 52..=58u32 {
            nodes_a.push((NodeKind::Tree, x, y));
        }
    }
    for x in 30..=34u32 {
        for y in 42..=45u32 {
            nodes_a.push((NodeKind::Tree, x, y));
        }
    }
    let animal_offsets: &[(EntityKind, i32, i32)] = &[
        // Near sheep (4) at 4–8.
        (EntityKind::Sheep, 4, 2),
        (EntityKind::Sheep, -4, 5),
        (EntityKind::Sheep, 6, -3),
        (EntityKind::Sheep, -3, -6),
        // Far sheep pairs (2+2) at 10–16.
        (EntityKind::Sheep, 12, 4),
        (EntityKind::Sheep, 13, 4),
        (EntityKind::Sheep, -12, -8),
        (EntityKind::Sheep, -13, -8),
        // Boars (2), split directions.
        (EntityKind::Boar, 18, -12),
        (EntityKind::Boar, -18, 14),
        // Deer: near herd (3), far herd (3).
        (EntityKind::Deer, 14, 8),
        (EntityKind::Deer, 15, 9),
        (EntityKind::Deer, 14, 10),
        (EntityKind::Deer, 24, -4),
        (EntityKind::Deer, 25, -3),
        (EntityKind::Deer, 24, -2),
        // Wolf pack (3) at ~28–30 (AI-08): the template Town
        // Center sits at (26, 60), so the pack stays on the map.
        (EntityKind::Wolf, 28, 12),
        (EntityKind::Wolf, 29, 13),
        (EntityKind::Wolf, 28, 14),
    ];
    let animals_a: Vec<(EntityKind, u32, u32)> = animal_offsets
        .iter()
        .map(|&(k, dx, dy)| (k, (tc_a.0 as i32 + dx) as u32, (tc_a.1 as i32 + dy) as u32))
        .collect();
    let mirror = |v: &[(NodeKind, u32, u32)]| -> Vec<(NodeKind, u32, u32)> {
        v.iter().map(|&(k, x, y)| (k, GRID_DIM - 1 - x, GRID_DIM - 1 - y)).collect()
    };
    let mirror_a = |v: &[(EntityKind, u32, u32)]| -> Vec<(EntityKind, u32, u32)> {
        v.iter().map(|&(k, x, y)| (k, GRID_DIM - 1 - x, GRID_DIM - 1 - y)).collect()
    };
    let pkg_a = StartPackage {
        tc_cell: tc_a,
        nodes: nodes_a.clone(),
        animals: animals_a.clone(),
    };
    let pkg_b = StartPackage {
        tc_cell: rot(tc_a.0, tc_a.1),
        nodes: mirror(&nodes_a),
        animals: mirror_a(&animals_a),
    };
    for pkg in [&pkg_a, &pkg_b] {
        for &(kind, x, y) in &pkg.nodes {
            if kind == NodeKind::Tree {
                grid.set_node_block(x, y, true);
            }
        }
    }
    // Water W2a (ECO-15): the template's two fixed ponds stock
    // fish by the same pure placement (read back off the grid).
    let terrain_now: Vec<u8> = {
        let mut v = Vec::with_capacity((GRID_DIM * GRID_DIM) as usize);
        for y in 0..GRID_DIM {
            for x in 0..GRID_DIM {
                v.push(grid.terrain_at(x, y));
            }
        }
        v
    };
    GeneratedMap {
        grid,
        starts: [pkg_a, pkg_b],
        neutral_nodes: fish_placement(&terrain_now),
        // MAP-13: the template's five relics — two mirrored
        // pairs and the equal-distance center cell (59, 26:
        // Chebyshev 34 from both Town Centers), all on free
        // land at least 20 tiles from both starts, verified
        // against this template's fixed cells.
        relics: vec![(50, 40), (69, 79), (45, 85), (74, 34), (59, 26)],
    }
}

/// Flatten one start's ground: the disc (radius 14 at the start's
/// level, blended out to 18) plus a radius-2 disc at the same level
/// around every package node, so each resource sits on level ground.
fn flatten_start(
    height: &mut [u8],
    tc: (u32, u32),
    level: u8,
    nodes: &[(NodeKind, u32, u32)],
) {
    for y in 0..GRID_DIM {
        for x in 0..GRID_DIM {
            let d = chebyshev(x, y, tc.0, tc.1);
            let i = idx(x, y);
            if d <= 14 {
                height[i] = level;
            } else if d <= 18 {
                // Blend ring: (d-14)/4 of the way from level to the
                // natural height — integer lerp, deterministic.
                let k = (d - 14) as i32;
                let h = height[i] as i32;
                height[i] = ((level as i32 * (4 - k) + h * k) / 4) as u8;
            }
        }
    }
    for &(_, nx, ny) in nodes {
        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let (x, y) = (nx as i32 + dx, ny as i32 + dy);
                if x >= 0 && y >= 0 && x < GRID_DIM as i32 && y < GRID_DIM as i32 {
                    height[idx(x as u32, y as u32)] = level;
                }
            }
        }
    }
}

/// MAP-07 cliff authoring: stamp one cliff line onto the finished
/// height field, plus its 180° rotational mirror (the map's
/// fairness symmetry). The shape, in (along, across) coordinates
/// with the face running along `u`:
///
/// ```text
///   v=0  falloff      2 2 2 2 … 2 2   (gentle step up to the shelf)
///   v=1  shelf        2 4 4 4 … 4 2
///   v=2  shelf        2 4 4 4 … 4 2   ← the face: 4 against 1
///   v=3  low strip      1 1 1 … 1     (end columns unforced)
///   v=4  low falloff    2 2 2 … 2     (end columns unforced)
/// ```
///
/// Every border inside the shape steps by at most 2 except the
/// face itself (4 against 1 — a step of 3, a cliff by
/// `grid::CLIFF_STEP`); every border between the shape and the
/// untouched field also steps by at most 2 (the falloff value 2
/// is within 2 of every legal height 0..=4), so the face is the
/// ONLY cliff the stamp creates. The shelf is enterable from the
/// falloff side and around both ends (the end columns step 2 → 4
/// onto the shelf); it is sealed across the face alone, exactly
/// like the reference's cliff lines, which end and can be walked
/// around.
///
/// Site discipline: the whole rectangle — and its mirror — must
/// be unclaimed dry land (no package element, animal, pond or
/// neutral dressing stands on it) at Chebyshev distance ≥ 26
/// from both Town Centers, outside the flattened start discs, so
/// the openings are untouched. Candidates are scanned on a fixed
/// lattice in a fixed order with alternating orientation; the
/// first fit wins, and no RNG is drawn, so the rest of
/// generation is unaffected. A candidate whose lattice finds no
/// site simply carries no cliff line (validation never requires
/// one; the hand-authored fallback has none).
fn author_cliffs(g: &mut Gen, tc_a: (u32, u32), tc_b: (u32, u32)) {
    /// Face length in cells.
    const LEN: i32 = 8;
    /// Chebyshev clearance from either Town Center.
    const CLEAR: u32 = 26;
    let mut n = 0i32;
    for ay in (8..GRID_DIM as i32 - 8).step_by(3) {
        for ax in (8..GRID_DIM as i32 - 8).step_by(3) {
            n += 1;
            if try_cliff(g, ax, ay, LEN, n % 2 == 0, tc_a, tc_b, CLEAR) {
                return;
            }
        }
    }
}

/// The height `author_cliffs` forces at shape coordinate
/// (u, v), or `None` to leave the cell's natural height (the two
/// end columns of the low rows, whose exact value the seal does
/// not depend on — see the shape comment).
fn cliff_value(u: i32, v: i32, len: i32) -> Option<u8> {
    match v {
        0 => Some(2),
        1 | 2 => Some(if u == -1 || u == len { 2 } else { 4 }),
        3 => {
            if u == -1 || u == len {
                None
            } else {
                Some(1)
            }
        }
        4 => {
            if u == -1 || u == len {
                None
            } else {
                Some(2)
            }
        }
        _ => None,
    }
}

/// Try one cliff site: anchor (ax, ay) is the shape's (u=-1, v=0)
/// corner; `vertical` transposes the shape. On success the shape
/// and its rotational mirror are stamped and `true` returned.
fn try_cliff(
    g: &mut Gen,
    ax: i32,
    ay: i32,
    len: i32,
    vertical: bool,
    tc_a: (u32, u32),
    tc_b: (u32, u32),
    clear: u32,
) -> bool {
    let cell = |u: i32, v: i32| -> (i32, i32) {
        if vertical {
            (ax + v, ay + u)
        } else {
            (ax + u, ay + v)
        }
    };
    // The full rectangle (both orientations span u -1..=len,
    // v 0..=4): every cell must be claimable ground for a cliff —
    // in bounds, dry, unclaimed, and clear of both starts.
    let mut rect: Vec<(u32, u32)> = Vec::new();
    let mut forced: Vec<((u32, u32), u8)> = Vec::new();
    for v in 0..=4i32 {
        for u in -1..=len {
            let (x, y) = cell(u, v);
            if !g.in_bounds(x, y) {
                return false;
            }
            let (x, y) = (x as u32, y as u32);
            if g.terrain[idx(x, y)] != TERRAIN_LAND || g.occ[idx(x, y)] != 0 {
                return false;
            }
            if chebyshev(x, y, tc_a.0, tc_a.1) < clear
                || chebyshev(x, y, tc_b.0, tc_b.1) < clear
            {
                return false;
            }
            rect.push((x, y));
            if let Some(h) = cliff_value(u, v, len) {
                forced.push(((x, y), h));
            }
        }
    }
    // The mirror rectangle must satisfy the same discipline and
    // the two rectangles must not overlap (the mirror's stamp
    // would otherwise overwrite this one's heights).
    let mut mirror_forced: Vec<((u32, u32), u8)> = Vec::new();
    for &(x, y) in &rect {
        let (mx, my) = rot(x, y);
        if rect.contains(&(mx, my)) {
            return false;
        }
        if g.terrain[idx(mx, my)] != TERRAIN_LAND || g.occ[idx(mx, my)] != 0 {
            return false;
        }
    }
    for &((x, y), h) in &forced {
        mirror_forced.push((rot(x, y), h));
    }
    for &((x, y), h) in forced.iter().chain(mirror_forced.iter()) {
        g.height[idx(x, y)] = h;
    }
    true
}

/// One box-blur pass over the height field (3×3 mean, edges clamped,
/// rounded) — the smoothing that turns white noise into hills.
fn blur(h: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; h.len()];
    for y in 0..GRID_DIM as i32 {
        for x in 0..GRID_DIM as i32 {
            let mut sum = 0i32;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = (x + dx).clamp(0, GRID_DIM as i32 - 1) as u32;
                    let ny = (y + dy).clamp(0, GRID_DIM as i32 - 1) as u32;
                    sum += h[idx(nx, ny)] as i32;
                }
            }
            out[idx(x as u32, y as u32)] = ((sum + 4) / 9) as u8;
        }
    }
    out
}

/// The working state of one candidate build.
struct Gen {
    rng: Pcg32,
    terrain: Vec<u8>,
    height: Vec<u8>,
    /// Claim map: 0 = free, 1 = taken by a package element, a Town
    /// Center block, pond-adjacent bookkeeping, or a neutral.
    occ: Vec<u8>,
    nodes_a: Vec<(NodeKind, u32, u32)>,
    animals_a: Vec<(EntityKind, u32, u32)>,
    neutral: Vec<(NodeKind, u32, u32)>,
}

impl Gen {
    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= MARGIN && y >= MARGIN && x < GRID_DIM as i32 - MARGIN && y < GRID_DIM as i32 - MARGIN
    }

    /// A cell a package element may occupy: in bounds, dry land,
    /// unclaimed.
    fn free(&self, x: u32, y: u32) -> bool {
        self.in_bounds(x as i32, y as i32)
            && self.terrain[idx(x, y)] == TERRAIN_LAND
            && self.occ[idx(x, y)] == 0
    }

    /// Claim a cell for a mirrored element: must currently be free.
    fn claim(&mut self, x: u32, y: u32) -> bool {
        if !self.free(x, y) {
            return false;
        }
        self.occ[idx(x, y)] = 1;
        true
    }

    fn claim_block(&mut self, c: (u32, u32)) {
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (x, y) = (c.0 as i32 + dx, c.1 as i32 + dy);
                if x >= 0 && y >= 0 && x < GRID_DIM as i32 && y < GRID_DIM as i32 {
                    self.occ[idx(x as u32, y as u32)] = 1;
                }
            }
        }
    }

    /// A random free cell at Chebyshev-free Euclidean band distance
    /// [dmin, dmax] (in tiles) from a center, integer math only.
    /// Bounded tries; `None` when the band is exhausted nearby.
    fn pick_in_band(&mut self, c: (u32, u32), dmin: i32, dmax: i32) -> Option<(u32, u32)> {
        for _ in 0..80 {
            let span = (2 * dmax + 1) as u32;
            let dx = self.rng.below(span) as i32 - dmax;
            let dy = self.rng.below(span) as i32 - dmax;
            let d2 = dx * dx + dy * dy;
            if d2 < dmin * dmin || d2 > dmax * dmax {
                continue;
            }
            let (x, y) = (c.0 as i32 + dx, c.1 as i32 + dy);
            if x < 0 || y < 0 {
                continue;
            }
            let (x, y) = (x as u32, y as u32);
            if self.free(x, y) {
                return Some((x, y));
            }
        }
        None
    }

    /// A random free cell in the Chebyshev box around a center
    /// (dx, dy each within `spread`) — the square-patch pick for
    /// clusters and herds, whose shapes are boxes, not discs (a
    /// Euclidean spread-1 'cluster' would be a 5-cell plus, too
    /// small for a 6-bush berry patch).
    fn pick_in_box(&mut self, c: (u32, u32), spread: i32) -> Option<(u32, u32)> {
        for _ in 0..40 {
            let span = (2 * spread + 1) as u32;
            let dx = self.rng.below(span) as i32 - spread;
            let dy = self.rng.below(span) as i32 - spread;
            let (x, y) = (c.0 as i32 + dx, c.1 as i32 + dy);
            if x < 0 || y < 0 {
                continue;
            }
            let (x, y) = (x as u32, y as u32);
            if self.free(x, y) {
                return Some((x, y));
            }
        }
        None
    }

    /// A random point (need not be free) in a distance band — for
    /// forest/pond centers.
    fn pick_point_in_band(&mut self, c: (u32, u32), dmin: i32, dmax: i32) -> Option<(u32, u32)> {
        for _ in 0..40 {
            let span = (2 * dmax + 1) as u32;
            let dx = self.rng.below(span) as i32 - dmax;
            let dy = self.rng.below(span) as i32 - dmax;
            let d2 = dx * dx + dy * dy;
            if d2 < dmin * dmin || d2 > dmax * dmax {
                continue;
            }
            let (x, y) = (c.0 as i32 + dx, c.1 as i32 + dy);
            if self.in_bounds(x, y) {
                return Some((x as u32, y as u32));
            }
        }
        None
    }

    fn put_node(&mut self, kind: NodeKind, x: u32, y: u32) -> bool {
        if !self.claim(x, y) {
            return false;
        }
        self.nodes_a.push((kind, x, y));
        true
    }

    fn put_animal(&mut self, kind: EntityKind, x: u32, y: u32) -> bool {
        if !self.claim(x, y) {
            return false;
        }
        self.animals_a.push((kind, x, y));
        true
    }

    /// A compact cluster of `count` nodes within `spread` tiles of
    /// an anchor (the anchor cell first). All-or-nothing.
    fn cluster(
        &mut self,
        anchor: (u32, u32),
        kind: NodeKind,
        count: usize,
        spread: i32,
    ) -> bool {
        let mut placed: Vec<(NodeKind, u32, u32)> = Vec::new();
        let mut cells: Vec<(u32, u32)> = Vec::new();
        if self.free(anchor.0, anchor.1) {
            cells.push(anchor);
        }
        let mut guard = 0;
        while cells.len() < count && guard < 60 {
            guard += 1;
            if let Some(p) = self.pick_in_box(anchor, spread) {
                if !cells.contains(&p) {
                    cells.push(p);
                }
            }
        }
        if cells.len() < count {
            return false;
        }
        for (x, y) in cells {
            if self.put_node(kind, x, y) {
                placed.push((kind, x, y));
            }
        }
        placed.len() == count
    }

    /// A loose herd of `count` animals within `spread` of an anchor.
    fn herd(&mut self, anchor: (u32, u32), kind: EntityKind, count: usize, spread: i32) -> bool {
        let mut cells: Vec<(u32, u32)> = Vec::new();
        if self.free(anchor.0, anchor.1) {
            cells.push(anchor);
        }
        let mut guard = 0;
        while cells.len() < count && guard < 60 {
            guard += 1;
            if let Some(p) = self.pick_in_box(anchor, spread) {
                if !cells.contains(&p) {
                    cells.push(p);
                }
            }
        }
        if cells.len() < count {
            return false;
        }
        for (x, y) in cells {
            if !self.put_animal(kind, x, y) {
                return false;
            }
        }
        true
    }

    /// A forest blob: a CONTIGUOUS mass of trees (forest
    /// readability pass, 2026-10-20 — Claudio's live-play feedback:
    /// the old 82%-density disc left ~1 in 5 interior cells empty,
    /// so a woodline read as scattered single trees on empty
    /// diamonds instead of a bosque). Every free cell of the disc
    /// is claimed; only the rim (cells whose distance is within
    /// one tile of the radius) is ragged — each rim cell is claimed
    /// with 60% probability, and a rim cell that would stick out
    /// alone (no claimed neighbour yet, in fixed cell order) is
    /// skipped, so the edge stays irregular without isolated
    /// one-tree islands. Interior holes never occur. Cells are
    /// visited in fixed order and every probability draw comes from
    /// the seeded stream, so a seed's forest is byte-identical on
    /// every machine. Returns the number of trees placed.
    fn forest(&mut self, center: (u32, u32), radius: i32) -> usize {
        let mut placed = 0;
        let mut claimed: Vec<(u32, u32)> = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let d2 = dx * dx + dy * dy;
                if d2 > radius * radius {
                    continue;
                }
                let (x, y) = (center.0 as i32 + dx, center.1 as i32 + dy);
                if x < 0 || y < 0 {
                    continue;
                }
                let (x, y) = (x as u32, y as u32);
                if !self.free(x, y) {
                    continue;
                }
                // Rim cells: strictly inside the disc's outer ring
                // (distance > radius - 1) are the ragged edge.
                // Integer-only, like the rest of generation.
                let rim = d2 > (radius - 1) * (radius - 1);
                if rim {
                    if self.rng.below(100) >= 60 {
                        continue;
                    }
                    // No isolated rim spikes: at least one already
                    // claimed 8-neighbour (fixed order ⇒ the cells
                    // above/left are the ones that can exist yet;
                    // interior cells below/right are claimed later
                    // with certainty only when non-rim — a rim cell
                    // whose only neighbours are unvisited rim cells
                    // is dropped, keeping the mass connected).
                    let has_nb = claimed.iter().any(|&(cx, cy)| {
                        cx.abs_diff(x) <= 1 && cy.abs_diff(y) <= 1 && (cx, cy) != (x, y)
                    });
                    if !has_nb {
                        continue;
                    }
                }
                if self.put_node(NodeKind::Tree, x, y) {
                    claimed.push((x, y));
                    placed += 1;
                }
            }
        }
        placed
    }

    /// One pond (a small disc) plus its rotational mirror. Skipped
    /// silently when the spot is taken — ponds are dressing.
    fn place_pond(&mut self, tc_a: (u32, u32), tc_b: (u32, u32)) {
        let Some(center) = self.pick_point_in_band((60, 60), 18, 46) else {
            return;
        };
        if chebyshev(center.0, center.1, tc_a.0, tc_a.1) <= WATER_START_CLEARANCE as u32 + 2
            || chebyshev(center.0, center.1, tc_b.0, tc_b.1) <= WATER_START_CLEARANCE as u32 + 2
        {
            return;
        }
        let radius = 1 + self.rng.below(2) as i32; // 1..=2 → discs of 5–13 cells
        let mut cells: Vec<(u32, u32)> = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                let (x, y) = (center.0 as i32 + dx, center.1 as i32 + dy);
                if !self.in_bounds(x, y) {
                    return;
                }
                cells.push((x as u32, y as u32));
            }
        }
        // Both the pond and its mirror must be untouched land.
        for &(x, y) in &cells {
            let (mx, my) = rot(x, y);
            if self.terrain[idx(x, y)] != TERRAIN_LAND
                || self.terrain[idx(mx, my)] != TERRAIN_LAND
                || self.occ[idx(x, y)] != 0
                || self.occ[idx(mx, my)] != 0
            {
                return;
            }
        }
        for &(x, y) in &cells {
            let (mx, my) = rot(x, y);
            self.terrain[idx(x, y)] = TERRAIN_WATER;
            self.terrain[idx(mx, my)] = TERRAIN_WATER;
        }
    }

    /// The mid-map neutral forest pair (dressing): one blob, its
    /// mirror claimed cell-for-cell where free.
    fn place_neutral_forest(&mut self) {
        let Some(center) = self.pick_point_in_band((60, 60), 6, 20) else {
            return;
        };
        let radius = 2 + self.rng.below(2) as i32;
        let mut placed: Vec<(u32, u32)> = Vec::new();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                let (x, y) = (center.0 as i32 + dx, center.1 as i32 + dy);
                if x < 0 || y < 0 {
                    continue;
                }
                let (x, y) = (x as u32, y as u32);
                if !self.free(x, y) {
                    continue;
                }
                // Solid interior, ragged rim — the same forest
                // shape as the woodlines (see `forest`); the rim
                // draw keeps the neutral dressing's total wood
                // comparable to the pre-pass density fill.
                if dx * dx + dy * dy > (radius - 1) * (radius - 1)
                    && self.rng.below(100) >= 60
                {
                    continue;
                }
                if self.claim(x, y) {
                    self.neutral.push((NodeKind::Tree, x, y));
                    placed.push((x, y));
                }
            }
        }
        for (x, y) in placed {
            let (mx, my) = rot(x, y);
            if self.claim(mx, my) {
                self.neutral.push((NodeKind::Tree, mx, my));
            }
        }
    }

    /// Start A's resource package (bands per OPENING-SPEC §2.3).
    /// `None` = this candidate is unusable; the caller retries.
    fn place_package(&mut self, tc: (u32, u32)) -> Option<()> {
        // Berries: 6 bushes in a tight patch at 5–8.
        let anchor = self.pick_in_band(tc, 5, 8)?;
        if !self.cluster(anchor, NodeKind::BerryBush, 6, 1) {
            return None;
        }
        // Near sheep: 4 singles at 4–8.
        for _ in 0..4 {
            let p = self.pick_in_band(tc, 4, 8)?;
            if !self.put_animal(EntityKind::Sheep, p.0, p.1) {
                return None;
            }
        }
        // Far sheep: two pairs at 10–16, split anchors.
        for (lo, hi) in [(10, 14), (12, 16)] {
            let a = self.pick_in_band(tc, lo, hi)?;
            if !self.put_animal(EntityKind::Sheep, a.0, a.1) {
                return None;
            }
            // The pair partner: first free neighbor in fixed order.
            let mut partner = None;
            for (dx, dy) in [(1i32, 0i32), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, -1)] {
                let (x, y) = (a.0 as i32 + dx, a.1 as i32 + dy);
                if x >= 0 && y >= 0 && self.free(x as u32, y as u32) {
                    partner = Some((x as u32, y as u32));
                    break;
                }
            }
            let p = partner?;
            if !self.put_animal(EntityKind::Sheep, p.0, p.1) {
                return None;
            }
        }
        // Straggler trees: 3 singles at 4–8.
        for _ in 0..3 {
            let p = self.pick_in_band(tc, 4, 8)?;
            if !self.put_node(NodeKind::Tree, p.0, p.1) {
                return None;
            }
        }
        // Main woodline: a real forest (dozens of trees), its edge
        // 10–15 tiles out — center 13–17 with radius 3–4.
        let center = self.pick_point_in_band(tc, 13, 17)?;
        let r1 = 3 + self.rng.below(2) as i32;
        let placed = self.forest(center, r1);
        if placed < 22 {
            return None;
        }
        // Second woodline, a different direction (fresh band pick),
        // center 15–20, radius 2–3.
        let center2 = self.pick_point_in_band(tc, 15, 20)?;
        let r2 = 2 + self.rng.below(2) as i32;
        let placed2 = self.forest(center2, r2);
        if placed2 < 9 {
            return None;
        }
        // Main gold: 7 tiles at 10–16; TWO secondary 4-piles
        // farther out (study 2, NEW 4: the reference Arabia gold
        // is 7 + 4 + 4, and OPENING-SPEC §2.3 always said so —
        // "Back golds: 2 × 4"). The first sits behind the main
        // along its direction; the second takes a split anchor —
        // a fresh band pick farther out, the same treatment the
        // boars get below.
        let gold_anchor = self.pick_in_band(tc, 10, 14)?;
        if !self.cluster(gold_anchor, NodeKind::GoldMine, 7, 1) {
            return None;
        }
        if !self.back_cluster(tc, gold_anchor, NodeKind::GoldMine, 4) {
            return None;
        }
        let gold2_anchor = self.pick_in_band(tc, 16, 22)?;
        if !self.cluster(gold2_anchor, NodeKind::GoldMine, 4, 1) {
            return None;
        }
        // Main stone: 5 tiles at 10–16; back stone: 4 behind it.
        let stone_anchor = self.pick_in_band(tc, 10, 14)?;
        if !self.cluster(stone_anchor, NodeKind::StoneMine, 5, 1) {
            return None;
        }
        if !self.back_cluster(tc, stone_anchor, NodeKind::StoneMine, 4) {
            return None;
        }
        // Boars: 2 at 15–24, split directions (two band picks).
        for _ in 0..2 {
            let p = self.pick_in_band(tc, 15, 24)?;
            if !self.put_animal(EntityKind::Boar, p.0, p.1) {
                return None;
            }
        }
        // Deer: a near herd of 3 at 12–18, a far herd of 3 at 20–28.
        let near = self.pick_in_band(tc, 12, 18)?;
        if !self.herd(near, EntityKind::Deer, 3, 2) {
            return None;
        }
        let far = self.pick_in_band(tc, 20, 28)?;
        if !self.herd(far, EntityKind::Deer, 3, 2) {
            return None;
        }
        // Wolves (AI-08): one pack of 3 at 26–34 — outside every
        // starting band (the far deer end at 28) and beyond the
        // resource-exclusion distances the map references document
        // (objects keep ~7 tiles from earlier spawns; cliffs keep
        // 22 from Town Centers, Aoe2Sheet knowledge base), so a
        // pack never spawns next to a starting Town Center. The
        // wiki documents wolves "often found in groups" without an
        // Arabia per-player count (they are neutral dressing in
        // the reference, not a per-start guarantee); the pack of 3
        // per start is this generator's symmetry-preserving
        // adoption — both starts get exactly one, mirrored.
        let pack = self.pick_in_band(tc, 26, 34)?;
        if !self.herd(pack, EntityKind::Wolf, 3, 2) {
            return None;
        }
        Some(())
    }

    /// The "back" mine cluster behind a main one: 5–8 tiles farther
    /// from the Town Center along the main cluster's direction.
    fn back_cluster(
        &mut self,
        tc: (u32, u32),
        main_anchor: (u32, u32),
        kind: NodeKind,
        count: usize,
    ) -> bool {
        let dirx = (main_anchor.0 as i32 - tc.0 as i32).signum();
        let diry = (main_anchor.1 as i32 - tc.1 as i32).signum();
        let step = 5 + self.rng.below(4) as i32;
        let (ax, ay) = (
            main_anchor.0 as i32 + dirx * step,
            main_anchor.1 as i32 + diry * step,
        );
        if ax < 0 || ay < 0 || !self.in_bounds(ax, ay) {
            return false;
        }
        self.cluster((ax as u32, ay as u32), kind, count, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_and_seed_varied() {
        let a = generate(42);
        let b = generate(42);
        assert_eq!(a, b, "same seed must produce a byte-identical map");
        let c = generate(43);
        assert_ne!(a, c, "different seeds must produce different maps");
        assert_ne!(
            a.grid.height_bytes(),
            c.grid.height_bytes(),
            "different seeds, identical height fields"
        );
    }

    /// Count cliff borders (adjacent pairs stepping by
    /// `crate::grid::CLIFF_STEP` or more) on a generated map.
    fn cliff_borders(m: &GeneratedMap) -> usize {
        let mut n = 0;
        for y in 0..GRID_DIM {
            for x in 0..GRID_DIM {
                for (dx, dy) in [(1i32, 0i32), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < GRID_DIM as i32
                        && ny < GRID_DIM as i32
                        && m.grid.is_cliff_between(x, y, nx as u32, ny as u32)
                    {
                        n += 1;
                    }
                }
            }
        }
        n
    }

    #[test]
    fn cliffs_are_authored_and_sealed_over_seeds() {
        // MAP-07 soak: across a spread of seeds, generated maps
        // carry authored cliff faces, and no A* path ever crosses
        // a cliff border (the seal — diagonal corner rule
        // included). Every map still validates (starts connected
        // under the seal).
        for seed in [1u64, 2, 3, 7, 42, 99, 1234, 31337, 555, 2026] {
            let m = generate(seed);
            assert!(validate(&m), "seed {seed}: map invalid with cliffs");
            let borders = cliff_borders(&m);
            assert!(
                borders >= 8,
                "seed {seed}: expected an authored cliff line (two faces of 8), found {borders} cliff borders"
            );
            // Path from start A's TC to start B's TC crosses no
            // cliff border, cell step by cell step.
            let a = m.grid.cell_center(m.starts[0].tc_cell.0, m.starts[0].tc_cell.1);
            let b = m.grid.cell_center(m.starts[1].tc_cell.0, m.starts[1].tc_cell.1);
            let path = m.grid.find_path(a, b).expect("starts must stay connected");
            let mut prev = m.starts[0].tc_cell;
            for &(wx, wy) in &path {
                let cell = m.grid.world_to_cell(wx, wy);
                if cell != prev {
                    assert!(
                        !m.grid.move_crosses_cliff(prev, cell),
                        "seed {seed}: path crosses a cliff from {prev:?} to {cell:?}"
                    );
                    prev = cell;
                }
            }
            // And a direct order across a face is refused outright:
            // for the first cliff border found, the two cells
            // cannot be pathed between *directly* — any route must
            // detour around the line's end (or not exist).
            'outer: for y in 0..GRID_DIM - 1 {
                for x in 0..GRID_DIM {
                    if m.grid.is_cliff_between(x, y, x, y + 1) {
                        let p = m.grid.find_path(m.grid.cell_center(x, y), m.grid.cell_center(x, y + 1));
                        if let Some(p) = p {
                            assert!(
                                p.len() > 2,
                                "seed {seed}: path stepped straight across the cliff at ({x},{y})"
                            );
                        }
                        break 'outer;
                    }
                }
            }
        }
        // The hand-authored fallback stays cliff-free by
        // construction (flat level-1 ground).
        assert_eq!(cliff_borders(&fallback()), 0);
    }

    #[test]
    fn generated_maps_validate_and_are_fair() {
        let mut tc_cells = std::collections::HashSet::new();
        for seed in [1u64, 2, 3, 7, 42, 99, 1234, 31337] {
            let m = generate(seed);
            tc_cells.insert(m.starts[0].tc_cell);
            assert!(validate(&m), "seed {seed} produced an invalid map");
            // Fairness: equal counts of every node/animal kind.
            for kind in [
                NodeKind::Tree,
                NodeKind::BerryBush,
                NodeKind::GoldMine,
                NodeKind::StoneMine,
            ] {
                let count = |pkg: &StartPackage| {
                    pkg.nodes.iter().filter(|n| n.0 == kind).count()
                };
                assert_eq!(
                    count(&m.starts[0]),
                    count(&m.starts[1]),
                    "seed {seed}: {kind:?} counts differ per start"
                );
            }
            for kind in [
                EntityKind::Sheep,
                EntityKind::Boar,
                EntityKind::Deer,
                EntityKind::Wolf,
            ] {
                let count = |pkg: &StartPackage| {
                    pkg.animals.iter().filter(|n| n.0 == kind).count()
                };
                assert_eq!(
                    count(&m.starts[0]),
                    count(&m.starts[1]),
                    "seed {seed}: {kind:?} counts differ per start"
                );
            }
            // The §2.3 package minimums.
            let a = &m.starts[0];
            let n = |k: NodeKind| a.nodes.iter().filter(|x| x.0 == k).count();
            assert_eq!(n(NodeKind::BerryBush), 6);
            // Gold is 7 + 4 + 4 (study 2, NEW 4): the count pins
            // the package; the bounds pin its shape — nothing
            // beyond 28 tiles, and at least one cell (the split
            // secondary's anchor, picked in the 16–22 band) sits
            // at 16+ tiles, farther out than the main's band.
            assert_eq!(n(NodeKind::GoldMine), 15);
            let (tcx, tcy) = (a.tc_cell.0 as i32, a.tc_cell.1 as i32);
            let gold_max_d2 = a
                .nodes
                .iter()
                .filter(|x| x.0 == NodeKind::GoldMine)
                .map(|x| {
                    let dx = x.1 as i32 - tcx;
                    let dy = x.2 as i32 - tcy;
                    dx * dx + dy * dy
                })
                .max()
                .unwrap_or(0);
            assert!(
                gold_max_d2 <= 28 * 28,
                "seed {seed}: gold beyond 28 tiles (d2 {gold_max_d2})"
            );
            assert!(
                gold_max_d2 >= 16 * 16,
                "seed {seed}: no secondary gold at 16+ tiles (max d2 {gold_max_d2})"
            );
            assert_eq!(n(NodeKind::StoneMine), 9);
            assert!(n(NodeKind::Tree) >= 30, "woodlines too thin: {}", n(NodeKind::Tree));
            let an = |k: EntityKind| a.animals.iter().filter(|x| x.0 == k).count();
            assert_eq!(an(EntityKind::Sheep), 8);
            assert_eq!(an(EntityKind::Boar), 2);
            assert_eq!(an(EntityKind::Deer), 6);
            // AI-08: exactly one wolf pack of 3 per start, and no
            // wolf inside 24 tiles of the Town Center (the pack
            // anchor is picked at 26–34; herd spread can pull a
            // member ~2 tiles in — 24 leaves the margin while
            // keeping every wolf far outside the starting bands).
            assert_eq!(an(EntityKind::Wolf), 3);
            let wolf_min_d2 = a
                .animals
                .iter()
                .filter(|x| x.0 == EntityKind::Wolf)
                .map(|x| {
                    let dx = x.1 as i32 - tcx;
                    let dy = x.2 as i32 - tcy;
                    dx * dx + dy * dy
                })
                .min()
                .unwrap_or(0);
            assert!(
                wolf_min_d2 >= 24 * 24,
                "seed {seed}: wolf pack too close to the Town Center (d2 {wolf_min_d2})"
            );
        }
        // The fixed fallback template has exactly one Town Center
        // cell; a spread of distinct cells across seeds proves the
        // random generator (not the fallback) made these maps.
        assert!(
            tc_cells.len() >= 5,
            "suspiciously few distinct starts: {tc_cells:?} (fallback leakage?)"
        );
    }

    #[test]
    fn forests_are_contiguous_masses() {
        // Forest readability pass (2026-10-20): a woodline must be
        // one touching mass, not scattered singles. The only
        // package trees allowed to stand alone are the 3 deliberate
        // stragglers, so >= 90% of a start's trees must have at
        // least one tree among their 8 neighbours. Counts are
        // printed (run with --nocapture) as the before/after record:
        // the pre-pass 82%-density disc gave 39-70 trees per start
        // on these seeds with ~18% interior holes.
        for seed in [1u64, 2, 3, 7, 42, 99, 1234, 31337] {
            let m = generate(seed);
            let a = &m.starts[0];
            let set: std::collections::HashSet<(u32, u32)> = a
                .nodes
                .iter()
                .filter(|n| n.0 == NodeKind::Tree)
                .map(|n| (n.1, n.2))
                .collect();
            let mut with_nb = 0;
            for &(x, y) in &set {
                let mut nb = false;
                for dx in -1i32..=1 {
                    for dy in -1i32..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx >= 0 && ny >= 0 && set.contains(&(nx as u32, ny as u32)) {
                            nb = true;
                        }
                    }
                }
                if nb {
                    with_nb += 1;
                }
            }
            println!(
                "seed {seed}: trees/start={} with_tree_neighbour={with_nb} neutral={}",
                set.len(),
                m.neutral_nodes.len()
            );
            assert!(
                with_nb * 10 >= set.len() * 9,
                "seed {seed}: forest not contiguous: {with_nb}/{} trees have a tree neighbour",
                set.len()
            );
        }
    }

    #[test]
    fn fallback_template_validates() {
        let m = fallback();
        assert!(validate(&m), "the fallback template must always validate");
    }

    #[test]
    fn ponds_carry_shallows_rims_over_seeds() {
        // Complete W1 soak (plan §5): across a spread of seeds,
        // every pond (any deep water) carries a shallows rim,
        // every map validates (which now includes the rim +
        // Dock-site requirement), and dry seeds stay dry — no
        // shallows without a pond.
        let mut pond_seeds = 0;
        let mut dry_seeds = 0;
        for seed in 1..=24u64 {
            let m = generate(seed);
            assert!(validate(&m), "seed {seed}: generated map must validate");
            let deep = m.grid.terrain_bytes().iter().filter(|&&t| t == TERRAIN_WATER).count();
            let shal = m
                .grid
                .terrain_bytes()
                .iter()
                .filter(|&&t| t == TERRAIN_SHALLOWS)
                .count();
            if deep > 0 {
                pond_seeds += 1;
                assert!(
                    shal > 0,
                    "seed {seed}: a pond without a shallows rim ({deep} deep, 0 shallows)"
                );
                // Every pond offers a terrain-legal Dock site
                // (validate's own check, restated as a soak pin).
                assert!(
                    ponds_have_rims_and_docks(&m.grid),
                    "seed {seed}: a pond with no legal Dock site"
                );
            } else {
                dry_seeds += 1;
                assert_eq!(shal, 0, "seed {seed}: shallows without a pond");
            }
        }
        assert!(pond_seeds > 0, "the soak never saw a pond");
        assert!(dry_seeds > 0, "the soak never saw a dry seed");
    }

    #[test]
    fn fallback_pond_is_rimmed_and_dockable() {
        // Complete W1: the hand-authored template's fixed pond
        // carries the rim rule too — a deep core ringed by
        // shallows, with a legal Dock placement on its shore.
        let m = fallback();
        let deep = m.grid.terrain_bytes().iter().filter(|&&t| t == TERRAIN_WATER).count();
        let shal = m
            .grid
            .terrain_bytes()
            .iter()
            .filter(|&&t| t == TERRAIN_SHALLOWS)
            .count();
        assert_eq!(deep, 10, "two ponds × a plus-shaped deep core of 5");
        assert_eq!(shal, 16, "two ponds × a rim of 8");
        let mut dockable = false;
        for ay in 14..=24u32 {
            for ax in 54..=64u32 {
                if m.grid.dock_placement_ok(ax, ay, 3) {
                    dockable = true;
                }
            }
        }
        assert!(dockable, "no legal Dock site on the template pond's shore");
    }
}


