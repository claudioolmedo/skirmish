// Wall-drag2 staging pins (bot-loop walldrag2 builder, 2026-11-21 —
// live run #459 open question).
//
// Live #459: a ~5-tile Palisade drag south of the TC stood only ONE
// segment (-3 wood, exact for 1). Run #455's watch on the SAME
// published build (b274be5) passed with 5 segments (-15) and 4
// segments (-12). Three candidate explanations were staged:
//
//   (a) TERRAIN — the sim founded exactly the individually-valid
//       cells and charged 3 per standing segment (correct DE
//       behaviour). Pinned on the sim side in
//       sim/tests/walldrag2.rs: stood == individually-valid count
//       and charge == 3 x stood across open / TC-reserved /
//       node-blocked drags, and open ground is exactly 5 / -15.
//   (b) CLIENT COLLAPSE — the b274be5 fix's remaining hole: a drag
//       whose press and release SCREEN coordinates disagree (e.g.
//       press on the canvas, release over a floating top/bottom
//       bar) makes `releaseMoved` false while `wallAnchorPending`
//       is also false, so the release falls into the anchor-click
//       branch and the line collapses. Staged below with the REAL
//       sliced handlers, the REAL evClient/eventScreen/eventWorld
//       (not re-implementations), and the REAL pointer-lock router
//       (routeLockedEvent/fireSynthetic): unlocked bar release,
//       locked bar release (router stopPropagation + synthetic
//       re-dispatch to the bar element, bubbling to window), and
//       locked events whose own clientX/Y are frozen at the press
//       point (the documented pointer-lock behaviour evClient
//       exists for).
//   (c) GHOST MISMATCH — the preview promised more than was
//       placed. Pinned below: for every delivery, the committed
//       buildline endpoints equal the anchor/release cells and the
//       committed cell set equals wallLineCells(anchor, release)
//       — the exact expansion the ghost draws — and for a tracked
//       drag whose release is its last move, ghost set == committed
//       set. (Per-cell green/red validity is the wallghost front,
//       already closed: the ghost's per-cell verdicts mirror the
//       sim, so a preview can honestly show red cells that the sim
//       will skip — that is (a) visible in advance, not (c).)
//
// Verdict of this staging (see handoff walldrag2-build.md): every
// client delivery below commits the full line on the current
// build — no collapse path was found. The only client sequence
// that yields a 1-cell commit is anchor-click + click on the
// anchor cell itself, which is the deliberate single-segment
// placement, pinned as a bound below.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { wallLineCells } from './walls.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(sig) {
  const s = appJs.indexOf(sig);
  assert.ok(s >= 0, `${sig} exists`);
  let i = appJs.indexOf('{', s), d = 0, j = i;
  for (; j < appJs.length; j++) {
    if (appJs[j] === '{') d++;
    else if (appJs[j] === '}') { d--; if (d === 0) break; }
  }
  return appJs.slice(s, j + 1);
}
function sliceTo(sig, endSig) {
  const s = appJs.indexOf(sig);
  assert.ok(s >= 0, `${sig} exists`);
  const e = appJs.indexOf(endSig, s);
  assert.ok(e > s, `${endSig} follows ${sig}`);
  return appJs.slice(s, e);
}
const downSrc = sliceTo(
  "canvas.addEventListener('mousedown', (ev) => {\n    if (ev.button !== 0) return;",
  "window.addEventListener('mousemove', (ev) => {");
const moveSrc = sliceTo(
  "window.addEventListener('mousemove', (ev) => {\n    if (!lDrag) return;",
  "window.addEventListener('mouseup', (ev) => {");
const upSrc = sliceTo(
  "window.addEventListener('mouseup', (ev) => {",
  "canvas.addEventListener('dblclick'");

// Staged world: 40x40 cells, CS 4, HALF 80, one villager (id 3).
const GW = 40, GH = 40, CS = 4, HALF = 80;
const cellOf = (x, y) => [Math.max(0, Math.min(GW - 1, Math.floor((x + HALF) / CS))),
  Math.max(0, Math.min(GH - 1, Math.floor((y + HALF) / CS)))];

