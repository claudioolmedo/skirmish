//! Construction model (M2.5): buildings a villager raises on a chosen
//! patch of ground — Houses and Farms — plus the population they
//! support.
//!
//! Numbers come from the public mechanics study
//! (`MECHANICS-STUDY.md` §5 and §9.4), converted once, by hand, at
//! authoring time:
//!
//! - House: 25 wood, 25 s build time (1500 ticks), +5 population room.
//!   (Study §5 gives the AoE2 farm footprint as 3×3; Skirmish uses the
//!   uniform 2×2-cell footprint for both buildings — see below.)
//! - Farm: 60 wood, 15 s build time (900 ticks), holds 175 food
//!   (§6.3 — the base amount; 250 is post-Horse-Collar).
//! - Town Center: +5 population room (§5 function notes, §9.4).
//!
//! Placement is defined on the coarse grid: a building occupies a
//! **2×2-cell footprint** anchored at its minimum cell. Houses block
//! their footprint cells in the grid's dynamic layer (they are solid);
//! farms do not block — a farmer must be able to stand on the field to
//! work it, exactly as in the reference game.

use fixed::types::I16F16;

use crate::grid::{CELL_SIZE, WORLD_HALF};

/// Footprint edge length, in grid cells. Both buildings are 2×2.
pub const FOOTPRINT_CELLS: u32 = 2;

/// House construction cost: 25 wood (study §5).
pub const HOUSE_WOOD_COST: I16F16 = I16F16::from_bits(1_638_400); // 25
/// House build time: 25 s at 60 Hz (study §5).
pub const HOUSE_BUILD_TICKS: u64 = 1500;
/// Farm construction cost: 60 wood (study §5).
pub const FARM_WOOD_COST: I16F16 = I16F16::from_bits(3_932_160); // 60
/// Farm build time: 15 s at 60 Hz (study §5).
pub const FARM_BUILD_TICKS: u64 = 900;
/// Population room from one completed House: +5 (study §5, §9.4).
pub const HOUSE_POP_SUPPORT: u32 = 5;
/// Population room from the Town Center: +5 (study §5, §9.4).
pub const TOWN_CENTER_POP_SUPPORT: u32 = 5;
/// A builder this close to the building's center is actively building:
/// 9.0 world units. Derived from the grid (not the study): a 2×2-cell
/// footprint is 8 units wide, so a builder standing on any cell of the
/// ring around it is at most ~8.5 units from the center.
pub const BUILD_RANGE: I16F16 = I16F16::from_bits(589_824); // 9.0

/// A buildable building kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingKind {
    House,
    Farm,
}

impl BuildingKind {
    /// Stable tag for the state hash and the WASM building array:
    /// 0 = house, 1 = farm.
    pub fn tag(self) -> u8 {
        match self {
            BuildingKind::House => 0,
            BuildingKind::Farm => 1,
        }
    }

    /// Wood paid at placement time.
    pub fn wood_cost(self) -> I16F16 {
        match self {
            BuildingKind::House => HOUSE_WOOD_COST,
            BuildingKind::Farm => FARM_WOOD_COST,
        }
    }

    /// Ticks of active building needed to complete.
    pub fn build_ticks(self) -> u64 {
        match self {
            BuildingKind::House => HOUSE_BUILD_TICKS,
            BuildingKind::Farm => FARM_BUILD_TICKS,
        }
    }

    /// Whether a completed (and under-construction) building of this
    /// kind blocks its footprint cells for pathing. Houses are solid;
    /// farms are ground a villager walks and works on.
    pub fn blocks_footprint(self) -> bool {
        match self {
            BuildingKind::House => true,
            BuildingKind::Farm => false,
        }
    }

    /// Population room granted on completion.
    pub fn pop_support(self) -> u32 {
        match self {
            BuildingKind::House => HOUSE_POP_SUPPORT,
            BuildingKind::Farm => 0,
        }
    }
}

