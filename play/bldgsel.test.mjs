// Placed-building ART selection, bot-loop bldgsel (2026-11-21;
// the residual named in the tcsel handoff, run #452: "Placed
// buildings have the same ground-plane shape in principle
// (buildingAt is footprint-only and their art also rises
// above it — cf. the old 'Barracks roof click cleared'
// sightings in early proofs). … no placed-building pixel was
// staged or proven here, so nothing was changed there.").
//
// ------------------------------------------------------------------
// STAGING (measured in this harness on pristine aa9cad7 — the
// #453 publish candidate, origin/main 602cfd4 + FIX #22; the
// scenes drive the REAL plain-click handlers, sliced verbatim
// in the tcsel style — canvas mousedown + window mouseup, the
// real selection state, selectionAt / buildingAt sliced
// whole; building at the origin, camS 8, centre screen
// (960,540); pixels are SCREEN pixels on the building's art,
// converted by the harness's own s2w exactly as eventWorld
// converts a real click; sprite alpha read off the shipped
// PNGs — every art pixel below is alpha 255 with a solid 5x5
// neighbourhood in the actual sprite, every "transparent"
// pixel is alpha 0):
//
//   BARRACKS (kind 2, 3x3 cells, footprint half 6 wu;
//   drawBarracks contract: art width 13.5 wu, base +2.6,
//   barracks.png 243x253 -> blit ix [-6.75,6.75],
//   iy [-11.46,2.6]; per-row alpha silhouette is SOLID and
//   widens downward from the tower tip ix [2.81,4.03] at
//   iy -11.0 to [-5.81,5.75] at iy -6.12):
//     R1 roof centre   (960,468) ground (-9.00,-9.00) OUT
//     R2 roof mid      (960,484) ground (-7.00,-7.00) OUT
//     R3 tower tip R   (984,456) ground (-9.00,-12.00) OUT
//     R4 roof left     (936,484) ground (-8.50,-5.50) OUT
//     R5 roof right    (984,484) ground (-5.50,-8.50) OUT
//     R6 footprint ctl (960,516) ground (-3.00,-3.00) IN
//     -> pristine: R1..R5 selectionAt=null entityAt=null
//        buildingAt=null -> DESELECT; R6 -> building (buildingAt)
//
//   HOUSE (kind 0, 2x2 cells, footprint half 4 wu; drawHouse
//   contract: HOUSE_W 8.6, HOUSE_DY 2.0, house.png 237x273
//   -> blit ix [-4.3,4.3], iy [-7.91,2.0]; per-row alpha
//   silhouette SOLID, widening from the chimney ix
//   [1.76,2.34] at iy -7.62 to [-3.86,4.01] at iy -3.99):
//     H1 roof centre   (960,492) ground (-6.00,-6.00) OUT
//     H2 roof mid      (960,500) ground (-5.00,-5.00) OUT
//     H3 chimney       (976,480) ground (-6.50,-8.50) OUT
//     H4 roof right    (984,500) ground (-3.50,-6.50) OUT
//     H5 footprint ctl (960,524) ground (-2.00,-2.00) IN
//     -> pristine: H1..H4 name nothing -> DESELECT;
//        H5 -> building (buildingAt)
//
// THE DEFECT (proven by the staging above): the plain-click
// handler's placed-building reading is buildingAt ALONE, and
// buildingAt tests the click's GROUND point against the
// footprint box (pos ± size*CS/2 on both axes). The buildings'
// art rises straight up the screen from the footprint, so a
// click on the art above the ground plane converts to a
// ground point displaced north-west of the centre (screen-up
// is the (-d,-d) world direction — the tcArtAt / canopy
// family), past the footprint edge on one or both axes (R1
// misses by 3 on both; H1 by 2 on both). Those pixels name no
// unit and no building, so the handler falls through to its
// empty-ground deselect — the "Barracks roof click cleared"
// sighting. In DE the building is the large object: clicking
// any visible part of it selects it.
//
// ------------------------------------------------------------------
// THE FIX (FIX #23, client selection fallback only): a new
// buildingArtAt(wx, wy) beside buildingAt — the click
// decomposed against each placed building's centre in SCREEN
// space (ix = dxw - dyw, iy = (dxw + dyw) / 2, the w2s axes)
// and tested against per-kind art bands stepped UNDER the
// sprites' measured per-row alpha silhouettes (each band's ix
// interval fits every sprite row in its iy interval; the
// rows are solid, so a claimed pixel is opaque art — the
// transparent margins and the silhouettes' diagonal notches
// claim nothing: R9/H8/H9 below stay deselects). Only the
// two staged kinds carry bands (House 0, Barracks 2); every
// other kind keeps its pristine reading, unstaged here. The
// plain-click handler consults it ONLY as buildingAt's
// fallback (bGround !== null ? bGround : buildingArtAt(...),
// logged 'building-art' like tcArtAt's 'tc-art'), in the
// branch reached only after every unit pick has named
// nothing and the TC readings have named nothing, under
// buildingAt's exact owner/fog gate. Every previously
// non-null reading is unchanged BY CONSTRUCTION: ground
// clicks keep buildingAt, unit pixels keep selectionAt,
// TC pixels keep tcAt/tcArtAt (FIX #22), and buildingAt's
// other callers (orders, repair) are untouched. Raw picks,
// entityAt, orderAt and the sim are untouched; golden hash
// untouched by construction.
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
// buildingAt's slice ends at the FIX #23 block when present,
// else at the tcAt comment (pristine) — either way it carries
// buildingAt alone, never a duplicate of the art block.
const buildingAtSrc = (() => {
  const a = appJs.indexOf('function buildingAt(wx, wy) {');
  assert.ok(a >= 0, 'buildingAt found in app.js');
  const bArt = appJs.indexOf('\n  // Placed-building ART hit test', a);
  const bTc = appJs.indexOf('\n  // Town Center hit test', a);
  const b = bArt > a && (bTc < 0 || bArt < bTc) ? bArt : bTc;
  assert.ok(b > a, 'buildingAt slice end found');
  return appJs.slice(a, b);
})();
// The art fallback is absent on pristine — slice it softly so
// the staging tests can run (and fail) there.
const bldgArtSrc = (() => {
  const a = appJs.indexOf('const BUILDING_ART_BANDS = {');
  if (a < 0) return '';
  const b = appJs.indexOf('\n  // Town Center hit test', a);
  assert.ok(b > a, 'buildingArtAt block ends before the tcAt comment');
  return appJs.slice(a, b);
})();
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

