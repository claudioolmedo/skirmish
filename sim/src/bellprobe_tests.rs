//! BELLPROBE (2026-11-19): diagnosis probe for the live
//! observation in bot-loop run #429 — after Town Bell →
//! "back to work", ungarrisoned villagers stayed IDLE beside
//! the Town Center instead of resuming their previous tasks.
//!
//! The rule probed against is GAR-07 (docs/MECHANICS-RULEBOOK.md,
//! Status MATCHES): ringing a Town Center's bell sends every
//! villager within 23 tiles to garrison in the nearest shelter,
//! snapshotting task + position; ringing again ungarrisons them
//! and restores each villager to its snapshot task and location
//! (snapshot: position + task + node/building ids; restore the
//! gather/build/repair task when its target still stands, else
//! walk back to the snapshot position).
//!
//! The existing `garrison_tests` GAR-07 family pins task-VARIANT
//! resumption (the restored villager's `Task` matches the
//! snapshot's family within a tick window). This probe measures
//! what a player actually sees, per snapshot task type staged
//! DISTINCTLY before the bell:
//!
//!   - gathering (berry / tree / gold): does the node keep
//!     depleting after the restore, and does a load actually get
//!     DELIVERED to the stockpile?
//!   - building: does construction progress resume and finish?
//!   - repairing (building and Town Center arms): does the
//!     target's HP resume rising and reach full?
//!   - farming: does the farm node keep depleting / deliver?
//!   - Returning (mid-haul, full carry): is the assignment kept,
//!     does the sheltered load get deposited, and by what route?
//!   - fallbacks: snapshot target depleted / destroyed while
//!     garrisoned → the villager must walk back to the snapshot
//!     POSITION (measured in world units), not idle at the exit.
//!   - controls: idle villager, and a villager mid-Move order.
//!
//! Every test asserts the GAR-07-documented outcome and prints
//! its measurements (`BELLPROBE …` lines, visible with
//! `--nocapture`): snapshot task/position, task immediately
//! after the restore tick, ticks to resumed progress, node /
//! stockpile / HP / progress deltas, and the final task +
//! position error vs the snapshot position. A red test here IS
//! the diagnosis; the numbers are the handoff table.

use crate::build::{Building, BuildingKind};
use crate::econ::{NodeKind, ResourceKind, ResourceNode};
use crate::entity::{Entity, EntityKind, GarrisonRef, Task, OWNER_PLAYER};
use crate::{Command, Sim};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
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

/// Run up to `cap` ticks, returning how many elapsed when `done`
/// first held (or `cap` if it never did).
fn ticks_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> u64 {
    for t in 0..cap {
        if done(sim) {
            return t;
        }
        sim.tick();
    }
    cap
}

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

fn placed(sim: &Sim, id: u32, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> Building {
    let size = kind.footprint().w;
    let (x0, y0) = sim.grid().cell_center(cell.0, cell.1);
    let half = I16F16::from_num(size as i32 * 2);
    Building {
        id,
        kind,
        owner,
        hp: I16F16::from_num(kind.max_hp()),
        cell_x: cell.0,
        cell_y: cell.1,
        pos_x: x0 + half - I16F16::from_num(2),
        pos_y: y0 + half - I16F16::from_num(2),
        progress_thirds: kind.build_thirds(),
        repair_hp_remainder: 0,
        repair_cost_remainder: 0,
        repair_stone_remainder: 0,
        repair_gold_remainder: 0,
        build_rate_remainder: 0,
        gate_along_x: true,
        queue_rate_remainder: 0,
        complete: true,
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
        relics: Vec::new(),
        relic_gold_remainder: 0,
    }
}

fn player_villager_ids(sim: &Sim) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER && !e.dead)
        .map(|e| e.id)
        .collect()
}

/// Spawn `n` extra player villagers on free cells in the ring
/// around the player's Town Center (inside the bell radius) —
/// the `garrison_tests` fixture, verbatim discipline.
fn spawn_villagers_near_tc(sim: &mut Sim, n: usize, first_id: u32) -> Vec<u32> {
    let tc = sim.town_center().id;
    let (tx, ty) = {
        let e = ent(sim, tc);
        (e.pos_x, e.pos_y)
    };
    let (ccx, ccy) = sim.grid().world_to_cell(tx, ty);
    let mut out = Vec::new();
    let mut id = first_id;
    'outer: for ring in 3i32..=6 {
        for dx in -ring..=ring {
            for dy in -ring..=ring {
                if dx.abs() != ring && dy.abs() != ring {
                    continue;
                }
                if out.len() >= n {
                    break 'outer;
                }
                let cx = ccx as i32 + dx;
                let cy = ccy as i32 + dy;
                if cx < 0 || cy < 0 || cx >= crate::GRID_DIM as i32 || cy >= crate::GRID_DIM as i32 {
                    continue;
                }
                let (cx, cy) = (cx as u32, cy as u32);
                if sim.grid().is_blocked(cx, cy) {
                    continue;
                }
                let (wx, wy) = sim.grid().cell_center(cx, cy);
                if sim
                    .entities()
                    .iter()
                    .any(|e| !e.dead && e.pos_x == wx && e.pos_y == wy)
                {
                    continue;
                }
                spawn(sim, id, EntityKind::Villager, OWNER_PLAYER, (cx, cy));
                out.push(id);
                id += 1;
            }
        }
    }
    assert_eq!(out.len(), n, "could not place {n} villagers near the TC");
    out
}

fn nearest_node_of_kind(sim: &Sim, from: (I16F16, I16F16), kind: NodeKind) -> u32 {
    sim.nodes()
        .iter()
        .filter(|n| n.kind == kind && n.amount_left > I16F16::ZERO)
        .min_by_key(|n| crate::steer::vec_len(n.pos_x - from.0, n.pos_y - from.1).to_bits())
        .unwrap_or_else(|| panic!("no live {kind:?} node on this map"))
        .id
}

