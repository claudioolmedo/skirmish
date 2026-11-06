//! Water W5 tests (TRD-05/07/08 extended): the Trade Cog —
//! its table row (WATER-SPEC §3.2, post-update-169123), Dock
//! training with the home stamp, tasking validity on the Dock
//! depot kind (foreign Docks only; the cross-kind pairings
//! refused), the two-leg loop on the shipped Trade machine
//! crediting the TRD-06 gold only on arrival home, and the
//! University ship pair — Careening (+1 pierce armor to the
//! SHIP class) and Clinker Construction (+10% ship speed) —
//! as TEC-06 lookup patches, with Caravan stacking on the Cog.
//!
//! In-crate staging follows the `trade_tests` fixture style;
//! the sea is painted onto a land seed's grid (terrain is grid
//! state, settable in-crate) so the two Docks stand 41 tiles
//! apart on one water body — a distance no Arabia pond offers.
//! Expectations are the documented DE values (wiki, Ship list
//! fresh crawl 2026-11-05 via WATER-SPEC §3.2; the official
//! update-169123 preview for the two tech costs, fetched
//! 2026-11-06), not recomputed ones — except the route gold,
//! which is TRD-06's own formula evaluated on the staged
//! geometry, as `trade_tests` pins it for the Cart.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
use crate::grid::{TERRAIN_LAND, TERRAIN_SHALLOWS, TERRAIN_WATER};
use crate::tech::{Age, TechId};
use crate::units::spec;
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

fn spawn(sim: &mut Sim, id: u32, kind: EntityKind, owner: u8, cell: (u32, u32)) {
    let (x, y) = sim.grid().cell_center(cell.0, cell.1);
    sim.entities.push(Entity::spawned(id, kind, owner, x, y));
}

