//! REPAIRPROBE (2026-11-19): follow-up to BUILDPROBE — do the
//! `Task::Repair` / `Task::RepairTc` arms of `work_tick` have
//! the same stale-order freeze shape the ToBuild arm had?
//!
//! BUILDPROBE proved: a `ToBuild` builder whose move order's
//! target cell became blocked AFTER the order was issued froze
//! forever — steering cannot enter a blocked cell, so the order
//! never completed, and the retry/give-up branch only ran when
//! `order.is_none()`. The fix drops a live order whose target
//! cell is now blocked (`build_order_target_blocked`) into the
//! existing retry/give-up path. The Repair arms, read on the
//! pristine base, have the identical shape: in range → work;
//! `order.is_none()` → approach/give-up; a live order out of
//! range → nothing, forever, if that order can never complete.
//!
//! Scenes (all staged through the real `Command` path —
//! `Build`, `Repair`, `RepairTc`, `Move`, `AttackBuilding`,
//! `Attack`, `DeleteBuilding`, `DeleteEntity` — in TwoPlayer
//! matches; damage is dealt by the opposing Scout, the repair
//! suite's proven route; no state is poked, no stockpile is
//! topped up; anchor/parking cells are CHOSEN by reading the
//! map, which is scene selection, not state on the path under
//! test):
//!
//! (a) target reaches full HP via another repairer while this
//!     repairer is still en route;
//! (b) target is destroyed by combat while en route (building);
//!     the Town Center's removal is staged via `DeleteEntity`
//!     (a TC has 2400 HP — no starting force destroys one inside
//!     a probe window; the deletion path lands on the same
//!     `dead`/missing-target release branch);
//! (c) the repairer's stand cell is blocked AFTER the repair
//!     order was issued — the BUILDPROBE seal: the target's
//!     whole stand ring is walled with completed Palisade
//!     segments except the one cell the repairer's order
//!     targets, and a closer then starts a segment on THAT
//!     cell while the repairer walks from far away;
//! (d) target building is deleted (`DeleteBuilding`) en route.
//!
//! The freeze signature, measured over a ≥14,400-tick window:
//! task stays `Repair`/`RepairTc` with a live order, the
//! target's HP never moves, and the repairer never appears in
//! the idle-villager query (`idle_villagers_of`). The healthy
//! outcomes are exactly two: the repairer gets in range and
//! works, or he is released to Idle (visible, re-taskable).
//!
//! OUTCOME (2026-11-19): paths (a), (b) and (d) were already
//! correct in BOTH arms — the full-HP check, the missing/dead
//! target branches and the destruction sweep all release an
//! en-route repairer to Idle, idle-visible, with no limbo.
//! Path (c) is the BUILDPROBE freeze, PROVEN in both arms on
//! the pristine base: Repair limbo 14,400/14,400 ticks at
//! 14.94 wu from the sealed House (build range 10.0), HP
//! 510→510; RepairTc limbo 14,400/14,400 at 17.55 wu from the
//! sealed Town Center (range 15.0), HP 2360→2360. The minimal
//! fix mirrors `ToBuild`: both arms now drop a live order
//! whose target cell has become blocked
//! (`build_order_target_blocked`) into the existing
//! retry/give-up path — post-fix both seal scenes release to
//! Idle with limbo 0 and HP unmoved. (One staging lesson,
//! recorded in the helpers: the House seal's keep cell must be
//! a ring CORNER — with the north-edge cell kept, a late-
//! arriving repairer can press to 9.96 wu, inside the 10.0
//! range, and legitimately work; the corner's press distance
//! is ~11.3+, outside range in every approach geometry.)

use crate::build::BuildingKind;
use crate::entity::{Entity, EntityKind, Task};
use crate::{Command, MatchMode, ResourceKind, Sim, OWNER_OPPONENT, OWNER_PLAYER};
use fixed::types::I16F16;

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).unwrap()
}

fn find_id(sim: &Sim, owner: u8, kind: EntityKind) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind && !e.dead)
        .unwrap()
        .id
}

fn villagers_of(sim: &Sim, owner: u8) -> Vec<u32> {
    sim.entities()
        .iter()
        .filter(|e| e.owner == owner && e.kind == EntityKind::Villager && !e.dead)
        .map(|e| e.id)
        .collect()
}

fn building(sim: &Sim, bid: u32) -> &crate::build::Building {
    sim.buildings().iter().find(|b| b.id == bid).unwrap()
}

