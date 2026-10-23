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
/// Predator animals (wolves and kin — the study's class 21 analog,
/// "Predator Animals"): carried by the Wolf kind so future
/// anti-predator bonuses (e.g. the Magyar villager rule) have a
/// class to key on; no current attack row uses it.
pub const PREDATOR: u8 = 13;

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