/// A complete, idle building record at a cell anchor (the
/// `trade_tests::placed` fixture's shape, completed). Registers
/// the footprint as dynamic, as real placement does — a Dock's
/// water cells must block ships, or the Cog would sail through
/// its own depot.
fn placed(sim: &mut Sim, id: u32, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> Building {
    let size = kind.footprint_size();
    for dy in 0..size {
        for dx in 0..size {
            sim.grid.set_dynamic(cell.0 + dx, cell.1 + dy, true);
        }
    }
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

fn gold(sim: &Sim, owner: u8) -> f64 {
    f(sim.stockpile_of(owner).amount(ResourceKind::Gold))
}

const HOME_DOCK: u32 = 910;
const FOE_DOCK: u32 = 911;
const COG: u32 = 9100;
/// TRD-06 on the staged route: depot centers 41 tiles apart on
/// one axis → inset leaves d = 36 → 0.46 × 36 × (36/120 + 0.3).
const ROUTE_GOLD: f64 = 9.936;

/// The staged sea: a water band across the seed-7 map (x 34..92,
/// y 54..74) with shallows rims on both shores; the player's
/// Dock straddles the west shore at anchor (47, 58) — land cells
/// at x 47–48, the shallows rim cell at x 49 — and the
/// OPPONENT's Dock straddles the east shore at (88, 58). A
/// player Cog floats mid-sea, homed on the west Dock.
fn staged_sea_route() -> Sim {
    let mut sim = Sim::new(7);
    for x in 34..92u32 {
        for y in 54..74u32 {
            // The sea is FLAT: painting terrain alone would
            // leave the land map's height steps under the water,
            // and the MAP-07 cliff rule refuses continuous
            // movement across a height border the A* search
            // sails through — a routed ship would wedge mid-sea
            // against an invisible cliff. Real water is level,
            // so the staging levels it.
            sim.grid.set_height(x, y, 0);
            // And no trees stand in a sea: the seed map's
            // mid-map forest would otherwise flood into node
            // cells the movement integrator's tree slide holds
            // even a ship against (real mapgen never puts a
            // forest on water, so the staged sea clears them).
            sim.grid.set_node_block(x, y, false);
            let tag = if x == 49 || x == 88 {
                TERRAIN_SHALLOWS
            } else if (50..88).contains(&x) {
                TERRAIN_WATER
            } else {
                TERRAIN_LAND
            };
            sim.grid.set_terrain(x, y, tag);
        }
    }
    let home = placed(&mut sim, HOME_DOCK, BuildingKind::Dock, OWNER_PLAYER, (47, 58));
    sim.buildings.push(home);
    let foe = placed(&mut sim, FOE_DOCK, BuildingKind::Dock, OWNER_OPPONENT, (88, 58));
    sim.buildings.push(foe);
    spawn(&mut sim, COG, EntityKind::TradeCog, OWNER_PLAYER, (60, 62));
    let cog = sim.entities.iter_mut().find(|e| e.id == COG).expect("cog staged");
    cog.trade_home = Some(HOME_DOCK);
    sim
}

// ------------------------------------------------------------------
// The table row (TRD-05's sea unit; WATER-SPEC §3.2).

#[test]
fn trade_cog_table_row_matches_the_wiki() {
    let p = spec(EntityKind::TradeCog);
    // Tag 33 is the plan §2 reserved allocation (32 is Water
    // W4's Transport Ship, on its sibling branch off this base).
    assert_eq!(EntityKind::TradeCog.tag(), 33);
    assert_eq!(EntityKind::from_tag(33), Some(EntityKind::TradeCog));
    assert_eq!(EntityKind::TradeCog.train_age(), Age::Feudal);
    assert_eq!(EntityKind::TradeCog.domain(), crate::grid::Domain::Water);
    assert_eq!(p.max_hp, 80);
    assert!(p.attacks.is_empty(), "a trade cog has no attack");
    assert_eq!((p.armor_melee, p.armor_pierce), (0, 6));
    // Update 141935: base speed 1.65 tiles/s (the Cog article's
    // changelog — Caravan was cut to +20% in the same patch).
    assert_eq!(p.speed.to_bits(), 7_209); // 1.65 tiles/s
    assert_eq!(f(p.los), 24.0); // 6 tiles
    assert_eq!((p.food_cost, p.wood_cost, p.gold_cost), (0, 100, 50));
    assert_eq!(p.train_ticks, 2160); // 36 s
    assert!(EntityKind::TradeCog.counts_population());
    assert!(EntityKind::TradeCog.is_trainable());
    assert!(!EntityKind::TradeCog.is_military());
    // One home only: the Dock (TRD-05).
    assert!(BuildingKind::Dock.trains(EntityKind::TradeCog));
    assert!(!BuildingKind::Market.trains(EntityKind::TradeCog));
}

// ------------------------------------------------------------------
// Training at the Dock: exact charges, the home stamp (TRD-05).

#[test]
fn trade_cog_trains_at_the_dock_and_stamps_home() {
    let mut sim = Sim::new(42);
    // A completed Dock on the seed-42 pond shore (the W2a
    // fixture's placement, via this module's `placed`).
    let anchor = (0..=117u32)
        .flat_map(|ay| (0..=117u32).map(move |ax| (ax, ay)))
        .find(|&(ax, ay)| sim.grid().dock_placement_ok(ax, ay, 3))
        .expect("seed 42 pond offers a Dock site");
    let dock = placed(&mut sim, HOME_DOCK, BuildingKind::Dock, OWNER_PLAYER, anchor);
    sim.buildings.push(dock);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(500));

    // Dark Age: the Cog is a Feudal unit even at a standing Dock.
    assert!(!sim.can_train_at(HOME_DOCK, EntityKind::TradeCog));
    sim.players[0].age = Age::Feudal;
    assert!(sim.can_train_at(HOME_DOCK, EntityKind::TradeCog));

    let (wood0, gold0) = (
        f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)),
        gold(&sim, OWNER_PLAYER),
    );
    sim.push_command(Command::TrainAt {
        building: HOME_DOCK,
        kind: EntityKind::TradeCog,
    });
    let born = run_until(&mut sim, 6_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::TradeCog && !e.dead)
    });
    assert!(born, "the Trade Cog never trained");
    // Exact charges: 100 wood, 50 gold.
    assert_eq!(
        wood0 - f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)),
        100.0
    );
    assert_eq!(gold0 - gold(&sim, OWNER_PLAYER), 50.0);
    // The home stamp: the Dock that trained it (TRD-05).
    let cog = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TradeCog)
        .unwrap();
    assert_eq!(cog.trade_home, Some(HOME_DOCK));
    assert_eq!(cog.trade_target, None);
    assert!(!cog.trade_loaded);
    // It spawned on the water side of its Dock, never beached.
    let (cx, cy) = sim.grid().world_to_cell(cog.pos_x, cog.pos_y);
    assert!(
        sim.grid().is_water_side(cx, cy) || sim.grid().is_blocked_for(crate::grid::Domain::Land, cx, cy),
        "the Cog spawned off the water at {cx},{cy}"
    );
}

// ------------------------------------------------------------------
// Tasking validity (TRD-07): any OTHER player's Dock, never own,
// and never across depot kinds.

