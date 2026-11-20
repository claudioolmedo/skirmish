// Occluded-outline ORDER degradation, round 21 (bot-loop
// occorder21, 2026-11-20; follow-up to occorder20 / run #446).
//
// THE OPEN QUESTION (occorder20's verdict): run #446's failed
// right-click was proven to be orderAt's empty-selection
// return — at click time the unit selection was ALREADY
// empty, and FIX #16 made that return self-report
// ('order-skip'). But WHO cleared the selection is still
// unproven: within the client the selection is emptied only
// by a handful of paths — the Esc stack's last step, a plain
// left-click or selection box on empty ground, the portrait
// click, selectAnimal / selectBuilding replacing the unit
// selection, and a match (re)start — and none of them logged
// anything, so a capture that starts at the failed click can
// never name the clearer.
//
// THE FIX (FIX #18, client instrumentation only): every one
// of those paths now calls occDeselect(via) BEFORE clearing,
// which records { ev:'deselect', via, size } under the same
// ?occlog=1 occLog mechanism (inert without the flag). via is
// one of 'esc' | 'empty-ground' | 'portrait' | 'animal' |
// 'building' | 'restart'; size is the unit-selection size
// BEFORE the clear. Only a non-empty unit selection logs —
// clearing an already-empty selection clears nothing and is
// not a deselection event. Behaviour is unchanged everywhere:
// no selection logic moves, no order path is touched, and
// with the flag off the log call returns before doing
// anything (proven below).
//
// This harness drives the REAL handler layer sliced verbatim
// from web-preview/app.js (canvas/window mouse listeners, the
// window keydown listener's Esc stack, and the real selection
// state + helpers incl. occDeselect) in the occorder20 style.
// The portrait listener and the three match-entry resets
// (resetSim / adoptSim / adoptLoadedSim) live inside large
// UI/match functions that cannot be sliced whole; those two
// paths are pinned by wiring tests over their exact source
// regions, and every other path is driven end-to-end.
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
function sliceIncl(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found in app.js`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b + endMarker.length);
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
    // Keydown / Esc-stack state.
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

// The occorder20 world, minus the occlusion staging (not
// needed here): TC 0; villagers 1..3; own Militia 4 at
// (12,2); foe Spearman 56 at (1,1).
function makeWorld(search = 'occlog=1') {
  const st = blankState(57);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 1, 0, 0, 5, 0, 50);
  put(st, 2, 0, 0, -5, 0, 50);
  put(st, 3, 0, 0, 0, 5, 50);
  put(st, 4, 3, 0, 12, 2, 40);
  put(st, 56, 9, 2, 1, 1, 45);
  const issued = [];
  const flashes = [];
  const records = [];
  const { fns, occEntries, canvas, winTarget } =
    makeHarness(st, records, issued, flashes, makeOccLog(search));
  const evAt = (world, button) => {
    const [sx, sy] = fns.worldToScreen(world[0], world[1]);
    return { button, clientX: sx, clientY: sy, shiftKey: false, preventDefault() {} };
  };
  const leftClick = (world) => {
    const ev = evAt(world, 0);
    fire(canvas, 'mousedown', ev);
    fire(winTarget, 'mouseup', ev);
  };
  const boxDrag = (fromWorld, toWorld) => {
    const a = evAt(fromWorld, 0);
    const b = evAt(toWorld, 0);
    fire(canvas, 'mousedown', a);
    fire(winTarget, 'mousemove', b);
    fire(winTarget, 'mouseup', b);
  };
  const pressEsc = () => fire(winTarget, 'keydown',
    { code: 'Escape', target: undefined, preventDefault() {} });
  const deselects = () => occEntries.filter((e) => e.ev === 'deselect');
  return { ...fns, issued, flashes, occEntries, deselects, st, leftClick, boxDrag, pressEsc };
}

// ------------------------------------------------------------------
// Wiring pins: the helper's shape, and the two paths that
// cannot be sliced whole (portrait, match (re)start).

test('FIX #18 wiring: occDeselect logs {ev:deselect, via, size} only for a non-empty selection', () => {
  const helper = sliceFn(appJs, 'function occDeselect(via) {', '\n}');
  assert.match(helper, /occLog\(\{ ev: 'deselect', via, size: selection\.size \}\)/,
    'the log line carries the event, the path name and the size before clearing');
  assert.match(helper, /selection\.size/, 'the non-empty guard lives in the helper');
  assert.match(appJs, /FIX #18 \(bot-loop occorder21/, 'the fix is marked in app.js');
});

test('FIX #18 wiring: every named path calls occDeselect exactly as specified', () => {
  const count = (via) => (appJs.match(new RegExp(`occDeselect\\('${via}'\\)`, 'g')) || []).length;
  assert.equal(count('esc'), 1, 'one Esc-stack call site');
  assert.equal(count('portrait'), 1, 'one portrait call site');
  assert.equal(count('restart'), 3, 'three match-entry call sites (resetSim / adoptSim / adoptLoadedSim)');
  assert.equal(count('animal'), 1, 'one call site, inside selectAnimal');
  assert.equal(count('building'), 1, 'one call site, inside selectBuilding');
  assert.equal(count('empty-ground'), 2, 'two call sites: plain click and selection box');
  // The helper calls inside selectAnimal / selectBuilding run
  // BEFORE the selection is replaced.
  const animal = sliceFn(appJs, 'function selectAnimal(id) {', '\n}');
  assert.ok(animal.indexOf("occDeselect('animal')") < animal.indexOf('selection = new Set()'),
    'selectAnimal logs before it empties the unit selection');
  const building = sliceFn(appJs, 'function selectBuilding(descOrNull) {', '\n}');
  assert.ok(building.indexOf("occDeselect('building')") < building.indexOf('selection = new Set()'),
    'selectBuilding logs before it empties the unit selection');
});

test('FIX #18 wiring: the portrait click logs via portrait before clearing', () => {
  const region = sliceFn(appJs, '// Portrait click deselects', "\n  helpCloseX.addEventListener");
  const iLog = region.indexOf("occDeselect('portrait')");
  assert.ok(iLog > 0, 'the portrait listener is instrumented');
  assert.ok(iLog < region.indexOf('selectUnits(new Set())'),
    'the portrait log is recorded before the clear, inside the same listener');
});

test('FIX #18 wiring: each match-entry reset logs via restart before clearing', () => {
  for (const [name, start, end] of [
    ['resetSim', 'function resetSim(mode = matchMode) {', '\n/// Adopt a freshly built sim'],
    ['adoptSim', 'function adoptSim(newSim, owner) {', '\nfunction makeNetSession() {'],
    ['adoptLoadedSim', 'function adoptLoadedSim(loaded) {', '\n// --- i18n: static strings'],
  ]) {
    const src = sliceFn(appJs, start, end);
    const iLog = src.indexOf("occDeselect('restart')");
    assert.ok(iLog > 0, `${name} is instrumented`);
    assert.ok(iLog < src.indexOf('selectUnits(new Set())'),
      `${name} logs before its selectUnits(new Set()) clear`);
  }
});

// ------------------------------------------------------------------
// Driven paths: each clearer logs, and behaviour is unchanged
// (selection actually cleared, no orders issued).

test('esc: the Esc stack\'s last step logs via esc and deselects', () => {
  const w = makeWorld();
  w.leftClick([12, 2]);
  assert.deepEqual(w.getSelection(), [4], 'staging: Militia selected through the real handlers');
  w.pressEsc();
  assert.deepEqual(w.deselects().map((e) => [e.via, e.size]), [['esc', 1]],
    'one deselect line: via esc, size 1 (the selection before the clear)');
  assert.deepEqual(w.getSelection(), [], 'Esc still deselects, exactly as before');
  assert.deepEqual(w.issued, [], 'no order was issued');
});

test('empty-ground: a plain left-click on empty ground logs via empty-ground and deselects', () => {
  const w = makeWorld();
  w.leftClick([12, 2]);
  assert.deepEqual(w.getSelection(), [4], 'staging: Militia selected');
  w.leftClick([30, 30]);
  assert.deepEqual(w.deselects().map((e) => [e.via, e.size]), [['empty-ground', 1]],
    'one deselect line: via empty-ground, size 1');
  assert.deepEqual(w.getSelection(), [], 'the click still clears the selection');
  assert.deepEqual(w.issued, [], 'no order was issued');
  assert.deepEqual(w.flashes, [], 'no toast — a plain deselect, as before');
});

test('empty-ground: a selection box over empty ground logs via empty-ground and deselects', () => {
  const w = makeWorld();
  w.leftClick([12, 2]);
  assert.deepEqual(w.getSelection(), [4], 'staging: Militia selected');
  w.boxDrag([20, 20], [40, 40]); // a drag box containing no own unit
  assert.deepEqual(w.deselects().map((e) => [e.via, e.size]), [['empty-ground', 1]],
    'one deselect line: via empty-ground, size 1');
  assert.deepEqual(w.getSelection(), [], 'the empty box still clears the selection');
  assert.deepEqual(w.issued, [], 'no order was issued');
});

test('animal: selectAnimal replacing a unit selection logs via animal', () => {
  const w = makeWorld();
  w.selectUnits(new Set([4]));
  assert.deepEqual(w.deselects(), [], 'selecting units logs no deselect');
  w.selectAnimal(56);
  assert.deepEqual(w.deselects().map((e) => [e.via, e.size]), [['animal', 1]],
    'one deselect line: via animal, size 1');
  assert.deepEqual(w.getSelection(), [], 'the unit selection is replaced, as before');
  assert.equal(w.getSelAnimal(), 56, 'the info selection stands');
  assert.deepEqual(w.issued, [], 'no order was issued');
});

test('building: selectBuilding replacing a unit selection logs via building', () => {
  const w = makeWorld();
  w.selectUnits(new Set([4, 1]));
  w.selectBuilding({ type: 'tc', id: 0, owner: 0 });
  assert.deepEqual(w.deselects().map((e) => [e.via, e.size]), [['building', 2]],
    'one deselect line: via building, size 2 (both selected units counted before the clear)');
  assert.deepEqual(w.getSelection(), [], 'the unit selection is replaced, as before');
  assert.deepEqual(w.getSelBuilding(), { type: 'tc', id: 0, owner: 0 }, 'the building selection stands');
  assert.deepEqual(w.issued, [], 'no order was issued');
});

// ------------------------------------------------------------------
// Behaviour guards: no log where nothing was deselected, and
// the instrument is inert without ?occlog=1.

test('no false positives: selecting, and clearing an empty selection, log nothing', () => {
  const w = makeWorld();
  w.leftClick([30, 30]); // empty-ground click with nothing selected
  assert.deepEqual(w.deselects(), [], 'clearing an already-empty selection is not a deselection event');
  w.pressEsc(); // Esc with nothing selected: the stack's last step is not even reached
  assert.deepEqual(w.deselects(), [], 'Esc with no selection logs nothing');
  w.boxDrag([10, 0], [16, 6]); // a box that DOES contain the Militia
  assert.deepEqual(w.getSelection(), [4], 'the box selects the Militia');
  assert.deepEqual(w.deselects(), [], 'a box that selects logs no deselect');
  w.selectAnimal(56); // (selection [4] -> this one DOES log; reset the slate by re-staging)
  assert.equal(w.deselects().length, 1, 'sanity: the one real replacement above logged exactly once');
});

test('inert without the flag: no ?occlog=1, no entries — and the clear still happens', () => {
  const w = makeWorld(''); // no occlog param
  w.leftClick([12, 2]);
  assert.deepEqual(w.getSelection(), [4], 'staging: Militia selected');
  w.leftClick([30, 30]);
  assert.deepEqual(w.getSelection(), [], 'behaviour unchanged: the empty-ground click still deselects');
  assert.deepEqual(w.occEntries, [], 'occLog recorded nothing without the flag');
});
