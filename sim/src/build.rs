//! Construction model (M2.5, M3, M6): buildings a villager raises on a
//! chosen patch of ground — Houses and Farms, the Barracks (M3), and
//! (M6) the economy and Feudal buildings: Lumber Camp, Mill, Mining
//! Camp, Archery Range, Stable, Blacksmith — plus the population
//! they support. BLD-13 adds the Palisade Wall: a 1×1 wall segment
//! (3 wood, 7 s, 150/250 HP by Age, 2/5 armor — the current DE row,
//! see `PALISADE_WALL_*`), placed in dragged lines.
//!
//! Numbers come from the public mechanics study
//! (`MECHANICS-STUDY.md` §5 and §9.4) and the OPENING-SPEC camps
//! table (§2.4), converted once, by hand, at authoring time:
//!
//! - House: 25 wood, 25 s build time (1500 ticks), +5 population room.
//! - Stone Wall (BLD-20): 5 stone per segment, 10 s build time
//!   (600 ticks), 1080/1800 HP by Age, 8/10 armor — the DE row in
//!   `~/workspace/skirmish-study/walls-de/DE-WALLS-DOSSIER.md` §2.
//! - Farm: 60 wood, 15 s build time (900 ticks), holds 175 food
//!   (§6.3 — the base amount; 250 is post-Horse-Collar), 3×3
//!   footprint (study §5; Claudio's decision 2026-10-22). M6 gates
//!   Farms on owning a completed Mill (OPENING-SPEC §2.4).
//! - Barracks (M3): 175 wood, 50 s build time (3000 ticks), 3×3
//!   footprint — the study's footprint, honored for this kind.
//! - Drop-off camps (M6): Lumber Camp / Mill / Mining Camp —
//!   100 wood, 35 s (2100 ticks), 2×2 each (OPENING-SPEC §2.4).
//!   Each accepts only its resources as a drop-off (ECO-06).
//! - Archery Range / Stable (M6): 175 wood, 50 s, 3×3, Feudal Age,
//!   require a standing Barracks (study §5). HP 1500, armor 1/8 —
//!   the study's Feudal row (the first of its age-keyed values).
//! - Blacksmith (M6): 150 wood (brief-pinned), Feudal Age, the
//!   generic tech host. Its build time (40 s), 3×3 footprint and
//!   1200 HP / 0/7 armor are NOT in the project docs — adopted
//!   values, flagged UNVERIFIED in the M6 report. The camps' HP
//!   (550) and armor (0/7) are likewise adopted (UNVERIFIED): the
//!   study's §5 table does not list the camps; the values follow
//!   its Dark-Age pattern for comparable economic buildings.
//! - Town Center: +5 population room (§5 function notes, §9.4).
//! - Multi-builder speed (GAP 4.18): total build time is
//!   T × 3/(n+2) for n builders — 1 builder takes T, 2 builders
//!   ≈ 0.75 T (the 25 s house in ≈19 s), 3 builders 0.6 T. Progress
//!   is therefore tracked in **thirds of a tick**, in which the rate
//!   is exact integer math: n builders add (n+2) thirds per tick.
//!
//! Placement is defined on the coarse grid: a building occupies a
//! square footprint anchored at its minimum cell — 2×2 cells for the
//! House and 3×3 for the Farm and the Barracks. (The Farm shipped
//! 2×2 through M6, a recorded divergence from the study's 3×3;
//! Claudio's decision of 2026-10-22 adopts the documented 3×3,
//! superseding the earlier 2×2 specification.) Houses and Barracks block their footprint
//! cells in the grid's dynamic layer (they are solid); farms do not
//! block — a farmer must be able to stand on the field to work it,
//! exactly as in the reference game.

use fixed::types::I16F16;

use crate::econ::ResourceKind;
use crate::entity::EntityKind;
use crate::grid::{CELL_SIZE, WORLD_HALF};
use crate::tech::{Age, ProdEntry};

/// Footprint edge length, in grid cells, of the 2×2 kinds
/// (House and the drop-off camps). The Farm and the Barracks are 3
/// (`BuildingKind::footprint_size`).
pub const FOOTPRINT_CELLS: u32 = 2;

/// House construction cost: 25 wood (study §5).
pub const HOUSE_WOOD_COST: I16F16 = I16F16::from_bits(1_638_400); // 25
/// House build time: 25 s at 60 Hz (study §5).
pub const HOUSE_BUILD_TICKS: u64 = 1500;
/// Farm construction cost: 60 wood (study §5).
pub const FARM_WOOD_COST: I16F16 = I16F16::from_bits(3_932_160); // 60
/// Farm build time: 15 s at 60 Hz (study §5).
pub const FARM_BUILD_TICKS: u64 = 900;
/// Barracks construction cost: 175 wood (study §5).
pub const BARRACKS_WOOD_COST: I16F16 = I16F16::from_bits(11_468_800); // 175
/// Barracks build time: 50 s at 60 Hz (study §5).
pub const BARRACKS_BUILD_TICKS: u64 = 3000;
/// Drop-off camp construction cost: 100 wood each — Lumber Camp,
/// Mill, Mining Camp (OPENING-SPEC §2.4).
pub const CAMP_WOOD_COST: I16F16 = I16F16::from_bits(6_553_600); // 100
/// Drop-off camp build time: 35 s at 60 Hz (OPENING-SPEC §2.4).
pub const CAMP_BUILD_TICKS: u64 = 2100;
/// Archery Range construction cost: 175 wood (study §5).
pub const ARCHERY_RANGE_WOOD_COST: I16F16 = I16F16::from_bits(11_468_800); // 175
/// Archery Range build time: 50 s at 60 Hz (study §5).
pub const ARCHERY_RANGE_BUILD_TICKS: u64 = 3000;
/// Stable construction cost: 175 wood (study §5).
pub const STABLE_WOOD_COST: I16F16 = I16F16::from_bits(11_468_800); // 175
/// Stable build time: 50 s at 60 Hz (study §5).
pub const STABLE_BUILD_TICKS: u64 = 3000;
/// Blacksmith construction cost: 150 wood (M6 brief).
pub const BLACKSMITH_WOOD_COST: I16F16 = I16F16::from_bits(9_830_400); // 150
/// Palisade Wall construction cost: 3 wood per segment (BLD-13;
/// AoE wiki, Palisade Wall infobox — 2 wood before DE update
/// 56005, 3 since; pinned CURRENT, 2026-10-22).
pub const PALISADE_WALL_WOOD_COST: I16F16 = I16F16::from_bits(196_608); // 3
/// Palisade Wall build time: 7 s per segment at 60 Hz (BLD-13;
/// wiki infobox — 5 s in The Age of Kings, 6 s from The
/// Forgotten, 7 s since DE update 42848).
pub const PALISADE_WALL_BUILD_TICKS: u64 = 420;
/// Stone Wall construction cost: 5 stone per segment (BLD-20;
/// AoE wiki, Stone Wall article — the current DE row).
pub const STONE_WALL_STONE_COST: I16F16 = I16F16::from_bits(327_680); // 5
/// Stone Wall build time: 10 s per segment at 60 Hz (BLD-20;
/// AoE wiki, Wall article's Building table).
pub const STONE_WALL_BUILD_TICKS: u64 = 600;
/// Blacksmith build time: 40 s at 60 Hz. UNVERIFIED — not in the
/// project docs (see the module docs).
pub const BLACKSMITH_BUILD_TICKS: u64 = 2400;
/// Watch Tower wood cost: 35 (BLD-18; the tower article's current
/// DE line — 125 stone / 35 wood since update 111772).
pub const WATCH_TOWER_WOOD_COST: I16F16 = I16F16::from_bits(2_293_760); // 35
/// Watch Tower stone cost: 125 (BLD-18; same source).
pub const WATCH_TOWER_STONE_COST: I16F16 = I16F16::from_bits(8_192_000); // 125
/// Watch Tower build time: 80 s at 60 Hz (BLD-18; tower article).
pub const WATCH_TOWER_BUILD_TICKS: u64 = 4800;
/// Outpost wood cost: 25 (BLD-18; Outpost article).
pub const OUTPOST_WOOD_COST: I16F16 = I16F16::from_bits(1_638_400); // 25
/// Outpost stone cost: 5 (BLD-18; Outpost article).
pub const OUTPOST_STONE_COST: I16F16 = I16F16::from_bits(327_680); // 5
/// Outpost build time: 15 s at 60 Hz (BLD-18; Outpost article).
pub const OUTPOST_BUILD_TICKS: u64 = 900;
/// Market construction cost: 175 wood (AoE wiki, Market infobox,
/// fetched 2026-10-22).
pub const MARKET_WOOD_COST: I16F16 = I16F16::from_bits(11_468_800); // 175
/// Market build time: 60 s at 60 Hz (AoE wiki, Market infobox).
pub const MARKET_BUILD_TICKS: u64 = 3600;
/// The Market's build range: its 4×4 footprint is 16 units wide, so
/// a builder on a ring cell can stand ~14.14 units from the center
/// (corner ring cell) — 15.5 covers the whole ring, in the geometry
/// of the two ranges below.
pub const MARKET_BUILD_RANGE: I16F16 = I16F16::from_bits(1_015_808); // 15.5
/// Population room from one completed House: +5 (study §5, §9.4).
pub const HOUSE_POP_SUPPORT: u32 = 5;
/// Population room from the Town Center: +5 (study §5, §9.4).
pub const TOWN_CENTER_POP_SUPPORT: u32 = 5;
/// A builder this close to the building's center is actively building:
/// 10.0 world units. Derived from the grid (not the study): a 2×2-cell
/// footprint is 8 units wide, so a builder standing on any cell of the
/// ring around it is at most ~8.5 units from the center — plus up to
/// 1.0 of steering arrival shortfall (a move order completes up to
/// WAYPOINT_REACH short of the ring cell's center). 9.0 did not cover
/// the corner ring cell plus that shortfall and could stall a builder
/// forever in an arrive/re-approach loop (found by M4's queued-build
/// test); 10.0 covers the whole ring with margin, like the Barracks
/// range below.
pub const BUILD_RANGE: I16F16 = I16F16::from_bits(655_360); // 10.0
/// The Barracks' build range: its 3×3 footprint is 12 units wide, so a
/// builder on a ring cell can stand ~11.3 units from the center
/// (corner ring cell) — 12.5 covers the whole ring.
pub const BARRACKS_BUILD_RANGE: I16F16 = I16F16::from_bits(819_200); // 12.5
/// The 1×1 kinds' build range (wall segments, Watch Tower,
/// Outpost): the builder's stand point sits 4.6 units from the
/// segment's center (see `Sim::approach_point` — the ring cell's
/// center itself livelocks a diagonal approach: steering
/// completes the order up to 1.0 short of it, stranding him up to
/// ≈6.66 out in an arrive/re-approach loop, the same stall the
/// BUILD_RANGE note records for 9.0). 5.8 covers the stand plus
/// the full 1.0 arrival shortfall with margin — and nothing
/// beyond the ring: the nearest point of any non-ring cell is
/// 6.0 units out, so a builder counted as building is provably
/// standing beside his segment. The generic 10.0 governed walls
/// until 2026-10-24, when Claudio's live play caught the defect:
/// a wall builder's walk order was cancelled up to 2.5 tiles from
/// his segment and he hammered from there, motionless — and a
/// lone villager could chain a whole line from one spot, the next
/// segment's center being only 4.0 away ("ele está muito longe e
/// não caminha").
pub const SMALL_BUILD_RANGE: I16F16 = I16F16::from_bits(380_109); // 5.8
/// Town Center construction cost: 275 wood + 100 stone (study §5;
/// wiki, Town Center (Age of Empires II)) — the first building
/// whose cost is not wood alone (see `BuildingKind::stone_cost`).
pub const TOWN_CENTER_WOOD_COST: I16F16 = I16F16::from_bits(18_022_400); // 275
/// Town Center construction stone cost: 100 (study §5).
pub const TOWN_CENTER_STONE_COST: I16F16 = I16F16::from_bits(6_553_600); // 100
/// Castle stone cost: 650 (Castle P3a; AoE wiki, Castle infobox
/// via the Castle Age dossier, fetched 2026-10-22) — the first
/// stone-priced building of the Castle Age, and the largest
/// single stone spend in the game.
pub const CASTLE_STONE_COST: I16F16 = I16F16::from_bits(42_598_400); // 650
/// Castle build time: 200 s at 60 Hz (Castle P3a; same source).
pub const CASTLE_BUILD_TICKS: u64 = 12000;
/// University wood cost: 200 (Castle P3c; AoE wiki, University
/// infobox via the Castle Age dossier, CASTLE-AGE-SPEC §5.5).
pub const UNIVERSITY_WOOD_COST: I16F16 = I16F16::from_bits(13_107_200); // 200
/// University build time: 60 s at 60 Hz (Castle P3c; same source).
pub const UNIVERSITY_BUILD_TICKS: u64 = 3600;
/// Siege Workshop wood cost: 200 (Castle P4a; AoE wiki, Siege
/// Workshop infobox via the Castle Age dossier, fetched
/// 2026-10-22).
pub const SIEGE_WORKSHOP_WOOD_COST: I16F16 = I16F16::from_bits(13_107_200); // 200
/// Siege Workshop build time: 40 s at 60 Hz (Castle P4a; same
/// source).
pub const SIEGE_WORKSHOP_BUILD_TICKS: u64 = 2400;
/// Population room from one completed Castle: +20 (Castle P3a;
/// same source).
pub const CASTLE_POP_SUPPORT: u32 = 20;
/// Town Center build time: 150 s at 60 Hz (study §5 — the
/// player-built figure; the 100 s techtree number is the
/// non-buildable Dark Age "stack" variant, per the study note).
pub const TOWN_CENTER_BUILD_TICKS: u64 = 9000;
/// The Town Center's build range: its 4×4 footprint is 16 units
/// wide, so a builder on a corner ring cell can stand ~14.1 units
/// from the center — 15.0 covers the whole ring.
pub const TOWN_CENTER_BUILD_RANGE: I16F16 = I16F16::from_bits(983_040); // 15.0

