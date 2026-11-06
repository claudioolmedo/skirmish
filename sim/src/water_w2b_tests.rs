//! Water W2b tests (ECO-15's Fish Trap clause): the building
//! table rows are the SPEC's, a Fishing Ship — the first
//! non-villager builder — raises a trap on open water, the
//! completed trap stamps a 715-food node the builder starts
//! working at the pinned 0.35/s, exactly one ship works a
//! trap (a second waits), the pairing gates refuse villagers,
//! land kinds and foreign ships, and a depleted trap retires
//! or replants through the Dock's reseed desk (paid queue,
//! then the auto toggle's on-the-spot charge — the ECO-14
//! desk's mirror at 100 wood).

use crate::build::{Building, BuildingKind, Placement};
use crate::econ::{NodeKind, ResourceKind};
use crate::entity::{Entity, EntityKind, Task, OWNER_OPPONENT, OWNER_PLAYER};
use crate::grid::GRID_DIM;
use crate::tech::{Age, TechId};
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

fn a_villager(sim: &Sim) -> u32 {
    sim.entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER && !e.dead)
        .map(|e| e.id)
        .expect("a starting villager")
}

/// Scan a fresh sim's map for a 3x3 anchor the Dock predicate
/// accepts (the W1/W2a tests' helper, duplicated so this
/// module stands alone).
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
/// W2a tests' `placed_dock` pattern; construction itself is
/// W1's proven path). Registers the footprint as dynamic, as
/// real placement does.
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

/// A water cell a trap may be founded on right now: the grid
/// predicate holds, no live node sits on it, and no standing
/// building covers it.
fn trap_cell(sim: &Sim) -> Option<(u32, u32)> {
    let grid = sim.grid();
    for ay in 0..GRID_DIM {
        for ax in 0..GRID_DIM {
            if !grid.fish_trap_placement_ok(ax, ay) {
                continue;
            }
            let taken = sim
                .nodes()
                .iter()
                .any(|n| !n.is_depleted() && grid.world_to_cell(n.pos_x, n.pos_y) == (ax, ay))
                || sim
                    .buildings()
                    .iter()
                    .any(|b| b.footprint_cells().contains(&(ax, ay)));
            if !taken {
                return Some((ax, ay));
            }
        }
    }
    None
}

fn spawn_ship(sim: &mut Sim, id: u32, owner: u8, x: I16F16, y: I16F16) -> u32 {
    sim.entities.push(Entity::spawned(id, EntityKind::FishingShip, owner, x, y));
    id
}

fn feudal(sim: &mut Sim) {
    let slot = crate::player_slot(OWNER_PLAYER).expect("player slot");
    sim.players[slot].age = Age::Feudal;
}

fn wood(sim: &Sim) -> I16F16 {
    sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)
}

fn food(sim: &Sim) -> I16F16 {
    sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)
}

fn give_wood(sim: &mut Sim, n: i32) {
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(n));
}

fn trap_building(sim: &Sim) -> Option<(u32, bool, Option<u32>)> {
    sim.buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::FishTrap)
        .map(|b| (b.id, b.complete, b.node_id))
}

fn task_of(sim: &Sim, id: u32) -> Task {
    sim.entities()
        .iter()
        .find(|e| e.id == id)
        .map(|e| e.task)
        .expect("entity")
}

/// A Feudal sim with a Dock on the pond, one ship standing on
/// a legal trap cell, and wood to build with. Returns
/// (sim, dock, ship, trap cell).
fn trap_stage() -> (Sim, u32, u32, (u32, u32)) {
    let mut sim = Sim::new(42);
    feudal(&mut sim);
    let dock = placed_dock(&mut sim, 900);
    let cell = trap_cell(&sim).expect("seed pond offers a trap cell");
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    let ship = spawn_ship(&mut sim, 950, OWNER_PLAYER, x, y);
    give_wood(&mut sim, 1000);
    (sim, dock, ship, cell)
}

