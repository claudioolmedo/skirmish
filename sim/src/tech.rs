//! Technologies & ages (M6) — RULEBOOK §11 (TEC).
//!
//! The per-player progression layer: an age state (Dark → Feudal in
//! M6) and a researched-technology set, both part of the hashed
//! state. Technologies are *data patches applied at lookup*
//! (TEC-06): the base unit table in `units` never changes; the sim
//! computes an owner's effective row by layering the owner's age
//! (CLS-04) and researched techs over it — see `Sim::stats_for`.
//!
//! Research itself is production (TEC-03/TEC-05): a research item
//! occupies its host's queue — the Town Center's queue is *shared*
//! with villager training, a Blacksmith's queue holds its tech
//! lines, the Barracks' queue holds the Man-at-Arms upgrade behind
//! any Militia in line — the cost is charged when the item is
//! enqueued, and the effect lands the tick the research completes.
//! (Cancellation with refund — the other half of TEC-05 — is not
//! implemented; no cancel exists anywhere in M6, as recorded in
//! the rulebook.)
//!
//! Numbers come from the project docs, converted once, by hand:
//! - Loom: 50 gold, 25 s (study §8; OPENING-SPEC §2.6). Effects:
//!   villager +15 HP, +1 melee / +2 pierce armor (study §8), and
//!   +0.1 tiles/s speed (RULEBOOK TEC-06 / CLS-05 — the study's
//!   effect row omits the speed; the rulebook is the behavior spec,
//!   so the speed is implemented and the discrepancy is recorded
//!   in the M6 report).
//! - Feudal Age: 500 food, 130 s, requires any 2 completed Dark-Age
//!   buildings besides Houses/Farms (TEC-01; study §7).
//! - Fletching: 100 food / 50 gold, 30 s — archers +1 attack and
//!   +1 range (study §8; the study's Castle/Tower/Town Center lines
//!   have no targets in M6 — no such attacks exist yet).
//! - Scale Mail Armor: 100 food, 40 s — infantry +1/+1 armor
//!   (study §8; the M6 brief's "+1 melee" summary is narrower than
//!   the study's row, and the study governs — recorded in the
//!   report).
//! - Padded Archer Armor: 100 food, 40 s — archers +1/+1 armor
//!   (study §8).
//! - Man-at-Arms upgrade: the study pins the *unit row* (§3.2) but
//!   no project doc pins the upgrade's cost or research time.
//!   100 food / 40 gold, 40 s is adopted from the reference game's
//!   public values and flagged UNVERIFIED (not in the project
//!   docs) here and in the M6 report.
//! - Wheelbarrow: 175 food / 50 wood, 75 s, Town Center, Feudal
//!   Age (wiki, Technology (Age of Empires II) — economy table;
//!   wiki, Wheelbarrow (Age of Empires II)). Effects: villager
//!   carry capacity +25% as the wiki's comparison table resolves
//!   it (10 → 13 ordinary, 35 → 44 hunted meat) and villager
//!   speed +10% — TEC-06's first *percentage* patch: the speed
//!   scalar multiplies the layered row (base + flat adds) in
//!   `Sim::stats_for`. Its successor Hand Cart is a Castle Age
//!   technology (same sources) and waits for that Age (ECO-03).
//! - The Feudal completion set (2026-10-22; all values fetched
//!   from the Age of Empires wiki this run, per-tech pages):
//!   Forging — Blacksmith, 150 food, 50 s: infantry and cavalry
//!   melee attack +1 (wiki, Forging: "Infantry, Cavalry and
//!   Hulk-line +1 attack"; the Hulk line is naval and has no
//!   target here, and the page's Ballista Elephant note confirms
//!   the bonus is to the *melee* attack — pierce-dealing cavalry
//!   gains nothing, and no shipped cavalry deals pierce).
//!   Scale Barding Armor — Blacksmith, 150 food, 45 s: cavalry
//!   +1 melee / +1 pierce armor (wiki, Scale Barding Armor).
//!   Bloodlines — Stable, 150 food / 100 gold, 50 s: mounted
//!   units +20 HP (wiki, Bloodlines).
//!   Double-Bit Axe — Lumber Camp, 100 food / 50 wood, 25 s:
//!   wood gather rate +20% (wiki, Double-Bit Axe).
//!   Gold Mining — Mining Camp, 100 food / 75 wood, 30 s: gold
//!   gather rate +15% (wiki, Gold Mining (Age of Empires II)).
//!   Stone Mining — Mining Camp, 100 food / 75 wood, 30 s:
//!   stone gather rate +15% (wiki, Stone Mining (Age of
//!   Empires II)). Horse Collar — Mill, 75 food / 75 wood, 20 s:
//!   farms hold +75 food (175 → 250); since update 169123 a
//!   standing farm also gains proportionally at the research
//!   tick — 75 × (remaining ⁄ full)² (wiki, Horse Collar, with
//!   its worked example: a farm at 50% gains 25% of 75).

