// Left-click SELECTION pick instrumentation (bot-loop
// selpicklog, 2026-11-20; the residual occorder22 left open,
// in the FIX #16/#18 style).
//
// THE GAP (occorder22, "Residuals"): the `pick` occLog line
// exists only in orderAt (right-click). The left-click path
// (selectionAt + the plain-click handler in app.js) logged
// NOTHING on a hit or a miss, so a failed left-click on a
// hidden foe could not be discriminated from its own capture
// — run #450's left-click had to be proven by staging three
// pixel classes (occorder22) instead of reading one log line.
//
// THE INSTRUMENT (FIX #21, client-only, ?occlog=1 only):
// every return in selectionAt first records WHICH stage
// resolved it in selectionAt.lastStage (write-only
// bookkeeping; every return statement itself is unchanged),
// and the plain-click handler emits exactly one
//   { ev:'selpick', wx, wy, shift, picked, stage,
//     pickedKind, pickedOwner }
// line per plain click, before the branch consumes the
// candidate — selectionAt's substage when it resolved
// ('ram-body' | 'outline' | 'body' | 'body-ground-shadow' |
// 'body-arena-shadow' | 'fallback-ground-shadow' |
// 'fallback-arena-shadow' | 'record-rect' | 'ground-create'
// | 'arena-create' | 'unitAt'), one of the handler's own
// fallback stages when it did not ('outline-fallback' |
// 'animal' | 'tc' | 'building'), or the null case
// { picked:null, stage:'none' }. INSTRUMENTATION ONLY:
// occLog is inert without the flag, lastStage is never read
// by any pick, and no selection behaviour changes — the
// behavioural assertions below pin the selections the
// staged scenes produced BEFORE this instrument existed
// (occorder21/occorder22's pins, driven here through the
// real handlers).
//
// DISCRIMINATION: on pristine 43a16e5 there is no selpick
// line at all — every selpick assertion below fails there
// (the selections themselves still pass, by design: the
// instrument changes no behaviour), and all pass here.
//
// This harness drives the REAL handler layer sliced verbatim
// from web-preview/app.js (canvas/window mouse listeners and
// the real selection state + helpers) in the occorder21
// style, over occorder22's staged scenes.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { occludedPickAt, occlusionFor } from './occlusion.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found in app.js`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}

const w2sSrc = sliceFn(appJs, 'function w2s(wx, wy, s, ox, oy) {', '\nfunction s2w');
const s2wSrc = sliceFn(appJs, 'function s2w(sx, sy, s, ox, oy) {', '\nfunction centerCamera');
const worldToCellSrc = sliceFn(appJs, 'function worldToCell(wx, wy) {', '\n// --- Placement ghost');
const heightAtWorldSrc = sliceFn(appJs, 'function heightAtWorld(wx, wy) {', '\nfunction fogAtWorld');
const worldToScreenSrc = sliceFn(appJs, 'const worldToScreen = (wx, wy) => {', '\n};') + '\n};';
const evClientSrc = sliceFn(appJs, 'function evClient(ev) {', '\n  // Left-click pick:');
const tcAtSrc = sliceFn(appJs, 'function tcAt(wx, wy) {', '\n  function nodeAt');
const ramBodySrc = sliceFn(appJs, 'function ramBodyD2(st, i, wx, wy) {', '\n  function unitAt');
const unitAtSrc = sliceFn(appJs, 'function unitAt(wx, wy) {', '\n  // Left-click selection pick');
const selectionAtSrc = sliceFn(appJs, 'function selectionAt(wx, wy) {', '\n  // Left-click pick for an animal');
const occSrc = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
const entityAtSrc = sliceFn(appJs, 'function entityAt(wx, wy, screenPick = false) {', '\n  // Placed-building hit test');
const smartOrderSrc = sliceFn(appJs, 'function smartOrderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Context order at a point');
const orderAtSrc = sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting');
const issueSrc = sliceFn(appJs, 'function issue(cmd) {', '\n// M5: the mode decides');
const selStateSrc = sliceFn(appJs, 'let selection = new Set();', '\n// Control groups (M4, SEL-07)');
const isOwnUnitSrc = sliceFn(appJs, 'const isOwnUnit = (st, i) =>', '\n// Every selectable unit of ours');
const ownUnitIdsSrc = sliceFn(appJs, 'function ownUnitIds() {', '\n// The current selection as an id list');
const selectedUnitsSrc = sliceFn(appJs, 'function selectedUnits() {', '\nfunction worldToCell');
const occLogSrc = sliceFn(appJs, 'const OCCLOG_ON', '\nconst canvas = document.getElementById');
const lDownSrc = sliceFn(appJs, "canvas.addEventListener('mousedown', (ev) => {", "\n  window.addEventListener('mousemove', (ev) => {");
const lMoveSrc = sliceFn(appJs, "window.addEventListener('mousemove', (ev) => {", "\n  window.addEventListener('mouseup', (ev) => {");
const lUpSrc = sliceFn(appJs, "window.addEventListener('mouseup', (ev) => {", "\n  canvas.addEventListener('dblclick', (ev) => {");
const keydownSrc = sliceFn(appJs, "addEventListener('keydown', (e) => {", "\naddEventListener('keyup'");

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
  return { occLog: r.occLog, entries: r.occLogEntries, on: r.OCCLOG_ON };
}

