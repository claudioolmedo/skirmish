//! Market tests (TRD-01..03): the building table row, atomic
//! buy/sell against the global rates, the rejection discipline,
//! the floor in-sim, and lockstep determinism — two sims fed an
//! identical trade stream (both seats trading) must agree on the
//! hash every tick, and a trade-free stream must hash exactly like
//! a sim that never trades (the conditional hash write).

use crate::build::{Building, BuildingKind};
use crate::econ::ResourceKind;
use crate::entity::{OWNER_OPPONENT, OWNER_PLAYER};
use crate::tech::Age;
use crate::{Command, Sim};
use fixed::types::I16F16;

fn n(v: I16F16) -> i32 {
    v.to_num::<i32>()
}

/// A Market building record at a cell anchor (the placed fixture's
/// shape), complete or not as asked.
fn market_at(sim: &Sim, id: u32, owner: u8, cell: (u32, u32), complete: bool) -> Building {
    let kind = BuildingKind::Market;
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
        progress_thirds: if complete { kind.build_thirds() } else { 0 },
        repair_hp_remainder: 0,
        repair_cost_remainder: 0,
        repair_stone_remainder: 0,
        complete,
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

fn sim_with_market() -> Sim {
    let mut sim = Sim::new(7);
    let m = market_at(&sim, 900, OWNER_PLAYER, (58, 58), true);
    sim.buildings.push(m);
    sim
}

#[test]
fn market_building_table_is_the_wiki_row() {
    let k = BuildingKind::Market;
    // AoE wiki, Market infobox (fetched 2026-10-22): Feudal Age,
    // requires a Mill, 175 wood, 4×4, 60 s build, 1800 HP and
    // 1/8 armor in the Feudal column.
    assert_eq!(k.tag(), 12);
    assert_eq!(k.wood_cost(), I16F16::from_num(175));
    assert_eq!(k.build_ticks(), 3600);
    assert_eq!(k.footprint_size(), 4);
    assert_eq!(k.age_required(), Age::Feudal);
    assert_eq!(k.requires_building(), Some(BuildingKind::Mill));
    assert_eq!(k.max_hp(), 1800);
    assert_eq!(k.armor_melee(), 1);
    assert_eq!(k.armor_pierce(), 8);
    assert_eq!(k.build_range(), I16F16::from_num(15.5));
    // A Feudal building never gates the Feudal research itself.
    assert!(!k.counts_toward_feudal());
}

#[test]
fn buy_pays_gold_adds_the_lot_and_raises_the_global_rate() {
    let mut sim = sim_with_market();
    sim.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(1000));
    assert!(sim.can_market_buy(900, ResourceKind::Wood));
    sim.push_command(Command::MarketBuy {
        building: 900,
        resource: ResourceKind::Wood,
    });
    sim.tick();
    // Start prices: wood buys at 130. 200 W + 100, 1100 G − 130.
    assert_eq!(n(sim.stockpile().amount(ResourceKind::Wood)), 300);
    assert_eq!(n(sim.stockpile().amount(ResourceKind::Gold)), 970);
    // The rate rose by 2 for everyone; the others stand.
    assert_eq!(sim.market_rates(), [115, 102, 130]);
    assert_eq!(sim.market_buy_price(ResourceKind::Wood), 133);
    assert_eq!(sim.market_sell_price(ResourceKind::Wood), 71);
}

#[test]
fn sell_takes_the_lot_pays_gold_and_lowers_the_global_rate() {
    let mut sim = sim_with_market();
    assert!(sim.can_market_sell(900, ResourceKind::Food));
    sim.push_command(Command::MarketSell {
        building: 900,
        resource: ResourceKind::Food,
    });
    sim.tick();
    // Food sells at 81 at the start: 200 F − 100, 100 G + 81.
    assert_eq!(n(sim.stockpile().amount(ResourceKind::Food)), 100);
    assert_eq!(n(sim.stockpile().amount(ResourceKind::Gold)), 181);
    assert_eq!(sim.market_rates(), [113, 100, 130]);
}

#[test]
fn insufficient_gold_rejects_the_trade_whole() {
    let mut sim = sim_with_market();
    let mut idle = sim_with_market();
    // Starting gold is 100; a food lot buys at 150.
    assert!(!sim.can_market_buy(900, ResourceKind::Food));
    sim.push_command(Command::MarketBuy {
        building: 900,
        resource: ResourceKind::Food,
    });
    sim.tick();
    idle.tick();
    assert_eq!(n(sim.stockpile().amount(ResourceKind::Food)), 200);
    assert_eq!(n(sim.stockpile().amount(ResourceKind::Gold)), 100);
    assert_eq!(sim.market_rates(), [115, 100, 130]);
    // Nothing moved anywhere — the rejected trade leaves the hash
    // stream identical to a sim that never saw the command.
    assert_eq!(sim.state_hash(), idle.state_hash());
}

