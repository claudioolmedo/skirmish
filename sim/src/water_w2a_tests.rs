//! Water W2a tests (ECO-15): fish nodes stock the ponds, the
//! Fishing Ship trains at the Dock, sails the water domain,
//! gathers at the pinned per-(node, gatherer) rates with the
//! pinned carry, deposits at the Dock only, and shelters in the
//! Dock under the age-keyed arrow cap; a villager works shore
//! fish from the bank and is refused deep fish. Mapgen's part is
//! pinned against the regression this package's completion
//! caught: fish on deep water must not fail map validation —
//! with the land-flood clause applied to fish, every pond
//! candidate was rejected and Arabia lost its ponds entirely.

use crate::build::{Building, BuildingKind};
use crate::econ::{NodeKind, ResourceKind};
use crate::entity::{Entity, EntityKind, GarrisonRef, OWNER_PLAYER};
use crate::grid::{Domain, GRID_DIM};
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

fn fish_nodes(sim: &Sim, kind: NodeKind) -> Vec<u32> {
    sim.nodes()
        .iter()
        .filter(|n| n.kind == kind)
        .map(|n| n.id)
        .collect()
}

fn cell_of(sim: &Sim, x: I16F16, y: I16F16) -> (u32, u32) {
    sim.grid().world_to_cell(x, y)
}

/// Scan a fresh sim's map for a 3x3 anchor the Dock predicate
/// accepts (the W1 tests' helper, duplicated so this module
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
/// garrison tests' `placed` pattern; construction itself is
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

fn spawn_ship_at(sim: &mut Sim, id: u32, node: u32) -> u32 {
    let n = sim.nodes().iter().find(|n| n.id == node).expect("node");
    let (x, y) = (n.pos_x, n.pos_y);
    sim.entities.push(Entity::spawned(id, EntityKind::FishingShip, OWNER_PLAYER, x, y));
    id
}

fn food(sim: &Sim) -> I16F16 {
    sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)
}

#[test]
fn fish_stock_ponds_symmetrically_and_dry_seeds_have_none() {
    // Seed 42 (the golden seed) carries a pond: both fish kinds
    // are stocked, at the pinned amounts (200 shore / 250 deep),
    // every fish cell is water-side, and the stocking is
    // mirror-symmetric — for each fish node at (x, y) a node of
    // the same kind stands at the rotational mirror cell.
    let sim = Sim::new(42);
    let shore = fish_nodes(&sim, NodeKind::ShoreFish);
    let deep = fish_nodes(&sim, NodeKind::DeepFish);
    assert!(!shore.is_empty(), "seed 42 stocks shore fish");
    assert!(!deep.is_empty(), "seed 42 stocks deep fish");
    for n in sim.nodes().iter().filter(|n| n.kind.is_fish()) {
        let (cx, cy) = cell_of(&sim, n.pos_x, n.pos_y);
        assert!(
            sim.grid().is_water_side(cx, cy),
            "fish node {} stands off water at {cx},{cy}",
            n.id
        );
        let want = if n.kind == NodeKind::ShoreFish { 200 } else { 250 };
        assert_eq!(n.amount_left, I16F16::from_num(want), "pinned fish amount");
        let (mx, my) = (GRID_DIM - 1 - cx, GRID_DIM - 1 - cy);
        assert!(
            sim.nodes().iter().any(|m| {
                m.kind == n.kind && cell_of(&sim, m.pos_x, m.pos_y) == (mx, my)
            }),
            "fish node {} at {cx},{cy} has no mirror twin",
            n.id
        );
    }
    // A dry seed stocks no fish at all (seed 4 is one of the
    // established dry probe seeds).
    let dry = Sim::new(4);
    assert!(fish_nodes(&dry, NodeKind::ShoreFish).is_empty());
    assert!(fish_nodes(&dry, NodeKind::DeepFish).is_empty());
    // Soak: across pond and dry seeds alike, generated maps
    // keep their ponds (the validation regression this package
    // caught: pond candidates rejected wholesale) — at least
    // one seed in 1..=12 must carry deep water with fish.
    let mut pond_seen = false;
    for seed in 1..=12u64 {
        let s = Sim::new(seed);
        let has_water = (0..GRID_DIM).any(|y| {
            (0..GRID_DIM).any(|x| s.grid().is_water(x, y))
        });
        if has_water {
            pond_seen = true;
            assert!(
                !fish_nodes(&s, NodeKind::ShoreFish).is_empty()
                    || !fish_nodes(&s, NodeKind::DeepFish).is_empty(),
                "seed {seed}: a pond without fish"
            );
        }
    }
    assert!(pond_seen, "no pond in seeds 1..=12 — ponds regressed away");
}

