<!--
  Skirmish II — pull request checklist. The gate line below is mandatory:
  a PR that cannot honestly tick it will be closed, however good the code.
-->

## The gate (mandatory)

- [ ] **This PR does not change any game rule, balance value, or mechanic — except to match documented Age of Empires II DE behavior, cited below. Rule changes, rebalances, and new variants/modes are out of scope in this project and will be closed.** (The mechanics are finished, like chess. See CONTRIBUTING.md, "The first law".)

## What changed

<!-- One or two sentences. If this implements rulebook rules, name the
     rule IDs (e.g. "Implements ECO-09") and update the rule's status in
     docs/MECHANICS-RULEBOOK.md in this same PR. -->

## Fidelity note

<!-- If any behavior changed: what does DE do, what did we do before,
     what do we do now — and the public source that proves DE's behavior
     (wiki link, guide, video timestamp). Public documentation only.
     If no behavior changed, write "No behavior change." -->

## How tested

- [ ] `cargo test` — full sim suite green
- [ ] Golden hash unchanged — OR changed, and here is exactly why (which state/behavior moved, and the test that proves the new hash is the intended one):
- [ ] Node suites for the client pass (`node --test web-preview/play/*.test.mjs` and any smoke I touched)
- [ ] Played it: I ran the preview (`./web-preview/build.sh` + static server) and exercised the change in a real match — OR this is a docs/tests-only change

## Scope check

- [ ] No game assets, code, text, or data copied from any existing game (clean room — public documentation only)
- [ ] New sim state, if any, is covered by the state hash and a determinism test
- [ ] Docs updated in the same PR where a status or fact changed
