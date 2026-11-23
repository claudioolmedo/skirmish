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
use crate::econ::{NodeKind, ResourceKind};
use crate::entity::{Entity, EntityKind, OWNER_PLAYER};
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

/// MAP-05 re-stage (map05lib): seed 42's Arabia is DRY since
/// MAP-05's `create_elevation` started consuming the shared RNG
/// stream ahead of pond placement (probe: seed 42 now generates
/// zero water cells; 55 of seeds 1..=120 still carry a dockable,
/// fish-stocked pond — the mapgen guarantees are intact, only
/// this suite's pinned seed moved). Derive the pond from the
/// generated maps at runtime: the first seed whose pond offers
/// everything this suite stages on.
fn pond_sim() -> Sim {
    for seed in 1..=400u64 {
        let sim = Sim::new(seed);
        if dock_anchor(&sim).is_some() {
            return sim;
        }
    }
    panic!("no seed in 1..=400 offers a Dock site");
}

#[test]
fn dock_builds_to_completion_on_a_pond_shore() {
    // The pond seed's rim offers Dock sites by the mapgen
    // validation guarantee (see `pond_sim`).
    let mut sim = pond_sim();
    let (ax, ay) = dock_anchor(&sim).expect("pond seed: a legal Dock site on the pond shore");
    let villager = a_villager(&sim);
    // Stage the builder on the site and let the fog settle:
    // the pond starts unexplored, and BLD-01's 2026-11-15
    // amendment refuses unexplored ground — the standing
    // builder's own sight explores the footprint within a
    // few visibility recomputes.
    {
        let (sx, sy) = sim.grid().cell_center(ax + 1, ay + 1);
        let e = sim.entities.iter_mut().find(|e| e.id == villager).unwrap();
        e.pos_x = sx;
        e.pos_y = sy;
    }
    for _ in 0..600 {
        if (0..3).all(|oy| (0..3).all(|ox| sim.cell_explored_by(OWNER_PLAYER, ax + ox, ay + oy))) {
            break;
        }
        sim.tick();
    }
    assert!(
        (0..3).all(|oy| (0..3).all(|ox| sim.cell_explored_by(OWNER_PLAYER, ax + ox, ay + oy))),
        "the Dock site never became explored"
    );
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
        let mut sim = Sim::new(seed);
        // No named grid borrow: the staging below mutates the
        // sim, so every grid read goes through a transient
        // `sim.grid()` call.
        let villager = a_villager(&sim);
        for ay in 0..=117u32 {
            for ax in 0..=117u32 {
                let cells: Vec<(u32, u32)> = (0..3)
                    .flat_map(|dy| (0..3).map(move |dx| (ax + dx, ay + dy)))
                    .collect();
                let has_shallows = cells.iter().any(|&(x, y)| sim.grid().is_shallows(x, y));
                let has_deep = cells.iter().any(|&(x, y)| sim.grid().is_water(x, y));
                if !has_shallows || has_deep {
                    continue;
                }
                let clean = cells.iter().all(|&(x, y)| {
                    !sim.grid().is_node_blocked(x, y)
                        && !sim.grid().is_dynamic(x, y)
                        && sim
                            .nodes()
                            .iter()
                            .all(|n| {
                                n.is_depleted()
                                    || sim.grid().world_to_cell(n.pos_x, n.pos_y) != (x, y)
                            })
                        && sim.entities().iter().all(|e| {
                            e.dead || sim.grid().world_to_cell(e.pos_x, e.pos_y) != (x, y)
                        })
                });
                if !clean || !sim.grid().dock_placement_ok(ax, ay, 3) {
                    continue;
                }
                // Stage the builder on the site and let the fog
                // settle, so terrain alone decides the pair below
                // (BLD-01's 2026-11-15 amendment refuses
                // unexplored ground before terrain is consulted).
                {
                    let (sx, sy) = sim.grid().cell_center(ax + 1, ay + 1);
                    let e = sim.entities.iter_mut().find(|e| e.id == villager).unwrap();
                    e.pos_x = sx;
                    e.pos_y = sy;
                }
                for _ in 0..600 {
                    if (0..3).all(|oy| {
                        (0..3).all(|ox| sim.cell_explored_by(OWNER_PLAYER, ax + ox, ay + oy))
                    }) {
                        break;
                    }
                    sim.tick();
                }
                assert!(
                    (0..3).all(|oy| {
                        (0..3).all(|ox| sim.cell_explored_by(OWNER_PLAYER, ax + ox, ay + oy))
                    }),
                    "seed {seed}: the shallows site never became explored"
                );
                let (px, py) = sim.grid().cell_center(ax, ay);
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

#[test]
fn dock_may_overlap_a_fish_shoal_and_the_fish_stays() {
    // (2026-11-16, Claudio's DE rule): a fish shoal does NOT
    // block the Dock. Scan pond seeds for a shoreline straddle
    // whose footprint holds a live fish node and nothing else
    // that refuses (no tree block, no dynamic cell, no other
    // live node, no standing unit).
    let mut staged: Option<(Sim, u32, u32, u32, u32)> = None;
    for seed in 1..=60u64 {
        let mut sim = Sim::new(seed);
        let villager = a_villager(&sim);
        'scan: for ay in 0..=117u32 {
            for ax in 0..=117u32 {
                if !sim.grid().dock_terrain_ok(ax, ay, 3) {
                    continue;
                }
                let cells: Vec<(u32, u32)> = (0..3)
                    .flat_map(|dy| (0..3).map(move |dx| (ax + dx, ay + dy)))
                    .collect();
                let cell_of = |sim: &Sim, x: I16F16, y: I16F16| sim.grid().world_to_cell(x, y);
                let fish = sim.nodes().iter().find(|n| {
                    n.kind.is_fish()
                        && !n.is_depleted()
                        && cells.contains(&cell_of(&sim, n.pos_x, n.pos_y))
                });
                let Some(fish) = fish else { continue };
                let fish_id = fish.id;
                let clean = cells.iter().all(|&(x, y)| {
                    !sim.grid().is_node_blocked(x, y)
                        && !sim.grid().is_dynamic(x, y)
                        && sim.nodes().iter().all(|n| {
                            n.is_depleted()
                                || n.kind.is_fish()
                                || cell_of(&sim, n.pos_x, n.pos_y) != (x, y)
                        })
                        && sim.entities().iter().all(|e| {
                            e.dead || cell_of(&sim, e.pos_x, e.pos_y) != (x, y)
                        })
                });
                if !clean {
                    continue;
                }
                staged = Some((sim, villager, ax, ay, fish_id));
                break 'scan;
            }
        }
        if staged.is_some() {
            break;
        }
    }
    let (mut sim, villager, ax, ay, fish_id) =
        staged.expect("no fish-overlapping Dock straddle found in 60 seeds");
    // Stage the builder on the site and let the fog settle
    // (BLD-01: unexplored ground refuses before anything else
    // is consulted — the W1 tests' pattern).
    {
        let (sx, sy) = sim.grid().cell_center(ax + 1, ay + 1);
        let e = sim.entities.iter_mut().find(|e| e.id == villager).unwrap();
        e.pos_x = sx;
        e.pos_y = sy;
    }
    for _ in 0..600 {
        if (0..3).all(|oy| (0..3).all(|ox| sim.cell_explored_by(OWNER_PLAYER, ax + ox, ay + oy))) {
            break;
        }
        sim.tick();
    }
    assert!(
        (0..3).all(|oy| (0..3).all(|ox| sim.cell_explored_by(OWNER_PLAYER, ax + ox, ay + oy))),
        "the fish-straddle site never became explored"
    );
    let (px, py) = sim.grid().cell_center(ax, ay);
    assert!(
        sim.can_build(villager, BuildingKind::Dock, px, py),
        "the Dock must accept a footprint that only overlaps fish"
    );
    // The exemption is fish-specific: stage a live gold node
    // onto a footprint cell and the same anchor refuses again;
    // move it back and the anchor accepts again. (No ticks run
    // while the node is staged, so nothing else reads it.)
    let gold = sim
        .nodes
        .iter()
        .position(|n| n.kind == NodeKind::GoldMine)
        .expect("the map stocks gold");
    let (gx, gy) = (sim.nodes[gold].pos_x, sim.nodes[gold].pos_y);
    let (fx, fy) = sim.grid().cell_center(ax, ay);
    sim.nodes[gold].pos_x = fx;
    sim.nodes[gold].pos_y = fy;
    assert!(
        !sim.can_build(villager, BuildingKind::Dock, px, py),
        "a live gold node on the footprint must still refuse the Dock"
    );
    sim.nodes[gold].pos_x = gx;
    sim.nodes[gold].pos_y = gy;
    assert!(sim.can_build(villager, BuildingKind::Dock, px, py));
    // Place it for real: the foundation lands on the shoal and
    // the fish node is NOT swallowed — it stays in place, at
    // its full amount, under the Dock.
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(1000));
    let fish_amount = sim
        .nodes()
        .iter()
        .find(|n| n.id == fish_id)
        .expect("the overlapped fish")
        .amount_left;
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
        .expect("the Dock foundation lands over the shoal");
    assert_eq!((placed.cell_x, placed.cell_y), (ax, ay));
    let fish = sim
        .nodes()
        .iter()
        .find(|n| n.id == fish_id)
        .expect("the overlapped fish node was removed by placement");
    assert_eq!(fish.amount_left, fish_amount, "placement ate the fish");
    // And the shoal's remaining tiles stay fishable beside the
    // Dock: a Fishing Ship works another fish node of the same
    // pond (not under the footprint) and the node depletes.
    let cells: Vec<(u32, u32)> = (0..3)
        .flat_map(|dy| (0..3).map(move |dx| (ax + dx, ay + dy)))
        .collect();
    let other = sim
        .nodes()
        .iter()
        .find(|n| {
            n.kind.is_fish()
                && n.id != fish_id
                && !cells.contains(&sim.grid().world_to_cell(n.pos_x, n.pos_y))
        })
        .expect("the pond stocks another fish node")
        .id;
    let (ox, oy) = {
        let n = sim.nodes().iter().find(|n| n.id == other).unwrap();
        (n.pos_x, n.pos_y)
    };
    sim.entities.push(Entity::spawned(
        950,
        EntityKind::FishingShip,
        OWNER_PLAYER,
        ox,
        oy,
    ));
    let before = sim
        .nodes()
        .iter()
        .find(|n| n.id == other)
        .unwrap()
        .amount_left;
    sim.push_command(Command::Gather {
        entity: 950,
        node: other,
    });
    let mut worked = false;
    for _ in 0..60_000 {
        sim.tick();
        if sim.nodes().iter().find(|n| n.id == other).unwrap().amount_left < before {
            worked = true;
            break;
        }
    }
    assert!(worked, "the ship never gathered from the shoal beside the Dock");
}
