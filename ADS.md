# Skirmish II — advertising

Skirmish II is open. A small amount of advertising pays for the servers and
keeps development going. This document explains exactly how it works, where
the ads are, and the rules they follow — the same transparency visitors get
on the page itself.

## The friendly rules (non-negotiable)

1. **Maximum two ad slots on the page.** There will never be more.
2. **Always labeled.** Every slot sits under a visible "Advertisement"
   caption, with a note that ads pay for the servers and keep Skirmish II open.
3. **Never over the game.** No slot covers, overlays or floats above the
   canvas; there are no interstitials, pop-ups, pop-unders, autoplay
   video ads, or ads that block play in any way.
4. **Honest notice.** The dismissible banner at the top of the page tells
   visitors plainly that ads pay for the servers. No dark patterns, no
   guilt-tripping.

## Where the slots are

| Slot element id | Page | Position | Size |
| --- | --- | --- | --- |
| `ad-slot-leaderboard` | Landing (`index.html`) | Under the hero, above the Play section | 728×90, collapsing to 320×50 on small screens |
| `ad-slot-play` | Landing (`index.html`) | Beside the game frame in the Play section (stacks underneath on narrow screens) | 300×250 |
| `ad-slot-lobby` | Play (`play/index.html`) | Right sidebar of the Lobby Browser — beside the open-games table on desktop, stacked below it on narrow screens | 300×250 |
| `ad-slot-loading` | Play (`play/index.html`) | The loading screen, under the players' loaded states | 300×250 |

The two-slots-per-page rule holds on every page: the landing carries
two, the play page carries two — both of them **waiting-view slots**
(the lobby and the loading screen), never on screen at the same
time, and both gone the moment a match starts, so an ad is never on
screen during play and never over the game canvas.

The landing slots are declared in `index.html`, the play-page slots
in `play/index.html`; styling lives in `site.css`; behavior and
configuration live in **`ads.js` — the only file that needs editing
to activate real ads.**

## Rotation during waits (M10)

Waits are the product's longest screens — in the reference game a
player may sit 5–15 minutes in a lobby or loading view before a
10–60 minute match — so the two waiting slots **rotate**: a fresh
creative every **60 seconds** (`ROTATE_MS` in `ads.js`), on one
shared timer. Rotation **pauses while the tab is hidden** and
**stops entirely the moment a match starts** (both slots leave the
screen with their overlays, and the rotator only ever touches
visible slots).

- **House phase (today): unrestricted.** The rotation alternates
  the designed Skirmish II promo with the friendly house
  placeholder — two house creatives cycling, so the wait reads
  alive and the mechanism is visibly working. No fake third-party
  advertisers, ever.
- **Real ads: policy check first.** Rotating a real AdSense unit
  is an *auto-refresh*, and AdSense policy constrains it (minimum
  refresh intervals — historically 60 s — visibility requirements,
  no refreshing of hidden units; the terms change over time).
  `ads.js` therefore ships with `CONFIG.rotateRealAds: false`:
  when real ads activate, the waiting slots serve one unit each
  and do **not** rotate until the integrator has confirmed the
  **current** AdSense auto-refresh policy and deliberately set
  `rotateRealAds: true` (adjusting `ROTATE_MS` if the policy's
  minimum interval is higher). Getting this wrong risks the
  account; the house rotation needs no such check.

## How it works today

`ads.js` ships with `enabled: false` and placeholder ids, so every slot
renders a **house placeholder**: a friendly message ("Your friendly ad will
live here soon — for now, thanks for playing!") with a link to the GitHub
repo. The two waiting slots render a designed **promo for the game
itself** (emblem, headline, button) that fills its 300×250 box like a
real unit, so the placements can be judged realistically — it never
pretends to be a third-party sponsor — and they rotate between that
promo and the friendly message while the player waits (see
"Rotation during waits"). The layout, labels and reserved sizes are
already final, so swapping in real ads causes no layout shift and no
redesign.

## Activating AdSense (once approved)

1. In the AdSense dashboard, confirm the site `skirmish.online` is approved.
2. Create four ad units (Display ads):
   - one responsive/leaderboard unit for the under-hero slot,
   - one 300×250 (medium rectangle) unit for the beside-the-game slot,
   - one 300×250 (medium rectangle) unit for the lobby sidebar slot,
   - one 300×250 (medium rectangle) unit for the loading-screen slot.
3. Open `web-preview/ads.js` and edit the `CONFIG` block at the top:
   - `clientId`: your publisher id (`ca-pub-…`),
   - `slots["ad-slot-leaderboard"]`: the leaderboard unit's slot id,
   - `slots["ad-slot-play"]`: the rectangle unit's slot id,
   - `slots["ad-slot-lobby"]`: the lobby unit's slot id,
   - `slots["ad-slot-loading"]`: the loading unit's slot id,
   - `enabled`: set to `true`.
   - Leave `rotateRealAds` at `false` until the rotation policy
     check above has been done.
4. Commit and publish as usual (`web-preview/` → `gh-pages`).

That is the only change needed. `ads.js` then loads the AdSense library
once, fills the slots, and the house placeholders disappear. To pause ads
at any time, set `enabled` back to `false` — the friendly placeholders
return instantly.

## Notes

- Revenue follows the project doctrine: basics and lobby first, ad revenue
  reinvested into development.
- If the provider ever changes, implement it inside `ads.js` behind the
  same config shape; the page markup and the two-slot rule stay untouched.
