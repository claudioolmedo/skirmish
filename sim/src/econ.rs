//! Economy model (M2): resources, resource nodes, and the stockpile.
//!
//! The AoE2-style loop in deterministic form: villagers walk to a resource
//! node, gather into a carry buffer, haul it back to the Town Center, and
//! deposit. Every number here is fixed-point; gather rates are per-tick
//! raw-bit constants tuned toward the publicly documented per-second rates
//! (community wiki values), converted once, by hand, at authoring time —
//! never computed with floats at runtime.
//!
//! Per-second targets → per-tick bits (rate × 65536 / 60, rounded):
//! - wood  0.39/s → 426 bits/tick (effective 0.3901/s)
//! - food  0.31/s → 339 bits/tick (effective 0.3104/s)  [berries]
//! - gold  0.38/s → 415 bits/tick (effective 0.3799/s)
//! - stone 0.36/s → 393 bits/tick (effective 0.3598/s)
//! - food  0.319/s → 348 bits/tick (effective 0.3186/s) [farm, M2.5]
//! - food  0.408/s → 446 bits/tick (effective 0.4082/s) [hunt carcass, M3]
//! - food  0.330/s → 360 bits/tick (effective 0.3296/s) [sheep carcass, M3]

use fixed::types::I16F16;

/// The four stockpile resources. Declaration order is the canonical order
/// used by the state hash and the WASM stockpile array: Food, Wood, Gold,
/// Stone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceKind {
    Food,
    Wood,
    Gold,
    Stone,
}

impl ResourceKind {
    /// All resources in canonical order.
    pub const ALL: [ResourceKind; 4] = [
        ResourceKind::Food,
        ResourceKind::Wood,
        ResourceKind::Gold,
        ResourceKind::Stone,
    ];

    /// Index into stockpile arrays (canonical order).
    pub fn index(self) -> usize {
        match self {
            ResourceKind::Food => 0,
            ResourceKind::Wood => 1,
            ResourceKind::Gold => 2,
            ResourceKind::Stone => 3,
        }
    }

    /// Hash tag: 1..=4 (0 is reserved for "no resource" / empty carry).
    pub fn tag(self) -> u8 {
        self.index() as u8 + 1
    }

    /// Inverse of `tag`; 0 and out-of-range values map to `None`.
    pub fn from_tag(tag: u8) -> Option<ResourceKind> {
        match tag {
            1 => Some(ResourceKind::Food),
            2 => Some(ResourceKind::Wood),
            3 => Some(ResourceKind::Gold),
            4 => Some(ResourceKind::Stone),
            _ => None,
        }
    }
}

/// A harvestable node type. Amounts are the standard full-node contents;
/// rates are per-tick fixed-point constants (see module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Tree,
    BerryBush,
    GoldMine,
    StoneMine,
    /// A completed Farm (M2.5): created by construction, never part of
    /// the starting layout. Yields food like a berry bush but at the
    /// farm rate, and holds exactly `FARM_FOOD_AMOUNT`.
    Farm,
    /// A hunted animal's carcass (M3): left where a boar or deer died,
    /// worked at the hunt rate with the hunt carry capacity (35), and
    /// the only node family that decays (ECO-12). The amount is the
    /// dead animal's food value, not this kind's nominal maximum.
    CarcassHunt,
    /// A herdable's carcass (M3): a killed sheep, worked at the
    /// shepherd rate with the standard carry capacity (10) and the
    /// slower herdable decay.
    CarcassHerd,
}

impl NodeKind {
    /// The resource this node yields.
    pub fn resource(self) -> ResourceKind {
        match self {
            NodeKind::Tree => ResourceKind::Wood,
            NodeKind::BerryBush => ResourceKind::Food,
            NodeKind::GoldMine => ResourceKind::Gold,
            NodeKind::StoneMine => ResourceKind::Stone,
            NodeKind::Farm => ResourceKind::Food,
            NodeKind::CarcassHunt => ResourceKind::Food,
            NodeKind::CarcassHerd => ResourceKind::Food,
        }
    }

    /// Contents of a fresh node (100 / 125 / 800 / 350 / 175). For
    /// carcasses this is only the *nominal* maximum (the client's
    /// amount-bar scale): a carcass node's real contents are the dead
    /// animal's food value, set at spawn — 340 is the largest (boar),
    /// 100 the sheep.
    pub fn full_amount(self) -> I16F16 {
        I16F16::from_num(match self {
            NodeKind::Tree => 100,
            NodeKind::BerryBush => 125,
            NodeKind::GoldMine => 800,
            NodeKind::StoneMine => 350,
            // Farm base food is 175 (public wiki value; 250 is the
            // post-Horse-Collar figure — see MECHANICS-STUDY §6.3).
            NodeKind::Farm => 175,
            NodeKind::CarcassHunt => 340,
            NodeKind::CarcassHerd => 100,
        })
    }