function makeHarness(st, buildings, records, issued, flashes, occEntriesBundle) {
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
    buildingList: () => buildings,
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
  };
  const factory = new Function(
    ...Object.keys(env),
    `${selStateSrc}\n${isOwnUnitSrc}\n${ownUnitIdsSrc}\n${selectedUnitsSrc}\n` +
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n${evClientSrc}\n` +
    `${buildingAtSrc}\n${bldgArtSrc}\n${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${selectionAtSrc}\n${occSrc}\n${entityAtSrc}\n${smartOrderSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `${lDownSrc}\n${lMoveSrc}\n${lUpSrc}\n` +
    `return { selectUnits, selectBuilding, selectAnimal,
      getSelection: () => [...selection], getSelBuilding: () => selectedBuilding, getSelAnimal: () => selectedAnimal,
      selectionAt, entityAt, buildingAt, buildingArtAt: typeof buildingArtAt === 'function' ? buildingArtAt : null,
      worldToScreen, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
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

// One placed building at the origin (+ own Militia 4 far
// south-east at (12,2), as in tcsel). kind 2 = Barracks
// (3x3, size 3, id 101); kind 0 = House (2x2, size 2, id 102).
function makeWorld(kind) {
  const st = blankState(8);
  put(st, 4, 3, 0, 12, 2, 40);
  const b = kind === 2
    ? { id: 101, kind: 2, x: 0, y: 0, progress: 1, complete: true, cellX: 59, cellY: 59, w: 3, h: 3, size: 3, queue: 0, trainFrac: 0, owner: 0, hpFrac: 1 }
    : { id: 102, kind: 0, x: 0, y: 0, progress: 1, complete: true, cellX: 59, cellY: 59, w: 2, h: 2, size: 2, queue: 0, trainFrac: 0, owner: 0, hpFrac: 1 };
  const issued = [];
  const flashes = [];
  const records = [];
  const { fns, occEntries, canvas, winTarget } =
    makeHarness(st, [b], records, issued, flashes, makeOccLog('occlog=1'));
  const leftClickScreen = (sx, sy) => {
    const ev = { button: 0, clientX: sx, clientY: sy, shiftKey: false, preventDefault() {} };
    fire(canvas, 'mousedown', ev);
    fire(winTarget, 'mouseup', ev);
  };
  return { ...fns, issued, flashes, occEntries, st, building: b, leftClickScreen };
}

const RAX = { // Barracks pixels (screen)
  roofCentre: [960, 468],
  roofMid: [960, 484],
  towerTip: [984, 456],
  roofLeft: [936, 484],
  roofRight: [984, 484],
  footCtrl: [960, 516],
  above: [960, 444],
  beside: [1024, 468],
  centreTopClear: [960, 460], // blit interior, alpha 0 (the silhouette's tip is the right-hand tower)
};
const HOUSE = { // House pixels (screen)
  roofCentre: [960, 492],
  roofMid: [960, 500],
  chimney: [976, 480],
  roofRight: [984, 500],
  footCtrl: [960, 524],
  above: [960, 472],
  beside: [1000, 492],
  roofLeftClear: [936, 500], // blit interior, alpha 0 (left roof slope notch)
  centreTopClear: [960, 486], // blit interior, alpha 0 (left of the chimney wedge)
};

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #23 wiring: buildingArtAt exists beside buildingAt and the handler consults it only as buildingAt\'s fallback, after the unit picks', () => {
  assert.match(appJs, /FIX #23 \(bot-loop bldgsel/,
    'the fix is marked in app.js');
  const art = sliceFn(appJs, 'function buildingArtAt(wx, wy) {', '\n  // Town Center hit test');
  assert.match(art, /BUILDING_ART_BANDS\[b\.kind\]/, 'the art test is banded per building kind');
  const bands = sliceFn(appJs, 'const BUILDING_ART_BANDS = {', '\n  function buildingArtAt');
  assert.match(bands, /-11\.0, -10\.3, 2\.9, 4\.0/, 'the Barracks tower-tip band carries the measured silhouette step');
  assert.match(bands, /-7\.6, -7\.0, 1\.8, 2\.3/, 'the House chimney band carries the measured silhouette step');
  const body = sliceFn(appJs, '// Plain click.', "canvas.addEventListener('dblclick'");
  assert.ok(body.indexOf('const v = selectionAt(wx, wy);') >= 0
    && body.indexOf('const v = selectionAt(wx, wy);') < body.indexOf('buildingArtAt(wx, wy)'),
    'the art fallback runs after selectionAt — unit pixels keep their pick');
  assert.ok(body.includes('const b = bGround !== null ? bGround : buildingArtAt(wx, wy);'),
    'the art fallback is consulted only where buildingAt named nothing — ground clicks keep their reading');
  assert.ok(body.includes("bGround !== null ? 'building' : 'building-art'"),
    'the selpick log keeps the art pick distinguishable from the ground pick, like tc/tc-art');
});

// ------------------------------------------------------------------
// Pick-level staging (the per-pixel tables, asserted): on
// pristine the art pixels name NOTHING at any stage.

test('staging, Barracks: footprint control names the building by buildingAt; roof/tower pixels name nothing at any stage', () => {
  const w = makeWorld(2);
  const at = (p) => w.s2w(p[0], p[1]);
  {
    const [wx, wy] = at(RAX.footCtrl);
    assert.equal(w.selectionAt(wx, wy), null, 'footCtrl: no unit stage fires');
    assert.equal(w.buildingAt(wx, wy)?.id, 101, 'footCtrl: the ground point lies inside the footprint — buildingAt names the Barracks');
  }
  for (const key of ['roofCentre', 'roofMid', 'towerTip', 'roofLeft', 'roofRight']) {
    const [wx, wy] = at(RAX[key]);
    assert.equal(w.selectionAt(wx, wy), null, `${key}: no unit stage fires (nothing to preserve, nothing to steal)`);
    assert.equal(w.entityAt(wx, wy, true), null, `${key}: the order pick agrees — nothing is named at this pixel`);
    assert.equal(w.buildingAt(wx, wy), null, `${key}: the ground point lies OUTSIDE the footprint box — the pristine handler's only placed-building reading misses`);
    assert.equal(w.buildingArtAt && w.buildingArtAt(wx, wy)?.id, 101, `${key}: post-fix the art test names the Barracks — on pristine buildingArtAt does not exist`);
  }
});

