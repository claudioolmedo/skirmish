// Headless tests for minimap.js — run with:  node --test minimap.test.mjs
// (from web-preview/play/). Uses a recording stub 2D context: every
// method the renderer calls is counted, so draw behavior is assertable
// without a browser.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  createMinimap,
  worldToMinimap,
  minimapToWorld,
  viewportRect,
  cameraCenteredOn,
  renderMinimap,
  NODE_COLORS,
  ALERT_COLOR,
} from './minimap.js';

// Sim constants (sim/src/grid.rs): world [-100, 100), 50x50 cells of 4.
const H = 100;
const MM = { width: 220, height: 124, pad: 2 };

function makeStubCtx() {
  const calls = {
    beginPath: 0, moveTo: 0, lineTo: 0, closePath: 0, fill: 0,
    stroke: 0, clip: 0, save: 0, restore: 0, arc: 0, fillRect: 0,
    strokeRect: 0,
  };
  const ctx = { calls, fillStyle: '', strokeStyle: '', lineWidth: 1 };
  for (const name of Object.keys(calls)) {
    ctx[name] = () => { calls[name]++; };
  }
  return ctx;
}

function emptyState(extra = {}) {
  return {
    gridW: 50, gridH: 50, cellSize: 4,
    obstacles: new Uint8Array(50 * 50),
    nodes: [], buildings: [], entities: [], cam: null,
    ...extra,
  };
}

const near = (a, b, eps = 1e-9) => Math.abs(a - b) <= eps;

test('geometry: fit and center', () => {
  const g = createMinimap({ worldHalf: H, ...MM });
  // spanU = 400, spanV = 200 -> scale = min(216/400, 120/200) = 0.54
  assert.ok(near(g.scale, 0.54), `scale ${g.scale}`);
  assert.equal(g.cx, 110);
  assert.equal(g.cy, 62);
  assert.throws(() => createMinimap({ worldHalf: 0, width: 10, height: 10 }));
});

test('worldToMinimap: diamond corners and center', () => {
  const g = createMinimap({ worldHalf: H, ...MM });
  const t = g.scale;
  const eq = ([mx, my], [ex, ey]) => assert.ok(near(mx, ex) && near(my, ey), `${mx},${my} != ${ex},${ey}`);
  eq(worldToMinimap(g, 0, 0), [110, 62]); // world center -> canvas center
  eq(worldToMinimap(g, -H, -H), [110, 62 - H * t]); // top corner
  eq(worldToMinimap(g, H, H), [110, 62 + H * t]); // bottom corner
  eq(worldToMinimap(g, -H, H), [110 - 2 * H * t, 62]); // left corner
  eq(worldToMinimap(g, H, -H), [110 + 2 * H * t, 62]); // right corner
});

test('minimapToWorld inverts worldToMinimap', () => {
  const g = createMinimap({ worldHalf: H, ...MM });
  for (const [wx, wy] of [[0, 0], [37.5, -82.25], [-100, 100], [99.9, 0.1], [-45, -45]]) {
    const [mx, my] = worldToMinimap(g, wx, wy);
    const [bx, by] = minimapToWorld(g, mx, my);
    assert.ok(near(bx, wx, 1e-9) && near(by, wy, 1e-9), `round-trip ${wx},${wy} -> ${bx},${by}`);
  }
});

test('viewportRect: centered camera maps to a rect centered on its focus', () => {
  const g = createMinimap({ worldHalf: H, ...MM });
  // Camera centered on world (10, -20) at zoom 5 over a 1000x800 view.
  const [ox, oy] = cameraCenteredOn(10, -20, 5, 1000, 800);
  assert.equal(ox, 500 - (10 - (-20)) * 5); // 350
  assert.equal(oy, 400 - ((10 + (-20)) / 2) * 5); // 425
  const r = viewportRect(g, { s: 5, ox, oy, viewW: 1000, viewH: 800 });
  // Visible iso span: u in [-70, 130], v in [-85, 75] -> 200 x 160 iso units.
  assert.ok(near(r.w, 200 * g.scale));
  assert.ok(near(r.h, 160 * g.scale));
  // The rect's center, mapped back to the world, is the camera focus.
  const [wx, wy] = minimapToWorld(g, r.x + r.w / 2, r.y + r.h / 2);
  assert.ok(near(wx, 10) && near(wy, -20), `focus ${wx},${wy}`);
});

test('renderMinimap: ground, clip and border with an empty world', () => {
  const g = createMinimap({ worldHalf: H, ...MM });
  const ctx = makeStubCtx();
  renderMinimap(ctx, g, emptyState());
  assert.equal(ctx.calls.save, 1);
  assert.equal(ctx.calls.clip, 1);
  assert.equal(ctx.calls.restore, 1);
  assert.ok(ctx.calls.fill >= 1, 'ground filled');
  assert.ok(ctx.calls.stroke >= 1, 'border stroked');
  assert.equal(ctx.calls.arc, 0, 'no dots in an empty world');
  assert.equal(ctx.calls.strokeRect, 0, 'no viewport without a camera');
});

