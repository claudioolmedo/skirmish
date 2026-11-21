// Quality/fog instrumentation (bot-loop qualitylog, 2026-11-21;
// the discriminating instrument for the qualitymm defect).
//
// THE DEFECT IT SERVES (runs #451/#452, live on the published
// build): after a pause-menu Quality Medium→Low→Medium switch,
// (i) the minimap rendered NO terrain fill except a small patch
// around the base, in every later state (Pause, Quality switches,
// Restart, hard reload, fresh match) — areas the Scout had
// explored (bright in the main view) never appeared on it; and
// (ii) the main ground at Medium SOMETIMES rendered flat (the
// Low cosmetic set: no two-tone checker strokes) — in run #452
// session 2, match 1 at Medium had the checker, but after a hard
// reload the fresh match at Medium was flat. The qualitymm
// scoping handoff (skirmish-study/handoffs/qualitymm-build.md)
// proved the switch exactly reversible through the real modules
// in a stub DOM, so the mechanism lives in state that harness
// cannot see. Its discriminating experiment needs three readings
// the UI does not expose: (1) a fog census of the frame's fog
// grid, (2) the governor's live level + Q.groundStrokes vs the
// pause menu's active button, (3) what the minimap pass actually
// painted that frame.
//
// THE INSTRUMENT (this package — instrumentation ONLY, zero
// behaviour change): under the existing ?occlog=1 gate, draw()
// emits one occLog line { ev:'qualstate', level, groundStrokes,
// fog0, fog1, fog2, mmTerrainCells } per rendered frame at a
// throttled cadence (one per 60 frames), plus one on the first
// frame after a qualLogRequest() — which applyQuality (every
// quality change), setPaused (pause open) and the three
// match-entry resets (resetSim / adoptSim / adoptLoadedSim:
// Restart + every match start) each call. The line is emitted
// from the REAL render path: occQualState runs at the END of
// draw(), after that frame's ground pass and renderMinimap,
// censusing the same FOG snapshot the frame drew from and the
// terrain-fillRect count renderMinimap itself returned for that
// frame (minimap.js now returns { terrainCells } — a pure
// census, no drawing change). Nothing is computed in parallel,
// so the readings cannot diverge from what was painted.
//
// This file pins: (1) the wiring (helper shape, the four
// request sites, the draw() emission point AFTER renderMinimap);
// (2) census correctness on staged scenes — a known fog grid
// yields exact fog0/fog1/fog2 counts, and a known minimap scene
// yields the exact terrainCells renderMinimap reports; (3)
// emission on a quality-change request, on the first frame
// after it (a request alone emits nothing — the line belongs
// to a rendered frame), with the NEW level's values; (4) the
// 60-frame throttle; (5) inertness without ?occlog=1 and
// without occLog at all (the slicing-harness discipline, as in
// occDeselect / FIX #18); (6) no behaviour change — the
// minimap's drawing calls are exactly the pre-instrument ones
// (the return value is additive only), and a foreign occLog
// sequence (rclick/pick/issue) passes through untouched, with
// qualstate lines only ever APPENDED at due frames. No existing
// sequence pin needed a supersession: the full node suite's
// occorder*/selpick* files run unedited against this tree.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { createMinimap, renderMinimap } from './minimap.js';
import { settingsFor } from './quality.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const minimapJs = readFileSync(join(here, 'minimap.js'), 'utf8');

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}
function trySlice(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  if (a < 0) return null;
  const b = src.indexOf(endMarker, a + startMarker.length);
  return b > a ? src.slice(a, b) : null;
}

const occLogSrc = sliceFn(appJs, 'const OCCLOG_ON', '\nconst canvas = document.getElementById');
const qualSrc = trySlice(appJs, '// QUALITYLOG (bot-loop qualitylog', '\n// SEL-05');

// Build the REAL occLog from its slice, under a chosen query
// string (the occorder5/occorder21 harness shape).
function makeOccLog(search) {
  const env = {
    params: new URLSearchParams(search),
    window: {},
    performance: { now: () => 0 },
  };
  const factory = new Function(
    ...Object.keys(env),
    `${occLogSrc}\nreturn { occLog, occLogEntries, OCCLOG_ON, win: window };`,
  );
  const r = factory(...Object.values(env));
  assert.equal(r.win.__occlog, r.occLogEntries, 'window.__occlog is the live entries array');
  return { occLog: r.occLog, entries: r.occLogEntries, on: r.OCCLOG_ON };
}