test('staging, House: footprint control names the building by buildingAt; roof/chimney pixels name nothing at any stage', () => {
  const w = makeWorld(0);
  const at = (p) => w.s2w(p[0], p[1]);
  {
    const [wx, wy] = at(HOUSE.footCtrl);
    assert.equal(w.selectionAt(wx, wy), null, 'footCtrl: no unit stage fires');
    assert.equal(w.buildingAt(wx, wy)?.id, 102, 'footCtrl: the ground point lies inside the footprint — buildingAt names the House');
  }
  for (const key of ['roofCentre', 'roofMid', 'chimney', 'roofRight']) {
    const [wx, wy] = at(HOUSE[key]);
    assert.equal(w.selectionAt(wx, wy), null, `${key}: no unit stage fires`);
    assert.equal(w.entityAt(wx, wy, true), null, `${key}: the order pick agrees — nothing is named at this pixel`);
    assert.equal(w.buildingAt(wx, wy), null, `${key}: the ground point lies OUTSIDE the footprint box — buildingAt misses`);
    assert.equal(w.buildingArtAt && w.buildingArtAt(wx, wy)?.id, 102, `${key}: post-fix the art test names the House — on pristine buildingArtAt does not exist`);
  }
});

// ------------------------------------------------------------------
// Driven handler outcomes: the regression tests (art pixels
// fail on pristine — the click deselects via the empty-ground
// fall-through).