/// Build the trap with the staged ship and run until it
/// completes; returns the stamped node's id.
fn build_the_trap(sim: &mut Sim, ship: u32, cell: (u32, u32)) -> u32 {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    let wood_before = wood(sim);
    sim.push_command(Command::Build {
        entity: ship,
        kind: BuildingKind::FishTrap,
        x,
        y,
    });
    sim.tick();
    assert_eq!(
        wood(sim),
        wood_before - I16F16::from_num(100),
        "placement charges the pinned 100 wood"
    );
    let done = run_until(sim, 20_000, |s| {
        matches!(trap_building(s), Some((_, true, Some(_))))
    });
    assert!(done, "the ship raises the trap to completion");
    trap_building(sim).expect("trap").2.expect("stamped node")
}

#[test]
fn fish_trap_tables_are_the_spec_rows() {
    // BuildingKind tag 24, both directions (append-only ABI).
    assert_eq!(BuildingKind::FishTrap.tag(), 24);
    assert_eq!(BuildingKind::from_tag(24), Some(BuildingKind::FishTrap));
    // The kind's row: 100 wood, 40 s, 1×1, Feudal-born, floats
    // on water, never blocks its cell (its worker stands on
    // it), sight 4 (the generic building row).
    let k = BuildingKind::FishTrap;
    assert_eq!(k.wood_cost(), I16F16::from_num(100));
    assert_eq!(k.stone_cost(), I16F16::ZERO);
    assert_eq!(k.gold_cost(), I16F16::ZERO);
    assert_eq!(k.build_ticks(), 2400);
    assert_eq!(k.footprint_size(), 1);
    assert_eq!(k.age_required(), Age::Feudal);
    assert_eq!(k.placement(), Placement::Water);
    assert!(!k.blocks_footprint());
    // HP/armor are the ADOPTED Farm row (480, 0/0) — pinned
    // here as an implementation constant, not DE data (the
    // dossier leaves them UNVERIFIED; see `FISH_TRAP_HP`).
    assert_eq!(k.max_hp(), 480);
    assert_eq!(k.armor_melee(), 0);
    assert_eq!(k.armor_pierce(), 0);
    // NodeKind tag 9, both directions; 715 food; the trap is
    // not a fish spot (the retarget family never chains onto
    // it) but is the trap family of one.
    assert_eq!(NodeKind::FishTrap.tag(), 9);
    assert_eq!(NodeKind::FishTrap.resource(), ResourceKind::Food);
    assert_eq!(NodeKind::FishTrap.full_amount(), I16F16::from_num(715));
    assert!(!NodeKind::FishTrap.is_fish());
    assert!(NodeKind::FishTrap.is_trap());
    // Gather rate: 0.35/s for the ship = 383 bits; the fishing
    // pair patches it multiplicatively like the fish rows
    // (383 -> 421 -> 463); a villager's (unreachable) row takes
    // no patch — the pair scopes to Fishing Ships.
    assert_eq!(NodeKind::FishTrap.gather_rate().to_bits(), 383);
    assert_eq!(
        NodeKind::FishTrap.gather_rate_for(EntityKind::FishingShip).to_bits(),
        383
    );
    let lines = TechId::FishingLines.bit();
    let both = lines | TechId::Gillnets.bit();
    assert_eq!(
        crate::tech::patched_gather_rate_for(NodeKind::FishTrap, EntityKind::FishingShip, lines)
            .to_bits(),
        421
    );
    assert_eq!(
        crate::tech::patched_gather_rate_for(NodeKind::FishTrap, EntityKind::FishingShip, both)
            .to_bits(),
        463
    );
    assert_eq!(
        crate::tech::patched_gather_rate_for(NodeKind::FishTrap, EntityKind::Villager, both)
            .to_bits(),
        383
    );
    // The desk's price: a reseed costs what a trap costs.
    assert_eq!(crate::TRAP_RESEED_WOOD_COST, 100);
}

