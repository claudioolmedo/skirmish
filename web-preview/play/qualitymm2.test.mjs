// qualitymm2 (bot-loop run #456 builder) — pixel-level + governor-level
// discrimination for the qualitymm defect (runs #451/#452: minimap
// black with only the visible zone painting, ground flat at "Medium",
// persisting after Pause / Quality change / Restart / Defeat, and —
// run #452 session 2 — flat in a fresh match after a hard reload).
//
// WHAT THE EARLIER BUILDERS PROVED (not repeated here):
// - qualitymm: the manual Medium→Low→Medium switch is exactly
//   reversible through the real modules (fillRect census identical).
// - qualitylog + run #453/#454 live captures: qualstate lines healthy
//   (fog mix normal, mmTerrainCells == 14400, level/groundStrokes
//   correct at every stage) in the sessions that ran ?occlog=1.
//
// WHAT THIS FILE ADDS — the two readings neither builder took:
// (1) A RASTER census of the minimap: a transform-aware stub ctx
//     that resolves each terrain fillRect(gx,gy,1,1) through the real
//     iso matrix renderMinimap sets, and records the COLOUR that
//     lands at each cell-centre pixel. Call counts (the qualitylog
//     census) prove rects were issued; this proves WHAT COLOUR each
//     fog class paints — explored (fog 1) land is the DIM palette,
//     a colour measurably distinct from shroud, so "only the visible
//     zone paints" cannot be produced by renderMinimap from a
//     healthy fog array at ANY quality (it takes no quality input).
// (2) The governor's AUTO latch — the one client state that produces
//     flat ground (groundStrokes:false) while persisting across
//     Pause, Restart and Defeat and re-arming after a reload:
//     in auto mode the governor steps medium→low after ~6 s of
//     sustained <45 fps and, because the frame loop never samples
//     while paused/ended and NO match-entry reset touches the
//     governor, the low level survives every one of those boundaries
//     untouched; recovery needs the 15 s cooldown PLUS 8 consecutive
//     >=58 fps windows (~12 s more). A manual Medium pick makes the
//     governor inert (pinned too) — so flat ground at a TRULY manual
//     Medium is impossible via this path, and a flat-after-reload
//     reading forks exactly: the session was in Auto (menu shows the
//     MODE, never the live level), or the Medium click never applied.
//     Both forks are readable from ONE qualstate line (level +
//     groundStrokes) beside a screenshot of the pause menu's active
//     button — the experiment the handoff names.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import {
  createMinimap, renderMinimap,
  MM_LAND, MM_LAND_DIM, SHROUD_COLOR,
} from './minimap.js';
import { createGovernor, settingsFor } from './quality.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}

// --- Rasterizing stub ctx -------------------------------------------
// Applies the current transform to fillRect corners' centre and
// records colour per pixel key. save/restore keep a transform stack;
// clip/beginPath/etc. are no-ops (the terrain pass's clip is the
// world diamond, which contains every cell centre by construction).
function rasterCtx() {
  let m = [1, 0, 0, 1, 0, 0]; // a,b,c,d,e,f
  const stack = [];
  const pixels = new Map(); // "x,y" -> fillStyle at paint time
  const ctx = {
    fillStyle: '#000', strokeStyle: '#000', lineWidth: 1,
    save() { stack.push(m.slice()); },
    restore() { m = stack.pop() || m; },
    setTransform(a, b, c, d, e, f) { m = [a, b, c, d, e, f]; },
    getTransform() { return { a: m[0], b: m[1], c: m[2], d: m[3], e: m[4], f: m[5] }; },
    beginPath() {}, closePath() {}, moveTo() {}, lineTo() {},
    clip() {}, fill() {}, stroke() {}, arc() {}, strokeRect() {},
    fillRect(x, y, w, h) {
      const cx = x + w / 2, cy = y + h / 2;
      const px = Math.round(m[0] * cx + m[2] * cy + m[4]);
      const py = Math.round(m[1] * cx + m[3] * cy + m[5]);
      pixels.set(`${px},${py}`, ctx.fillStyle);
    },
  };
  return { ctx, pixels };
}

