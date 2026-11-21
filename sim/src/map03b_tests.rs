//! MAP03-B tests: the placement matrix — every building
//! category × every terrain tag, read through the real
//! `Sim::can_build` (the `validate_build` verdict a player's
//! order gets) — plus the generated-map census proving no
//! Arabia map emits a MAP03-B tag yet (the golden-hash guard:
//! definition-only tags cannot move generated terrain bytes).
//!
//! The passability half of the matrix (both A* domains per tag)
//! is pinned in `grid::tests::terrain_table_covers_the_map03b_tags`.
//!
//! Expected matrix (DE rows, sk-02's class table):
//!
//! | tag | Normal | Wall | Dock | FishTrap |
//! |---|---|---|---|---|
//! | land / dirt / sand / snow (class 1) | yes | yes | no | no |
//! | deep water (7) | no | no | no* | yes |
//! | shallows (6) | no | no | yes† | yes |
//! | ice (4) | no | no | no | no |
//! | ice navigable (5) | no | no | no | yes |
//! | beach (2, WallsDocks) | no | yes | yes | yes |
//! | mangrove shallows (6b, build Full) | yes | yes | yes† | yes |
//!
//! *A pure deep-water footprint has no land side; a Dock is
//! admitted only straddling (its W1 pond-shore proof stands).
//! †The footprint sits at the patch's coast edge, where the
//! amphibious cells beside coast land count as the land side —
//! the shallows reading W1 already shipped, mangrove's twin.

use crate::build::BuildingKind;
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_PLAYER};
use crate::grid::{
    TERRAIN_BEACH, TERRAIN_DIRT, TERRAIN_ICE, TERRAIN_ICE_NAV, TERRAIN_LAND,
    TERRAIN_MANGROVE_SHALLOWS, TERRAIN_SAND, TERRAIN_SHALLOWS, TERRAIN_SNOW, TERRAIN_WATER,
};
use crate::tech::Age;
use crate::{player_slot, Sim};
use fixed::types::I16F16;

const TAGS: [u8; 10] = [
    TERRAIN_LAND,
    TERRAIN_WATER,
    TERRAIN_SHALLOWS,
    TERRAIN_DIRT,
    TERRAIN_SAND,
    TERRAIN_SNOW,
    TERRAIN_ICE,
    TERRAIN_ICE_NAV,
    TERRAIN_BEACH,
    TERRAIN_MANGROVE_SHALLOWS,
];
/// Patch geometry: cells 52..72 on both axes are painted with
/// the tag under test; anchors sit inside it (the Dock's at the
/// west coast edge, where the patch meets ordinary land).
const PATCH: (u32, u32) = (52, 72);

fn a_villager(sim: &Sim) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER && !e.dead)
        .map(|e| e.id)
        .expect("a starting villager")
}

/// Stage a fresh dry-seed sim whose centre patch is one solid
/// terrain tag, with the player at Imperial Age, deep stockpiles,
/// no live node or foreign unit on the patch, and the patch
/// fully explored by the player.
fn staged(tag: u8) -> Sim {
    let mut sim = Sim::new(7);
    let slot = player_slot(OWNER_PLAYER).expect("player slot");
    sim.players[slot].age = Age::Imperial;
    for kind in [
        ResourceKind::Wood,
        ResourceKind::Food,
        ResourceKind::Gold,
        ResourceKind::Stone,
    ] {
        sim.stockpile_mut(OWNER_PLAYER)
            .add(kind, I16F16::from_num(30_000));
    }
    let (lo, hi) = PATCH;
    // Every other living entity waits in the far corner, so the
    // standing-unit check can only ever read the ordering
    // builder (exempt) on the patch.
    let (cx, cy) = sim.grid().cell_center(2, 2);
    for e in sim.entities.iter_mut() {
        e.pos_x = cx;
        e.pos_y = cy;
    }
    // Paint exploration FIRST, while the patch is still ordinary
    // land a villager can stand on: ride the villager over the
    // patch on a coarse lattice, ticking between stops, until
    // every patch cell is explored (BLD-01's gate must not be
    // the variable; explored memory survives the terrain paint
    // below — a villager staged on deep water explores nothing).
    let villager = a_villager(&sim);
    let mut stops: Vec<(u32, u32)> = Vec::new();
    let mut y = lo;
    while y < hi {
        let mut x = lo;
        while x < hi {
            stops.push((x, y));
            x += 3;
        }
        y += 3;
    }
    for (sx, sy) in stops {
        let (wx, wy) = sim.grid().cell_center(sx, sy);
        let e = sim.entities.iter_mut().find(|e| e.id == villager).unwrap();
        e.pos_x = wx;
        e.pos_y = wy;
        for _ in 0..12 {
            sim.tick();
        }
        if (lo..hi).all(|cy| (lo..hi).all(|cx| sim.cell_explored_by(OWNER_PLAYER, cx, cy))) {
            break;
        }
    }
    assert!(
        (lo..hi).all(|cy| (lo..hi).all(|cx| sim.cell_explored_by(OWNER_PLAYER, cx, cy))),
        "tag {tag}: the patch never became fully explored"
    );
    // Now paint the tag under test and clear its obstructions.
    for y in lo..hi {
        for x in lo..hi {
            sim.grid.set_terrain(x, y, tag);
            sim.grid.set_height(x, y, 0);
            sim.grid.set_node_block(x, y, false);
        }
    }
    for n in sim.nodes.iter_mut() {
        let (cx, cy) = sim.grid.world_to_cell(n.pos_x, n.pos_y);
        if (lo..hi).contains(&cx) && (lo..hi).contains(&cy) {
            n.amount_left = I16F16::ZERO;
        }
    }
    sim
}