#[test]
fn ship_builds_a_trap_and_works_it_end_to_end() {
    let (mut sim, _dock, ship, cell) = trap_stage();
    let food_before = food(&sim);
    let node = build_the_trap(&mut sim, ship, cell);
    // The stamp: a FishTrap node at the building's cell with
    // the full 715 aboard.
    let n = sim.nodes().iter().find(|n| n.id == node).expect("node");
    assert_eq!(n.kind, NodeKind::FishTrap);
    assert_eq!(n.amount_left, I16F16::from_num(715));
    assert_eq!(n.full_amount, I16F16::from_num(715));
    assert_eq!(
        sim.grid().world_to_cell(n.pos_x, n.pos_y),
        cell,
        "the node sits on the trap's own cell"
    );
    // The builder starts working its own trap...
    let working = run_until(&mut sim, 5_000, |s| task_of(s, ship) == Task::Gathering);
    assert!(working, "the builder ship starts gathering from its trap");
    // ...at the pinned rate: over 600 gathering ticks the node
    // drains by the ship's patched rate (383 bits/tick, no
    // fishing techs in this sim) — sampled while the ship is
    // mid-load (a full 15 takes ~2350 ticks, so no deposit
    // interrupts the window).
    let before = sim
        .nodes()
        .iter()
        .find(|n| n.id == node)
        .unwrap()
        .amount_left;
    for _ in 0..600 {
        sim.tick();
    }
    let after = sim
        .nodes()
        .iter()
        .find(|n| n.id == node)
        .unwrap()
        .amount_left;
    let drained = (before - after).to_bits();
    assert!(
        (383 * 600 - 383..=383 * 600 + 383).contains(&drained),
        "the trap drains at the ship's 0.35/s row (drained {drained} bits in 600 ticks)"
    );
    // The full cycle closes: the ship fills its 15, sails to
    // the Dock and deposits (the food stockpile rises by 15).
    let deposited = run_until(&mut sim, 30_000, |s| {
        food(s) >= food_before + I16F16::from_num(15)
    });
    assert!(deposited, "the ship deposits a full 15-food carry at the Dock");
}

#[test]
fn pairing_gates_refuse_the_wrong_builders_and_foreign_ships() {
    let (mut sim, _dock, ship, cell) = trap_stage();
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    // A villager may not build a Fish Trap (ship-only path).
    let villager = a_villager(&sim);
    let wood_before = wood(&sim);
    sim.push_command(Command::Build {
        entity: villager,
        kind: BuildingKind::FishTrap,
        x,
        y,
    });
    sim.tick();
    assert!(trap_building(&sim).is_none(), "no trap from a villager's order");
    assert_eq!(wood(&sim), wood_before, "a refused order charges nothing");
    // A Fishing Ship may build nothing else — not even a Farm
    // (its land analog) or a House on the shore.
    sim.push_command(Command::Build {
        entity: ship,
        kind: BuildingKind::Farm,
        x,
        y,
    });
    sim.push_command(Command::Build {
        entity: ship,
        kind: BuildingKind::House,
        x,
        y,
    });
    sim.tick();
    assert!(
        sim.buildings().iter().all(|b| b.kind != BuildingKind::Farm),
        "no farm from a ship's order"
    );
    assert!(
        sim.buildings().iter().all(|b| b.kind != BuildingKind::House),
        "no house from a ship's order"
    );
    // Dark Age refuses the trap outright (Feudal-born kind):
    // a fresh Dark sim's ship order founds nothing.
    let mut dark = Sim::new(42);
    let dcell = trap_cell(&dark).expect("trap cell");
    let (dx, dy) = dark.grid().cell_center(dcell.0, dcell.1);
    let dship = spawn_ship(&mut dark, 951, OWNER_PLAYER, dx, dy);
    give_wood(&mut dark, 1000);
    dark.push_command(Command::Build {
        entity: dship,
        kind: BuildingKind::FishTrap,
        x: dx,
        y: dy,
    });
    dark.tick();
    assert!(trap_building(&dark).is_none(), "no trap before the Feudal Age");
    // A foreign ship may not work the trap: build it, then
    // order an opponent's ship onto the stamped node — the
    // order is refused whole and the stock does not move.
    let node = build_the_trap(&mut sim, ship, cell);
    let foe = spawn_ship(&mut sim, 952, OWNER_OPPONENT, x, y);
    sim.push_command(Command::Gather {
        entity: foe,
        node,
    });
    for _ in 0..600 {
        sim.tick();
    }
    assert_ne!(task_of(&sim, foe), Task::Gathering, "the foreign ship never works the trap");
    assert_ne!(task_of(&sim, foe), Task::ToNode, "the foreign ship is never even tasked");
    // A villager is refused the trap node too.
    sim.push_command(Command::Gather {
        entity: villager,
        node,
    });
    sim.tick();
    assert_ne!(task_of(&sim, villager), Task::ToNode, "no villager is tasked to a trap");
}

