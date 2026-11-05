//! Water W1 tests (complete W1 — the Dock half): the Dock's
//! placement predicate end-to-end (BLD-14) — refused on dry land
//! and in open water, accepted straddling a generated pond's
//! shallows rim, built to completion by a villager who walks to
//! the shore — and the Normal-placement shallows refusal (no
//! building stands in the shallows except the Dock). The Dock
//! row itself (cost/time/HP/armor/LoS/drop-off/garrison data) is
//! pinned in `build::tests::dock_row_is_the_water_spec_row`; the
//! grid predicate's cell cases are pinned in `grid::tests`.

use crate::build::BuildingKind;
use crate::econ::ResourceKind;
use crate::entity::{EntityKind, OWNER_PLAYER};
use crate::{Command, Sim};
use fixed::types::I16F16;

/// The first player villager of a fresh sandbox sim.
fn a_villager(sim: &Sim) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER && !e.dead)
        .map(|e| e.id)
        .expect("a starting villager")
}

/// Scan a fresh sim's map for a 3×3 anchor the Dock predicate
/// accepts outright (terrain + obstruction halves, on the grid).
fn dock_anchor(sim: &Sim) -> Option<(u32, u32)> {
    let grid = sim.grid();
    for ay in 0..=117u32 {
        for ax in 0..=117u32 {
            if grid.dock_placement_ok(ax, ay, 3) {
                return Some((ax, ay));
            }
        }
    }
    None
}

#[test]
fn dock_builds_to_completion_on_a_pond_shore() {
    // Seed 42's Arabia carries a pond (26 deep cells before the
    // rim pass — measured by the W1 probe); its rim offers Dock
    // sites by the mapgen validation guarantee.
    let mut sim = Sim::new(42);
    let (ax, ay) = dock_anchor(&sim).expect("seed 42: a legal Dock site on the pond shore");
    let villager = a_villager(&sim);
    let (px, py) = sim.grid().cell_center(ax, ay);
    assert!(
        sim.can_build(villager, BuildingKind::Dock, px, py),
        "the sim must accept the predicate's site"
    );
    // Top the stockpile up so the 150 wood is never the question.
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(1000));
    let wood_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood);
    sim.push_command(Command::Build {
        entity: villager,
        kind: BuildingKind::Dock,
        x: px,
        y: py,
    });
    sim.tick();
    let placed = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Dock)
        .expect("the Dock foundation lands the tick the order applies");
    assert_eq!((placed.cell_x, placed.cell_y), (ax, ay));
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        wood_before - I16F16::from_num(150),
        "placement spends exactly the Dock's 150 wood"
    );
    // The finished Dock stands half in the water: at least one
    // footprint cell is water-side, and the footprint blocks the
    // water domain too (ships do not sail through Docks).
    let cells = placed.footprint_cells();
    assert!(
        cells.iter().any(|&(cx, cy)| sim.grid().is_water_side(cx, cy)),
        "the Dock footprint touches the water side"
    );
    // Build it out: the lone builder walks to the shore and works
    // the 2100-tick build (generous cap for the walk).
    let bid = placed.id;
    let mut done = false;
    for _ in 0..120_000 {
        sim.tick();
        if sim.buildings().iter().any(|b| b.id == bid && b.complete) {
            done = true;
            break;
        }
    }
    assert!(done, "the Dock completes under its builder");
    let dock = sim
        .buildings()
        .iter()
        .find(|b| b.id == bid)
        .expect("the completed Dock stands");
    assert_eq!(dock.hp, I16F16::from_num(1800), "a fresh Dock stands at full HP");
    for &(cx, cy) in &cells {
        assert!(
            sim.grid().is_dynamic(cx, cy),
            "footprint cell {cx},{cy} blocks after completion"
        );
    }
    // W1 scope: the Dock trains NOTHING yet (ships are W2) — its
    // roster is empty for every kind in the game.
    for kind in [
        EntityKind::Villager,
        EntityKind::Militia,
        EntityKind::Archer,
        EntityKind::ScoutCavalry,
        EntityKind::Knight,
        EntityKind::Monk,
        EntityKind::TradeCart,
    ] {
        assert!(!BuildingKind::Dock.trains(kind), "the W1 Dock trains nothing");
    }
}

#[test]
fn dock_is_refused_on_dry_land() {
    let sim = Sim::new(42);
    let villager = a_villager(&sim);
    // The Town Center's own ground: all-land 3×3 (and reserved) —
    // no water side, so the predicate refuses before any other
    // check matters. Scan a coarse lattice of anchors far from
    // the pond instead: every all-land anchor must refuse.
    let grid = sim.grid();
    let mut refused = 0;
    for ay in (0..=117u32).step_by(7) {
        for ax in (0..=117u32).step_by(7) {
            let (px, py) = grid.cell_center(ax, ay);
            let all_land = (0..3).all(|dy| {
                (0..3).all(|dx| grid.terrain_at(ax + dx, ay + dy) == crate::grid::TERRAIN_LAND)
            });
            if all_land {
                assert!(
                    !sim.can_build(villager, BuildingKind::Dock, px, py),
                    "a Dock was placeable on dry land at {ax},{ay}"
                );
                refused += 1;
            }
        }
    }
    assert!(refused > 0, "the lattice never sampled dry land");
}

#[test]
fn normal_buildings_are_refused_on_shallows() {
    // Find (over pond seeds) a 3×3 footprint that touches
    // shallows but no deep water, is otherwise clean land (no
    // node block, no building, no live node, no standing unit),
    // and straddles for the Dock: the Barracks must refuse it
    // (the shallows rule) while the Dock accepts the same
    // anchor — the pair isolates terrain as the deciding rule.
    let mut checked = false;
    for seed in 1..=40u64 {
        let sim = Sim::new(seed);
        let grid = sim.grid();
        let villager = a_villager(&sim);
        for ay in 0..=117u32 {
            for ax in 0..=117u32 {
                let cells: Vec<(u32, u32)> = (0..3)
                    .flat_map(|dy| (0..3).map(move |dx| (ax + dx, ay + dy)))
                    .collect();
                let has_shallows = cells.iter().any(|&(x, y)| grid.is_shallows(x, y));
                let has_deep = cells.iter().any(|&(x, y)| grid.is_water(x, y));
                if !has_shallows || has_deep {
                    continue;
                }
                let clean = cells.iter().all(|&(x, y)| {
                    !grid.is_node_blocked(x, y)
                        && !grid.is_dynamic(x, y)
                        && sim
                            .nodes()
                            .iter()
                            .all(|n| n.is_depleted() || grid.world_to_cell(n.pos_x, n.pos_y) != (x, y))
                        && sim.entities().iter().all(|e| {
                            e.dead || grid.world_to_cell(e.pos_x, e.pos_y) != (x, y)
                        })
                });
                if !clean || !grid.dock_placement_ok(ax, ay, 3) {
                    continue;
                }
                let (px, py) = grid.cell_center(ax, ay);
                assert!(
                    !sim.can_build(villager, BuildingKind::Barracks, px, py),
                    "seed {seed}: a Barracks was placeable in the shallows at {ax},{ay}"
                );
                assert!(
                    sim.can_build(villager, BuildingKind::Dock, px, py),
                    "seed {seed}: the Dock must accept the shallows straddle at {ax},{ay}"
                );
                checked = true;
                break;
            }
            if checked {
                break;
            }
        }
        if checked {
            break;
        }
    }
    assert!(checked, "no clean shallows straddle found in 40 seeds");
}
