// Diplomacy panel unclosable — regression pins (bot-loop, 2026-11-05).
//
// Defect (live two-client INF-04 proof on the real client, build
// e794788): at a 1919x998 viewport the Diplomacy panel could not be
// closed at all — its ✕ sat above the visible viewport, Escape did
// not close it, and the top-bar Diplomacy button is under the
// overlay's own backdrop while the panel is open. The only exit was
// a page reload.
//
// Root causes, both in the client shell:
//  1. `.overlay` centers its card with flex `align-items:center`
//     plus `overflow-y:auto`: a card taller than the viewport (the
//     Diplomacy card carries the stance table + the tribute desk)
//     overflows symmetrically and its TOP — where `.card-close`
//     lives — lands above the reachable scroll area. The standard
//     fix is auto margins on the card: free space centers it, and
//     on overflow the margins collapse to 0 and the card aligns to
//     the top, fully scrollable.
//  2. The Escape stack (Claudio's 2026-10-26 design: step 2 is
//     "close a transient panel") only knew the Help overlay — the
//     live panels (Diplomacy / Score / Technology Tree), which are
//     transient by their own docs ("a live panel, never a pause"),
//     were missing, so Esc fell through to deselect and the panel
//     stayed up.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const playCss = readFileSync(join(here, 'play.css'), 'utf8');

test('overlay cards use auto margins so tall cards stay reachable', () => {
  const m = playCss.match(/\.overlay-card\s*\{[^}]*\}/);
  assert.ok(m, '.overlay-card rule found in play.css');
  assert.ok(/margin:\s*auto/.test(m[0]),
    'the card centers via auto margins (top stays scrollable on overflow)');
});

test('the overlay centers SAFELY so an overflowing card aligns to the top', () => {
  // The first fix attempt (auto margins alone) failed live: with
  // negative free space the margins collapse to 0 and the unsafe
  // `align-items: center` centering still clips the card's top.
  // `safe center` is the alignment that falls back to start.
  const m = playCss.match(/\.overlay\s*\{[^}]*\}/);
  assert.ok(m, '.overlay rule found in play.css');
  assert.ok(/align-items:\s*safe center/.test(m[0]),
    'cross-axis centering is safe (top reachable on overflow)');
  assert.ok(!/align-items:\s*center;/.test(m[0]),
    'no unsafe plain centering remains on the cross axis');
});

test('Escape closes the live panels before deselecting', () => {
  const esc = appJs.match(/if \(e\.code === 'Escape'\) \{[\s\S]*?\n    return;\n  \}/);
  assert.ok(esc, 'Escape branch found in app.js');
  const body = esc[0];
  for (const [overlay, closer] of [
    ['diploOverlay', 'setDiplo(false)'],
    ['scoreOverlay', 'setScore(false)'],
    ['techtreeOverlay', 'setTechTree(false)'],
  ]) {
    assert.ok(body.includes(`${overlay}.style.display !== 'none'`),
      `Escape checks whether ${overlay} is open`);
    assert.ok(body.includes(closer), `Escape closes via ${closer}`);
    assert.ok(
      body.indexOf(closer) < body.indexOf('selectUnits(new Set())'),
      `${closer} runs before the deselect step`);
  }
});

test('the Diplomacy close button and toggle stay wired to setDiplo', () => {
  assert.ok(appJs.includes("diploClose.addEventListener('click', () => setDiplo(false))"),
    'the ✕ closes the Diplomacy panel');
  assert.ok(appJs.includes("setDiplo(diploOverlay.style.display === 'none')"),
    'the top-bar Diplomacy button toggles the panel');
});