    /// Amount gathered per tick while a villager works this node.
    /// Raw bits on purpose (see module docs for the per-second targets).
    pub fn gather_rate(self) -> I16F16 {
        I16F16::from_bits(match self {
            NodeKind::Tree => 426,
            NodeKind::BerryBush => 339,
            NodeKind::GoldMine => 415,
            NodeKind::StoneMine => 393,
            NodeKind::Farm => 348,
            // Hunt 0.408/s (study §6.1); shepherd 0.330/s (SPEC §2.2).
            NodeKind::CarcassHunt => 446,
            NodeKind::CarcassHerd => 360,
        })
    }

    /// How much a villager can carry away from this node before it
    /// must haul back: 10 for everything except hunt carcasses, whose
    /// meat bundles to 35 (ECO-03 — the case keys off the node kind,
    /// not a villager state).
    pub fn carry_cap(self) -> I16F16 {
        match self {
            NodeKind::CarcassHunt => HUNT_CARRY_CAPACITY,
            _ => CARRY_CAPACITY,
        }
    }

    /// Stable tag for the state hash and the WASM node array:
    /// 0 tree, 1 berry, 2 gold, 3 stone, 4 farm, 5 hunt carcass,
    /// 6 herdable carcass.
    pub fn tag(self) -> u8 {
        match self {
            NodeKind::Tree => 0,
            NodeKind::BerryBush => 1,
            NodeKind::GoldMine => 2,
            NodeKind::StoneMine => 3,
            NodeKind::Farm => 4,
            NodeKind::CarcassHunt => 5,
            NodeKind::CarcassHerd => 6,
        }
    }
}

/// One harvestable resource node on the map. Position is a fixed-point
/// world point (a grid cell center for placed nodes; the death spot
/// for carcasses); `amount_left` drains as villagers gather and the
/// node is inert at zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceNode {
    pub id: u32,
    pub kind: NodeKind,
    pub pos_x: I16F16,
    pub pos_y: I16F16,
    pub amount_left: I16F16,
    /// Per-tick decay (M3, ECO-12): carcasses — and only carcasses —
    /// lose this much food every tick from the moment of death,
    /// gathered or not. Zero for every placed node.
    pub decay_per_tick: I16F16,
}

impl ResourceNode {
    /// A placed starting-layout / farm node: full, and never decays.
    pub fn placed(id: u32, kind: NodeKind, pos_x: I16F16, pos_y: I16F16) -> Self {
        ResourceNode {
            id,
            kind,
            pos_x,
            pos_y,
            amount_left: kind.full_amount(),
            decay_per_tick: I16F16::ZERO,
        }
    }
}

impl ResourceNode {
    /// The resource this node yields.
    pub fn resource(&self) -> ResourceKind {
        self.kind.resource()
    }

    /// A depleted node yields nothing and rejects new gather orders.
    pub fn is_depleted(&self) -> bool {
        self.amount_left <= I16F16::ZERO
    }
}

/// The player's resource stockpile. Only *deposited* resources live here —
/// what a villager carries is still on the villager (see `Entity::carry`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stockpile {
    amounts: [I16F16; 4], // canonical order: Food, Wood, Gold, Stone
}

impl Stockpile {
    /// Standard start: 200 food, 200 wood, 100 gold, 200 stone.
    pub fn starting() -> Self {
        Stockpile {
            amounts: [
                I16F16::from_num(200),
                I16F16::from_num(200),
                I16F16::from_num(100),
                I16F16::from_num(200),
            ],
        }
    }

    pub fn amount(&self, kind: ResourceKind) -> I16F16 {
        self.amounts[kind.index()]
    }

    /// All four amounts in canonical order (Food, Wood, Gold, Stone).
    pub fn amounts(&self) -> [I16F16; 4] {
        self.amounts
    }

    pub fn add(&mut self, kind: ResourceKind, amount: I16F16) {
        let i = kind.index();
        self.amounts[i] += amount;
    }

    /// Deduct `amount` if the stockpile covers it. Returns success; on
    /// failure nothing changes (no partial spends).
    pub fn spend(&mut self, kind: ResourceKind, amount: I16F16) -> bool {
        let i = kind.index();
        if self.amounts[i] < amount {
            return false;
        }
        self.amounts[i] -= amount;
        true
    }
}

