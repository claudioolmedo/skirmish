//! Unexplored-ground placement refusal (BLD-01 amendment,
//! 2026-11-15, Claudio's authorization — the documented DE rule):
//! a building may only be founded on ground the placing player
//! has explored at some point. Unexplored cells refuse the order
//! whole (no foundation, no charge); explored-but-currently-
//! fogged cells stay legal; a dragged wall line founds only its
//! explored segments and charges exactly for those, consistent
//! with the occupied-tile skip.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, EntityKind, ResourceKind, Sim, OWNER_PLAYER,
};

const HOUSE: BuildingKind = BuildingKind::House;
const WALL: BuildingKind = BuildingKind::PalisadeWall;

fn villager_of(sim: &Sim, owner: u8) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == EntityKind::Villager)
        .unwrap()
        .id
}

fn scout_of(sim: &Sim, owner: u8) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == EntityKind::ScoutCavalry)
        .unwrap()
        .id
}

fn cell_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn wood(sim: &Sim) -> I16F16 {
    sim.stockpile().amount(ResourceKind::Wood)
}

/// Ground a House could stand on but for exploration: every
/// footprint cell open land (not blocked, not shallows, no live
/// node, outside every Town Center's reserved 4×4) and at least
/// one open ring cell — scanned in rings around `(cx, cy)`.
/// Whether the cells are explored is the caller's concern.
fn open_house_anchor(sim: &Sim, cx: u32, cy: u32) -> (u32, u32) {
    let size = HOUSE.footprint().w;
    let tc_cells: Vec<(u32, u32)> = sim
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::TownCenter && !e.dead)
        .map(|e| sim.grid().world_to_cell(e.pos_x, e.pos_y))
        .collect();
    let clear = |sim: &Sim, ax: u32, ay: u32| -> bool {
        let mut ring_open = false;
        for dy in 0..size {
            for dx in 0..size {
                let (x, y) = (ax + dx, ay + dy);
                if sim.grid().is_blocked(x, y) || sim.grid().is_shallows(x, y) {
                    return false;
                }
                if sim
                    .nodes()
                    .iter()
                    .any(|n| !n.is_depleted() && sim.grid().world_to_cell(n.pos_x, n.pos_y) == (x, y))
                {
                    return false;
                }
                for &(tx, ty) in &tc_cells {
                    if x + 1 >= tx && x <= tx + 2 && y + 1 >= ty && y <= ty + 2 {
                        return false;
                    }
                }
            }
        }
        for dy in -1i32..=(size as i32) {
            for dx in -1i32..=(size as i32) {
                let (x, y) = (ax as i32 + dx, ay as i32 + dy);
                if x < 0 || y < 0 {
                    continue;
                }
                let (x, y) = (x as u32, y as u32);
                if x >= ax && x < ax + size && y >= ay && y < ay + size {
                    continue;
                }
                if !sim.grid().is_blocked(x, y) {
                    ring_open = true;
                }
            }
        }
        ring_open
    };
    for r in 0..=40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if r > 0 && dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (cx as i32 + dx, cy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                if clear(sim, ax, ay) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no open house ground near ({cx}, {cy})");
}

fn footprint_explored(sim: &Sim, owner: u8, ax: u32, ay: u32, size: u32) -> bool {
    (0..size).all(|dy| (0..size).all(|dx| sim.cell_explored_by(owner, ax + dx, ay + dy)))
}

fn footprint_visibility(sim: &Sim, owner: u8, ax: u32, ay: u32, size: u32) -> Vec<u8> {
    let mut out = Vec::new();
    for dy in 0..size {
        for dx in 0..size {
            out.push(sim.cell_visibility(owner, ax + dx, ay + dy));
        }
    }
    out
}

/// A far, open, still-unexplored House plot: scan around the map
/// centre for open ground whose whole footprint the player has
/// never seen, far from both starts.
fn far_unexplored_plot(sim: &Sim) -> (u32, u32) {
    let (ax, ay) = open_house_anchor(sim, 60, 30);
    assert!(
        !footprint_explored(sim, OWNER_PLAYER, ax, ay, HOUSE.footprint().w),
        "plot at ({ax}, {ay}) must start unexplored"
    );
    (ax, ay)
}

#[test]
fn placement_on_unexplored_ground_is_refused_and_charges_nothing() {
    let mut sim = Sim::new(31);
    let villager = villager_of(&sim, OWNER_PLAYER);
    let (ax, ay) = far_unexplored_plot(&sim);
    let (x, y) = cell_point(&sim, ax, ay);

    // The read-only query and the command path agree: refused.
    assert!(
        !sim.can_build(villager, HOUSE, x, y),
        "can_build must refuse unexplored ground at ({ax}, {ay})"
    );
    let wood0 = wood(&sim);
    let buildings0 = sim.buildings().len();
    sim.push_command(Command::Build {
        entity: villager,
        kind: HOUSE,
        x,
        y,
    });
    sim.tick();
    assert_eq!(
        sim.buildings().len(),
        buildings0,
        "a refused placement founds nothing"
    );
    assert_eq!(wood(&sim), wood0, "a refused placement charges nothing");
}

