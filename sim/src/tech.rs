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

use crate::build::BuildingKind;
use crate::entity::EntityKind;
use crate::units;

/// The ages a player can stand in (M6 ships the first step of the
/// TEC-01 ladder; Castle/Imperial are later milestones). Ordered:
/// `Dark < Feudal`, so age requirements are plain comparisons.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Age {
    #[default]
    Dark,
    Feudal,
}

impl Age {
    /// Stable tag for the state hash and the WASM ABI:
    /// 0 = Dark Age, 1 = Feudal Age.
    pub fn tag(self) -> u8 {
        match self {
            Age::Dark => 0,
            Age::Feudal => 1,
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
}

impl TechId {
    /// All technologies, in tag order.
    pub const ALL: [TechId; 7] = [
        TechId::Loom,
        TechId::FeudalAge,
        TechId::Fletching,
        TechId::ScaleMail,
        TechId::PaddedArcherArmor,
        TechId::ManAtArms,
        TechId::Wheelbarrow,
    ];

    /// Stable tag: 0 loom, 1 feudal age, 2 fletching, 3 scale mail,
    /// 4 padded archer armor, 5 man-at-arms, 6 wheelbarrow.
    pub fn tag(self) -> u8 {
        match self {
            TechId::Loom => 0,
            TechId::FeudalAge => 1,
            TechId::Fletching => 2,
            TechId::ScaleMail => 3,
            TechId::PaddedArcherArmor => 4,
            TechId::ManAtArms => 5,
            TechId::Wheelbarrow => 6,
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
            TechId::Loom | TechId::FeudalAge | TechId::Wheelbarrow => TechHost::TownCenter,
            TechId::Fletching | TechId::ScaleMail | TechId::PaddedArcherArmor => {
                TechHost::Building(BuildingKind::Blacksmith)
            }
            TechId::ManAtArms => TechHost::Building(BuildingKind::Barracks),
        }
    }

    /// The age the researching player must stand in (TEC-07). The
    /// age advancement itself is researched *from* the Dark Age.
    pub fn age_required(self) -> Age {
        match self {
            TechId::Loom | TechId::FeudalAge => Age::Dark,
            _ => Age::Feudal,
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
        }
    }
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
        // Tags are stable and round-trip.
        for t in TechId::ALL {
            assert_eq!(TechId::from_tag(t.tag()), Some(t));
        }
        assert_eq!(TechId::from_tag(7), None);
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