test('Barracks driven: roof and tower-tip clicks select the Barracks (fail on pristine)', () => {
  for (const key of ['roofCentre', 'roofMid', 'towerTip', 'roofLeft', 'roofRight']) {
    const w = makeWorld(2);
    w.selectUnits(new Set([4])); // a standing selection, so the pristine deselect is observable
    w.leftClickScreen(...RAX[key]);
    assert.deepEqual(w.getSelBuilding(), { type: 'building', id: 101, kind: 2, owner: 0 },
      `${key}: the click on the Barracks' art selects the Barracks — on pristine it cleared the selection`);
    assert.deepEqual(w.getSelection(), [], `${key}: the building selection replaces the unit selection, as for any building click`);
    assert.deepEqual(w.issued, [], `${key}: no order issued`);
  }
});

test('House driven: roof and chimney clicks select the House (fail on pristine)', () => {
  for (const key of ['roofCentre', 'roofMid', 'chimney', 'roofRight']) {
    const w = makeWorld(0);
    w.selectUnits(new Set([4]));
    w.leftClickScreen(...HOUSE[key]);
    assert.deepEqual(w.getSelBuilding(), { type: 'building', id: 102, kind: 0, owner: 0 },
      `${key}: the click on the House's art selects the House — on pristine it cleared the selection`);
    assert.deepEqual(w.getSelection(), [], `${key}: the building selection replaces the unit selection`);
    assert.deepEqual(w.issued, [], `${key}: no order issued`);
  }
});

test('driven controls: footprint clicks select the building in both scenes (pass pre/post)', () => {
  const r = makeWorld(2);
  r.leftClickScreen(...RAX.footCtrl);
  assert.deepEqual(r.getSelBuilding(), { type: 'building', id: 101, kind: 2, owner: 0 }, 'Barracks footprint click selects via buildingAt, as before');
  const h = makeWorld(0);
  h.leftClickScreen(...HOUSE.footCtrl);
  assert.deepEqual(h.getSelBuilding(), { type: 'building', id: 102, kind: 0, owner: 0 }, 'House footprint click selects via buildingAt, as before');
});

// ------------------------------------------------------------------
// Bounds: the art test claims nothing outside the measured
// silhouettes — not above the art, not beside it, and not in
// the blit rect's transparent notches.

test('bounds driven: clicks above the art, beside the art, and on transparent blit-interior pixels still deselect', () => {
  for (const key of ['above', 'beside', 'centreTopClear']) {
    const w = makeWorld(2);
    w.selectUnits(new Set([4]));
    w.leftClickScreen(...RAX[key]);
    assert.equal(w.getSelBuilding(), null, `Barracks ${key}: no building selection`);
    assert.deepEqual(w.getSelection(), [], `Barracks ${key}: the click still clears (empty sky/ground)`);
  }
  for (const key of ['above', 'beside', 'roofLeftClear', 'centreTopClear']) {
    const w = makeWorld(0);
    w.selectUnits(new Set([4]));
    w.leftClickScreen(...HOUSE[key]);
    assert.equal(w.getSelBuilding(), null, `House ${key}: no building selection`);
    assert.deepEqual(w.getSelection(), [], `House ${key}: the click still clears`);
  }
});

test('bounds at the pick level: buildingArtAt does not fire for far ground or an unstaged kind, and the Militia body click is untouched', () => {
  const w = makeWorld(2);
  assert.equal(w.buildingArtAt && w.buildingArtAt(30, 30), null, 'far ground names no building by art');
  assert.equal(w.buildingArtAt && w.buildingArtAt(-12, -12), null, 'above the Barracks art top names nothing');
  const h = makeWorld(0);
  assert.equal(h.buildingArtAt && h.buildingArtAt(-8.5, -8.5), null, 'above the House art top names nothing');
  w.leftClickScreen(...(() => { const [sx, sy] = w.worldToScreen(11, 0.8); return [sx, sy]; })());
  assert.deepEqual(w.getSelection(), [4], 'a click on our Militia\'s drawn body still selects the Militia (selectionAt, untouched)');
  assert.equal(w.getSelBuilding(), null, 'the art fallback is not reached when a unit is named');
});
