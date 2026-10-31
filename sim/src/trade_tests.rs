//! Package D tests (2026-10-29): the Trade Cart (TRD-05..08) —
//! its table row, Market training with the home stamp, the
//! two-waypoint route machine with the gold credited ONLY on
//! arrival home, by the TRD-06 formula, at a depot owned by
//! another player (TRD-07, diplomacy never consulted), own-Market
//! tasking refused, Stop ending the route, home re-resolution
//! when the stamped Market falls, and the Caravan research
//! (Market host, Castle Age, +20% speed in `stats_for`).
//!
//! In-crate staging (direct entity/building records) follows the
//! `castle_p3_tests` fixture style; expectations are the
//! documented DE values (wiki, "Trade Cart (Age of Empires II)" /
//! "Caravan (Age of Empires II)", fetched 2026-10-29), not
//! recomputed ones.

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{Entity, EntityKind, OWNER_OPPONENT, OWNER_PLAYER};
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
/// `castle_p3_tests::placed` fixture's shape, completed).
fn placed(sim: &Sim, id: u32, kind: BuildingKind, owner: u8, cell: (u32, u32)) -> Building {
    let size = kind.footprint_size();
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
    }
}

fn gold(sim: &Sim, owner: u8) -> f64 {
    f(sim.stockpile_of(owner).amount(ResourceKind::Gold))
}

/// The standard route fixture: the player's Market at cell
/// (40, 60), the OPPONENT's Market 45 tiles east at (85, 60), and
/// a player Trade Cart standing beside its home Market, stamped
/// with it as home. Building centers are then exactly 45 tiles
/// apart on one axis, so TRD-06 prices the round trip at
/// 0.46 × 40 × (40/120 + 0.3) = 11.6533… gold.
const HOME_MARKET: u32 = 900;
const FOE_MARKET: u32 = 901;
const CART: u32 = 9000;
const ROUTE_GOLD: f64 = 11.6533;

fn staged_route() -> Sim {
    let mut sim = Sim::new(7);
    let home = placed(&sim, HOME_MARKET, BuildingKind::Market, OWNER_PLAYER, (40, 60));
    sim.buildings.push(home);
    let foe = placed(&sim, FOE_MARKET, BuildingKind::Market, OWNER_OPPONENT, (85, 60));
    sim.buildings.push(foe);
    spawn(&mut sim, CART, EntityKind::TradeCart, OWNER_PLAYER, (37, 62));
    let cart = sim
        .entities
        .iter_mut()
        .find(|e| e.id == CART)
        .expect("cart staged");
    cart.trade_home = Some(HOME_MARKET);
    sim
}

// ------------------------------------------------------------------
// The table row (TRD-05's unit; wiki values, update 141935).

#[test]
fn trade_cart_table_row_matches_the_wiki() {
    let p = spec(EntityKind::TradeCart);
    assert_eq!(EntityKind::TradeCart.tag(), 16);
    assert_eq!(EntityKind::from_tag(16), Some(EntityKind::TradeCart));
    assert_eq!(EntityKind::TradeCart.train_age(), Age::Feudal);
    assert_eq!(p.max_hp, 70);
    assert!(p.attacks.is_empty(), "a trade cart has no attack");
    assert_eq!((p.armor_melee, p.armor_pierce), (0, 0));
    // Update 141935: base speed 1 → 1.25 tiles/s.
    assert_eq!(p.speed.to_bits(), 5_461); // 1.25 tiles/s
    assert_eq!(f(p.los), 28.0); // 7 tiles
    assert_eq!(f(p.radius), 1.0); // 0.25 tiles
    assert_eq!((p.food_cost, p.wood_cost, p.gold_cost), (0, 100, 50));
    assert_eq!(p.train_ticks, 3060); // 51 s
    assert!(EntityKind::TradeCart.counts_population());
    assert!(EntityKind::TradeCart.is_trainable());
    assert!(!EntityKind::TradeCart.is_military());
    // One home only: the Market (TRD-05).
    assert!(BuildingKind::Market.trains(EntityKind::TradeCart));
    assert!(!BuildingKind::Barracks.trains(EntityKind::TradeCart));
    assert!(!BuildingKind::Castle.trains(EntityKind::TradeCart));
}