/// A buildable building kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildingKind {
    House,
    Farm,
    /// M3: trains Militia; solid 3×3 footprint. (Archers trained
    /// here in M3 by brief; M6 moves them to the Archery Range,
    /// where the reference trains them.)
    Barracks,
    /// M6: wood drop-off (ECO-06), Dark Age, 2×2.
    LumberCamp,
    /// M6: food drop-off and the Farm prerequisite (OPENING-SPEC
    /// §2.4), Dark Age, 2×2.
    Mill,
    /// M6: gold + stone drop-off, Dark Age, 2×2.
    MiningCamp,
    /// M6: trains Archers; Feudal Age, 3×3, requires a Barracks.
    ArcheryRange,
    /// M6: trains Scout Cavalry; Feudal Age, 3×3, requires a
    /// Barracks.
    Stable,
    /// M6: the generic technology host (Fletching and the armor
    /// lines); Feudal Age, 3×3.
    Blacksmith,
    /// BLD-13: one Palisade Wall segment — a 1×1 solid timber
    /// fence, Dark Age, placed in dragged lines (see
    /// `wall_line_cells` and `Command::BuildLine`). Blocks every
    /// player including its owner (only a gate passes, and gates
    /// do not exist yet).
    PalisadeWall,
    /// BLD-18: the first building that fights — Feudal Age, 1×1,
    /// stone-heavy, base arrow fire (garrison arrows are a recorded
    /// remainder; see `BuildingWeapon`).
    WatchTower,
    /// BLD-18: a pure line-of-sight structure — Dark Age, 1×1, no
    /// weapon; its sight radius is its whole reason to exist.
    Outpost,
    /// The Market: the commodity exchange (TRD-01..03). Feudal Age,
    /// requires a completed Mill, 4×4 footprint, 175 wood, 60 s
    /// build, 1800 HP and 1/8 armor in the Feudal column (AoE wiki,
    /// Market infobox, fetched 2026-10-22). The Castle/Imperial
    /// steps (2100 HP, 2/9 then 3/10 armor) land with the Castle
    /// Age package — the `Age` enum does not reach them yet, and
    /// the flat Feudal row is the only one this table can hold.
    Market,
    /// Castle Age P1: an additional Town Center. The kind exists
    /// only as a *construction record*: the Town Center is an
    /// entity in this architecture (BLD-16), so the tick the
    /// foundation completes, `Sim::complete_building` converts the
    /// record into a Town Center entity with its own production
    /// state and frees the footprint (a standing TC never blocks
    /// its ground). Castle Age only; 275 wood + 100 stone, 150 s,
    /// 4×4 (study §5; BLD-15's Castle extension).
    TownCenter,
    /// BLD-20: one Stone Wall segment — a 1×1 solid stone fence,
    /// Feudal Age, placed in dragged lines like the Palisade Wall
    /// (see `wall_line_cells` and `Command::BuildLine`). The DE
    /// row (walls dossier, 2026-10-23): 5 stone per segment,
    /// 10 s build, 1080 HP in Feudal / 1800 from Castle, 8/10
    /// armor. The Fortified Wall upgrade (University, Castle
    /// Age) is a recorded follow-up — there is no University to
    /// host the research yet (dossier §6).
    StoneWall,
    /// Castle P3a: the Castle — the Castle Age's namesake
    /// building. 650 stone (the game's first stone-priced
    /// building), 200 s build, 4×4, 4800 HP, armor 8/11, LoS 11,
    /// +20 population support; Castle Age, no building
    /// prerequisite. It trains the Petard (its only production)
    /// and fires on its own — the designed second user of the
    /// BLD-18 building-weapon machinery, with a base volley of
    /// 5 projectiles (`CASTLE_WEAPON`). Garrison (capacity 20,
    /// GAR-05 arrow scaling on top of the base volley) is the
    /// P3c slice, not this one.
    Castle,
    /// Castle P3c: the University — the Castle Age research
    /// building. 200 wood, 60 s build, 4×4, 2100 HP, armor 2/9,
    /// LoS 6; Castle Age, no building prerequisite (the Castle
    /// P3 dossier: CASTLE-AGE-SPEC §5.5 + the building table;
    /// AoE wiki, University infobox). Research only: it trains
    /// nothing and fires nothing. It hosts Ballistics, Murder
    /// Holes, Treadmill Crane, Masonry and the Fortified Wall
    /// upgrade (TechIds 20–24, `crate::tech`).
    University,
    /// Castle P4a: the Siege Workshop — the Castle Age's siege
    /// production building. 200 wood, 40 s build, **4×4** (the
    /// DE footprint is 4×4, not 3×3 — RISKS R8), 1800 HP in the
    /// Castle column (2100 Imperial), armor 2/9, LoS 6; Castle
    /// Age, and it requires a completed Blacksmith (BLD-15's
    /// chain). It trains the Battering Ram (the Mangonel and
    /// Scorpion join its roster in the P4b/P4c slices) and,
    /// like every production building, holds a production
    /// garrison of 10 (GAR-08). It does not fire — no
    /// `BuildingWeapon`.
    SiegeWorkshop,
}

impl BuildingKind {
    /// Stable tag for the state hash and the WASM building array:
    /// 0 = house, 1 = farm, 2 = barracks, 3 = lumber camp, 4 = mill,
    /// 5 = mining camp, 6 = archery range, 7 = stable, 8 = blacksmith,
    /// 9 = palisade wall, 10 = watch tower, 11 = outpost,
    /// 12 = market, 13 = town center (construction record only —
    /// see the variant), 14 = stone wall (BLD-20), 15 = castle
    /// (Castle P3a), 16 = university (Castle P3c), 17 = siege
    /// workshop (Castle P4a). Tags are an ABI contract — append
    /// only, never renumber.
    pub fn tag(self) -> u8 {
        match self {
            BuildingKind::House => 0,
            BuildingKind::Farm => 1,
            BuildingKind::Barracks => 2,
            BuildingKind::LumberCamp => 3,
            BuildingKind::Mill => 4,
            BuildingKind::MiningCamp => 5,
            BuildingKind::ArcheryRange => 6,
            BuildingKind::Stable => 7,
            BuildingKind::Blacksmith => 8,
            BuildingKind::PalisadeWall => 9,
            BuildingKind::WatchTower => 10,
            BuildingKind::Outpost => 11,
            BuildingKind::Market => 12,
            BuildingKind::TownCenter => 13,
            BuildingKind::StoneWall => 14,
            BuildingKind::Castle => 15,
            BuildingKind::University => 16,
            BuildingKind::SiegeWorkshop => 17,
        }
    }

    /// Inverse of `tag` (the AI and tests name kinds by tag).
    pub fn from_tag(tag: u8) -> Option<BuildingKind> {
        match tag {
            0 => Some(BuildingKind::House),
            1 => Some(BuildingKind::Farm),
            2 => Some(BuildingKind::Barracks),
            3 => Some(BuildingKind::LumberCamp),
            4 => Some(BuildingKind::Mill),
            5 => Some(BuildingKind::MiningCamp),
            6 => Some(BuildingKind::ArcheryRange),
            7 => Some(BuildingKind::Stable),
            8 => Some(BuildingKind::Blacksmith),
            9 => Some(BuildingKind::PalisadeWall),
            10 => Some(BuildingKind::WatchTower),
            11 => Some(BuildingKind::Outpost),
            12 => Some(BuildingKind::Market),
            13 => Some(BuildingKind::TownCenter),
            14 => Some(BuildingKind::StoneWall),
            15 => Some(BuildingKind::Castle),
            16 => Some(BuildingKind::University),
            17 => Some(BuildingKind::SiegeWorkshop),
            _ => None,
        }
    }

    /// Whether this kind is a wall segment (BLD-13, BLD-20): 1×1,
    /// solid to everyone, placed by dragged lines. The Palisade
    /// Wall (Dark Age, wood) and the Stone Wall (Feudal Age,
    /// stone).
    pub fn is_wall(self) -> bool {
        matches!(self, BuildingKind::PalisadeWall | BuildingKind::StoneWall)
    }

    /// Wood paid at placement time.
    pub fn wood_cost(self) -> I16F16 {
        match self {
            BuildingKind::House => HOUSE_WOOD_COST,
            BuildingKind::Farm => FARM_WOOD_COST,
            BuildingKind::Barracks => BARRACKS_WOOD_COST,
            BuildingKind::LumberCamp | BuildingKind::Mill | BuildingKind::MiningCamp => {
                CAMP_WOOD_COST
            }
            BuildingKind::ArcheryRange => ARCHERY_RANGE_WOOD_COST,
            BuildingKind::Stable => STABLE_WOOD_COST,
            BuildingKind::Blacksmith => BLACKSMITH_WOOD_COST,
            BuildingKind::PalisadeWall => PALISADE_WALL_WOOD_COST,
            BuildingKind::WatchTower => WATCH_TOWER_WOOD_COST,
            BuildingKind::Outpost => OUTPOST_WOOD_COST,
            BuildingKind::Market => MARKET_WOOD_COST,
            BuildingKind::TownCenter => TOWN_CENTER_WOOD_COST,
            // BLD-20: the Stone Wall's price is all stone.
            BuildingKind::StoneWall => I16F16::ZERO,
            // Castle P3a: the Castle's price is all stone too.
            BuildingKind::Castle => I16F16::ZERO,
            // Castle P3c: the University's 200 wood.
            BuildingKind::University => UNIVERSITY_WOOD_COST,
            // Castle P4a: the Siege Workshop's 200 wood.
            BuildingKind::SiegeWorkshop => SIEGE_WORKSHOP_WOOD_COST,
        }
    }

    /// Stone paid at placement time (BLD-18: the first kinds that
    /// cost stone at all — the Watch Tower's 125 is the reason stone
    /// exists in the Feudal economy; the Town Center's 100 (Castle
    /// P1) joins them as the third stone-cost kind; every other
    /// kind reads 0, so the placement/repair/refund paths that sum
    /// both resources are behavior-identical for them).
    pub fn stone_cost(self) -> I16F16 {
        match self {
            BuildingKind::WatchTower => WATCH_TOWER_STONE_COST,
            BuildingKind::Outpost => OUTPOST_STONE_COST,
            BuildingKind::TownCenter => TOWN_CENTER_STONE_COST,
            // BLD-20: 5 stone per segment (dossier §2).
            BuildingKind::StoneWall => STONE_WALL_STONE_COST,
            // Castle P3a: 650 stone (dossier §2).
            BuildingKind::Castle => CASTLE_STONE_COST,
            _ => I16F16::ZERO,
        }
    }

    /// Total placement cost across resources (VIC-04's razing
    /// credit unit: resources, summed).
    pub fn total_cost(self) -> I16F16 {
        self.wood_cost() + self.stone_cost()
    }

    /// Ticks of active building needed to complete.
    pub fn build_ticks(self) -> u64 {
        match self {
            BuildingKind::House => HOUSE_BUILD_TICKS,
            BuildingKind::Farm => FARM_BUILD_TICKS,
            BuildingKind::Barracks => BARRACKS_BUILD_TICKS,
            BuildingKind::LumberCamp | BuildingKind::Mill | BuildingKind::MiningCamp => {
                CAMP_BUILD_TICKS
            }
            BuildingKind::ArcheryRange => ARCHERY_RANGE_BUILD_TICKS,
            BuildingKind::Stable => STABLE_BUILD_TICKS,
            BuildingKind::Blacksmith => BLACKSMITH_BUILD_TICKS,
            BuildingKind::PalisadeWall => PALISADE_WALL_BUILD_TICKS,
            BuildingKind::WatchTower => WATCH_TOWER_BUILD_TICKS,
            BuildingKind::Outpost => OUTPOST_BUILD_TICKS,
            BuildingKind::Market => MARKET_BUILD_TICKS,
            BuildingKind::TownCenter => TOWN_CENTER_BUILD_TICKS,
            BuildingKind::StoneWall => STONE_WALL_BUILD_TICKS,
            BuildingKind::Castle => CASTLE_BUILD_TICKS,
            BuildingKind::University => UNIVERSITY_BUILD_TICKS,
            BuildingKind::SiegeWorkshop => SIEGE_WORKSHOP_BUILD_TICKS,
        }
    }