fn wood(sim: &Sim, owner: u8) -> f64 {
    f(sim.stockpile_of(owner).amount(ResourceKind::Wood))
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

fn anchor_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn build(sim: &mut Sim, entity: u32, kind: BuildingKind, cx: u32, cy: u32) {
    let (x, y) = anchor_point(sim, cx, cy);
    sim.push_command(Command::Build { entity, kind, x, y });
}

fn move_to_cell(sim: &mut Sim, entity: u32, cell: (u32, u32)) {
    let (x, y) = anchor_point(sim, cell.0, cell.1);
    sim.push_command(Command::Move { entity, x, y });
}

/// Issue a Move to `cell` and wait until the villager is
/// actually there: tick once first (a pushed command only
/// applies on the next tick — checking `order.is_none()`
/// before that reads the pre-command state and returns at
/// once, the staging trap this probe's first run hit), then
/// wait for the order to complete, then verify the arrival
/// (within a cell of the target center).
fn move_and_wait(sim: &mut Sim, entity: u32, cell: (u32, u32)) -> bool {
    move_to_cell(sim, entity, cell);
    sim.tick();
    if ent(sim, entity).order.is_none() {
        // The order already completed (he was standing there)
        // or was refused; the position check decides.
    }
    let arrived = run_until(sim, 30_000, |s| ent(s, entity).order.is_none());
    if !arrived {
        return false;
    }
    let (tx, ty) = anchor_point(sim, cell.0, cell.1);
    let e = ent(sim, entity);
    f(e.pos_x - tx).abs() <= 6.0 && f(e.pos_y - ty).abs() <= 6.0
}

/// Park `entity` on the open cell at Chebyshev distance `dist`
/// (scanning outward) from `center`, and wait until the move
/// completes. The cell must be unblocked and off the target's
/// footprint and ring (pass the forbidden set explicitly).
fn park_far(
    sim: &mut Sim,
    entity: u32,
    center: (u32, u32),
    dist: i32,
    forbidden: &[(u32, u32)],
) -> (u32, u32) {
    for r in dist..=dist + 10 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (center.0 as i32 + dx, center.1 as i32 + dy);
                if x < 1 || y < 1 {
                    continue;
                }
                let cell = (x as u32, y as u32);
                if forbidden.contains(&cell) || sim.grid().is_blocked(cell.0, cell.1) {
                    continue;
                }
                if move_and_wait(sim, entity, cell) {
                    return cell;
                }
            }
        }
    }
    panic!("no parkable cell at distance {dist} from {center:?}");
}

/// The first House anchor near `center_id`'s Town Center where
/// the House validates AND every one of its 12 ring cells
/// individually validates as a Palisade segment (the seal
/// scenes need a fully sealable ring; the other scenes just
/// need a house).
fn house_anchor(sim: &Sim, center_id: u32, builder: u32, sealable: bool) -> (u32, u32) {
    let c = ent(sim, center_id);
    let (tcx, tcy) = sim.grid().world_to_cell(c.pos_x, c.pos_y);
    for r in 2..=14i32 {
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
                let (x, y) = anchor_point(sim, ax, ay);
                if !sim.can_build(builder, BuildingKind::House, x, y) {
                    continue;
                }
                if sealable {
                    let ring = crate::build::rect_ring_cells(ax, ay, 2, 2);
                    let ok = ring.iter().all(|&(cx, cy)| {
                        let (px, py) = anchor_point(sim, cx, cy);
                        sim.can_build(builder, BuildingKind::PalisadeWall, px, py)
                    });
                    if !ok {
                        continue;
                    }
                }
                return (ax, ay);
            }
        }
    }
    panic!("no house anchor near the Town Center");
}

struct DamagedHouse {
    sim: Sim,
    bid: u32,
    hp: f64,
    max_hp: f64,
    anchor: (u32, u32),
    opp_villagers: Vec<u32>,
    human_scout: u32,
}

