// selbody — the run #460 selection signature (bot-loop,
// 2026-11-21, live build 8c912d0 = this tree's app.js,
// unchanged through f677f43): a left-click on an enemy
// Militia's red OUTLINE under the player's Town Center roof
// logged
//   { ev:'selpick', picked:67, stage:'body',
//     pickedKind:3, pickedOwner:2 }
// but the Selection panel stayed "No selection" — the pick
// resolved the enemy, the selection was (reportedly) not
// applied. Contrasts from the same session: a foe ON the
// roof (stage 'arena-create') DID select; the same Militia
// stepped OUT from under the roof (stage 'outline',
// picked:67) DID select.
//
// TASK: stage the exact geometry in the real-handler harness
// (selpicklog idiom) and find WHERE the picked enemy is
// dropped between the selpick log and the selection
// application on the stage-'body' path.
//
// FINDING (this file is the evidence): NO drop reproduces.
// The stage-'body' pixel class exists exactly as logged —
// with an INF-12 record present, body-box pixels of the foe
// that fall OUTSIDE the record's rect n cover resolve
// through selectionAt's sprite-body stage (the outline stage
// runs first and misses there), stage 'body', picked 67 —
// and in every staged scene the handler applies the
// information selection (selectAnimal(67)), and the panel
// head (updateSelPanel's bookkeeping + context/signature
// computation, driven below) keeps it at context 'animal'.
// Sweeps behind these pins (probe, 2026-11-21): foe Militia
// 67 inside the TC footprint at (0,0)/(2,2)/(-3,1)/(4,4)/
// (0,-5), with and without a record, an own Villager
// bystander at (6,6): ~2,068 clicks per scene over the foe's
// outline + body region — every stage-'body' click
// (99/scene without a record) and every picked:67 click of
// ANY stage landed with getSelAnimal() === 67. The handler's
// non-own branch is stage-blind by construction: logSelPick
// then selectAnimal(v) for any v selectionAt returns.
//
// WHAT REMAINS UNEXCLUDED (not staged, by honesty): a
// shift-modified click — the handler's shift branch logs the
// selpick line and by design changes NO selection (pinned
// as characterization below; the run #460 excerpt preserved
// in the log does not carry the entry's shift field, so this
// can neither be confirmed nor excluded from the capture) —
// and any post-click clearer in the live session after the
// click (a later click/Esc/portrait in the same observation
// window), which no client code path ties to the 'body'
// stage. NO FIX is shipped with these pins: the discipline
// is a proven mechanism or nothing.
//
// These pins now stand guard over the exact signature: if a
// future change ever drops a stage-'body' pick between the
// log and selectAnimal, scenes 1-3 fail.
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
    buildingArtAt: () => null, // FIX #23: no placed buildings in this harness's scenes
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
function makeWorld({ foePos = [-9, 0], withRecord = true, search = 'occlog=1', ownAt = null } = {}) {
  const st = blankState(80);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 4, 3, 0, 12, 2, 40);
  put(st, 67, 3, 2, foePos[0], foePos[1], 40);
  if (ownAt) put(st, 5, 0, 0, ownAt[0], ownAt[1], 25);
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
    if (res.occluded) records.push({ id: 67, rect, cover: res.cover.map((o) => o.rect), depth });
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
// The live signature, staged: stage 'body', picked 67.

test('body, no record: a click on the centre foe\'s body logs {picked:67, stage:body} and info-selects it', () => {
  const w = makeWorld({ foePos: [0, 0], withRecord: false });
  const [fx, fy] = w.worldToScreen(0, 0);
  w.clickScreen(fx, fy - 16); // 2.0 wu up the Militia's body box
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[67, 'body']],
    'the live signature itself: the sprite-body stage resolves the foe under the roof');
  assert.equal(w.selpicks()[0].pickedKind, 3, 'the line carries the Militia kind');
  assert.equal(w.selpicks()[0].pickedOwner, 2, 'the line carries the enemy owner');
  assert.equal(w.getSelAnimal(), 67, 'the selection IS applied: the foe is information-selected');
  assert.deepEqual(w.getSelection(), [], 'no unit selection rides along');
  assert.equal(w.getSelBuilding(), null, 'the TC is not selected instead');
  assert.deepEqual(w.issued, [], 'no order was issued');
});

test('body, no record, off-centre foe: the same signature at (2,2) inside the footprint', () => {
  const w = makeWorld({ foePos: [2, 2], withRecord: false });
  const [fx, fy] = w.worldToScreen(2, 2);
  w.clickScreen(fx, fy - 16);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[67, 'body']]);
  assert.equal(w.getSelAnimal(), 67, 'the foe is information-selected');
});

test('body with a standing selection: the info selection replaces our Militia, as any foe pick does', () => {
  const w = makeWorld({ foePos: [0, 0], withRecord: false });
  w.clickWorld(11, 0.8);
  assert.deepEqual(w.getSelection(), [4], 'staging: our Militia selected first');
  const [fx, fy] = w.worldToScreen(0, 0);
  w.clickScreen(fx, fy - 16);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[4, 'body'], [67, 'body']],
    'both clicks resolve through the body stage (ours, then the foe)');
  assert.equal(w.getSelAnimal(), 67, 'the foe is information-selected');
  assert.deepEqual(w.getSelection(), [], 'selectAnimal cleared the unit selection, as designed');
});

