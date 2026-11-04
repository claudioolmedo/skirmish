//! Forest-interior trees (bot-loop defect, live 2026-11-03 +
//! harness reproduction on the seed-7 map, trees 32/33): a tree
//! whose 8 ring cells are ALL blocked by other trees has no
//! standing cell, so `move_order_to_node` installed no order and
//! the gather assignment died silently in `work_tick` — the
//! villager idled forever beside the forest while the player who
//! clicked that tree (trunk or canopy) saw nothing happen. The
//! codebase's doctrine is that interior trees "are workable the
//! AoE2 way, by chopping in from the edge": the order now
//! substitutes the nearest workable tree and re-points the
//! assignment at it; depletion retargeting (ECO-09) then walks
//! the cutter inward. These tests pin the substitution, the
//! untouched normal case, and determinism of the new path.

use fixed::types::I16F16;
use skirmish_sim::{Command, EntityKind, NodeKind, ResourceKind, Sim, Task};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

/// A tree is "interior" when every one of its 8 ring cells is
/// blocked — the exact predicate `tree_stand_order` fails on.
fn ring_all_blocked(sim: &Sim, node_id: u32) -> bool {
    let n = sim.nodes().iter().find(|n| n.id == node_id).unwrap();
    let (tx, ty) = sim.grid().world_to_cell(n.pos_x, n.pos_y);
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (cx, cy) = (tx as i32 + dx, ty as i32 + dy);
            if cx < 0 || cy < 0 || cx >= sim.grid().width() as i32 || cy >= sim.grid().width() as i32 {
                continue;
            }
            if !sim.grid().is_blocked(cx as u32, cy as u32) {
                return false;
            }
        }
    }
    true
}

fn interior_tree(sim: &Sim) -> u32 {
    sim.nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Tree)
        .map(|n| n.id)
        .find(|&id| ring_all_blocked(sim, id))
        .expect("the seed-7 forest has interior trees")
}

fn nearest_villager(sim: &Sim, x: I16F16, y: I16F16) -> u32 {
    sim.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager)
        .min_by_key(|e| {
            let dx = f(e.pos_x - x);
            let dy = f(e.pos_y - y);
            ((dx * dx + dy * dy) * 1024.0) as i64
        })
        .unwrap()
        .id
}

#[test]
fn gather_on_interior_tree_substitutes_the_edge_and_chops() {
    let mut sim = Sim::new(7);
    let tree = interior_tree(&sim);
    let t = sim.nodes().iter().find(|n| n.id == tree).unwrap();
    let (tx, ty) = (t.pos_x, t.pos_y);
    let v = nearest_villager(&sim, tx, ty);
    let start_wood = sim.stockpile().amount(ResourceKind::Wood);

    sim.push_command(Command::Gather { entity: v, node: tree });
    sim.tick();
    let vill = sim.entities().iter().find(|e| e.id == v).unwrap();
    assert_ne!(
        vill.task_node,
        Some(tree),
        "the assignment is re-pointed at a workable tree, not the interior one"
    );
    assert!(
        matches!(vill.task, Task::ToNode | Task::Gathering),
        "the villager is on a work assignment, not dropped: {:?}",
        vill.task
    );
    let substitute = vill.task_node.expect("a substitute assignment exists");

    // Within 3 game minutes the substitute tree is being felled
    // and wood reaches the stockpile: the click produced work.
    let sub_start = sim
        .nodes()
        .iter()
        .find(|n| n.id == substitute)
        .unwrap()
        .amount_left;
    let mut chopped = false;
    let mut delivered = false;
    for _ in 0..10_800 {
        sim.tick();
        let amt = sim
            .nodes()
            .iter()
            .find(|n| n.id == substitute)
            .unwrap()
            .amount_left;
        if amt < sub_start {
            chopped = true;
        }
        if sim.stockpile().amount(ResourceKind::Wood) > start_wood {
            delivered = true;
        }
        if chopped && delivered {
            break;
        }
    }
    assert!(chopped, "the substitute tree is felled (amount drops)");
    assert!(delivered, "wood from the substitute reaches the stockpile");
}

#[test]
fn gather_on_edge_tree_still_targets_that_tree() {
    let mut sim = Sim::new(7);
    let edge = sim
        .nodes()
        .iter()
        .filter(|n| n.kind == NodeKind::Tree)
        .map(|n| n.id)
        .find(|&id| !ring_all_blocked(&sim, id))
        .expect("the seed-7 forest has edge trees");
    let t = sim.nodes().iter().find(|n| n.id == edge).unwrap();
    let v = nearest_villager(&sim, t.pos_x, t.pos_y);
    sim.push_command(Command::Gather { entity: v, node: edge });
    sim.tick();
    let vill = sim.entities().iter().find(|e| e.id == v).unwrap();
    assert_eq!(
        vill.task_node,
        Some(edge),
        "a workable tree keeps its own assignment (no substitution)"
    );
}

#[test]
fn interior_tree_substitution_is_deterministic() {
    let run = || {
        let mut sim = Sim::new(7);
        let tree = interior_tree(&sim);
        let t = sim.nodes().iter().find(|n| n.id == tree).unwrap();
        let v = nearest_villager(&sim, t.pos_x, t.pos_y);
        sim.push_command(Command::Gather { entity: v, node: tree });
        for _ in 0..3600 {
            sim.tick();
        }
        sim.state_hash()
    };
    assert_eq!(run(), run(), "same order, same stream, same hash");
}