use crate::build::BuildingKind;
use crate::econ::NodeKind;
use crate::entity::EntityKind;
use crate::units;
use fixed::types::I16F16;

/// The ages a player can stand in (M6 ships the first step of the
/// TEC-01 ladder; Castle/Imperial are later milestones). Ordered:
/// `Dark < Feudal`, so age requirements are plain comparisons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Age {
    #[default]
    Dark,
    Feudal,
    /// The Castle Age (Castle P1, 2026-10-22).
    Castle,
}

impl Age {
    /// Stable tag for the state hash and the WASM ABI:
    /// 0 = Dark Age, 1 = Feudal Age, 2 = Castle Age.
    pub fn tag(self) -> u8 {
        match self {
            Age::Dark => 0,
            Age::Feudal => 1,
            Age::Castle => 2,
        }
    }
}

/// One researchable technology (or age advancement). The tag order
/// is the ABI/hash contract — append, never renumber.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TechId {
    /// Town Center, Dark Age: villager durability (TEC-04).
    Loom,
    /// Town Center: the age advancement itself (TEC-01).
    FeudalAge,
    /// Blacksmith, Feudal: archer attack/range (study §8).
    Fletching,
    /// Blacksmith, Feudal: infantry armor (study §8).
    ScaleMail,
    /// Blacksmith, Feudal: archer armor (study §8).
    PaddedArcherArmor,
    /// Barracks, Feudal: the Militia line upgrades (CLS-03).
    ManAtArms,
    /// Town Center, Feudal: villager carry + speed (ECO-03).
    Wheelbarrow,
    /// Blacksmith, Feudal: infantry + cavalry melee attack +1.
    Forging,
    /// Blacksmith, Feudal: cavalry +1/+1 armor.
    ScaleBarding,
    /// Stable, Feudal: mounted units +20 HP.
    Bloodlines,
    /// Lumber Camp, Feudal: wood gather rate +20%.
    DoubleBitAxe,
    /// Mining Camp, Feudal: gold gather rate +15%.
    GoldMining,
    /// Mining Camp, Feudal: stone gather rate +15%.
    StoneMining,
    /// Mill, Feudal: farms +75 food (ECO-14).
    HorseCollar,
    /// Town Center: the Castle Age advancement itself (TEC-01).
    CastleAge,
    /// Town Center, Castle: villager carry + speed, the second
    /// link of the ECO-03 chain (requires Wheelbarrow, TEC-07).
    HandCart,
}

impl TechId {
    /// All technologies, in tag order.
    pub const ALL: [TechId; 16] = [
        TechId::Loom,
        TechId::FeudalAge,
        TechId::Fletching,
        TechId::ScaleMail,
        TechId::PaddedArcherArmor,
        TechId::ManAtArms,
        TechId::Wheelbarrow,
        TechId::Forging,
        TechId::ScaleBarding,
        TechId::Bloodlines,
        TechId::DoubleBitAxe,
        TechId::GoldMining,
        TechId::StoneMining,
        TechId::HorseCollar,
        TechId::CastleAge,
        TechId::HandCart,
    ];

    /// Stable tag: 0 loom, 1 feudal age, 2 fletching, 3 scale mail,
    /// 4 padded archer armor, 5 man-at-arms, 6 wheelbarrow,
    /// 7 forging, 8 scale barding, 9 bloodlines, 10 double-bit
    /// axe, 11 gold mining, 12 stone mining, 13 horse collar,
    /// 14 castle age, 15 hand cart (Castle P1 — appended after the
    /// Feudal-completion package took 7–13, so no tag moved).
    pub fn tag(self) -> u8 {
        match self {
            TechId::Loom => 0,
            TechId::FeudalAge => 1,
            TechId::Fletching => 2,
            TechId::ScaleMail => 3,
            TechId::PaddedArcherArmor => 4,
            TechId::ManAtArms => 5,
            TechId::Wheelbarrow => 6,
            TechId::Forging => 7,
            TechId::ScaleBarding => 8,
            TechId::Bloodlines => 9,
            TechId::DoubleBitAxe => 10,
            TechId::GoldMining => 11,
            TechId::StoneMining => 12,
            TechId::HorseCollar => 13,
            TechId::CastleAge => 14,
            TechId::HandCart => 15,
        }
    }