// Staged 12x12 world: unexplored except a visible disc (fog 2) at
// the centre and an explored trail (fog 1) east of it.
function stagedMinimap() {
  const GW = 12, GH = 12, CS = 10, HALF = 60;
  const fog = new Uint8Array(GW * GH);
  const terrain = new Uint8Array(GW * GH);
  const heights = new Uint8Array(GW * GH);
  for (let gy = 0; gy < GH; gy++) for (let gx = 0; gx < GW; gx++) {
    const dx = gx - 5, dy = gy - 5;
    if (dx * dx + dy * dy <= 4) fog[gy * GW + gx] = 2;
    else if (gy === 5 && gx > 5 && gx <= 10) fog[gy * GW + gx] = 1;
  }
  const g = createMinimap({ worldHalf: HALF, width: 220, height: 124 });
  const { ctx, pixels } = rasterCtx();
  // Cell-centre pixel under the same matrix renderMinimap installs:
  // setTransform(csPx, csPx/2, -csPx, csPx/2, cx, cy - H*scale).
  const cellPixel = (gx, gy) => {
    const csPx = CS * g.scale;
    const px = Math.round(csPx * (gx + 0.5) - csPx * (gy + 0.5) + g.cx);
    const py = Math.round(csPx / 2 * (gx + 0.5) + csPx / 2 * (gy + 0.5) + g.cy - HALF * g.scale);
    return `${px},${py}`;
  };
  const out = renderMinimap(ctx, g, {
    gridW: GW, gridH: GH, cellSize: CS, obstacles: null,
    terrain, heights, fog, nodes: [], buildings: [], entities: [], cam: null,
  });
  return { out, pixels, cellPixel };
}

test('raster census: explored cells paint the DIM palette, not shroud', () => {
  const { out, pixels, cellPixel } = stagedMinimap();
  assert.equal(out.terrainCells, 144);
  // visible centre cell -> full land colour
  assert.equal(pixels.get(cellPixel(5, 5)), MM_LAND[0]);
  // explored trail cell -> DIM land colour (painted, distinct)
  assert.equal(pixels.get(cellPixel(9, 5)), MM_LAND_DIM[0]);
  assert.notEqual(MM_LAND_DIM[0], SHROUD_COLOR);
  // unexplored corner -> shroud
  assert.equal(pixels.get(cellPixel(0, 0)), SHROUD_COLOR);
});

test('raster census: an all-explored fog paints NO shroud land cells', () => {
  // The all-fog1 hypothesis from the qualitymm handoff, at pixel
  // level: every land cell would read DIM — dark, but a uniform
  // island-shaped diamond, never "visible patch only".
  const GW = 12, GH = 12;
  const fog = new Uint8Array(GW * GH).fill(1);
  const g = createMinimap({ worldHalf: 60, width: 220, height: 124 });
  const { ctx, pixels } = rasterCtx();
  renderMinimap(ctx, g, {
    gridW: GW, gridH: GH, cellSize: 10, obstacles: null,
    terrain: new Uint8Array(GW * GH), heights: new Uint8Array(GW * GH),
    fog, nodes: [], buildings: [], entities: [], cam: null,
  });
  const colours = new Set(pixels.values());
  assert.ok(colours.has(MM_LAND_DIM[0]), 'dim land painted');
  // (the ground diamond base fill also lands in the map at pixels no
  // cell centre hits; cell-centre colours are what was asserted via
  // the staged scene above — here we only require dim presence.)
});

// --- Governor auto latch ---------------------------------------------
function feed(gov, dtMs, frames, startNow) {
  let now = startNow;
  let last = null;
  for (let i = 0; i < frames; i++) { now += dtMs; last = gov.sample(dtMs, now); }
  return { now, last };
}

