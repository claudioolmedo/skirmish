# Suggestions for improving Skirmish II

These are suggestions from a review of revision `84fb98e`, not implemented features or confirmed defects. The aim is to close documented fidelity gaps and make existing behavior easier to verify. Rule changes require public evidence for the targeted DE version, as described in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Suggested priorities

| Priority | Suggestion | Starting point | Acceptance criteria |
|---|---|---|---|
| 1 | Reconcile the documentation with the shipped simulation | [Gap analysis](GAP-ANALYSIS.md), [roadmap](ROADMAP.md), [rulebook](MECHANICS-RULEBOOK.md) | Remove stale claims that combat and HP are absent. Each updated status names its implementation and relevant tests, with a review revision. |
| 2 | Verify damage composition at low damage values | `sim/src/combat.rs`: `compute_damage_elev`, `apply_hit`; CMB-01/02/13 | Establish the publicly documented order of elevation, mishit reduction and minimum damage. Cover high armor, fractional HP, exactly 1 HP and death below 1 HP. |
| 3 | Add a blast-damage policy | `Projectile`, `resolve_projectile`; PRJ-07, CMB-12 | Specify radius, target footprint, boundary inclusion, falloff, primary-target exceptions and per-candidate accuracy. Test inside/on/outside the boundary and multiple candidates. |
| 4 | Make attack eligibility explicit | `resolve_projectile`; CMB-12, PRJ-09 | Distinguish ordinary shots, blast and pass-through. Specify own/allied/enemy eligibility independently for each policy; test directed relations when diplomacy becomes available. |
| 5 | Complete Ballistics and review collision along a flight path | `resolve_strike`, `resolve_projectile`; PRJ-05/06/09 | Gate prediction on the technology. Cover stationary, crossing and retreating targets. Evaluate segment collision for fast shots and pass-through against public behavior before changing endpoint collision. |
| 6 | Build garrison occupancy before extra-arrow logic | GAR-01…08; entity state, production/rally and hashing | Specify admission, capacity, occupancy, unloading and destruction first. Then add healing and arrow contributions, with full-capacity and invalid-target cases. |

The first two suggestions can be small contributions. Blast and eligibility should be designed together; garrison is a separate subsystem. These priorities do not replace the project's roadmap.

## A small first investigation

Currently, `compute_damage_elev` applies a minimum of 1, and `apply_hit` subsequently halves a mishit. This produces 0.5 damage when the full hit is 1. Verify the intended DE behavior before calling this a bug or moving the clamp. A focused regression case is more useful than an assumed correction.

For production and technology work, extend the existing systems rather than creating replacements. Useful regression scenarios include cancelling the active versus a waiting queue entry, destroying a producer, upgrading a wounded unit, and completing training while population is blocked. Specify the expected payment, refund, progress and HP outcomes from public evidence first.

## Keep the implementation reviewable

- Implement one rule or a small dependency group per change; cite its rulebook IDs and public behavior source.
- Keep fixed-point arithmetic and deterministic candidate ordering. Define when an RNG draw occurs, including misses and rejected targets.
- Include new gameplay state in the state hash and exercise it through replay tests.
- Add a positive case, a rejection case and a boundary case for the behavior being changed. Use the existing simulation fixtures.
- Update the relevant rulebook status only after implementation and verification; retain explicit gaps and unverified values.

Documentation-only changes need link and consistency checks. Gameplay changes should pass `cargo test`, the project's determinism checks and the WebAssembly build; client changes also need the checks in [CONTRIBUTING.md](../CONTRIBUTING.md).

No gameplay code or balance values are changed by this proposal.

## Maintainer disposition (2026-10-22)

Priorities 1 and 2 were executed the same day this review landed.

- **Priority 1 — done.** The gap analysis was re-audited against the
  shipped code at `0f303ca` (every corrected status names its
  implementation and tests), the roadmap scoreboard now matches the
  rulebook's mechanical recount (218 rules: 119 MATCHES / 33 DIFFERS /
  66 MISSING), and the rulebook's own Appendix A table was corrected
  where the recount found it stale (§7: 12/4/1).
- **Priority 2 — done; the flagged case was a real bug.** Public DE
  documentation composes all multipliers before the minimum clamp
  (wiki, *Attack*: `Damage = max(1, k_ele × k_miss × …)`; wiki,
  *Armor class*, steps 5–6: "the minimum damage rule is applied after
  all the attack multipliers" — both fetched 2026-10-22). Our code
  clamped first and halved after, so a mishit of a full-1 hit dealt
  0.5. Fixed in `compute_damage_hit` (`sim/src/combat.rs`), with
  regression tests covering high armor (Σ = 0), fractional HP,
  exactly 1 HP, death below 1 HP, the mishit-at-full-1 case and
  elevation × mishit composition; CMB-01/CMB-13 record the verified
  order. The golden hash did not move.
- **Priority 3 (blast-damage policy), Priority 4 (attack eligibility)
  and Priority 5 (Ballistics + flight-path collision) — design
  follow-ups, not yet scheduled as code.** They are tied to the Siege
  (Castle Age package P4) and University/Ballistics work: blast and
  pass-through projectiles do not exist in the roster yet, and
  eligibility only becomes multi-policy once they do. Each will be
  specified from public evidence before any code, per this file's
  own hygiene rules.
- **Priority 6 (garrison) — accepted, folded into the Castle Age P3
  package design**: occupancy (admission, capacity, unloading,
  destruction) is specified before arrows and healing, as suggested.
  Sequencing inside that package is pending the maintainer's
  garrison decision.
