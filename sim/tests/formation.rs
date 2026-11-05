//! MOV-07 (public API): formations assign slots by class
//! layer — proven through the ordinary command queue on the
//! standard opening package: the starting Scout Cavalry
//! (melee layer) ordered in Line formation together with
//! Villagers (civilian layer) receives a slot AHEAD of the
//! villagers' slots along the facing, instead of every unit
//! converging on the clicked point.

use fixed::types::I16F16;
use skirmish_sim::{
    assign_column_slots, column_anchor_points, Command, Entity, EntityKind, FormationKind,
    FormationMember, Sim,
};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn unit_of(sim: &Sim, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == kind && e.owner == 0 && !e.dead)
        .map(|e| e.id)
        .unwrap_or_else(|| panic!("no living {kind:?}"))
}

fn villagers(sim: &Sim) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager && e.owner == 0 && !e.dead)
        .map(|e| e.id)
        .collect()
}

/// A destination well east of the opening group with
/// headroom beyond it: the scan looks at rows near the
/// Scout's for a run of 22+ unblocked cells and names a
/// point 16 cells into the run — so the formation's front
/// slots (up to a full spacing past the anchor) still land
/// on open ground and no member falls back to the anchor.
fn east_dest(sim: &Sim) -> (I16F16, I16F16) {
    let scout = ent(sim, unit_of(sim, EntityKind::ScoutCavalry));
    let g = sim.grid();
    let (cx, cy) = g.world_to_cell(scout.pos_x, scout.pos_y);
    for row in (cy.saturating_sub(6))..=(cy + 6).min(119) {
        let mut run_start: Option<u32> = None;
        for x in cx.saturating_sub(4)..120u32 {
            if g.is_blocked(x, row) {
                run_start = None;
                continue;
            }
            let start = *run_start.get_or_insert(x);
            if x + 1 - start >= 22 {
                return g.cell_center(start + 16, row);
            }
        }
    }
    panic!("no 22-cell open run near the scout");
}

#[test]
fn opening_group_forms_up_with_the_scout_ahead() {
    let mut sim = Sim::new(7);
    let scout = unit_of(&sim, EntityKind::ScoutCavalry);
    let vils = villagers(&sim);
    assert!(vils.len() >= 2, "the opening needs 2+ villagers");
    let group: Vec<u32> = std::iter::once(scout).chain(vils.iter().copied()).collect();
    // The facing: from the group's centroid toward the dest.
    let (mut cx, mut cy) = (0.0f64, 0.0f64);
    for &id in &group {
        let e = ent(&sim, id);
        cx += f(e.pos_x);
        cy += f(e.pos_y);
    }
    cx /= group.len() as f64;
    cy /= group.len() as f64;
    let (dx, dy) = east_dest(&sim);
    let (fx, fy) = (f(dx) - cx, f(dy) - cy);
    let len = (fx * fx + fy * fy).sqrt();
    let (fx, fy) = (fx / len, fy / len);
    for &id in &group {
        sim.push_command(Command::FormationMove {
            entity: id,
            x: dx,
            y: dy,
            formation: FormationKind::Line,
        });
    }
    sim.tick();
    let depth = |id: u32| {
        let o = ent(&sim, id).order.as_ref().expect("formation order missing");
        (f(o.target_x) - cx) * fx + (f(o.target_y) - cy) * fy
    };
    let scout_depth = depth(scout);
    for &v in &vils {
        assert!(
            scout_depth > depth(v),
            "the scout's slot ({scout_depth}) is not ahead of villager {v}'s ({})",
            depth(v)
        );
    }
    // And the villagers did NOT all converge on the click:
    // their slot targets differ from the destination itself.
    assert!(
        vils.iter().any(|&v| {
            let o = ent(&sim, v).order.as_ref().unwrap();
            (o.target_x, o.target_y) != (dx, dy)
        }),
        "no slot was assigned — every villager still targets the click point"
    );
}

#[test]
fn lone_formation_move_targets_the_destination() {
    let mut sim = Sim::new(7);
    let scout = unit_of(&sim, EntityKind::ScoutCavalry);
    let (dx, dy) = east_dest(&sim);
    sim.push_command(Command::FormationMove {
        entity: scout,
        x: dx,
        y: dy,
        formation: FormationKind::Flank,
    });
    sim.tick();
    let o = ent(&sim, scout).order.as_ref().expect("no order");
    assert_eq!((o.target_x, o.target_y), (dx, dy));
}