#[test]
fn trap_placement_refuses_land_fish_cells_and_dock_cells() {
    let (mut sim, _dock, ship, _cell) = trap_stage();
    let wood_before = wood(&sim);
    // Land: any dry cell refuses (the cell under the player's
    // Town Center is dry land).
    let tc = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TownCenter && e.owner == OWNER_PLAYER)
        .expect("tc");
    sim.push_command(Command::Build {
        entity: ship,
        kind: BuildingKind::FishTrap,
        x: tc.pos_x,
        y: tc.pos_y,
    });
    sim.tick();
    assert!(trap_building(&sim).is_none(), "no trap on land");
    // A fish spot's cell refuses: a trap never swallows a live
    // node (shore and deep alike).
    for kind in [NodeKind::ShoreFish, NodeKind::DeepFish] {
        let spot = sim
            .nodes()
            .iter()
            .find(|n| n.kind == kind)
            .unwrap_or_else(|| panic!("seed pond stocks {kind:?}"));
        sim.push_command(Command::Build {
            entity: ship,
            kind: BuildingKind::FishTrap,
            x: spot.pos_x,
            y: spot.pos_y,
        });
        sim.tick();
        assert!(trap_building(&sim).is_none(), "no trap over a {kind:?} spot");
    }
    // A Dock's water cell refuses (the footprint is occupied
    // ground — the dynamic layer bars it).
    let dock = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Dock)
        .expect("dock")
        .clone();
    let (ax, ay) = (dock.cell_x, dock.cell_y);
    let mut dock_water = None;
    for dy in 0..3u32 {
        for dx in 0..3u32 {
            if sim.grid().is_water_side(ax + dx, ay + dy) {
                dock_water = Some((ax + dx, ay + dy));
            }
        }
    }
    let (wx, wy) = dock_water.expect("the Dock straddles water");
    let (px, py) = sim.grid().cell_center(wx, wy);
    sim.push_command(Command::Build {
        entity: ship,
        kind: BuildingKind::FishTrap,
        x: px,
        y: py,
    });
    sim.tick();
    assert!(trap_building(&sim).is_none(), "no trap on a Dock cell");
    assert_eq!(wood(&sim), wood_before, "every refusal left the wood untouched");
}

#[test]
fn one_ship_works_a_trap_and_the_second_waits() {
    let (mut sim, _dock, ship, cell) = trap_stage();
    let node = build_the_trap(&mut sim, ship, cell);
    // A second own ship, spawned beside the trap, is ordered
    // onto the same node.
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    let second = spawn_ship(&mut sim, 953, OWNER_PLAYER, x + I16F16::from_num(4), y);
    sim.push_command(Command::Gather {
        entity: second,
        node,
    });
    // While the first ship gathers, the second must never be
    // in Gathering — it waits in the travel phase at the trap.
    let first_working = run_until(&mut sim, 5_000, |s| task_of(s, ship) == Task::Gathering);
    assert!(first_working);
    for _ in 0..900 {
        sim.tick();
        assert_ne!(
            task_of(&sim, second),
            Task::Gathering,
            "two ships never work one trap at once"
        );
    }
    // When the worker fills up and sails off to deposit, the
    // waiter takes the trap.
    let took_over = run_until(&mut sim, 30_000, |s| task_of(s, second) == Task::Gathering);
    assert!(took_over, "the waiting ship works the trap once it frees up");
}

#[test]
fn depleted_trap_retires_without_a_desk() {
    let (mut sim, _dock, ship, cell) = trap_stage();
    let node = build_the_trap(&mut sim, ship, cell);
    // Drain the trap to a sip and let the worker finish it.
    let ni = sim.nodes().iter().position(|n| n.id == node).expect("node");
    sim.nodes[ni].amount_left = I16F16::from_num(1);
    let gone = run_until(&mut sim, 30_000, |s| trap_building(s).is_none());
    assert!(gone, "with no queue and no auto-reseed the depleted trap retires");
    // The dead node stays in the list, inert (the farm rule).
    let n = sim.nodes().iter().find(|n| n.id == node).expect("node record");
    assert!(n.is_depleted());
}

