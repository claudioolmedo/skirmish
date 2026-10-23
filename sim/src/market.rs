//! The Market's commodity exchange (TRD-01..03): one global
//! exchange rate per commodity, shared by every player, moved only
//! by executed buy/sell commands — never by wall-clock, never by
//! the AI, never by anything outside the command stream, so
//! lockstep and replays stay exact.
//!
//! Numbers from the public mechanics study of the reference game
//! (AoE wiki, "Market (Age of Empires II)", "Exchange of resources"
//! and infobox, fetched 2026-10-22):
//!
//! - Commodities are food, wood and stone, traded against gold in
//!   lots of 100 (TRD-01). Gold itself is never a commodity.
//! - Starting exchange rates (gold per 100): wood 100, food 115,
//!   stone 130. Food's 115 is the current-DE value (update 125283
//!   raised it from 100).
//! - The default commodity trading fee is 30%: the sell price is
//!   rate × (1 − fee), the buy price rate × (1 + fee), each rounded
//!   to the nearest gold (halves away from zero — the wiki's worked
//!   numbers pin this: at the start, wood sells/buys 70/130, food
//!   81/150, stone 91/169). The fee is the generic civilization's;
//!   Guilds (15%, Imperial Age) and civilization-specific fees are
//!   later content — the *rate* stays global regardless (TRD-02).
//! - Every transaction moves the rate by 2 for everyone: a buy
//!   raises it, a sell lowers it (TRD-03). Rates clamp to
//!   [20, 9999] — the wiki's floor (sell 14 / buy 26 at the default
//!   fee) and ceiling (sell 6999 / buy 12999) reproduce exactly
//!   under the price formulas below.

use crate::econ::ResourceKind;

/// Units of a commodity per transaction (TRD-01).
pub const TRADE_LOT: i32 = 100;
/// Rate movement per transaction (TRD-03).
pub const RATE_STEP: i32 = 2;
/// Exchange-rate floor (TRD-03).
pub const RATE_MIN: i32 = 20;
/// Exchange-rate ceiling (TRD-03).
pub const RATE_MAX: i32 = 9999;
/// The default commodity trading fee, percent (TRD-02).
pub const FEE_PERCENT: i32 = 30;

/// Starting exchange rates (gold per 100), canonical commodity
/// order (food, wood, stone — `ResourceKind` order minus gold).
pub const START_RATES: [i32; 3] = [115, 100, 130];

/// Whether a resource is a tradeable commodity (food, wood, stone).
/// Gold is the currency, never a commodity.
pub fn is_commodity(resource: ResourceKind) -> bool {
    !matches!(resource, ResourceKind::Gold)
}

/// Slot of a commodity in the rate table (food 0, wood 1, stone 2 —
/// the canonical resource order with gold skipped). Gold maps to
/// the food slot; callers must gate on `is_commodity` first.
fn slot(resource: ResourceKind) -> usize {
    match resource {
        ResourceKind::Food => 0,
        ResourceKind::Wood => 1,
        ResourceKind::Stone => 2,
        ResourceKind::Gold => 0,
    }
}

/// The commodities in rate-table order.
pub const COMMODITIES: [ResourceKind; 3] = [
    ResourceKind::Food,
    ResourceKind::Wood,
    ResourceKind::Stone,
];

/// Sell price of one lot at `rate`: rate × (1 − fee), rounded to
/// the nearest integer (halves away from zero). Pure integer math;
/// reproduces every worked number on the wiki's Market page
/// (70 at rate 100, 81 at 115, 91 at 130, 14 at 20, 6999 at 9999).
pub fn sell_price_at(rate: i32) -> i32 {
    (rate * (100 - FEE_PERCENT) + 50) / 100
}

/// Buy price of one lot at `rate`: rate × (1 + fee), rounded the
/// same way (130 at rate 100, 150 at 115, 169 at 130, 26 at 20,
/// 12999 at 9999).
pub fn buy_price_at(rate: i32) -> i32 {
    (rate * (100 + FEE_PERCENT) + 50) / 100
}

