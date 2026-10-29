// Canopy-pick regression pins (bot-loop defect, 2026-10-29;
// live formals #86, #87, #88 all reported the same sequence:
// the FIRST right-click on a tree's canopy produced no gathering
// — the villager walked to empty ground and idled — while a
// second click on the trunk/base started chopping at once).
//
// Root cause: the tree sprite is drawn TREE_H world units tall,
// rising straight up the screen from the trunk's ground point,
// but clicks are converted to world coordinates on the ground
// plane (screen-up = the (-d, -d) world direction). A canopy
// click therefore lands up to ~TREE_H units north-west of the
// trunk, outside nodeAt's 4-unit ground radius, and orderAt fell
// through to a plain ground move. In DE, clicking any visible
// part of a tree tasks the villager onto it.
//
// The fix is a client-only fallback pick, treeCanopyAt: when the
// plain ground pick misses AND the click came from the main
// canvas, the click is decomposed against each tree's trunk —
// H = height up the sprite, dx = screen-x offset from the
// trunk's column — and names the tree whose drawn canopy covers
// the click. These pins keep the pick geometry tied to the draw
// geometry (TREE_H / TREE_WIDE, parsed from app.js itself) and
// keep the fallback canvas-only (the minimap draws no tall
// sprites; its pick must stay the plain ground pick).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function constValue(src, name) {
  const m = src.match(new RegExp(`const ${name} = ([0-9.]+);`));
  assert.ok(m, `${name} found in app.js`);
  return Number(m[1]);
}

const treeH = constValue(appJs, 'TREE_H');
const treeWide = constValue(appJs, 'TREE_WIDE');

test('canopy half width is derived from the draw constants', () => {
  assert.match(
    appJs,
    /const CANOPY_HALF_W = TREE_H \* 0\.76 \* TREE_WIDE \/ 2;/,
    'CANOPY_HALF_W comes from drawTreeImage\'s own width formula',
  );
  const halfW = treeH * 0.76 * treeWide / 2;
  assert.ok(halfW > 3.5 && halfW < 4.0, `canopy half width ${halfW}`);
});

test('treeCanopyAt decomposes the click against tree trunks only', () => {
  assert.match(appJs, /function treeCanopyAt\(wx, wy\)/, 'the fallback pick exists');
  assert.match(
    appJs,
    /if \(n\.kind !== 0 \|\| n\.amount <= 0\) continue;/,
    'only living trees (kind 0) are canopy-pickable',
  );
  assert.match(appJs, /const h = -\(dxw \+ dyw\) \/ 2;/, 'H is the screen-up decomposition');
  assert.match(appJs, /const dx = dxw - dyw;/, 'dx is the screen-x decomposition');
  assert.match(
    appJs,
    /if \(h < 0 \|\| h > TREE_H \|\| Math\.abs\(dx\) > CANOPY_HALF_W\) continue;/,
    'the candidate window is exactly the drawn sprite box',
  );
});

test('the fallback is wired after the plain pick, canvas only', () => {
  const plain = appJs.indexOf('let n = nodeAt(wx, wy);');
  const fallback = appJs.indexOf('if (!n && fromCanvas) n = treeCanopyAt(wx, wy);');
  assert.ok(plain >= 0 && fallback > plain, 'canopy pick runs only when the ground pick missed');
  assert.match(
    appJs,
    /function smartOrderAt\(wx, wy, queued = false, fromCanvas = false\)/,
    'smartOrderAt carries the canvas flag, defaulting off',
  );
  assert.match(appJs, /orderAt\(wx, wy, queued, fromCanvas\);/, 'the flag reaches orderAt');
  assert.match(
    appJs,
    /smartOrderAt\(wx, wy, e\.shiftKey, true\);/,
    'the main-canvas right-click passes the flag',
  );
  assert.match(
    appJs,
    /smartOrderAt\(w\[0\], w\[1\], ev\.shiftKey\);/,
    'the minimap right-click does NOT pass the flag (SEL-16 pick unchanged)',
  );
});

// The pick geometry, re-implemented from the same formulas the
// pins above lock into app.js, exercised on a tree at (100, 100).
const HALF_W = treeH * 0.76 * treeWide / 2;
const groundPick = (x, y) => Math.hypot(x - 100, y - 100) < 4;
function canopyPick(x, y) {
  const dxw = x - 100;
  const dyw = y - 100;
  const h = -(dxw + dyw) / 2;
  const dx = dxw - dyw;
  return h >= 0 && h <= treeH && Math.abs(dx) <= HALF_W;
}

test('geometry: canopy clicks miss the ground pick but name the tree', () => {
  // Trunk/base click: the ground pick already works (unchanged).
  assert.equal(groundPick(100, 100), true);
  // Canopy centre (~55% up the sprite, straight up-screen from
  // the trunk): the exact click the live formals saw dropped.
  const cx = 100 - treeH * 0.55;
  const cy = 100 - treeH * 0.55;
  assert.equal(groundPick(cx, cy), false, 'canopy centre is outside the 4-unit ground radius');
  assert.equal(canopyPick(cx, cy), true, 'canopy centre names the tree');
  // Near the treetop and at a canopy edge: still the tree.
  assert.equal(canopyPick(100 - treeH * 0.95, 100 - treeH * 0.95), true);
  assert.equal(canopyPick(100 - 3 + 1.5, 100 - 3 - 1.5), true, 'dx within the drawn half width');
});

test('geometry: ground clicks the canopy does not cover stay moves', () => {
  // Above the treetop.
  assert.equal(canopyPick(100 - treeH - 1, 100 - treeH - 1), false);
  // North ground beside the tree (screen up-right): H inside the
  // sprite's height band but dx beyond the drawn canopy width.
  assert.equal(canopyPick(100, 100 - 6), false);
  // South of the trunk (below the sprite on screen): H negative.
  assert.equal(canopyPick(105, 105), false);
  // Open ground a full tile east.
  assert.equal(canopyPick(108, 100), false);
});
