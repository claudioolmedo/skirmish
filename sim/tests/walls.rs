//! Palisade Wall tests (BLD-13): dragged-line placement with
//! per-segment validation and charging, the villager walking the
//! line, blocking/pathing for owner and enemy alike, the Town
//! Center reservation leaving gaps, wood truncation, foundation
//! delete refunds (BLD-02), repair (BLD-17), and the age-up
//! exclusion (walls never count toward TEC-01).
//!
//! BLD-20 adds: the ghost==founded regression across drag shapes
//! (the client's preview and the sim's expansion are one line),
//! segment founding under standing units (DE placement ignores
//! units), foundation blocking that starts with construction
//! (the DE quickwall rule), multi-villager distribution along a
//! line, and the Stone Wall (Feudal Age, 5 stone per segment,
//! 1080/1800 HP, 8/10 armor, 10 s build — the DE row in
//! `~/workspace/skirmish-study/walls-de/DE-WALLS-DOSSIER.md`).
//!
//! Numbers under test are the current DE row (AoE wiki, Palisade
//! Wall infobox, fetched 2026-10-22): Dark Age, 3 wood per segment
//! (update 56005), 7 s build (update 42848), 150 HP Dark / 250 HP
//! Feudal (update 44725), 2 melee / 5 pierce armor.

use fixed::types::I16F16;
use skirmish_sim::{
    wall_line_cells, Age, BuildingKind, Command, Entity, EntityKind, MatchMode, NodeKind,
    ResourceKind, Sim, TechId, OWNER_OPPONENT, OWNER_PLAYER,
};

const WALL: BuildingKind = BuildingKind::PalisadeWall;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn villagers_of(sim: &Sim, owner: u8) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && e.kind == EntityKind::Villager)
        .map(|e| e.id)
        .collect()
}

fn find_id(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind)
        .unwrap()
        .id
}

/// A world point inside cell `(cx, cy)` (its center).
fn cell_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

/// The start of a horizontal run of `len` cells (row `cy`, columns
/// `cx..cx+len`) where every cell individually validates as a wall
/// placement for `entity`, scanning outward from the Town Center
/// of `owner` — the wall tests' open ground.
fn open_run_near_tc(sim: &Sim, owner: u8, entity: u32, len: u32) -> (u32, u32) {
    let tc = ent(sim, find_id(sim, owner, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    for r in 3..=40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let ok = (0..len).all(|i| {
                    let (x, y) = cell_point(sim, ax + i, ay);
                    sim.can_build(entity, WALL, x, y)
                });
                if ok {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no open wall run of {len} near the Town Center");
}

fn build_line(sim: &mut Sim, entity: u32, a: (u32, u32), b: (u32, u32)) {
    let (x0, y0) = cell_point(sim, a.0, a.1);
    let (x1, y1) = cell_point(sim, b.0, b.1);
    sim.push_command(Command::BuildLine {
        entity,
        kind: WALL,
        x0,
        y0,
        x1,
        y1,
    });
}

fn wall_buildings(sim: &Sim) -> Vec<&skirmish_sim::Building> {
    sim.buildings().iter().filter(|b| b.kind == WALL).collect()
}

#[test]
fn line_places_every_segment_and_charges_per_segment() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 6);
    let wood0 = sim.stockpile().amount(ResourceKind::Wood);
    build_line(&mut sim, vil, (ax, ay), (ax + 5, ay));
    sim.tick(); // the line is applied here
    let walls = wall_buildings(&sim);
    assert_eq!(walls.len(), 6, "one building record per segment");
    // Per-segment charge: 6 × 3 wood, nothing more.
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        wood0 - I16F16::from_num(18)
    );
    // Segments sit on the dragged cells, in drag order. An
    // UNTOUCHED foundation does not block its cell (BLD-20: the
    // DE quickwall rule — a segment blocks once construction
    // starts on it, not at placement; see
    // `foundation_blocks_only_once_construction_starts`).
    for (i, b) in walls.iter().enumerate() {
        assert_eq!((b.cell_x, b.cell_y), (ax + i as u32, ay));
        assert!(!b.complete);
        assert!(
            !sim.grid().is_blocked(b.cell_x, b.cell_y),
            "an untouched wall foundation must not block"
        );
    }
    // The villager is on construction duty for the first segment.
    assert_eq!(ent(&sim, vil).task_building, Some(walls[0].id));
}

#[test]
fn villager_walks_the_line_and_raises_every_segment() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 5);
    build_line(&mut sim, vil, (ax, ay), (ax + 4, ay));
    sim.tick();
    assert_eq!(wall_buildings(&sim).len(), 5);
    let done = run_until(&mut sim, 120_000, |s| {
        s.buildings().iter().all(|b| b.complete)
    });
    assert!(done, "the villager never finished the line");
    // A completed Dark-Age segment stands at the wiki's 150 HP.
    for b in wall_buildings(&sim) {
        assert_eq!(f(b.hp), 150.0);
    }
    // The villager is free again (no task, nothing queued).
    let v = ent(&sim, vil);
    assert!(v.task_building.is_none());
    assert!(v.order_queue.is_empty());
}