function makeWorld({ locked = false } = {}) {
  const foundations = new Set();
  const issued = [];
  const canvasH = {}, windowH = {};
  const canvas = {
    width: 800, height: 600,
    getBoundingClientRect: () => ({ left: 0, top: 0, width: 800, height: 600 }),
    addEventListener: (t, f) => { canvasH[t] = f; },
  };
  // A floating bar element (top/bottom HUD band): it has no mouseup
  // handler of its own; a (synthetic) event dispatched to it BUBBLES
  // to the window listeners, exactly as in a real browser.
  const barEl = {
    dispatch: (type, init) => {
      const ev = { button: 0, shiftKey: false, ...init };
      if (windowH[type]) windowH[type](ev);
    },
  };
  const st = { kinds: [0, 0, 0, 0], owners: [0, 0, 0, 0], pos: [0, 0, 0, 0, 0, 0, -60, -60] };
  const virtualCursor = { x: 0, y: 0 };
  const hitRef = { el: canvas }; // what elementFromPoint reports (router world)
  const env = {
    GW, GH, CS, HALF, canvas, canvasH, windowH, barEl,
    window: { addEventListener: (t, f) => { windowH[t] = f; }, innerWidth: 800, innerHeight: 600 },
    addEventListener: (t, f) => { windowH[t] = f; },
    document: { elementFromPoint: () => hitRef.el },
    camS: 8, camOx: 400, camOy: 100, ELEV_LIFT: 1.1,
    pointerLocked: locked, virtualCursor,
    BUILD_NAME: { 9: 'Palisade Wall', 14: 'Stone Wall' },
    entityState: () => st, selectedUnits: () => [3],
    flash: () => {}, t: (k) => k, playAudioCue: () => {}, playSfx: () => {},
    flareArm: false, repairArm: false, healArm: false, convertArm: false, orderArm: null,
    setFlareArm: () => {}, sendFlare: () => {}, attemptRepair: () => {},
    attemptHeal: () => {}, attemptConvert: () => {}, attemptOrderGround: () => {},
    heightAtWorld: () => 0,
    positionLockCursor: () => {},
    issue: (cmd) => {
      issued.push(cmd);
      if (cmd.t === 'buildline') {
        const a = cellOf(cmd.x0, cmd.y0), b = cellOf(cmd.x1, cmd.y1);
        let any = false;
        for (const [x, y] of wallLineCells(a[0], a[1], b[0], b[1])) {
          foundations.add(x + ',' + y); any = true;
        }
        return any;
      }
      if (cmd.t === 'build') {
        const c = cellOf(cmd.x, cmd.y);
        foundations.add(c[0] + ',' + c[1]);
        return true;
      }
      return true;
    },
  };
  const factory = new Function(...Object.keys(env),
    `${sliceFn('function w2s(')}\n${sliceFn('function s2w(')}\n${sliceFn('function worldToCell(')}\n${sliceFn('function isWallKind(')}\n`
    + `${sliceFn('function evClient(')}\n${sliceFn('function eventWorld(')}\n${sliceFn('function eventScreen(')}\n`
    + `let placement=null, wallAnchor=null, wallAnchorPending=false, lDrag=null, mouseWorld=null; let selection=new Set([3]);\n`
    + `let routingSyntheticDepth=0, lockDragTarget=null;\n`
    + `${sliceFn('function fireSynthetic(')}\n${sliceFn('function routeLockedEvent(')}\n`
    + `${sliceFn('function attemptPlace(')}\n${sliceFn('function attemptPlaceWallLine(')}\n${downSrc}\n${moveSrc}\n${upSrc}\n`
    + `return {canvasH, windowH, virtualCursor, routeLockedEvent, setPlacement:(p)=>{placement=p;}, get placement(){return placement;}, get wallAnchorPending(){return wallAnchorPending;}, get wallAnchor(){return wallAnchor;}, cellScreen:(cx,cy)=>w2s(-HALF+(cx+0.5)*CS,-HALF+(cy+0.5)*CS,camS,camOx,camOy)};`);
  const api = factory(...Object.values(env));
  api.setHit = (el) => { hitRef.el = el; };
  return { api, foundations, issued, canvas, barEl };
}