// ------------------------------------------------------------------
// Training at the Market: exact charges, the home stamp (TRD-05).

#[test]
fn trade_cart_trains_at_the_market_and_stamps_home() {
    let mut sim = Sim::new(7);
    let market = placed(&sim, HOME_MARKET, BuildingKind::Market, OWNER_PLAYER, (58, 58));
    sim.buildings.push(market);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Wood, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(500));

    // Dark Age: the cart is a Feudal unit even at a standing Market.
    assert!(!sim.can_train_at(HOME_MARKET, EntityKind::TradeCart));
    sim.players[0].age = Age::Feudal;
    assert!(sim.can_train_at(HOME_MARKET, EntityKind::TradeCart));

    let (wood0, gold0) = (
        f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)),
        gold(&sim, OWNER_PLAYER),
    );
    sim.push_command(Command::TrainAt {
        building: HOME_MARKET,
        kind: EntityKind::TradeCart,
    });
    let born = run_until(&mut sim, 6_000, |s| {
        s.entities()
            .iter()
            .any(|e| e.kind == EntityKind::TradeCart && !e.dead)
    });
    assert!(born, "the Trade Cart never trained");
    // Exact charges: 100 wood, 50 gold.
    assert_eq!(
        wood0 - f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Wood)),
        100.0
    );
    assert_eq!(gold0 - gold(&sim, OWNER_PLAYER), 50.0);
    // The home stamp: the Market that trained it (TRD-05).
    let cart = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::TradeCart)
        .unwrap();
    assert_eq!(cart.trade_home, Some(HOME_MARKET));
    assert_eq!(cart.trade_target, None);
    assert!(!cart.trade_loaded);
}

// ------------------------------------------------------------------
// Tasking validity (TRD-07): any OTHER player's depot, never own.

#[test]
fn trade_tasking_accepts_foreign_depots_only() {
    let mut sim = staged_route();
    // The enemy's Market is a valid target in 1v1 (TRD-07).
    assert!(sim.can_trade(CART, FOE_MARKET));
    // One's own Market is not: gold comes from another player's.
    assert!(!sim.can_trade(CART, HOME_MARKET));
    // A villager cannot trade; an unknown depot cannot be tasked.
    let vil = sim
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Villager && e.owner == OWNER_PLAYER)
        .unwrap()
        .id;
    assert!(!sim.can_trade(vil, FOE_MARKET));
    assert!(!sim.can_trade(CART, 999_999));
    // The refused command changes nothing.
    sim.push_command(Command::Trade {
        entity: CART,
        building: HOME_MARKET,
    });
    sim.tick();
    assert_eq!(ent(&sim, CART).trade_target, None);
    // An unfinished foreign Market is not a depot yet.
    let mut sim2 = staged_route();
    sim2.buildings
        .iter_mut()
        .find(|b| b.id == FOE_MARKET)
        .unwrap()
        .complete = false;
    assert!(!sim2.can_trade(CART, FOE_MARKET));
}

// ------------------------------------------------------------------
// The loop (TRD-05/06/07): gold only on arrival home, by the
// formula, repeating; the depot's owner untouched.

