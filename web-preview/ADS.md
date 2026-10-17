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

| Slot element id | Position | Size |
| --- | --- | --- |
| `ad-slot-leaderboard` | Under the hero, above the Play section | 728×90, collapsing to 320×50 on small screens |
| `ad-slot-play` | Beside the game frame in the Play section (stacks underneath on narrow screens) | 300×250 |

Both slots are declared in `index.html`; styling lives in `site.css`;
behavior and configuration live in **`ads.js` — the only file that needs
editing to activate real ads.**

## How it works today

`ads.js` ships with `enabled: false` and placeholder ids, so every slot
renders a **house placeholder**: a friendly message ("Your friendly ad will
live here soon — for now, thanks for playing!") with a link to the GitHub
repo. The layout, labels and reserved sizes are already final, so swapping
in real ads causes no layout shift and no page redesign.

## Activating AdSense (once approved)

1. In the AdSense dashboard, confirm the site `skirmish.online` is approved.
2. Create two ad units (Display ads):
   - one responsive/leaderboard unit for the under-hero slot,
   - one 300×250 (medium rectangle) unit for the beside-the-game slot.
3. Open `web-preview/ads.js` and edit the `CONFIG` block at the top:
   - `clientId`: your publisher id (`ca-pub-…`),
   - `slots["ad-slot-leaderboard"]`: the leaderboard unit's slot id,
   - `slots["ad-slot-play"]`: the rectangle unit's slot id,
   - `enabled`: set to `true`.
4. Commit and publish as usual (`web-preview/` → `gh-pages`).

That is the only change needed. `ads.js` then loads the AdSense library
once, fills both slots, and the house placeholders disappear. To pause ads
at any time, set `enabled` back to `false` — the friendly placeholders
return instantly.

## Notes

- Revenue follows the project doctrine: basics and lobby first, ad revenue
  reinvested into development.
- If the provider ever changes, implement it inside `ads.js` behind the
  same config shape; the page markup and the two-slot rule stay untouched.
