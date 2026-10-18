# Skirmish II — AdSense activation runbook

Goal: turn on real AdSense ads in the existing slots (see `ADS.md` for the
slot map and rules). Nothing here changes the two-slot rule, the labels, or
the never-over-the-game rule.

## Step 1 — Owner steps (Claudio only)

Only the site owner can do these; they need his own accounts and documents.

- [ ] Create the AdSense account at <https://www.google.com/adsense> using
      his own Google account.
- [ ] Complete the identity verification and tax information in AdSense
      (individual, US resident).
- [ ] In AdSense, add the site `skirmish.online` (Sites → Add site).
- [ ] Copy from the AdSense console:
  - the publisher ID (`pub-XXXXXXXXXXXXXXXX`), and
  - the AdSense verification code snippet (contains `ca-pub-…`).
- [ ] Hand both to Muse for Step 2.

## Step 2 — Integration steps (Muse, after Step 1)

- [ ] Edit `web-preview/ads.js` — `CONFIG` block only:
  - `clientId` → the real `ca-pub-…` publisher ID,
  - `slots["ad-slot-leaderboard"]` → the leaderboard unit's slot ID,
  - `slots["ad-slot-play"]` → the beside-the-game unit's slot ID,
  - `slots["ad-slot-lobby"]` → the lobby unit's slot ID,
  - `enabled` → `true`.

    (Create the three Display ad units in the AdSense console first if
    their slot IDs are not known yet: one responsive/leaderboard, two
    300×250 — see `ADS.md`. With `clientId` set and `enabled: true`,
    `ads.js` loads Google's library, which also serves as the site's
    verification code.)
- [ ] Create `web-preview/ads.txt` (publishes as `https://skirmish.online/ads.txt`)
      with exactly one line, using the real publisher ID from Step 1:

      ```
      google.com, pub-XXXXXXXXXXXXXXXX, DIRECT, f08c47fec0942fa0
      ```
- [ ] Link the Privacy Policy in the landing footer: in
      `web-preview/index.html`, add `<a href="privacy/">Privacy Policy</a>`
      to the footer's link row (page itself: `web-preview/privacy/index.html`).
- [ ] Publish `web-preview/` → `gh-pages` as usual, preserving the `CNAME`
      file.
- [ ] Back in the AdSense console, request review for `skirmish.online`
      (if the console offers ads.txt-based verification, confirm it reads
      the published file).
- [ ] After approval: load <https://skirmish.online/> and
      <https://skirmish.online/play/> and verify real ads render in all
      three slots, the "Advertisement" labels remain, and no slot covers
      the game. Verify `https://skirmish.online/ads.txt` returns the line.

To pause ads at any time: set `enabled` back to `false` in `ads.js` and
publish — the house placeholders return.

## Step 3 — Review readiness

How the site stands against what the AdSense site review looks for:

| Requirement | State |
| --- | --- |
| Original content | ✓ Clean-room game and art, written from scratch; project page content is original. |
| Working, substantial site | ✓ Playable test build live at `/play/`; landing page with real content, roadmap, and documentation. |
| Clear navigation | ✓ Header nav (Play, Why Skirmish II, Roadmap, GitHub) on the landing page. |
| No prohibited content | ✓ General-audience strategy game; nothing sexual, violent-graphic, deceptive, or copied. No pop-ups or interstitials; ads never overlay the game. |
| Custom domain with HTTPS | ✓ `skirmish.online` on GitHub Pages, HTTPS enforced. |
| Privacy policy page | ✓ Page written (`web-preview/privacy/index.html`) — **TODO:** footer link + publish (Step 2), so reviewers can reach it from the site. |
| `ads.txt` | **TODO** — created in Step 2, once the real publisher ID exists. |
| AdSense code on the site | **TODO** — `ads.js` still ships placeholder IDs with `enabled: false` until Step 2. |