    /// Total construction progress needed to complete, in thirds of
    /// a tick (`build_ticks() * 3`) — the unit `Building` progress is
    /// accumulated in, so the multi-builder rate below stays exact.
    pub fn build_thirds(self) -> u64 {
        self.build_ticks() * 3
    }

    /// Whether a completed (and under-construction) building of this
    /// kind blocks its footprint cells for pathing. Houses and the
    /// Barracks are solid; farms are ground a villager walks and
    /// works on. A Town Center *construction record* blocks like
    /// any foundation; the standing Town Center it converts into
    /// never does (BLD-16 pass-through) — the conversion frees the
    /// cells.
    pub fn blocks_footprint(self) -> bool {
        match self {
            BuildingKind::Farm => false,
            _ => true,
        }
    }

    /// Population room granted on completion.
    pub fn pop_support(self) -> u32 {
        match self {
            BuildingKind::House => HOUSE_POP_SUPPORT,
            // Castle P3a: the Castle supports +20 (dossier §2).
            BuildingKind::Castle => CASTLE_POP_SUPPORT,
            _ => 0,
        }
    }

    /// Footprint edge length in grid cells: 2 for the House and the
    /// camps, 3 for the Farm, the Barracks and the Feudal buildings
    /// (the study's footprints where it gives them; the Blacksmith's
    /// 3×3 is adopted — UNVERIFIED, see the module docs). The Farm's
    /// 3×3 is Claudio's decision of 2026-10-22, adopting the
    /// DE-documented footprint over the earlier 2×2 specification),
    /// and 4 for the Town Center (study §5).
    pub fn footprint_size(self) -> u32 {
        match self {
            BuildingKind::PalisadeWall
            | BuildingKind::StoneWall
            | BuildingKind::WatchTower
            | BuildingKind::Outpost => 1,
            BuildingKind::House
            | BuildingKind::LumberCamp
            | BuildingKind::Mill
            | BuildingKind::MiningCamp => FOOTPRINT_CELLS,
            BuildingKind::Farm
            | BuildingKind::Barracks
            | BuildingKind::ArcheryRange
            | BuildingKind::Stable
            | BuildingKind::Blacksmith => 3,
            // The placed 4×4 kinds: the Market, the Town Center
            // construction record (the standing Town Center is an
            // entity, not a kind — BLD-16), the Castle
            // (Castle P3a, dossier §2), the University
            // (Castle P3c, dossier §5.5) and the Siege Workshop
            // (Castle P4a, RISKS R8: the DE footprint is 4×4).
            BuildingKind::Market
            | BuildingKind::TownCenter
            | BuildingKind::Castle
            | BuildingKind::University
            | BuildingKind::SiegeWorkshop => 4,
        }
    }

    /// A builder this close to the building's center is actively
    /// building (see the range constants for the geometry).
    pub fn build_range(self) -> I16F16 {
        if self == BuildingKind::Market {
            // The 4×4 kinds keep their own ranges (Market 15.5,
            // Town Center 15.0 — see the constants' geometry notes).
            MARKET_BUILD_RANGE
        } else if self == BuildingKind::TownCenter
            || self == BuildingKind::Castle
            || self == BuildingKind::University
            || self == BuildingKind::SiegeWorkshop
        {
            // The Castle is the third 4×4 kind (Castle P3a): the
            // Town Center's 15.0 covers its ring by the same
            // geometry (corner ring cell ≈14.1 from the center).
            // The University (Castle P3c) is the fourth, the
            // Siege Workshop (Castle P4a) the fifth.
            TOWN_CENTER_BUILD_RANGE
        } else if self.footprint_size() == 3 {
            BARRACKS_BUILD_RANGE
        } else if self.footprint_size() == 1 {
            // Walls, Watch Tower, Outpost: the builder must stand
            // beside the segment (see SMALL_BUILD_RANGE).
            SMALL_BUILD_RANGE
        } else {
            BUILD_RANGE
        }
    }

    /// Whether this building trains units (a production building —
    /// it also holds a rally point, SEL-13). The Market joins with
    /// Package D: it trains the Trade Cart (TRD-05), and the
    /// reference game's Markets hold rally points like every other
    /// producing building.
    pub fn is_production(self) -> bool {
        matches!(
            self,
            BuildingKind::Barracks
                | BuildingKind::ArcheryRange
                | BuildingKind::Stable
                | BuildingKind::Castle
                | BuildingKind::Market
                | BuildingKind::SiegeWorkshop
        )
    }

    /// The unit kind this building trains, if any (M6): the
    /// per-building half of the trainable relation — the Barracks
    /// trains Militia, the Archery Range trains Archers, the
    /// Stable trains Scout Cavalry. The Spearman (Barracks) and
    /// Skirmisher (Archery Range) join their buildings' rosters;
    /// their Feudal availability is the unit-level half of the
    /// rule (`EntityKind::train_age`) — the Barracks itself
    /// stands from the Dark Age. Castle P2 adds the Knight
    /// (Stable) and the Cavalry Archer (Archery Range) the same
    /// way, gated by their Castle `train_age`.
    pub fn trains(self, kind: EntityKind) -> bool {
        matches!(
            (self, kind),
            (BuildingKind::Barracks, EntityKind::Militia)
                | (BuildingKind::Barracks, EntityKind::Spearman)
                | (BuildingKind::ArcheryRange, EntityKind::Archer)
                | (BuildingKind::ArcheryRange, EntityKind::Skirmisher)
                | (BuildingKind::ArcheryRange, EntityKind::CavalryArcher)
                | (BuildingKind::Stable, EntityKind::ScoutCavalry)
                | (BuildingKind::Stable, EntityKind::Knight)
                // Castle P3a: the Petard trains at the Castle
                // only (the Castle's whole generic roster).
                | (BuildingKind::Castle, EntityKind::Petard)
                // Package D: the Trade Cart trains at the Market
                // only (TRD-05; wiki, "Trade Cart").
                | (BuildingKind::Market, EntityKind::TradeCart)
                // Castle P4a: the Battering Ram trains at the
                // Siege Workshop only (the Mangonel and Scorpion
                // join this roster in the P4b/P4c slices, when
                // their kinds exist).
                | (BuildingKind::SiegeWorkshop, EntityKind::BatteringRam)
        )
    }

    /// The age a player must stand in to place this building
    /// (BLD-15: availability keys on the owner's age and built set).
    pub fn age_required(self) -> Age {
        match self {
            BuildingKind::ArcheryRange
            | BuildingKind::Stable
            | BuildingKind::Blacksmith
            | BuildingKind::WatchTower
            | BuildingKind::Market
            | BuildingKind::StoneWall => Age::Feudal,
            BuildingKind::TownCenter
            | BuildingKind::Castle
            | BuildingKind::University
            | BuildingKind::SiegeWorkshop => Age::Castle,
            _ => Age::Dark,
        }
    }

    /// Line of sight in tiles, keyed on the OWNER's age (VIS-01's
    /// building stamp): the generic placed building reads the
    /// study-adopted `BUILDING_SIGHT_TILES` (4); the two BLD-18
    /// structures are the exceptions their designs call for.
    ///
    /// - Watch Tower 10: ADOPTED — UNVERIFIED. The fetched tower
    ///   sources pin range 8 but no sight figure; 10 follows the
    ///   defensive-building pattern of sight = range + 2 (the
    ///   Castle's documented 8 range pairs with double-digit sight
    ///   in every secondary account).
    /// - Outpost 6 in the Dark Age, 8 from Feudal: the Outpost
    ///   article states its line of sight "increases with each
    ///   Age" without numbers in the fetched text; the step values
    ///   are ADOPTED — UNVERIFIED, chosen so the Dark-Age outpost
    ///   already out-sees every ordinary building (its whole
    ///   purpose) and the Feudal step matches the Town Center's
    ///   documented 8.
    pub fn sight_tiles_at(self, age: Age) -> u32 {
        match self {
            // BLD-20: walls see 2 tiles in DE (the wiki Wall
            // article's Building table, both wall rows) — the
            // palisade shipped with the generic building sight 4;
            // this arm gives both wall kinds their DE row.
            BuildingKind::PalisadeWall | BuildingKind::StoneWall => 2,
            // Castle P3a: the Castle's documented sight is 11
            // (dossier §2, wiki Castle infobox).
            BuildingKind::Castle => 11,
            // Castle P3c: the University's documented sight is 6
            // (dossier §5.5 building row, wiki University infobox).
            BuildingKind::University => 6,
            // Castle P4a: the Siege Workshop's documented sight
            // is 6 (dossier §2, wiki Siege Workshop infobox).
            BuildingKind::SiegeWorkshop => 6,
            BuildingKind::WatchTower => 10,
            BuildingKind::Outpost => match age {
                Age::Dark => 6,
                // Castle holds the Feudal step (ADOPTED — the
                // article's per-Age numbers stay UNVERIFIED; the
                // Castle rung exists since Castle Age P1).
                Age::Feudal | Age::Castle | Age::Imperial => 8,
            },
            _ => crate::BUILDING_SIGHT_TILES,
        }
    }

    /// The building weapon this kind carries, if any (BLD-18):
    /// only the Watch Tower fires. The returned spec is the BASE
    /// profile; researched patches (Fletching, TEC-06-style) layer
    /// on top in `Sim::building_weapon`.
    pub fn weapon(self) -> Option<BuildingWeapon> {
        match self {
            BuildingKind::WatchTower => Some(WATCH_TOWER_WEAPON),
            // Castle P3a: the designed second user ships — the
            // Castle fires its base volley of 5 (GAR-05's extra
            // garrison arrows layer on in P3c, not here).
            BuildingKind::Castle => Some(CASTLE_WEAPON),
            _ => None,
        }
    }

    /// Garrison capacity (Castle P3b, GAR-01 — the resolved
    /// per-building data column, scope 2026-10-29): Town Centers
    /// hold 15 (the TC is an entity — its capacity lives in
    /// `crate::TC_GARRISON_CAPACITY`); towers 5; the Castle 20;
    /// production buildings 10 (production garrison only — entry
    /// solely as the rally/newly-trained case, GAR-08). `None` =
    /// not garrisonable at all.
    pub fn garrison_capacity(self) -> Option<u32> {
        match self {
            BuildingKind::WatchTower | BuildingKind::Outpost => Some(5),
            BuildingKind::Castle => Some(20),
            BuildingKind::Barracks
            | BuildingKind::ArcheryRange
            | BuildingKind::Stable
            | BuildingKind::SiegeWorkshop => Some(10),
            _ => None,
        }
    }

    /// Whether this kind's garrison is a DEFENSIVE garrison
    /// (Castle P3b, GAR-01): units may be ordered inside and may
    /// re-enter. Production buildings are the other type: eject
    /// freely, never re-enter (GAR-08).
    pub fn is_defensive_garrison(self) -> bool {
        matches!(
            self,
            BuildingKind::WatchTower | BuildingKind::Outpost | BuildingKind::Castle
        )
    }

    /// Whether this kind's garrison is production-only (GAR-08).
    pub fn is_production_garrison(self) -> bool {
        matches!(
            self,
            BuildingKind::Barracks
                | BuildingKind::ArcheryRange
                | BuildingKind::Stable
                | BuildingKind::SiegeWorkshop
        )
    }

    /// Whether a unit kind may garrison in this building as a
    /// DEFENSIVE garrison (Castle P3b, GAR-01): foot units in
    /// towers; foot + mounted in the Castle. Production buildings
    /// answer false here — their entry is the rally case, checked
    /// separately (`garrison_capacity` + the spawn path).
    pub fn admits_defensive(self, kind: crate::entity::EntityKind) -> bool {
        match self {
            BuildingKind::WatchTower | BuildingKind::Outpost => kind.is_foot_garrisonable(),
            BuildingKind::Castle => {
                kind.is_foot_garrisonable() || kind.is_mounted_garrisonable()
            }
            _ => false,
        }
    }