    /// Inverse of `tag`; unknown values map to `None`.
    pub fn from_tag(tag: u8) -> Option<TechId> {
        TechId::ALL.iter().find(|t| t.tag() == tag).copied()
    }

    /// Bit in a player's researched-technology mask.
    pub fn bit(self) -> u64 {
        1u64 << self.tag()
    }

    /// Where this research is performed: the Town Center, or one
    /// placed building kind.
    pub fn host(self) -> TechHost {
        match self {
            TechId::Loom
            | TechId::FeudalAge
            | TechId::Wheelbarrow
            | TechId::CastleAge
            | TechId::HandCart => TechHost::TownCenter,
            TechId::Fletching
            | TechId::ScaleMail
            | TechId::PaddedArcherArmor
            | TechId::Forging
            | TechId::ScaleBarding => TechHost::Building(BuildingKind::Blacksmith),
            TechId::ManAtArms => TechHost::Building(BuildingKind::Barracks),
            TechId::Bloodlines => TechHost::Building(BuildingKind::Stable),
            TechId::DoubleBitAxe => TechHost::Building(BuildingKind::LumberCamp),
            TechId::GoldMining | TechId::StoneMining => {
                TechHost::Building(BuildingKind::MiningCamp)
            }
            TechId::HorseCollar => TechHost::Building(BuildingKind::Mill),
        }
    }

    /// The age the researching player must stand in (TEC-07). The
    /// age advancement itself is researched *from* the Dark Age.
    pub fn age_required(self) -> Age {
        match self {
            TechId::Loom | TechId::FeudalAge => Age::Dark,
            TechId::CastleAge => Age::Feudal,
            TechId::HandCart => Age::Castle,
            _ => Age::Feudal,
        }
    }

    /// A technology that must already be researched before this
    /// one can start (TEC-07's tech→tech prerequisite links — the
    /// Castle package lands the first: Hand Cart chains from
    /// Wheelbarrow, wiki, Hand Cart: "the further upgrade of
    /// Wheelbarrow"). Age advancements read off the age state
    /// itself, so they never appear here.
    pub fn requires_tech(self) -> Option<TechId> {
        match self {
            TechId::HandCart => Some(TechId::Wheelbarrow),
            _ => None,
        }
    }

    /// Research time in ticks (60 Hz): Loom 25 s, Feudal 130 s,
    /// Fletching 30 s, the armor lines and the Man-at-Arms upgrade
    /// 40 s each (study §8 / §7; Man-at-Arms UNVERIFIED — see the
    /// module docs).
    pub fn ticks(self) -> u64 {
        match self {
            TechId::Loom => 1500,
            TechId::FeudalAge => 7800,
            TechId::Fletching => 1800,
            TechId::ScaleMail => 2400,
            TechId::PaddedArcherArmor => 2400,
            TechId::ManAtArms => 2400, // UNVERIFIED (module docs)
            TechId::Wheelbarrow => 4500,
            TechId::Forging => 3000,        // 50 s (wiki, Forging)
            TechId::ScaleBarding => 2700,   // 45 s (wiki, Scale Barding Armor)
            TechId::Bloodlines => 3000,     // 50 s (wiki, Bloodlines)
            TechId::DoubleBitAxe => 1500,   // 25 s (wiki, Double-Bit Axe)
            TechId::GoldMining => 1800,     // 30 s (wiki, Gold Mining)
            TechId::StoneMining => 1800,    // 30 s (wiki, Stone Mining)
            TechId::HorseCollar => 1200,    // 20 s (wiki, Horse Collar)
            TechId::CastleAge => 9600,      // 160 s (study §7)
            TechId::HandCart => 3300,       // 55 s (wiki, Hand Cart)
        }
    }