#[test]
fn cog_tasking_accepts_foreign_docks_only() {
    let mut sim = staged_sea_route();
    // The enemy's Dock is a valid target in 1v1 (TRD-07).
    assert!(sim.can_trade(COG, FOE_DOCK));
    // One's own Dock is not: gold comes from another player's.
    assert!(!sim.can_trade(COG, HOME_DOCK));
    // A villager cannot trade; an unknown depot cannot be tasked.
    let vil = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
        .unwrap()
        .id;
    assert!(!sim.can_trade(vil, FOE_DOCK));
    assert!(!sim.can_trade(COG, 999_999));
    // The cross-kind pairings are refused both ways: a Cog does
    // not trade with Markets, a Cart does not trade with Docks.
    let market = placed(&mut sim, 912, BuildingKind::Market, OWNER_OPPONENT, (20, 20));
    sim.buildings.push(market);
    assert!(!sim.can_trade(COG, 912), "a Cog tasked to a Market");
    spawn(&mut sim, 9101, EntityKind::TradeCart, OWNER_PLAYER, (22, 24));
    assert!(!sim.can_trade(9101, FOE_DOCK), "a Cart tasked to a Dock");
    assert!(sim.can_trade(9101, 912), "the Cart's own pairing stands");
    // The refused command changes nothing.
    sim.push_command(Command::Trade {
        entity: COG,
        building: HOME_DOCK,
    });
    sim.tick();
    assert_eq!(ent(&sim, COG).trade_target, None);
    // An unfinished foreign Dock is not a depot yet.
    let mut sim2 = staged_sea_route();
    sim2.buildings
        .iter_mut()
        .find(|b| b.id == FOE_DOCK)
        .unwrap()
        .complete = false;
    assert!(!sim2.can_trade(COG, FOE_DOCK));
}

// ------------------------------------------------------------------
// The loop (TRD-05/06/07) on water: gold only on arrival home,
// by the formula, repeating; the depot's owner untouched.

#[test]
fn cog_trade_loop_credits_gold_only_on_arrival_home() {
    let mut sim = staged_sea_route();
    let gold0 = gold(&sim, OWNER_PLAYER);
    let foe_gold0 = gold(&sim, OWNER_OPPONENT);
    sim.push_command(Command::Trade {
        entity: COG,
        building: FOE_DOCK,
    });

    // Outbound and at the foreign Dock: no gold exists yet — a
    // trade unit generates nothing one-way (TRD-05).
    let mut saw_loaded = false;
    let mut gold_at_load = f64::NAN;
    let credited = run_until(&mut sim, 60_000, |s| {
        let c = ent(s, COG);
        if c.trade_loaded && !saw_loaded {
            saw_loaded = true;
            gold_at_load = gold(s, OWNER_PLAYER);
        }
        gold(s, OWNER_PLAYER) > gold0 + 1.0
    });
    assert!(credited, "no gold was ever credited");
    assert!(saw_loaded, "the Cog never loaded at the foreign Dock");
    assert_eq!(gold_at_load, gold0, "gold existed before the return home");
    // Exactly the TRD-06 price of the 41-tile route.
    let credited_amount = gold(&sim, OWNER_PLAYER) - gold0;
    assert!(
        (credited_amount - ROUTE_GOLD).abs() < 0.02,
        "credited {credited_amount}, expected ≈ {ROUTE_GOLD}"
    );
    // The depot's owner gained and lost nothing (TRD-07).
    assert_eq!(gold(&sim, OWNER_OPPONENT), foe_gold0);
    // The cycle repeats forever (TRD-05): a second credit lands.
    let gold1 = gold(&sim, OWNER_PLAYER);
    let second = run_until(&mut sim, 60_000, |s| gold(s, OWNER_PLAYER) > gold1 + 1.0);
    assert!(second, "the Cog's route did not loop for a second trip");
    let second_amount = gold(&sim, OWNER_PLAYER) - gold1;
    assert!(
        (second_amount - ROUTE_GOLD).abs() < 0.02,
        "second trip credited {second_amount}, expected ≈ {ROUTE_GOLD}"
    );
    assert_eq!(ent(&sim, COG).trade_target, Some(FOE_DOCK));
    // Stop ends the Cog's route too (the shared machine's rule).
    sim.push_command(Command::Stop { entity: COG });
    sim.tick();
    assert_eq!(ent(&sim, COG).trade_target, None);
    assert!(!ent(&sim, COG).trade_loaded);
}

// ------------------------------------------------------------------
// The foreign Dock falls mid-route: the route is over (the
// shared machine's stranded-unit rule, on the Dock kind).

#[test]
fn cog_route_ends_when_the_foreign_dock_falls() {
    let mut sim = staged_sea_route();
    sim.push_command(Command::Trade {
        entity: COG,
        building: FOE_DOCK,
    });
    for _ in 0..30 {
        sim.tick();
    }
    assert_eq!(ent(&sim, COG).trade_target, Some(FOE_DOCK));
    sim.buildings.retain(|b| b.id != FOE_DOCK);
    sim.tick();
    let c = ent(&sim, COG);
    assert_eq!(c.trade_target, None);
    assert!(!c.trade_loaded);
}

// ------------------------------------------------------------------
// Careening (W5): University host, Castle Age, exact charges,
// +1 pierce armor to the SHIP class at lookup.