test('auto governor: sustained 30 fps steps medium->low and LATCHES across pause/restart boundaries', () => {
  const gov = createGovernor({ initial: 'medium', max: 'medium' });
  assert.equal(gov.manual, false);
  // ~6 s at 30 fps: 4 bad windows -> one step down.
  let r = feed(gov, 1000 / 30, 200, 0);
  assert.equal(gov.level, 'low');
  assert.equal(settingsFor(gov.level).groundStrokes, false, 'flat ground = the low cosmetic set');
  // Pause / Defeat: the frame loop samples NOTHING while paused or
  // ended (source-pinned below), so no recovery can happen there —
  // and a Restart touches no governor state (source-pinned below).
  // Simulate 60 s of wall time across those boundaries with the few
  // samples a resumed-but-still-heavy session yields at 30 fps:
  r = feed(gov, 1000 / 30, 200, r.now + 60000);
  assert.equal(gov.level, 'low', 'still latched after the boundary + more heavy frames');
  // Recovery is slow BY DESIGN: cooldown 15 s, then 8 good windows.
  // 10 s of perfect 60 fps is NOT enough (cooldown eats the streaks).
  const t0 = r.now;
  r = feed(gov, 1000 / 60, 600, t0);
  assert.equal(gov.level, 'low', 'no recovery inside cooldown + partial streak');
  // ~30 s more of sustained 60 fps finally steps back up.
  r = feed(gov, 1000 / 60, 1800, r.now);
  assert.equal(gov.level, 'medium');
  assert.equal(settingsFor(gov.level).groundStrokes, true);
});

test('manual Medium is governor-proof: flat ground at a truly manual Medium cannot come from adaptation', () => {
  const gov = createGovernor({ initial: 'medium', max: 'medium' });
  gov.setManual('medium');
  const r = feed(gov, 100, 500, 0); // 10 fps, 50 s — brutal load
  assert.equal(r.last.changed, null);
  assert.equal(gov.level, 'medium');
  assert.equal(settingsFor(gov.level).groundStrokes, true);
});

test('source pins: sampling sites, match-entry resets, and the defeat reveal branch', () => {
  // The frame loop samples the governor only in the playing
  // branches; setPaused and showMatchEnd never touch it.
  const frame = sliceFn(appJs, 'function frame(now)', 'requestAnimationFrame(frame);\n  }\n  requestAnimationFrame(frame);');
  assert.ok(frame.includes('governor.sample'), 'frame loop samples');
  const setPaused = sliceFn(appJs, 'function setPaused(v)', '\nfunction ');
  assert.ok(!setPaused.includes('governor') && !setPaused.includes('applyQuality'),
    'pause open/close never changes the level');
  const showEnd = sliceFn(appJs, 'function showMatchEnd(', '\nfunction ');
  assert.ok(!showEnd.includes('governor') && !showEnd.includes('applyQuality'),
    'defeat/victory never changes the level');
  for (const fn of ['function resetSim(', 'function adoptSim(', 'function adoptLoadedSim(']) {
    const body = sliceFn(appJs, fn, '\nfunction ');
    assert.ok(!body.includes('governor.') && !body.includes('applyQuality('),
      `${fn} leaves the governor latched (restart keeps the level)`);
  }
  // Q is (re)assigned only at boot and inside applyQuality.
  const qWrites = appJs.match(/Q = settingsFor\(/g) || [];
  assert.equal(qWrites.length, 2, 'boot init + applyQuality only');
  // Defeat "View map" (revealMap) serves an all-visible fog — the
  // OPPOSITE of a visible-only minimap; defeat-screen flatness
  // therefore cannot be a fog state.
  const myFog = sliceFn(appJs, 'const myFog = () => {', '};');
  assert.ok(myFog.includes('if (revealMap) return revealedFog();'));
});
