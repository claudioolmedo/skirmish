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

// ---------------------------------------------------------------------------
// INF-12 vector-only units (bot-loop, 2026-11-02): the SAME structural
// blindness the monk had, closed preventively for every remaining
// production draw that was 100% vector (zero drawImage, no art-sheet
// path consulted). Audit of UNIT_DRAW on origin/main 27f28f0: Villager,
// Scout, Militia/Man-at-Arms, Archer, Spearman, Skirmisher and the
// animals all route through drawImage art paths (drawDirUnit /
// drawUnitPose / drawSpriteH) and were already visible to the pass;
// only the five draws below — plus the monk above — never emitted a
// drawImage. Each now paints its identical figure once into a cached
// offscreen sprite and blits it, keeping a vector fallback. Every
// figure is static (no walk/attack animation state feeds any of
// these draws); the Trade Cart's only state is its binary loaded
// flag, which keys its cache. Presentation only; INF-12's Status
// is untouched (filled-vs-outline remains Claudio's decision).

test('drawCavalry blits a cached sprite through drawImage', () => {
  const body = fnBody('drawCavalry');
  assert.match(body, /cavalrySprite\(variant, cloth\)/,
    'drawCavalry obtains the cached cavalry sprite');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'drawCavalry records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintCavalryFigure\(ctx, sx, sy, u, variant, cloth\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per cavalry draw');
});

test('the cavalry sprite is cached per variant and cloth colour', () => {
  const body = fnBody('cavalrySprite');
  assert.match(body, /cavalrySpriteCache\.has\(key\)/,
    'cache is consulted before painting');
  assert.match(body, /cavalrySpriteCache\.set\(key, cv\)/,
    'painted sprite is stored per (variant, cloth) key');
  assert.match(body, /paintCavalryFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintCavalryFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintCavalryFigure');
  for (const colour of ['#5d4630', '#6e6258', '#8a6844', '#77572f', '#b9bec6', '#9aa1ab', '#e8c39a', '#d8d8d8']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (legs, horses, armor, helm, weapons)`);
  }
  assert.match(body, /g\.ellipse\(sx, sy - 0\.95 \* u, 1\.15 \* u, 0\.55 \* u/,
    'the horse body keeps its geometry');
  assert.match(body, /variant === 'knight'/, 'knight variant branch preserved');
  assert.match(body, /variant === 'cavarcher'/, 'cavalry archer variant branch preserved');
});

test('drawPetard blits a cached sprite through drawImage', () => {
  const body = fnBody('drawPetard');
  assert.match(body, /petardSprite\(cloth\)/,
    'drawPetard obtains the cached petard sprite');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'drawPetard records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintPetardFigure\(ctx, sx, sy, u, cloth\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per petard draw');
});

test('the petard sprite is cached per tunic colour', () => {
  const body = fnBody('petardSprite');
  assert.match(body, /petardSpriteCache\.has\(cloth\)/,
    'cache is consulted before painting');
  assert.match(body, /petardSpriteCache\.set\(cloth, cv\)/,
    'painted sprite is stored per tunic colour');
  assert.match(body, /paintPetardFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintPetardFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintPetardFigure');
  for (const colour of ['#e8c39a', '#4a3826', '#2e2a24', '#ffb347']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (head, barrel, bands, fuse spark)`);
  }
  assert.match(body, /g\.ellipse\(sx \+ 0\.55 \* u, sy - 0\.95 \* u, 0\.52 \* u, 0\.62 \* u/,
    'the powder barrel keeps its geometry');
});

test('drawBatteringRam blits a cached sprite through drawImage', () => {
  const body = fnBody('drawBatteringRam');
  assert.match(body, /batteringRamSprite\(roof\)/,
    'drawBatteringRam obtains the cached ram sprite');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'drawBatteringRam records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintBatteringRamFigure\(ctx, sx, sy, u, roof\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per ram draw');
});

