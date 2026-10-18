# Skirmish II — The Self-Sustaining System

This document is engineering doctrine: how Skirmish II is designed to keep
running, stay faithful, and pay for itself for decades — without depending
on any single person, company, or server staying alive. The ambition is
stated plainly: a game still running in 200 years. That sounds like a
slogan until you look at the reference game's history, which is the whole
point of section 1.

The short version: the playable game needs nothing but static files, the
source and the knowledge are public, the running-cost floor is near zero by
architecture, and the revenue loop — when it activates — is contractually
pointed at the project's own costs and development, in that order.

---

## 1. The lesson: survival is a design property

Age of Empires II was released in 1999. Its studio, Ensemble, closed in
2009. Its original multiplayer servers were shut down years before that.
And yet the game never died: on 2026-10-01 it had more players on Steam
than Age of Empires IV, Age of Mythology: Retold, Company of Heroes 3,
Stormgate, and Tempest Rising **combined**.

Why? Not luck, and not nostalgia alone. The mechanics are balanced the way
chess is balanced — nothing essential needs to change, so nothing essential
was ever lost when the institutions around the game disappeared. The game
survived *because the game itself was the durable part*: the rules lived in
the players' hands and in the community's knowledge, not in a company's
continued existence.

Skirmish II takes that as its founding engineering constraint:

- **Mechanics are treated as finished mathematics, not content.** We copy
  the reference game's proven balance from public documentation and then
  *stop*. There is no churn roadmap, no seasonal rebalancing, no mechanics
  that exist to be patched later.
- **Everything else in this document exists to make the durable part
  indestructible**: no dependency that can be switched off, no knowledge
  that lives in one head, no cost that grows faster than the player base.

A game that needs its maker alive is a service. A game that doesn't is a
classic. We are building the second kind.

## 2. Zero-dependency core

The playable game is **static files**: HTML, JavaScript, and a WebAssembly
binary, served as-is. Single-player requires:

- no server-side game logic,
- no account, no login, no license check,
- no company, no API, no third-party service in the loop.

If every service connected to this project died tomorrow — the hosting, the
domain, the repository host — anyone holding a copy of the files could put
them on any static host, or open them locally, and the game would run
bit-identically. The simulation is deterministic (pure Rust, fixed-point
math, seeded RNG inside the state, per-tick state hash), so "the game"
is not a running process somewhere; it is a *function* from seed + commands
to state, and functions do not go offline.

This is deliberate. Every dependency we refuse at the core is a way the
game cannot die.

## 3. Open by construction

The source code is public, and — just as important — so is the *knowledge*:

- The complete behavioral rulebook, the gap analysis against the reference
  game, the opening spec, and the research notes all live in
  [docs/](.) and are kept current with the code.
- [ROADMAP.md](ROADMAP.md) writes down, rule by rule, what is still
  missing — so the project's unfinished work is public property, not a
  private backlog that dies with its owner.
- [CONTRIBUTING.md](../CONTRIBUTING.md) documents how to build, test, and
  submit, so a new contributor needs no one's permission or private
  briefing to pick the project up.

If the current maintainers vanished, the repository would still contain
everything needed to continue: the code, the spec it implements, the tests
that prove it, and the map of what remains. Openness here is not a
marketing position; it is the disaster-recovery plan.

## 4. A near-zero cost floor, by architecture

Survival is much easier when staying alive is cheap. The numbers today:

| Layer | Cost today | Why |
|-------|-----------|-----|
| Game hosting | **$0** | Static files on free-tier static hosting; the game is small (the WASM sim + sprites are measured in hundreds of KB) |
| Single-player | **$0** | No server at all (section 2) |
| Multiplayer relay (shipped, M8) | **$0 today** — free tier; ~$2/month class if it outgrows it | See below |