fn node_amount(sim: &Sim, id: u32) -> f64 {
    sim.nodes()
        .iter()
        .find(|n| n.id == id)
        .map(|n| f(n.amount_left))
        .unwrap_or(-1.0)
}

fn node_pos(sim: &Sim, id: u32) -> (I16F16, I16F16) {
    let n = sim.nodes().iter().find(|n| n.id == id).unwrap();
    (n.pos_x, n.pos_y)
}

fn dist(sim: &Sim, id: u32, x: I16F16, y: I16F16) -> f64 {
    let e = ent(sim, id);
    f(crate::steer::vec_len(e.pos_x - x, e.pos_y - y))
}

fn stock(sim: &Sim, kind: ResourceKind) -> f64 {
    f(sim.stockpile().amount(kind))
}

/// One villager's bell snapshot, captured the tick after the
/// ring command applies.
#[derive(Clone, Copy, Debug)]
struct Snap {
    task: Task,
    task_node: Option<u32>,
    task_building: Option<u32>,
    x: I16F16,
    y: I16F16,
}

fn snapshot_of(sim: &Sim, id: u32) -> Snap {
    let s = ent(sim, id)
        .bell_snapshot
        .unwrap_or_else(|| panic!("villager {id} holds no bell snapshot after the ring"));
    Snap {
        task: s.task,
        task_node: s.task_node,
        task_building: s.task_building,
        x: s.pos_x,
        y: s.pos_y,
    }
}

/// Ring the bell and wait until every player villager is inside
/// a shelter. Returns each requested villager's snapshot.
fn ring_and_shelter(sim: &mut Sim, tc: u32, watch: &[u32]) -> Vec<Snap> {
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    assert!(sim.tc_bell_active(tc), "the bell did not ring");
    let snaps: Vec<Snap> = watch.iter().map(|&id| snapshot_of(sim, id)).collect();
    let inside = run_until(sim, 60_000, |s| {
        player_villager_ids(s)
            .iter()
            .all(|&id| ent(s, id).garrisoned_in.is_some())
    });
    assert!(
        inside,
        "the bell did not shelter every villager: {:?}",
        player_villager_ids(sim)
            .iter()
            .map(|&id| (id, ent(sim, id).garrisoned_in, ent(sim, id).garrison_target))
            .collect::<Vec<_>>()
    );
    snaps
}

/// Ring again ("back to work") and apply the restore tick.
fn back_to_work(sim: &mut Sim, tc: u32) {
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    assert!(!sim.tc_bell_active(tc), "the bell is still active after the second ring");
    assert_eq!(sim.garrison_count_tc(tc), 0, "the TC did not empty on back-to-work");
}

// ------------------------------------------------------------------
// Gathering scenes (berry / tree / gold): the node must keep
// depleting after the restore and a load must reach the stockpile.

fn gather_scene(kind: NodeKind, resource: ResourceKind, label: &str) {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    let vpos = {
        let e = ent(&sim, vil);
        (e.pos_x, e.pos_y)
    };
    let picked = nearest_node_of_kind(&sim, vpos, kind);
    sim.push_command(Command::Gather {
        entity: vil,
        node: picked,
    });
    // Stage: actively Gathering with a partial load — gathering
    // is PROVEN before the bell, not just tasked. The node of
    // record is the villager's actual assignment (a tree order
    // can be substituted to a workable neighbour at order time).
    let staged = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Gathering && ent(s, vil).carry >= I16F16::from_num(2)
    });
    assert!(staged, "{label}: the villager never reached Gathering with a load");
    let node = ent(&sim, vil).task_node.expect("a gatherer holds a node");
    let amount_pre = node_amount(&sim, node);
    let carry_pre = f(ent(&sim, vil).carry);
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    assert_eq!(snap.task, Task::Gathering, "{label}: snapshot task {:?}", snap.task);
    assert_eq!(snap.task_node, Some(node), "{label}: snapshot lost its node");
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    let amount_restore = node_amount(&sim, node);
    let stock_restore = stock(&sim, resource);
    // Measure: the node depletes further (gathering resumed)…
    let t_deplete = ticks_until(&mut sim, 60_000, |s| {
        node_amount(s, node) <= amount_restore - 2.0
    });
    // …and a load is delivered to the stockpile (full cycle).
    let t_deliver = ticks_until(&mut sim, 90_000, |s| {
        stock(s, resource) > stock_restore + 0.5
    });
    let e = ent(&sim, vil);
    eprintln!(
        "BELLPROBE {label}: snap_task={:?} node={node} carry_pre={carry_pre:.2} \
         amount_pre={amount_pre:.2} amount_at_restore={amount_restore:.2} \
         task_after_restore={task_after:?} t_deplete={t_deplete} t_deliver={t_deliver} \
         final_task={:?} final_node={:?} final_carry={:.2} stock_delta={:.2} \
         dist_to_snapshot={:.2}",
        snap.task,
        e.task,
        e.task_node,
        f(e.carry),
        stock(&sim, resource) - stock_restore,
        dist(&sim, vil, snap.x, snap.y),
    );
    assert!(
        t_deplete < 60_000,
        "{label}: node did not deplete after back-to-work (gathering did not resume): \
         task_after={task_after:?} final_task={:?} amount {amount_restore:.2} -> {:.2}",
        e.task,
        node_amount(&sim, node),
    );
    assert!(
        t_deliver < 90_000,
        "{label}: no load was delivered after back-to-work: final_task={:?} \
         stock {stock_restore:.2} -> {:.2}",
        e.task,
        stock(&sim, resource),
    );
    assert!(
        matches!(e.task, Task::ToNode | Task::Gathering | Task::Returning)
            && e.task_node == Some(node),
        "{label}: villager ended outside its gather cycle: task={:?} node={:?}",
        e.task,
        e.task_node,
    );
}

