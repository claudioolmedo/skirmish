//! Armor classes (M3) — the damage taxonomy.
//!
//! Following MECHANICS-RULEBOOK CLS-01/CMB-03: every attack is a list of
//! (class, value) pairs; a pair only contributes damage when the target
//! *belongs* to that class, and class membership is a bitmask computed
//! from the unit's table row — never a sentinel armor value. The melee
//! and pierce classes are special: every unit has an armor value for
//! them (possibly 0, possibly negative later), so those pairs always
//! participate, mitigated by the matching armor.
//!
//! The ids below are Skirmish's own compact numbering (the reference
//! game's engine ids are documented in MECHANICS-STUDY §1.3 for
//! orientation, but our tables are self-contained data).

/// Damage classes carried by attacks, and membership classes carried
/// by units. Ids are bit positions in the membership mask.
pub const MELEE: u8 = 0;
pub const PIERCE: u8 = 1;
pub const INFANTRY: u8 = 2;
pub const ARCHERY: u8 = 3;
pub const CAVALRY: u8 = 4;
pub const SPEARMAN: u8 = 5;
pub const VILLAGER: u8 = 6;
pub const ALL_BUILDINGS: u8 = 7;
pub const STANDARD_BUILDINGS: u8 = 8;
/// Boars and other hunted animals (the study's class 24 analog).
pub const BOAR_HUNTED: u8 = 9;
pub const SHOCK_INFANTRY: u8 = 10;
pub const CAMEL: u8 = 11;
pub const HERDABLE: u8 = 12;
/// Elephant units (the study's class 18 analog). No elephant unit
/// ships yet; the class exists so the Spearman line's documented
/// +15 bonus row is data, exactly like the Boar's CAMEL row was
/// data before any camel existed.
pub const ELEPHANT: u8 = 13;
/// Ships (the study's class 16 analog) — same data-first rationale
/// as ELEPHANT: the Spearman's +9 vs Ships row is documented DE
/// data with no member kind in the game yet.
pub const SHIP: u8 = 14;
/// Skirmishers (the study's class 38 analog): the Skirmisher's own
/// membership beside ARCHERY — the class future anti-skirmisher
/// rows (e.g. the Genitour's) target.
pub const SKIRMISHER: u8 = 15;
/// Walls and gates (BLD-13): the membership the reference gives
/// wall segments on top of All/Standard Buildings. No attack row
/// in the roster carries a bonus against it yet — the membership
/// is data, ready for the rows that use it.
pub const WALLS_AND_GATES: u8 = 16;
/// Predator animals (wolves and kin — the study's class 21 analog,
/// "Predator Animals"): carried by the Wolf kind so future
/// anti-predator bonuses (e.g. the Magyar villager rule) have a
/// class to key on; no current attack row uses it. (Numbered 17
/// on landing: 13–16 were taken on main by ELEPHANT, SHIP,
/// SKIRMISHER and WALLS_AND_GATES while the wolf branch waited.)
pub const PREDATOR: u8 = 17;

/// Membership bit for one class id.
pub const fn bit(class: u8) -> u64 {
    1u64 << class
}

/// A membership mask from a list of class ids (const-friendly).
pub const fn mask(classes: &[u8]) -> u64 {
    let mut m = 0u64;
    let mut i = 0;
    while i < classes.len() {
        m |= bit(classes[i]);
        i += 1;
    }
    m
}
