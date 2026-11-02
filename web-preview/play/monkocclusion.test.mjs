// Monk occlusion regression pins (bot-loop, 2026-11-02) — the
// "Train Monk produces nothing visible" defect, chased through
// four live sessions (2026-11-01 runs 09:22 → 23:22).
//
// Root cause, proven this run: a trained unit spawns on the
// first free ring cell in row-major order — the NW corner cell
// behind its building (sim probe: monk at Monastery anchor
// (30,30) spawns at cell (29,29), rel (-1,-1); a Militia at a
// Barracks does the same). The Monastery's roof then paints
// over the monk in the sorted pass. For sprite units that is
// fine: INF-12 (recordFrameSprites / drawOccludedOutlines)
// repaints the hidden part as an owner-colour silhouette. But
// the pass can only see units whose draw records drawImage
// calls, and drawMonk was pure vector — zero recorded calls,
// and drawOccludedOutlines skips call-less items outright
// (`if (!it.calls.length) continue;`). So the monk alone got
// no silhouette: live testers saw only the hood tip and the
// staff's gold cross peeking above the roof's north vertex,
// and box-selects over the building found nothing.
//
// The fix paints the identical vector figure once into a
// cached offscreen sprite per sash colour and blits it with
// ctx.drawImage, so the recorder captures the monk like any
// sprite unit. These pins keep the three layers from drifting
// apart again: the mechanism (skip rule), the paint path
// (drawImage through the cache), and the figure itself
// (original colours/shapes in paintMonkFigure).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function fnBody(name) {
  const m = appJs.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n\\}`));
  assert.ok(m, `${name} found in app.js`);
  return m[0];
}

test('the occlusion pass still skips call-less items (the mechanism)', () => {
  assert.match(fnBody('drawOccludedOutlines'),
    /if \(!it\.calls\.length\) continue;/,
    'INF-12 skips items with no recorded drawImage calls — the reason a vector-only unit gets no silhouette');
});

test('drawMonk blits a cached sprite through drawImage', () => {
  const body = fnBody('drawMonk');
  assert.match(body, /monkSprite\(sash\)/,
    'drawMonk obtains the cached monk sprite');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'drawMonk records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintMonkFigure\(ctx, sx, sy, u, sash\)/,
    'vector fallback retained for environments without a canvas');
});

test('the monk sprite is painted once per sash colour and cached', () => {
  const body = fnBody('monkSprite');
  assert.match(body, /monkSpriteCache\.has\(sash\)/,
    'cache is consulted before painting');
  assert.match(body, /monkSpriteCache\.set\(sash, cv\)/,
    'painted sprite is stored per sash colour');
  assert.match(body, /paintMonkFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintMonkFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintMonkFigure');
  for (const colour of ['#4a3826', '#d8b34a', '#6b4a2e', '#54381f', '#e8c39a']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (staff, cross, robe, hood, face)`);
  }
  assert.match(body, /g\.ellipse\(sx, sy - 0\.85 \* u, 0\.62 \* u, 0\.82 \* u/,
    'the robe bell keeps its geometry');
});

test('the unit draw table still routes kind 20 to drawMonk', () => {
  assert.match(appJs, /drawMonk\]; \/\/ 20: Monk/,
    'UNIT_DRAW[20] remains drawMonk');
});
