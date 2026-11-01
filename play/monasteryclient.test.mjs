// Monastery client renumber pins (bot-loop, 2026-11-01) — regression
// for the game-breaking freeze found live on the P5a combo build
// (59f42df): the P5a landing renumbered the Monastery BuildingKind
// 18 -> 20 (18/19 are the Package J gates), but three client spots
// kept the builder branch's 18:
//   1. NEW_BUILDING_STYLE had no entry for kind 20, so the vector
//      fallback in drawNewBuilding read `.wall` of undefined and
//      threw inside draw(); frame() schedules its next rAF only
//      after draw(), so ONE Monastery on screen killed the whole
//      game loop permanently (sim frozen, DOM overlays still alive)
//      — exactly the live report (grey 3x3 slab, resources frozen
//      35+ min, Tech Tree/Menu still opening).
//   2. drawBuildingFeature's dome+cross branch tested kind 18.
//   3. buildBtn's Castle-Age gate tested kind 18, so the Monastery
//      build button showed enabled in the Dark Age while the
//      University/Siege Workshop showed "Requires Castle Age."
// These pins read app.js the way buildingart.test.mjs does.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function block(name) {
  const m = appJs.match(new RegExp(`const ${name} = \\{([\\s\\S]*?)\\};`));
  assert.ok(m, `${name} block found in app.js`);
  return m[1];
}
function keysOf(name) {
  return new Set([...block(name).matchAll(/(\d+):/g)].map((x) => Number(x[1])));
}

test('every drawNewBuilding kind has a style or an art key', () => {
  // drawBuilding routes kind >= 3 to drawNewBuilding except the
  // dedicated paths (2 Barracks, 9/14 walls, 18/19 gates, 1 Farm,
  // 0 House). The vector fallback dereferences the style entry, so
  // each routed kind needs a style entry or a building-art key.
  const routed = [3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 15, 16, 17, 20];
  const styles = keysOf('NEW_BUILDING_STYLE');
  const art = keysOf('BUILDING_ART_KEY');
  for (const k of routed) {
    assert.ok(styles.has(k) || art.has(k),
      `kind ${k} reaches drawNewBuilding with no style and no art key (undefined style deref freezes the frame loop)`);
  }
});

test('Monastery style and feature ride kind 20, not the gates\u2019 18', () => {
  assert.match(block('NEW_BUILDING_STYLE'),
    /20:\s*\{\s*wall: '#b3a284'[\s\S]*?Monastery/,
    'Monastery style entry sits on kind 20');
  const feat = appJs.match(/function drawBuildingFeature\(kind[\s\S]*?\n\}/);
  assert.ok(feat, 'drawBuildingFeature found');
  assert.match(feat[0], /kind === 20\) \{\s*\/\/ The Monastery's dome/,
    'dome+cross feature branch tests kind 20');
});

test('Monastery build button is Castle-Age gated like its neighbours', () => {
  assert.match(appJs, /kind === 20 && age < 2\) gate = t\('gate\.castle'\)/,
    'buildBtn age gate tests kind 20');
});
