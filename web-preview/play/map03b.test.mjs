// MAP03-B client tests — run with:  node --test map03b.test.mjs
// (from web-preview/play/). Two halves:
//  1. The minimap paints every MAP03-B terrain tag with its own
//     colour (full + pre-dimmed variants), read through the real
//     renderMinimap with a recording stub context.
//  2. The placement-ghost mirrors in app.js read the MAP03-B tag
//     sets (source pins, the established mirror-test style):
//     water-side / buildable / coast-land sets, the wall-on-beach
//     exception in mirrorValid, and a ground colour per new tag.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  createMinimap,
  renderMinimap,
  MM_DIRT, MM_DIRT_DIM, MM_SAND, MM_SAND_DIM, MM_SNOW, MM_SNOW_DIM,
  MM_ICE, MM_ICE_DIM, MM_ICE_NAV, MM_ICE_NAV_DIM, MM_BEACH, MM_BEACH_DIM,
  MM_MANGROVE, MM_MANGROVE_DIM,
} from './minimap.js';

function recordingCtx() {
  const fills = [];
  const ctx = {
    fillStyle: '', strokeStyle: '', lineWidth: 1,
    beginPath() {}, moveTo() {}, lineTo() {}, closePath() {},
    fill() {}, stroke() {}, clip() {}, save() {}, restore() {},
    setTransform() {},
    fillRect() { fills.push(ctx.fillStyle); },
  };
  return { ctx, fills };
}

test('minimap: every MAP03-B tag paints its own colour', () => {
  const g = createMinimap({ worldHalf: 100, width: 220, height: 124, pad: 2 });
  // 10x10 grid; row y carries tag y (0..9), fog visible (2) in
  // the first pass and explored-dim (1) in the second.
  const terrain = new Uint8Array(100);
  for (let y = 0; y < 10; y++) for (let x = 0; x < 10; x++) terrain[y * 10 + x] = y;
  const base = {
    gridW: 10, gridH: 10, cellSize: 4,
    obstacles: new Uint8Array(100),
    terrain, heights: new Uint8Array(100),
    nodes: [], buildings: [], entities: [], cam: null,
  };
  const full = [null, null, null, MM_DIRT, MM_SAND, MM_SNOW, MM_ICE, MM_ICE_NAV, MM_BEACH, MM_MANGROVE];
  const dim = [null, null, null, MM_DIRT_DIM, MM_SAND_DIM, MM_SNOW_DIM, MM_ICE_DIM, MM_ICE_NAV_DIM, MM_BEACH_DIM, MM_MANGROVE_DIM];
  for (const [fogVal, table] of [[2, full], [1, dim]]) {
    const { ctx, fills } = recordingCtx();
    renderMinimap(ctx, g, { ...base, fog: new Uint8Array(100).fill(fogVal) });
    // The flat ground diamond is a path fill, not a fillRect:
    // the recorded fillRect styles are exactly the per-cell
    // pass, in row-major order.
    const cells = fills.slice(0, 100);
    assert.equal(cells.length, 100, 'one fill per cell');
    for (let tag = 3; tag <= 9; tag++) {
      assert.equal(cells[tag * 10], table[tag], `tag ${tag} fog ${fogVal}`);
    }
  }
});

const appJs = readFileSync(new URL('../app.js', import.meta.url), 'utf8');

test('app.js: the MAP03-B tag sets carry the sim rows', () => {
  assert.match(appJs, /TERRAIN_WATER_SIDE = new Set\(\[1, 2, 7, 8, 9\]\)/);
  assert.match(appJs, /TERRAIN_BUILDABLE = new Set\(\[0, 3, 4, 5, 9\]\)/);
  assert.match(appJs, /TERRAIN_COAST_LAND = new Set\(\[0, 3, 4, 5, 8\]\)/);
});

test('app.js: mirrors read the sets, walls stand on beach', () => {
  // mirrorValid: buildable set + the WallsDocks wall exception.
  assert.match(appJs, /!TERRAIN_BUILDABLE\.has\(TERRAIN\[y \* GW \+ x\]\)/);
  assert.match(appJs, /isWallKind\(kind\) && TERRAIN\[y \* GW \+ x\] === TERRAIN_BEACH/);
  // mirrorTrapValid: water-side set, not t < 1.
  assert.match(appJs, /if \(!TERRAIN_WATER_SIDE\.has\(t\)\) return false; \/\/ water-side only/);
  // mirrorDockValid: both straddle halves via the sets.
  assert.match(appJs, /if \(TERRAIN_WATER_SIDE\.has\(t\)\) waterSide = true;/);
  assert.match(appJs, /TERRAIN_COAST_LAND\.has\(t\) \|\| \(\(t === 2 \|\| t === 9\) && landNeighbor\(x, y\)\)/);
});

test('app.js: ground colours exist for every new tag', () => {
  for (const tag of [3, 4, 5, 6, 7, 8, 9]) {
    assert.match(appJs, new RegExp(`TERRAIN\\[fi\\] === ${tag}\\) base =`), `tag ${tag} ground colour`);
  }
});
