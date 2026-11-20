// applyKeys pause/menu gate (bot-loop applykeys, 2026-11-20;
// follow-up to run #440, found by the pausebg builder).
//
// THE DEFECT (pristine 6b6a14f): applyKeys(now) pans the camera
// with WASD / arrow keys with NO pause/menu gate, while
// applyEdgeScroll(now) — called at the end of applyKeys —
// refuses to pan when `paused || matchEnded || playback`, when
// the body lacks `.in-match`, or when the help overlay / pause
// menu is visible. So with the Pause menu open (or the help
// overlay, or replay playback, or a finished match), held
// WASD/arrow keys still pan the camera behind the menu —
// inconsistent with the edge-scroll behaviour and with the
// game being paused.
//
// THE FIX (client JS only): the gate condition is extracted
// once into cameraPanGated() — the exact condition
// applyEdgeScroll already used — and the key-pan part of
// applyKeys consults it. applyEdgeScroll keeps calling the
// same gate (its behaviour, including the lastEdgeAt
// bookkeeping that runs before the gates, is unchanged), and
// applyKeys still calls applyEdgeScroll unconditionally, so a
// gated frame still advances the edge-scroll clock. In the
// ungated case nothing changes: same keys, same 14 px/frame.
//
// The harness slices the REAL applyKeys / cameraPanGated /
// applyEdgeScroll out of app.js (occorder-style) and runs them
// against stub state. On the pristine base cameraPanGated does
// not exist, the harness runs applyKeys ungated, and every
// gated case below FAILS (the camera moves); the live-pan and
// edge-scroll cases pass on both trees.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { edgePan } from './hotkeys.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found in app.js`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}

const hasHelper = appJs.includes('function cameraPanGated()');
const helperSrc = hasHelper
  ? sliceFn(appJs, 'function cameraPanGated() {', '\nfunction applyKeys')
  : '';
const applyKeysSrc = sliceFn(appJs, 'function applyKeys(now) {', '\n// --- M13: edge scrolling');
const edgeConstsSrc = sliceFn(appJs, 'const EDGE_MARGIN_PX', '\nfunction applyEdgeScroll');
const applyEdgeScrollSrc = sliceFn(appJs, 'function applyEdgeScroll(now) {', '\n// --- Fullscreen');