test('the battering ram sprite is cached per roof colour', () => {
  const body = fnBody('batteringRamSprite');
  assert.match(body, /ramSpriteCache\.has\(roof\)/,
    'cache is consulted before painting');
  assert.match(body, /ramSpriteCache\.set\(roof, cv\)/,
    'painted sprite is stored per roof colour');
  assert.match(body, /paintBatteringRamFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintBatteringRamFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintBatteringRamFigure');
  for (const colour of ['#2e2418', '#5f4c36', '#8a6844', '#c8ccd2', '#3a3a40']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (wheels, frame, log, iron cap, chains)`);
  }
  assert.match(body, /g\.fillRect\(sx - 1\.3 \* u, sy - 1\.55 \* u, 2\.35 \* u, 0\.5 \* u\)/,
    'the slung log keeps its geometry');
});

test('drawMangonel blits a cached sprite through drawImage', () => {
  const body = fnBody('drawMangonel');
  assert.match(body, /mangonelSprite\(cloth\)/,
    'drawMangonel obtains the cached mangonel sprite');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'drawMangonel records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintMangonelFigure\(ctx, sx, sy, u, cloth\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per mangonel draw');
});

test('the mangonel sprite is cached per cloth colour', () => {
  const body = fnBody('mangonelSprite');
  assert.match(body, /mangonelSpriteCache\.has\(cloth\)/,
    'cache is consulted before painting');
  assert.match(body, /mangonelSpriteCache\.set\(cloth, cv\)/,
    'painted sprite is stored per cloth colour');
  assert.match(body, /paintMangonelFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintMangonelFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintMangonelFigure');
  for (const colour of ['#2e2418', '#8a6844', '#5f4c36', '#6d543a', '#4a4234', '#3a3a40', '#6e6e73', '#5b5b60']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (wheels, frame, arm, sling, stones)`);
  }
  assert.match(body, /g\.moveTo\(sx \+ 0\.75 \* u, sy - 1\.35 \* u\)/,
    'the cocked throwing arm keeps its geometry');
});

test('drawTradeCart blits a cached sprite through drawImage', () => {
  const body = fnBody('drawTradeCart');
  assert.match(body, /tradeCartSprite\(cloth, loaded\)/,
    'drawTradeCart obtains the cached trade cart sprite');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'drawTradeCart records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintTradeCartFigure\(ctx, sx, sy, u, cloth, loaded\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per trade cart draw');
});

test('the trade cart sprite is cached per cloth colour and load state', () => {
  const body = fnBody('tradeCartSprite');
  assert.match(body, /tradeCartSpriteCache\.has\(key\)/,
    'cache is consulted before painting');
  assert.match(body, /tradeCartSpriteCache\.set\(key, cv\)/,
    'painted sprite is stored per (cloth, loaded) key');
  assert.match(body, /paintTradeCartFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintTradeCartFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintTradeCartFigure');
  for (const colour of ['#6d4c33', '#5d4030', '#8a6844', '#4a3826', '#c9a227', '#b8a888', '#e8c39a']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (horse, shafts, bed, gold, canvas, driver)`);
  }
  assert.match(body, /if \(loaded\)/,
    'the loaded/unloaded load branch is preserved inside the painter');
  assert.match(body, /g\.fillRect\(sx - 1\.35 \* u, sy - 1\.28 \* u, 1\.35 \* u, 0\.5 \* u\)/,
    'the cart bed keeps its geometry');
});

test('the unit draw table still routes the converted kinds', () => {
  assert.match(appJs, /drawKnight, drawLightCavalry, drawCavalryArcher,/,
    'UNIT_DRAW[12/13/14] remain the cavalry draws');
  assert.match(appJs, /drawPetard, \/\/ 15: Petard/,
    'UNIT_DRAW[15] remains drawPetard');
  assert.match(appJs, /drawTradeCart, \/\/ 16: Trade Cart/,
    'UNIT_DRAW[16] remains drawTradeCart');
  assert.match(appJs, /drawBatteringRam, \/\/ 17: Battering Ram/,
    'UNIT_DRAW[17] remains drawBatteringRam');
  assert.match(appJs, /drawMangonel, \/\/ 18: Mangonel/,
    'UNIT_DRAW[18] remains drawMangonel');
});
