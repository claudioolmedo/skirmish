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

test('the occlusion pass skips items with neither calls nor geometry (the mechanism)', () => {
  // Fix1's pin read `if (!it.calls.length) continue;` — the
  // rule that made vector-only items invisible to INF-12.
  // Fix2 refined it: an item may still take part when it
  // carries synthesized geometry (occ.rect); what is skipped
  // is an item with no rect from either source.
  const body = fnBody('drawOccludedOutlines');
  assert.match(body, /if \(!rect\) continue;/,
    'INF-12 skips items that produced no rect — the reason a vector-only unit got no silhouette under fix1');
  assert.match(body, /if \(!rect && it\.occ\.rect\)/,
    'synthesized occluder geometry is the fallback rect source (fix2)');
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

// ------------------------------------------------------------------
// Fix2 (2026-11-02, same run): the OCCLUDER half of the defect.
// The live verification of the unit-side fix (session 26cdf5a2)
// showed the monk STILL invisible — while villagers behind the
// Town Center and a sheep behind the House DID silhouette in
// the same build. Code read: BUILDING_ART_KEY has no entries
// for kinds 16/17/20, so the Monastery (and University / Siege
// Workshop) draw through the vector fallback permanently and
// record no drawImage calls — as occluders they were skipped
// by the same call-less rule, so no silhouette could ever be
// clipped through them. Placed buildings now carry synthesized
// occluder geometry (buildingOccGeo), used only when an item
// recorded no calls.

test('art-less building kinds are exactly the vector-fallback occluders', () => {
  const m = appJs.match(/const BUILDING_ART_KEY = \{([\s\S]*?)\};/);
  assert.ok(m, 'BUILDING_ART_KEY found');
  for (const k of [16, 17, 20]) {
    assert.ok(!new RegExp(`\\b${k}:`).test(m[1]),
      `kind ${k} has no building art key (its draw is the vector fallback)`);
  }
});

test('placed buildings carry synthesized occluder geometry', () => {
  assert.match(appJs,
    /occ: \{ role: 'occluder', \.\.\.buildingOccGeo\(b\) \}/,
    'the building draw entry spreads buildingOccGeo into its occ record');
  const geo = fnBody('buildingOccGeo');
  assert.match(geo, /worldToScreen\(b\.x - half, b\.y - half\)/,
    'geometry corners go through the same projection as the draw');
  assert.match(geo, /b\.size === 3 \? 3\.2 : 2\.1/,
    'height mirrors the vector fallback box heights');
  assert.match(geo, /poly = \[up\(t\), up\(r\), r, bt, l, up\(l\)\]/,
    'the silhouette polygon is the raised footprint diamond');
});

test('the pass falls back to synthesized rects for call-less items', () => {
  assert.match(fnBody('drawOccludedOutlines'),
    /if \(!rect && it\.occ\.rect\) rect = \{ \.\.\.it\.occ\.rect \};/,
    'a call-less item with occ.rect still takes part in the pass');
});

test('the mask paints the synthesized polygon for call-less occluders', () => {
  assert.match(fnBody('paintOccludedSilhouette'),
    /if \(!o\.it\.calls\.length && o\.it\.occ\.poly\)/,
    'call-less occluders contribute their polygon to the silhouette mask');
});