function makeHarness({
  held = [], paused = false, matchEnded = false, playback = null,
  inMatch = true, helpVisible = false, pauseMenuVisible = false,
  mouseClientPt = null,
} = {}) {
  const env = {
    keys: new Set(held),
    camOx: 1000, camOy: 500,
    paused, matchEnded, playback,
    document: {
      body: { classList: { contains: (c) => c === 'in-match' && inMatch } },
    },
    helpOverlay: { style: { display: helpVisible ? 'flex' : 'none' } },
    pauseMenu: { style: { display: pauseMenuVisible ? 'flex' : 'none' } },
    mouseClientPt,
    window: { innerWidth: 1280, innerHeight: 800 },
    canvas: { width: 1280, getBoundingClientRect: () => ({ width: 1280 }) },
    edgePan,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${helperSrc}\n${applyKeysSrc}\n${edgeConstsSrc}\n${applyEdgeScrollSrc}\n` +
    'return { applyKeys, applyEdgeScroll, ' +
    'gated: typeof cameraPanGated === "function" ? cameraPanGated : null, ' +
    'get camOx() { return camOx; }, get camOy() { return camOy; } };',
  );
  return factory(...Object.values(env));
}

function panFrames(h, frames = 3) {
  for (let i = 0; i < frames; i++) h.applyKeys(1000 + i * (1000 / 60));
  return [h.camOx, h.camOy];
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine: no shared gate exists).

test('wiring: applyKeys consults the shared cameraPanGated gate — the exact applyEdgeScroll condition, extracted once', () => {
  assert.ok(hasHelper, 'cameraPanGated() is defined in app.js (the extracted shared gate)');
  assert.match(applyKeysSrc, /cameraPanGated\(\)/,
    'the key-pan part of applyKeys is gated by cameraPanGated()');
  assert.match(helperSrc, /paused \|\| matchEnded \|\| playback/,
    'the gate carries the paused / ended / playback leg');
  assert.match(helperSrc, /classList\.contains\('in-match'\)/,
    'the gate carries the body .in-match leg');
  assert.match(helperSrc, /helpOverlay\.style\.display !== 'none'/,
    'the gate carries the help-overlay leg');
  assert.match(helperSrc, /pauseMenu\.style\.display !== 'none'/,
    'the gate carries the pause-menu leg');
  assert.match(applyEdgeScrollSrc, /cameraPanGated\(\)/,
    'applyEdgeScroll consults the same extracted gate (one condition, two callers)');
  // The ungated behaviour is untouched: same speed, same keys,
  // and applyKeys still hands the frame to applyEdgeScroll.
  assert.match(applyKeysSrc, /const P = 14;/, 'the pan speed is still 14 px/frame');
  for (const line of [
    "if (keys.has('KeyA') || keys.has('ArrowLeft')) camOx += P;",
    "if (keys.has('KeyD') || keys.has('ArrowRight')) camOx -= P;",
    "if (keys.has('KeyW') || keys.has('ArrowUp')) camOy += P;",
    "if (keys.has('KeyS') || keys.has('ArrowDown')) camOy -= P;",
    'applyEdgeScroll(now);',
  ]) assert.ok(applyKeysSrc.includes(line), `applyKeys still contains: ${line}`);
});

// ------------------------------------------------------------------
// Gated cases (all FAIL on pristine: the camera pans).

test('(a) paused with the pause menu visible: held WASD/arrows do not move the camera', () => {
  const h = makeHarness({
    held: ['KeyA', 'KeyW', 'ArrowUp'],
    paused: true, pauseMenuVisible: true,
  });
  assert.deepEqual(panFrames(h), [1000, 500],
    'no key pan while the pause menu is open');
});

test('(a) the pause menu visible on its own (display leg, paused flag clear) still gates the key pan', () => {
  const h = makeHarness({ held: ['KeyA'], pauseMenuVisible: true });
  assert.deepEqual(panFrames(h), [1000, 500],
    'the pauseMenu display leg of the gate applies to keys exactly as to edge scroll');
});

test('(b) help overlay visible: held WASD/arrows do not move the camera', () => {
  const h = makeHarness({ held: ['KeyS', 'ArrowRight'], helpVisible: true });
  assert.deepEqual(panFrames(h), [1000, 500],
    'no key pan while the help overlay is up');
});

test('(c) replay playback: held WASD/arrows do not move the camera', () => {
  const h = makeHarness({ held: ['KeyW', 'ArrowLeft'], playback: { idx: 0 } });
  assert.deepEqual(panFrames(h), [1000, 500],
    'no key pan during playback');
});

test('(c) match ended: held WASD/arrows do not move the camera', () => {
  const h = makeHarness({ held: ['KeyA', 'ArrowUp'], matchEnded: true });
  assert.deepEqual(panFrames(h), [1000, 500],
    'no key pan after the match has ended');
});

test('outside a match (body lacks .in-match — lobby / room / loading): held WASD/arrows do not move the camera', () => {
  const h = makeHarness({ held: ['KeyD', 'KeyS'], inMatch: false });
  assert.deepEqual(panFrames(h), [1000, 500],
    'no key pan outside .in-match');
});

// ------------------------------------------------------------------
// Live case (passes on pristine and post-fix: behaviour kept).

test('(d) live unpaused match: WASD pan at 14 px/frame in the established directions', () => {
  const one = (held) => {
    const h = makeHarness({ held });
    h.applyKeys(1000);
    return [h.camOx - 1000, h.camOy - 500];
  };
  assert.deepEqual(one(['KeyA']), [14, 0], 'A pans the view left (+ox)');
  assert.deepEqual(one(['KeyD']), [-14, 0], 'D pans the view right (-ox)');
  assert.deepEqual(one(['KeyW']), [0, 14], 'W pans the view up (+oy)');
  assert.deepEqual(one(['KeyS']), [0, -14], 'S pans the view down (-oy)');
  assert.deepEqual(one(['KeyA', 'KeyW']), [14, 14], 'A+W pans diagonally');
});

test('(d) live unpaused match: arrow keys pan at the same rate, and held keys accumulate per frame', () => {
  const one = (held) => {
    const h = makeHarness({ held });
    h.applyKeys(1000);
    return [h.camOx - 1000, h.camOy - 500];
  };
  assert.deepEqual(one(['ArrowLeft']), [14, 0], 'ArrowLeft = A');
  assert.deepEqual(one(['ArrowRight']), [-14, 0], 'ArrowRight = D');
  assert.deepEqual(one(['ArrowUp']), [0, 14], 'ArrowUp = W');
  assert.deepEqual(one(['ArrowDown']), [0, -14], 'ArrowDown = S');
  const h = makeHarness({ held: ['KeyD'] });
  assert.deepEqual(panFrames(h, 2), [1000 - 28, 500],
    'two frames of held D move the camera exactly 2 x 14 px');
});

// ------------------------------------------------------------------
// Edge scroll through applyKeys is unchanged (regression pin:
// passes on both trees — applyKeys still calls applyEdgeScroll,
// whose own gates and clock bookkeeping are untouched).

test('edge scroll via applyKeys still pans live and still rests gated', () => {
  const atLeftEdge = { mouseClientPt: { x: 0, y: 400 } };
  const live = makeHarness({ held: [], ...atLeftEdge });
  live.applyKeys(1000); // primes lastEdgeAt (dt 0, no step)
  live.applyKeys(1000 + 1000 / 60); // one 60 Hz frame at the left edge
  assert.ok(Math.abs(live.camOx - 1014) < 1e-9 && live.camOy === 500,
    `edge pan still moves the camera one 14 px step (got ${live.camOx}, ${live.camOy})`);
  const gated = makeHarness({ held: [], paused: true, pauseMenuVisible: true, ...atLeftEdge });
  gated.applyKeys(1000);
  gated.applyKeys(1000 + 1000 / 60);
  assert.deepEqual([gated.camOx, gated.camOy], [1000, 500],
    'edge pan still rests while paused');
});
