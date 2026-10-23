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