#[test]
fn paid_queue_replants_the_trap_and_dequeue_refunds() {
    let (mut sim, dock, ship, cell) = trap_stage();
    let node = build_the_trap(&mut sim, ship, cell);
    // Buy one reseed ahead at the Dock: 100 wood now, queue 1.
    let wood_before = wood(&sim);
    sim.push_command(Command::QueueTrapReseed { building: dock });
    sim.tick();
    assert_eq!(sim.trap_reseed_queue_of(OWNER_PLAYER), 1);
    assert_eq!(wood(&sim), wood_before - I16F16::from_num(100));
    // Dequeue refunds it whole.
    sim.push_command(Command::DequeueTrapReseed { building: dock });
    sim.tick();
    assert_eq!(sim.trap_reseed_queue_of(OWNER_PLAYER), 0);
    assert_eq!(wood(&sim), wood_before, "the dequeue refunds the 100 wood");
    // Buy it again and deplete the trap: the queue feeds the
    // replant — the trap returns as a foundation, the queue is
    // consumed, and the ship that worked it rebuilds it.
    sim.push_command(Command::QueueTrapReseed { building: dock });
    sim.tick();
    assert_eq!(sim.trap_reseed_queue_of(OWNER_PLAYER), 1);
    let ni = sim.nodes().iter().position(|n| n.id == node).expect("node");
    sim.nodes[ni].amount_left = I16F16::from_num(1);
    let foundation = run_until(&mut sim, 30_000, |s| {
        matches!(trap_building(s), Some((_, false, _)))
    });
    assert!(foundation, "the paid reseed returns the trap as a foundation");
    assert_eq!(sim.trap_reseed_queue_of(OWNER_PLAYER), 0, "the reseed was consumed");
    // The replant completes and stamps a fresh 715.
    let done = run_until(&mut sim, 30_000, |s| {
        matches!(trap_building(s), Some((_, true, Some(_))))
    });
    assert!(done, "the replanted trap completes");
    let (_, _, new_node) = trap_building(&sim).expect("trap");
    let n = sim
        .nodes()
        .iter()
        .find(|n| n.id == new_node.expect("node"))
        .expect("fresh node");
    assert_eq!(n.amount_left, I16F16::from_num(715), "a replanted trap stocks the full 715");
}

#[test]
fn auto_reseed_charges_on_depletion_and_retires_when_broke() {
    let (mut sim, dock, ship, cell) = trap_stage();
    let node = build_the_trap(&mut sim, ship, cell);
    sim.push_command(Command::SetAutoTrapReseed {
        building: dock,
        on: true,
    });
    sim.tick();
    assert!(sim.auto_trap_reseed_of(OWNER_PLAYER));
    // Spend the stockpile down to exactly the reseed price,
    // then deplete: the toggle charges 100 on the spot and the
    // trap replants.
    let excess = wood(&sim) - I16F16::from_num(100);
    sim.stockpile_mut(OWNER_PLAYER).spend(ResourceKind::Wood, excess);
    assert_eq!(wood(&sim), I16F16::from_num(100));
    let ni = sim.nodes().iter().position(|n| n.id == node).expect("node");
    sim.nodes[ni].amount_left = I16F16::from_num(1);
    let foundation = run_until(&mut sim, 30_000, |s| {
        matches!(trap_building(s), Some((_, false, _)))
    });
    assert!(foundation, "auto-reseed replants the depleted trap");
    assert_eq!(wood(&sim), I16F16::ZERO, "the 100 wood was charged at depletion");
    // The replant completes; deplete it again with an empty
    // stockpile and the toggle cannot conjure wood — it retires.
    let done = run_until(&mut sim, 30_000, |s| {
        matches!(trap_building(s), Some((_, true, Some(new))) if new != node)
    });
    assert!(done, "the auto-replanted trap completes");
    let (_, _, new_node) = trap_building(&sim).expect("trap");
    let new_node = new_node.expect("node");
    let ni = sim.nodes().iter().position(|n| n.id == new_node).expect("node");
    sim.nodes[ni].amount_left = I16F16::from_num(1);
    let gone = run_until(&mut sim, 30_000, |s| trap_building(s).is_none());
    assert!(gone, "auto-reseed without the wood retires the trap");
}