#[test]
fn diagonal_line_lays_an_edge_connected_staircase() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    // The supercover of a (0,0)->(3,3) drag: seven cells, each
    // exact corner filled on the canonical side. Find a start
    // where all seven validate.
    let want = wall_line_cells(0, 0, 3, 3);
    assert_eq!(want.len(), 7);
    let tc = ent(&sim, find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let mut start = None;
    'scan: for r in 3..=30i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 3 || ay < 3 || ax > 112 || ay > 112 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let ok = want.iter().all(|&(cx, cy)| {
                    let (x, y) = cell_point(&sim, ax + cx, ay + cy);
                    sim.can_build(vil, WALL, x, y)
                });
                if ok {
                    start = Some((ax, ay));
                    break 'scan;
                }
            }
        }
    }
    let (ax, ay) = start.expect("no open diagonal near the Town Center");
    build_line(&mut sim, vil, (ax, ay), (ax + 3, ay + 3));
    sim.tick();
    let cells: Vec<(u32, u32)> = wall_buildings(&sim)
        .iter()
        .map(|b| (b.cell_x, b.cell_y))
        .collect();
    // The committed buildings are exactly the shared expansion
    // (ghost == committed, end to end), in drag order.
    let want_abs: Vec<(u32, u32)> = want.iter().map(|&(cx, cy)| (ax + cx, ay + cy)).collect();
    assert_eq!(cells, want_abs);
    // And the chain has no corner-only joints: every consecutive
    // pair of segments is edge-adjacent — one continuous fence.
    for w in cells.windows(2) {
        let step = (w[1].0 as i64 - w[0].0 as i64).abs() + (w[1].1 as i64 - w[0].1 as i64).abs();
        assert_eq!(step, 1, "edge-adjacent step");
    }
}

#[test]
fn line_leaves_gaps_at_the_town_center_reservation() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let tc = ent(&sim, find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    // Drag straight through the TC along its row: the reserved
    // 4×4 (anchor = TC cell − 1 .. + 2) must stay empty (BLD-16),
    // the segments outside it must stand.
    build_line(&mut sim, vil, (tcx - 6, tcy), (tcx + 6, tcy));
    sim.tick();
    let walls = wall_buildings(&sim);
    assert!(!walls.is_empty(), "the line's outer segments stand");
    for b in &walls {
        let in_reserve = b.cell_x >= tcx - 1
            && b.cell_x <= tcx + 2
            && b.cell_y >= tcy - 1
            && b.cell_y <= tcy + 2;
        assert!(!in_reserve, "a segment entered the TC reservation");
    }
    assert!(
        walls.iter().any(|b| b.cell_x < tcx - 1),
        "segments west of the TC stand"
    );
    assert!(
        walls.iter().any(|b| b.cell_x > tcx + 2),
        "segments east of the TC stand"
    );
    // Charged only for what stands: segments × 3 wood.
    let spent = I16F16::from_num(200) - sim.stockpile().amount(ResourceKind::Wood);
    assert_eq!(spent, I16F16::from_num(3 * walls.len() as i32));
}

#[test]
fn line_truncates_where_the_wood_runs_out() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    // Drain the stockpile to exactly 10 wood with House
    // foundations (25 each from the standard 200): seven houses
    // leave 25; one more segment-priced step is impossible, so
    // instead burn 190 via 7 houses (175) + ... houses alone
    // leave multiples of 25 — use 6 houses (150, leaving 50) and
    // a 20-segment line: exactly 16 segments (48 wood) stand.
    let tc = ent(&sim, find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let mut placed = 0;
    'drain: for r in 2..=14i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if placed >= 6 {
                    break 'drain;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (x, y) = cell_point(&sim, ax as u32, ay as u32);
                if sim.can_build(vil, BuildingKind::House, x, y) {
                    // Count only REAL placements: an anchor inside
                    // the previous house's footprint is a build
                    // RESUME (SEL-11) — can_build says yes, but
                    // nothing is charged or founded. The scan's
                    // consecutive anchors hit that every other
                    // cell for a 2x2 kind, so watch the wood.
                    let before = sim.stockpile().amount(ResourceKind::Wood);
                    sim.push_command(Command::Build {
                        entity: vil,
                        kind: BuildingKind::House,
                        x,
                        y,
                    });
                    sim.tick();
                    if sim.stockpile().amount(ResourceKind::Wood) < before {
                        placed += 1;
                    }
                }
            }
        }
    }
    assert_eq!(placed, 6);
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        I16F16::from_num(50)
    );
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 20);
    build_line(&mut sim, vil, (ax, ay), (ax + 19, ay));
    sim.tick();
    assert_eq!(
        wall_buildings(&sim).len(),
        16,
        "50 wood buys exactly 16 segments at 3 wood"
    );
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        I16F16::from_num(2)
    );
}