#[test]
fn bellprobe_berry_gatherer_resumes_and_delivers() {
    gather_scene(NodeKind::BerryBush, ResourceKind::Food, "berry");
}

#[test]
fn bellprobe_tree_gatherer_resumes_and_delivers() {
    gather_scene(NodeKind::Tree, ResourceKind::Wood, "tree");
}

#[test]
fn bellprobe_gold_gatherer_resumes_and_delivers() {
    gather_scene(NodeKind::GoldMine, ResourceKind::Gold, "gold");
}

// ------------------------------------------------------------------
// Farming: a completed Farm's node is a gather assignment whose
// building record also exists — staged by direct records, the
// completion stamp's own shape (building + placed node).

#[test]
fn bellprobe_farmer_resumes_and_delivers() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let (tcx, tcy) = {
        let e = ent(&sim, tc);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    // First 3x3 anchor near the TC whose cells are all unblocked.
    let mut anchor = None;
    'scan: for ax in (tcx + 5)..=(tcx + 10) {
        for ay in tcy..=(tcy + 6) {
            let mut free = true;
            for cx in ax..ax + 3 {
                for cy in ay..ay + 3 {
                    if sim.grid().is_blocked(cx, cy) {
                        free = false;
                    }
                }
            }
            if free {
                anchor = Some((ax, ay));
                break 'scan;
            }
        }
    }
    let anchor = anchor.expect("no free 3x3 farm plot near the TC");
    let farm_node = 990_001u32;
    let farm = placed(&sim, 900, BuildingKind::Farm, OWNER_PLAYER, anchor);
    let (fx, fy) = (farm.pos_x, farm.pos_y);
    sim.nodes.push(ResourceNode::placed(farm_node, NodeKind::Farm, fx, fy));
    let mut farm = farm;
    farm.node_id = Some(farm_node);
    sim.buildings.push(farm);
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    sim.push_command(Command::Gather {
        entity: vil,
        node: farm_node,
    });
    let staged = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Gathering && ent(s, vil).carry >= I16F16::from_num(2)
    });
    assert!(staged, "farm: the farmer never reached Gathering with a load");
    let amount_pre = node_amount(&sim, farm_node);
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    assert_eq!(snap.task_node, Some(farm_node), "farm: snapshot lost the farm node");
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    let amount_restore = node_amount(&sim, farm_node);
    let stock_restore = stock(&sim, ResourceKind::Food);
    let t_deplete = ticks_until(&mut sim, 60_000, |s| {
        node_amount(s, farm_node) <= amount_restore - 2.0
    });
    let t_deliver = ticks_until(&mut sim, 90_000, |s| {
        stock(s, ResourceKind::Food) > stock_restore + 0.5
    });
    let e = ent(&sim, vil);
    eprintln!(
        "BELLPROBE farm: snap_task={:?} amount_pre={amount_pre:.2} \
         amount_at_restore={amount_restore:.2} task_after_restore={task_after:?} \
         t_deplete={t_deplete} t_deliver={t_deliver} final_task={:?} final_node={:?}",
        snap.task, e.task, e.task_node,
    );
    assert!(
        t_deplete < 60_000,
        "farm: the farm node did not deplete after back-to-work: task_after={task_after:?} \
         final_task={:?} amount {amount_restore:.2} -> {:.2}",
        e.task,
        node_amount(&sim, farm_node),
    );
    assert!(
        t_deliver < 90_000,
        "farm: no food was delivered after back-to-work: final_task={:?}",
        e.task,
    );
}

// ------------------------------------------------------------------
// Building: construction progress must resume and finish.
/// MAP-05 re-stage (map05lib): the fixed House plot at TC+(4,4)
/// is refused on seed 7's reshuffled layout — probe-verified, a
/// starting sheep stands on footprint cell (34,78) and
/// `validate_build`'s standing-unit check rejects the Build
/// order whole (`can_build` false there), so the builder never
/// took the task. Derive the plot from the live map instead:
/// the first ring cell around the TC whose House placement the
/// sim itself accepts for this builder (the scan the repair
/// suites' `house_anchor` already uses).
fn house_plot_near_tc(sim: &Sim, vil: u32) -> (I16F16, I16F16) {
    let tc = sim.town_center().id;
    let (tcx, tcy) = {
        let e = ent(sim, tc);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    for r in 2i32..=10 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (ax, ay) = (tcx as i32 + dx, tcy as i32 + dy);
                if ax < 0 || ay < 0 {
                    continue;
                }
                let (hx, hy) = sim.grid().cell_center(ax as u32, ay as u32);
                if sim.can_build(vil, BuildingKind::House, hx, hy) {
                    return (hx, hy);
                }
            }
        }
    }
    panic!("no can_build House plot near the TC");
}