#[test]
fn fishing_rates_carry_and_tech_rows_are_the_de_rows() {
    // Per-(node, gatherer) rates (ECO-15 as amended by update
    // 169123): ship shore 0.24, ship deep 0.42, villager shore
    // 0.43 — raw bits 262 / 459 / 470.
    assert_eq!(
        NodeKind::ShoreFish.gather_rate_for(EntityKind::FishingShip).to_bits(),
        262
    );
    assert_eq!(
        NodeKind::DeepFish.gather_rate_for(EntityKind::FishingShip).to_bits(),
        459
    );
    assert_eq!(
        NodeKind::ShoreFish.gather_rate_for(EntityKind::Villager).to_bits(),
        470
    );
    // The fishing pair patches the ship's rates multiplicatively
    // (TEC-08 stacking, truncating steps): 262 -> 288 -> 316 and
    // 459 -> 504 -> 554; the villager's shore rate is unpatched.
    let lines = TechId::FishingLines.bit();
    let both = lines | TechId::Gillnets.bit();
    assert_eq!(
        crate::tech::patched_gather_rate_for(NodeKind::ShoreFish, EntityKind::FishingShip, lines)
            .to_bits(),
        288
    );
    assert_eq!(
        crate::tech::patched_gather_rate_for(NodeKind::DeepFish, EntityKind::FishingShip, both)
            .to_bits(),
        554
    );
    assert_eq!(
        crate::tech::patched_gather_rate_for(NodeKind::ShoreFish, EntityKind::Villager, both)
            .to_bits(),
        470
    );
    // Carry: 15 base for the ship.
    assert_eq!(crate::econ::FISHING_SHIP_CARRY_CAPACITY, I16F16::from_num(15));
    // Tech rows: hosts, ages, costs, times, the chain.
    assert_eq!(TechId::FishingLines.cost(), (50, 100, 0, 0));
    assert_eq!(TechId::Gillnets.cost(), (150, 200, 0, 0));
    assert_eq!(TechId::FishingLines.age_required(), Age::Feudal);
    assert_eq!(TechId::Gillnets.age_required(), Age::Castle);
    assert_eq!(TechId::Gillnets.requires_tech(), Some(TechId::FishingLines));
    assert!(matches!(
        TechId::FishingLines.host(),
        crate::tech::TechHost::Building(BuildingKind::Dock)
    ));
}

#[test]
fn ship_trains_at_the_dock_spawns_on_water_and_fishes_a_full_cycle() {
    let mut sim = Sim::new(42);
    let dock = placed_dock(&mut sim, 900);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(1000));
    let wood_before = sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood);
    let food_before = food(&sim);
    assert!(sim.can_train_at(dock, EntityKind::FishingShip));
    sim.push_command(Command::TrainAt {
        building: dock,
        kind: EntityKind::FishingShip,
    });
    sim.tick();
    assert_eq!(
        sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood),
        wood_before - I16F16::from_num(75),
        "training charges the pinned 75 wood on enqueue"
    );
    let spawned = run_until(&mut sim, 10_000, |s| {
        s.entities().iter().any(|e| e.kind == EntityKind::FishingShip)
    });
    assert!(spawned, "the Fishing Ship leaves the Dock queue");
    let ship = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::FishingShip)
        .expect("the ship")
        .id;
    // The spawn stands on the water domain, never beached.
    let e = sim.entities().iter().find(|e| e.id == ship).unwrap();
    let (cx, cy) = cell_of(&sim, e.pos_x, e.pos_y);
    assert!(
        sim.grid().is_water_side(cx, cy),
        "the ship spawned off water at {cx},{cy}"
    );
    // Task it on the nearest deep fish and run the full cycle:
    // sail, gather a full 15, return, deposit at the Dock. The
    // ship must stand on a water-side cell on every tick.
    let deep = fish_nodes(&sim, NodeKind::DeepFish)[0];
    sim.push_command(Command::Gather { entity: ship, node: deep });
    let mut deposited = false;
    for _ in 0..200_000 {
        sim.tick();
        let e = sim.entities().iter().find(|e| e.id == ship).unwrap();
        let (cx, cy) = cell_of(&sim, e.pos_x, e.pos_y);
        assert!(
            sim.grid().is_water_side(cx, cy),
            "the ship left the water at {cx},{cy}"
        );
        if food(&sim) >= food_before + I16F16::from_num(15) {
            deposited = true;
            break;
        }
    }
    assert!(deposited, "the ship deposited a full 15-food carry at the Dock");
}