#[test]
fn walls_do_not_count_toward_the_feudal_requirement() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 4);
    build_line(&mut sim, vil, (ax, ay), (ax + 3, ay));
    sim.tick();
    let done = run_until(&mut sim, 120_000, |s| {
        !s.buildings().is_empty() && s.buildings().iter().all(|b| b.complete)
    });
    assert!(done, "the wall line never completed");
    assert_eq!(sim.buildings().len(), 4);
    // Four completed buildings stand — all walls — and the
    // Feudal requirement count is still zero (TEC-01/BLD-13).
    assert_eq!(sim.feudal_building_count(OWNER_PLAYER), 0);
}

#[test]
fn foundation_delete_refunds_the_segment_cost() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 3);
    build_line(&mut sim, vil, (ax, ay), (ax + 2, ay));
    sim.tick();
    let ids: Vec<u32> = wall_buildings(&sim).iter().map(|b| b.id).collect();
    assert_eq!(ids.len(), 3);
    let wood_after_line = sim.stockpile().amount(ResourceKind::Wood);
    // Delete the middle foundation: full 3-wood refund (BLD-02),
    // the record leaves, and its cell frees up. (Checked on the
    // spot: the villager still holds a queued order for that
    // cell and would re-place it later — the queued-order
    // re-validation contract — so this test does not run on.)
    sim.push_command(Command::DeleteBuilding { building: ids[1] });
    sim.tick();
    assert_eq!(wall_buildings(&sim).len(), 2);
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        wood_after_line + I16F16::from_num(3)
    );
    assert!(!sim.grid().is_blocked(ax + 1, ay), "the cell is free again");
}

#[test]
fn completed_segment_delete_refunds_nothing() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 2);
    build_line(&mut sim, vil, (ax, ay), (ax + 1, ay));
    sim.tick();
    let ids: Vec<u32> = wall_buildings(&sim).iter().map(|b| b.id).collect();
    assert_eq!(ids.len(), 2);
    let done = run_until(&mut sim, 120_000, |s| {
        s.buildings().iter().all(|b| b.complete)
    });
    assert!(done, "the segments never completed");
    let wood_before = sim.stockpile().amount(ResourceKind::Wood);
    sim.push_command(Command::DeleteBuilding { building: ids[0] });
    sim.tick();
    assert_eq!(wall_buildings(&sim).len(), 1);
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        wood_before,
        "a completed building refunds nothing (BLD-02)"
    );
    assert!(!sim.grid().is_blocked(ax, ay), "the ground frees up");
}

#[test]
fn completed_wall_blocks_owner_and_enemy_pathing() {
    let mut sim = Sim::with_mode(31, MatchMode::TwoPlayer);
    // The OPPONENT raises a 5-segment line near its own TC.
    let opp_vil = villagers_of(&sim, OWNER_OPPONENT)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_OPPONENT, opp_vil, 5);
    build_line(&mut sim, opp_vil, (ax, ay), (ax + 4, ay));
    sim.tick();
    assert_eq!(wall_buildings(&sim).len(), 5);
    let done = run_until(&mut sim, 200_000, |s| {
        s.buildings().iter().all(|b| b.complete)
    });
    assert!(done, "the opponent's line never completed");
    let wall_cells: Vec<(u32, u32)> = wall_buildings(&sim)
        .iter()
        .map(|b| (b.cell_x, b.cell_y))
        .collect();

    // Grid level: a path from just north of the middle segment to
    // just south of it exists (around an end) but never steps on
    // a wall cell. Probe rows are scanned outward until both
    // stand on open ground.
    let mid = wall_cells[2];
    let mut probes = None;
    for d in 1..=3u32 {
        let (n, s) = ((mid.0, mid.1 - d), (mid.0, mid.1 + d));
        if !sim.grid().is_blocked(n.0, n.1) && !sim.grid().is_blocked(s.0, s.1) {
            probes = Some((n, s));
            break;
        }
    }
    let (nc, sc) = probes.expect("no open probe rows beside the line");
    let (nx, ny) = cell_point(&sim, nc.0, nc.1);
    let (sx, sy) = cell_point(&sim, sc.0, sc.1);
    let path = sim
        .grid()
        .find_path((nx, ny), (sx, sy))
        .expect("a detour around the line must exist");
    assert!(path.len() > 2, "the path detours, not straight through");
    for (px, py) in &path {
        let cell = sim.grid().world_to_cell(*px, *py);
        assert!(
            !wall_cells.contains(&cell),
            "path crosses wall cell {cell:?}"
        );
    }

    // Entity level: the PLAYER's Scout, ordered to a point on the
    // far side of the line, gets there by going around — and its
    // cell is never a wall cell on the way. (Owner pass-through
    // would fail this identically: the grid has no per-player
    // exception without a gate.)
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let se = ent(&sim, scout);
    let (_, scout_cy) = sim.grid().world_to_cell(se.pos_x, se.pos_y);
    // Target: the middle segment's column, three cells to the
    // side of the line opposite the Scout's approach.
    let target = if scout_cy < mid.1 {
        (mid.0, mid.1 + 3)
    } else {
        (mid.0, mid.1 - 3)
    };
    let (tx, ty) = cell_point(&sim, target.0, target.1);
    sim.push_command(Command::Move {
        entity: scout,
        x: tx,
        y: ty,
    });
    let start_side = scout_cy < mid.1;
    let mut arrived = false;
    for _ in 0..600_000 {
        sim.tick();
        let s = ent(&sim, scout);
        let cell = sim.grid().world_to_cell(s.pos_x, s.pos_y);
        assert!(
            !wall_cells.contains(&cell),
            "the Scout entered a wall cell"
        );
        if s.order.is_none() {
            // Arrived: the order completed on the FAR side of the
            // wall row (within a cell of the target — the exact
            // cell can shift if the target point hugs an obstacle).
            let far_side = (cell.1 < mid.1) != start_side;
            let near = (cell.0 as i32 - target.0 as i32).abs() <= 1
                && (cell.1 as i32 - target.1 as i32).abs() <= 1;
            arrived = far_side && near;
            break;
        }
    }
    assert!(arrived, "the Scout never reached the far side");
}