/// A destination FAR east of the opening group (MOV-08):
/// the scan looks near the Scout's row for a run of 52+
/// unblocked cells and names a point 46 cells into the run,
/// so the group faces a 40+-cell (160+ world-unit) move —
/// past the ~30-tile column threshold — with headroom
/// beyond it for the formation's front slots.
fn far_east_dest(sim: &Sim) -> (I16F16, I16F16) {
    let scout = ent(sim, unit_of(sim, EntityKind::ScoutCavalry));
    let g = sim.grid();
    let (cx, cy) = g.world_to_cell(scout.pos_x, scout.pos_y);
    for row in (cy.saturating_sub(10))..=(cy + 10).min(119) {
        let mut run_start: Option<u32> = None;
        for x in cx.saturating_sub(4)..120u32 {
            if g.is_blocked(x, row) {
                run_start = None;
                continue;
            }
            let start = *run_start.get_or_insert(x);
            if x + 1 - start >= 52 {
                return g.cell_center(start + 46, row);
            }
        }
    }
    panic!("no 52-cell open run near the scout");
}

#[test]
fn opening_group_travels_in_column_on_a_long_move() {
    let mut sim = Sim::new(7);
    let scout = unit_of(&sim, EntityKind::ScoutCavalry);
    let vils = villagers(&sim);
    let group: Vec<u32> = std::iter::once(scout).chain(vils.iter().copied()).collect();
    let (mut cx, mut cy) = (0i64, 0i64);
    for &id in &group {
        let e = ent(&sim, id);
        cx += e.pos_x.to_bits() as i64;
        cy += e.pos_y.to_bits() as i64;
    }
    let n = group.len() as i64;
    let centroid = (
        I16F16::from_bits((cx / n) as i32),
        I16F16::from_bits((cy / n) as i32),
    );
    let (dx, dy) = far_east_dest(&sim);
    let facing = (dx - centroid.0, dy - centroid.1);
    let members: Vec<FormationMember> = group
        .iter()
        .map(|&id| FormationMember {
            id,
            kind: ent(&sim, id).kind,
        })
        .collect();
    let (assembly, reform) = column_anchor_points(centroid, (dx, dy), facing);
    let assembly_slots = assign_column_slots(&members, assembly, facing);
    let reform_slots = assign_column_slots(&members, reform, facing);
    for &id in &group {
        sim.push_command(Command::FormationMove {
            entity: id,
            x: dx,
            y: dy,
            formation: FormationKind::Line,
        });
    }
    sim.tick();
    for &id in &group {
        let o = ent(&sim, id).order.as_ref().expect("formation order missing");
        // The final target is a formation slot near the
        // destination, not the click point itself...
        assert!(
            (f(o.target_x) - f(dx)).abs() <= 4.0 && (f(o.target_y) - f(dy)).abs() <= 4.0,
            "unit {id}'s target ({}, {}) is not a formation slot near the destination",
            f(o.target_x),
            f(o.target_y)
        );
        // ...and the trail routes through its column slots
        // at both travel anchors (MOV-08).
        let col = |slots: &[(u32, (I16F16, I16F16))]| {
            slots.iter().find(|(i, _)| *i == id).unwrap().1
        };
        assert!(
            o.waypoints.contains(&col(&assembly_slots)),
            "unit {id}'s trail misses its assembly column slot"
        );
        assert!(
            o.waypoints.contains(&col(&reform_slots)),
            "unit {id}'s trail misses its re-form column slot"
        );
    }
}

#[test]
fn formation_opening_travel_replays_bit_identically() {
    let run = || {
        let mut sim = Sim::new(7);
        let scout = unit_of(&sim, EntityKind::ScoutCavalry);
        let vils = villagers(&sim);
        let (dx, dy) = east_dest(&sim);
        for id in std::iter::once(scout).chain(vils.iter().copied()) {
            sim.push_command(Command::FormationMove {
                entity: id,
                x: dx,
                y: dy,
                formation: FormationKind::Box,
            });
        }
        for _ in 0..300 {
            sim.tick();
        }
        sim.state_hash()
    };
    assert_eq!(run(), run(), "formation travel broke replay determinism");
}
