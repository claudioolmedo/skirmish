# The contributor system

How Skirmish II turns a player into a credited contributor — and how the
maintainer manages that flow without it eating the project. This is the
operating manual for the human side of the repository. The technical
rules live in [CONTRIBUTING.md](../CONTRIBUTING.md); this document is
about people.

## The first law (restated, because everything hangs on it)

The game is chess. **Nobody modifies the rules of the game, and nobody
creates a new version or variant of it** — not the maintainer, not a
contributor. All work converges toward documented Age of Empires II:
Definitive Edition behavior only:

- ✅ bug fixes (the game disobeys its own rules)
- ✅ fidelity gaps closed toward DE, with cited public evidence
- ✅ art toward the sprite-pipeline standard
- ✅ docs, tests, infrastructure
- ❌ rebalances, new mechanics, new modes, "improved" rules, variants —
  closed on sight, kindly, with a pointer to this law

Any issue or PR that proposes a behavior change must cite the public DE
evidence for it (the *Fidelity report* template enforces the format).
No citation, no behavior change — the issue is parked as a `question`
until evidence arrives.

## The funnel

```
PLAY → STAR → FEEDBACK → ISSUE → CLAIM → PR → CREDIT
```

1. **Play.** skirmish.online — no install, no account. The game is the
   top of the funnel; everything else is downstream of a real match.
2. **Star.** The end-of-match card and the site ask for a GitHub star.
   A star is a bookmark and a public signal — it is how strangers find
   the repo.
3. **Feedback.** The end-of-match card collects a star rating + "what
   broke for you?" into the project's own relay. Players who want more
   are pointed to the GitHub *Player feedback* issue template.
4. **Issue.** Feedback with a reproduction becomes a *Bug report*; "in
   DE it works differently" becomes a *Fidelity report* (evidence
   required); anything actionable and small gets scoped, labeled
   `good first issue`, and put in the claim queue.
5. **Claim.** First come, first served — see the claim rules below.
6. **PR.** The PR template enforces the first law + the determinism
   checklist. Review is a conversation, not a wall.
7. **Credit.** Merged work is credited in the README and in the release
   notes of the milestone it lands in. **3 merged PRs** → the
   contributor is listed in the site footer credits on skirmish.online.
   This is standing policy, not a favor.

## Label taxonomy

| Label | Meaning |
|---|---|
| `bug` | The game disobeys its own rules — crash, freeze, wrong behavior |
| `fidelity` | A difference from DE behavior (evidence cited or requested) |
| `feedback` | Player experience reports — fun, feel, confusion |
| `good first issue` | Scoped, safe, claimable by a newcomer |
| `help wanted` | Maintainer actively wants this picked up |
| `art` | Sprites, the sprite pipeline, visuals |
| `docs` | Documentation only — rulebook, guides, README |
| `sim` | The deterministic simulation (`sim/`) |
| `client` | The browser client (`web-preview/`) |
| `question` | Needs information before it can be anything else |
| `duplicate` | Already reported — closed with a link to the original |
| `wontfix` | Out of scope (usually the first law) — closed with the pointer |

Area labels (`sim`, `client`, `art`, `docs`) combine with kind labels
(`bug`, `fidelity`, `feedback`). A claimable issue always carries
`good first issue` + one area label.

## Triage rhythm

- An **automated daily process** collects everything new — in-game
  feedback and star ratings from the relay, new GitHub issues and PRs,
  new stars — into one digest for the maintainer.
- The maintainer's commitment, stated in CONTRIBUTING.md: a **triage
  reply within a day** on every new issue and PR (a label, a question,
  an assignment, or a close-with-pointer), and PR reviews within a few
  days.
- Weekly, the maintainer sweeps: stale claims past their window are
  freed, `good first issue` stock is topped back up from the rulebook's
  DIFFERS/MISSING entries (docs/tests/infra scopes only, or DE-evidenced
  fidelity fixes), and landed work is credited.

## Claiming — the rules

1. Comment on the issue: *"I'd like to work on this."*
2. The maintainer assigns you — **first come, first served**. One
   active claim per person.
3. You have **~2 weeks** to open a PR; a draft PR counts and resets the
   clock's spirit (the work is visibly alive). No PR and no comment in
   that window → the issue is unassigned and returns to the queue.
4. Need more time or get stuck? Comment. Asking is never punished;
   silence is what frees the issue.
5. A PR that arrives for an unclaimed, unassigned issue is still read —
   but if it collides with an active claim, the claim wins.

## Welcome comment (template)

Maintainers paste/adapt this on a newcomer's first issue or claim:

> Welcome — and thank you for playing Skirmish II. 🏰
>
> Quick orientation: this project's mechanics are finished like chess;
> we only move behavior *toward* documented Age of Empires II DE
> behavior, with public evidence — [CONTRIBUTING.md](https://github.com/claudioolmedo/skirmish/blob/main/CONTRIBUTING.md)
> explains the first law in two minutes.
>
> [For a claim:] You're assigned — first come, first served. You have
> about two weeks to open a PR (a draft is fine and shows the work is
> alive). Stuck or short on time? Just comment here; asking is never a
> problem.
>
> [For a report:] This is triaged as `<label>`. [What happens next:
> e.g. "If you can add the DE source for this behavior, it goes to the
> fast lane."]

## Escalation

- **Rule-change / variant proposals** → one kind reply quoting the
  first law, label `wontfix`, close. Not a debate thread; the law is
  the project's reason to exist. If the proposal carries genuine DE
  evidence, it is not a rule change — re-label `fidelity` and treat it
  as gold.
- **Spam** (ads, link drops, empty AI-generated issues with no
  reproduction or evidence) → close + lock, no extended discussion.
- **Abuse / harassment** → Code of Conduct enforcement ladder
  (correction → warning → temporary ban → permanent ban); offending
  threads are locked and, where GitHub's tools require it, reported.
- **Security issues** (a way to cheat in online matches, a relay
  vulnerability) → the maintainer may move the discussion private
  until a fix ships; the reporter is credited in the fix notes unless
  they prefer otherwise.
- **Licensing questions** (the project's license is still being
  finalized by the maintainer) → label `question`, assign to the
  maintainer; contributors are never asked to guess at legal terms.