#[test]
fn bellprobe_builder_resumes_and_completes() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    let (hx, hy) = house_plot_near_tc(&sim, vil);
    sim.push_command(Command::Build {
        entity: vil,
        kind: BuildingKind::House,
        x: hx,
        y: hy,
    });
    let staged = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Building
            && s.buildings()
                .iter()
                .find(|b| b.kind == BuildingKind::House && !b.complete)
                .map(|b| b.progress_thirds > 0)
                .unwrap_or(false)
    });
    assert!(staged, "build: the builder never started building");
    let fid = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::House && !b.complete)
        .map(|b| b.id)
        .unwrap();
    let progress_pre = sim
        .buildings()
        .iter()
        .find(|b| b.id == fid)
        .map(|b| b.progress_thirds)
        .unwrap();
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    assert!(
        matches!(snap.task, Task::ToBuild | Task::Building),
        "build: snapshot task {:?}",
        snap.task
    );
    assert_eq!(snap.task_building, Some(fid), "build: snapshot lost the foundation");
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    let t_resume = ticks_until(&mut sim, 60_000, |s| {
        matches!(ent(s, vil).task, Task::ToBuild | Task::Building)
            && ent(s, vil).task_building == Some(fid)
            && s.buildings()
                .iter()
                .find(|b| b.id == fid)
                .map(|b| b.progress_thirds > progress_pre || b.complete)
                .unwrap_or(false)
    });
    let t_complete = ticks_until(&mut sim, 90_000, |s| {
        s.buildings()
            .iter()
            .find(|b| b.id == fid)
            .map(|b| b.complete)
            .unwrap_or(false)
    });
    let e = ent(&sim, vil);
    eprintln!(
        "BELLPROBE build: snap_task={:?} foundation={fid} progress_pre={progress_pre} \
         task_after_restore={task_after:?} t_resume_progress={t_resume} \
         t_complete={t_complete} final_task={:?}",
        snap.task, e.task,
    );
    assert!(
        t_resume < 60_000,
        "build: construction did not resume after back-to-work: task_after={task_after:?} \
         final_task={:?} task_building={:?}",
        e.task, e.task_building,
    );
    assert!(
        t_complete < 90_000,
        "build: the House never completed after back-to-work: final_task={:?}",
        e.task,
    );
}

// ------------------------------------------------------------------
// Repairing a building: HP must resume rising and reach full.

#[test]
fn bellprobe_repairer_resumes_and_finishes() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    let (tcx, tcy) = {
        let e = ent(&sim, tc);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::House, OWNER_PLAYER, (tcx + 5, tcy)));
    let hi = sim.buildings.iter().position(|b| b.id == 900).unwrap();
    sim.buildings[hi].hp = I16F16::from_num(50);
    let max_hp = f(I16F16::from_num(BuildingKind::House.max_hp()));
    sim.push_command(Command::Repair {
        entity: vil,
        building: 900,
    });
    let staged = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Repair { building: 900 }
            && f(s.buildings().iter().find(|b| b.id == 900).unwrap().hp) > 55.0
    });
    assert!(staged, "repair: the repairer never started repairing");
    let hp_pre = f(sim.buildings().iter().find(|b| b.id == 900).unwrap().hp);
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    assert_eq!(
        snap.task,
        Task::Repair { building: 900 },
        "repair: snapshot task {:?}",
        snap.task
    );
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    let hp_restore = f(sim.buildings().iter().find(|b| b.id == 900).unwrap().hp);
    let t_resume = ticks_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Repair { building: 900 }
            && f(s.buildings().iter().find(|b| b.id == 900).unwrap().hp) > hp_restore + 1.0
    });
    let t_full = ticks_until(&mut sim, 90_000, |s| {
        f(s.buildings().iter().find(|b| b.id == 900).unwrap().hp) >= max_hp - 0.01
    });
    let e = ent(&sim, vil);
    eprintln!(
        "BELLPROBE repair: snap_task={:?} hp_pre={hp_pre:.2} hp_at_restore={hp_restore:.2} \
         task_after_restore={task_after:?} t_resume={t_resume} t_full={t_full} \
         final_task={:?} final_hp={:.2}",
        snap.task,
        e.task,
        f(sim.buildings().iter().find(|b| b.id == 900).unwrap().hp),
    );
    assert!(
        t_resume < 60_000,
        "repair: HP did not resume rising after back-to-work: task_after={task_after:?} \
         final_task={:?} hp {hp_restore:.2} -> {:.2}",
        e.task,
        f(sim.buildings().iter().find(|b| b.id == 900).unwrap().hp),
    );
    assert!(
        t_full < 90_000,
        "repair: the House never reached full HP after back-to-work: final_task={:?}",
        e.task,
    );
}

// ------------------------------------------------------------------
// Repairing the Town Center (the RepairTc snapshot arm).

#[test]
fn bellprobe_tc_repairer_resumes_and_finishes() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    let max_hp = f(ent(&sim, tc).hp); // the undamaged Dark-Age maximum
    let ti = sim.entities.iter().position(|e| e.id == tc).unwrap();
    sim.entities[ti].hp = I16F16::from_num(1200);
    sim.push_command(Command::RepairTc { entity: vil, tc });
    let staged = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::RepairTc { tc } && f(ent(s, tc).hp) > 1205.0
    });
    assert!(staged, "repair-tc: the repairer never started on the TC");
    let hp_pre = f(ent(&sim, tc).hp);
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    assert_eq!(
        snap.task,
        Task::RepairTc { tc },
        "repair-tc: snapshot task {:?}",
        snap.task
    );
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    let hp_restore = f(ent(&sim, tc).hp);
    let t_resume = ticks_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::RepairTc { tc } && f(ent(s, tc).hp) > hp_restore + 1.0
    });
    let t_full = ticks_until(&mut sim, 120_000, |s| f(ent(s, tc).hp) >= max_hp - 0.5);
    let e = ent(&sim, vil);
    eprintln!(
        "BELLPROBE repair-tc: snap_task={:?} hp_pre={hp_pre:.2} hp_at_restore={hp_restore:.2} \
         task_after_restore={task_after:?} t_resume={t_resume} t_full={t_full} \
         final_task={:?} final_hp={:.2}",
        snap.task,
        e.task,
        f(ent(&sim, tc).hp),
    );
    assert!(
        t_resume < 60_000,
        "repair-tc: TC HP did not resume rising after back-to-work: task_after={task_after:?} \
         final_task={:?}",
        e.task,
    );
    assert!(
        t_full < 120_000,
        "repair-tc: the TC never reached full HP after back-to-work: final_task={:?} hp={:.2}",
        e.task,
        f(ent(&sim, tc).hp),
    );
}

