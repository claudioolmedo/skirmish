//! Town Center footprint rule (BLD-16, 2026-10-19): the Town
//! Center's 4x4 is PASSABLE to units but RESERVED against building
//! placement.
//!
//! Claudio's rule, correcting an earlier misreading that would
//! have made the TC block: in the reference game units walk
//! through the Town Center's footprint on all sides (under its
//! canopy roofs) — passability is a documented TC special
//! property — while its tiles stay reserved for construction, so
//! no building may be placed on them. These tests pin both halves:
//!
//! (a) a unit ordered across the TC walks straight through its
//!     4x4 area (the TC never enters the blocking grid);
//! (b) a villager deposits at the TC exactly as before (the
//!     center-distance deposit rule is unchanged);
//! (c) build placement overlapping a living TC's 4x4 cells is
//!     rejected — the TC's own center cell, its footprint edges,
//!     and partial overlaps, both players' TCs — while placement
//!     fully outside the footprint is accepted.

use fixed::types::I16F16;
use skirmish_sim::{
    BuildingKind, Command, EntityKind, MatchMode, NodeKind, ResourceKind, Sim, CARRY_CAPACITY,
    OWNER_OPPONENT,
};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn by_id(sim: &Sim, id: u32) -> &skirmish_sim::Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

/// The reserved 4x4 rect of a TC standing at cell `(tcx, tcy)`:
/// cells `tcx-1..=tcx+2` x `tcy-1..=tcy+2`.
fn in_reserved(cx: i32, cy: i32, tcx: i32, tcy: i32) -> bool {
    (tcx - 1..=tcx + 2).contains(&cx) && (tcy - 1..=tcy + 2).contains(&cy)
}

/// Does a House footprint anchored at `(ax, ay)` touch the
/// reserved rect of the TC at `(tcx, tcy)`?
fn house_touches_tc(ax: i32, ay: i32, tcx: i32, tcy: i32) -> bool {
    (0..2).any(|dy| (0..2).any(|dx| in_reserved(ax + dx, ay + dy, tcx, tcy)))
}

#[test]
fn a_unit_ordered_across_the_tc_walks_straight_through() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center();
    let (tcx, tcy) = (f(tc.pos_x), f(tc.pos_y));

    // Pick a starting villager whose mirrored target — reflected
    // through the TC center and doubled, so the straight line runs
    // across the whole footprint — lands on open ground.
    let mut chosen = None;
    for id in 1..=3u32 {
        let v = by_id(&sim, id);
        let (vx, vy) = (f(v.pos_x), f(v.pos_y));
        let (tx, ty) = (tcx + (tcx - vx) * 2.0, tcy + (tcy - vy) * 2.0);
        let cell = sim
            .grid()
            .world_to_cell(I16F16::from_num(tx), I16F16::from_num(ty));
        if !sim.grid().is_blocked(cell.0, cell.1) {
            chosen = Some((id, tx, ty));
            break;
        }
    }
    let (id, tx, ty) = chosen.expect("no villager has an open crossing target");

    sim.push_command(Command::Move {
        entity: id,
        x: I16F16::from_num(tx),
        y: I16F16::from_num(ty),
    });

    let mut min_tc_dist = f64::MAX;
    let mut arrived = false;
    for _ in 0..6_000 {
        sim.tick();
        let v = by_id(&sim, id);
        let d = ((f(v.pos_x) - tcx).powi(2) + (f(v.pos_y) - tcy).powi(2)).sqrt();
        min_tc_dist = min_tc_dist.min(d);
        if v.order.is_none() {
            arrived = true;
            break;
        }
    }
    assert!(arrived, "villager {id} never completed the crossing");
    let v = by_id(&sim, id);
    let end_d = ((f(v.pos_x) - tx).powi(2) + (f(v.pos_y) - ty).powi(2)).sqrt();
    assert!(end_d < 1.5, "villager stopped {end_d:.2} from the far side");
    // The 4x4 footprint's half-extent is 8 world units: crossing
    // "through" means the path entered it, not skirted a corner.
    assert!(
        min_tc_dist < 6.0,
        "path kept {min_tc_dist:.2} wu from the TC center — it detoured around the footprint instead of passing through"
    );
}