    /// Shelter healing rate inside this building, HP per minute
    /// (Castle P3b, GAR-04 base): towers 0.1 HP/s = 6/min; the
    /// Castle 0.2 HP/s = 12/min; production garrisons heal nothing
    /// ("healing inside other buildings: none" — GAR-04). The Town
    /// Center's 6/min lives with its weapon/capacity constants in
    /// lib.rs. Herbal Medicine's ×6 is Castle P5.
    pub fn garrison_heal_hp_per_min(self) -> i64 {
        match self {
            BuildingKind::WatchTower | BuildingKind::Outpost => 6,
            BuildingKind::Castle => 12,
            _ => 0,
        }
    }

    /// A building that must already stand *completed* before this
    /// kind can be placed (BLD-15's chain; study §5): the Archery
    /// Range and the Stable both require a Barracks.
    pub fn requires_building(self) -> Option<BuildingKind> {
        match self {
            BuildingKind::ArcheryRange | BuildingKind::Stable => Some(BuildingKind::Barracks),
            // BLD-15's chain: the Mill gates the Market.
            BuildingKind::Market => Some(BuildingKind::Mill),
            // Castle P4a (BLD-15 extension): the Siege Workshop
            // requires a completed Blacksmith (dossier §2).
            BuildingKind::SiegeWorkshop => Some(BuildingKind::Blacksmith),
            _ => None,
        }
    }

    /// Whether this building is a resource drop-off point (M6,
    /// ECO-06): the three camps. The Town Center is the other
    /// drop-off — it is an entity, not a placed building, and
    /// accepts everything.
    pub fn is_dropoff(self) -> bool {
        matches!(
            self,
            BuildingKind::LumberCamp | BuildingKind::Mill | BuildingKind::MiningCamp
        )
    }

    /// Whether a completed building of this kind accepts `resource`
    /// as a deposit (ECO-06's per-resource validity): Lumber Camp
    /// wood, Mill food, Mining Camp gold + stone; every other kind
    /// accepts nothing.
    pub fn accepts(self, resource: ResourceKind) -> bool {
        match self {
            BuildingKind::LumberCamp => resource == ResourceKind::Wood,
            BuildingKind::Mill => resource == ResourceKind::Food,
            BuildingKind::MiningCamp => {
                resource == ResourceKind::Gold || resource == ResourceKind::Stone
            }
            _ => false,
        }
    }

    /// Whether this kind can produce units (VIC-01's "building
    /// capable of producing units"): the Barracks, Archery Range
    /// and Stable. The Town Center also produces, but it is an
    /// entity in this architecture, not a placed building — the
    /// elimination predicate checks it separately. A Town Center
    /// *foundation* counts here on the same footing as the other
    /// production foundations (VIC-01 counts placed production
    /// buildings at any construction stage): a player raising a
    /// new Town Center has not been eliminated.
    pub fn produces_units(self) -> bool {
        matches!(
            self,
            BuildingKind::Barracks
                | BuildingKind::ArcheryRange
                | BuildingKind::Stable
                | BuildingKind::TownCenter
                | BuildingKind::Castle
                | BuildingKind::SiegeWorkshop
        )
    }

    /// Whether this kind counts toward the Feudal Age research
    /// requirement (TEC-01): any completed Dark-Age building other
    /// than Houses and Farms. Walls never count (BLD-13 — the
    /// reference excludes walls and towers from age-up building
    /// requirements, the same exclusion the Castle-Age dossier
    /// records for the next rung), and neither do the Watch Tower
    /// or the Outpost (BLD-18 — defensive structures are not
    /// age-up buildings).
    pub fn counts_toward_feudal(self) -> bool {
        matches!(
            self,
            BuildingKind::Barracks
                | BuildingKind::LumberCamp
                | BuildingKind::Mill
                | BuildingKind::MiningCamp
        )
    }

    /// Whether this kind counts toward the Castle Age research
    /// requirement (TEC-01 Castle rung; study §7): any completed
    /// building *of the Feudal Age* — the kinds whose availability
    /// begins in Feudal (walls and towers excluded, the same
    /// exclusion `counts_toward_feudal` records). The reading
    /// mirrors `counts_toward_feudal` one rung up: the Barracks
    /// counted toward Feudal because it is a Dark-Age building;
    /// being Dark-Age-born, it does NOT count toward Castle — the
    /// qualifying set is the Feudal-born kinds (Range, Stable,
    /// Blacksmith, and the Market since BLD-19 landed it; the
    /// Dock is Feudal-born too but does not exist in this roster).
    pub fn counts_toward_castle(self) -> bool {
        matches!(
            self,
            BuildingKind::ArcheryRange
                | BuildingKind::Stable
                | BuildingKind::Blacksmith
                | BuildingKind::Market
        )
    }

    /// Whether this kind counts toward the Imperial Age research
    /// requirement (TEC-02's Imperial clause): the Castle-born
    /// kinds. TEC-02's gate itself is "a Castle alone suffices,
    /// or any two Castle-Age buildings" — the Castle is the only
    /// Castle-born kind in the current roster; the University,
    /// Siege Workshop and Monastery join this set when they land.
    pub fn counts_toward_imperial(self) -> bool {
        // Castle P3c: the University joins the set, exactly as
        // this column's doc above anticipated ("the University,
        // Siege Workshop and Monastery join this set when they
        // land"). Castle P4a: the Siege Workshop joins it the
        // same way; the Monastery remains.
        matches!(
            self,
            BuildingKind::Castle | BuildingKind::University | BuildingKind::SiegeWorkshop
        )
    }

    /// Hit points of a fresh building of this kind (M5 — buildings
    /// take damage now; study §5, Dark Age row where the value is
    /// age-keyed). Death is HP < 1, the entity rule (CMB-02).
    /// M6 kinds: Range/Stable 1500 (study §5, Feudal row); the
    /// camps' 550 and the Blacksmith's 1200 are adopted values —
    /// UNVERIFIED (see the module docs).
    pub fn max_hp(self) -> i32 {
        match self {
            BuildingKind::House => 550,
            BuildingKind::Farm => 480,
            BuildingKind::Barracks => 1200,
            BuildingKind::LumberCamp | BuildingKind::Mill | BuildingKind::MiningCamp => 550,
            BuildingKind::ArcheryRange | BuildingKind::Stable => 1500,
            BuildingKind::Blacksmith => 1200,
            // BLD-13: the Dark-Age column of the wiki row
            // (150/250/250/250); the Feudal step lives in
            // `max_hp_at`.
            BuildingKind::PalisadeWall => 150,
            // BLD-20: the Feudal column of the wiki Stone Wall
            // row (1080/1800 — the Feudal step is DE update
            // 66692); the Castle step lives in `max_hp_at`. The
            // kind is Feudal-born, so this base row is the
            // Feudal one.
            BuildingKind::StoneWall => 1080,
            // BLD-18: the Watch Tower's Feudal row — 850 HP before
            // the Castle Age (update 61321 raised the pre-Castle
            // value to 850; the Castle step to 1020 lands with the
            // Castle Age column, when the `Age` enum reaches it).
            BuildingKind::WatchTower => 850,
            // BLD-18: the Outpost's flat 500 (Outpost article).
            BuildingKind::Outpost => 500,
            // Feudal column of the wiki's Market table (Castle and
            // Imperial both read 2100 — see the kind's doc).
            BuildingKind::Market => 1800,
            // The Town Center entity's own row (study §5): 2400 at
            // every Age — the construction record shares it.
            BuildingKind::TownCenter => 2400,
            // Castle P3a: 4800 HP (dossier §2, wiki Castle
            // infobox) — flat; the kind is Castle-born.
            BuildingKind::Castle => 4800,
            // Castle P3c: 2100 HP (dossier §5.5, wiki University
            // infobox) — flat; the kind is Castle-born.
            BuildingKind::University => 2100,
            // Castle P4a: the Castle column, 1800 HP (dossier
            // §2, wiki Siege Workshop infobox); the Imperial
            // step to 2100 lives in `max_hp_at`.
            BuildingKind::SiegeWorkshop => 1800,
        }
    }

    /// Hit points at the owner's current Age (BLD-11): a building's
    /// HP follows its owner's Age column in the data table with no
    /// research and no player action. The study (§5) pins the
    /// per-age rows: House 550/750/**900** (Dark/Feudal/Castle),
    /// Barracks 1200/1500/**1800**, Archery Range and Stable
    /// 1500 (Feudal-born) → **1800** in Castle. The Town Center is
    /// 2400 at every Age; the camps and the Blacksmith keep their
    /// flat adopted rows at every age (no per-age data is pinned
    /// for them; nothing is invented — their Castle columns stay
    /// UNVERIFIED). The maximum is a *lookup* (TEC-04): a standing
    /// building's current HP is never rewritten by an age advance —
    /// the max rises around the damage already taken.
    pub fn max_hp_at(self, age: Age) -> i32 {
        match (self, age) {
            (BuildingKind::House, Age::Feudal) => 750,
            (BuildingKind::House, Age::Castle | Age::Imperial) => 900,
            (BuildingKind::Barracks, Age::Feudal) => 1500,
            // BLD-13: 150 HP in the Dark Age, 250 from Feudal on
            // (wiki Palisade Wall row 150/250/250/250 — the step
            // is DE update 44725; BLD-11's lookup semantics apply:
            // a standing segment's max rises with the age, its
            // current HP is untouched). Written as "not Dark"
            // rather than naming Feudal so the row stays right
            // when the Castle Age lands (250 there too).
            (BuildingKind::PalisadeWall, age) if age != Age::Dark => 250,
            // BLD-20: the Stone Wall's Castle step (wiki row
            // 1080 Feudal / 1800 Castle+).
            (BuildingKind::StoneWall, Age::Castle | Age::Imperial) => 1800,
            (BuildingKind::Barracks, Age::Castle | Age::Imperial) => 1800,
            (BuildingKind::ArcheryRange, Age::Castle | Age::Imperial) => 1800,
            (BuildingKind::Stable, Age::Castle | Age::Imperial) => 1800,
            // The Castle steps BLD-18/BLD-19 deferred to this
            // package: Watch Tower 850 → 1020 (tower article,
            // update 61321), Market 1800 → 2100 (Market infobox).
            (BuildingKind::WatchTower, Age::Castle | Age::Imperial) => 1020,
            (BuildingKind::Market, Age::Castle | Age::Imperial) => 2100,
            // Castle P4a: the Siege Workshop's Imperial step
            // (dossier §2: 1800 Castle / 2100 Imperial).
            (BuildingKind::SiegeWorkshop, Age::Imperial) => 2100,
            _ => self.max_hp(),
        }
    }

    /// Melee armor (study §5, Dark Age row; Feudal row for the
    /// M6 Feudal buildings). The House's is *negative* in the Dark
    /// Age — early houses are soft to melee, exactly as the
    /// study's table records. The camps' and Blacksmith's values
    /// are adopted — UNVERIFIED (see the module docs). The Town
    /// Center construction record carries the entity's Dark row
    /// (3); its per-Age steps live in `armor_melee_at`.
    pub fn armor_melee(self) -> i32 {
        match self {
            BuildingKind::House => -2,
            // The Watch Tower's 1 melee is its documented row
            // (BLD-18; tower article statistics).
            BuildingKind::ArcheryRange
            | BuildingKind::Stable
            | BuildingKind::WatchTower
            | BuildingKind::Market => 1,
            // BLD-13: palisade melee armor 2 (wiki infobox; flat
            // across ages).
            BuildingKind::PalisadeWall => 2,
            // BLD-20: stone wall melee armor 8 (wiki Building
            // table; flat across ages).
            BuildingKind::StoneWall => 8,
            // Castle P3a: the Castle's melee armor is 8
            // (dossier §2, wiki Castle infobox).
            BuildingKind::Castle => 8,
            // Castle P3c: the University's melee armor is 2
            // (dossier §5.5, wiki University infobox).
            BuildingKind::University => 2,
            // Castle P4a: the Siege Workshop's melee armor is 2
            // (dossier §2, wiki Siege Workshop infobox).
            BuildingKind::SiegeWorkshop => 2,
            BuildingKind::TownCenter => 3,
            _ => 0,
        }
    }

    /// Melee armor at the owner's current Age (BLD-11, study §5):
    /// House −2 Dark → −1 Feudal → **1 Castle** (the wiki House
    /// changelog, update 56005: "−2/−1/1 melee armor in the
    /// Dark/Feudal/Castle Age"); Barracks 0 → 1 → **2**; Range
    /// and Stable 1 → **2**; Town Center 3 → 4 → **5** (study §5's
    /// 3/4/5/6 progression). Other kinds keep their flat row.
    pub fn armor_melee_at(self, age: Age) -> i32 {
        match (self, age) {
            (BuildingKind::House, Age::Feudal) => -1,
            (BuildingKind::House, Age::Castle | Age::Imperial) => 1,
            (BuildingKind::Barracks, Age::Feudal) => 1,
            (BuildingKind::Barracks, Age::Castle | Age::Imperial) => 2,
            (BuildingKind::ArcheryRange, Age::Castle | Age::Imperial) => 2,
            (BuildingKind::Stable, Age::Castle | Age::Imperial) => 2,
            // Market Castle step (BLD-19's deferred column: 2/9).
            (BuildingKind::Market, Age::Castle) => 2,
            (BuildingKind::TownCenter, Age::Feudal) => 4,
            (BuildingKind::TownCenter, Age::Castle) => 5,
            // BLD-19: the Market's Imperial step (3 melee).
            (BuildingKind::Market, Age::Imperial) => 3,
            (BuildingKind::TownCenter, Age::Imperial) => 6,
            _ => self.armor_melee(),
        }
    }