#[test]
fn dock_desk_commands_gate_on_a_completed_dock() {
    let (mut sim, dock, ship, cell) = trap_stage();
    let _ = build_the_trap(&mut sim, ship, cell);
    let trap_id = trap_building(&sim).expect("trap").0;
    let wood_before = wood(&sim);
    // Naming the trap itself (not a Dock) refuses the whole desk.
    sim.push_command(Command::QueueTrapReseed { building: trap_id });
    sim.push_command(Command::SetAutoTrapReseed {
        building: trap_id,
        on: true,
    });
    sim.tick();
    assert_eq!(sim.trap_reseed_queue_of(OWNER_PLAYER), 0);
    assert!(!sim.auto_trap_reseed_of(OWNER_PLAYER));
    assert_eq!(wood(&sim), wood_before);
    // Naming nothing real refuses too.
    sim.push_command(Command::QueueTrapReseed { building: 424_242 });
    sim.tick();
    assert_eq!(sim.trap_reseed_queue_of(OWNER_PLAYER), 0);
    // An unfinished Dock is no desk either: found a real one
    // and name it before it completes.
    let villager = a_villager(&sim);
    let (ax, ay) = dock_anchor(&sim).expect("a second Dock site");
    let (px, py) = sim.grid().cell_center(ax, ay);
    sim.push_command(Command::Build {
        entity: villager,
        kind: BuildingKind::Dock,
        x: px,
        y: py,
    });
    sim.tick();
    let unfinished = sim
        .buildings()
        .iter()
        .find(|b| b.kind == BuildingKind::Dock && !b.complete)
        .map(|b| b.id);
    if let Some(uid) = unfinished {
        sim.push_command(Command::QueueTrapReseed { building: uid });
        sim.tick();
        assert_eq!(
            sim.trap_reseed_queue_of(OWNER_PLAYER),
            0,
            "an unfinished Dock sells no reseeds"
        );
    }
    // Insufficient wood refuses at the real Dock.
    let all = wood(&sim);
    sim.stockpile_mut(OWNER_PLAYER).spend(ResourceKind::Wood, all);
    sim.push_command(Command::QueueTrapReseed { building: dock });
    sim.tick();
    assert_eq!(sim.trap_reseed_queue_of(OWNER_PLAYER), 0, "no wood, no reseed");
    // Dequeue on an empty queue is a no-op (no phantom refund).
    sim.push_command(Command::DequeueTrapReseed { building: dock });
    sim.tick();
    assert_eq!(wood(&sim), I16F16::ZERO, "an empty dequeue refunds nothing");
}

#[test]
fn trap_state_round_trips_through_a_save() {
    let (mut sim, dock, ship, cell) = trap_stage();
    let _ = build_the_trap(&mut sim, ship, cell);
    sim.push_command(Command::QueueTrapReseed { building: dock });
    sim.push_command(Command::QueueTrapReseed { building: dock });
    sim.push_command(Command::SetAutoTrapReseed {
        building: dock,
        on: true,
    });
    sim.tick();
    assert_eq!(sim.trap_reseed_queue_of(OWNER_PLAYER), 2);
    let bytes = sim.save_game();
    let back = Sim::load_game(&bytes).expect("the save loads");
    assert_eq!(back.state_hash(), sim.state_hash(), "the restored state hashes identically");
    assert_eq!(back.trap_reseed_queue_of(OWNER_PLAYER), 2);
    assert!(back.auto_trap_reseed_of(OWNER_PLAYER));
    assert!(
        back.buildings().iter().any(|b| b.kind == BuildingKind::FishTrap && b.complete),
        "the trap itself survives the round-trip"
    );
    assert!(
        back.nodes().iter().any(|n| n.kind == NodeKind::FishTrap),
        "the trap node survives the round-trip"
    );
}