#[test]
fn careening_researches_at_the_university_and_armors_ships() {
    // The tech row itself first (W5 Passo 0 pins).
    assert_eq!(TechId::Careening.tag(), 48);
    assert_eq!(TechId::Careening.cost(), (100, 0, 200, 0));
    assert_eq!(
        TechId::Careening.host(),
        crate::tech::TechHost::Building(BuildingKind::University)
    );
    assert_eq!(TechId::Careening.age_required(), Age::Castle);

    let mut sim = Sim::new(7);
    let uni = placed(&mut sim, 920, BuildingKind::University, OWNER_PLAYER, (58, 58));
    sim.buildings.push(uni);
    let dock = placed(&mut sim, 921, BuildingKind::Dock, OWNER_PLAYER, (66, 58));
    sim.buildings.push(dock);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(500));

    // Feudal: Careening is a Castle Age technology — locked.
    sim.players[0].age = Age::Feudal;
    assert_eq!(
        sim.research_block_at(920, TechId::Careening),
        crate::tech::ResearchBlock::Locked
    );
    sim.players[0].age = Age::Castle;
    // The Dock is not its host in current DE — locked there too.
    assert_eq!(
        sim.research_block_at(921, TechId::Careening),
        crate::tech::ResearchBlock::Locked
    );
    assert_eq!(
        sim.research_block_at(920, TechId::Careening),
        crate::tech::ResearchBlock::Ok
    );
    // Before: the base rows (Galley 3, Fishing Ship 1, Cog 6).
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Galley).armor_pierce, 3);
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::FishingShip).armor_pierce,
        1
    );
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::TradeCog).armor_pierce, 6);
    let (food0, gold0) = (
        f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)),
        gold(&sim, OWNER_PLAYER),
    );
    sim.push_command(Command::ResearchAt {
        building: 920,
        tech: TechId::Careening,
    });
    let done = run_until(&mut sim, 5_000, |s| s.has_tech(OWNER_PLAYER, TechId::Careening));
    assert!(done, "Careening never completed");
    // Exact charges: 100 food, 200 gold.
    assert_eq!(
        food0 - f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)),
        100.0
    );
    assert_eq!(gold0 - gold(&sim, OWNER_PLAYER), 200.0);
    // After: every SHIP-class row +1 pierce, nothing else moves.
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::Galley).armor_pierce, 4);
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::FishingShip).armor_pierce,
        2
    );
    assert_eq!(sim.stats_for(OWNER_PLAYER, EntityKind::TradeCog).armor_pierce, 7);
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Galley).armor_melee,
        spec(EntityKind::Galley).armor_melee
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Villager).armor_pierce,
        spec(EntityKind::Villager).armor_pierce
    );
}

// ------------------------------------------------------------------
// Clinker Construction (W5) + Caravan on the Cog (TRD-08): the
// speed patches, keyed and stacked at lookup.

#[test]
fn clinker_and_caravan_speed_the_ships() {
    // The tech row itself first (W5 Passo 0 pins).
    assert_eq!(TechId::ClinkerConstruction.tag(), 49);
    assert_eq!(TechId::ClinkerConstruction.cost(), (150, 100, 0, 0));
    assert_eq!(
        TechId::ClinkerConstruction.host(),
        crate::tech::TechHost::Building(BuildingKind::University)
    );
    assert_eq!(TechId::ClinkerConstruction.age_required(), Age::Castle);

    let mut sim = Sim::new(7);
    let base_ship = spec(EntityKind::FishingShip).speed.to_bits();
    let base_cog = spec(EntityKind::TradeCog).speed.to_bits();
    // Clinker alone: every ship +10% (×11/10, truncating).
    sim.players[0].techs |= TechId::ClinkerConstruction.bit();
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::FishingShip).speed.to_bits(),
        ((base_ship as i64 * 11) / 10) as i32
    );
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::TradeCog).speed.to_bits(),
        ((base_cog as i64 * 11) / 10) as i32
    );
    // A land unit is unmoved by the ship pair.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Villager).speed,
        spec(EntityKind::Villager).speed
    );
    // Caravan alone on the Cog (TRD-08's Cog clause goes live):
    // ×6/5 over the base row — 7209 × 6/5 = 8650 bits.
    sim.players[0].techs = TechId::Caravan.bit();
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::TradeCog).speed.to_bits(),
        8_650
    );
    // The Cart's Caravan row is untouched by the extension.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::TradeCart).speed.to_bits(),
        6_553
    );
    // Both: the layers apply in patch order — Caravan's ×6/5
    // first, then Clinker's ×11/10 over the layered row.
    sim.players[0].techs |= TechId::ClinkerConstruction.bit();
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::TradeCog).speed.to_bits(),
        ((8_650i64 * 11) / 10) as i32
    );
    // Careening and Clinker are independent patches: the armor
    // patch does not ride the speed mask, nor the reverse.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::TradeCog).armor_pierce,
        6
    );
}