    /// Research cost as (food, wood, gold, stone), charged at
    /// enqueue (TEC-05). Man-at-Arms is UNVERIFIED (module docs).
    pub fn cost(self) -> (i32, i32, i32, i32) {
        match self {
            TechId::Loom => (0, 0, 50, 0),
            TechId::FeudalAge => (500, 0, 0, 0),
            TechId::Fletching => (100, 0, 50, 0),
            TechId::ScaleMail => (100, 0, 0, 0),
            TechId::PaddedArcherArmor => (100, 0, 0, 0),
            TechId::ManAtArms => (100, 0, 40, 0), // UNVERIFIED (module docs)
            TechId::Wheelbarrow => (175, 50, 0, 0),
            TechId::Forging => (150, 0, 0, 0),       // wiki, Forging
            TechId::ScaleBarding => (150, 0, 0, 0),  // wiki, Scale Barding Armor
            TechId::Bloodlines => (150, 0, 100, 0),  // wiki, Bloodlines
            TechId::DoubleBitAxe => (100, 50, 0, 0), // wiki, Double-Bit Axe
            TechId::GoldMining => (100, 75, 0, 0),   // wiki, Gold Mining
            TechId::StoneMining => (100, 75, 0, 0),  // wiki, Stone Mining
            TechId::HorseCollar => (75, 75, 0, 0),   // wiki, Horse Collar
            TechId::CastleAge => (800, 0, 200, 0),   // study §7
            TechId::HandCart => (300, 200, 0, 0),    // wiki, Hand Cart
        }
    }
}

/// The gather rate of a node kind for a villager whose owner has
/// researched `techs` (the researched-mask bits): TEC-06
/// *percentage* patches on the economy, applied at the gather
/// lookup exactly like Wheelbarrow's speed scalar in
/// `Sim::stats_for` — the base table in `econ` never changes.
/// Double-Bit Axe scales Tree by ×120/100, Gold Mining GoldMine
/// by ×115/100, Stone Mining StoneMine by ×115/100 (wiki pages,
/// module docs); the multiply truncates the sub-bit remainder,
/// the same rounding the Wheelbarrow speed patch records.
/// Every other (kind, mask) pair returns the base rate.
pub fn patched_gather_rate(kind: NodeKind, techs: u64) -> I16F16 {
    let pct: i64 = match kind {
        NodeKind::Tree if techs & TechId::DoubleBitAxe.bit() != 0 => 120,
        NodeKind::GoldMine if techs & TechId::GoldMining.bit() != 0 => 115,
        NodeKind::StoneMine if techs & TechId::StoneMining.bit() != 0 => 115,
        _ => return kind.gather_rate(),
    };
    let base = kind.gather_rate();
    I16F16::from_bits(((base.to_bits() as i64 * pct) / 100) as i32)
}

/// Where a technology is researched (see `TechId::host`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TechHost {
    TownCenter,
    Building(BuildingKind),
}

/// One entry in a production queue (M6): a unit in training or a
/// technology in research, sharing one FIFO per host (TEC-03).
/// Before M6 the queues held unit kinds alone; the type change is
/// the substrate every M6 research path rides on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProdEntry {
    Unit(EntityKind),
    Tech(TechId),
}

impl ProdEntry {
    /// Ticks the entry occupies its host's head: a unit's train
    /// time, or a technology's research time.
    pub fn ticks(self) -> u64 {
        match self {
            ProdEntry::Unit(kind) => units::spec(kind).train_ticks,
            ProdEntry::Tech(tech) => tech.ticks(),
        }
    }

    /// Flat code for the WASM queue arrays: unit kinds keep their
    /// entity tag (0–8); technologies are `100 + tech tag`. The
    /// state hash writes the same code (as u32).
    pub fn code(self) -> u32 {
        match self {
            ProdEntry::Unit(kind) => kind.tag() as u32,
            ProdEntry::Tech(tech) => 100 + tech.tag() as u32,
        }
    }
}

/// Why a research cannot currently proceed (M6) — the research
/// counterpart of `TrainBlock`, with its own ABI tag contract:
/// 0 ok, 1 cannot afford, 2 queue full / host unavailable,
/// 3 already researched (or already in a queue), 4 requirements
/// not met (age, prerequisite buildings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResearchBlock {
    Ok,
    NoFunds,
    QueueFull,
    AlreadyDone,
    Locked,
}

