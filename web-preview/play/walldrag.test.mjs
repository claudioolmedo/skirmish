// Wall-drag commit regression pins (bot-loop walldrag builder,
// 2026-11-21 — runs #453 live evidence, two sessions).
//
// Defect (live): dragging a Palisade wall line produced only 2
// isolated endpoint segments with a large gap, and testers lost
// placement mode mid-placement. Staged here with the REAL client
// drag code — the canvas mousedown, the window mousemove/mouseup
// handlers, attemptPlace and attemptPlaceWallLine sliced out of
// app.js and driven by synthetic mouse events against a stub sim
// (wallghost pattern):
//
//   * A perfect hold-drag (down, intermediate mousemoves, up)
//     always worked — the control rows below pass before and
//     after the fix.
//   * A drag delivered WITHOUT intermediate mousemoves (fast
//     flick, coalesced/dropped moves, automation that presses at
//     the start and releases at the end) collapsed to a single
//     segment at the RELEASE cell: the release handler classified
//     the gesture solely by `lDrag.moved`, a flag only the
//     intermediate mousemove handler sets, and fell into the
//     single-click placement path, which also disarmed placement.
//   * A drag delivered as click-move-click (press+release at the
//     start, move, press+release at the end — the other common
//     automation/human delivery) placed a single segment at the
//     start click, DISARMED placement (the "lost placement mode"
//     symptom), and — after a manual re-arm — a single segment at
//     the end click: exactly the 2 isolated endpoints observed.
//
// Fix (client only, app.js): (1) the release handler now also
// measures the press-to-release screen distance from the release
// event itself (`releaseMoved`), so a drag is a drag however its
// moves were delivered; (2) a plain click with a wall kind armed
// ANCHORS the line (wallAnchorPending) and keeps placement armed
// instead of placing a lone segment — the next plain click
// commits the whole line through the same attemptPlaceWallLine,
// and a second click on the anchor cell itself places the single
// segment. Hold-drag behaviour is unchanged.
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

function makeWorld() {
  const foundations = new Set();
  const issued = [];
  const canvasH = {}, windowH = {};
  const canvas = {
    width: 800, height: 600,
    getBoundingClientRect: () => ({ left: 0, top: 0, width: 800, height: 600 }),
    addEventListener: (t, f) => { canvasH[t] = f; },
  };
  const st = { kinds: [0, 0, 0, 0], owners: [0, 0, 0, 0], pos: [0, 0, 0, 0, 0, 0, -60, -60] };
  const env = {
    GW, GH, CS, HALF, canvas, canvasH, windowH,
    window: { addEventListener: (t, f) => { windowH[t] = f; } },
    addEventListener: (t, f) => { windowH[t] = f; },
    camS: 8, camOx: 400, camOy: 100,
    BUILD_NAME: { 9: 'Palisade Wall', 14: 'Stone Wall' },
    entityState: () => st, selectedUnits: () => [3],
    flash: () => {}, t: (k) => k, playAudioCue: () => {}, playSfx: () => {},
    flareArm: false, repairArm: false, healArm: false, convertArm: false, orderArm: null,
    setFlareArm: () => {}, sendFlare: () => {}, attemptRepair: () => {},
    attemptHeal: () => {}, attemptConvert: () => {}, attemptOrderGround: () => {},
    heightAtWorld: () => 0,
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
    + `function evClient(ev){return {x:ev.clientX,y:ev.clientY};}\n`
    + `function eventScreen(ev){const r=canvas.getBoundingClientRect(); const p=evClient(ev); return [(p.x-r.left)*(canvas.width/r.width),(p.y-r.top)*(canvas.height/r.height)];}\n`
    + `function eventWorld(ev){const r=canvas.getBoundingClientRect(); const p=evClient(ev); const sx=(p.x-r.left)*(canvas.width/r.width); const sy=(p.y-r.top)*(canvas.height/r.height); return s2w(sx,sy,camS,camOx,camOy);}\n`
    + `let placement=null, wallAnchor=null, wallAnchorPending=false, lDrag=null, mouseWorld=null; let selection=new Set([3]);\n`
    + `${sliceFn('function attemptPlace(')}\n${sliceFn('function attemptPlaceWallLine(')}\n${downSrc}\n${moveSrc}\n${upSrc}\n`
    + `return {canvasH, windowH, setPlacement:(p)=>{placement=p;}, get placement(){return placement;}, get wallAnchorPending(){return wallAnchorPending;}, cellScreen:(cx,cy)=>w2s(-HALF+(cx+0.5)*CS,-HALF+(cy+0.5)*CS,camS,camOx,camOy)};`);
  const api = factory(...Object.values(env));
  return { api, foundations, issued };
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

// ----------------------------------------------------------------
// Controls (pass before and after the fix).

test('control: a hold-drag with intermediate moves commits the whole line', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  holdDrag(api, [10, 10], [16, 10]);
  assert.deepEqual(issued.map((c) => c.t), ['buildline']);
  assert.deepEqual([...foundations].sort(), lineCells([10, 10], [16, 10]),
    'every cell of the 7-cell row is founded');
});