// ------------------------------------------------------------------
// Returning: a villager caught mid-haul with a FULL load. GAR-07's
// snapshot task is Returning; the yardstick outcome is that the
// assignment survives, the sheltered load is deposited, and the
// villager ends in its gather cycle — never idle at the exit.
// The route is measured: does it walk back OUT to the node
// before depositing?

#[test]
fn bellprobe_returning_villager_deposits_and_resumes() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    let vpos = {
        let e = ent(&sim, vil);
        (e.pos_x, e.pos_y)
    };
    let picked = nearest_node_of_kind(&sim, vpos, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: vil,
        node: picked,
    });
    // Catch the haul itself: Returning with a full load.
    let staged = run_until(&mut sim, 120_000, |s| {
        ent(s, vil).task == Task::Returning && ent(s, vil).carry >= I16F16::from_num(10)
    });
    assert!(staged, "returning: the villager never hauled a full load");
    let node = ent(&sim, vil).task_node.expect("a hauler keeps its node");
    let (nx, ny) = node_pos(&sim, node);
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    let snap = snapshot_of(&sim, vil);
    assert_eq!(
        snap.task,
        Task::Returning,
        "returning: staging missed the haul — snapshot task {:?}",
        snap.task
    );
    let carry_snap = f(ent(&sim, vil).carry);
    let inside = run_until(&mut sim, 60_000, |s| {
        player_villager_ids(s)
            .iter()
            .all(|&id| ent(s, id).garrisoned_in.is_some())
    });
    assert!(inside, "returning: the bell did not shelter every villager");
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    let stock_restore = stock(&sim, ResourceKind::Food);
    // Step manually: first deposit tick, and whether the villager
    // walked back to the node before depositing.
    let mut t_deposit = u64::MAX;
    let mut visited_node_first = false;
    for t in 0..90_000u64 {
        if dist(&sim, vil, nx, ny) <= 3.0 {
            visited_node_first = true;
        }
        if stock(&sim, ResourceKind::Food) > stock_restore + 0.5 {
            t_deposit = t;
            break;
        }
        sim.tick();
    }
    let deposited = t_deposit != u64::MAX;
    // Let the cycle settle after the deposit.
    for _ in 0..600 {
        sim.tick();
    }
    let e = ent(&sim, vil);
    eprintln!(
        "BELLPROBE returning: snap_task={:?} node={node} carry_at_snap={carry_snap:.2} \
         task_after_restore={task_after:?} t_deposit={} visited_node_before_deposit={} \
         final_task={:?} final_node={:?} stock_delta={:.2}",
        snap.task,
        if deposited { t_deposit as i64 } else { -1 },
        visited_node_first,
        e.task,
        e.task_node,
        stock(&sim, ResourceKind::Food) - stock_restore,
    );
    assert!(
        deposited,
        "returning: the sheltered load was never deposited after back-to-work: \
         task_after={task_after:?} final_task={:?} carry={:.2}",
        e.task,
        f(e.carry),
    );
    assert!(
        matches!(e.task, Task::ToNode | Task::Gathering | Task::Returning)
            && e.task_node == Some(node),
        "returning: villager ended outside its gather cycle: task={:?} node={:?}",
        e.task,
        e.task_node,
    );
}

// ------------------------------------------------------------------
// Fallbacks: the snapshot target dies while the villager is
// garrisoned. GAR-07: walk back to the snapshot position.

#[test]
fn bellprobe_fallback_node_depleted_walks_back_to_snapshot() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    let vpos = {
        let e = ent(&sim, vil);
        (e.pos_x, e.pos_y)
    };
    let node = nearest_node_of_kind(&sim, vpos, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: vil,
        node,
    });
    let staged = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Gathering && ent(s, vil).carry >= I16F16::from_num(1)
    });
    assert!(staged, "fallback-node: the villager never started gathering");
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    // The bush runs dry while the villager shelters.
    let ni = sim.nodes.iter().position(|n| n.id == node).unwrap();
    sim.nodes[ni].amount_left = I16F16::ZERO;
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    // Watch for any retarget to a different node on the way.
    let mut retargeted = false;
    let mut settled = false;
    for _ in 0..60_000 {
        let e = ent(&sim, vil);
        if let Some(n) = e.task_node {
            if n != node && matches!(e.task, Task::ToNode | Task::Gathering | Task::Returning) {
                retargeted = true;
            }
        }
        if e.task == Task::Idle && e.order.is_none() {
            settled = true;
            break;
        }
        sim.tick();
    }
    let e = ent(&sim, vil);
    let d = dist(&sim, vil, snap.x, snap.y);
    eprintln!(
        "BELLPROBE fallback-node: snap_task={:?} node={node} task_after_restore={task_after:?} \
         retargeted={retargeted} settled_idle={settled} final_task={:?} final_node={:?} \
         dist_to_snapshot={d:.2} carry_kept={:.2}",
        snap.task,
        e.task,
        e.task_node,
        f(e.carry),
    );
    assert!(
        settled,
        "fallback-node: the villager never settled (task={:?}, order={})",
        e.task,
        e.order.is_some()
    );
    assert!(
        d < 3.0,
        "fallback-node: GAR-07 fallback is the snapshot position — villager settled \
         {d:.2} wu away from it (task={:?}); if it idled beside the TC instead, \
         this is run #429's defect in sim form",
        e.task,
    );
}