test('renderMinimap: obstacles, live nodes only, entities, viewport', () => {
  const g = createMinimap({ worldHalf: H, ...MM });
  const obstacles = new Uint8Array(50 * 50);
  obstacles[10 * 50 + 10] = 1;
  obstacles[20 * 50 + 30] = 1;
  const ctx = makeStubCtx();
  renderMinimap(ctx, g, emptyState({
    obstacles,
    nodes: [
      { x: 0, y: 0, kind: 2, amount: 800 }, // gold, live
      { x: 5, y: 5, kind: 0, amount: 0 }, // depleted tree: skipped
      { x: -8, y: 3, kind: 1, amount: 125 }, // berry, live
    ],
    buildings: [
      { x: 12, y: 12, kind: 0, complete: true }, // house: filled diamond
      { x: 20, y: 20, kind: 0, complete: false }, // construction: stroked diamond
      { x: 30, y: 30, kind: 1, complete: true }, // finished farm: skipped (node draws it)
    ],
    entities: [
      { x: 1, y: 1, kind: 0, color: '#4da3ff', selected: true },
      { x: 2, y: 2, kind: 0, color: '#4da3ff' },
      { x: 0, y: 0, kind: 1, color: '#4da3ff' }, // Town Center
    ],
    cam: { s: 5, ox: 500, oy: 400, viewW: 1000, viewH: 800 },
  }));
  // arcs: 2 live nodes + gold underlay for the selected villager
  // + 2 villager dots (the TC is a fillRect) = 5.
  assert.equal(ctx.calls.arc, 5, `arc calls ${ctx.calls.arc}`);
  assert.equal(ctx.calls.fillRect, 1, 'TC square');
  assert.equal(ctx.calls.strokeRect, 1, 'viewport rect');
  // fills: ground + 2 obstacle cells + 2 nodes + 1 house + selected
  // underlay + 2 villagers + TC counted via fillRect; fill() itself:
  // ground, 2 obstacles, 2 nodes, house, underlay, 2 villagers = 9.
  assert.equal(ctx.calls.fill, 9, `fill calls ${ctx.calls.fill}`);
  // M3: two carcass node kinds (hunt + herd) extend the palette 5 -> 7.
  assert.equal(NODE_COLORS.length, 7);
});

// --- M7: fog/terrain base -------------------------------------------
// With fog + terrain layers present, the ground is drawn per cell
// (an affine-transformed fillRect per cell): unexplored cells in
// the shroud color, water and land shaded by fog state. The legacy
// state (no layers) must keep the old ground + obstacle rendering
// exactly — the tests above pin it; this one pins the new branch.
test('fog base: per-cell terrain under fog states (M7)', () => {
  const fills = [];
  const ctx = {
    fillStyle: '', strokeStyle: '', lineWidth: 1,
    beginPath() {}, moveTo() {}, lineTo() {}, closePath() {},
    fill() {}, stroke() {}, clip() {}, save() {}, restore() {},
    arc() {}, strokeRect() {},
    setTransform() {},
    fillRect(x, y, w, h) { fills.push({ x, y, fill: this.fillStyle }); },
  };
  const g = createMinimap({ worldHalf: 240, width: 220, height: 124, pad: 2 });
  const GW = 120, GH = 120;
  const terrain = new Uint8Array(GW * GH);
  const heights = new Uint8Array(GW * GH);
  const fog = new Uint8Array(GW * GH); // all unexplored
  terrain[5 * GW + 5] = 1; // a water cell...
  fog[5 * GW + 5] = 2;     // ...currently visible
  fog[6 * GW + 6] = 1;     // an explored land cell
  heights[6 * GW + 6] = 3;
  renderMinimap(ctx, g, {
    gridW: GW, gridH: GH, cellSize: 4,
    obstacles: new Uint8Array(GW * GH),
    terrain, heights, fog,
    nodes: [], buildings: [], entities: [], cam: null,
  });
  // One base rect per cell, in row-major cell order.
  assert.equal(fills.length, GW * GH);
  assert.equal(fills[0].fill, '#05070a', 'unexplored = shroud');
  assert.equal(fills[5 * GW + 5].fill, '#2e6da8', 'visible water');
  assert.equal(fills[6 * GW + 6].fill, '#2f4a26', 'explored land at height 3 (dim)');
  assert.deepEqual([fills[6 * GW + 6].x, fills[6 * GW + 6].y], [6, 6]);
});

test('renderMinimap: the INF-01 alert X paints only while its blink is lit', () => {
  const g = createMinimap({ worldHalf: H, ...MM });
  const base = makeStubCtx();
  renderMinimap(base, g, emptyState());
  const lit = makeStubCtx();
  renderMinimap(lit, g, emptyState({ alert: { x: 10, y: -20, on: true } }));
  // The X is two crossed strokes: +1 stroke, +2 moveTo, +2 lineTo.
  assert.equal(lit.calls.stroke, base.calls.stroke + 1);
  assert.equal(lit.calls.moveTo, base.calls.moveTo + 2);
  assert.equal(lit.calls.lineTo, base.calls.lineTo + 2);
  const dark = makeStubCtx();
  renderMinimap(dark, g, emptyState({ alert: { x: 10, y: -20, on: false } }));
  assert.deepEqual(dark.calls, base.calls);
  assert.equal(ALERT_COLOR, '#e8402f');
});