#[test]
fn trade_loop_credits_gold_only_on_arrival_home() {
    let mut sim = staged_route();
    let gold0 = gold(&sim, OWNER_PLAYER);
    let foe_gold0 = gold(&sim, OWNER_OPPONENT);
    sim.push_command(Command::Trade {
        entity: CART,
        building: FOE_MARKET,
    });

    // Outbound and at the foreign depot: no gold exists yet — a
    // trade unit generates nothing one-way (TRD-05). The first
    // credit must follow the loaded leg, so watch for the load
    // and confirm the stockpile is still untouched at that tick.
    let mut saw_loaded = false;
    let mut gold_at_load = f64::NAN;
    let credited = run_until(&mut sim, 40_000, |s| {
        let c = ent(s, CART);
        if c.trade_loaded && !saw_loaded {
            saw_loaded = true;
            gold_at_load = gold(s, OWNER_PLAYER);
        }
        gold(s, OWNER_PLAYER) > gold0 + 1.0
    });
    assert!(credited, "no gold was ever credited");
    assert!(saw_loaded, "the cart never loaded at the foreign depot");
    assert_eq!(gold_at_load, gold0, "gold existed before the return home");
    // Exactly the TRD-06 price of the 45-tile route.
    let credited_amount = gold(&sim, OWNER_PLAYER) - gold0;
    assert!(
        (credited_amount - ROUTE_GOLD).abs() < 0.02,
        "credited {credited_amount}, expected ≈ {ROUTE_GOLD}"
    );
    // The depot's owner gained and lost nothing (TRD-07).
    assert_eq!(gold(&sim, OWNER_OPPONENT), foe_gold0);
    // The cycle repeats forever (TRD-05): a second credit lands.
    let gold1 = gold(&sim, OWNER_PLAYER);
    let second = run_until(&mut sim, 40_000, |s| gold(s, OWNER_PLAYER) > gold1 + 1.0);
    assert!(second, "the route did not loop for a second trip");
    let second_amount = gold(&sim, OWNER_PLAYER) - gold1;
    assert!(
        (second_amount - ROUTE_GOLD).abs() < 0.02,
        "second trip credited {second_amount}, expected ≈ {ROUTE_GOLD}"
    );
    assert_eq!(ent(&sim, CART).trade_target, Some(FOE_MARKET));
}

// ------------------------------------------------------------------
// Stop ends the route (TRD-05's "until re-tasked" — SEL-14 form).

#[test]
fn stop_ends_the_trade_route() {
    let mut sim = staged_route();
    sim.push_command(Command::Trade {
        entity: CART,
        building: FOE_MARKET,
    });
    for _ in 0..30 {
        sim.tick();
    }
    assert_eq!(ent(&sim, CART).trade_target, Some(FOE_MARKET));
    sim.push_command(Command::Stop { entity: CART });
    sim.tick();
    let c = ent(&sim, CART);
    assert_eq!(c.trade_target, None);
    assert!(!c.trade_loaded);
    // And a plain Move re-task ends it too.
    sim.push_command(Command::Trade {
        entity: CART,
        building: FOE_MARKET,
    });
    sim.tick();
    assert_eq!(ent(&sim, CART).trade_target, Some(FOE_MARKET));
    let (x, y) = (ent(&sim, CART).pos_x, ent(&sim, CART).pos_y);
    sim.push_command(Command::Move {
        entity: CART,
        x,
        y: y + I16F16::from_num(8),
    });
    sim.tick();
    assert_eq!(ent(&sim, CART).trade_target, None);
}

// ------------------------------------------------------------------
// Home re-resolution: the stamped Market falls, the far one takes
// over, and the credit prices the route actually run (TRD-05/06).