#[test]
fn damaged_wall_repairs_at_the_standard_rate() {
    let mut sim = Sim::with_mode(81, MatchMode::TwoPlayer);
    // Opponent builds one segment; the player's Scout damages
    // it; the opponent's villager repairs it (BLD-17 is generic
    // over building kinds — this pins that walls are included).
    let opp_vil = villagers_of(&sim, OWNER_OPPONENT)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_OPPONENT, opp_vil, 1);
    let (x, y) = cell_point(&sim, ax, ay);
    sim.push_command(Command::Build {
        entity: opp_vil,
        kind: WALL,
        x,
        y,
    });
    sim.tick();
    assert_eq!(wall_buildings(&sim).len(), 1);
    let bid = wall_buildings(&sim)[0].id;
    let done = run_until(&mut sim, 60_000, |s| {
        s.buildings().iter().any(|b| b.id == bid && b.complete)
    });
    assert!(done, "the segment never completed");
    // The builder steps away from the ring before the beating
    // (repair.rs's deadlock note: an idle villager on the chase's
    // approach cell jams a melee attacker short of strike range).
    let otc = ent(&sim, find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter));
    let (otx, oty) = (otc.pos_x, otc.pos_y);
    sim.push_command(Command::Move {
        entity: opp_vil,
        x: otx,
        y: oty,
    });

    // Beat it down with the player's Scout, then withdraw. If the
    // first approach stalls (the melee_building chase edge), walk
    // the Scout to the segment's west ring cell and re-issue.
    let scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    sim.push_command(Command::AttackBuilding {
        entity: scout,
        building: bid,
    });
    let hp_below = |s: &Sim| {
        s.buildings()
            .iter()
            .find(|b| b.id == bid)
            .map(|b| f(b.hp) < 120.0)
            .unwrap_or(false)
    };
    let mut beaten = run_until(&mut sim, 120_000, hp_below);
    if !beaten {
        let (wx, wy) = cell_point(&sim, ax - 1, ay);
        sim.push_command(Command::Move {
            entity: scout,
            x: wx,
            y: wy,
        });
        run_until(&mut sim, 30_000, |s| ent(s, scout).order.is_none());
        sim.push_command(Command::AttackBuilding {
            entity: scout,
            building: bid,
        });
        beaten = run_until(&mut sim, 120_000, hp_below);
    }
    assert!(beaten, "the Scout never damaged the wall");
    let tc = ent(&sim, find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter));
    let (hx, hy) = (tc.pos_x, tc.pos_y);
    sim.push_command(Command::Move {
        entity: scout,
        x: hx,
        y: hy,
    });
    run_until(&mut sim, 60_000, |s| ent(s, scout).order.is_none());
    let hp_damaged = f(sim
        .buildings()
        .iter()
        .find(|b| b.id == bid)
        .unwrap()
        .hp);
    assert!(hp_damaged < 150.0, "the wall kept its damage");

    // Repair: HP climbs back to the full 150 and the repairer is
    // released at completion.
    sim.push_command(Command::Repair {
        entity: opp_vil,
        building: bid,
    });
    let healed = run_until(&mut sim, 120_000, |s| {
        s.buildings()
            .iter()
            .find(|b| b.id == bid)
            .map(|b| f(b.hp) >= 150.0)
            .unwrap_or(false)
    });
    assert!(healed, "the wall never repaired to full");
}

#[test]
fn single_segment_build_command_places_one_wall() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 1);
    let (x, y) = cell_point(&sim, ax, ay);
    assert!(sim.can_build(vil, WALL, x, y));
    sim.push_command(Command::Build {
        entity: vil,
        kind: WALL,
        x,
        y,
    });
    sim.tick();
    assert_eq!(wall_buildings(&sim).len(), 1);
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        I16F16::from_num(197)
    );
}