    /// Pierce armor (study §5, Dark/Feudal rows as above). The
    /// Town Center construction record carries the entity's Dark
    /// row (5); its per-Age steps live in `armor_pierce_at`.
    pub fn armor_pierce(self) -> i32 {
        match self {
            BuildingKind::House => 7,
            BuildingKind::Farm => 0,
            BuildingKind::ArcheryRange | BuildingKind::Stable | BuildingKind::Market => 8,
            // BLD-13: palisade pierce armor 5 (wiki infobox; 2 in
            // The Age of Kings, 5 since The Conquerors patch 1.0b).
            BuildingKind::PalisadeWall => 5,
            // BLD-20: stone wall pierce armor 10 (wiki Building
            // table; flat across ages).
            BuildingKind::StoneWall => 10,
            // Castle P3a: the Castle's pierce armor is 11
            // (dossier §2, wiki Castle infobox).
            BuildingKind::Castle => 11,
            // Castle P3c: the University's pierce armor is 9
            // (dossier §5.5, wiki University infobox).
            BuildingKind::University => 9,
            // Castle P4a: the Siege Workshop's pierce armor is
            // 9 (dossier §2, wiki Siege Workshop infobox).
            BuildingKind::SiegeWorkshop => 9,
            BuildingKind::TownCenter => 5,
            _ => 7,
        }
    }

    /// Pierce armor at the owner's current Age (BLD-11, study §5):
    /// House 7 → 8 → **9**, Barracks 7 → 8 → **9**, Range and
    /// Stable 8 → **9**, Town Center 5 → 6 → **7** (study §5's
    /// progressions). Other kinds keep their flat row.
    pub fn armor_pierce_at(self, age: Age) -> i32 {
        match (self, age) {
            (BuildingKind::House, Age::Feudal) => 8,
            (BuildingKind::House, Age::Castle | Age::Imperial) => 9,
            (BuildingKind::Barracks, Age::Feudal) => 8,
            (BuildingKind::Barracks, Age::Castle | Age::Imperial) => 9,
            (BuildingKind::ArcheryRange, Age::Castle | Age::Imperial) => 9,
            (BuildingKind::Stable, Age::Castle | Age::Imperial) => 9,
            // Market Castle step (BLD-19's deferred column: 2/9).
            (BuildingKind::Market, Age::Castle) => 9,
            (BuildingKind::TownCenter, Age::Feudal) => 6,
            (BuildingKind::TownCenter, Age::Castle) => 7,
            // BLD-19: the Market's Imperial step (10 pierce).
            (BuildingKind::Market, Age::Imperial) => 10,
            (BuildingKind::TownCenter, Age::Imperial) => 8,
            _ => self.armor_pierce(),
        }
    }
}

/// Progress a building gains per tick, in thirds of a tick, while
/// `builders` villagers are actively building it (≥ 1): the AoE2
/// multi-builder rule — total build time T × 3/(n+2) — expressed as
/// a rate. One builder works at exactly 1 tick/tick (3 thirds);
/// every extra builder adds a further third of a tick per tick, so
/// 2 builders accrue 4 thirds/tick (a 1500-tick house in 1125
/// ticks), 3 builders 5 thirds/tick, and so on.
pub fn build_rate_thirds(builders: u32) -> u64 {
    builders as u64 + 2
}

/// Repair rate of the FIRST villager repairing a building
/// (BLD-07): 750 HP per minute = 12.5 HP/s. Documented DE values
/// (Fandom Villager page, Phase 0 study) — repair is a flat HP
/// rate, independent of the building's build time or max HP; it
/// is NOT the construction rate.
pub const REPAIR_HP_PER_MIN_FIRST: i64 = 750;
/// Repair rate added by each ADDITIONAL repairer (BLD-07):
/// 375 HP per minute = 6.25 HP/s.
pub const REPAIR_HP_PER_MIN_EXTRA: i64 = 375;
/// Repair cost (BLD-07): restoring a building from 0 to full HP
/// costs this fraction of its build (wood) cost, charged as a
/// continuous drip proportional to the HP restored each tick.
pub const REPAIR_COST_FRACTION: I16F16 = I16F16::from_bits(32_768); // 0.5

/// BLD-05: the HP ceiling of an under-construction building —
/// the proportional line from 1 HP at zero progress to the full
/// maximum at completion (a building reaches full HP exactly when
/// its construction does). Current HP rides the same distance
/// below the ceiling for its whole construction: damage taken
/// while building persists (the construction phase adds the
/// ceiling's per-tick rise to whatever HP stands, never more).
/// The 1 HP intercept is the proportional line's natural floor —
/// a building dies below 1 HP (CMB-02's threshold, applied to
/// buildings in `combat::damage_building`), so a fresh foundation
/// stands at exactly 1 HP until the first construction tick.
/// (The reference's exact starting fraction is UNVERIFIED in
/// public sources; the proportional growth itself is the
/// documented behavior — RULEBOOK BLD-05.) Integer math over raw
/// fixed-point bits; progress is clamped to the build total (the
/// completing tick overshoots it).
pub fn construction_hp_ceiling(
    max_hp: i32,
    progress_thirds: u64,
    build_thirds: u64,
) -> I16F16 {
    let max_bits = (max_hp as i64) << 16;
    let one_hp = 1i64 << 16;
    let p = progress_thirds.min(build_thirds) as i64;
    let total = build_thirds as i64;
    if total <= 0 {
        return I16F16::from_bits(max_bits as i32);
    }
    I16F16::from_bits((one_hp + (max_bits - one_hp) * p / total) as i32)
}

/// Total repair rate in HP per minute for `repairers` villagers
/// (≥ 1) working the same building: 750 + 375·(n − 1)
/// = 375·(n + 1) HP/min — the first repairer works at the full
/// rate, every additional one at half of it (BLD-07).
pub fn repair_rate_hp_per_min(repairers: u32) -> i64 {
    debug_assert!(repairers >= 1);
    REPAIR_HP_PER_MIN_FIRST + REPAIR_HP_PER_MIN_EXTRA * (repairers as i64 - 1)
}

/// A building weapon (BLD-18): the static firing profile of a
/// completed defensive building. The Watch Tower was the first
/// user; the Castle (Castle P3a) is the second — its base volley
/// of 5 projectiles rides the `volley` field below, and GAR-05's
/// garrison projectiles will add on top of the same base
/// machinery in P3c.
///
/// The Watch Tower's numbers, from the AoE wiki's tower article
/// and the project study (§2's band table, §5): 5 pierce attack
/// (+7 vs Ships, +2 vs Spearmen — the tower article's bonus rows),
/// range 1–8 (the minimum range is the study's documented
/// building band; the generic building-attack article's "5–8"
/// describes garrisoned volleys inside a Castle's own footprint
/// and does not govern a 1×1 tower), one shot every 2.0 s.
///
/// The flight profile reuses the Archer's arrow constants: the
/// projectile speed 0.5 wu/tick and the scattered-landing spread
/// 2.0 are PRJ-02/PRJ-04 design constants (the reference publishes
/// no numbers for either), and the attack delay takes the
/// Archer's documented 21 ticks as the arrow-weapon precedent —
/// ADOPTED for buildings, UNVERIFIED (no fetched source pins a
/// tower frame delay). Accuracy 100: the study's §2.2 profiles
/// foot archers at 80/90, but a defensive building's arrows are
/// the always-on-target case in every account of tower rushes —
/// a moving target still dodges by travel time (PRJ-06), which
/// is where tower "inaccuracy" lives in the reference.
///
/// GARRISON REMAINDER (recorded, not shipped): a tower with
/// villagers or archers inside fires extra arrows in the
/// reference (up to its garrison capacity of 5). Garrisoning
/// itself is the GAR rule family's own package; BLD-18 ships the
/// tower's BASE fire only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildingWeapon {
    /// (class, value) attack pairs, CMB-03 form.
    pub attacks: &'static [(u8, i32)],
    /// Firing band, world units, measured from the footprint's
    /// edge (PRJ-08's building form).
    pub range_max: I16F16,
    pub range_min: I16F16,
    /// Ticks between strikes (CMB-05: counted from the previous
    /// strike's landing).
    pub reload_ticks: u64,
    /// First-strike delay of an engagement (CMB-04).
    pub attack_delay_ticks: u64,
    /// Percent chance the shot flies true; a miss still flies and
    /// lands scattered (PRJ-05).
    pub accuracy: u32,
    /// Arrow speed (PRJ-02 design constant, shared with archers).
    pub projectile_speed: I16F16,
    /// Scattered-landing spread on a miss (PRJ-04 design constant).
    pub scatter: I16F16,
    /// Projectiles loosed per shot (Castle P3a): 1 for the Watch
    /// Tower; the Castle's BASE volley is 5 (dossier §2). The
    /// firing loop in `combat::building_weapon_release` looses
    /// this many projectiles at the standing target on each
    /// release — same aim, same flight time; accuracy is 100 on
    /// both armed kinds today, so a volley draws no extra RNG.
    /// GAR-05's garrison arrows (P3c) raise the count on top of
    /// this base when the garrison system lands.
    pub volley: u32,
}

/// The Watch Tower's base profile (BLD-18; sources in the
/// `BuildingWeapon` docs).
pub const WATCH_TOWER_WEAPON: BuildingWeapon = BuildingWeapon {
    attacks: &[(crate::classes::PIERCE, 5), (crate::classes::SHIP, 7), (crate::classes::SPEARMAN, 2)],
    range_max: I16F16::from_bits(2_097_152), // 32.0 = 8 tiles
    range_min: I16F16::from_bits(262_144),   // 4.0 = 1 tile
    reload_ticks: 120,                       // 2.0 s
    attack_delay_ticks: 21,
    accuracy: 100,
    projectile_speed: I16F16::from_bits(32_768), // 0.5
    scatter: I16F16::from_bits(131_072),         // 2.0
    volley: 1,
};

/// The Castle's base profile (Castle P3a; AoE wiki, Castle
/// infobox via the Castle Age dossier, fetched 2026-10-22):
/// 11 pierce attack with +2 vs Spearmen on the main projectile,
/// range 8 with a 1-tile minimum (the building band, PRJ-08 —
/// Murder Holes, the University tech that removes the minimum,
/// is P3b), one volley every 2.0 s, accuracy 100%, and a base
/// volley of **5 projectiles**. The flight profile reuses the
/// tower's adopted constants (the Archer's 21-tick delay, the
/// PRJ-02/PRJ-04 speed/scatter) — the same UNVERIFIED-for-
/// buildings adoption BLD-18 records for the tower.
pub const CASTLE_WEAPON: BuildingWeapon = BuildingWeapon {
    attacks: &[(crate::classes::PIERCE, 11), (crate::classes::SPEARMAN, 2)],
    range_max: I16F16::from_bits(2_097_152), // 32.0 = 8 tiles
    range_min: I16F16::from_bits(262_144),   // 4.0 = 1 tile
    reload_ticks: 120,                       // 2.0 s
    attack_delay_ticks: 21,
    accuracy: 100,
    projectile_speed: I16F16::from_bits(32_768), // 0.5
    scatter: I16F16::from_bits(131_072),         // 2.0
    volley: 5,
};

/// The Town Center's weapon (Castle P3b, GAR-06): 5 pierce,
/// range 6 tiles, RoF 2.0 s — it fires ONLY while the Town Center
/// holds a garrison, with the GAR-05 arrow count (minimum one
/// arrow while garrisoned). The flight profile reuses the armed
/// buildings' adopted constants (the Archer's 21-tick delay, the
/// PRJ-02/PRJ-04 speed/scatter), the same UNVERIFIED-for-buildings
/// adoption BLD-18 records. Range is measured from the TC
/// footprint's edge like every building weapon (PRJ-08's building
/// form); no minimum range is documented for the TC, so none is
/// applied. The `volley` field is the ungarrisoned placeholder 1 —
/// the TC weapon step always looses the stored GAR-05 count.
pub const TOWN_CENTER_WEAPON: BuildingWeapon = BuildingWeapon {
    attacks: &[(crate::classes::PIERCE, 5)],
    range_max: I16F16::from_bits(1_572_864), // 24.0 = 6 tiles
    range_min: I16F16::ZERO,
    reload_ticks: 120,                       // 2.0 s
    attack_delay_ticks: 21,
    accuracy: 100,
    projectile_speed: I16F16::from_bits(32_768), // 0.5
    scatter: I16F16::from_bits(131_072),         // 2.0
    volley: 1,
};