#[test]
fn bellprobe_fallback_repair_target_destroyed_walks_back_to_snapshot() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    let (tcx, tcy) = {
        let e = ent(&sim, tc);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::House, OWNER_PLAYER, (tcx + 5, tcy)));
    let hi = sim.buildings.iter().position(|b| b.id == 900).unwrap();
    sim.buildings[hi].hp = I16F16::from_num(50);
    sim.push_command(Command::Repair {
        entity: vil,
        building: 900,
    });
    let staged = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Repair { building: 900 }
    });
    assert!(staged, "fallback-repair: the repairer never took the task");
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    // The House is destroyed while the repairer shelters.
    sim.buildings.retain(|b| b.id != 900);
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    let settled = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Idle && ent(s, vil).order.is_none()
    });
    let e = ent(&sim, vil);
    let d = dist(&sim, vil, snap.x, snap.y);
    eprintln!(
        "BELLPROBE fallback-repair: snap_task={:?} task_after_restore={task_after:?} \
         settled_idle={settled} final_task={:?} dist_to_snapshot={d:.2}",
        snap.task, e.task,
    );
    assert!(settled, "fallback-repair: the villager never settled: task={:?}", e.task);
    assert!(
        d < 3.0,
        "fallback-repair: GAR-07 fallback is the snapshot position — villager settled \
         {d:.2} wu away from it (task={:?})",
        e.task,
    );
}

#[test]
fn bellprobe_fallback_foundation_destroyed_walks_back_to_snapshot() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    let (hx, hy) = house_plot_near_tc(&sim, vil);
    sim.push_command(Command::Build {
        entity: vil,
        kind: BuildingKind::House,
        x: hx,
        y: hy,
    });
    let staged = run_until(&mut sim, 60_000, |s| {
        matches!(ent(s, vil).task, Task::ToBuild | Task::Building)
    });
    assert!(staged, "fallback-build: the builder never took the task");
    let fid = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::House && !b.complete)
        .map(|b| b.id)
        .expect("the foundation exists");
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    // The foundation is destroyed while the builder shelters.
    sim.buildings.retain(|b| b.id != fid);
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    let settled = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Idle && ent(s, vil).order.is_none()
    });
    let e = ent(&sim, vil);
    let d = dist(&sim, vil, snap.x, snap.y);
    eprintln!(
        "BELLPROBE fallback-build: snap_task={:?} foundation={fid} \
         task_after_restore={task_after:?} settled_idle={settled} final_task={:?} \
         dist_to_snapshot={d:.2}",
        snap.task, e.task,
    );
    assert!(settled, "fallback-build: the villager never settled: task={:?}", e.task);
    assert!(
        d < 3.0,
        "fallback-build: GAR-07 fallback is the snapshot position — villager settled \
         {d:.2} wu away from it (task={:?})",
        e.task,
    );
}

// ------------------------------------------------------------------
// Controls: idle at ring time; walking (Move order) at ring time.

#[test]
fn bellprobe_idle_control_returns_to_snapshot_position() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    let (px, py) = {
        let e = ent(&sim, vil);
        (e.pos_x, e.pos_y)
    };
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    assert_eq!(snap.task, Task::Idle, "idle control: snapshot task {:?}", snap.task);
    back_to_work(&mut sim, tc);
    let settled = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Idle && ent(s, vil).order.is_none()
    });
    let d = dist(&sim, vil, px, py);
    eprintln!(
        "BELLPROBE idle-control: snap_task={:?} settled={settled} dist_to_snapshot={d:.2}",
        snap.task,
    );
    assert!(settled, "idle control: the villager never settled");
    assert!(d < 3.0, "idle control: settled {d:.2} wu from its snapshot position");
}

#[test]
fn bellprobe_walking_control_returns_to_snapshot_not_destination() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    let (tcx, tcy) = {
        let e = ent(&sim, tc);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    // A destination 15 tiles east — inside the bell radius for
    // the whole walk, far from the spawn ring.
    let (dx, dy) = sim.grid().cell_center(tcx + 15, tcy);
    sim.push_command(Command::Move {
        entity: vil,
        x: dx,
        y: dy,
    });
    for _ in 0..30 {
        sim.tick();
    }
    assert!(
        ent(&sim, vil).order.is_some() && dist(&sim, vil, dx, dy) > 10.0,
        "walking control: the villager is not en route (dist {:.2})",
        dist(&sim, vil, dx, dy)
    );
    let snaps = ring_and_shelter(&mut sim, tc, &[vil]);
    let snap = snaps[0];
    assert_eq!(
        snap.task,
        Task::Idle,
        "walking control: a Move order rides `order`, so the snapshot task is {:?}",
        snap.task
    );
    back_to_work(&mut sim, tc);
    let settled = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Idle && ent(s, vil).order.is_none()
    });
    let d_snap = dist(&sim, vil, snap.x, snap.y);
    let d_dest = dist(&sim, vil, dx, dy);
    eprintln!(
        "BELLPROBE walking-control: snap_task={:?} settled={settled} \
         dist_to_snapshot={d_snap:.2} dist_to_original_destination={d_dest:.2}",
        snap.task,
    );
    assert!(settled, "walking control: the villager never settled");
    assert!(
        d_snap < 3.0,
        "walking control: GAR-07 restores the snapshot position — settled {d_snap:.2} wu away"
    );
}

// ------------------------------------------------------------------
// Shelter variation: the bell sends the villager to the NEAREST
// shelter — a Watch Tower beside its berry patch, not the Town
// Center. The restore must work from any host.

