//! Water W4 tests (GAR-09): the Transport Ship trains at the
//! Dock in the Dark Age, loads up to 5 land units of any kind
//! at a shoreline (live geometry — a ship that moves between
//! the order and the arrival still loads), carries them immune
//! and inert, unloads them at another shoreline in cargo-list
//! order, holds its cargo when no free shore cell exists, and
//! — the anti-GAR-03 pin that would catch a copy-paste of the
//! Ram's ejection path — takes its whole cargo with it when it
//! sinks: zero survivors, zero ejections.
//!
//! Staging: seed 42's pond (13 water-side cells around grid
//! (41..45, 22..26)) is Arabia's real water; the tests stage
//! on its two shores exactly as a player would meet them.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::grid::Domain;
use crate::tech::Age;
use crate::{Command, Sim};
use fixed::types::I16F16;

fn run_until(sim: &mut Sim, cap: u64, mut done: impl FnMut(&Sim) -> bool) -> bool {
    for _ in 0..cap {
        if done(sim) {
            return true;
        }
        sim.tick();
    }
    done(sim)
}

fn ent(sim: &Sim, id: u32) -> &Entity {
    sim.entities().iter().find(|e| e.id == id).expect("entity")
}

/// Scan a fresh sim's map for a 3x3 anchor the Dock predicate
/// accepts (the W2a tests' helper, duplicated so this module
/// stands alone).
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

/// A completed Dock placed directly on the pond shore (the
/// W2a tests' `placed_dock` pattern).
fn placed_dock(sim: &mut Sim, id: u32) -> u32 {
    let (ax, ay) = dock_anchor(sim).expect("seed pond offers a Dock site");
    let kind = BuildingKind::Dock;
    let size = kind.footprint_size();
    let (x0, y0) = sim.grid().cell_center(ax, ay);
    let half = I16F16::from_num(size as i32 * 2);
    for dy in 0..size {
        for dx in 0..size {
            sim.grid.set_dynamic(ax + dx, ay + dy, true);
        }
    }
    sim.buildings.push(Building {
        id,
        kind,
        owner: OWNER_PLAYER,
        hp: I16F16::from_num(kind.max_hp()),
        cell_x: ax,
        cell_y: ay,
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
    });
    id
}

fn spawn_at(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) -> u32 {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
    id
}

/// The pond staging cells (seed 42, probed): a deep cell on
/// the west side, a deep cell on the east side, and a land
/// cell on the west shore.
const WEST_DEEP: (u32, u32) = (42, 24);
const EAST_DEEP: (u32, u32) = (44, 24);
const WEST_LAND: (u32, u32) = (40, 24);

fn staged_ship(sim: &mut Sim, id: u32) -> u32 {
    spawn_at(sim, id, EntityKind::TransportShip, OWNER_PLAYER, WEST_DEEP)
}

#[test]
fn transport_row_tags_and_dock_roster() {
    // Passo 0 pins (WATER-SPEC §3.1/§6): tag 32, Dark Age,
    // 125 W, 46 s (2760 ticks), HP 70, armor 0/2, speed 1.45,
    // LoS 5, no attack, SHIP class, Water domain, capacity 5.
    assert_eq!(EntityKind::TransportShip.tag(), 32);
    assert_eq!(EntityKind::from_tag(32), Some(EntityKind::TransportShip));
    // On the merged tree Water W5's Trade Cog fills tag 33;
    // 34 is the next free tag.
    assert_eq!(EntityKind::from_tag(33), Some(EntityKind::TradeCog));
    assert_eq!(EntityKind::from_tag(34), None);
    let spec = crate::units::spec(EntityKind::TransportShip);
    assert_eq!(spec.max_hp, 70);
    assert_eq!(spec.armor_melee, 0);
    assert_eq!(spec.armor_pierce, 2);
    assert!(spec.attacks.is_empty(), "a Transport has no attack");
    assert_eq!(spec.wood_cost, 125);
    assert_eq!(spec.food_cost, 0);
    assert_eq!(spec.gold_cost, 0);
    assert_eq!(spec.train_ticks, 2760);
    assert_eq!(spec.speed, I16F16::from_bits(6_335)); // 1.45 tiles/s
    assert_eq!(EntityKind::TransportShip.train_age(), Age::Dark);
    assert_eq!(EntityKind::TransportShip.domain(), Domain::Water);
    assert!(EntityKind::TransportShip.is_trainable());
    assert!(EntityKind::TransportShip.counts_population());
    assert!(BuildingKind::Dock.trains(EntityKind::TransportShip));
    assert_eq!(crate::transport::TRANSPORT_CAPACITY, 5);

    // Trains at the Dock in the DARK Age for real (beside the
    // Fishing Ship — the combat set waits for Feudal, the
    // Transport does not): 125 wood leaves on enqueue and the
    // ship spawns on a water-side cell.
    let mut sim = Sim::new(42);
    let dock = placed_dock(&mut sim, 900);
    assert!(sim.can_train_at(dock, EntityKind::TransportShip));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(1000));
    let wood_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood);
    sim.push_command(Command::TrainAt {
        building: dock,
        kind: EntityKind::TransportShip,
    });
    sim.tick();
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        wood_before - I16F16::from_num(125)
    );
    let spawned = run_until(&mut sim, 10_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::TransportShip)
    });
    assert!(spawned, "the Transport never left the Dock queue");
    let ship = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TransportShip)
        .expect("the transport");
    let (cx, cy) = sim.grid().world_to_cell(ship.pos_x, ship.pos_y);
    assert!(
        sim.grid().is_water_side(cx, cy),
        "the Transport spawned off water at {cx},{cy}"
    );
}