#[test]
fn insufficient_lot_and_gold_itself_cannot_trade() {
    let mut sim = sim_with_market();
    // Drain food to 50: no full lot to sell.
    assert!(sim.stockpiles[0].spend(ResourceKind::Food, I16F16::from_num(150)));
    assert!(!sim.can_market_sell(900, ResourceKind::Food));
    sim.push_command(Command::MarketSell {
        building: 900,
        resource: ResourceKind::Food,
    });
    // Gold is the currency, never a commodity.
    sim.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(10_000));
    assert!(!sim.can_market_buy(900, ResourceKind::Gold));
    assert!(!sim.can_market_sell(900, ResourceKind::Gold));
    sim.push_command(Command::MarketBuy {
        building: 900,
        resource: ResourceKind::Gold,
    });
    sim.tick();
    assert_eq!(n(sim.stockpile().amount(ResourceKind::Food)), 50);
    assert_eq!(sim.market_rates(), [115, 100, 130]);
}

#[test]
fn unfinished_and_wrong_kind_buildings_cannot_trade() {
    let mut sim = Sim::new(7);
    let unfinished = market_at(&sim, 900, OWNER_PLAYER, (58, 58), false);
    sim.buildings.push(unfinished);
    let rax = market_at(&sim, 901, OWNER_PLAYER, (20, 20), true);
    let mut rax = rax;
    rax.kind = BuildingKind::Barracks;
    sim.buildings.push(rax);
    sim.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(10_000));
    assert!(!sim.can_market_buy(900, ResourceKind::Wood));
    assert!(!sim.can_market_buy(901, ResourceKind::Wood));
    assert!(!sim.can_market_buy(999, ResourceKind::Wood));
    sim.push_command(Command::MarketBuy {
        building: 900,
        resource: ResourceKind::Wood,
    });
    sim.push_command(Command::MarketBuy {
        building: 901,
        resource: ResourceKind::Wood,
    });
    sim.tick();
    assert_eq!(sim.market_rates(), [115, 100, 130]);
    assert_eq!(n(sim.stockpile().amount(ResourceKind::Wood)), 200);
}

#[test]
fn repeated_sells_bottom_out_at_the_wiki_floor() {
    let mut sim = sim_with_market();
    sim.stockpiles[0].add(ResourceKind::Wood, I16F16::from_num(30_000));
    // 40 sells of wood move the rate 100 → 20 (the wiki's 4,000
    // wood to bottom out); the 41st still executes — at the floor
    // sell price of 14 — without moving the rate further.
    for _ in 0..41 {
        sim.push_command(Command::MarketSell {
            building: 900,
            resource: ResourceKind::Wood,
        });
        sim.tick();
    }
    assert_eq!(sim.market_rates()[1], 20);
    assert_eq!(sim.market_sell_price(ResourceKind::Wood), 14);
    assert_eq!(sim.market_buy_price(ResourceKind::Wood), 26);
}

#[test]
fn two_sims_stay_in_lockstep_across_a_shared_trade_stream() {
    let mut a = Sim::new(7);
    let mut b = Sim::new(7);
    for sim in [&mut a, &mut b] {
        let mine = market_at(sim, 900, OWNER_PLAYER, (58, 58), true);
        sim.buildings.push(mine);
        let theirs = market_at(sim, 901, OWNER_OPPONENT, (20, 20), true);
        sim.buildings.push(theirs);
        sim.stockpiles[0].add(ResourceKind::Gold, I16F16::from_num(5_000));
        sim.stockpiles[1].add(ResourceKind::Gold, I16F16::from_num(5_000));
        sim.stockpiles[1].add(ResourceKind::Stone, I16F16::from_num(5_000));
    }
    // An identical script for both sims: my buys, the opponent's
    // sells — the opponent's stone sells move the price *I* pay,
    // which is the global-rate rule (TRD-02) made observable.
    let script: [Command; 6] = [
        Command::MarketBuy { building: 900, resource: ResourceKind::Wood },
        Command::MarketSell { building: 901, resource: ResourceKind::Stone },
        Command::MarketSell { building: 900, resource: ResourceKind::Food },
        Command::MarketBuy { building: 901, resource: ResourceKind::Food },
        Command::MarketSell { building: 901, resource: ResourceKind::Stone },
        Command::MarketBuy { building: 900, resource: ResourceKind::Stone },
    ];
    let stone_buy_before = a.market_buy_price(ResourceKind::Stone);
    for (i, cmd) in script.iter().enumerate() {
        a.push_command(*cmd);
        b.push_command(*cmd);
        a.tick();
        b.tick();
        assert_eq!(a.state_hash(), b.state_hash(), "desync after command {i}");
        assert_eq!(a.market_rates(), b.market_rates());
    }
    // Stone was sold twice by the opponent before my buy: rate
    // 130 → 126, so my buy price fell from 169 to 164 — shared
    // prices, moved only by commands, identical on both sims.
    assert_eq!(stone_buy_before, 169);
    assert_eq!(a.market_rates(), [115, 102, 128]);
}