#[test]
fn villager_works_shore_fish_and_the_pairing_gates_refuse() {
    // A villager tasked on shore fish walks to the bank, gathers
    // his 10 and deposits (the food stockpile rises by 10).
    let mut sim = Sim::new(42);
    let villager = a_villager(&sim);
    let shore = fish_nodes(&sim, NodeKind::ShoreFish)[0];
    let food_before = food(&sim);
    sim.push_command(Command::Gather {
        entity: villager,
        node: shore,
    });
    let done = run_until(&mut sim, 300_000, |s| {
        food(s) >= food_before + I16F16::from_num(10)
    });
    assert!(done, "the villager deposited a shore-fish carry");

    // The gates: a villager may not work deep fish, a ship may
    // not work a land node — both orders are refused outright
    // (no carry accrues, the node is untouched).
    let mut sim = Sim::new(42);
    let villager = a_villager(&sim);
    let deep = fish_nodes(&sim, NodeKind::DeepFish)[0];
    let deep_amount = sim
        .nodes()
        .iter()
        .find(|n| n.id == deep)
        .unwrap()
        .amount_left;
    sim.push_command(Command::Gather {
        entity: villager,
        node: deep,
    });
    let ship = spawn_ship_at(&mut sim, 950, deep);
    let tree = sim
        .nodes()
        .iter()
        .find(|n| n.kind == NodeKind::Tree)
        .unwrap()
        .id;
    sim.push_command(Command::Gather {
        entity: ship,
        node: tree,
    });
    for _ in 0..600 {
        sim.tick();
    }
    let v = sim.entities().iter().find(|e| e.id == villager).unwrap();
    assert_eq!(v.carry, I16F16::ZERO, "a villager gathered deep fish");
    let s = sim.entities().iter().find(|e| e.id == ship).unwrap();
    assert_eq!(s.carry, I16F16::ZERO, "a ship gathered a tree");
    assert_eq!(
        sim.nodes().iter().find(|n| n.id == deep).unwrap().amount_left,
        deep_amount,
        "the deep spot was touched by a refused order"
    );
}

#[test]
fn ship_holds_off_land_and_sails_the_domain() {
    let mut sim = Sim::new(42);
    let deep = fish_nodes(&sim, NodeKind::DeepFish)[0];
    let ship = spawn_ship_at(&mut sim, 950, deep);
    // Ordered at the Town Center (deep in land): the water
    // domain refuses the destination and the ship holds on
    // water (MOV-11/MOV-12).
    let tc = sim.town_center();
    let (tx, ty) = (tc.pos_x, tc.pos_y);
    sim.push_command(Command::Move { entity: ship, x: tx, y: ty });
    for _ in 0..3600 {
        sim.tick();
        let e = sim.entities().iter().find(|e| e.id == ship).unwrap();
        let (cx, cy) = cell_of(&sim, e.pos_x, e.pos_y);
        assert!(
            sim.grid().is_water_side(cx, cy),
            "the ship beached at {cx},{cy}"
        );
    }
    // Ordered to another fish spot across the pond: it sails.
    let other = fish_nodes(&sim, NodeKind::DeepFish)
        .into_iter()
        .find(|&id| id != deep)
        .expect("a second deep spot");
    let n = sim.nodes().iter().find(|n| n.id == other).unwrap();
    let (gx, gy) = (n.pos_x, n.pos_y);
    sim.push_command(Command::Move { entity: ship, x: gx, y: gy });
    let arrived = run_until(&mut sim, 60_000, |s| {
        let e = s.entities().iter().find(|e| e.id == ship).unwrap();
        let (cx, cy) = cell_of(s, e.pos_x, e.pos_y);
        let (tx2, ty2) = cell_of(s, gx, gy);
        cx.abs_diff(tx2) <= 1 && cy.abs_diff(ty2) <= 1
    });
    assert!(arrived, "the ship never sailed to the far spot");
    // The domain lookup itself: land kinds read Land, the ship
    // reads Water.
    assert_eq!(EntityKind::FishingShip.domain(), Domain::Water);
    assert_eq!(EntityKind::Villager.domain(), Domain::Land);
}