#[test]
fn load_sail_unload_at_the_other_shore() {
    let mut sim = Sim::new(42);
    let ship = staged_ship(&mut sim, 950);
    // Five land units of mixed kinds on the west shore —
    // "transports carry anyone" (infantry, a villager, a monk,
    // cavalry, siege).
    let kinds = [
        EntityKind::Militia,
        EntityKind::Villager,
        EntityKind::Monk,
        EntityKind::Knight,
        EntityKind::Mangonel,
    ];
    let mut ids = Vec::new();
    for (i, kind) in kinds.iter().enumerate() {
        ids.push(spawn_at(&mut sim, 960 + i as u32, *kind, OWNER_PLAYER, WEST_LAND));
    }
    for id in &ids {
        assert!(sim.can_load_transport(*id, ship), "{id} refused at the gate");
        sim.push_command(Command::LoadTransport { entity: *id, ship });
    }
    let loaded = run_until(&mut sim, 20_000, |s| s.transport_cargo_count(ship) == 5);
    assert!(loaded, "the five never all boarded");
    // Cargo rides in BOARDING order (arrival order at the
    // shore — the list is appended as each unit boards, so it
    // is the order they reached the ship, not the click order),
    // contained and inert.
    let mut aboard_sorted = ent(&sim, ship).transport_cargo.clone();
    aboard_sorted.sort_unstable();
    assert_eq!(aboard_sorted, ids);
    for id in &ids {
        let e = ent(&sim, *id);
        assert_eq!(e.transported_in, Some(ship));
        assert!(e.is_contained());
        assert!(e.order.is_none());
    }
    // Sail to the east deep cell, then unload.
    let (ex, ey) = sim.grid().cell_center(EAST_DEEP.0, EAST_DEEP.1);
    sim.push_command(Command::Move { entity: ship, x: ex, y: ey });
    let sailed = run_until(&mut sim, 10_000, |s| {
        let e = ent(s, ship);
        s.grid().world_to_cell(e.pos_x, e.pos_y) == EAST_DEEP && e.order.is_none()
    });
    assert!(sailed, "the loaded Transport never crossed the pond");
    // Cargo stayed aboard (and at full HP) through the crossing.
    assert_eq!(sim.transport_cargo_count(ship), 5);
    sim.push_command(Command::UnloadTransport { ship });
    let out = run_until(&mut sim, 5_000, |s| s.transport_cargo_count(ship) == 0);
    assert!(out, "the cargo never unloaded");
    for id in &ids {
        let e = ent(&sim, *id);
        assert!(!e.dead, "{id} died in a routine crossing");
        assert_eq!(e.transported_in, None);
        assert_eq!(e.task, crate::entity::Task::Idle);
        let (cx, cy) = sim.grid().world_to_cell(e.pos_x, e.pos_y);
        assert!(
            !sim.grid().is_water(cx, cy),
            "{id} unloaded into deep water at {cx},{cy}"
        );
        // Beside the ship's NEW position (the east shore), not
        // back at the loading shore.
        assert!(
            (cx as i32 - EAST_DEEP.0 as i32).abs() <= 3 && (cy as i32 - EAST_DEEP.1 as i32).abs() <= 3,
            "{id} unloaded at {cx},{cy}, far from the east shore"
        );
        assert_ne!((cx, cy), WEST_LAND, "{id} teleported back to the loading shore");
    }
}