impl ResearchBlock {
    pub fn tag(self) -> u8 {
        match self {
            ResearchBlock::Ok => 0,
            ResearchBlock::NoFunds => 1,
            ResearchBlock::QueueFull => 2,
            ResearchBlock::AlreadyDone => 3,
            ResearchBlock::Locked => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tech_table_matches_the_docs() {
        // Loom: 50 gold / 25 s at the Town Center (study §8).
        assert_eq!(TechId::Loom.cost(), (0, 0, 50, 0));
        assert_eq!(TechId::Loom.ticks(), 25 * 60);
        assert_eq!(TechId::Loom.host(), TechHost::TownCenter);
        // Feudal Age: 500 food / 130 s (TEC-01, study §7).
        assert_eq!(TechId::FeudalAge.cost(), (500, 0, 0, 0));
        assert_eq!(TechId::FeudalAge.ticks(), 130 * 60);
        // Fletching: 100 F / 50 G, 30 s, Blacksmith (study §8).
        assert_eq!(TechId::Fletching.cost(), (100, 0, 50, 0));
        assert_eq!(TechId::Fletching.ticks(), 30 * 60);
        assert_eq!(
            TechId::Fletching.host(),
            TechHost::Building(BuildingKind::Blacksmith)
        );
        // The armor lines: 100 food, 40 s (study §8).
        assert_eq!(TechId::ScaleMail.cost(), (100, 0, 0, 0));
        assert_eq!(TechId::ScaleMail.ticks(), 40 * 60);
        assert_eq!(TechId::PaddedArcherArmor.cost(), (100, 0, 0, 0));
        assert_eq!(TechId::PaddedArcherArmor.ticks(), 40 * 60);
        // Wheelbarrow: 175 F / 50 W, 75 s, Town Center (wiki,
        // Technology (Age of Empires II), economy table).
        assert_eq!(TechId::Wheelbarrow.cost(), (175, 50, 0, 0));
        assert_eq!(TechId::Wheelbarrow.ticks(), 75 * 60);
        assert_eq!(TechId::Wheelbarrow.host(), TechHost::TownCenter);
        assert_eq!(TechId::Wheelbarrow.age_required(), Age::Feudal);
        // Castle Age: 800 F / 200 G, 160 s at the Town Center,
        // researched from the Feudal Age (TEC-01; study §7).
        assert_eq!(TechId::CastleAge.cost(), (800, 0, 200, 0));
        assert_eq!(TechId::CastleAge.ticks(), 160 * 60);
        assert_eq!(TechId::CastleAge.host(), TechHost::TownCenter);
        assert_eq!(TechId::CastleAge.age_required(), Age::Feudal);
        // Hand Cart: 300 F / 200 W, 55 s, Town Center, Castle Age,
        // chained from Wheelbarrow (wiki, Hand Cart).
        assert_eq!(TechId::HandCart.cost(), (300, 200, 0, 0));
        assert_eq!(TechId::HandCart.ticks(), 55 * 60);
        assert_eq!(TechId::HandCart.host(), TechHost::TownCenter);
        assert_eq!(TechId::HandCart.age_required(), Age::Castle);
        assert_eq!(TechId::HandCart.requires_tech(), Some(TechId::Wheelbarrow));
        assert_eq!(TechId::Wheelbarrow.requires_tech(), None);
        // Age tags: 0 Dark, 1 Feudal, 2 Castle.
        assert_eq!(Age::Dark.tag(), 0);
        assert_eq!(Age::Feudal.tag(), 1);
        assert_eq!(Age::Castle.tag(), 2);
        assert!(Age::Dark < Age::Feudal && Age::Feudal < Age::Castle);
        // Tags are stable and round-trip.
        for t in TechId::ALL {
            assert_eq!(TechId::from_tag(t.tag()), Some(t));
        }
        assert_eq!(TechId::from_tag(16), None);
    }

    #[test]
    fn feudal_completion_table_matches_the_wiki() {
        // All values fetched from the Age of Empires wiki,
        // 2026-10-22 (per-tech pages; see the module docs).
        // Forging: Blacksmith, 150 F, 50 s.
        assert_eq!(TechId::Forging.cost(), (150, 0, 0, 0));
        assert_eq!(TechId::Forging.ticks(), 50 * 60);
        assert_eq!(
            TechId::Forging.host(),
            TechHost::Building(BuildingKind::Blacksmith)
        );
        // Scale Barding Armor: Blacksmith, 150 F, 45 s.
        assert_eq!(TechId::ScaleBarding.cost(), (150, 0, 0, 0));
        assert_eq!(TechId::ScaleBarding.ticks(), 45 * 60);
        assert_eq!(
            TechId::ScaleBarding.host(),
            TechHost::Building(BuildingKind::Blacksmith)
        );
        // Bloodlines: Stable, 150 F / 100 G, 50 s.
        assert_eq!(TechId::Bloodlines.cost(), (150, 0, 100, 0));
        assert_eq!(TechId::Bloodlines.ticks(), 50 * 60);
        assert_eq!(
            TechId::Bloodlines.host(),
            TechHost::Building(BuildingKind::Stable)
        );
        // Double-Bit Axe: Lumber Camp, 100 F / 50 W, 25 s.
        assert_eq!(TechId::DoubleBitAxe.cost(), (100, 50, 0, 0));
        assert_eq!(TechId::DoubleBitAxe.ticks(), 25 * 60);
        assert_eq!(
            TechId::DoubleBitAxe.host(),
            TechHost::Building(BuildingKind::LumberCamp)
        );
        // Gold Mining / Stone Mining: Mining Camp, 100 F / 75 W, 30 s.
        for t in [TechId::GoldMining, TechId::StoneMining] {
            assert_eq!(t.cost(), (100, 75, 0, 0));
            assert_eq!(t.ticks(), 30 * 60);
            assert_eq!(t.host(), TechHost::Building(BuildingKind::MiningCamp));
        }
        // Horse Collar: Mill, 75 F / 75 W, 20 s.
        assert_eq!(TechId::HorseCollar.cost(), (75, 75, 0, 0));
        assert_eq!(TechId::HorseCollar.ticks(), 20 * 60);
        assert_eq!(
            TechId::HorseCollar.host(),
            TechHost::Building(BuildingKind::Mill)
        );
        // Every one of them is a Feudal Age research (TEC-07).
        for t in [
            TechId::Forging,
            TechId::ScaleBarding,
            TechId::Bloodlines,
            TechId::DoubleBitAxe,
            TechId::GoldMining,
            TechId::StoneMining,
            TechId::HorseCollar,
        ] {
            assert_eq!(t.age_required(), Age::Feudal);
        }
    }

    #[test]
    fn gather_rate_patches_scale_exactly_the_documented_percentages() {
        let none = 0u64;
        // Unpatched: the base table, whatever the mask's other bits.
        assert_eq!(
            patched_gather_rate(NodeKind::Tree, none),
            NodeKind::Tree.gather_rate()
        );
        assert_eq!(
            patched_gather_rate(NodeKind::Tree, TechId::Loom.bit()),
            NodeKind::Tree.gather_rate()
        );
        // Double-Bit Axe: wood × 120/100 — 426 bits → 511.
        assert_eq!(
            patched_gather_rate(NodeKind::Tree, TechId::DoubleBitAxe.bit()).to_bits(),
            511
        );
        // …and it touches nothing but trees.
        assert_eq!(
            patched_gather_rate(NodeKind::GoldMine, TechId::DoubleBitAxe.bit()),
            NodeKind::GoldMine.gather_rate()
        );
        // Gold Mining: × 115/100 — 415 bits → 477.
        assert_eq!(
            patched_gather_rate(NodeKind::GoldMine, TechId::GoldMining.bit()).to_bits(),
            477
        );
        // Stone Mining: × 115/100 — 393 bits → 451.
        assert_eq!(
            patched_gather_rate(NodeKind::StoneMine, TechId::StoneMining.bit()).to_bits(),
            451
        );
        // The two mining patches are independent.
        assert_eq!(
            patched_gather_rate(NodeKind::StoneMine, TechId::GoldMining.bit()),
            NodeKind::StoneMine.gather_rate()
        );
        assert_eq!(
            patched_gather_rate(NodeKind::GoldMine, TechId::StoneMining.bit()),
            NodeKind::GoldMine.gather_rate()
        );
    }

    #[test]
    fn entry_ticks_and_codes() {
        assert_eq!(
            ProdEntry::Unit(EntityKind::Villager).ticks(),
            crate::econ::VILLAGER_TRAIN_TICKS
        );
        assert_eq!(ProdEntry::Tech(TechId::Loom).ticks(), 1500);
        assert_eq!(ProdEntry::Unit(EntityKind::Archer).code(), 4);
        assert_eq!(ProdEntry::Tech(TechId::ManAtArms).code(), 105);
    }
}