The multiplayer design is where the architecture pays for itself.
Multiplayer is **deterministic lockstep**: the simulation runs identically
on every player's machine, so a relay server only has to *forward
commands* — a few bytes per tick — and never compute, store, or even
understand game state. That is the cheapest possible thing a server can
do, which is why a single small relay can carry matches — ours (M8,
shipped 2026-10-18) runs on a free tier today and the design's
ceiling is a ~$2/month-class instance, not a fleet — and why the
per-turn state hash doubles as a free
desync detector instead of a server-side anti-cheat system.

Costs that scale with *players* are the classic way game servers die.
Lockstep makes our server cost scale with almost nothing.

## 5. The self-feeding loop

When the revenue side activates, the loop is closed and ordered:

```
players
   │
   ▼
two small, clearly labeled ad slots ──► revenue
   │                                      │
   │            100% reinvested, in order:
   │              1. running costs (hosting, relay, domain)
   │              2. the AI-token development fund
   │                                      │
   ▼                                      ▼
retention ◄── automated fidelity & maintenance cycles (section 6)
```

Rules of the loop:

- **Two ad slots, ever.** One in the lobby sidebar, one on the loading
  screen. They are labeled "Advertisement", they never overlay the game,
  and there are no interstitials. The caps are part of the doctrine: the
  loop must never be allowed to eat the game.
- **100% of ad revenue is reinvested.** First the running costs (which
  section 4 keeps near zero), then the remainder funds development —
  specifically, the AI-token budget that powers the automated fidelity
  and maintenance cycles, plus human development where tokens cannot do the job. (A paid
  remove-the-ads option may be added later for players who prefer it; it
  would feed the same loop, not a different one.)
- **Retention comes before new features.** The loop spends on making the
  game *right* — parity, feel, reliability — before making it *bigger*,
  because retention is what feeds the top of the funnel.
- **Play keeps the game faithful — it never redesigns it.** The game's
  mechanics are not a work in progress: they are the chess, finished.
  What play feeds is *convergence and survival*. From the first
  multiplayer build (M8), the client reports anonymous match telemetry —
  device and quality-tier performance, desync events, match length,
  what gets built and what wins — and players can attach a replay.
  That evidence is spent in exactly two directions: **fidelity**
  (finding where this implementation still diverges from the
  reference mechanics, and closing the gap toward them — never away
  from them) and **quality** (frame performance per device class,
  reliability, the operating health of the service). Telemetry never
  proposes new mechanics or balance changes; the rules are fixed,
  and every change passes the deterministic tests, the zero-learning
  rule, and a human review before it ships.

Honest status note: this loop's revenue side is not live yet. Ad revenue
activates with the AdSense milestone (the slots and their configuration
already exist in the client), and the relay cost becomes real with the
multiplayer milestone (M8). Until then the project's actual burn is the
$0 floor of section 4 plus the founder fronting the development tokens —
which is exactly the bootstrap the loop is designed to grow out of.

## 6. Automation principles

A system that needs a human on duty is a system with an expiry date. So:

- **Builds, tests, and deploys are scripted.** One build script produces
  the WASM package and the site; publishing is a scripted
  branch-to-branch step, not a manual file shuffle.
- **Correctness is verified by machines, not babysitting.** The
  deterministic sim makes this possible in a way most games cannot match:
  every change is checked by the test suites and by **replay hashes** —
  the same seed + commands must produce the same per-tick state hash, or
  the change does not ship. Improvement cycles can therefore run with AI
  assistance and minimal human review: the replays *are* the review.
- **Running a match needs no operator.** Single-player needs no server
  (section 2); multiplayer needs only the dumb relay of section 4.
  Nothing in the live path pages a human when it breaks — and almost
  nothing in the live path can break in a way that stops the game from
  being playable.

Put together: static files that cannot be switched off, public knowledge
that cannot be lost, costs that cannot grow teeth, a revenue loop pointed
at the project's own survival, and machines checking the machines. That is
what "built to run forever" means when you write it as engineering instead
of as a motto.