/// How much a villager can carry before it must haul back: 10.
pub const CARRY_CAPACITY: I16F16 = I16F16::from_bits(655_360);
/// Carry capacity from a hunt carcass (boar/deer meat): 35 (ECO-03).
pub const HUNT_CARRY_CAPACITY: I16F16 = I16F16::from_bits(2_293_760); // 35
/// Villager training cost: 50 food, deducted when the order is enqueued.
pub const VILLAGER_FOOD_COST: I16F16 = I16F16::from_bits(3_276_800); // 50
/// Villager training time: 25 s at 60 Hz.
pub const VILLAGER_TRAIN_TICKS: u64 = 1500;
/// Maximum number of villagers waiting in the Town Center queue.
pub const TRAIN_QUEUE_CAP: u32 = 15;
/// A villager this close to its node is gathering from it: 2.5 units.
/// Generous on purpose — separation can hold a latecomer slightly off the
/// node point when others are already working it.
pub const GATHER_RANGE: I16F16 = I16F16::from_bits(163_840); // 2.5
/// A villager this close to the Town Center deposits instantly: 3.0 units.
pub const DEPOSIT_RANGE: I16F16 = I16F16::from_bits(196_608); // 3.0
/// ECO-09 retarget search radius (M4): when a gatherer's node
/// depletes, the nearest live node of the same resource *around the
/// depleted node* is searched within this radius (the same woodline /
/// berry patch / mine cluster). ECO-09 records the exact radius as
/// UNVERIFIED in public sources — this is a tuned constant: 12 world
/// units = 3 cells, which chains a cluster (its nodes sit 4 apart)
/// but never reaches a different cluster or patch.
pub const RETARGET_RADIUS: I16F16 = I16F16::from_bits(786_432); // 12.0
/// Rally-point node snap radius (M4, SEL-13): a rally point set
/// within this distance of a live node snaps to it — new villagers
/// then go straight to work on that node. Matches the client's
/// right-click node pick radius (4 world units).
pub const RALLY_NODE_SNAP_RADIUS: I16F16 = I16F16::from_bits(262_144); // 4.0

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starting_stockpile_is_standard() {
        let s = Stockpile::starting();
        assert_eq!(s.amount(ResourceKind::Food), I16F16::from_num(200));
        assert_eq!(s.amount(ResourceKind::Wood), I16F16::from_num(200));
        assert_eq!(s.amount(ResourceKind::Gold), I16F16::from_num(100));
        assert_eq!(s.amount(ResourceKind::Stone), I16F16::from_num(200));
    }

    #[test]
    fn spend_requires_funds_and_is_atomic() {
        let mut s = Stockpile::starting();
        assert!(s.spend(ResourceKind::Food, I16F16::from_num(50)));
        assert_eq!(s.amount(ResourceKind::Food), I16F16::from_num(150));
        assert!(!s.spend(ResourceKind::Food, I16F16::from_num(151)));
        assert_eq!(
            s.amount(ResourceKind::Food),
            I16F16::from_num(150),
            "failed spend must not deduct"
        );
    }

    #[test]
    fn node_kind_numbers() {
        assert_eq!(NodeKind::Tree.full_amount(), I16F16::from_num(100));
        assert_eq!(NodeKind::BerryBush.full_amount(), I16F16::from_num(125));
        assert_eq!(NodeKind::GoldMine.full_amount(), I16F16::from_num(800));
        assert_eq!(NodeKind::StoneMine.full_amount(), I16F16::from_num(350));
        assert_eq!(NodeKind::Tree.resource(), ResourceKind::Wood);
        assert_eq!(NodeKind::BerryBush.resource(), ResourceKind::Food);
        assert_eq!(NodeKind::GoldMine.resource(), ResourceKind::Gold);
        assert_eq!(NodeKind::StoneMine.resource(), ResourceKind::Stone);
        // Rates stay in the documented ballpark (0.30–0.40 per second).
        for kind in [
            NodeKind::Tree,
            NodeKind::BerryBush,
            NodeKind::GoldMine,
            NodeKind::StoneMine,
        ] {
            let per_sec = kind.gather_rate() * I16F16::from_num(60);
            assert!(
                per_sec > I16F16::from_num(0.3) && per_sec < I16F16::from_num(0.4),
                "{kind:?} rate off target: {per_sec}/s"
            );
        }
    }

    #[test]
    fn resource_tags_round_trip() {
        for kind in ResourceKind::ALL {
            assert_eq!(ResourceKind::from_tag(kind.tag()), Some(kind));
        }
        assert_eq!(ResourceKind::from_tag(0), None);
    }

    // M7: the fixed NODE_SPECS layout table is gone — node placement
    // is the map generator's job now (`mapgen`), and its tests pin
    // the generated packages' uniqueness and fairness.
}
