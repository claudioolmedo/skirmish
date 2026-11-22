//! Hero self-regeneration (CLS-06): the tick phase that reads
//! the unit row's `regenerates` column (HP per minute).
//!
//! In the reference, hero-mode bit 4 makes a hero regenerate
//! 30 HP per minute (0.5 HP/s), unconditionally — no idle or
//! out-of-combat condition is documented (wiki, Hero;
//! wiki, Regeneration). The phase mirrors the GAR-04 shelter
//! accumulator exactly: per tick it adds `rate × 65536` (the
//! HP/min rate in fixed-point bits) to a conceptual accumulator
//! whose quotient over 3600 (ticks per minute at 60 ticks/s) is
//! the HP restored, keeping the remainder on the entity
//! (`Entity::regen_remainder`) — so a rate-30 hero gains exactly
//! 30 HP in exactly 3 600 ticks, deterministically, and the heal
//! stacks with monk healing and (for a fielded hero) any other
//! heal source, as the reference's stacking rule requires
//! (Claudio's 2026-11-16 ruling, sk-17 D4).
//!
//! Dead and contained (garrisoned / transported) entities are
//! skipped: a contained unit is out of the field (GAR-02), and
//! inside a host the shelter rates are the host's business.
//! A unit already at full HP accrues nothing (remainder reset,
//! the GAR-04 shape). Rows with rate 0 — every row but the hero
//! subject's, the King's included — are a true no-op, so no
//! default-match stream changes by a single bit.

use fixed::types::I16F16;

use crate::Sim;

impl Sim {
    pub(crate) fn regen_tick(&mut self) {
        for idx in 0..self.entities.len() {
            let rate = crate::units::spec(self.entities[idx].kind).regenerates as i64;
            if rate <= 0 {
                continue;
            }
            let e = &self.entities[idx];
            if e.dead || e.is_contained() {
                continue;
            }
            let max_hp = I16F16::from_num(self.unit_stats(idx).max_hp);
            let e = &mut self.entities[idx];
            if e.hp >= max_hp {
                e.regen_remainder = 0;
                continue;
            }
            let total = e.regen_remainder + rate * 65_536;
            let heal_bits = total / 3600;
            e.regen_remainder = total % 3600;
            e.hp += I16F16::from_bits(heal_bits as i32);
            if e.hp > max_hp {
                e.hp = max_hp;
            }
        }
    }
}