#[test]
fn a_villager_deposits_at_the_tc_as_before() {
    let mut sim = Sim::new(7);
    let berry = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::BerryBush)
        .unwrap()
        .id;
    sim.push_command(Command::Gather {
        entity: 1,
        node: berry,
    });

    // First deposit must be exactly one full carry (10 food) —
    // the center-distance deposit rule, unchanged by BLD-16.
    let start_food = sim.stockpile().amount(ResourceKind::Food);
    let mut deposited = false;
    for _ in 0..20_000 {
        sim.tick();
        let food = sim.stockpile().amount(ResourceKind::Food);
        if food > start_food {
            assert_eq!(
                food,
                start_food + CARRY_CAPACITY,
                "first TC deposit was not exactly one full carry"
            );
            deposited = true;
            break;
        }
    }
    assert!(deposited, "villager never deposited at the Town Center");
}

#[test]
fn placement_overlapping_the_tc_footprint_is_rejected_adjacent_accepted() {
    let mut sim = Sim::new(42);
    let tc = sim.town_center();
    let (tcx, tcy) = sim.grid().world_to_cell(tc.pos_x, tc.pos_y);
    let (tcx, tcy) = (tcx as i32, tcy as i32);
    let start_wood = sim.stockpile().amount(ResourceKind::Wood);

    // Every House anchor in the surrounding box whose footprint
    // touches the reserved 4x4 must be refused.
    let mut probed = 0;
    for ay in tcy - 3..=tcy + 3 {
        for ax in tcx - 3..=tcx + 3 {
            if !house_touches_tc(ax, ay, tcx, tcy) {
                continue;
            }
            probed += 1;
            let (x, y) = sim.grid().cell_center(ax as u32, ay as u32);
            assert!(
                !sim.can_build(1, BuildingKind::House, x, y),
                "House anchor ({ax},{ay}) overlaps the TC's reserved 4x4 but was accepted"
            );
        }
    }
    assert!(probed > 0, "the probe box found no overlapping anchors");

    // The command path agrees — at the TC's center cell and at a
    // footprint-edge cell (anchor (tcx+2, tcy+2) covers exactly one
    // reserved cell) — and a refusal deducts nothing.
    for (ax, ay) in [(tcx, tcy), (tcx + 2, tcy + 2)] {
        let (x, y) = sim.grid().cell_center(ax as u32, ay as u32);
        sim.push_command(Command::Build {
            entity: 1,
            kind: BuildingKind::House,
            x,
            y,
        });
        sim.tick();
        assert_eq!(
            sim.buildings().len(),
            0,
            "build at ({ax},{ay}) overlapping the TC footprint was placed"
        );
    }
    assert_eq!(
        sim.stockpile().amount(ResourceKind::Wood),
        start_wood,
        "rejected TC-overlap placements must not deduct"
    );

    // Adjacent placement — footprint fully outside the 4x4, in the
    // same surrounding box — is accepted somewhere around the TC.
    let mut adjacent_ok = false;
    for ay in tcy - 3..=tcy + 3 {
        for ax in tcx - 3..=tcx + 3 {
            if house_touches_tc(ax, ay, tcx, tcy) {
                continue;
            }
            let (x, y) = sim.grid().cell_center(ax as u32, ay as u32);
            if sim.can_build(1, BuildingKind::House, x, y) {
                adjacent_ok = true;
            }
        }
    }
    assert!(
        adjacent_ok,
        "no buildable ground adjacent to the TC — the reservation reaches too far"
    );
}

#[test]
fn the_opponents_tc_footprint_is_reserved_too() {
    let sim = Sim::with_mode(42, MatchMode::TwoPlayer);
    let otc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == OWNER_OPPONENT)
        .expect("two-player match has an opponent Town Center");
    let (tcx, tcy) = sim.grid().world_to_cell(otc.pos_x, otc.pos_y);
    let (tcx, tcy) = (tcx as i32, tcy as i32);

    let mut probed = 0;
    for ay in tcy - 3..=tcy + 3 {
        for ax in tcx - 3..=tcx + 3 {
            if !house_touches_tc(ax, ay, tcx, tcy) {
                continue;
            }
            probed += 1;
            let (x, y) = sim.grid().cell_center(ax as u32, ay as u32);
            assert!(
                !sim.can_build(1, BuildingKind::House, x, y),
                "House anchor ({ax},{ay}) overlaps the opponent TC's reserved 4x4 but was accepted"
            );
        }
    }
    assert!(probed > 0, "the probe box found no overlapping anchors");
}