test('control: a diagonal hold-drag commits the diagonal chain', () => {
  const { api, foundations } = makeWorld();
  api.setPlacement({ kind: 9 });
  holdDrag(api, [10, 10], [14, 14]);
  assert.deepEqual([...foundations].sort(), lineCells([10, 10], [14, 14]));
});

// ----------------------------------------------------------------
// The defect (fail before the fix, pass after).

test('DEFECT: a drag with NO intermediate mousemoves still commits the whole line', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  api.canvasH.mousedown(evAt(api, 10, 10));
  // No mousemove at all reaches the drag tracker (fast flick /
  // coalesced moves): the release event itself is the only proof
  // the pointer travelled.
  api.windowH.mouseup(evAt(api, 16, 10));
  assert.deepEqual(issued.map((c) => c.t), ['buildline'],
    'the release distance alone must classify the gesture as a drag');
  assert.deepEqual([...foundations].sort(), lineCells([10, 10], [16, 10]),
    'pre-fix this founded only the release cell (16,10)');
});

test('DEFECT: click-move-click commits the whole line, not 2 endpoint segments', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  click(api, [10, 10]); // anchor click
  assert.equal(foundations.size, 0, 'the anchor click alone founds nothing yet');
  assert.deepEqual(api.placement, { kind: 9 },
    'placement mode survives the anchor click (pre-fix it was disarmed)');
  click(api, [16, 10]); // commit click — no re-arm in between
  assert.deepEqual(issued.map((c) => c.t), ['buildline']);
  assert.deepEqual([...foundations].sort(), lineCells([10, 10], [16, 10]),
    'pre-fix this founded only (10,10) and, after a manual re-arm, (16,10)');
});

test('DEFECT bound: click the anchor, then click the SAME cell, places the single segment', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 9 });
  click(api, [10, 10]);
  click(api, [10, 10]);
  assert.deepEqual(issued.map((c) => c.t), ['buildline']);
  assert.deepEqual([...foundations], ['10,10'],
    'a 1-cell line is the single-segment placement, as before');
  assert.equal(api.placement, null, 'placement disarms after the commit, as before');
});

test('DEFECT bound: the click-anchor flow works for the Stone Wall kind too', () => {
  const { api, foundations, issued } = makeWorld();
  api.setPlacement({ kind: 14 });
  click(api, [10, 12]);
  click(api, [13, 12]);
  assert.deepEqual(issued.map((c) => c.t), ['buildline']);
  assert.equal(issued[0].k, 14);
  assert.deepEqual([...foundations].sort(), lineCells([10, 12], [13, 12]));
});

// ----------------------------------------------------------------
// Wiring pins for the fix's disarm paths.

test('wiring: the release classifies drags by the release event distance', () => {
  assert.ok(appJs.includes('const releaseMoved = d.moved'),
    'the mouseup handler computes releaseMoved');
  assert.ok(appJs.includes('wallAnchorPending'),
    'the click-anchor state exists');
});

test('wiring: every placement-disarm path also clears the pending wall anchor', () => {
  for (const sig of ['function resetSim(', 'function adoptSim(']) {
    const start = appJs.indexOf(sig);
    assert.ok(start >= 0, `${sig} exists`);
    const body = appJs.slice(start, appJs.indexOf('\n}\n', start));
    assert.ok(body.includes('wallAnchorPending = false'),
      `${sig} must clear the pending wall anchor for the fresh match`);
  }
});