/// Build an opponent House, have the human Scout beat it down
/// until at least `dent` HP is missing (the repair suite's
/// stall-breaker included: the building chase can jam on a
/// ring corner, so a damage stall re-seats the Scout on a west
/// edge ring cell and re-issues the attack), withdraw the
/// Scout, and verify the HP is stable.
fn damaged_house(seed: u64, dent: f64, sealable: bool) -> DamagedHouse {
    let mut sim = Sim::with_mode(seed, MatchMode::TwoPlayer);
    let opp_tc = find_id(&sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let human_tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let opp_villagers = villagers_of(&sim, OWNER_OPPONENT);
    let human_scout = find_id(&sim, OWNER_PLAYER, EntityKind::ScoutCavalry);
    let opp_scout = find_id(&sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    let max_hp = sim.building_max_hp(BuildingKind::House, OWNER_OPPONENT) as f64;

    // Park the opponent's Scout far away so the damage phase is
    // a clean one-sided beating (the repair suite's staging).
    let (tc_x, tc_y) = {
        let tc = ent(&sim, opp_tc);
        (tc.pos_x, tc.pos_y)
    };
    sim.push_command(Command::Move {
        entity: opp_scout,
        x: tc_x + I16F16::from_num(80),
        y: tc_y + I16F16::from_num(80),
    });

    let v1 = opp_villagers[0];
    let anchor = house_anchor(&sim, opp_tc, v1, sealable);
    build(&mut sim, v1, BuildingKind::House, anchor.0, anchor.1);
    sim.tick();
    assert_eq!(sim.buildings().len(), 1, "house placement was rejected");
    let bid = sim.buildings()[0].id;
    let up = run_until(&mut sim, 30_000, |s| building(s, bid).complete);
    assert!(up, "house never completed");
    // Send the builder home so the ring is clear for the Scout.
    sim.push_command(Command::Move { entity: v1, x: tc_x, y: tc_y });

    sim.push_command(Command::AttackBuilding { entity: human_scout, building: bid });
    let stop_hp = max_hp - dent;
    let mut last_hp = max_hp;
    let mut stall = 0u64;
    let mut ticks = 0u64;
    let beaten = loop {
        if f(building(&sim, bid).hp) <= stop_hp {
            break true;
        }
        if ticks > 600_000 {
            break false;
        }
        sim.tick();
        ticks += 1;
        let hp = f(building(&sim, bid).hp);
        if hp < last_hp {
            last_hp = hp;
            stall = 0;
        } else {
            stall += 1;
        }
        if stall >= 3_000 {
            stall = 0;
            let b = building(&sim, bid);
            let (wax, way) = (b.cell_x - 1, b.cell_y + b.kind.footprint().w / 2);
            let (wx, wy) = anchor_point(&sim, wax, way);
            sim.push_command(Command::Move { entity: human_scout, x: wx, y: wy });
            run_until(&mut sim, 10_000, |s| ent(s, human_scout).order.is_none());
            sim.push_command(Command::AttackBuilding { entity: human_scout, building: bid });
        }
    };
    assert!(beaten, "the Scout never dented the house by {dent} HP");
    let htc = ent(&sim, human_tc);
    sim.push_command(Command::Move { entity: human_scout, x: htc.pos_x, y: htc.pos_y });
    sim.tick();
    let hp = f(building(&sim, bid).hp);
    for _ in 0..30 {
        sim.tick();
    }
    assert_eq!(f(building(&sim, bid).hp), hp, "HP moved after the attacker withdrew");
    assert!(hp > 1.0, "the house must survive the beating");
    DamagedHouse { sim, bid, hp, max_hp, anchor, opp_villagers, human_scout }
}

/// Dent the PLAYER's Town Center with the opponent's Scout
/// (the tc_repair suite's staging): order the attack, wait
/// until at least `amount` HP is lost, withdraw the Scout
/// toward its own Town Center until it is far away.
fn dent_player_tc(sim: &mut Sim, amount: f64) -> f64 {
    let tc = find_id(sim, OWNER_PLAYER, EntityKind::TownCenter);
    let scout = find_id(sim, OWNER_OPPONENT, EntityKind::ScoutCavalry);
    let full = f(ent(sim, tc).hp);
    sim.push_command(Command::Attack { entity: scout, target: tc });
    let dented = run_until(sim, 240_000, |s| f(ent(s, tc).hp) <= full - amount);
    assert!(dented, "opponent Scout never dented the Town Center");
    let opp_tc = find_id(sim, OWNER_OPPONENT, EntityKind::TownCenter);
    let (x, y) = {
        let t = ent(sim, opp_tc);
        (t.pos_x, t.pos_y)
    };
    sim.push_command(Command::Move { entity: scout, x, y });
    let gone = run_until(sim, 120_000, |s| {
        let e = ent(s, scout);
        let t = ent(s, find_id(s, OWNER_PLAYER, EntityKind::TownCenter));
        let dx = f(e.pos_x - t.pos_x);
        let dy = f(e.pos_y - t.pos_y);
        dx * dx + dy * dy > 40.0 * 40.0
    });
    assert!(gone, "opponent Scout never withdrew");
    f(ent(sim, tc).hp)
}

fn tc_cell(sim: &Sim, owner: u8) -> (u32, u32) {
    let tc = find_id(sim, owner, EntityKind::TownCenter);
    let t = ent(sim, tc);
    sim.grid().world_to_cell(t.pos_x, t.pos_y)
}

/// A repair measurement over a fixed window: the limbo
/// signature is `limbo_ticks` running to the whole window with
/// the target's HP unmoved and the repairer never idle-listed.
#[derive(Default, Debug)]
struct RepairMeasure {
    limbo_ticks: u64,
    end_task: Option<Task>,
    end_order: bool,
    end_idle_listed: bool,
    hp_start: f64,
    hp_end: f64,
    min_dist: f64,
    end_dist: f64,
}

fn measure_repair_building(sim: &mut Sim, vil: u32, bid: u32, owner: u8, window: u64) -> RepairMeasure {
    let mut m = RepairMeasure { min_dist: f64::INFINITY, ..Default::default() };
    m.hp_start = f(building(sim, bid).hp);
    for _ in 0..window {
        sim.tick();
        let e = ent(sim, vil);
        if matches!(e.task, Task::Repair { .. }) && e.order.is_some() {
            m.limbo_ticks += 1;
        }
        let b = building(sim, bid);
        let d = f(crate::steer::vec_len(e.pos_x - b.pos_x, e.pos_y - b.pos_y));
        if d < m.min_dist {
            m.min_dist = d;
        }
        m.end_dist = d;
    }
    let e = ent(sim, vil);
    m.end_task = Some(e.task);
    m.end_order = e.order.is_some();
    m.end_idle_listed = sim.idle_villagers_of(owner).contains(&vil);
    m.hp_end = sim.buildings().iter().find(|b| b.id == bid).map(|b| f(b.hp)).unwrap_or(-1.0);
    m
}

fn measure_repair_tc(sim: &mut Sim, vil: u32, tc: u32, owner: u8, window: u64) -> RepairMeasure {
    let mut m = RepairMeasure { min_dist: f64::INFINITY, ..Default::default() };
    m.hp_start = f(ent(sim, tc).hp);
    for _ in 0..window {
        sim.tick();
        let e = ent(sim, vil);
        if matches!(e.task, Task::RepairTc { .. }) && e.order.is_some() {
            m.limbo_ticks += 1;
        }
        let t = ent(sim, tc);
        let d = f(crate::steer::vec_len(e.pos_x - t.pos_x, e.pos_y - t.pos_y));
        if d < m.min_dist {
            m.min_dist = d;
        }
        m.end_dist = d;
    }
    let e = ent(sim, vil);
    m.end_task = Some(e.task);
    m.end_order = e.order.is_some();
    m.end_idle_listed = sim.idle_villagers_of(owner).contains(&vil);
    m.hp_end = f(ent(sim, tc).hp);
    m
}

fn report(tag: &str, m: &RepairMeasure) -> String {
    format!(
        "{tag} limbo={} end_task={:?} end_order={} idle_listed={} hp {:.1}->{:.1} min_dist={:.2} end_dist={:.2}",
        m.limbo_ticks, m.end_task, m.end_order, m.end_idle_listed, m.hp_start, m.hp_end, m.min_dist, m.end_dist
    )
}

// ---------------------------------------------------------------------
// Baseline control: a far repairer walks and finishes, untouched.
// ---------------------------------------------------------------------

#[test]
fn baseline_far_repairer_walks_and_finishes() {
    let mut fx = damaged_house(701, 40.0, false);
    let bid = fx.bid;
    let repairer = fx.opp_villagers[0];
    let ring = crate::build::rect_ring_cells(fx.anchor.0, fx.anchor.1, 2, 2);
    park_far(&mut fx.sim, repairer, fx.anchor, 16, &ring);
    assert!(fx.sim.can_repair(repairer, bid));
    fx.sim.push_command(Command::Repair { entity: repairer, building: bid });
    fx.sim.tick();
    {
        let e = ent(&fx.sim, repairer);
        assert_eq!(e.task, Task::Repair { building: bid }, "repair order did not take");
        assert!(e.order.is_some(), "far repairer has no travel order");
    }
    let full = run_until(&mut fx.sim, 60_000, |s| f(building(s, bid).hp) >= fx.max_hp - 0.01);
    assert!(full, "baseline: the far repairer never finished the house");
    let idle = run_until(&mut fx.sim, 2_000, |s| ent(s, repairer).task == Task::Idle);
    assert!(idle, "baseline: repairer did not stand down at full HP");
    assert!(fx.sim.idle_villagers_of(OWNER_OPPONENT).contains(&repairer));
}

// ---------------------------------------------------------------------
// (a) Full HP via another repairer releases the en-route repairer.
// ---------------------------------------------------------------------

#[test]
fn a_repair_full_hp_by_other_releases_enroute_repairer() {
    let mut fx = damaged_house(703, 40.0, false);
    let bid = fx.bid;
    let near = fx.opp_villagers[1];
    let far = fx.opp_villagers[0];
    let ring = crate::build::rect_ring_cells(fx.anchor.0, fx.anchor.1, 2, 2);
    // The near repairer waits ON a ring cell; the far one waits
    // 16 cells out — its travel leg is far longer than the
    // ~200 ticks the near repairer needs for 40 HP.
    assert!(move_and_wait(&mut fx.sim, near, ring[4]), "staging: near repairer never reached the ring");
    park_far(&mut fx.sim, far, fx.anchor, 16, &ring);
    fx.sim.push_command(Command::Repair { entity: near, building: bid });
    fx.sim.push_command(Command::Repair { entity: far, building: bid });
    fx.sim.tick();
    for _ in 0..30 {
        fx.sim.tick();
    }
    {
        let e = ent(&fx.sim, far);
        assert_eq!(e.task, Task::Repair { building: bid }, "far repairer not on duty");
        assert!(e.order.is_some(), "far repairer not en route at sample");
    }
    let full = run_until(&mut fx.sim, 60_000, |s| f(building(s, bid).hp) >= fx.max_hp - 0.01);
    assert!(full, "(a) the house never reached full HP");
    // The en-route repairer must stand down (the work step's
    // full-HP check or the repair phase's release) and become
    // visible to the idle query — never freeze with the order.
    let m = measure_repair_building(&mut fx.sim, far, bid, OWNER_OPPONENT, 14_400);
    eprintln!("REPAIRPROBE {}", report("a-repair", &m));
    assert_eq!(m.end_task, Some(Task::Idle), "(a) en-route repairer not released: {}", report("a-repair", &m));
    assert!(m.end_idle_listed, "(a) released repairer invisible to the idle query");
    assert!(!m.end_order, "(a) released repairer still holds an order");
}

#[test]
fn a_repairtc_full_hp_by_other_releases_enroute_repairer() {
    let mut sim = Sim::with_mode(705, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    dent_player_tc(&mut sim, 40.0);
    let (tcx, tcy) = tc_cell(&sim, OWNER_PLAYER);
    let ring = crate::build::rect_ring_cells(tcx - 1, tcy - 1, 4, 4);
    let near = vils[1];
    let far = vils[0];
    assert!(move_and_wait(&mut sim, near, ring[5]), "staging: near repairer never reached the TC ring");
    park_far(&mut sim, far, (tcx, tcy), 16, &ring);
    sim.push_command(Command::RepairTc { entity: near, tc });
    sim.push_command(Command::RepairTc { entity: far, tc });
    sim.tick();
    for _ in 0..30 {
        sim.tick();
    }
    {
        let e = ent(&sim, far);
        assert_eq!(e.task, Task::RepairTc { tc }, "far repairer not on TC duty");
        assert!(e.order.is_some(), "far TC repairer not en route at sample");
    }
    let max_hp = sim.stats_for(OWNER_PLAYER, EntityKind::TownCenter).max_hp as f64;
    let full = run_until(&mut sim, 60_000, |s| f(ent(s, tc).hp) >= max_hp - 0.01);
    assert!(full, "(a) the Town Center never reached full HP");
    let m = measure_repair_tc(&mut sim, far, tc, OWNER_PLAYER, 14_400);
    eprintln!("REPAIRPROBE {}", report("a-repairtc", &m));
    assert_eq!(m.end_task, Some(Task::Idle), "(a) en-route TC repairer not released: {}", report("a-repairtc", &m));
    assert!(m.end_idle_listed, "(a) released TC repairer invisible to the idle query");
    assert!(!m.end_order, "(a) released TC repairer still holds an order");
}

// ---------------------------------------------------------------------
// (b) Destroyed en route (building, combat) / removed (TC, delete).
// ---------------------------------------------------------------------

#[test]
fn b_repair_target_destroyed_enroute_releases_repairer() {
    // Dent deep (to <= 90 HP) so the Scout's second beating
    // kills quickly once the repairer is on the road.
    let mut fx = damaged_house(707, 460.0, false);
    let bid = fx.bid;
    assert!(fx.hp <= 90.0, "fixture HP {} not low enough for a quick kill", fx.hp);
    let repairer = fx.opp_villagers[0];
    let ring = crate::build::rect_ring_cells(fx.anchor.0, fx.anchor.1, 2, 2);
    park_far(&mut fx.sim, repairer, fx.anchor, 22, &ring);
    fx.sim.push_command(Command::Repair { entity: repairer, building: bid });
    fx.sim.tick();
    {
        let e = ent(&fx.sim, repairer);
        assert_eq!(e.task, Task::Repair { building: bid });
        assert!(e.order.is_some(), "repairer not en route at sample");
    }
    // The Scout returns and finishes the house off.
    let scout = fx.human_scout;
    fx.sim.push_command(Command::AttackBuilding { entity: scout, building: bid });
    let mut stall = 0u64;
    let mut last_hp = fx.hp;
    let mut ticks = 0u64;
    let gone = loop {
        if !fx.sim.buildings().iter().any(|b| b.id == bid) {
            break true;
        }
        if ticks > 600_000 {
            break false;
        }
        fx.sim.tick();
        ticks += 1;
        if let Some(b) = fx.sim.buildings().iter().find(|b| b.id == bid) {
            let hp = f(b.hp);
            if hp < last_hp {
                last_hp = hp;
                stall = 0;
            } else {
                stall += 1;
            }
            if stall >= 3_000 {
                stall = 0;
                let (wax, way) = (b.cell_x - 1, b.cell_y + b.kind.footprint().w / 2);
                let (wx, wy) = anchor_point(&fx.sim, wax, way);
                fx.sim.push_command(Command::Move { entity: scout, x: wx, y: wy });
                run_until(&mut fx.sim, 10_000, |s| ent(s, scout).order.is_none());
                fx.sim.push_command(Command::AttackBuilding { entity: scout, building: bid });
            }
        }
    };
    assert!(gone, "(b) the Scout never destroyed the house");
    // Whatever state the repairer was in when the target fell,
    // he must end released and idle-visible, with no limbo.
    let mut limbo_after = 0u64;
    for _ in 0..14_400 {
        fx.sim.tick();
        let e = ent(&fx.sim, repairer);
        if matches!(e.task, Task::Repair { .. }) {
            limbo_after += 1;
        }
    }
    let e = ent(&fx.sim, repairer);
    eprintln!(
        "REPAIRPROBE b-repair limbo_after_destroy={} end_task={:?} idle_listed={}",
        limbo_after,
        e.task,
        fx.sim.idle_villagers_of(OWNER_OPPONENT).contains(&repairer)
    );
    assert_eq!(e.task, Task::Idle, "(b) repairer of a destroyed building not released");
    assert!(fx.sim.idle_villagers_of(OWNER_OPPONENT).contains(&repairer), "(b) repairer invisible to the idle query");
}

#[test]
fn b_repairtc_target_removed_enroute_releases_repairer() {
    let mut sim = Sim::with_mode(709, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    dent_player_tc(&mut sim, 40.0);
    let (tcx, tcy) = tc_cell(&sim, OWNER_PLAYER);
    let ring = crate::build::rect_ring_cells(tcx - 1, tcy - 1, 4, 4);
    let repairer = vils[0];
    park_far(&mut sim, repairer, (tcx, tcy), 18, &ring);
    sim.push_command(Command::RepairTc { entity: repairer, tc });
    sim.tick();
    {
        let e = ent(&sim, repairer);
        assert_eq!(e.task, Task::RepairTc { tc });
        assert!(e.order.is_some(), "TC repairer not en route at sample");
    }
    // The owner deletes its own Town Center (SEL-14) while the
    // repairer walks: the entity dies through the ordinary
    // death sweep, the same end state combat destruction
    // reaches, on the arm's dead/missing-target branch.
    sim.push_command(Command::DeleteEntity { entity: tc });
    let mut limbo_after = 0u64;
    for _ in 0..14_400 {
        sim.tick();
        let e = ent(&sim, repairer);
        if matches!(e.task, Task::RepairTc { .. }) {
            limbo_after += 1;
        }
    }
    let e = ent(&sim, repairer);
    eprintln!(
        "REPAIRPROBE b-repairtc limbo_after_remove={} end_task={:?} idle_listed={}",
        limbo_after,
        e.task,
        sim.idle_villagers_of(OWNER_PLAYER).contains(&repairer)
    );
    assert_eq!(e.task, Task::Idle, "(b) TC repairer not released when the TC was removed");
    assert!(sim.idle_villagers_of(OWNER_PLAYER).contains(&repairer), "(b) TC repairer invisible to the idle query");
}

// ---------------------------------------------------------------------
// (d) Deleted en route (building).
// ---------------------------------------------------------------------

#[test]
fn d_repair_target_deleted_enroute_releases_repairer() {
    let mut fx = damaged_house(711, 40.0, false);
    let bid = fx.bid;
    let repairer = fx.opp_villagers[0];
    let ring = crate::build::rect_ring_cells(fx.anchor.0, fx.anchor.1, 2, 2);
    park_far(&mut fx.sim, repairer, fx.anchor, 18, &ring);
    fx.sim.push_command(Command::Repair { entity: repairer, building: bid });
    fx.sim.tick();
    {
        let e = ent(&fx.sim, repairer);
        assert_eq!(e.task, Task::Repair { building: bid });
        assert!(e.order.is_some(), "repairer not en route at sample");
    }
    let wood_before_delete = wood(&fx.sim, OWNER_OPPONENT);
    fx.sim.push_command(Command::DeleteBuilding { building: bid });
    fx.sim.tick();
    assert!(!fx.sim.buildings().iter().any(|b| b.id == bid), "delete did not remove the house");
    // The release itself must be prompt (the destruction sweep
    // stands repairers down; the work step's missing-target
    // branch is the backstop).
    let released = run_until(&mut fx.sim, 5_000, |s| {
        ent(s, repairer).task == Task::Idle && s.idle_villagers_of(OWNER_OPPONENT).contains(&repairer)
    });
    assert!(released, "(d) repairer not promptly released + idle-listed after the delete");
    // Long window: he must never re-enter repair duty on the
    // gone building while alive (a wolf can kill a parked
    // villager over 14,400 ticks — that is staging weather,
    // not the release under test, so the count reads live
    // repair ticks only).
    let mut limbo_after = 0u64;
    for _ in 0..14_400 {
        fx.sim.tick();
        let e = ent(&fx.sim, repairer);
        if matches!(e.task, Task::Repair { .. }) && !e.dead {
            limbo_after += 1;
        }
    }
    let e = ent(&fx.sim, repairer);
    eprintln!(
        "REPAIRPROBE d-repair limbo_after_delete={} end_task={:?} dead={}",
        limbo_after, e.task, e.dead
    );
    assert_eq!(limbo_after, 0, "(d) repairer re-entered repair duty on a deleted building");
    assert!(!matches!(e.task, Task::Repair { .. }), "(d) repairer ends on repair duty for a deleted building");
    // A completed building refunds nothing (BLD-02).
    assert_eq!(wood(&fx.sim, OWNER_OPPONENT), wood_before_delete);
}

// ---------------------------------------------------------------------
// (c) The seal scenes: the stand ring closes AFTER the order.
// ---------------------------------------------------------------------

/// The ring cells of an anchor in clockwise order starting at
/// the north-west corner (BUILDPROBE's seal order).
fn ring_cw(anchor: (u32, u32), w: u32, h: u32) -> Vec<(u32, u32)> {
    let (x0, y0) = (anchor.0 - 1, anchor.1 - 1);
    let (x1, y1) = (anchor.0 + w, anchor.1 + h);
    let mut out = Vec::new();
    for x in x0..=x1 {
        out.push((x, y0));
    }
    for y in (y0 + 1)..=y1 {
        out.push((x1, y));
    }
    for x in (x0..x1).rev() {
        out.push((x, y1));
    }
    for y in ((y0 + 1)..y1).rev() {
        out.push((x0, y));
    }
    out
}

/// Build completed Palisade segments on every ring cell of
/// `anchor` (footprint `w`×`h`) except `keep`, using `sealer`
/// alone through the real Build path, one segment at a time,
/// parking the sealer on open ground outside the ring after
/// each (BUILDPROBE's rotational discipline, generalized).
/// `center` is the site's center cell, for parking scans.
fn seal_ring_except(
    sim: &mut Sim,
    sealer: u32,
    anchor: (u32, u32),
    w: u32,
    h: u32,
    keep: (u32, u32),
    center: (u32, u32),
) {
    let cw = ring_cw(anchor, w, h);
    let ring = crate::build::rect_ring_cells(anchor.0, anchor.1, w, h);
    assert_eq!(cw.len(), ring.len(), "cw ring and rect ring disagree");
    for c in &cw {
        assert!(ring.contains(c), "staging broken: {c:?} is not a ring cell");
    }
    let k = cw.iter().position(|&c| c == keep).expect("keep must be a ring cell");
    let order: Vec<(u32, u32)> = (1..cw.len()).map(|i| cw[(k + i) % cw.len()]).collect();
    // Parking: open cells at Chebyshev distance 3..=6 from the
    // center, off the ring and off the footprint.
    let footprint: Vec<(u32, u32)> = (0..w)
        .flat_map(|dx| (0..h).map(move |dy| (anchor.0 + dx, anchor.1 + dy)))
        .collect();
    let mut forbidden = ring.clone();
    forbidden.extend(footprint.iter().copied());
    let park = park_cell_near(sim, center, &forbidden);
    for cell in order {
        assert!(
            matches!(ent(sim, sealer).task, Task::Idle),
            "staging broken: sealer busy before segment {cell:?}"
        );
        build(sim, sealer, BuildingKind::PalisadeWall, cell.0, cell.1);
        let done = run_until(sim, 60_000, |s| {
            s.buildings().iter().any(|b| {
                b.kind == BuildingKind::PalisadeWall && (b.cell_x, b.cell_y) == cell && b.complete
            })
        });
        assert!(done, "staging broken: seal segment at {cell:?} never completed");
        let idle = run_until(sim, 30_000, |s| ent(s, sealer).task == Task::Idle);
        assert!(idle, "staging broken: sealer never stood down");
        move_to_cell(sim, sealer, park);
        let out = run_until(sim, 60_000, |s| {
            let e = ent(s, sealer);
            e.order.is_none() && !ring.contains(&s.grid().world_to_cell(e.pos_x, e.pos_y))
        });
        assert!(out, "staging broken: sealer never parked clear of the ring");
    }
}

fn park_cell_near(sim: &Sim, center: (u32, u32), forbidden: &[(u32, u32)]) -> (u32, u32) {
    for r in 3..=8i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (center.0 as i32 + dx, center.1 as i32 + dy);
                if x < 1 || y < 1 {
                    continue;
                }
                let cell = (x as u32, y as u32);
                if forbidden.contains(&cell) || sim.grid().is_blocked(cell.0, cell.1) {
                    continue;
                }
                return cell;
            }
        }
    }
    panic!("no park cell near {center:?}");
}

/// Stage `closer` on the open cell diagonally outside `keep`
/// (one step further from the site's center), ready to wall
/// the kept cell shut the moment the repair order lands.
fn stage_closer(sim: &mut Sim, closer: u32, center: (u32, u32), keep: (u32, u32)) {
    let dx = (keep.0 as i32 - center.0 as i32).signum();
    let dy = (keep.1 as i32 - center.1 as i32).signum();
    // Outward candidates, diagonal first, then the orthogonal
    // exits, then one step further out: the first unblocked
    // cell the closer can actually reach and stand on.
    let mut candidates: Vec<(u32, u32)> = Vec::new();
    for (ox, oy) in [(dx, dy), (dx, 0), (0, dy), (2 * dx, 2 * dy), (2 * dx, dy), (dx, 2 * dy)] {
        let (x, y) = (keep.0 as i32 + ox, keep.1 as i32 + oy);
        if x < 1 || y < 1 {
            continue;
        }
        let cell = (x as u32, y as u32);
        if !candidates.contains(&cell) && !sim.grid().is_blocked(cell.0, cell.1) {
            candidates.push(cell);
        }
    }
    for cell in candidates {
        if move_and_wait(sim, closer, cell) {
            return;
        }
    }
    panic!("staging broken: the closer never took position outside {keep:?}");
}

#[test]
fn c_repair_stand_cell_blocked_after_order() {
    let mut fx = damaged_house(713, 40.0, true);
    let bid = fx.bid;
    let a = fx.anchor;
    let repairer = fx.opp_villagers[0];
    let sealer = fx.opp_villagers[1];
    let closer = fx.opp_villagers[2];
    let keep = (a.0 - 1, a.1 - 1); // the ring's north-west corner cell
    let center = (a.0 + 1, a.1 + 1);
    stage_closer(&mut fx.sim, closer, center, keep);
    seal_ring_except(&mut fx.sim, sealer, a, 2, 2, keep, center);
    let ring = crate::build::rect_ring_cells(a.0, a.1, 2, 2);
    park_far(&mut fx.sim, repairer, a, 18, &ring);
    // The repair order: with the ring sealed but for `keep`,
    // the approach can only choose the kept cell.
    assert!(fx.sim.can_repair(repairer, bid));
    fx.sim.push_command(Command::Repair { entity: repairer, building: bid });
    fx.sim.tick();
    {
        let e = ent(&fx.sim, repairer);
        assert_eq!(e.task, Task::Repair { building: bid }, "repair order did not take");
        let o = e.order.as_ref().expect("far repairer has no travel order");
        assert_eq!(
            fx.sim.grid().world_to_cell(o.target_x, o.target_y),
            keep,
            "the repairer's order does not target the kept cell"
        );
    }
    let hp_at_issue = f(building(&fx.sim, bid).hp);
    // The closer walls the kept cell shut.
    build(&mut fx.sim, closer, BuildingKind::PalisadeWall, keep.0, keep.1);
    let sealed = run_until(&mut fx.sim, 30_000, |s| s.grid().is_dynamic(keep.0, keep.1));
    assert!(sealed, "staging broken: the kept segment never started (cell never blocked)");
    // Long window: the repairer must build (impossible — the
    // ring is sealed) or be released to Idle. The freeze is:
    // limbo for the whole window, HP unmoved, never idle.
    let m = measure_repair_building(&mut fx.sim, repairer, bid, OWNER_OPPONENT, 14_400);
    eprintln!("REPAIRPROBE {}", report("c-repair-seal", &m));
    assert_eq!(m.hp_end, hp_at_issue, "(c) HP moved — the repairer worked, staging is not a seal");
    assert_eq!(
        m.end_task,
        Some(Task::Idle),
        "(c) repairer froze with a stale order outside a sealed site: {}",
        report("c-repair-seal", &m)
    );
    assert!(m.end_idle_listed, "(c) released repairer invisible to the idle query");
    assert!(!m.end_order, "(c) released repairer still holds an order");
}

#[test]
fn c_repairtc_stand_cell_blocked_after_order() {
    let mut sim = Sim::with_mode(715, MatchMode::TwoPlayer);
    let tc = find_id(&sim, OWNER_PLAYER, EntityKind::TownCenter);
    let vils = villagers_of(&sim, OWNER_PLAYER);
    dent_player_tc(&mut sim, 40.0);
    let (tcx, tcy) = tc_cell(&sim, OWNER_PLAYER);
    let anchor = (tcx - 1, tcy - 1); // the TC footprint's anchor (BLD-16)
    let center = (tcx, tcy);
    let keep = (tcx - 2, tcy - 2); // the ring's north-west corner
    let repairer = vils[0];
    let sealer = vils[1];
    let closer = vils[2];
    stage_closer(&mut sim, closer, center, keep);
    let ring = crate::build::rect_ring_cells(anchor.0, anchor.1, 4, 4);
    park_far(&mut sim, repairer, center, 10, &ring);
    seal_ring_except(&mut sim, sealer, anchor, 4, 4, keep, center);
    assert!(sim.can_repair_tc(repairer, tc));
    sim.push_command(Command::RepairTc { entity: repairer, tc });
    sim.tick();
    {
        let e = ent(&sim, repairer);
        assert_eq!(e.task, Task::RepairTc { tc }, "RepairTc order did not take");
        let o = e.order.as_ref().expect("far TC repairer has no travel order");
        assert_eq!(
            sim.grid().world_to_cell(o.target_x, o.target_y),
            keep,
            "the TC repairer's order does not target the kept cell"
        );
    }
    let hp_at_issue = f(ent(&sim, tc).hp);
    build(&mut sim, closer, BuildingKind::PalisadeWall, keep.0, keep.1);
    let sealed = run_until(&mut sim, 30_000, |s| s.grid().is_dynamic(keep.0, keep.1));
    assert!(sealed, "staging broken: the kept TC segment never started (cell never blocked)");
    let m = measure_repair_tc(&mut sim, repairer, tc, OWNER_PLAYER, 14_400);
    eprintln!("REPAIRPROBE {}", report("c-repairtc-seal", &m));
    assert_eq!(m.hp_end, hp_at_issue, "(c) TC HP moved — the repairer worked, staging is not a seal");
    assert_eq!(
        m.end_task,
        Some(Task::Idle),
        "(c) TC repairer froze with a stale order outside a sealed ring: {}",
        report("c-repairtc-seal", &m)
    );
    assert!(m.end_idle_listed, "(c) released TC repairer invisible to the idle query");
    assert!(!m.end_order, "(c) released TC repairer still holds an order");
}