/// Park the ordering villager on the ring cell west of an
/// anchor and let its position settle, then ask the sim.
fn can_build_from_ring(sim: &mut Sim, kind: BuildingKind, anchor: (u32, u32)) -> bool {
    let villager = a_villager(sim);
    let (wx, wy) = sim.grid().cell_center(anchor.0 - 1, anchor.1 + 1);
    let e = sim.entities.iter_mut().find(|e| e.id == villager).unwrap();
    e.pos_x = wx;
    e.pos_y = wy;
    for _ in 0..6 {
        sim.tick();
    }
    let (px, py) = sim.grid().cell_center(anchor.0, anchor.1);
    sim.can_build(villager, kind, px, py)
}

#[test]
fn placement_matrix_every_category_by_every_tag() {
    // (Normal, Wall, Dock) expectations per tag, in TAGS order.
    let expect: [(bool, bool, bool); 10] = [
        (true, true, false),   // land
        (false, false, false), // deep water (no land side for a Dock)
        (false, false, true),  // shallows (coast-edge straddle)
        (true, true, false),   // dirt
        (true, true, false),   // sand
        (true, true, false),   // snow
        (false, false, false), // ice
        (false, false, false), // ice navigable
        (false, true, true),   // beach (WallsDocks)
        (true, true, true),    // mangrove shallows (build Full)
    ];
    // Representative kinds per category: House (Normal 2×2),
    // Barracks (Normal 3×3), Castle (Normal 4×4), Wonder
    // (Normal 5×5), WatchTower (tower 1×1, Normal class),
    // PalisadeWall + StoneWall (walls), Dock (Shoreline).
    let normals = [
        BuildingKind::House,
        BuildingKind::Barracks,
        BuildingKind::Castle,
        BuildingKind::Wonder,
        BuildingKind::WatchTower,
    ];
    for (i, &tag) in TAGS.iter().enumerate() {
        let (want_normal, want_wall, want_dock) = expect[i];
        let mut sim = staged(tag);
        for kind in normals {
            let got = can_build_from_ring(&mut sim, kind, (58, 58));
            assert_eq!(got, want_normal, "tag {tag}: {kind:?} (Normal category)");
        }
        for kind in [BuildingKind::PalisadeWall, BuildingKind::StoneWall] {
            let got = can_build_from_ring(&mut sim, kind, (60, 60));
            assert_eq!(got, want_wall, "tag {tag}: {kind:?} (wall category)");
        }
        let got = can_build_from_ring(&mut sim, BuildingKind::Dock, (52, 58));
        assert_eq!(got, want_dock, "tag {tag}: Dock (Shoreline category)");
    }
}

#[test]
fn fish_trap_matrix_reads_water_side() {
    // The Fish Trap (Water category) is admitted exactly on the
    // water-side tags: deep water, shallows, ice navigable,
    // beach, mangrove shallows.
    let expect = [
        false, // land
        true,  // deep water
        true,  // shallows
        false, // dirt
        false, // sand
        false, // snow
        false, // ice
        true,  // ice navigable
        true,  // beach
        true,  // mangrove shallows
    ];
    for (i, &tag) in TAGS.iter().enumerate() {
        let mut sim = staged(tag);
        // A Fishing Ship rides to the trap cell (58, 60).
        let (wx, wy) = sim.grid().cell_center(58, 60);
        sim.entities.push(Entity::spawned(
            9999,
            EntityKind::FishingShip,
            OWNER_PLAYER,
            wx,
            wy,
        ));
        for _ in 0..12 {
            sim.tick();
        }
        let got = sim.can_build(9999, BuildingKind::FishTrap, wx, wy);
        assert_eq!(got, expect[i], "tag {tag}: Fish Trap (Water category)");
    }
}

#[test]
fn generated_maps_emit_no_map03b_tag() {
    // Census over the soak seeds (the scoping handoff's probe
    // set) plus a wider sweep: generated Arabia terrain is
    // still only tags 0/1/2, so the golden hash cannot move.
    let mut seeds: Vec<u64> = vec![1, 2, 3, 7, 42, 99, 1234, 5555];
    seeds.extend(0..64);
    for seed in seeds {
        let m = crate::mapgen::generate(seed);
        let census: Vec<u8> = {
            let mut v: Vec<u8> = m.grid.terrain_bytes().to_vec();
            v.sort_unstable();
            v.dedup();
            v
        };
        assert!(
            census.iter().all(|&t| t <= TERRAIN_SHALLOWS),
            "seed {seed}: generated tags {census:?} include a MAP03-B tag"
        );
    }
}