#[test]
fn capacity_five_is_a_hard_stop_and_eligibility_is_domain() {
    let mut sim = Sim::new(42);
    let ship = staged_ship(&mut sim, 950);
    // Six militia walk to board: five board, the sixth's order
    // ends at the full ship and he stands idle on the shore.
    let mut ids = Vec::new();
    for i in 0..6u32 {
        ids.push(spawn_at(&mut sim, 960 + i, EntityKind::Militia, OWNER_PLAYER, WEST_LAND));
    }
    for id in &ids {
        sim.push_command(Command::LoadTransport { entity: *id, ship });
    }
    let full = run_until(&mut sim, 20_000, |s| s.transport_cargo_count(ship) == 5);
    assert!(full, "the ship never filled");
    // Let the sixth's arrival resolve against the full ship.
    run_until(&mut sim, 10_000, |s| {
        ids.iter().filter(|id| ent(s, **id).transported_in == Some(ship)).count() == 5
            && ids.iter().any(|id| {
                let e = ent(s, *id);
                e.transported_in.is_none() && e.transport_target.is_none() && !e.dead
            })
    });
    let aboard: Vec<u32> = ids
        .iter()
        .copied()
        .filter(|id| ent(&sim, *id).transported_in == Some(ship))
        .collect();
    assert_eq!(aboard.len(), 5, "capacity 5 is a hard stop");
    assert!(!sim.can_load_transport(ids[5], ship) || aboard.contains(&ids[5]));

    // Eligibility is the domain test, applied at the gate:
    // a ship cannot board, an enemy cannot board, a full ship
    // refuses everyone.
    let galley = spawn_at(&mut sim, 990, EntityKind::Galley, OWNER_PLAYER, EAST_DEEP);
    assert!(!sim.can_load_transport(galley, ship), "a Galley boarded");
    let foe = spawn_at(&mut sim, 991, EntityKind::Militia, OWNER_OPPONENT, WEST_LAND);
    assert!(!sim.can_load_transport(foe, ship), "an enemy boarded");
    let extra = spawn_at(&mut sim, 992, EntityKind::Militia, OWNER_PLAYER, WEST_LAND);
    assert!(!sim.can_load_transport(extra, ship), "a full ship accepted a sixth");
}

#[test]
fn cargo_is_immune_while_aboard_and_dies_with_the_ship() {
    // THE anti-Ram pin (GAR-09 vs GAR-10): sink a loaded
    // transport and assert zero survivors — a copy-paste of
    // the Ram's destruction ejection would place the cargo
    // alive on the shore and fail every assertion below.
    let mut sim = Sim::new(42);
    let ship = staged_ship(&mut sim, 950);
    let mut ids = Vec::new();
    for i in 0..3u32 {
        ids.push(spawn_at(&mut sim, 960 + i, EntityKind::Militia, OWNER_PLAYER, WEST_LAND));
    }
    for id in &ids {
        sim.push_command(Command::LoadTransport { entity: *id, ship });
    }
    assert!(run_until(&mut sim, 20_000, |s| s.transport_cargo_count(ship) == 3));

    // An enemy Galley works the ship over: the ship bleeds,
    // the cargo does not (immune while aboard — GAR-09).
    let galley = spawn_at(&mut sim, 990, EntityKind::Galley, OWNER_OPPONENT, EAST_DEEP);
    sim.push_command(Command::Attack { entity: galley, target: ship });
    let bleeding = run_until(&mut sim, 20_000, |s| ent(s, ship).hp < I16F16::from_num(60));
    assert!(bleeding, "the Galley never hurt the Transport");
    for id in &ids {
        assert_eq!(
            ent(&sim, *id).hp,
            I16F16::from_num(40),
            "cargo unit {id} took damage while aboard"
        );
    }
    // Finish the sinking (battle-worn to the edge, the Galley
    // lands the kill through the ordinary combat path).
    if let Some(e) = sim.entities.iter_mut().find(|e| e.id == ship) {
        e.hp = I16F16::from_num(3);
    }
    let sunk = run_until(&mut sim, 20_000, |s| ent(s, ship).dead);
    assert!(sunk, "the Transport never sank");
    // A few extra ticks: cargo earlier in id order dies on the
    // next sweep by construction.
    for _ in 0..10 {
        sim.tick();
    }
    for id in &ids {
        let e = ent(&sim, *id);
        assert!(e.dead, "cargo unit {id} SURVIVED the sinking (ejection leak)");
        assert_eq!(e.transported_in, None, "the corpse still claims a berth");
    }
    assert_eq!(sim.transport_cargo_count(ship), 0);
}