const evAt = (api, cx, cy, button = 0) => {
  const [x, y] = api.cellScreen(cx, cy);
  return { button, clientX: x, clientY: y, shiftKey: false };
};
const evXY = (x, y, button = 0) => ({ button, clientX: x, clientY: y, shiftKey: false });
function holdDrag(api, a, b, steps = 5) {
  api.canvasH.mousedown(evAt(api, ...a));
  const [x0, y0] = api.cellScreen(...a), [x1, y1] = api.cellScreen(...b);
  for (let i = 1; i <= steps; i++) {
    api.windowH.mousemove(evXY(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps));
  }
  api.windowH.mouseup(evAt(api, ...b));
}
function click(api, c) {
  api.canvasH.mousedown(evAt(api, ...c));
  api.windowH.mouseup(evAt(api, ...c));
}
const lineCells = (a, b) => wallLineCells(a[0], a[1], b[0], b[1]).map(([x, y]) => x + ',' + y).sort();
const cmdCells = (cmd) => [cellOf(cmd.x0, cmd.y0), cellOf(cmd.x1, cmd.y1)];

// ----------------------------------------------------------------
// (b/c) South drags in every delivery: endpoints + ghost expansion.

test('south drag (cell-space +y, 5 cells), tracked: endpoints and ghost expansion match the commit', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  holdDrag(api, [10, 10], [10, 14]);
  assert.deepEqual(issued.map((c) => c.t), ['buildline']);
  assert.deepEqual(cmdCells(issued[0]), [[10, 10], [10, 14]],
    'the committed endpoints are exactly the anchor and release cells');
  assert.deepEqual([...foundations].sort(), lineCells([10, 10], [10, 14]),
    'committed set == wallLineCells(anchor, release) — the ghost expansion');
  assert.equal(foundations.size, 5);
});

test('screen-south diagonal drag (5 cells), tracked and flick: full line both ways', () => {
  for (const flick of [false, true]) {
    const { api, foundations, issued } = makeWorld();
    api.setPlacement({ kind: 9 });
    if (flick) {
      api.canvasH.mousedown(evAt(api, 10, 10));
      api.windowH.mouseup(evAt(api, 14, 14)); // no intermediate moves at all
    } else {
      holdDrag(api, [10, 10], [14, 14]);
    }
    assert.deepEqual(issued.map((c) => c.t), ['buildline'], `flick=${flick}`);
    assert.deepEqual(cmdCells(issued[0]), [[10, 10], [14, 14]], `flick=${flick}`);
    assert.deepEqual([...foundations].sort(), lineCells([10, 10], [14, 14]), `flick=${flick}`);
    assert.equal(foundations.size, 5, `flick=${flick}`);
  }
});

test('click-anchor + click commit, south: full line, endpoints are the two click cells', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  click(api, [12, 10]);
  assert.equal(issued.length, 0, 'the anchor click commits nothing');
  click(api, [12, 14]);
  assert.deepEqual(issued.map((c) => c.t), ['buildline']);
  assert.deepEqual(cmdCells(issued[0]), [[12, 10], [12, 14]]);
  assert.equal(foundations.size, 5);
});

// ----------------------------------------------------------------
// (b) Release over a floating bar — unlocked.

test('BAR (unlocked): press on canvas, release over the bottom bar band commits the full line', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  // Anchor (28,28) screens at y=372 (canvas band); release (33,33)
  // screens at y=532 — inside the bottom bar's band (y >= 500 of
  // the 600px canvas). The release event's target is the bar; the
  // window mouseup listener receives it by bubbling, and both
  // eventScreen reads go through the canvas rect, so the press
  // and release screen coordinates AGREE by construction.
  const [, ay] = api.cellScreen(28, 28);
  const [, ry] = api.cellScreen(33, 33);
  assert.ok(ay < 500 && ry >= 500, `staging sanity: anchor y=${ay} on canvas, release y=${ry} over the bar`);
  holdDrag(api, [28, 28], [33, 33]);
  assert.deepEqual(issued.map((c) => c.t), ['buildline'],
    'a bar release must not collapse into the anchor-click branch');
  assert.deepEqual(cmdCells(issued[0]), [[28, 28], [33, 33]]);
  assert.equal(foundations.size, 6);
  assert.equal(api.placement, null, 'the commit disarms placement, as any commit does');
  assert.equal(api.wallAnchorPending, false, 'no pending anchor is left behind');
});