const CS = 4, HALF = 240, GW = 120, GH = 120;
const TALL_RECT = { x0: -1.4, y0: -3.9, w: 3.5, h: 4.05 }; // occorder22's tall pose

function makeStubTarget() {
  const listeners = {};
  return {
    __listeners: listeners,
    addEventListener(type, fn) { (listeners[type] ||= []).push(fn); },
  };
}
function fire(target, type, ev) {
  for (const fn of target.__listeners[type] || []) fn(ev);
}

function makeHarness(st, records, issued, flashes, occEntriesBundle) {
  const canvas = Object.assign(makeStubTarget(), {
    width: 1920, height: 1080,
    getBoundingClientRect: () => ({ left: 0, top: 0, width: 1920, height: 1080 }),
  });
  const winTarget = makeStubTarget();
  const { occLog, entries } = occEntriesBundle;
  const applyLocalOrder = (cmd) => { issued.push({ ...cmd, ok: true }); return true; };
  const armBoom = (name) => () => { throw new Error(`arm path taken in harness: ${name}`); };
  const env = {
    canvas,
    window: winTarget,
    addEventListener: (type, fn) => winTarget.addEventListener(type, fn),
    camS: 8, camOx: 960, camOy: 540,
    S_MIN: 1.2, S_MAX: 24,
    pointerLocked: false, virtualCursor: { x: 0, y: 0 },
    ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    lastOccludedOutlines: records,
    occludedPickAt,
    cellVisibleW: () => true,
    cellExploredW: () => true,
    buildingAt: () => null,
    animalAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    localOwner: 0,
    foeOwner: () => 2,
    formationApplies: () => false,
    formationChoice: { tag: () => 0 },
    ackClassForKinds: () => null,
    playAck: () => {},
    playAudioCue: () => {},
    playSelectAck: () => {},
    flash: (m) => flashes.push(m),
    t: (k) => k,
    carryKind: new Set(),
    clickMarkers: { add: () => {} },
    firstGarrisonedRelic: () => null,
    sim: { entity_kinds: () => st.kinds, positions: () => st.pos, tick_count: () => 0 },
    OBST: new Array(GW * GH).fill(0),
    tcHalf: () => 8,
    KIND_TC: 13, KIND_RAM: 17, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
    playback: null, net: null, playing: true, localRecorder: null,
    applyLocalOrder,
    occLog,
    lDrag: null, mouseWorld: null,
    placement: null, wallAnchor: null,
    repairArm: false, healArm: false, convertArm: false,
    orderArm: null, flareArm: false,
    buildPage: null, buildingGroup: null,
    isWallKind: () => false,
    setFlareArm: () => {},
    sendFlare: armBoom('sendFlare'),
    attemptPlace: armBoom('attemptPlace'),
    attemptPlaceWallLine: armBoom('attemptPlaceWallLine'),
    attemptRepair: armBoom('attemptRepair'),
    attemptHeal: armBoom('attemptHeal'),
    attemptConvert: armBoom('attemptConvert'),
    attemptOrderGround: armBoom('attemptOrderGround'),
    renderCommands: () => {},
    keys: new Set(),
    chatInputOpen: () => false,
    openChatInput: armBoom('openChatInput'),
    closeChatInput: () => {},
    chatLink: null,
    document: { activeElement: null },
    helpOverlay: { style: { display: 'none' } },
    diploOverlay: null, scoreOverlay: null, techtreeOverlay: null,
    setDiplo: () => {}, setScore: () => {}, setTechTree: () => {},
    handleGameKey: () => false,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${selStateSrc}\n${isOwnUnitSrc}\n${ownUnitIdsSrc}\n${selectedUnitsSrc}\n` +
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n${evClientSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${selectionAtSrc}\n${occSrc}\n${entityAtSrc}\n${smartOrderSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `${lDownSrc}\n${lMoveSrc}\n${lUpSrc}\n${keydownSrc}\n` +
    `return { selectUnits, selectBuilding, selectAnimal, occDeselect,
      getSelection: () => [...selection], getSelBuilding: () => selectedBuilding, getSelAnimal: () => selectedAnimal,
      worldToScreen };`,
  );
  const fns = factory(...Object.values(env));
  return { fns, occEntries: entries, canvas, winTarget };
}

function blankState(count) {
  return {
    kinds: new Array(count).fill(undefined),
    owners: new Array(count).fill(undefined),
    pos: new Array(count * 2).fill(0),
    hp: new Array(count * 2).fill(0),
    count, garrisoned: [],
  };
}
function put(st, i, kind, owner, x, y, h) {
  st.kinds[i] = kind; st.owners[i] = owner;
  st.pos[i * 2] = x; st.pos[i * 2 + 1] = y; st.hp[i * 2] = h; st.hp[i * 2 + 1] = h;
}

const s2wFn = new Function(`${s2wSrc}\nreturn s2w;`)();

// occorder22's #450 scene, driven through the real handlers:
// TC 0 at the origin; own Militia 4 at (12,2); foe 28 at
// foePos, with an INF-12 record unless withRecord is false.
function makeWorld({ foePos = [-9, 0], withRecord = true, search = 'occlog=1' } = {}) {
  const st = blankState(29);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 4, 3, 0, 12, 2, 40);
  put(st, 28, 2, 2, foePos[0], foePos[1], 45);
  const issued = [];
  const flashes = [];
  const records = [];
  const { fns, occEntries, canvas, winTarget } =
    makeHarness(st, records, issued, flashes, makeOccLog(search));
  if (withRecord) {
    const [tsx, tsy] = fns.worldToScreen(0, 0);
    const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
    const [sx, sy] = fns.worldToScreen(foePos[0], foePos[1]);
    const rect = { x: sx + TALL_RECT.x0 * 8, y: sy + TALL_RECT.y0 * 8, w: TALL_RECT.w * 8, h: TALL_RECT.h * 8 };
    const depth = foePos[0] + foePos[1];
    const res = occlusionFor({ rect, depth, fogVisible: true }, [{ rect: tcCover, depth: 8 }]);
    if (res.occluded) records.push({ id: 28, rect, cover: res.cover.map((o) => o.rect), depth });
  }
  const clickScreen = (sx, sy, shiftKey = false) => {
    const ev = { button: 0, clientX: sx, clientY: sy, shiftKey, preventDefault() {} };
    fire(canvas, 'mousedown', ev);
    fire(winTarget, 'mouseup', ev);
  };
  const clickWorld = (wx, wy, shiftKey = false) => {
    const [sx, sy] = fns.worldToScreen(wx, wy);
    clickScreen(sx, sy, shiftKey);
  };
  const selpicks = () => occEntries.filter((e) => e.ev === 'selpick');
  return { ...fns, issued, flashes, occEntries, selpicks, st, records, clickScreen, clickWorld };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #21 wiring: selectionAt records its stage; the handler emits the selpick line', () => {
  assert.match(selectionAtSrc, /FIX #21 \(bot-loop selpick/,
    'the instrument is marked in selectionAt');
  for (const stage of ['ram-body', 'outline', 'body-ground-shadow', 'body-arena-shadow',
    'body', 'fallback-ground-shadow', 'fallback-arena-shadow',
    'record-rect', 'ground-create', 'arena-create', 'unitAt']) {
    assert.ok(selectionAtSrc.includes(`selectionAt.lastStage = '${stage}';`),
      `selectionAt records the '${stage}' stage`);
  }
  assert.match(lUpSrc, /ev: 'selpick'/, 'the plain-click handler emits the selpick line');
  assert.match(lUpSrc, /const logSelPick = /,
    'the handler resolves the line through its one helper');
  for (const stage of ['outline-fallback', 'animal', 'tc', 'building', 'none']) {
    assert.ok(lUpSrc.includes(`'${stage}'`), `the handler names its '${stage}' stage`);
  }
  // Behaviour by construction: selectionAt's return lines
  // are the pristine ones, in the pristine order.
  assert.ok(selectionAtSrc.includes('return unitAt(wx, wy);'),
    'selectionAt still ends in the unitAt ground pick');
  assert.ok(selectionAtSrc.includes('if (bodyBest !== null) return bodyBest;'),
    'the Ram-body return is unchanged');
  assert.ok(selectionAtSrc.includes('if (occ !== null) return occ;'),
    'the outline return is unchanged');
});

// ------------------------------------------------------------------
// Driven scenes: one selpick line per plain click, naming
// the stage — and the selection itself exactly as before.

test('body: a click on our Militia\'s body logs {picked:4, stage:body} and selects it', () => {
  const w = makeWorld();
  w.clickWorld(11, 0.8);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[4, 'body']],
    'one selpick line: the Militia, resolved by the sprite-body stage');
  assert.equal(w.selpicks()[0].pickedKind, 3, 'the line carries the picked kind');
  assert.equal(w.selpicks()[0].pickedOwner, 0, 'the line carries the picked owner');
  assert.deepEqual(w.getSelection(), [4], 'behaviour unchanged: the Militia is selected');
  assert.deepEqual(w.issued, [], 'no order was issued');
});

test('outline: a click ON the painted outline logs {picked:28, stage:outline} and info-selects the foe', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  assert.equal(w.records.length, 1, 'staging: the foe holds an INF-12 record');
  const [wx, wy] = s2wFn(896, 490, 8, 960, 540);
  w.clickWorld(wx, wy);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[28, 'outline']],
    'one selpick line: the foe, resolved by the outline stage — the run #450 pixel class, now self-reporting');
  assert.equal(w.getSelAnimal(), 28, 'behaviour unchanged: the foe is information-selected');
});

test('record-rect: occorder22\'s class R pixel logs {picked:28, stage:record-rect}', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  const [wx, wy] = s2wFn(880, 478, 8, 960, 540);
  w.clickWorld(wx, wy);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[28, 'record-rect']],
    'one selpick line: the foe, resolved by the FIX #20 record-rect creator');
  assert.equal(w.getSelAnimal(), 28, 'behaviour unchanged: the foe is information-selected');
});

test('ground-create: occorder22\'s class G pixel logs {picked:28, stage:ground-create}', () => {
  const w = makeWorld({ foePos: [1, 1], withRecord: false });
  w.clickWorld(2.2, 1.6);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[28, 'ground-create']],
    'one selpick line: the foe, resolved by the FIX #20 ground creator');
  assert.equal(w.getSelAnimal(), 28, 'behaviour unchanged: the foe is information-selected');
});

test('arena-create: occorder22\'s class A pixel logs {picked:28, stage:arena-create}', () => {
  const w = makeWorld({ foePos: [0, 0], withRecord: false });
  w.clickWorld(3.4, 3.4);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[28, 'arena-create']],
    'one selpick line: the centre foe, resolved by the FIX #20 TC-arena creator');
  assert.equal(w.getSelAnimal(), 28, 'behaviour unchanged: the foe is information-selected');
});

test('null case: an empty-ground click logs {picked:null, stage:none} and still clears the selection', () => {
  const w = makeWorld();
  w.clickWorld(11, 0.8);
  assert.deepEqual(w.getSelection(), [4], 'staging: Militia selected');
  w.clickWorld(30, 30);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[4, 'body'], [null, 'none']],
    'exactly one selpick line per plain click — the second is the null case, self-reporting at last');
  assert.deepEqual(w.getSelection(), [], 'behaviour unchanged: the click clears the selection');
  assert.deepEqual(w.issued, [], 'no order was issued');
});

test('tc fallback: a deliberate TC click past a non-centre foe logs {picked:0, stage:tc} and selects the TC', () => {
  const w = makeWorld({ foePos: [-6, -1], withRecord: false });
  w.clickWorld(7, 7);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[0, 'tc']],
    'one selpick line: selectionAt named nothing, the handler\'s TC stage resolved the click');
  assert.deepEqual(w.getSelBuilding(), { type: 'tc', id: 0, owner: 0 },
    'behaviour unchanged: the TC is selected');
});

test('shift on empty ground: no selection change, and the click still logs {picked:null, stage:none}', () => {
  const w = makeWorld();
  w.clickWorld(11, 0.8);
  assert.deepEqual(w.getSelection(), [4], 'staging: Militia selected');
  w.clickWorld(30, 30, true);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage, e.shift]), [[4, 'body', false], [null, 'none', true]],
    'the shift click gets its selpick line too, shift flag included');
  assert.deepEqual(w.getSelection(), [4], 'behaviour unchanged: shift-click on empty ground keeps the selection');
});

test('inert without the flag: no ?occlog=1, no entries — and the selections still happen', () => {
  const w = makeWorld({ search: '' });
  w.clickWorld(11, 0.8);
  assert.deepEqual(w.getSelection(), [4], 'behaviour unchanged: the Militia selects');
  const w2 = makeWorld({ foePos: [1, 1], withRecord: false, search: '' });
  w2.clickWorld(2.2, 1.6);
  assert.equal(w2.getSelAnimal(), 28, 'behaviour unchanged: the foe info-selects');
  assert.deepEqual(w.occEntries, [], 'occLog recorded nothing without the flag');
  assert.deepEqual(w2.occEntries, [], 'occLog recorded nothing without the flag');
});