#[test]
fn dock_shelters_the_ship_and_its_arrows_are_age_keyed() {
    let mut sim = Sim::new(42);
    let dock = placed_dock(&mut sim, 900);
    let deep = fish_nodes(&sim, NodeKind::DeepFish)[0];
    let ship = spawn_ship_at(&mut sim, 950, deep);
    // Eligibility is the data column: the ship may shelter, a
    // villager may not.
    assert!(sim.can_garrison_building(ship, dock));
    let villager = a_villager(&sim);
    assert!(!sim.can_garrison_building(villager, dock));
    sim.push_command(Command::GarrisonBuilding {
        entity: ship,
        building: dock,
    });
    let inside = run_until(&mut sim, 60_000, |s| {
        s.entities()
            .iter()
            .find(|e| e.id == ship)
            .unwrap()
            .garrisoned_in
            == Some(GarrisonRef::Building(dock))
    });
    assert!(inside, "the ship never sheltered in the Dock");
    // Dark Age: the arrow cap is 0, so the sheltered Dock holds
    // no arrows even with a ship inside.
    let b = sim.buildings().iter().find(|b| b.id == dock).unwrap();
    assert_eq!(b.garrison_arrows, 0, "a Dark Age Dock fired");
    assert_eq!(b.garrison.len(), 1);
    // Feudal: the cap steps to 3 — one sheltered ship, one
    // arrow (the age-up recompute the tech completion drives).
    let slot = crate::player_slot(OWNER_PLAYER).expect("player slot");
    sim.players[slot].age = Age::Feudal;
    sim.recompute_dock_arrows_for(OWNER_PLAYER);
    let b = sim.buildings().iter().find(|b| b.id == dock).unwrap();
    assert_eq!(b.garrison_arrows, 1, "one sheltered ship, one Feudal arrow");
    // Ungarrison: the ship exits onto a water-side cell.
    sim.push_command(Command::UngarrisonBuilding { building: dock });
    let out = run_until(&mut sim, 10_000, |s| {
        s.entities()
            .iter()
            .find(|e| e.id == ship)
            .unwrap()
            .garrisoned_in
            .is_none()
    });
    assert!(out, "the ship never left the Dock");
    let e = sim.entities().iter().find(|e| e.id == ship).unwrap();
    let (cx, cy) = cell_of(&sim, e.pos_x, e.pos_y);
    assert!(
        sim.grid().is_water_side(cx, cy),
        "the ship ungarrisoned onto land at {cx},{cy}"
    );
}

#[test]
fn fishing_techs_research_at_the_dock_behind_their_age_gates() {
    let mut sim = Sim::new(42);
    let dock = placed_dock(&mut sim, 900);
    // Dark Age: neither tech is researchable at the Dock.
    assert!(!sim.can_research_at(dock, TechId::FishingLines));
    assert!(!sim.can_research_at(dock, TechId::Gillnets));
    let slot = crate::player_slot(OWNER_PLAYER).expect("player slot");
    sim.players[slot].age = Age::Feudal;
    assert!(sim.can_research_at(dock, TechId::FishingLines));
    assert!(
        !sim.can_research_at(dock, TechId::Gillnets),
        "Gillnets is Castle Age research"
    );
    // Research Fishing Lines for real: 50 food / 100 wood, 20 s.
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(1000));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(1000));
    sim.push_command(Command::ResearchAt {
        building: dock,
        tech: TechId::FishingLines,
    });
    let done = run_until(&mut sim, 10_000, |s| {
        s.has_tech(OWNER_PLAYER, TechId::FishingLines)
    });
    assert!(done, "Fishing Lines never completed at the Dock");
    // Castle Age unlocks Gillnets (its prereq now satisfied).
    sim.players[slot].age = Age::Castle;
    assert!(sim.can_research_at(dock, TechId::Gillnets));
}