#[test]
fn build_line_rejects_non_wall_kinds_and_dead_lines() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 4);
    // A BuildLine naming a non-wall kind is rejected whole.
    let (x0, y0) = cell_point(&sim, ax, ay);
    let (x1, y1) = cell_point(&sim, ax + 3, ay);
    sim.push_command(Command::BuildLine {
        entity: vil,
        kind: BuildingKind::House,
        x0,
        y0,
        x1,
        y1,
    });
    sim.tick();
    assert!(sim.buildings().is_empty(), "no house line was placed");
    assert!(!sim.can_build_line(vil, BuildingKind::House, x0, y0, x1, y1));
    // A line dragged entirely over the TC reservation takes no
    // effect either (every segment rejects).
    let tc = ent(&sim, find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let (rx0, ry0) = cell_point(&sim, tcx - 1, tcy);
    let (rx1, ry1) = cell_point(&sim, tcx + 2, tcy);
    assert!(!sim.can_build_line(vil, WALL, rx0, ry0, rx1, ry1));
    sim.push_command(Command::BuildLine {
        entity: vil,
        kind: WALL,
        x0: rx0,
        y0: ry0,
        x1: rx1,
        y1: ry1,
    });
    sim.tick();
    assert!(sim.buildings().is_empty(), "no segment inside the reservation");
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        I16F16::from_num(200),
        "rejected lines charge nothing"
    );
}

#[test]
fn wall_line_is_deterministic() {
    let run = |seed: u64| {
        let mut sim = Sim::new(seed);
        let vil = villagers_of(&sim, OWNER_PLAYER)[0];
        let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 8);
        build_line(&mut sim, vil, (ax, ay), (ax + 7, ay));
        let mut hash = 0;
        for _ in 0..6_000 {
            hash = sim.tick();
        }
        (hash, sim.buildings().len())
    };
    let a = run(31);
    let b = run(31);
    assert_eq!(a, b, "same seed + same line = same state");
    assert_eq!(a.1, 8);
}

// ---------------------------------------------------------------------
// BLD-20: the DE wall pass — ghost == founded, units never delete
// segments, foundations block when construction starts, villagers
// distribute, and the Stone Wall.
// ---------------------------------------------------------------------

const STONE_WALL: BuildingKind = BuildingKind::StoneWall;

fn build_line_kind(
    sim: &mut Sim,
    entity: u32,
    kind: BuildingKind,
    a: (u32, u32),
    b: (u32, u32),
) {
    let (x0, y0) = cell_point(sim, a.0, a.1);
    let (x1, y1) = cell_point(sim, b.0, b.1);
    sim.push_command(Command::BuildLine { entity, kind, x0, y0, x1, y1 });
}

fn founded_cells(sim: &Sim, kind: BuildingKind) -> Vec<(u32, u32)> {
    let mut cells: Vec<(u32, u32)> = sim
        .buildings()
        .iter()
        .filter(|b| b.kind == kind)
        .map(|b| (b.cell_x, b.cell_y))
        .collect();
    cells.sort();
    cells
}

fn cell_of_ent(sim: &Sim, id: u32) -> (u32, u32) {
    let e = ent(sim, id);
    sim.grid().world_to_cell(e.pos_x, e.pos_y)
}