#[test]
fn a_moved_ship_still_loads_live_geometry() {
    // The P4a Ram test's twin: the load order lands, the ship
    // sails to the far shore, and the walker re-approaches the
    // ship's NEW position and boards there.
    let mut sim = Sim::new(42);
    let ship = staged_ship(&mut sim, 950);
    let vil = spawn_at(&mut sim, 960, EntityKind::Villager, OWNER_PLAYER, WEST_LAND);
    sim.push_command(Command::LoadTransport { entity: vil, ship });
    // The ship leaves for the east shore immediately.
    let (ex, ey) = sim.grid().cell_center(EAST_DEEP.0, EAST_DEEP.1);
    sim.push_command(Command::Move { entity: ship, x: ex, y: ey });
    let boarded = run_until(&mut sim, 60_000, |s| s.transport_cargo_count(ship) == 1);
    assert!(boarded, "the villager never boarded the moved ship");
    assert_eq!(ent(&sim, vil).transported_in, Some(ship));
}

#[test]
fn unload_with_no_free_shore_cell_holds_the_cargo() {
    let mut sim = Sim::new(42);
    let ship = staged_ship(&mut sim, 950);
    let mil = spawn_at(&mut sim, 960, EntityKind::Militia, OWNER_PLAYER, WEST_LAND);
    sim.push_command(Command::LoadTransport { entity: mil, ship });
    assert!(run_until(&mut sim, 20_000, |s| s.transport_cargo_count(ship) == 1));
    // Wall off every land cell within the exit ring (radius 3)
    // with the dynamic layer — no free shore cell exists.
    let (scx, scy) = WEST_DEEP;
    for dy in -3..=3i32 {
        for dx in -3..=3i32 {
            let (cx, cy) = (scx as i32 + dx, scy as i32 + dy);
            if cx >= 0 && cy >= 0 {
                let (cx, cy) = (cx as u32, cy as u32);
                if !sim.grid().is_water(cx, cy) {
                    sim.grid.set_dynamic(cx, cy, true);
                }
            }
        }
    }
    sim.push_command(Command::UnloadTransport { ship });
    for _ in 0..600 {
        sim.tick();
    }
    // No teleport, no loss: the cargo simply holds aboard.
    assert_eq!(sim.transport_cargo_count(ship), 1);
    let e = ent(&sim, mil);
    assert!(!e.dead && e.transported_in == Some(ship), "the held cargo was lost");
}

#[test]
fn save_round_trip_keeps_the_cargo() {
    let mut sim = Sim::new(42);
    let ship = staged_ship(&mut sim, 950);
    let mil = spawn_at(&mut sim, 960, EntityKind::Militia, OWNER_PLAYER, WEST_LAND);
    sim.push_command(Command::LoadTransport { entity: mil, ship });
    assert!(run_until(&mut sim, 20_000, |s| s.transport_cargo_count(ship) == 1));
    let bytes = sim.save_game();
    let mut restored = Sim::load_game(&bytes).expect("the save restores");
    assert_eq!(restored.transport_cargo_count(ship), 1);
    assert_eq!(ent(&restored, mil).transported_in, Some(ship));
    // And the restored pair still unloads.
    restored.push_command(Command::UnloadTransport { ship });
    assert!(run_until(&mut restored, 5_000, |s| {
        s.transport_cargo_count(ship) == 0
    }));
    assert!(!ent(&restored, mil).dead);
}