test('BAR (unlocked), flick variant: no moves, release over the bar still commits the full line', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  api.canvasH.mousedown(evAt(api, 28, 28));
  api.windowH.mouseup(evAt(api, 33, 33));
  assert.deepEqual(issued.map((c) => c.t), ['buildline']);
  assert.equal(foundations.size, 6);
});

// ----------------------------------------------------------------
// (b) Release over a floating bar — pointer-locked, through the
// REAL router. Under lock the browser retargets every event to the
// canvas and an event's own clientX/Y freeze at the capture point;
// evClient substitutes the virtual cursor. The router stops a
// mouseup whose virtual point is over a HUD element and
// re-dispatches a synthetic to that element, which bubbles to the
// window handler.

function lockedDrag(api, a, b, { canvas, barEl }) {
  // Press: virtual cursor at the anchor; elementFromPoint = canvas
  // -> the router passes the event through to the canvas handler.
  // The event's own clientX/Y are FROZEN at (0,0) (capture point).
  const [ax, ay] = api.cellScreen(...a);
  api.virtualCursor.x = ax; api.virtualCursor.y = ay;
  api.setHit(canvas);
  const downEv = { button: 0, clientX: 0, clientY: 0, shiftKey: false, stopped: false,
    stopPropagation() { this.stopped = true; }, preventDefault() {} };
  api.routeLockedEvent('mousedown', downEv);
  assert.equal(downEv.stopped, false, 'a press over the canvas passes the router');
  api.canvasH.mousedown(downEv);
  // Moves: mousemove is never routed; the virtual cursor advances
  // (movement deltas) and the tracker reads it via evClient.
  const [bx, by] = api.cellScreen(...b);
  for (let i = 1; i <= 5; i++) {
    api.virtualCursor.x = ax + (bx - ax) * i / 5;
    api.virtualCursor.y = ay + (by - ay) * i / 5;
    api.windowH.mousemove({ button: 0, clientX: 0, clientY: 0, shiftKey: false });
  }
  // Release: virtual cursor at the release cell, which is over the
  // floating bar -> the router stops the original and re-dispatches
  // a synthetic mouseup to the bar element; it bubbles to window.
  api.virtualCursor.x = bx; api.virtualCursor.y = by;
  api.setHit(barEl);
  const upEv = { button: 0, clientX: 0, clientY: 0, shiftKey: false, stopped: false,
    stopPropagation() { this.stopped = true; }, preventDefault() {} };
  api.routeLockedEvent('mouseup', upEv);
  assert.equal(upEv.stopped, true, 'staging sanity: the router did intercept the bar release');
  // (fireSynthetic has already delivered the synthetic to the bar
  // element inside routeLockedEvent, and the bar's dispatch bubbled
  // it to the window mouseup handler.)
}

test('BAR (pointer-locked, real router): frozen event coords + routed release still commit the full line', () => {
  const { api, foundations, issued, canvas, barEl } = makeWorld({ locked: true });
  api.setPlacement({ kind: 9 });
  lockedDrag(api, [28, 28], [33, 33], { canvas, barEl });
  assert.deepEqual(issued.map((c) => c.t), ['buildline'],
    'the routed synthetic release reaches the window handler with the virtual release point');
  assert.deepEqual(cmdCells(issued[0]), [[28, 28], [33, 33]],
    'endpoints come from the virtual cursor at press and at release, not the frozen clientX/Y');
  assert.equal(foundations.size, 6);
  assert.equal(api.wallAnchorPending, false);
});