/// The match's global exchange rates (TRD-02). Sim state: created
/// at the start rates, mutated only by `apply_buy` / `apply_sell`
/// from the market command appliers, and hashed once any rate has
/// moved (see `Sim::hash_parts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Market {
    rates: [i32; 3],
}

impl Market {
    pub fn new() -> Self {
        Market { rates: START_RATES }
    }

    /// The current exchange rate (gold per 100) of a commodity.
    pub fn rate(&self, resource: ResourceKind) -> i32 {
        self.rates[slot(resource)]
    }

    /// All three rates in canonical commodity order.
    pub fn rates(&self) -> [i32; 3] {
        self.rates
    }

    /// What a player receives for selling one lot of `resource`
    /// right now.
    pub fn sell_price(&self, resource: ResourceKind) -> i32 {
        sell_price_at(self.rate(resource))
    }

    /// What a player pays to buy one lot of `resource` right now.
    pub fn buy_price(&self, resource: ResourceKind) -> i32 {
        buy_price_at(self.rate(resource))
    }

    /// True while every rate still stands at its start value — the
    /// hash writes the rates only once this stops holding (the
    /// VIC-04 conditional-write precedent), so a trade-free stream
    /// stays byte-identical to the pre-Market stream.
    pub fn is_default(&self) -> bool {
        self.rates == START_RATES
    }

    /// Record a completed buy: the rate rises by `RATE_STEP`,
    /// clamped at `RATE_MAX` (TRD-03).
    pub fn apply_buy(&mut self, resource: ResourceKind) {
        let s = slot(resource);
        self.rates[s] = (self.rates[s] + RATE_STEP).min(RATE_MAX);
    }

    /// Record a completed sell: the rate falls by `RATE_STEP`,
    /// clamped at `RATE_MIN` (TRD-03).
    pub fn apply_sell(&mut self, resource: ResourceKind) {
        let s = slot(resource);
        self.rates[s] = (self.rates[s] - RATE_STEP).max(RATE_MIN);
    }
}

impl Default for Market {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_prices_match_the_wiki_table() {
        let m = Market::new();
        // Wiki, Market: starting sell/buy 70/130 (wood), 81/150
        // (food), 91/169 (stone) at the default 30% fee.
        assert_eq!(m.sell_price(ResourceKind::Wood), 70);
        assert_eq!(m.buy_price(ResourceKind::Wood), 130);
        assert_eq!(m.sell_price(ResourceKind::Food), 81);
        assert_eq!(m.buy_price(ResourceKind::Food), 150);
        assert_eq!(m.sell_price(ResourceKind::Stone), 91);
        assert_eq!(m.buy_price(ResourceKind::Stone), 169);
        assert!(m.is_default());
    }

    #[test]
    fn floor_and_ceiling_prices_match_the_wiki() {
        // Wiki: the rate cannot go below 20 (sell 14 / buy 26) or
        // above 9,999 (max sell 6,999 / max buy 12,999, no Guilds).
        assert_eq!(sell_price_at(RATE_MIN), 14);
        assert_eq!(buy_price_at(RATE_MIN), 26);
        assert_eq!(sell_price_at(RATE_MAX), 6999);
        assert_eq!(buy_price_at(RATE_MAX), 12999);
    }

    #[test]
    fn bottoming_out_takes_the_wiki_amounts() {
        // Wiki: prices bottom out after selling 4,000 wood, 4,800
        // food, 5,500 stone — the clamp must engage after exactly
        // those lot counts (40 / 48 / 55 sells).
        let mut m = Market::new();
        for _ in 0..40 {
            m.apply_sell(ResourceKind::Wood);
        }
        assert_eq!(m.rate(ResourceKind::Wood), RATE_MIN);
        let mut m = Market::new();
        for _ in 0..48 {
            m.apply_sell(ResourceKind::Food);
        }
        assert_eq!(m.rate(ResourceKind::Food), RATE_MIN);
        let mut m = Market::new();
        for _ in 0..55 {
            m.apply_sell(ResourceKind::Stone);
        }
        assert_eq!(m.rate(ResourceKind::Stone), RATE_MIN);
        // Further sells stay clamped.
        m.apply_sell(ResourceKind::Stone);
        assert_eq!(m.rate(ResourceKind::Stone), RATE_MIN);
    }
}