test('body WITH a record: body-box pixels outside rect n cover resolve stage body (the run #460 pixel class)', () => {
  // Foe at (-9,0) with selpicklog's tall-pose record: the
  // record's cover intersection covers only part of the
  // Militia's body box, so pixels low/top on the body miss
  // the outline stage and resolve through the body stage —
  // with the record live in lastOccludedOutlines, exactly
  // the live frame state. (At the foe's screen centre the
  // outline stage wins instead — the contrast pin below.)
  const w = makeWorld({ foePos: [-9, 0], withRecord: true });
  assert.equal(w.records.length, 1, 'staging: the foe holds an INF-12 record');
  const [fx, fy] = w.worldToScreen(-9, 0);
  w.clickScreen(fx, fy - 16);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[67, 'body']],
    'a body pixel outside the painted-outline intersection: stage body, with a record present');
  assert.equal(w.getSelAnimal(), 67, 'the foe is information-selected');
});

// ------------------------------------------------------------------
// Contrasts from the same live session (both select).

test('contrast outline: a pixel inside rect n cover logs stage outline and selects the same foe', () => {
  const w = makeWorld({ foePos: [0, 0], withRecord: true });
  const [fx, fy] = w.worldToScreen(0, 0);
  w.clickScreen(fx, fy - 16);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[67, 'outline']],
    'the same screen offset, record present and foe at the centre: the outline stage resolves it');
  assert.equal(w.getSelAnimal(), 67, 'the foe is information-selected');
});

test('contrast arena-create: a low inside-footprint click logs stage arena-create and selects', () => {
  const w = makeWorld({ foePos: [0, 0], withRecord: false });
  w.clickWorld(3.4, 3.4);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage]), [[67, 'arena-create']]);
  assert.equal(w.getSelAnimal(), 67, 'the foe is information-selected');
});

// ------------------------------------------------------------------
// Characterization (NOT the proven live cause): the one
// handler path that logs a selpick line for a foe and
// applies no selection is a shift-modified click — the
// shift branch exists to add/remove OUR units only, so a
// non-own candidate is logged and left alone, by design.
// The run #460 capture excerpt does not preserve the
// entry's shift field; this pin bounds the signature
// without claiming it.

test('characterization: shift + body click logs {picked:67, stage:body} and changes no selection', () => {
  const w = makeWorld({ foePos: [0, 0], withRecord: false });
  const [fx, fy] = w.worldToScreen(0, 0);
  w.clickScreen(fx, fy - 16, true);
  assert.deepEqual(w.selpicks().map((e) => [e.picked, e.stage, e.shift]), [[67, 'body', true]],
    'the shift click gets the same selpick line, shift flag included');
  assert.equal(w.getSelAnimal(), null, 'by handler design a shift click info-selects nobody');
  assert.deepEqual(w.getSelection(), [], 'and selects no unit');
});

// ------------------------------------------------------------------
// Panel head: updateSelPanel's bookkeeping is the only
// per-frame clearer of an info selection. Driven here (the
// head of the real function, sliced verbatim: the dead-TC
// guard, the animal bookkeeping, the context + signature
// computation — everything before the render branches).

const panelHeadSrc = (() => {
  const start = appJs.indexOf('function updateSelPanel() {');
  assert.ok(start >= 0, 'updateSelPanel found in app.js');
  const sigMark = 'if (sig === selSig) return;';
  const end = appJs.indexOf(sigMark, start);
  assert.ok(end > start, 'the signature gate found in updateSelPanel');
  const after = appJs.indexOf('selSig = sig;', end);
  assert.ok(after > end, 'the signature store follows the gate');
  return appJs.slice(start, after + 'selSig = sig;'.length) + '\n  return context;\n}';
})();

function makePanel(st) {
  const env = {
    entityState: () => st,
    localOwner: 0,
    foeOwner: () => 2,
    sim: {
      building_train_block: () => -1,
      building_queue: () => [],
      entity_display_stats: () => null,
      building_display_stats: () => null,
      market_prices: () => [0, 0, 0, 0, 0, 0],
      tc_garrison_count: () => 0,
      building_garrison_count: () => 0,
    },
    buildingList: () => [],
    myTrainQueueLen: () => 0,
    myTrainBlock: () => null,
    myTcQueue: () => [],
    myTcProgress: () => 0,
    myAge: () => 0,
    myTechMask: () => 0,
    KIND_TC: 13, KIND_SHEEP: 5,
    buildPage: null, buildingGroup: null,
    occLog: () => {},
  };
  const factory = new Function(
    ...Object.keys(env),
    `${selStateSrc}\n${isOwnUnitSrc}\n${selectedUnitsSrc}\nlet selSig = '';\n${panelHeadSrc}\n` +
    `return { selectAnimal, selectUnits, updateSelPanel,
      getAnimal: () => selectedAnimal, getBuilding: () => selectedBuilding };`,
  );
  return factory(...Object.values(env));
}

test('panel head: an info-selected enemy Militia survives the per-frame bookkeeping at context animal', () => {
  const st = blankState(80);
  put(st, 0, 13, 0, 0, 0, 2185); // the draining TC of the live scene
  put(st, 67, 3, 2, 0, 0, 40);
  const p = makePanel(st);
  p.selectAnimal(67);
  const context = p.updateSelPanel();
  assert.equal(p.getAnimal(), 67, 'the bookkeeping does not drop a living enemy info selection');
  assert.equal(context, 'animal', 'the panel context for the state is the animal (info) context — not none');
});

test('panel head control: the same bookkeeping DOES drop a dead foe — the drive is real', () => {
  const st = blankState(80);
  put(st, 0, 13, 0, 0, 0, 2185);
  put(st, 67, 3, 2, 0, 0, 40);
  st.hp[67 * 2] = 0; // the foe dies under the panel
  const p = makePanel(st);
  p.selectAnimal(67);
  const context = p.updateSelPanel();
  assert.equal(p.getAnimal(), null, 'a dead info-selected foe drops, exactly as documented');
  assert.equal(context, 'none', 'and only then does the panel read no-selection');
});