#[test]
fn bellprobe_tower_sheltered_gatherer_resumes_and_delivers() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    // The FARTHEST berry bush still inside the bell radius: the
    // near bushes sit ~10 wu from the TC center, where the TC's
    // own edge wins the nearest-shelter comparison against any
    // tower placed beside the bush (measured, twice). At a far
    // bush the tower beside it wins decisively.
    let (tcx_w, tcy_w) = {
        let e = ent(&sim, tc);
        (e.pos_x, e.pos_y)
    };
    let node = sim
        .nodes()
        .iter()
        .filter(|n| {
            n.kind == NodeKind::BerryBush
                && n.amount_left > I16F16::ZERO
                && crate::steer::vec_len(n.pos_x - tcx_w, n.pos_y - tcy_w)
                    <= I16F16::from_num(60)
        })
        .max_by_key(|n| {
            crate::steer::vec_len(n.pos_x - tcx_w, n.pos_y - tcy_w).to_bits()
        })
        .expect("a berry bush within the bell radius")
        .id;
    // A Watch Tower one cell east of the bush: its footprint
    // edge is the nearest shelter to the gatherer at the bush.
    let (ncx, ncy) = sim.grid().world_to_cell(node_pos(&sim, node).0, node_pos(&sim, node).1);
    sim.buildings.push(placed(
        &sim,
        900,
        BuildingKind::WatchTower,
        OWNER_PLAYER,
        (ncx + 1, ncy),
    ));
    sim.push_command(Command::Gather {
        entity: vil,
        node,
    });
    let staged = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Gathering && ent(s, vil).carry >= I16F16::from_num(2)
    });
    assert!(staged, "tower: the villager never reached Gathering");
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    let snap = snapshot_of(&sim, vil);
    let inside = run_until(&mut sim, 60_000, |s| {
        player_villager_ids(s)
            .iter()
            .all(|&id| ent(s, id).garrisoned_in.is_some())
    });
    assert!(inside, "tower: the bell did not shelter every villager");
    assert_eq!(
        ent(&sim, vil).garrisoned_in,
        Some(GarrisonRef::Building(900)),
        "tower: staging failed — the gatherer sheltered in {:?}, not the tower",
        ent(&sim, vil).garrisoned_in,
    );
    back_to_work(&mut sim, tc);
    assert_eq!(sim.garrison_count_building(900), 0, "tower: the tower did not empty");
    let task_after = ent(&sim, vil).task;
    let amount_restore = node_amount(&sim, node);
    let stock_restore = stock(&sim, ResourceKind::Food);
    let t_deplete = ticks_until(&mut sim, 60_000, |s| {
        node_amount(s, node) <= amount_restore - 2.0
    });
    let t_deliver = ticks_until(&mut sim, 90_000, |s| {
        stock(s, ResourceKind::Food) > stock_restore + 0.5
    });
    let e = ent(&sim, vil);
    eprintln!(
        "BELLPROBE tower: snap_task={:?} task_after_restore={task_after:?} \
         t_deplete={t_deplete} t_deliver={t_deliver} final_task={:?} final_node={:?}",
        snap.task, e.task, e.task_node,
    );
    assert!(
        t_deplete < 60_000 && t_deliver < 90_000,
        "tower: the tower-sheltered gatherer did not resume: task_after={task_after:?} \
         final_task={:?}",
        e.task,
    );
}

// ------------------------------------------------------------------
// Fast second ring on a TASKED villager: the second ring lands
// while the gatherer is still walking to shelter (snapshot held,
// not yet garrisoned). The walk must cancel and the task resume.

#[test]
fn bellprobe_second_ring_while_walking_to_shelter_restores_task() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let vil = spawn_villagers_near_tc(&mut sim, 1, 500)[0];
    let vpos = {
        let e = ent(&sim, vil);
        (e.pos_x, e.pos_y)
    };
    let node = nearest_node_of_kind(&sim, vpos, NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: vil,
        node,
    });
    let staged = run_until(&mut sim, 60_000, |s| {
        ent(s, vil).task == Task::Gathering && ent(s, vil).carry >= I16F16::from_num(2)
    });
    assert!(staged, "fast-ring: the villager never reached Gathering");
    sim.push_command(Command::TownBell { tc });
    sim.tick();
    let snap = snapshot_of(&sim, vil);
    // A handful of ticks only: the villager is en route, holding
    // a garrison walk and its snapshot, not yet inside.
    for _ in 0..10 {
        sim.tick();
    }
    assert!(
        ent(&sim, vil).garrisoned_in.is_none() && ent(&sim, vil).garrison_target.is_some(),
        "fast-ring: staging failed — the villager is not walking to shelter \
         (inside={:?}, target={:?})",
        ent(&sim, vil).garrisoned_in,
        ent(&sim, vil).garrison_target,
    );
    back_to_work(&mut sim, tc);
    let task_after = ent(&sim, vil).task;
    assert!(
        ent(&sim, vil).garrison_target.is_none(),
        "fast-ring: the villager kept its garrison walk after back-to-work"
    );
    let amount_restore = node_amount(&sim, node);
    let stock_restore = stock(&sim, ResourceKind::Food);
    let t_deplete = ticks_until(&mut sim, 60_000, |s| {
        node_amount(s, node) <= amount_restore - 2.0
    });
    let t_deliver = ticks_until(&mut sim, 90_000, |s| {
        stock(s, ResourceKind::Food) > stock_restore + 0.5
    });
    let e = ent(&sim, vil);
    eprintln!(
        "BELLPROBE fast-ring: snap_task={:?} task_after_restore={task_after:?} \
         t_deplete={t_deplete} t_deliver={t_deliver} final_task={:?} final_node={:?}",
        snap.task, e.task, e.task_node,
    );
    assert!(
        t_deplete < 60_000 && t_deliver < 90_000,
        "fast-ring: the walking gatherer's task did not resume: task_after={task_after:?} \
         final_task={:?}",
        e.task,
    );
}