// Build the REAL qualitylog helper from its slice, over a staged
// live state (FOG / governor / Q as draw() would read them).
// setState swaps the staged state between frames, exactly as a
// quality change or a new match does between two real frames.
function makeQual(search, opts = {}) {
  assert.ok(qualSrc, 'the QUALITYLOG helper block exists in app.js (absent on pristine)');
  const { fog = null, level = 'medium' } = opts;
  const { occLog, entries } = makeOccLog(search);
  const env = { occLog, FOG: fog, governor: { level }, Q: settingsFor(level), settingsFor };
  const factory = new Function(
    ...Object.keys(env),
    `${qualSrc}\nreturn { occQualState, qualLogRequest, fogCensus, ` +
    `setState: (f, lvl) => { FOG = f; governor = { level: lvl }; Q = settingsFor(lvl); } };`,
  );
  const fns = factory(...Object.values(env));
  return { ...fns, entries };
}

// A recording stub 2D context (the minimap.test.mjs shape):
// counts every call the renderer makes, records terrain fills.
function makeStubCtx({ withTransform = true } = {}) {
  const calls = {
    beginPath: 0, moveTo: 0, lineTo: 0, closePath: 0, fill: 0,
    stroke: 0, clip: 0, save: 0, restore: 0, arc: 0, fillRect: 0,
    strokeRect: 0,
  };
  const fills = [];
  const ctx = { calls, fills, fillStyle: '', strokeStyle: '', lineWidth: 1 };
  for (const name of Object.keys(calls)) {
    ctx[name] = (...a) => { calls[name]++; if (name === 'fillRect') fills.push(ctx.fillStyle); };
  }
  if (withTransform) ctx.setTransform = () => {};
  return ctx;
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('QUALITYLOG wiring: the helper emits the qualstate shape from live state', () => {
  assert.ok(qualSrc, 'the QUALITYLOG helper block exists beside occDeselect');
  assert.match(qualSrc, /function qualLogRequest\(\) \{\s*qualLogPending = true;/,
    'qualLogRequest only arms the next rendered frame — it emits nothing itself');
  assert.match(qualSrc, /function occQualState\(mmTerrainCells\) \{/,
    'the emitter takes the frame\u2019s minimap census as its argument');
  assert.match(qualSrc, /ev: 'qualstate', level: governor\.level, groundStrokes: !!Q\.groundStrokes,/,
    'the line carries the governor\u2019s live level and Q.groundStrokes at that frame');
  assert.match(qualSrc, /fog0: c\.fog0, fog1: c\.fog1, fog2: c\.fog2, mmTerrainCells,/,
    'the line carries the fog census and the minimap census');
  assert.match(qualSrc, /qualLogFrameCount % 60 !== 0/,
    'the periodic cadence is one line per 60 rendered frames');
  assert.match(qualSrc, /if \(typeof occLog !== 'function'\) return;/,
    'the emitter is typeof-guarded, like occDeselect (slicing-harness discipline)');
});

test('QUALITYLOG wiring: every discriminating moment requests a line', () => {
  const applySrc = sliceFn(appJs, 'function applyQuality(level', '\n// Pause-menu override');
  assert.ok(applySrc.indexOf('qualLogRequest();') > applySrc.indexOf('updateQualityUi();'),
    'applyQuality (every quality change, manual or governor) requests after the new level is applied');
  const pausedSrc = sliceFn(appJs, 'function setPaused(v) {', '\n// --- M8:');
  assert.match(pausedSrc, /if \(v && !wasPaused\) qualLogRequest\(\);/,
    'setPaused requests on pause OPEN only (not on close)');
  const fresh = appJs.split("qualLogRequest(); // QUALITYLOG: the fresh match's first frames report qualstate").length - 1;
  assert.equal(fresh, 3,
    'the three match-entry resets (resetSim / adoptSim / adoptLoadedSim) each request — Restart and every match start');
  for (const [name, start, end] of [
    ['resetSim', 'function resetSim(mode = matchMode) {', '\n/// Adopt a freshly built sim'],
    ['adoptSim', 'function adoptSim(newSim, owner) {', '\nfunction makeNetSession() {'],
    ['adoptLoadedSim', 'function adoptLoadedSim(loaded) {', '\n// --- i18n: static strings'],
  ]) {
    const src = sliceFn(appJs, start, end);
    assert.ok(src.includes("qualLogRequest(); // QUALITYLOG: the fresh match's first frames report qualstate"),
      `${name} requests a qualstate line for the fresh match`);
  }
});

test('QUALITYLOG wiring: draw() emits AFTER renderMinimap, from its returned census', () => {
  const drawSrc = sliceFn(appJs, 'function draw() {', '\nmain().catch');
  const iMm = drawSrc.indexOf('const mmResult = renderMinimap(mmCtx, mmGeom, {');
  assert.ok(iMm > 0, 'draw() captures renderMinimap\u2019s return — the census comes from the real render');
  const iTake = drawSrc.indexOf('mmTerrainCells = mmResult.terrainCells;');
  assert.ok(iTake > iMm, 'draw() takes terrainCells off that return');
  const iEmit = drawSrc.indexOf('occQualState(mmTerrainCells);');
  assert.ok(iEmit > iTake, 'occQualState runs after the minimap pass, at the end of draw()');
  assert.match(minimapJs, /terrainCells \+= 1;/,
    'minimap.js counts each terrain fillRect it issues');
  assert.match(minimapJs, /return \{ terrainCells \};/,
    'renderMinimap returns the census (additive — it returned nothing before)');
});

// ------------------------------------------------------------------
// Census correctness on staged scenes (fail on pristine: the
// helper is absent and renderMinimap returns undefined).

test('fogCensus: a known fog grid yields exact counts', () => {
  const q = makeQual('occlog=1');
  // 4x3 grid staged by hand: five 0s, four 1s, three 2s.
  const fog = new Uint8Array([0, 0, 1, 2, 0, 1, 1, 2, 0, 0, 1, 2]);
  assert.deepEqual(q.fogCensus(fog), { fog0: 5, fog1: 4, fog2: 3 });
  assert.deepEqual(q.fogCensus(new Uint8Array(12)), { fog0: 12, fog1: 0, fog2: 0 },
    'an all-shroud grid censuses as all fog0 (the fog-reset reading)');
  assert.deepEqual(q.fogCensus(new Uint8Array(12).fill(1)), { fog0: 0, fog1: 12, fog2: 0 },
    'an all-explored grid censuses as all fog1 (the single-state reading)');
  assert.deepEqual(q.fogCensus(new Uint8Array(12).fill(2)), { fog0: 0, fog1: 0, fog2: 12 });
  assert.deepEqual(q.fogCensus(null), { fog0: 0, fog1: 0, fog2: 0 },
    'no fog layer yet (pre-match) censuses as zeros, never throws');
});

test('staged frame: exact qualstate line from a known fog grid + known minimap scene', () => {
  // Minimap scene: a 4x3 world whose fog is the staged grid.
  const GW = 4, GH = 3;
  const fog = new Uint8Array([0, 0, 1, 2, 0, 1, 1, 2, 0, 0, 1, 2]); // 5 / 4 / 3
  const terrain = new Uint8Array(GW * GH);
  const g = createMinimap({ worldHalf: 8, width: 220, height: 124, pad: 2 });
  const ctx = makeStubCtx();
  const mm = renderMinimap(ctx, g, {
    gridW: GW, gridH: GH, cellSize: 4,
    obstacles: new Uint8Array(GW * GH),
    terrain, heights: new Uint8Array(GW * GH), fog,
    nodes: [], buildings: [], entities: [], cam: null,
  });
  assert.ok(mm && typeof mm.terrainCells === 'number',
    'renderMinimap returns its census (undefined on pristine)');
  assert.equal(mm.terrainCells, GW * GH, 'one terrain fillRect per cell of the staged grid');
  assert.equal(ctx.calls.fillRect, GW * GH, 'the census equals the fillRects actually issued');

  const q = makeQual('occlog=1', { fog, level: 'medium' });
  q.qualLogRequest(); // a match start / quality change armed this frame
  q.occQualState(mm.terrainCells);
  assert.equal(q.entries.length, 1, 'exactly one line for the armed frame');
  const e = q.entries[0];
  assert.deepEqual(
    { ev: e.ev, level: e.level, groundStrokes: e.groundStrokes, fog0: e.fog0, fog1: e.fog1, fog2: e.fog2, mmTerrainCells: e.mmTerrainCells },
    { ev: 'qualstate', level: 'medium', groundStrokes: true, fog0: 5, fog1: 4, fog2: 3, mmTerrainCells: 12 },
    'the line shape and every census field are exact for the staged scene');
  assert.deepEqual(Object.keys(e).sort(), ['at', 'ev', 'fog0', 'fog1', 'fog2', 'groundStrokes', 'level', 'mmTerrainCells'],
    'no extra fields ride the line (at is occLog\u2019s own timestamp)');
});

test('minimap census: no fog/terrain layers, or no setTransform, paints zero terrain cells', () => {
  const g = createMinimap({ worldHalf: 100, width: 220, height: 124, pad: 2 });
  const plain = renderMinimap(makeStubCtx(), g, {
    gridW: 50, gridH: 50, cellSize: 4,
    obstacles: new Uint8Array(50 * 50),
    nodes: [], buildings: [], entities: [], cam: null,
  });
  assert.equal(plain.terrainCells, 0, 'the pre-M7 path issues no terrain fillRects');
  const GW = 4, GH = 3;
  const noTf = renderMinimap(makeStubCtx({ withTransform: false }), g, {
    gridW: GW, gridH: GH, cellSize: 4,
    obstacles: new Uint8Array(GW * GH),
    terrain: new Uint8Array(GW * GH), heights: new Uint8Array(GW * GH),
    fog: new Uint8Array(GW * GH).fill(2),
    nodes: [], buildings: [], entities: [], cam: null,
  });
  assert.equal(noTf.terrainCells, 0,
    'when the transform pass cannot run, the census truthfully reports zero painted cells');
});

// ------------------------------------------------------------------
// Emission discipline: throttle, event requests, live values.

test('throttle: one line per 60 rendered frames, censusing live state each time', () => {
  const fogA = new Uint8Array(12).fill(2);
  const q = makeQual('occlog=1', { fog: fogA, level: 'medium' });
  for (let i = 0; i < 59; i++) q.occQualState(12);
  assert.deepEqual(q.entries, [], 'no line before the 60th frame');
  q.occQualState(12);
  assert.equal(q.entries.length, 1, 'the 60th frame emits');
  assert.equal(q.entries[0].fog2, 12);
  // The fog degrades to all-explored between periods: the next
  // periodic line must read the NEW grid (it censuses per frame,
  // never a cached snapshot).
  q.setState(new Uint8Array(12).fill(1), 'medium');
  for (let i = 0; i < 59; i++) q.occQualState(12);
  assert.equal(q.entries.length, 1, 'no early second line');
  q.occQualState(12);
  assert.equal(q.entries.length, 2, 'the 120th frame emits');
  assert.deepEqual([q.entries[1].fog0, q.entries[1].fog1, q.entries[1].fog2], [0, 12, 0],
    'the second line censuses the degraded all-fog1 grid');
});

test('emission on quality change: the first frame after the request, with the NEW level', () => {
  const q = makeQual('occlog=1', { fog: new Uint8Array(12).fill(2), level: 'medium' });
  for (let i = 0; i < 10; i++) q.occQualState(12);
  assert.deepEqual(q.entries, [], 'nothing due yet');
  // The quality change itself: state swaps (as applyQuality
  // leaves it) and the hook requests a line. The request alone
  // must emit NOTHING — the line belongs to a rendered frame.
  q.setState(new Uint8Array(12).fill(2), 'low');
  q.qualLogRequest();
  assert.deepEqual(q.entries, [], 'a request without a rendered frame emits no line');
  q.occQualState(12); // frame 11 — far from the 60-frame cadence
  assert.equal(q.entries.length, 1, 'the first rendered frame after the change emits immediately');
  assert.equal(q.entries[0].level, 'low', 'the line reads the governor\u2019s NEW live level');
  assert.equal(q.entries[0].groundStrokes, false, 'and the NEW Q (Low\u2019s cosmetic set)');
  // Back to Medium, the run #451 direction: same discipline.
  q.setState(new Uint8Array(12).fill(2), 'medium');
  q.qualLogRequest();
  q.qualLogRequest(); // two requests between frames coalesce
  q.occQualState(12);
  assert.equal(q.entries.length, 2, 'coalesced requests still yield exactly one line for the frame');
  assert.deepEqual([q.entries[1].level, q.entries[1].groundStrokes], ['medium', true],
    'Medium restored: level medium, groundStrokes back on — a live line reading low here would prove the latch');
});

// ------------------------------------------------------------------
// Inertness (the task's hard requirement, occorder5 discipline).

test('inert without the flag: requests and frames record nothing', () => {
  const q = makeQual('', { fog: new Uint8Array(12).fill(1), level: 'low' });
  q.qualLogRequest();
  for (let i = 0; i < 120; i++) q.occQualState(12);
  assert.deepEqual(q.entries, [], 'no ?occlog=1, no qualstate lines — and no behaviour change');
});

test('the emitter never throws without occLog (the slicing-harness shape)', () => {
  assert.ok(qualSrc, 'the QUALITYLOG helper block exists in app.js (absent on pristine)');
  const env = {
    occLog: undefined,
    FOG: new Uint8Array(4),
    governor: { level: 'high' },
    Q: settingsFor('high'),
  };
  const factory = new Function(
    ...Object.keys(env),
    `${qualSrc}\nreturn { occQualState, qualLogRequest };`,
  );
  const fns = factory(...Object.values(env));
  assert.doesNotThrow(() => {
    fns.qualLogRequest();
    for (let i = 0; i < 61; i++) fns.occQualState(4);
  }, 'typeof-guarded: harnesses that slice other functions keep working unchanged');
});

// ------------------------------------------------------------------
// No behaviour change.

test('no behaviour change: renderMinimap draws exactly as before (the census is additive)', () => {
  const g = createMinimap({ worldHalf: 100, width: 220, height: 124, pad: 2 });
  // Empty world — the call counts minimap.test.mjs pins for the
  // pre-instrument renderer, restated here against this tree.
  const ctx = makeStubCtx();
  const r = renderMinimap(ctx, g, {
    gridW: 50, gridH: 50, cellSize: 4,
    obstacles: new Uint8Array(50 * 50),
    nodes: [], buildings: [], entities: [], cam: null,
  });
  assert.equal(ctx.calls.save, 1);
  assert.equal(ctx.calls.clip, 1);
  assert.equal(ctx.calls.restore, 1);
  assert.ok(ctx.calls.fill >= 1, 'ground filled');
  assert.ok(ctx.calls.stroke >= 1, 'border stroked');
  assert.equal(ctx.calls.arc, 0, 'no dots in an empty world');
  assert.deepEqual(r, { terrainCells: 0 }, 'the ONLY change is the returned census');
  // Fog scene: the painted colours are the pre-instrument ones,
  // cell for cell (shroud / dim land at height 3 / visible water).
  const GW = 2, GH = 2;
  const fog = new Uint8Array([0, 1, 2, 2]);
  const terrain = new Uint8Array([0, 0, 1, 0]);
  const heights = new Uint8Array([0, 3, 0, 0]);
  const ctx2 = makeStubCtx();
  renderMinimap(ctx2, g, {
    gridW: GW, gridH: GH, cellSize: 4,
    obstacles: new Uint8Array(GW * GH),
    terrain, heights, fog,
    nodes: [], buildings: [], entities: [], cam: null,
  });
  assert.deepEqual(ctx2.fills, ['#05070a', '#2f4a26', '#2e6da8', '#3d6030'],
    'shroud, explored land h3 (dim), visible water, visible land h0 — unchanged colours, unchanged order');
});

test('no behaviour change: qualstate lines only ever append at due frames, nothing else is written', () => {
  // The occorder/selpick sequences ride the same occLog; this
  // instrument's whole footprint on that log is the qualstate
  // lines themselves, appended only at due frames — the full
  // suite's unedited occorder*/selpick* pins are the sequence
  // proof, and this pin covers the log-level additivity here.
  const q = makeQual('occlog=1', { fog: new Uint8Array(12).fill(2), level: 'medium' });
  for (let i = 0; i < 30; i++) q.occQualState(12);
  assert.deepEqual(q.entries.map((e) => e.ev), [], 'non-due frames add nothing to the log');
  q.qualLogRequest();
  q.occQualState(12); // frame 31: the forced line
  assert.deepEqual(q.entries.map((e) => e.ev), ['qualstate'],
    'the due frame appends exactly one qualstate line, nothing else');
  for (let i = 0; i < 28; i++) q.occQualState(12); // frames 32..59
  assert.deepEqual(q.entries.map((e) => e.ev), ['qualstate'],
    'and the frames after it add nothing until the next due frame');
  q.occQualState(12); // frame 60: the periodic line
  assert.deepEqual(q.entries.map((e) => e.ev), ['qualstate', 'qualstate'],
    'the next line is the 60-frame periodic one — the forced line did not reset the cadence');
});
