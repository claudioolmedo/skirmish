// Occluded-outline ORDER degradation, round 20 (bot-loop
// occorder20, 2026-11-20; follow-up to run #446, the live
// session on build f41c65b / client ?occlog=1, solo vs
// AI·Normal).
//
// THE CAPTURE (run #446, live): an enemy Spearman camped
// under/behind the player's Town Center roof, attacking it
// (TC 2400 → destroyed; the Spearman still at FULL 45/45 HP
// at game 25:56). The player selected their own Militia
// (Aggressive, 40/40) and right-clicked LOW at the Spearman's
// feet/outline under the TC roof. The occlog recorded ONLY
// an "rclick" line for that click (screen [773.0, 451.6],
// world [-89.38, 69.92]) — NO "pick" line and NO "issue" line
// followed, unlike every other right-click in the session
// (gather/build clicks all logged pick + issue). The Militia
// did not move or attack; the selection CLEARED; a toast
// "Select a unit first." appeared; and the camera zoomed out
// sharply at the same moment. A subsequent LEFT-click on the
// same figure DID select the Spearman ("enemy unit.
// Information only"). The same session also saw spontaneous
// sharp camera zoom-outs at other moments (opening the House
// build menu; a plain left-click on empty ground in a
// restarted match).
//
// ------------------------------------------------------------------
// DISCRIMINATION (this harness drives the REAL handler layer
// — the canvas/window mousedown/mouseup/dblclick/wheel
// listeners, smartOrderAt and orderAt sliced verbatim, over
// the REAL selection state — not just orderAt with a stubbed
// selection as occorder14/17/18 did):
//
//   - Branch (a) — the rclick handler early-returns before
//     the pick: EXCLUDED for this capture. The handler's five
//     early returns (placement / repairArm / healArm /
//     orderArm / buildPage) all sit BEFORE the rclick occLog
//     line in the source (pinned below), so a logged rclick
//     proves none of them fired; and the faithful sequence
//     (select Militia by a real left-click through the real
//     handlers, then the real right-click at the foe's feet)
//     produces rclick → pick → issue(attack) on pristine.
//   - Branch (b) — pick runs but returns without logging:
//     EXCLUDED. orderAt's pick occLog is unconditional once
//     the empty-selection return is passed, and the staged
//     scene (foe Spearman INSIDE the footprint at the click
//     point, the most-covered geometry of occorder17) logs
//     picked:56 / afterGate:56 and issues the attack.
//   - Branch (c) — orderAt runs and takes its EMPTY-SELECTION
//     return: PROVEN as the unique producer of the capture
//     signature. That return is the only order path that
//     flashes and logs NOTHING (no pick, no issue — issue()
//     itself logs unconditionally, so the rally path, which
//     does call issue(), is excluded too). Its fallback flash
//     key, flash.selectUnitFirst ("Select a unit first."), is
//     used at exactly ONE site in app.js — this return, when
//     selectedAnimal == null AND selectedBuilding == null.
//     The sweep below drives every selection state through
//     the real handlers at the capture point: own-TC-selected
//     logs a rally issue; info-animal-selected flashes
//     notYourAnimal; foe-building-selected flashes
//     enemyOrders; ONLY empty+null+null reproduces
//     [rclick] + "Select a unit first." + no pick + no issue,
//     verbatim. So at the instant of run #446's click the
//     unit selection was ALREADY empty — the right-click
//     sequence itself clears nothing (test 2: the selection
//     survives it intact). Who cleared it is NOT in the
//     captured click: within the client, the selection is
//     emptied only by the paths enumerated in the handoff
//     (Esc stack's last step, a plain left-click on empty
//     ground, a box over empty ground, the portrait click,
//     selectAnimal/selectBuilding replacing it, a match
//     (re)start) — none of which is part of, or triggered by,
//     a right-click. The live antecedent sits before the
//     excerpt the capture preserved.
//   - The zoom-out: NO click path resets the camera zoom.
//     camS is assigned at exactly five places in app.js —
//     the three match-entry resets (resetSim / adoptSim /
//     adoptLoadedSim, camS = INITIAL_S) and the wheel /
//     pinch handlers. The left-click, right-click, dblclick,
//     selection-clear and minimap-jump paths never touch
//     camS (pinned + driven below: wheel-zoom to ~13, then
//     clear-by-empty-click, dblclick-select and the capture
//     right-click all leave camS untouched). A sharp
//     apparent zoom-out without a camS writer is the
//     fitCanvas family (quality-governor DPR step via
//     applyQuality, fullscreen/resize refit): the backing
//     store changes under a constant camS, which rescales
//     the whole view at once. That is the adaptive-quality
//     system working as designed, not a click defect —
//     reported, not fixed, in this round.
//
// THE FIX (FIX #16, client instrumentation only): orderAt's
// empty-selection return was the ONE order path invisible
// to ?occlog=1 — run #446's capture was undiagnosable from
// its own log precisely because this return logged nothing
// (the loop spent a full builder round eliminating the
// other branches by hand). The return now records an
// 'order-skip' occLog line (selection state included)
// before flashing, in the occorder5 style. Behaviour is
// unchanged: DE's reading stands — no selection, no order,
// the hint flashes. No pick/order logic moves.
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
// The REAL event handlers, verbatim registrations.
const lDownSrc = sliceFn(appJs, "canvas.addEventListener('mousedown', (ev) => {", "\n  window.addEventListener('mousemove', (ev) => {");
const lMoveSrc = sliceFn(appJs, "window.addEventListener('mousemove', (ev) => {", "\n  window.addEventListener('mouseup', (ev) => {");
const lUpSrc = sliceFn(appJs, "window.addEventListener('mouseup', (ev) => {", "\n  canvas.addEventListener('dblclick', (ev) => {");
const dblSrc = sliceFn(appJs, "canvas.addEventListener('dblclick', (ev) => {", '\n  // Zoom toward the cursor with the mouse wheel.');
const wheelSrc = sliceFn(appJs, "canvas.addEventListener('wheel', (ev) => {", '\n  // Pinch zoom (touch');
const rBlockSrc = sliceIncl(appJs, '  let drag = null;', "canvas.addEventListener('contextmenu', (e) => e.preventDefault());");

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
const FOOT_RECT = { x0: -0.9, y0: -2.8, w: 1.8, h: 2.95 }; // spearman/militia foot pose
const VIL_RECT = { x0: -0.9, y0: -2.6, w: 1.8, h: 2.75 };

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
  const applyLocalOrder = (cmd) => {
    let ok = true;
    if (cmd.t === 'garrison_tc' || cmd.t === 'garrison_u') {
      ok = st.kinds[cmd.e] === 0 || st.kinds[cmd.e] === 3;
    }
    issued.push({ ...cmd, ok });
    return ok;
  };
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
    // Handler-layer state (the real declarations live at module
    // scope in app.js; here they are factory bindings the sliced
    // handlers assign exactly as the real ones do).
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
  };
  const factory = new Function(
    ...Object.keys(env),
    `${selStateSrc}\n${isOwnUnitSrc}\n${ownUnitIdsSrc}\n${selectedUnitsSrc}\n` +
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n${evClientSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${selectionAtSrc}\n${occSrc}\n${entityAtSrc}\n${smartOrderSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `${lDownSrc}\n${lMoveSrc}\n${lUpSrc}\n${dblSrc}\n${wheelSrc}\n${rBlockSrc}\n` +
    `return { orderAt, smartOrderAt, entityAt, selectionAt, occludedUnitAt, occludedRectUnitAt, tcAt, worldToScreen,
      selectUnits, selectBuilding, selectAnimal, selectedUnits,
      getSelection: () => [...selection], getSelBuilding: () => selectedBuilding, getSelAnimal: () => selectedAnimal,
      cam: () => ({ s: camS, ox: camOx, oy: camOy }),
      s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
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

// The #446 scene, TC-relative (as in every occorder harness;
// live world coords are this frame translated to the
// generated TC cell — the capture's [-89.38, 69.92] is the
// click's ground point inside the footprint, staged here as
// (2.8, 2.8) relative to the TC at the origin).
// TC 0; villagers 1..3; own Militia 4 (kind 3, 40/40) at
// (12,2), outside the footprint; foe Spearman 56 (kind 9,
// owner 2, 45/45) at (1,1) — INSIDE the footprint, camped
// under the roof at the click point; foe TC 24 far away for
// the selection-state sweep.
function makeWorld() {
  const st = blankState(57);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 1, 0, 0, 5, 0, 50);
  put(st, 2, 0, 0, -5, 0, 50);
  put(st, 3, 0, 0, 0, 5, 50);
  put(st, 4, 3, 0, 12, 2, 40);
  put(st, 24, 13, 2, 60, 60, 2400);
  put(st, 56, 9, 2, 1, 1, 45);
  const issued = [];
  const flashes = [];
  const records = [];
  const { fns, occEntries, canvas, winTarget } =
    makeHarness(st, records, issued, flashes, makeOccLog('occlog=1'));
  const [tsx, tsy] = fns.worldToScreen(0, 0);
  const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
  let foeRec = null;
  for (const [id, box] of [[1, VIL_RECT], [2, VIL_RECT], [3, VIL_RECT], [56, FOOT_RECT]]) {
    const [sx, sy] = fns.worldToScreen(st.pos[id * 2], st.pos[id * 2 + 1]);
    const rect = { x: sx + box.x0 * 8, y: sy + box.y0 * 8, w: box.w * 8, h: box.h * 8 };
    const depth = st.pos[id * 2] + st.pos[id * 2 + 1];
    const res = occlusionFor({ rect, depth, fogVisible: true }, [{ rect: tcCover, depth: 8 }]);
    if (res.occluded) {
      const r = { id, rect, cover: res.cover.map((o) => o.rect), depth };
      records.push(r);
      if (id === 56) foeRec = r;
    }
  }
  const evAt = (world, button) => {
    const [sx, sy] = fns.worldToScreen(world[0], world[1]);
    return { button, clientX: sx, clientY: sy, shiftKey: false, preventDefault() {} };
  };
  const leftClick = (world) => {
    const ev = evAt(world, 0);
    fire(canvas, 'mousedown', ev);
    fire(winTarget, 'mouseup', ev);
  };
  const rightClick = (world) => {
    const ev = evAt(world, 2);
    fire(canvas, 'mousedown', ev);
    fire(winTarget, 'mouseup', ev);
  };
  return { ...fns, issued, flashes, records, occEntries, st, foeRec, leftClick, rightClick, canvas, winTarget, evAt };
}

const FEET = [2.8, 2.8]; // low at the Spearman's feet: foe (1,1) + (1.8,1.8)

// ------------------------------------------------------------------
// Wiring pins.

test('wiring: a logged rclick excludes every handler early return (branch (a) pin)', () => {
  const iLog = rBlockSrc.indexOf("ev: 'rclick'");
  assert.ok(iLog > 0, 'the rclick occLog line is in the right-click handler');
  for (const gate of ['if (placement)', 'if (repairArm)', 'if (healArm)', 'if (orderArm)', 'if (buildPage)']) {
    const i = rBlockSrc.indexOf(gate);
    assert.ok(i >= 0 && i < iLog, `${gate} is checked BEFORE the rclick log — a logged rclick proves it did not fire`);
  }
  assert.ok(rBlockSrc.indexOf('smartOrderAt(') > iLog, 'smartOrderAt is called after the rclick log');
});

test('wiring: flash.selectUnitFirst has exactly one call site — orderAt\'s empty-selection return', () => {
  assert.equal((appJs.match(/flash\.selectUnitFirst/g) || []).length, 1,
    'the toast text of run #446 names this return and no other code');
  const iFlash = orderAtSrc.indexOf("t('flash.selectUnitFirst')");
  assert.ok(iFlash > 0 && iFlash < orderAtSrc.indexOf("ev: 'pick'"),
    'the empty-selection flash+return precedes the pick occLog — the return logs no pick by construction (pre-FIX #16)');
});

test('FIX #16 wiring: the empty-selection return records an order-skip line before flashing', () => {
  const iSkip = orderAtSrc.indexOf("ev: 'order-skip'");
  assert.ok(iSkip > 0, 'the empty-selection return is instrumented (FIX #16)');
  assert.ok(iSkip < orderAtSrc.indexOf("t('flash.selectUnitFirst')"),
    'the order-skip line is recorded before the flash, inside the same return');
  assert.match(orderAtSrc, /FIX #16 \(bot-loop occorder20/, 'the fix is marked in orderAt');
});

// ------------------------------------------------------------------
// The faithful sequence: branches (a) and (b) excluded.

test('capture scene, selection present: the real right-click at the foe\'s feet attacks (branches (a)/(b) excluded)', () => {
  const w = makeWorld();
  assert.ok(w.foeRec, 'staging: the Spearman holds an INF-12 record — its outline is painted through the TC art');
  assert.equal(w.tcAt(FEET[0], FEET[1]), 0, 'staging: the click\'s ground point lies inside our own TC footprint (the capture point)');
  assert.equal(w.occludedUnitAt(FEET[0], FEET[1]), null, 'staging: no outline winner LOW at the feet');
  assert.equal(w.occludedRectUnitAt(FEET[0], FEET[1]), null, 'staging: no record-rect winner at the feet either');
  // Select the Militia through the REAL left-click handlers.
  w.leftClick([12, 2]);
  assert.deepEqual(w.getSelection(), [4], 'the left-click selected our Militia (the capture\'s stated selection)');
  const camBefore = w.cam();
  // The diagnostic right-click, through the REAL handlers.
  w.rightClick(FEET);
  // SUPERSESSION (selpicklog / FIX #21): the staging
  // left-click above now emits its own selpick line first —
  // the sequence gains 'selpick' ahead of the right-click's
  // shape, which is otherwise unchanged.
  assert.deepEqual(w.occEntries.map((e) => e.ev), ['selpick', 'rclick', 'pick', 'issue'],
    'selpick (the staging left-click, FIX #21) → rclick → pick → issue, the healthy shape every other right-click in the session showed');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick.picked === 56 && pick.afterGate === 56 && pick.selection.length === 1 && pick.selection[0] === 4,
    'the pick names the Spearman (56) for the selected Militia (4) — the pick layer is healthy at this geometry');
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg, c.ok]), [['attack', 4, 56, true]],
    'exactly one command: {t:\'attack\', e:4, tg:56}');
  assert.deepEqual(w.getSelection(), [4], 'the right-click sequence itself clears NOTHING — the selection survives it intact');
  assert.deepEqual(w.cam(), camBefore, 'the right-click does not touch the camera');
});

// ------------------------------------------------------------------
// Branch (c): the capture signature, reproduced verbatim —
// and proven unique to the empty+null+null state.

test('capture signature reproduced: an empty selection at the same click yields rclick alone + "Select a unit first."', () => {
  const w = makeWorld();
  assert.deepEqual(w.getSelection(), [], 'staging: no unit selection');
  assert.equal(w.getSelBuilding(), null, 'staging: no building selection');
  assert.equal(w.getSelAnimal(), null, 'staging: no info selection');
  w.rightClick(FEET);
  const evs = w.occEntries.map((e) => e.ev);
  assert.ok(!evs.includes('pick') && !evs.includes('issue'),
    'NO pick line and NO issue line follow the rclick — run #446 verbatim');
  assert.deepEqual(w.flashes, ['flash.selectUnitFirst'],
    'the one toast: "Select a unit first." (flash.selectUnitFirst)');
  assert.deepEqual(w.issued, [], 'no command was issued — the Militia neither moves nor attacks');
  const skip = w.occEntries.find((e) => e.ev === 'order-skip');
  assert.ok(skip && skip.reason === 'empty-selection' && skip.selAnimal === null && skip.selBuilding === null,
    'FIX #16: the order-skip line names the empty+null+null state that produced the signature');
});

test('uniqueness sweep: only empty+null+null produces the capture toast — every other selection state differs', () => {
  // Own TC selected: smartOrderAt takes the rally path (issue IS logged).
  {
    const w = makeWorld();
    w.selectBuilding({ type: 'tc', id: 0, owner: 0 });
    w.rightClick(FEET);
    assert.deepEqual(w.issued.map((c) => c.t), ['rally_tc'], 'own TC selected → a rally order, logged as an issue');
    assert.ok(w.occEntries.some((e) => e.ev === 'issue'), 'the rally path logs its issue — excluded by the capture\'s missing issue line');
    assert.deepEqual(w.flashes, [], 'no selectUnitFirst toast');
  }
  // Info-selected animal (the foe itself, as the later left-click left it):
  // orderAt flashes notYourAnimal — a different toast, still no pick/issue.
  {
    const w = makeWorld();
    w.selectAnimal(56);
    w.rightClick(FEET);
    assert.deepEqual(w.flashes, ['flash.notYourAnimal'], 'info-animal selected → "not your animal", not the capture toast');
    assert.ok(!w.occEntries.some((e) => e.ev === 'pick' || e.ev === 'issue'), 'no pick/issue — but the toast discriminates');
  }
  // Foe building selected for information: enemyOrders toast.
  {
    const w = makeWorld();
    w.selectBuilding({ type: 'tc', id: 24, owner: 2 });
    w.rightClick(FEET);
    assert.deepEqual(w.flashes, ['flash.enemyOrders'], 'foe building selected → the enemy-orders toast, not the capture toast');
    assert.ok(!w.occEntries.some((e) => e.ev === 'pick' || e.ev === 'issue'), 'no pick/issue — the toast discriminates again');
  }
});

test('control: the subsequent LEFT-click on the same figure info-selects the Spearman (the live control)', () => {
  const w = makeWorld();
  assert.ok(w.foeRec, 'staging: the Spearman holds a record — its outline is painted through the TC art');
  w.rightClick(FEET); // the failed right-click first (empty selection)
  // Then the left-click ON THE FIGURE (the painted outline,
  // rect ∩ cover) — the live control's gesture. (A left-click
  // at the FEET point itself selects the TC instead:
  // selectionAt's arena only shadows an own winner and never
  // creates a selection from nothing — occorder14's doctrine.)
  const r = w.foeRec.rect;
  const c = w.foeRec.cover[0];
  const sx = Math.max(r.x, c.x) + 2;
  const sy = Math.max(r.y, c.y) + 2;
  assert.ok(sx < Math.min(r.x + r.w, c.x + c.w) && sy < Math.min(r.y + r.h, c.y + c.h),
    'staging: the click point lies on the painted outline (rect ∩ cover)');
  const [wx, wy] = w.s2w(sx, sy);
  assert.equal(w.occludedUnitAt(wx, wy), 56, 'staging: the outline stage names the Spearman at that pixel');
  w.leftClick([wx, wy]);
  assert.equal(w.getSelAnimal(), 56, 'the left-click names the Spearman as an information selection — "enemy unit. Information only"');
  assert.deepEqual(w.getSelection(), [], 'an information selection holds no unit selection — the state the failed click already implied');
});

// ------------------------------------------------------------------
// Zoom staging: no click path resets camS.

test('zoom: wheel zooms; selection clear, dblclick and the capture right-click all leave camS untouched', () => {
  const w = makeWorld();
  // Wheel-zoom IN toward the canvas centre (the one mouse
  // gesture that owns camS besides pinch).
  const wheelEv = { deltaY: -400, clientX: 960, clientY: 540, shiftKey: false, preventDefault() {} };
  fire(w.canvas, 'wheel', wheelEv);
  const zoomed = w.cam().s;
  assert.ok(zoomed > 12 && zoomed < 14, `staging: the wheel took camS 8 → ${zoomed.toFixed(2)}`);
  // Select the Militia, then clear by a plain left-click on
  // empty ground (the restarted-match zoom-out sighting's
  // gesture).
  w.leftClick([12, 2]);
  assert.deepEqual(w.getSelection(), [4], 'staging: Militia selected');
  w.leftClick([30, 30]);
  assert.deepEqual(w.getSelection(), [], 'a plain left-click on empty ground clears the selection');
  assert.equal(w.cam().s, zoomed, 'the selection-clearing click does NOT reset the zoom');
  // Double-click select-all-of-kind.
  const dblEv = w.evAt([12, 2], 0);
  fire(w.canvas, 'dblclick', dblEv);
  assert.deepEqual(w.getSelection(), [4], 'dblclick selects the Militia (all of its kind on screen)');
  assert.equal(w.cam().s, zoomed, 'dblclick does NOT reset the zoom');
  // The capture right-click at the zoomed state.
  w.rightClick(FEET);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack'], 'the right-click still attacks');
  assert.equal(w.cam().s, zoomed, 'the right-click does NOT reset the zoom');
});

test('zoom wiring: camS is written only by the match-entry resets and the wheel/pinch handlers', () => {
  assert.equal((appJs.match(/^\s+camS = INITIAL_S;/gm) || []).length, 3,
    'three match-entry resets (resetSim / adoptSim / adoptLoadedSim) — a (re)started match reframes by design');
  for (const [name, src] of [['left mouseup', lUpSrc], ['right-click block', rBlockSrc], ['dblclick', dblSrc]]) {
    assert.ok(!/camS\s*=/.test(src), `${name} never assigns camS`);
  }
  assert.match(wheelSrc, /camS = ns/, 'the wheel handler is the click-adjacent camS writer');
});