#[test]
fn placement_on_explored_but_fogged_ground_is_allowed() {
    let mut sim = Sim::new(31);
    let villager = villager_of(&sim, OWNER_PLAYER);
    let scout = scout_of(&sim, OWNER_PLAYER);
    let (ax, ay) = far_unexplored_plot(&sim);
    let size = HOUSE.footprint().w;
    let (px, py) = cell_point(&sim, ax, ay);

    // Ride the Scout to the plot: the ground becomes explored.
    sim.push_command(Command::Move {
        entity: scout,
        x: px,
        y: py,
    });
    let mut explored = false;
    for _ in 0..20_000 {
        sim.tick();
        if footprint_explored(&sim, OWNER_PLAYER, ax, ay, size) {
            explored = true;
            break;
        }
    }
    assert!(explored, "the Scout never explored the plot");

    // Ride home: the plot drops to explored memory (1) — known,
    // no longer seen.
    let tc = sim.town_center();
    let (hx, hy) = (tc.pos_x, tc.pos_y);
    sim.push_command(Command::Move {
        entity: scout,
        x: hx,
        y: hy,
    });
    let mut fogged = false;
    for _ in 0..20_000 {
        sim.tick();
        let vis = footprint_visibility(&sim, OWNER_PLAYER, ax, ay, size);
        if vis.iter().all(|&v| v == 1) {
            fogged = true;
            break;
        }
    }
    assert!(fogged, "the plot never dropped to explored-only memory");

    // Explored-but-fogged is buildable, at the exact House price.
    let (x, y) = cell_point(&sim, ax, ay);
    assert!(
        sim.can_build(villager, HOUSE, x, y),
        "can_build must allow explored-but-fogged ground"
    );
    let wood0 = wood(&sim);
    sim.push_command(Command::Build {
        entity: villager,
        kind: HOUSE,
        x,
        y,
    });
    sim.tick();
    let founded = sim
        .buildings()
        .iter()
        .find(|b| b.kind == HOUSE && b.owner == OWNER_PLAYER && b.cell_x == ax && b.cell_y == ay);
    assert!(founded.is_some(), "the House foundation must stand");
    assert_eq!(
        wood(&sim),
        wood0 - HOUSE.wood_cost(),
        "exactly the House price is charged"
    );
}

#[test]
fn wall_line_partially_over_unexplored_places_only_explored_segments() {
    let mut sim = Sim::new(31);
    let villager = villager_of(&sim, OWNER_PLAYER);
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);

    // Walk east along the TC's row to the exploration frontier:
    // the first cell the player has never seen. The line runs
    // from three cells inside explored ground to three cells
    // past the frontier — every cell open wall ground.
    let mut frontier = None;
    for cx in (tcx + 4)..(tcx + 40) {
        if !sim.cell_explored_by(OWNER_PLAYER, cx, tcy) {
            frontier = Some(cx);
            break;
        }
    }
    let frontier = frontier.expect("no exploration frontier east of the TC");
    let (x0, x1) = (frontier - 3, frontier + 3);
    for cx in x0..=x1 {
        assert!(
            !sim.grid().is_blocked(cx, tcy) && !sim.grid().is_shallows(cx, tcy),
            "line cell ({cx}, {tcy}) must be open ground"
        );
        assert!(
            sim.nodes()
                .iter()
                .all(|n| n.is_depleted() || sim.grid().world_to_cell(n.pos_x, n.pos_y) != (cx, tcy)),
            "line cell ({cx}, {tcy}) must hold no live node"
        );
    }
    let explored_cells: Vec<u32> = (x0..=x1)
        .filter(|&cx| sim.cell_explored_by(OWNER_PLAYER, cx, tcy))
        .collect();
    assert!(
        !explored_cells.is_empty() && explored_cells.len() < 7,
        "the line must straddle the frontier"
    );

    let (sx, sy) = cell_point(&sim, x0, tcy);
    let (ex, ey) = cell_point(&sim, x1, tcy);
    let wood0 = wood(&sim);
    sim.push_command(Command::BuildLine {
        entity: villager,
        kind: WALL,
        x0: sx,
        y0: sy,
        x1: ex,
        y1: ey,
    });
    sim.tick();

    let founded: Vec<u32> = sim
        .buildings()
        .iter()
        .filter(|b| b.kind == WALL && b.owner == OWNER_PLAYER)
        .map(|b| b.cell_x)
        .collect();
    assert_eq!(
        founded, explored_cells,
        "only the explored segments are founded"
    );
    assert_eq!(
        wood(&sim),
        wood0 - WALL.wood_cost() * I16F16::from_num(explored_cells.len() as i32),
        "the charge is exactly the founded segments' price"
    );
}