#[test]
fn loaded_cart_reroutes_home_when_its_market_falls() {
    let mut sim = Sim::new(7);
    // Two own Markets west of the foe's: the stamped home at
    // (40, 60) and a farther one at (30, 60).
    let near = placed(&sim, HOME_MARKET, BuildingKind::Market, OWNER_PLAYER, (40, 60));
    sim.buildings.push(near);
    let far = placed(&sim, 902, BuildingKind::Market, OWNER_PLAYER, (30, 60));
    sim.buildings.push(far);
    let foe = placed(&sim, FOE_MARKET, BuildingKind::Market, OWNER_OPPONENT, (85, 60));
    sim.buildings.push(foe);
    // The cart stands loaded at the foreign depot, homed on the
    // near Market.
    spawn(&mut sim, CART, EntityKind::TradeCart, OWNER_PLAYER, (82, 62));
    {
        let cart = sim.entities.iter_mut().find(|e| e.id == CART).unwrap();
        cart.trade_home = Some(HOME_MARKET);
        cart.trade_target = Some(FOE_MARKET);
        cart.trade_loaded = true;
    }
    // The near Market falls before the cart sets out.
    sim.buildings.retain(|b| b.id != HOME_MARKET);
    let gold0 = gold(&sim, OWNER_PLAYER);
    let credited = run_until(&mut sim, 40_000, |s| gold(s, OWNER_PLAYER) > gold0 + 1.0);
    assert!(credited, "the loaded cart never got home");
    // It re-homed on the far Market…
    assert_eq!(ent(&sim, CART).trade_home, Some(902));
    // …and the credit prices the 55-tile route actually run:
    // 0.46 × 50 × (50/120 + 0.3) = 16.4833…
    let amount = gold(&sim, OWNER_PLAYER) - gold0;
    assert!(
        (amount - 16.4833).abs() < 0.02,
        "credited {amount}, expected ≈ 16.4833 (the far route)"
    );
}

// ------------------------------------------------------------------
// Caravan (TRD-08): Market host, Castle Age, +20% at lookup.

#[test]
fn caravan_researches_at_the_market_and_speeds_trade_carts() {
    let mut sim = Sim::new(7);
    let market = placed(&sim, HOME_MARKET, BuildingKind::Market, OWNER_PLAYER, (58, 58));
    sim.buildings.push(market);
    let rax = placed(&sim, 903, BuildingKind::Barracks, OWNER_PLAYER, (66, 58));
    sim.buildings.push(rax);
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Food, I16F16::from_num(500));
    sim.stockpile_mut(OWNER_PLAYER)
        .add(ResourceKind::Gold, I16F16::from_num(500));

    // Feudal: Caravan is a Castle Age technology — locked.
    sim.players[0].age = Age::Feudal;
    assert_eq!(
        sim.research_block_at(HOME_MARKET, TechId::Caravan),
        crate::tech::ResearchBlock::Locked
    );
    sim.players[0].age = Age::Castle;
    // The Barracks is not its host — locked there too.
    assert_eq!(
        sim.research_block_at(903, TechId::Caravan),
        crate::tech::ResearchBlock::Locked
    );
    assert_eq!(
        sim.research_block_at(HOME_MARKET, TechId::Caravan),
        crate::tech::ResearchBlock::Ok
    );
    // Before: the base row, 1.25 tiles/s = 5461 bits.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::TradeCart).speed.to_bits(),
        5_461
    );
    let (food0, gold0) = (
        f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)),
        gold(&sim, OWNER_PLAYER),
    );
    sim.push_command(Command::ResearchAt {
        building: HOME_MARKET,
        tech: TechId::Caravan,
    });
    let done = run_until(&mut sim, 5_000, |s| s.has_tech(OWNER_PLAYER, TechId::Caravan));
    assert!(done, "Caravan never completed");
    // Exact charges: 200 food, 200 gold.
    assert_eq!(
        food0 - f(sim.stockpile_of(OWNER_PLAYER).amount(ResourceKind::Food)),
        200.0
    );
    assert_eq!(gold0 - gold(&sim, OWNER_PLAYER), 200.0);
    // After: +20% — 5461 × 6/5 = 6553 bits (1.5 tiles/s), the
    // pre-update-141935 upgraded speed exactly (TRD-08).
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::TradeCart).speed.to_bits(),
        6_553
    );
    // The patch is the trade kinds' alone: a villager is unmoved.
    assert_eq!(
        sim.stats_for(OWNER_PLAYER, EntityKind::Villager).speed,
        spec(EntityKind::Villager).speed
    );
}