/// The Watch Tower's attack table with Fletching researched
/// (TEC-06's tower line: +1 attack — the pierce component; the
/// +1 range layers in `Sim::building_weapon`).
pub const WATCH_TOWER_FLETCHING_ATTACKS: &[(u8, i32)] = &[
    (crate::classes::PIERCE, 6),
    (crate::classes::SHIP, 7),
    (crate::classes::SPEARMAN, 2),
];

/// `Projectile::attacker` value for a shot loosed by a building:
/// no entity bears this id (entity ids are sequential from 1 and
/// the match never fields billions of them), so every consumer
/// that resolves the attacker as an entity finds none — exactly
/// the semantics a building shot needs (no carcass harvester, no
/// retaliation target; the kill credit rides
/// `Projectile::attacker_building` and `Entity::last_hit_owner`).
pub const BUILDING_ATTACKER_ID: u32 = u32::MAX;

/// One placed building, under construction or complete. Position is the
/// footprint's center in world coordinates; `cell_x`/`cell_y` are the
/// anchor (minimum) cell of the footprint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Building {
    pub id: u32,
    pub kind: BuildingKind,
    /// The player who placed this building (M5): `OWNER_PLAYER` or
    /// `OWNER_OPPONENT`. Placement spends that player's wood, the
    /// population support counts for that player, a production
    /// building trains for that player, and Conquest counts the
    /// building as that player's. Part of the hashed state.
    pub owner: u8,
    /// Current hit points (M5): a placed building can be attacked
    /// and destroyed (see `combat::damage_building`). While under
    /// construction the HP rides below the proportional ceiling of
    /// `construction_hp_ceiling` (BLD-05): placed at 1 HP, rising
    /// with progress, reaching the kind's table maximum exactly at
    /// completion if undamaged. Part of the hashed state.
    pub hp: I16F16,
    pub cell_x: u32,
    pub cell_y: u32,
    pub pos_x: I16F16,
    pub pos_y: I16F16,
    /// Construction progress so far, in thirds of a tick of solo
    /// work (accrues at `build_rate_thirds(n)` while n villagers
    /// build). The building is non-functional until
    /// `progress_thirds >= kind.build_thirds()`.
    pub progress_thirds: u64,
    /// Repair fractional-HP accumulator (BLD-07): repair restores
    /// HP at a flat per-minute rate that does not divide evenly
    /// into per-tick fixed-point bits, so the sub-bit remainder is
    /// carried here — each repair tick adds
    /// `rate_hp_per_min × 65536` to a conceptual accumulator whose
    /// quotient over 3600 (ticks per minute) is the HP bits
    /// restored; this field holds that accumulator's remainder
    /// (0 ≤ r < 3600, in bits-per-minute units). Reset to 0 when
    /// the building reaches full HP. Part of the hashed state.
    pub repair_hp_remainder: i64,
    /// Repair wood-drip accumulator (BLD-07): the per-tick wood
    /// charge is `wood_cost × REPAIR_COST_FRACTION × restored /
    /// max_hp`, accumulated in numerator form over the denominator
    /// `max_hp_bits × 65536` so the drip totals exactly the
    /// fraction of the build cost over a full repair (to < 1 bit);
    /// this field holds the numerator remainder. Reset to 0 when
    /// the building reaches full HP. Part of the hashed state.
    pub repair_cost_remainder: i64,
    /// Repair stone-drip accumulator (BLD-07 × BLD-18): the stone
    /// half of the repair drip for stone-cost buildings, in the
    /// same numerator form as `repair_cost_remainder`. Zero for
    /// every wood-only kind. Reset to 0 at full HP. Part of the
    /// hashed state (written only while nonzero).
    pub repair_stone_remainder: i64,
    /// Treadmill Crane construction-rate remainder (Castle P3c,
    /// TEC-09): the ×6/5 construction scalar does not divide the
    /// thirds rate evenly, so the sub-third remainder carries
    /// here (0 ≤ r < 5, in fifths of a third) — each construction
    /// tick adds `rate × 6 + r`, accrues `(rate × 6 + r) / 5`
    /// thirds and keeps the rest. Always 0 for an owner without
    /// Treadmill Crane. Part of the hashed state (written only
    /// while nonzero).
    pub build_rate_remainder: i64,
    pub complete: bool,
    /// For a completed Farm: the resource node it became. The building
    /// record stays as the footprint registry until the node depletes,
    /// then both retire together (see `Sim` construction phase).
    pub node_id: Option<u32>,
    /// Production/research queue (M3 as unit kinds; M6 generalized
    /// to `ProdEntry`): units waiting to train and technologies
    /// waiting to research, head first, in one shared FIFO
    /// (TEC-03). Cost is paid on enqueue, like the Town Center's
    /// queue; enqueue is refused at the population cap (the
    /// HOUSED rule, 2026-10-27), and a queued unit's progress
    /// stalls only if the cap later drops below the living
    /// population; research progress never stalls. Any building
    /// can hold research (Blacksmith lines, the
    /// Barracks' Man-at-Arms upgrade); only production buildings
    /// hold units.
    pub queue: Vec<ProdEntry>,
    /// Ticks of progress on the queue's head entry.
    pub train_progress: u64,
    /// Rally point (M4, SEL-13; production buildings only): where a
    /// freshly trained unit walks on spawn. `None` = unset — the unit
    /// simply stays at its spawn point just outside the building
    /// (the reference game's default). Part of the hashed state.
    pub rally: Option<(I16F16, I16F16)>,
    /// The resource node the rally point snapped to, if any (M4,
    /// SEL-13): a new *villager* goes straight to work on it instead
    /// of merely walking to the point. Resolved when the rally is
    /// set; re-checked (still live?) when a unit spawns.
    pub rally_node: Option<u32>,
    // --- BLD-18: building-weapon state (inert unless the kind
    // carries a weapon; only the Watch Tower does today) ---
    /// The entity this building's weapon is engaged with — a unit
    /// or the enemy Town Center entity (the TC is an entity, so it
    /// shares this slot; the building-target slot below is for
    /// placed buildings). Re-validated every tick by the weapon
    /// step, exactly like an entity's `attack_target`.
    pub weapon_target: Option<u32>,
    /// The placed building this building's weapon is engaged with.
    pub weapon_target_building: Option<u32>,
    /// Tick at which a pending first strike is loosed (the attack
    /// delay window — CMB-04's building form).
    pub weapon_strike_tick: Option<u64>,
    /// Earliest tick the next shot may begin (reload — CMB-05).
    pub weapon_next_tick: u64,
    /// Whether the weapon has already loosed a shot at its current
    /// target (the attack delay applies only to an engagement's
    /// first shot).
    pub weapon_engaged: bool,
    // --- Castle P3b: garrison state (GAR-01..05) ---
    /// The entities garrisoned inside this building, in entry order
    /// (hashed list of entity ids — GAR-01's impl note). Empty for
    /// non-garrisonable kinds. The Town Center's garrison lives on
    /// its `TcState` instead (the TC is an entity, BLD-16).
    pub garrison: Vec<u32>,
    /// The building's current total arrows per volley (GAR-05):
    /// recomputed on every garrison change (event, not per tick) —
    /// base volley + capped extra arrows for armed kinds; 0 for
    /// unarmed kinds and for an ungarrisoned state on kinds whose
    /// count is purely the base (towers/Castle store their base
    /// volley here once computed; see `Sim::recompute_garrison_arrows`).
    pub garrison_arrows: u32,
}

impl Building {
    /// The footprint cells, in fixed (row-major) order.
    pub fn footprint_cells(&self) -> Vec<(u32, u32)> {
        footprint_cells(self.cell_x, self.cell_y, self.kind.footprint_size())
    }

    /// Build progress as a fraction of the kind's build total, in raw
    /// fixed-point bits via integer math (0 … 65536). Presentation aid
    /// for the ABI; the sim compares third counts, never this value.
    pub fn progress_frac_bits(&self) -> i32 {
        let total = self.kind.build_thirds() as i64;
        let done = (self.progress_thirds as i64).min(total);
        ((done * 65536) / total) as i32
    }

    /// Production progress on the queue's head as a fraction of its
    /// train/research time, in raw fixed-point bits (0 … 65536).
    /// Zero when the queue is empty. Presentation aid for the ABI.
    pub fn train_frac_bits(&self) -> i32 {
        match self.queue.first() {
            None => 0,
            Some(&head) => {
                let total = head.ticks() as i64;
                if total <= 0 {
                    return 0;
                }
                let done = (self.train_progress as i64).min(total);
                ((done * 65536) / total) as i32
            }
        }
    }

    /// How many *units* wait in this building's queue (research
    /// entries do not occupy population — M6).
    pub fn queued_units(&self) -> u32 {
        self.queue
            .iter()
            .filter(|e| matches!(e, ProdEntry::Unit(_)))
            .count() as u32
    }

    /// Remaining HP as a fraction of the kind's maximum at the
    /// owner's current Age (BLD-11), in raw fixed-point bits
    /// (0 … 65536). Presentation aid for the ABI (the
    /// damaged-building HP bar).
    pub fn hp_frac_bits(&self, owner_age: Age) -> i32 {
        let total = self.kind.max_hp_at(owner_age) as i64;
        if total <= 0 {
            return 65536;
        }
        let left = (self.hp.to_bits() as i64).clamp(0, total << 16);
        ((left * 65536) / (total << 16)) as i32
    }
}

/// The cells of a square footprint of edge `size` anchored at
/// `(cell_x, cell_y)`, in fixed (row-major) order.
pub fn footprint_cells(cell_x: u32, cell_y: u32, size: u32) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity((size * size) as usize);
    for dy in 0..size {
        for dx in 0..size {
            out.push((cell_x + dx, cell_y + dy));
        }
    }
    out
}

