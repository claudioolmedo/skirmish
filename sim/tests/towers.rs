//! BLD-18 behavior tests: the Watch Tower's base fire and the
//! Outpost (vision only), staged end to end through the public
//! command API in TwoPlayer mode (both seats scripted, no AI).
//!
//! The branch package shipped the build-table numbers as
//! assertions inside `build.rs` but no scenario in which a tower
//! actually shoots; these tests close that gap on the landing
//! tree: the Dark-Age gate, the exact stone/wood charges, a
//! foundation's silence, the 1-tile dead zone, a kill in the
//! band with VIC-04 credit to the tower's owner, and the
//! Outpost's harmlessness.

use fixed::types::I16F16;
use skirmish_sim::{
    Age, BuildingKind, Command, EntityKind, MatchMode, NodeKind, ResourceKind, Sim, TechId,
    OWNER_OPPONENT, OWNER_PLAYER,
};

fn f(v: I16F16) -> f64 {
    v.to_num::<f64>()
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

fn cell_point(sim: &Sim, cx: u32, cy: u32) -> (I16F16, I16F16) {
    sim.grid().cell_center(cx, cy)
}

fn cell_of(sim: &Sim, x: I16F16, y: I16F16) -> (u32, u32) {
    sim.grid().world_to_cell(x, y)
}

fn anchor_near(sim: &Sim, entity: u32, kind: BuildingKind, center: (u32, u32)) -> (u32, u32) {
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

fn build_at(sim: &mut Sim, entity: u32, kind: BuildingKind, cell: (u32, u32)) {
    let (x, y) = cell_point(sim, cell.0, cell.1);
    sim.push_command(Command::Build { entity, kind, x, y });
}

fn building_of(sim: &Sim, kind: BuildingKind, owner: u8) -> Option<&skirmish_sim::Building> {
    sim.buildings()
        .iter()
        .find(|b| b.kind == kind && b.owner == owner)
}

fn entity(sim: &Sim, owner: u8, kind: EntityKind) -> &skirmish_sim::Entity {
    sim.entities()
        .iter()
        .find(|e| e.owner == owner && e.kind == kind && !e.dead)
        .unwrap()
}

fn first_node(sim: &Sim, kind: NodeKind) -> u32 {
    sim.nodes()
        .iter()
        .find(|n| n.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?} node"))
        .id
}

fn stock(sim: &Sim, kind: ResourceKind) -> f64 {
    f(sim.stockpile_of(OWNER_PLAYER).amount(kind))
}

/// Build and wait for completion — a fresh order to the builder
/// mid-construction reassigns it and strands the foundation, so
/// staging that re-tasks the builder must wait (the feudal_techs
/// `raise` discipline).
fn raise(sim: &mut Sim, entity: u32, kind: BuildingKind, cell: (u32, u32)) {
    build_at(sim, entity, kind, cell);
    let done = run_until(sim, 120_000, |s| {
        building_of(s, kind, OWNER_PLAYER)
            .map(|b| b.complete)
            .unwrap_or(false)
    });
    assert!(done, "{kind:?} never completed");
}

/// The feudal_techs staging, verbatim in spirit: Mill + Lumber
/// Camp by the Town Center, berries to 500 food, research, wait.
fn reach_feudal(sim: &mut Sim) {
    let tc = sim.town_center();
    let tc_cell = cell_of(sim, tc.pos_x, tc.pos_y);
    let mill = anchor_near(sim, 2, BuildingKind::Mill, tc_cell);
    raise(sim, 2, BuildingKind::Mill, mill);
    let lumber = anchor_near(sim, 3, BuildingKind::LumberCamp, tc_cell);
    raise(sim, 3, BuildingKind::LumberCamp, lumber);
    let berry = first_node(sim, NodeKind::BerryBush);
    for v in [1, 2] {
        sim.push_command(Command::Gather { entity: v, node: berry });
    }
    let food = run_until(sim, 180_000, |s| stock(s, ResourceKind::Food) >= 500.0);
    assert!(food, "never gathered the Feudal food");
    sim.push_command(Command::ResearchTc { tech: TechId::FeudalAge });
    let up = run_until(sim, 60_000, |s| s.age_of(OWNER_PLAYER) == Age::Feudal);
    assert!(up, "never reached Feudal");
}

/// Chop until the wood stockpile reads at least `want`.
fn wait_wood(sim: &mut Sim, want: f64) {
    if stock(sim, ResourceKind::Wood) >= want {
        return;
    }
    let tree = first_node(sim, NodeKind::Tree);
    sim.push_command(Command::Gather { entity: 3, node: tree });
    let ok = run_until(sim, 240_000, |s| stock(s, ResourceKind::Wood) >= want);
    assert!(ok, "never chopped {want} wood (at {})", stock(sim, ResourceKind::Wood));
}

/// Walk the opponent's Scout Cavalry to a world point and wait
/// until it stands within 1.5 wu of it.
fn stage_opp_scout(sim: &mut Sim, x: f64, y: f64) -> u32 {
    let scout = entity(sim, OWNER_OPPONENT, EntityKind::ScoutCavalry).id;
    sim.push_command(Command::Move {
        entity: scout,
        x: I16F16::from_num(x),
        y: I16F16::from_num(y),
    });
    let there = run_until(sim, 90_000, |s| {
        let e = s
            .entities()
            .iter()
            .find(|e| e.id == scout)
            .unwrap();
        let dx = f(e.pos_x) - x;
        let dy = f(e.pos_y) - y;
        dx * dx + dy * dy <= 1.5 * 1.5
    });
    assert!(there, "opponent scout never reached its staging point");
    scout
}

fn hp_of(sim: &Sim, id: u32) -> f64 {
    f(sim.entities().iter().find(|e| e.id == id).unwrap().hp)
}

fn dead(sim: &Sim, id: u32) -> bool {
    sim.entities().iter().find(|e| e.id == id).unwrap().dead
}

#[test]
fn dark_age_gate_and_outpost_is_harmless() {
    let mut sim = Sim::with_mode(21, MatchMode::TwoPlayer);
    let tc = sim.town_center();
    let tc_cell = cell_of(&sim, tc.pos_x, tc.pos_y);
    // The Watch Tower is a Feudal kind: the Dark-Age order is
    // rejected whole — no foundation, no charge.
    let cell = anchor_near(&sim, 1, BuildingKind::Outpost, tc_cell);
    build_at(&mut sim, 1, BuildingKind::WatchTower, cell);
    for _ in 0..600 {
        sim.tick();
    }
    assert!(
        building_of(&sim, BuildingKind::WatchTower, OWNER_PLAYER).is_none(),
        "a Watch Tower foundation stood in the Dark Age"
    );
    assert_eq!(stock(&sim, ResourceKind::Wood), 200.0);
    assert_eq!(stock(&sim, ResourceKind::Stone), 200.0);
    // The Outpost is a Dark-Age kind: 25 W / 5 S at placement.
    build_at(&mut sim, 1, BuildingKind::Outpost, cell);
    let placed = run_until(&mut sim, 30_000, |s| {
        building_of(s, BuildingKind::Outpost, OWNER_PLAYER).is_some()
    });
    assert!(placed, "the Outpost was never placed");
    assert_eq!(stock(&sim, ResourceKind::Wood), 175.0);
    assert_eq!(stock(&sim, ResourceKind::Stone), 195.0);
    let done = run_until(&mut sim, 60_000, |s| {
        building_of(s, BuildingKind::Outpost, OWNER_PLAYER)
            .map(|b| b.complete)
            .unwrap_or(false)
    });
    assert!(done, "the Outpost never completed");
    // Vision only: an enemy scout parked 3 tiles from its edge
    // for a full minute takes no damage at all.
    let ob = building_of(&sim, BuildingKind::Outpost, OWNER_PLAYER).unwrap();
    let (ox, oy) = (f(ob.pos_x), f(ob.pos_y));
    let scout = stage_opp_scout(&mut sim, ox + 2.0 + 12.0, oy);
    let full = hp_of(&sim, scout);
    for _ in 0..3600 {
        sim.tick();
    }
    assert_eq!(hp_of(&sim, scout), full, "the Outpost dealt damage");
}

#[test]
fn watch_tower_foundation_silent_dead_zone_then_kill_credit() {
    let mut sim = Sim::with_mode(23, MatchMode::TwoPlayer);
    reach_feudal(&mut sim);
    wait_wood(&mut sim, 35.0);
    let tc = sim.town_center();
    let tc_cell = cell_of(&sim, tc.pos_x, tc.pos_y);
    let cell = anchor_near(&sim, 1, BuildingKind::WatchTower, tc_cell);
    // Park every villager by the Town Center so no deposit can
    // land inside the measurement window (a move override keeps
    // any carry undeposited), then let them settle.
    let (px, py) = (f(tc.pos_x), f(tc.pos_y));
    for v in [1, 2, 3] {
        sim.push_command(Command::Move {
            entity: v,
            x: I16F16::from_num(px),
            y: I16F16::from_num(py),
        });
    }
    for _ in 0..1200 {
        sim.tick();
    }
    // Stage the enemy scout in the band FIRST (4 tiles from the
    // footprint edge of the cell the tower will occupy): its
    // cross-map walk is longer than the tower's construction, so
    // it must already be standing there when the foundation is
    // placed. A 1x1 building's position is its cell's center.
    let (cxp, cyp) = cell_point(&sim, cell.0, cell.1);
    let (tx, ty) = (f(cxp), f(cyp));
    let scout = stage_opp_scout(&mut sim, tx + 2.0 + 16.0, ty);
    let full = hp_of(&sim, scout);
    // Exact charges at placement: 35 wood, 125 stone (reach_feudal
    // spent its own; read the deltas around the placement).
    let (w0, s0) = (stock(&sim, ResourceKind::Wood), stock(&sim, ResourceKind::Stone));
    build_at(&mut sim, 1, BuildingKind::WatchTower, cell);
    let placed = run_until(&mut sim, 30_000, |s| {
        building_of(s, BuildingKind::WatchTower, OWNER_PLAYER).is_some()
    });
    assert!(placed, "the Watch Tower was never placed");
    assert_eq!(stock(&sim, ResourceKind::Wood), w0 - 35.0);
    assert_eq!(stock(&sim, ResourceKind::Stone), s0 - 125.0);
    // While the tower is a foundation, no shot may leave.
    for _ in 0..600 {
        sim.tick();
    }
    assert_eq!(hp_of(&sim, scout), full, "a foundation fired");
    assert!(
        !building_of(&sim, BuildingKind::WatchTower, OWNER_PLAYER).unwrap().complete,
        "the tower completed during the foundation window (staging broken)"
    );
    // Into the dead zone while the foundation is still silent:
    // 3 wu from the center is 0.75 tiles — inside the 1-tile
    // minimum under either range convention (edge or center).
    let scout2 = stage_opp_scout(&mut sim, tx + 3.0, ty);
    assert_eq!(scout2, scout);
    assert_eq!(hp_of(&sim, scout), full, "the move itself was shot");
    // Completion (80 s of solo work) with the scout parked in the
    // dead zone the whole time.
    let done = run_until(&mut sim, 240_000, |s| {
        building_of(s, BuildingKind::WatchTower, OWNER_PLAYER)
            .map(|b| b.complete)
            .unwrap_or(false)
    });
    assert!(done, "the Watch Tower never completed");
    // Send the builder away: an idle villager beside the tower
    // would be a lawful target for the staged scout's stance.
    sim.push_command(Command::Move {
        entity: 1,
        x: I16F16::from_num(px),
        y: I16F16::from_num(py),
    });
    for _ in 0..3600 {
        sim.tick();
    }
    assert_eq!(hp_of(&sim, scout), full, "the dead zone was not safe");
    // Back in the band the tower fires, kills, and the kill is
    // credited to the tower's owner (VIC-04 via last_hit_owner).
    let scout3 = stage_opp_scout(&mut sim, tx + 2.0 + 16.0, ty);
    assert_eq!(scout3, scout);
    let hurt = run_until(&mut sim, 60_000, |s| hp_of(s, scout) < full);
    assert!(hurt, "the tower never hit a target in its band");
    let killed = run_until(&mut sim, 240_000, |s| dead(s, scout));
    assert!(killed, "the tower never killed a target in its band");
    assert!(
        sim.score_for(OWNER_PLAYER).military > 0,
        "no VIC-04 military credit for the tower's kill"
    );
}