/// An anchor where the drag to `anchor + shape` founds completely
/// for `entity` (every supercover cell individually validates),
/// scanning outward from the player's Town Center.
fn open_anchor_for_shape(sim: &Sim, entity: u32, shape: (i32, i32)) -> (u32, u32) {
    let tc = ent(sim, find_id(sim, OWNER_PLAYER, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    for r in 4..=45i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 2 || ay < 2 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let b = ((ax as i32 + shape.0) as u32, (ay as i32 + shape.1) as u32);
                let ok = wall_line_cells(ax, ay, b.0, b.1).iter().all(|&(cx, cy)| {
                    let (x, y) = cell_point(sim, cx, cy);
                    sim.can_build(entity, WALL, x, y)
                });
                if ok {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no open anchor for drag shape {shape:?}");
}

/// BLD-20 regression for Claudio's report ("o de madeira não
/// segue a linha"): for every drag shape, the founded segments
/// are EXACTLY the supercover cells the drag ghost previewed —
/// no gaps, no extras, including 45°, shallow and steep diagonals
/// and a long drag.
#[test]
fn ghost_cells_equal_founded_cells_for_all_drag_shapes() {
    let shapes: [(i32, i32); 6] = [(7, 0), (0, 7), (6, 6), (8, 2), (2, 8), (20, 3)];
    for &(sx, sy) in &shapes {
        let mut sim = Sim::new(31);
        let vil = villagers_of(&sim, OWNER_PLAYER)[0];
        let (ax, ay) = open_anchor_for_shape(&sim, vil, (sx, sy));
        let b = ((ax as i32 + sx) as u32, (ay as i32 + sy) as u32);
        let mut want = wall_line_cells(ax, ay, b.0, b.1);
        want.sort();
        build_line(&mut sim, vil, (ax, ay), b);
        sim.tick(); // the line is applied here
        assert_eq!(
            founded_cells(&sim, WALL),
            want,
            "drag shape ({sx},{sy}): founded segments != ghost cells"
        );
        // Consecutive founded segments are edge-adjacent (the
        // supercover guarantee: no gap a unit can slip through,
        // diagonals included).
        let line = wall_line_cells(ax, ay, b.0, b.1);
        for w in line.windows(2) {
            let d = (w[1].0 as i32 - w[0].0 as i32).abs() + (w[1].1 as i32 - w[0].1 as i32).abs();
            assert_eq!(d, 1, "drag shape ({sx},{sy}): non-adjacent step {w:?}");
        }
    }
}

/// BLD-20 (dossier §3): DE placement never checks units — a line
/// dragged through a cell another villager is standing on founds
/// that segment too. Before this pass the sim silently dropped
/// every segment under a living unit (the ghost showed green), so
/// walls beside the TC — where villagers and sheep mill about —
/// came up full of holes.
#[test]
fn line_founds_segments_under_standing_units() {
    let mut sim = Sim::new(31);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    let (gx, gy) = open_run_near_tc(&sim, OWNER_PLAYER, vils[0], 6);
    // Park the second villager on open ground the line can cross.
    let park = (gx + 2, gy);
    let (px, py) = cell_point(&sim, park.0, park.1);
    sim.push_command(Command::Move { entity: vils[1], x: px, y: py });
    let parked = run_until(&mut sim, 30_000, |s| cell_of_ent(s, vils[1]) == park);
    assert!(parked, "the second villager never reached the parking cell");
    let his = cell_of_ent(&sim, vils[1]);
    // A 6-run through HIS cell that fully validates: his own cell
    // validates only because walls are exempt from the unit check
    // (pre-BLD-20 this scan found no line through him at all).
    let mut start = None;
    for s0 in his.0.saturating_sub(5)..=his.0 {
        let ok = (0..6).all(|i| {
            let (x, y) = cell_point(&sim, s0 + i, his.1);
            sim.can_build(vils[0], WALL, x, y)
        });
        if ok {
            start = Some(s0);
            break;
        }
    }
    let lx = start.expect("no valid wall line through the standing villager");
    build_line(&mut sim, vils[0], (lx, his.1), (lx + 5, his.1));
    sim.tick();
    let cells = founded_cells(&sim, WALL);
    assert_eq!(cells.len(), 6, "the segment under the standing villager was dropped");
    assert!(cells.contains(&his), "no foundation on the occupied cell {his:?}");
}

/// BLD-20 (dossier §3): an untouched wall foundation is walkable
/// ground; its cell joins the blocking grid when construction
/// first advances on it (the DE quickwall rule), and a completed
/// segment always blocks.
#[test]
fn foundation_blocks_only_once_construction_starts() {
    let mut sim = Sim::new(31);
    let vil = villagers_of(&sim, OWNER_PLAYER)[0];
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vil, 4);
    build_line(&mut sim, vil, (ax, ay), (ax + 3, ay));
    sim.tick();
    let cells = founded_cells(&sim, WALL);
    assert_eq!(cells.len(), 4);
    for &(cx, cy) in &cells {
        assert!(
            !sim.grid().is_blocked(cx, cy),
            "untouched foundation blocks at ({cx},{cy})"
        );
    }
    // The builder reaches segment one and starts it: its cell —
    // and only its cell — joins the blocking grid. (Later
    // segments cannot be started before segment one completes,
    // hundreds of ticks later, so this observation is exact.)
    let first = cells[0];
    let started = run_until(&mut sim, 60_000, |s| s.grid().is_blocked(first.0, first.1));
    assert!(started, "the started segment never joined the blocking grid");
    for &(cx, cy) in &cells[1..] {
        assert!(
            !sim.grid().is_blocked(cx, cy),
            "unstarted segment ({cx},{cy}) blocked too early"
        );
    }
    let done = run_until(&mut sim, 120_000, |s| {
        s.buildings().iter().all(|b| b.complete)
    });
    assert!(done, "the line never completed");
    for &(cx, cy) in &cells {
        assert!(
            sim.grid().is_blocked(cx, cy),
            "completed segment ({cx},{cy}) does not block"
        );
    }
}

/// BLD-20 (dossier §3): DE tasks every selected villager to the
/// line; the client issues the line per villager, and the sim's
/// distribution rule gives each builder the first segment nobody
/// is building yet — two villagers on one line start on segment
/// one and segment two, never both on segment one.
#[test]
fn two_villagers_distribute_along_the_line() {
    let mut sim = Sim::new(31);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vils[0], 5);
    build_line(&mut sim, vils[0], (ax, ay), (ax + 4, ay));
    build_line(&mut sim, vils[1], (ax, ay), (ax + 4, ay));
    sim.tick(); // both line commands apply here
    let walls = wall_buildings(&sim);
    assert_eq!(walls.len(), 5, "the second line must resume the same five foundations");
    assert_eq!(ent(&sim, vils[0]).task_building, Some(walls[0].id));
    assert_eq!(
        ent(&sim, vils[1]).task_building,
        Some(walls[1].id),
        "second villager piled onto segment one instead of distributing"
    );
    // The whole line still completes (and faster than one builder
    // would manage alone is the multi-builder rate's business,
    // covered by the construction tests).
    let done = run_until(&mut sim, 120_000, |s| {
        s.buildings().iter().all(|b| b.complete)
    });
    assert!(done, "the two-villager line never completed");
}

/// The Stone Wall's kind-level row (dossier §2) — pure values, no
/// sim: tag 14, 5 stone, 10 s, 1080 Feudal / 1800 Castle HP,
/// 8/10 armor, Feudal-gated, wall LoS 2 (the palisade's too).
#[test]
fn stone_wall_values_match_the_de_row() {
    let k = STONE_WALL;
    assert_eq!(k.tag(), 14);
    assert_eq!(BuildingKind::from_tag(14), Some(k));
    assert!(k.is_wall());
    assert_eq!(f(k.wood_cost()), 0.0);
    assert_eq!(f(k.stone_cost()), 5.0);
    assert_eq!(k.build_ticks(), 600);
    assert_eq!(k.max_hp_at(Age::Feudal), 1080);
    assert_eq!(k.max_hp_at(Age::Castle), 1800);
    assert_eq!(k.armor_melee_at(Age::Feudal), 8);
    assert_eq!(k.armor_pierce_at(Age::Feudal), 10);
    assert_eq!(k.age_required(), Age::Feudal);
    assert_eq!(k.sight_tiles_at(Age::Feudal), 2);
    assert_eq!(BuildingKind::PalisadeWall.sight_tiles_at(Age::Dark), 2);
}

/// `open_run_near_tc` for an arbitrary kind (the stone tests run
/// on a drained wood stockpile — the Mill and the Lumber Camp ate
/// it — so the run must validate against the STONE row).
fn open_run_for(sim: &Sim, entity: u32, kind: BuildingKind, len: u32) -> (u32, u32) {
    let tc = ent(sim, find_id(sim, OWNER_PLAYER, EntityKind::TownCenter));
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    for r in 3..=40i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                // A cell holding ANY standing building is out —
                // including our own incomplete wall foundations,
                // which `can_build` would otherwise read as a
                // Resume and hand back the same run forever.
                let occupied = |cx: u32, cy: u32| {
                    sim.buildings().iter().any(|b| {
                        let bs = b.kind.footprint_size();
                        cx >= b.cell_x && cx < b.cell_x + bs && cy >= b.cell_y && cy < b.cell_y + bs
                    })
                };
                let ok = (0..len).all(|i| {
                    let (x, y) = cell_point(sim, ax + i, ay);
                    !occupied(ax + i, ay) && sim.can_build(entity, kind, x, y)
                });
                if ok {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no open run of {len} for {kind:?} near the Town Center");
}

fn first_node_walls(sim: &Sim, kind: NodeKind) -> u32 {
    sim.nodes()
        .iter()
        .find(|n| n.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} node"))
        .id
}

fn anchor_near_walls(
    sim: &Sim,
    entity: u32,
    kind: BuildingKind,
    center: (u32, u32),
) -> (u32, u32) {
    for r in 1..=14i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (center.0 as i32 + dx, center.1 as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (ax, ay) = (ax as u32, ay as u32);
                let (x, y) = cell_point(sim, ax, ay);
                if sim.can_build(entity, kind, x, y) {
                    return (ax, ay);
                }
            }
        }
    }
    panic!("no buildable anchor near {center:?} for {kind:?}");
}