// ------------------------------------------------------------------
// Combined scene — the run-#429 shape: several DISTINCT tasks in
// ONE base, one bell cycle, each measured by real progress.

#[test]
fn bellprobe_combined_scene_all_tasks_resume_with_progress() {
    let mut sim = Sim::new(7);
    let tc = sim.town_center().id;
    let ids = spawn_villagers_near_tc(&mut sim, 4, 500);
    let (v_berry, v_tree, v_build, v_repair) = (ids[0], ids[1], ids[2], ids[3]);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    let (tcx, tcy) = {
        let e = ent(&sim, tc);
        sim.grid().world_to_cell(e.pos_x, e.pos_y)
    };
    let pos_of = |s: &Sim, id: u32| {
        let e = ent(s, id);
        (e.pos_x, e.pos_y)
    };
    let berry = nearest_node_of_kind(&sim, pos_of(&sim, v_berry), NodeKind::BerryBush);
    sim.push_command(Command::Gather {
        entity: v_berry,
        node: berry,
    });
    let tree = nearest_node_of_kind(&sim, pos_of(&sim, v_tree), NodeKind::Tree);
    sim.push_command(Command::Gather {
        entity: v_tree,
        node: tree,
    });
    let (hx, hy) = house_plot_near_tc(&sim, v_build);
    sim.push_command(Command::Build {
        entity: v_build,
        kind: BuildingKind::House,
        x: hx,
        y: hy,
    });
    sim.buildings
        .push(placed(&sim, 900, BuildingKind::House, OWNER_PLAYER, (tcx + 5, tcy)));
    let hi = sim.buildings.iter().position(|b| b.id == 900).unwrap();
    sim.buildings[hi].hp = I16F16::from_num(50);
    sim.push_command(Command::Repair {
        entity: v_repair,
        building: 900,
    });
    let staged = run_until(&mut sim, 90_000, |s| {
        ent(s, v_berry).task == Task::Gathering
            && ent(s, v_tree).task == Task::Gathering
            && matches!(ent(s, v_build).task, Task::ToBuild | Task::Building)
            && ent(s, v_repair).task == Task::Repair { building: 900 }
    });
    assert!(
        staged,
        "combined: staging failed — berry={:?} tree={:?} build={:?} repair={:?}",
        ent(&sim, v_berry).task,
        ent(&sim, v_tree).task,
        ent(&sim, v_build).task,
        ent(&sim, v_repair).task,
    );
    let berry_node = ent(&sim, v_berry).task_node.unwrap();
    let tree_node = ent(&sim, v_tree).task_node.unwrap();
    let fid = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::House && !b.complete)
        .map(|b| b.id)
        .unwrap();
    let watch = [v_berry, v_tree, v_build, v_repair];
    let snaps = ring_and_shelter(&mut sim, tc, &watch);
    back_to_work(&mut sim, tc);
    let tasks_after: Vec<Task> = watch.iter().map(|&id| ent(&sim, id).task).collect();
    let berry_restore = node_amount(&sim, berry_node);
    let tree_restore = node_amount(&sim, tree_node);
    // Every front must show REAL progress within the window:
    // both nodes deplete further, the foundation completes, the
    // repair finishes.
    let t_all = ticks_until(&mut sim, 120_000, |s| {
        node_amount(s, berry_node) <= berry_restore - 2.0
            && node_amount(s, tree_node) <= tree_restore - 2.0
            && s.buildings()
                .iter()
                .find(|b| b.id == fid)
                .map(|b| b.complete)
                .unwrap_or(false)
            && f(s.buildings().iter().find(|b| b.id == 900).unwrap().hp)
                >= f(I16F16::from_num(BuildingKind::House.max_hp())) - 0.01
    });
    let finals: Vec<(Task, Option<u32>)> = watch
        .iter()
        .map(|&id| {
            let e = ent(&sim, id);
            (e.task, e.task_node)
        })
        .collect();
    eprintln!(
        "BELLPROBE combined: snaps={:?} tasks_after_restore={tasks_after:?} \
         t_all_progress={t_all} finals={finals:?} \
         berry {berry_restore:.2}->{:.2} tree {tree_restore:.2}->{:.2}",
        snaps.iter().map(|s| s.task).collect::<Vec<_>>(),
        node_amount(&sim, berry_node),
        node_amount(&sim, tree_node),
    );
    assert!(
        t_all < 120_000,
        "combined: not every task resumed with real progress after back-to-work: \
         tasks_after={tasks_after:?} finals={finals:?} \
         berry {berry_restore:.2}->{:.2} tree {tree_restore:.2}->{:.2} \
         foundation_complete={} repair_hp={:.2}",
        node_amount(&sim, berry_node),
        node_amount(&sim, tree_node),
        sim.buildings()
            .iter()
            .find(|b| b.id == fid)
            .map(|b| b.complete)
            .unwrap_or(false),
        f(sim.buildings().iter().find(|b| b.id == 900).unwrap().hp),
    );
    // And nobody ends idle beside the TC: both gatherers are in
    // their cycles (the builder/repairer may legitimately be
    // Idle — their jobs FINISHED, at their work sites).
    for (id, node) in [(v_berry, berry_node), (v_tree, tree_node)] {
        let e = ent(&sim, id);
        assert!(
            matches!(e.task, Task::ToNode | Task::Gathering | Task::Returning)
                && e.task_node == Some(node),
            "combined: gatherer {id} ended outside its cycle: task={:?} node={:?}",
            e.task,
            e.task_node,
        );
    }
}