/// The cells ringing a square footprint of edge `size` (the builder's
/// standing spots), in fixed row-major order: the (size+2)² block
/// around the footprint minus the footprint itself. May contain
/// out-of-grid cells; callers check bounds.
pub fn ring_cells(cell_x: u32, cell_y: u32, size: u32) -> Vec<(u32, u32)> {
    let mut out = Vec::with_capacity(((size + 2) * (size + 2) - size * size) as usize);
    let span = size as i32 + 2;
    for dy in 0..span {
        for dx in 0..span {
            let x = cell_x as i32 + dx - 1;
            let y = cell_y as i32 + dy - 1;
            if (1..=size as i32).contains(&dx) && (1..=size as i32).contains(&dy) {
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

/// The cells of a dragged wall line (BLD-13), from anchor cell
/// `(ax, ay)` to anchor cell `(bx, by)`, both endpoints included,
/// in drag order: the STRAIGHT segment between the two cells'
/// centers rasterized as an 8-direction line — at each of the
/// `n + 1` steps of the major axis (`n = max(|dx|, |dy|)`), the
/// cell nearest the ideal point of the segment at that step,
/// each ideal coordinate rounded to the nearest integer (halves
/// round up) in exact integer math. The rounding is a pure
/// function of the ABSOLUTE ideal position, so the reversed
/// drag yields the identical set in exactly reversed order.
///
/// Consecutive cells are always KING-ADJACENT (Chebyshev step
/// exactly 1, never 0): the major coordinate advances by one
/// cell per step, the minor by at most one, so along a diagonal
/// run the line steps diagonally, cell corner to cell corner,
/// and a 45° drag is a pure chain of diagonal neighbours — one
/// straight slanted line of `n + 1` segments, hugging the ideal
/// segment end to end (no cell center is ever more than half a
/// cell off it, on either axis). A purely horizontal/vertical
/// drag is exactly that row/column. The corner joints of a
/// diagonal chain are sealed for movement — the A* corner rule
/// (`Grid::astar_cells`) bars the diagonal step between the two
/// flanking cells whenever an orthogonal neighbour is a
/// building cell, and grid integrity (tick phase 2.5) displaces
/// any unit that ends a tick inside a blocked cell — pinned by
/// the sealing test in `sim/tests/walls.rs`.
///
/// DE evidence (correction addendum, study dossier
/// `walls-de/DE-WALL-LINE-CORRECTION.md`): DE rasterizes a true
/// straight line exactly for same-row, same-column and
/// same-diagonal drags — the shapes this expansion reproduces
/// cell-for-cell — and lays its other drags as an L of a
/// diagonal leg plus a straight leg at the same `n + 1` tile
/// count (official forum, "Get rid of L shape when creating
/// walls"). Per Claudio's ruling (2026-10-24) Skirmish lays
/// every drag on the ideal end-to-end line instead of the L:
/// same endpoints, same optimal segment count, the diagonal
/// steps spread where the line itself crosses.
///
/// History: this replaced the Amanatides–Woo SUPERCOVER on
/// 2026-10-24 (Claudio's live report: a diagonal drag rendered
/// as a zigzag of alternating straight runs — "não existe este
/// tipo de muralha; normalmente ele pega uma ponta a outra para
/// fazer em uma linha reta; o diagonal como vc sugere nunca vi
/// no jogo"). The supercover laid every cell the ideal segment
/// touched (`1 + |dx| + |dy|` cells, a 4-connected staircase);
/// in the dimetric projection each axis step flips the drawn
/// band's slope, so any diagonal read as a sawtooth. The
/// supercover had itself replaced the classic integer
/// Bresenham on 2026-10-22 (disjoint row runs, direction-
/// dependent tie-breaking: 9,096 of 28,392 drags in a 13×13
/// scan changed set when reversed — this expansion's absolute
/// rounding is reversal-exact by construction). Pure integer
/// math: the expansion of a `Command::BuildLine` is a
/// deterministic function of its two endpoints, and the
/// client's preview (`web-preview/play/walls.js`) runs the
/// identical algorithm so the ghost never promises a
/// different line.
pub fn wall_line_cells(ax: u32, ay: u32, bx: u32, by: u32) -> Vec<(u32, u32)> {
    let dx = bx as i64 - ax as i64;
    let dy = by as i64 - ay as i64;
    let n = dx.abs().max(dy.abs());
    if n == 0 {
        return vec![(ax, ay)];
    }
    // The coordinate of the cell nearest the ideal point at
    // step k along one axis: round(a + d·k/n), halves up,
    // computed exactly as floor((2·(a·n + d·k) + n) / (2·n)).
    // The numerator is never negative on a real drag — ideal
    // positions stay between the two endpoint cells, which are
    // in-grid — so the truncating division floors exactly.
    let coord = |a: u32, d: i64, k: i64| -> u32 {
        ((2 * (a as i64 * n + d * k) + n) / (2 * n)) as u32
    };
    let mut out = Vec::with_capacity((n + 1) as usize);
    for k in 0..=n {
        out.push((coord(ax, dx, k), coord(ay, dy, k)));
    }
    out
}

/// World coordinates of a footprint's center: `anchor + size/2 cells`
/// along each axis from the grid origin — for a 2×2 footprint, the
/// corner shared by its four cells.
pub fn footprint_center(cell_x: u32, cell_y: u32, size: u32) -> (I16F16, I16F16) {
    let half = I16F16::from_num(size as i32) * CELL_SIZE / I16F16::from_num(2);
    let x = I16F16::from_num(cell_x as i32) * CELL_SIZE + half - WORLD_HALF;
    let y = I16F16::from_num(cell_y as i32) * CELL_SIZE + half - WORLD_HALF;
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
        assert_eq!(BARRACKS_WOOD_COST, I16F16::from_num(175));
        assert_eq!(BARRACKS_BUILD_TICKS, 50 * 60);
        assert_eq!(HOUSE_POP_SUPPORT, 5);
        assert_eq!(TOWN_CENTER_POP_SUPPORT, 5);
        // Build totals in thirds: house 4500, farm 2700.
        assert_eq!(BuildingKind::House.build_thirds(), 4500);
        assert_eq!(BuildingKind::Farm.build_thirds(), 2700);
        // Footprints: 2×2 for the House, 3×3 for the Farm and the
        // Barracks. Farm 3×3: Claudio's decision 2026-10-22 (the
        // DE-documented footprint, superseding the 2×2 spec).
        assert_eq!(BuildingKind::House.footprint_size(), 2);
        assert_eq!(BuildingKind::Farm.footprint_size(), 3);
        assert_eq!(BuildingKind::Barracks.footprint_size(), 3);
        assert!(BuildingKind::Barracks.blocks_footprint());
        assert!(BuildingKind::Barracks.is_production());
        // M5 building combat numbers (study §5, Dark Age rows):
        // House 550 HP, −2/7 armor; Farm 480 HP, 0/0; Barracks
        // 1200 HP, 0/7.
        assert_eq!(BuildingKind::House.max_hp(), 550);
        assert_eq!(BuildingKind::House.armor_melee(), -2);
        assert_eq!(BuildingKind::House.armor_pierce(), 7);
        assert_eq!(BuildingKind::Farm.max_hp(), 480);
        assert_eq!(BuildingKind::Farm.armor_melee(), 0);
        assert_eq!(BuildingKind::Farm.armor_pierce(), 0);
        assert_eq!(BuildingKind::Barracks.max_hp(), 1200);
        assert_eq!(BuildingKind::Barracks.armor_melee(), 0);
        assert_eq!(BuildingKind::Barracks.armor_pierce(), 7);
        // M6 numbers: camps 100 wood / 35 s / 2×2 (OPENING-SPEC
        // §2.4); Range/Stable 175 wood / 50 s / 3×3 with the
        // study's Feudal combat row (1500 HP, 1/8); Blacksmith
        // 150 wood (brief).
        for camp in [
            BuildingKind::LumberCamp,
            BuildingKind::Mill,
            BuildingKind::MiningCamp,
        ] {
            assert_eq!(camp.wood_cost(), I16F16::from_num(100));
            assert_eq!(camp.build_ticks(), 35 * 60);
            assert_eq!(camp.footprint_size(), 2);
            assert!(camp.is_dropoff());
            assert!(camp.counts_toward_feudal());
        }
        assert!(BuildingKind::LumberCamp.accepts(ResourceKind::Wood));
        assert!(BuildingKind::Mill.accepts(ResourceKind::Food));
        assert!(BuildingKind::MiningCamp.accepts(ResourceKind::Gold));
        assert!(BuildingKind::MiningCamp.accepts(ResourceKind::Stone));
        assert!(!BuildingKind::Mill.accepts(ResourceKind::Wood));
        for kind in [BuildingKind::ArcheryRange, BuildingKind::Stable] {
            assert_eq!(kind.wood_cost(), I16F16::from_num(175));
            assert_eq!(kind.build_ticks(), 50 * 60);
            assert_eq!(kind.footprint_size(), 3);
            assert_eq!(kind.max_hp(), 1500);
            assert_eq!(kind.armor_melee(), 1);
            assert_eq!(kind.armor_pierce(), 8);
            assert_eq!(kind.age_required(), Age::Feudal);
            assert_eq!(kind.requires_building(), Some(BuildingKind::Barracks));
        }
        assert_eq!(BuildingKind::Blacksmith.wood_cost(), I16F16::from_num(150));
        assert_eq!(BuildingKind::Blacksmith.age_required(), Age::Feudal);
        // BLD-18 numbers (tower + Outpost articles, fetched
        // 2026-10-22): Watch Tower 35W/125S, 80 s, 1×1, Feudal,
        // 850 HP, 1/7 armor; Outpost 25W/5S, 15 s, 1×1, Dark,
        // 500 HP. Neither counts toward an age-up requirement.
        let wt = BuildingKind::WatchTower;
        // Tags 10/11 since the merge with BLD-13: the Palisade
        // Wall took 9 on main first (ABI append-only rule).
        assert_eq!(wt.tag(), 10);
        assert_eq!(wt.wood_cost(), I16F16::from_num(35));
        assert_eq!(wt.stone_cost(), I16F16::from_num(125));
        assert_eq!(wt.total_cost(), I16F16::from_num(160));
        assert_eq!(wt.build_ticks(), 80 * 60);
        assert_eq!(wt.footprint_size(), 1);
        assert_eq!(wt.age_required(), Age::Feudal);
        assert_eq!(wt.max_hp(), 850);
        assert_eq!(wt.armor_melee(), 1);
        assert_eq!(wt.armor_pierce(), 7);
        assert!(!wt.counts_toward_feudal());
        assert!(!wt.produces_units());
        assert!(wt.weapon().is_some());
        let op = BuildingKind::Outpost;
        assert_eq!(op.tag(), 11);
        assert_eq!(op.wood_cost(), I16F16::from_num(25));
        assert_eq!(op.stone_cost(), I16F16::from_num(5));
        assert_eq!(op.build_ticks(), 15 * 60);
        assert_eq!(op.footprint_size(), 1);
        assert_eq!(op.age_required(), Age::Dark);
        assert_eq!(op.max_hp(), 500);
        assert!(!op.counts_toward_feudal());
        assert!(op.weapon().is_none());
        // Sight (VIS-01): the generic building reads 4; the two
        // BLD-18 structures carry their adopted discs.
        assert_eq!(BuildingKind::House.sight_tiles_at(Age::Dark), 4);
        assert_eq!(wt.sight_tiles_at(Age::Feudal), 10);
        assert_eq!(op.sight_tiles_at(Age::Dark), 6);
        assert_eq!(op.sight_tiles_at(Age::Feudal), 8);
        // The weapon profile: 5 pierce +7 Ship +2 Spearman,
        // band 1–8 tiles, 2.0 s reload.
        let w = wt.weapon().unwrap();
        assert_eq!(w.attacks, &[(crate::classes::PIERCE, 5), (crate::classes::SHIP, 7), (crate::classes::SPEARMAN, 2)]);
        assert_eq!(w.range_max, I16F16::from_num(32));
        assert_eq!(w.range_min, I16F16::from_num(4));
        assert_eq!(w.reload_ticks, 120);
        // Town Center (Castle P1; study §5): 275 W + 100 S, 150 s,
        // 4×4, Castle Age, 2400 HP; tag 13 (10–12 went to the
        // BLD-18/19 kinds on main first — append-only rule).
        assert_eq!(BuildingKind::TownCenter.tag(), 13);
        assert_eq!(BuildingKind::TownCenter.wood_cost(), I16F16::from_num(275));
        assert_eq!(BuildingKind::TownCenter.stone_cost(), I16F16::from_num(100));
        assert_eq!(BuildingKind::TownCenter.build_ticks(), 150 * 60);
        assert_eq!(BuildingKind::TownCenter.footprint_size(), 4);
        assert_eq!(BuildingKind::TownCenter.age_required(), Age::Castle);
        assert_eq!(BuildingKind::TownCenter.max_hp(), 2400);
        assert_eq!(BuildingKind::House.stone_cost(), I16F16::ZERO);
        // Castle qualifying set (TEC-01): the Feudal-born kinds
        // only — the Dark-born Barracks does not count, and
        // neither do walls or towers (BLD-18's exclusion); the
        // Market counts (Feudal-born, BLD-19).
        assert!(BuildingKind::ArcheryRange.counts_toward_castle());
        assert!(BuildingKind::Stable.counts_toward_castle());
        assert!(BuildingKind::Blacksmith.counts_toward_castle());
        assert!(BuildingKind::Market.counts_toward_castle());
        assert!(!BuildingKind::Barracks.counts_toward_castle());
        assert!(!BuildingKind::Mill.counts_toward_castle());
        assert!(!BuildingKind::WatchTower.counts_toward_castle());
        assert!(!BuildingKind::Outpost.counts_toward_castle());
        // BLD-11 Castle steps (study §5): House 900 HP, −1→1
        // melee, 8→9 pierce; Barracks 1800, 1→2, 8→9; Range and
        // Stable 1800, 1→2, 8→9; Town Center armor 4/6 Feudal →
        // 5/7 Castle at a flat 2400 HP; camps stay flat. Plus the
        // steps BLD-18/19 deferred to this package: Watch Tower
        // 850 → 1020 HP; Market 1800 → 2100 HP, armor 1/8 → 2/9.
        assert_eq!(BuildingKind::House.max_hp_at(Age::Castle), 900);
        assert_eq!(BuildingKind::House.armor_melee_at(Age::Castle), 1);
        assert_eq!(BuildingKind::House.armor_pierce_at(Age::Castle), 9);
        assert_eq!(BuildingKind::Barracks.max_hp_at(Age::Castle), 1800);
        assert_eq!(BuildingKind::Barracks.armor_melee_at(Age::Castle), 2);
        assert_eq!(BuildingKind::Barracks.armor_pierce_at(Age::Castle), 9);
        assert_eq!(BuildingKind::ArcheryRange.max_hp_at(Age::Castle), 1800);
        assert_eq!(BuildingKind::ArcheryRange.armor_melee_at(Age::Castle), 2);
        assert_eq!(BuildingKind::Stable.armor_pierce_at(Age::Castle), 9);
        assert_eq!(BuildingKind::TownCenter.max_hp_at(Age::Castle), 2400);
        // Imperial columns (TEC-01's last rung): the documented
        // Castle/Imperial-equal rows carry over (House 900,
        // Stone Wall 1800), the TC step is 6/8 (BLD-11) and the
        // Market step is 3/10 (BLD-19).
        assert_eq!(BuildingKind::House.max_hp_at(Age::Imperial), 900);
        assert_eq!(BuildingKind::StoneWall.max_hp_at(Age::Imperial), 1800);
        assert_eq!(BuildingKind::Barracks.max_hp_at(Age::Imperial), 1800);
        assert_eq!(BuildingKind::House.armor_melee_at(Age::Imperial), 1);
        assert_eq!(BuildingKind::House.armor_pierce_at(Age::Imperial), 9);
        assert_eq!(BuildingKind::TownCenter.armor_melee_at(Age::Imperial), 6);
        assert_eq!(BuildingKind::TownCenter.armor_pierce_at(Age::Imperial), 8);
        assert_eq!(BuildingKind::Market.armor_melee_at(Age::Imperial), 3);
        assert_eq!(BuildingKind::Market.armor_pierce_at(Age::Imperial), 10);
        assert!(BuildingKind::Castle.counts_toward_imperial());
        assert!(!BuildingKind::ArcheryRange.counts_toward_imperial());
        assert!(!BuildingKind::TownCenter.counts_toward_imperial());
        assert_eq!(BuildingKind::TownCenter.armor_melee_at(Age::Feudal), 4);
        assert_eq!(BuildingKind::TownCenter.armor_pierce_at(Age::Castle), 7);
        assert_eq!(BuildingKind::Mill.max_hp_at(Age::Castle), 550);
        assert_eq!(BuildingKind::WatchTower.max_hp_at(Age::Castle), 1020);
        assert_eq!(BuildingKind::Market.max_hp_at(Age::Castle), 2100);
        assert_eq!(BuildingKind::Market.armor_melee_at(Age::Castle), 2);
        assert_eq!(BuildingKind::Market.armor_pierce_at(Age::Castle), 9);
        assert_eq!(BuildingKind::Outpost.sight_tiles_at(Age::Castle), 8);
        // The trainable relation (M6): one home per unit kind.
        assert!(BuildingKind::Barracks.trains(EntityKind::Militia));
        assert!(BuildingKind::Barracks.trains(EntityKind::Spearman));
        assert!(!BuildingKind::Barracks.trains(EntityKind::Archer));
        assert!(BuildingKind::ArcheryRange.trains(EntityKind::Archer));
        assert!(BuildingKind::ArcheryRange.trains(EntityKind::Skirmisher));
        assert!(!BuildingKind::ArcheryRange.trains(EntityKind::Spearman));
        assert!(BuildingKind::Stable.trains(EntityKind::ScoutCavalry));
        // Package D: the Market trains the Trade Cart (TRD-05)
        // and is a production building (queue + rally); it does
        // NOT join `produces_units` in this package — the VIC-01
        // elimination predicate is a separate rule's surface and
        // stays exactly as the Castle P3a front left it.
        assert!(BuildingKind::Market.trains(EntityKind::TradeCart));
        assert!(!BuildingKind::Market.trains(EntityKind::Militia));
        assert!(BuildingKind::Market.is_production());
        assert!(!BuildingKind::Barracks.trains(EntityKind::TradeCart));
    }

    #[test]
    fn multi_builder_rate_is_the_aoe2_formula() {
        // Total time T × 3/(n+2), as a per-tick rate in thirds:
        // n builders add (n+2) thirds per tick. Solo is exactly 1×;
        // 2 builders finish a 1500-tick house in 4500/4 = 1125 ticks
        // (≈19 s — the opening's first timing trick).
        assert_eq!(build_rate_thirds(1), 3);
        assert_eq!(build_rate_thirds(2), 4);
        assert_eq!(build_rate_thirds(3), 5);
        assert_eq!(build_rate_thirds(4), 6);
        assert_eq!(BuildingKind::House.build_thirds() / build_rate_thirds(2), 1125);
    }

    #[test]
    fn repair_numbers_are_the_documented_de_values() {
        // BLD-07 (Fandom Villager page, Phase 0 study): the first
        // repairer restores 750 HP/min, each additional +375 —
        // 375·(n + 1) in total; a full 0→100% repair costs 50% of
        // the build cost.
        assert_eq!(REPAIR_HP_PER_MIN_FIRST, 750);
        assert_eq!(REPAIR_HP_PER_MIN_EXTRA, 375);
        assert_eq!(repair_rate_hp_per_min(1), 750);
        assert_eq!(repair_rate_hp_per_min(2), 1125);
        assert_eq!(repair_rate_hp_per_min(3), 1500);
        assert_eq!(repair_rate_hp_per_min(4), 1875);
        assert_eq!(REPAIR_COST_FRACTION.to_bits(), 32_768); // 0.5
    }

    #[test]
    fn progress_fraction_is_measured_in_thirds() {
        let mut b = Building {
            id: 0,
            kind: BuildingKind::House,
            owner: crate::entity::OWNER_PLAYER,
            hp: I16F16::from_num(BuildingKind::House.max_hp()),
            cell_x: 0,
            cell_y: 0,
            pos_x: I16F16::ZERO,
            pos_y: I16F16::ZERO,
            progress_thirds: 0,
            repair_hp_remainder: 0,
            repair_cost_remainder: 0,
            repair_stone_remainder: 0,
            build_rate_remainder: 0,
            complete: false,
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
        };
        assert_eq!(b.progress_frac_bits(), 0);
        b.progress_thirds = 2250; // half of 4500
        assert_eq!(b.progress_frac_bits(), 32768);
        b.progress_thirds = 4500;
        assert_eq!(b.progress_frac_bits(), 65536);
        // Overshoot (the completing tick can pass the total) clamps.
        b.progress_thirds = 4503;
        assert_eq!(b.progress_frac_bits(), 65536);
    }

    #[test]
    fn footprint_and_ring_are_the_right_shape() {
        let cells = footprint_cells(10, 10, 2);
        assert_eq!(
            cells,
            vec![(10, 10), (11, 10), (10, 11), (11, 11)]
        );
        let ring = ring_cells(10, 10, 2);
        assert_eq!(ring.len(), 12, "a 2x2 footprint has 12 ring cells");
        for c in ring {
            assert!(!cells.contains(&c), "ring cell inside footprint");
        }
        // Ring at the map corner: out-of-grid cells are dropped.
        assert!(ring_cells(0, 0, 2).len() < 12);
        // The 3×3 Barracks: 9 footprint cells, 16 ring cells.
        let cells3 = footprint_cells(10, 10, 3);
        assert_eq!(cells3.len(), 9);
        assert_eq!(cells3[8], (12, 12));
        let ring3 = ring_cells(10, 10, 3);
        assert_eq!(ring3.len(), 16, "a 3x3 footprint has 16 ring cells");
        for c in ring3 {
            assert!(!cells3.contains(&c), "ring cell inside footprint");
        }
    }

    #[test]
    fn palisade_wall_numbers_are_the_current_de_row() {
        // BLD-13 (AoE wiki, Palisade Wall infobox + Building
        // table, fetched 2026-10-22): Dark Age, 3 wood (update
        // 56005), 7 s build (update 42848), 1×1, HP 150 Dark /
        // 250 Feudal+ (update 44725), armor 2 melee / 5 pierce.
        let w = BuildingKind::PalisadeWall;
        assert_eq!(w.tag(), 9);
        assert!(w.is_wall());
        assert_eq!(w.wood_cost(), I16F16::from_num(3));
        assert_eq!(w.build_ticks(), 7 * 60);
        assert_eq!(w.footprint_size(), 1);
        assert!(w.blocks_footprint());
        assert_eq!(w.age_required(), Age::Dark);
        assert_eq!(w.requires_building(), None);
        assert_eq!(w.max_hp(), 150);
        assert_eq!(w.max_hp_at(Age::Dark), 150);
        assert_eq!(w.max_hp_at(Age::Feudal), 250);
        assert_eq!(w.armor_melee(), 2);
        assert_eq!(w.armor_pierce(), 5);
        assert_eq!(w.armor_melee_at(Age::Feudal), 2);
        assert_eq!(w.armor_pierce_at(Age::Feudal), 5);
        // Walls never count toward age-up requirements, give no
        // population room, train nothing and accept no drop-offs.
        assert!(!w.counts_toward_feudal());
        assert_eq!(w.pop_support(), 0);
        assert!(!w.produces_units());
        assert!(!w.is_dropoff());
    }

    #[test]
    fn wall_line_cells_is_a_straight_eight_direction_line() {
        // The same golden vectors pin the client's expansion in
        // web-preview/play/walls.test.mjs — ghost and sim expand
        // identically, cell for cell.
        // A click (no drag) is one segment.
        assert_eq!(wall_line_cells(5, 5, 5, 5), vec![(5, 5)]);
        // Horizontal and vertical drags are exact rows/columns.
        assert_eq!(
            wall_line_cells(2, 3, 6, 3),
            vec![(2, 3), (3, 3), (4, 3), (5, 3), (6, 3)]
        );
        assert_eq!(
            wall_line_cells(4, 1, 4, 4),
            vec![(4, 1), (4, 2), (4, 3), (4, 4)]
        );
        // A 45° drag is a pure chain of diagonal neighbours —
        // four segments for (1,1)->(4,4), NOT the seven-cell
        // edge-connected staircase the supercover laid
        // (Claudio's 2026-10-24 correction: a diagonal drag is
        // one straight line, end to end).
        assert_eq!(
            wall_line_cells(1, 1, 4, 4),
            vec![(1, 1), (2, 2), (3, 3), (4, 4)]
        );
        assert_eq!(
            wall_line_cells(0, 0, 6, 6),
            vec![(0, 0), (1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (6, 6)]
        );
        // A shallow slope: the diagonal steps land where the
        // ideal segment crosses the half-cell line, so the cells
        // hug the end-to-end line instead of bunching at an end.
        assert_eq!(
            wall_line_cells(0, 0, 7, 2),
            vec![
                (0, 0),
                (1, 0),
                (2, 1),
                (3, 1),
                (4, 1),
                (5, 1),
                (6, 2),
                (7, 2)
            ]
        );
        // A steep slope: the transposed chain.
        assert_eq!(
            wall_line_cells(0, 0, 2, 7),
            vec![
                (0, 0),
                (0, 1),
                (1, 2),
                (1, 3),
                (1, 4),
                (1, 5),
                (2, 6),
                (2, 7)
            ]
        );
        // The long shallow drag the client ghost pins too:
        // max(|dx|,|dy|)+1 = 19 cells (the supercover laid 21),
        // diagonal steps at x=4->5 and x=13->14.
        let line = wall_line_cells(0, 0, 18, 2);
        assert_eq!(line.len(), 19);
        assert!(line.contains(&(4, 0)) && line.contains(&(5, 1)));
        assert!(line.contains(&(13, 1)) && line.contains(&(14, 2)));
        // Properties over every drag in a 13×13 field: exact
        // count max(|dx|, |dy|) + 1, every consecutive pair a
        // king move (Chebyshev step exactly 1 — diagonal steps
        // included, never a repeat, never a jump), no cell
        // repeated, and the REVERSED drag yields the identical
        // cells in exactly reversed order (the absolute-position
        // rounding has no direction bias).
        for ax in 0..=12u32 {
            for ay in 0..=12u32 {
                for bx in 0..=12u32 {
                    for by in 0..=12u32 {
                        let cells = wall_line_cells(ax, ay, bx, by);
                        assert_eq!(
                            cells.len(),
                            1 + (bx as i64 - ax as i64)
                                .unsigned_abs()
                                .max((by as i64 - ay as i64).unsigned_abs())
                                as usize
                        );
                        for w in cells.windows(2) {
                            let step = (w[1].0 as i64 - w[0].0 as i64)
                                .abs()
                                .max((w[1].1 as i64 - w[0].1 as i64).abs());
                            assert_eq!(step, 1, "king-adjacent step");
                        }
                        let mut sorted = cells.clone();
                        sorted.sort();
                        sorted.dedup();
                        assert_eq!(sorted.len(), cells.len(), "no repeated cell");
                        let mut rev = wall_line_cells(bx, by, ax, ay);
                        rev.reverse();
                        assert_eq!(cells, rev, "reversed drag, identical order");
                    }
                }
            }
        }
    }

    #[test]
    fn footprint_center_is_the_shared_corner() {
        // M7 geometry (120×120 cells, world [-240, 240)): cell (0,0)
        // spans [-240,-236); the 2x2 footprint there spans
        // [-240,-232) on both axes, so its center is (-236, -236).
        let (x, y) = footprint_center(0, 0, 2);
        assert_eq!((x, y), (I16F16::from_num(-236), I16F16::from_num(-236)));
        // World center: footprint (59,59) spans [-4, 4) → center 0.
        let (x, y) = footprint_center(59, 59, 2);
        assert_eq!((x, y), (I16F16::from_num(0), I16F16::from_num(0)));
        // A 3×3 footprint at (20,20) spans [-160,-148) → center -154.
        let (x, y) = footprint_center(20, 20, 3);
        assert_eq!((x, y), (I16F16::from_num(-154), I16F16::from_num(-154)));
    }
}