fn raise_walls(sim: &mut Sim, entity: u32, kind: BuildingKind, cell: (u32, u32)) {
    let (x, y) = cell_point(sim, cell.0, cell.1);
    sim.push_command(Command::Build { entity, kind, x, y });
    let done = run_until(sim, 120_000, |s| {
        s.buildings()
            .iter()
            .find(|b| b.kind == kind && b.owner == OWNER_PLAYER)
            .map(|b| b.complete)
            .unwrap_or(false)
    });
    assert!(done, "{kind:?} never completed");
}

/// The towers/feudal staging in walls idioms: Mill + Lumber Camp
/// by the Town Center, berries to 500 food, research, wait.
fn reach_feudal_walls(sim: &mut Sim) {
    let vils = villagers_of(sim, OWNER_PLAYER);
    let tc = ent(sim, find_id(sim, OWNER_PLAYER, EntityKind::TownCenter));
    let tc_cell = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let mill = anchor_near_walls(sim, vils[1], BuildingKind::Mill, tc_cell);
    raise_walls(sim, vils[1], BuildingKind::Mill, mill);
    let lumber = anchor_near_walls(sim, vils[2], BuildingKind::LumberCamp, tc_cell);
    raise_walls(sim, vils[2], BuildingKind::LumberCamp, lumber);
    let berry = first_node_walls(sim, NodeKind::BerryBush);
    for v in [vils[0], vils[1]] {
        sim.push_command(Command::Gather { entity: v, node: berry });
    }
    let food = run_until(sim, 180_000, |s| {
        f(s.stockpile().amount(ResourceKind::Food)) >= 500.0
    });
    assert!(food, "never gathered the Feudal food");
    sim.push_command(Command::ResearchTc { tech: TechId::FeudalAge });
    let up = run_until(sim, 60_000, |s| s.age_of(OWNER_PLAYER) == Age::Feudal);
    assert!(up, "never reached Feudal");
}