test('BAR (pointer-locked, real router), flick: no moves, routed release commits the full line', () => {
  const { api, foundations, issued, canvas, barEl } = makeWorld({ locked: true });
  api.setPlacement({ kind: 9 });
  const [ax, ay] = api.cellScreen(28, 28);
  api.virtualCursor.x = ax; api.virtualCursor.y = ay;
  api.setHit(canvas);
  const downEv = { button: 0, clientX: 0, clientY: 0, shiftKey: false, stopped: false,
    stopPropagation() { this.stopped = true; }, preventDefault() {} };
  api.routeLockedEvent('mousedown', downEv);
  api.canvasH.mousedown(downEv);
  const [bx, by] = api.cellScreen(33, 33);
  api.virtualCursor.x = bx; api.virtualCursor.y = by;
  api.setHit(barEl);
  const upEv = { button: 0, clientX: 0, clientY: 0, shiftKey: false, stopped: false,
    stopPropagation() { this.stopped = true; }, preventDefault() {} };
  api.routeLockedEvent('mouseup', upEv);
  assert.equal(upEv.stopped, true);
  assert.deepEqual(issued.map((c) => c.t), ['buildline'],
    'releaseMoved is measured on the virtual release point, so a flick over a bar is still a drag');
  assert.equal(foundations.size, 6);
});

// ----------------------------------------------------------------
// Bounds: the ONLY 1-cell commits, and the anchor branch itself.

test('BOUND: a plain click alone commits nothing — it only anchors (the collapse signature, first half)', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  click(api, [10, 10]);
  assert.equal(issued.length, 0);
  assert.equal(foundations.size, 0);
  assert.equal(api.wallAnchorPending, true);
  assert.deepEqual(api.placement, { kind: 9 }, 'placement stays armed after an anchor click');
});

test('BOUND: anchor click + click on the SAME cell is the single-segment placement (1 cell by design)', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  click(api, [10, 10]);
  click(api, [10, 10]);
  assert.deepEqual(issued.map((c) => c.t), ['buildline']);
  assert.deepEqual(cmdCells(issued[0]), [[10, 10], [10, 10]]);
  assert.deepEqual([...foundations], ['10,10']);
});

test('BOUND: after an anchor click, a click elsewhere commits from the ANCHOR, never a lone cell at the click', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  click(api, [10, 10]); // anchor
  click(api, [10, 11]); // one cell away: a 2-cell line from the anchor, not a lone (10,11)
  assert.deepEqual(cmdCells(issued[0]), [[10, 10], [10, 11]]);
  assert.equal(foundations.size, 2);
});

// ----------------------------------------------------------------
// Wiring pins: the coordinate agreement the verdict rests on.

test('wiring: press and release screen coords share one accessor (evClient) and one canvas rect', () => {
  for (const sig of ['function eventScreen(', 'function eventWorld(']) {
    const body = sliceFn(sig);
    assert.ok(body.includes('evClient(ev)'), `${sig} reads the point through evClient`);
    assert.ok(body.includes('canvas.getBoundingClientRect()'), `${sig} uses the canvas rect`);
    assert.ok(!body.includes('ev.clientX'), `${sig} never reads the raw event point directly`);
  }
  const upStart = appJs.indexOf("window.addEventListener('mouseup', (ev) => {");
  const upBlock = appJs.slice(upStart, appJs.indexOf('attemptPlaceWallLine', upStart));
  assert.ok(upBlock.includes('const [rsx, rsy] = eventScreen(ev);'),
    'the release distance is measured with the same eventScreen as the press');
  assert.ok(upBlock.includes('Math.abs(rsx - d.x0) + Math.abs(rsy - d.y0) > 6'),
    'releaseMoved compares release screen against press screen in the same space');
});

test('wiring: mousemove is never stopped by the pointer-lock router', () => {
  const start = appJs.indexOf('const LOCK_ROUTED_TYPES = [');
  assert.ok(start >= 0);
  const block = appJs.slice(start, appJs.indexOf('];', start));
  assert.ok(!block.includes("'mousemove'"),
    'the lDrag tracker and mouseWorld see every move, over canvas and bars alike');
  assert.ok(block.includes("'mouseup'"),
    'mouseup IS routed (and staged above through the real router)');
});