/// One placed building, under construction or complete. Position is the
/// footprint's center in world coordinates; `cell_x`/`cell_y` are the
/// anchor (minimum) cell of the 2×2 footprint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Building {
    pub id: u32,
    pub kind: BuildingKind,
    pub cell_x: u32,
    pub cell_y: u32,
    pub pos_x: I16F16,
    pub pos_y: I16F16,
    /// Ticks of active building applied so far. The building is
    /// non-functional until `progress_ticks >= kind.build_ticks()`.
    pub progress_ticks: u64,
    pub complete: bool,
    /// For a completed Farm: the resource node it became. The building
    /// record stays as the footprint registry until the node depletes,
    /// then both retire together (see `Sim` construction phase).
    pub node_id: Option<u32>,
}

impl Building {
    /// The four footprint cells, in fixed (row-major) order.
    pub fn footprint_cells(&self) -> [(u32, u32); 4] {
        footprint_cells(self.cell_x, self.cell_y)
    }

    /// Build progress as a fraction of the kind's build time, in raw
    /// fixed-point bits via integer math (0 … 65536). Presentation aid
    /// for the ABI; the sim compares tick counts, never this value.
    pub fn progress_frac_bits(&self) -> i32 {
        let total = self.kind.build_ticks() as i64;
        let done = (self.progress_ticks as i64).min(total);
        ((done * 65536) / total) as i32
    }
}

/// The four cells of a 2×2 footprint anchored at `(cell_x, cell_y)`,
/// in fixed (row-major) order.
pub fn footprint_cells(cell_x: u32, cell_y: u32) -> [(u32, u32); 4] {
    [
        (cell_x, cell_y),
        (cell_x + 1, cell_y),
        (cell_x, cell_y + 1),
        (cell_x + 1, cell_y + 1),
    ]
}

/// The cells ringing a 2×2 footprint (the builder's standing spots),
/// in fixed row-major order: the 4×4 block around the footprint minus
/// the footprint itself. May contain out-of-grid cells; callers check
/// bounds.
pub fn ring_cells(cell_x: u32, cell_y: u32) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(12);
    for dy in 0..4i32 {
        for dx in 0..4i32 {
            let x = cell_x as i32 + dx - 1;
            let y = cell_y as i32 + dy - 1;
            if (1..=2).contains(&dx) && (1..=2).contains(&dy) {
                continue; // the footprint itself
            }
            if x < 0 || y < 0 {
                continue;
            }
            out.push((x as u32, y as u32));
        }
    }
    out
}

/// World coordinates of a footprint's center: the corner shared by its
/// four cells — `anchor + 1 cell` along each axis from the grid origin.
pub fn footprint_center(cell_x: u32, cell_y: u32) -> (I16F16, I16F16) {
    let x = I16F16::from_num(cell_x as i32 + 1) * CELL_SIZE - WORLD_HALF;
    let y = I16F16::from_num(cell_y as i32 + 1) * CELL_SIZE - WORLD_HALF;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_match_the_study() {
        assert_eq!(HOUSE_WOOD_COST, I16F16::from_num(25));
        assert_eq!(HOUSE_BUILD_TICKS, 25 * 60);
        assert_eq!(FARM_WOOD_COST, I16F16::from_num(60));
        assert_eq!(FARM_BUILD_TICKS, 15 * 60);
        assert_eq!(HOUSE_POP_SUPPORT, 5);
        assert_eq!(TOWN_CENTER_POP_SUPPORT, 5);
    }

    #[test]
    fn footprint_and_ring_are_the_right_shape() {
        let cells = footprint_cells(10, 10);
        assert_eq!(
            cells,
            [(10, 10), (11, 10), (10, 11), (11, 11)]
        );
        let ring = ring_cells(10, 10);
        assert_eq!(ring.len(), 12, "a 2x2 footprint has 12 ring cells");
        for c in ring {
            assert!(!cells.contains(&c), "ring cell inside footprint");
        }
        // Ring at the map corner: out-of-grid cells are dropped.
        assert!(ring_cells(0, 0).len() < 12);
    }

    #[test]
    fn footprint_center_is_the_shared_corner() {
        // Cell (0,0) spans [-100,-96); the 2x2 footprint there spans
        // [-100,-92) on both axes, so its center is (-96, -96).
        let (x, y) = footprint_center(0, 0);
        assert_eq!((x, y), (I16F16::from_num(-96), I16F16::from_num(-96)));
        // World center-ish: footprint (24,24) spans [-4, 4) → center 0.
        let (x, y) = footprint_center(24, 24);
        assert_eq!((x, y), (I16F16::from_num(0), I16F16::from_num(0)));
    }
}