/// BLD-20: the Stone Wall line is refused in the Dark Age (the
/// Feudal gate), then founds in Feudal at exactly 5 stone per
/// segment and completes at the Feudal HP row of 1080.
#[test]
fn stone_wall_line_gated_by_age_then_charges_stone_per_segment() {
    let mut sim = Sim::new(31);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    let (ax, ay) = open_run_near_tc(&sim, OWNER_PLAYER, vils[0], 4);
    let (x0, y0) = cell_point(&sim, ax, ay);
    let (x1, y1) = cell_point(&sim, ax + 3, ay);
    assert!(
        !sim.can_build_line(vils[0], STONE_WALL, x0, y0, x1, y1),
        "a Dark Age stone line reports placeable"
    );
    build_line_kind(&mut sim, vils[0], STONE_WALL, (ax, ay), (ax + 3, ay));
    sim.tick();
    assert_eq!(
        founded_cells(&sim, STONE_WALL).len(),
        0,
        "a Dark Age stone line founded segments"
    );

    reach_feudal_walls(&mut sim);
    // A fresh run (the Mill and Lumber Camp stand by the TC now,
    // and the wood stockpile is drained — the run validates
    // against the stone row).
    let (bx, by) = open_run_for(&sim, vils[0], STONE_WALL, 4);
    let (bx0, by0) = cell_point(&sim, bx, by);
    let (bx1, by1) = cell_point(&sim, bx + 3, by);
    assert!(sim.can_build_line(vils[0], STONE_WALL, bx0, by0, bx1, by1));
    let stone0 = sim.stockpile().amount(ResourceKind::Stone);
    build_line_kind(&mut sim, vils[0], STONE_WALL, (bx, by), (bx + 3, by));
    sim.tick();
    let cells = founded_cells(&sim, STONE_WALL);
    assert_eq!(cells.len(), 4, "the Feudal stone line did not found every segment");
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Stone),
        stone0 - I16F16::from_num(20),
        "stone charge is not 5 per segment"
    );
    let done = run_until(&mut sim, 120_000, |s| {
        s.buildings()
            .iter()
            .filter(|b| b.kind == STONE_WALL)
            .all(|b| b.complete)
    });
    assert!(done, "the stone line never completed");
    for b in sim.buildings().iter().filter(|b| b.kind == STONE_WALL) {
        assert_eq!(f(b.hp), 1080.0, "a completed Feudal stone segment is not 1080 HP");
        assert!(sim.grid().is_blocked(b.cell_x, b.cell_y));
    }
}

/// BLD-20: past the stone stockpile the line truncates in drag
/// order — segments the stone cannot pay for are skipped, never
/// partially charged (the palisade wood rule, on stone).
#[test]
fn stone_wall_line_truncates_where_the_stone_runs_out() {
    let mut sim = Sim::new(31);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    reach_feudal_walls(&mut sim);
    // Burn the 200 starting stone down to exactly 20 with three
    // 12-segment lines (36 × 5 = 180), farmed across open runs.
    for _ in 0..3 {
        let (ax, ay) = open_run_for(&sim, vils[0], STONE_WALL, 12);
        build_line_kind(&mut sim, vils[0], STONE_WALL, (ax, ay), (ax + 11, ay));
        sim.tick();
    }
    assert_eq!(f(sim.stockpile().amount(ResourceKind::Stone)), 20.0);
    assert_eq!(founded_cells(&sim, STONE_WALL).len(), 36);
    // A 5-segment line on 20 stone founds exactly its first four.
    let (ax, ay) = open_run_for(&sim, vils[0], STONE_WALL, 5);
    build_line_kind(&mut sim, vils[0], STONE_WALL, (ax, ay), (ax + 4, ay));
    sim.tick();
    let cells = founded_cells(&sim, STONE_WALL);
    assert_eq!(cells.len(), 40, "the line did not truncate at the stockpile");
    for i in 0..4u32 {
        assert!(cells.contains(&(ax + i, ay)), "segment {i} of the paid prefix missing");
    }
    assert!(!cells.contains(&(ax + 4, ay)), "the unaffordable segment was founded");
    assert_eq!(f(sim.stockpile().amount(ResourceKind::Stone)), 0.0);
}
