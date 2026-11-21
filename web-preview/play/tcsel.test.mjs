// Town Center ART selection, bot-loop tcsel (run #452,
// 2026-11-20; live proof #452 on the published FIX #20 build
// 43a16e5, solo vs AI, client ?occlog=1).
//
// THE CAPTURE (proof #452, live): two faces of one defect in
// the client pick path.
//   (1) While an enemy Scout stood parked under the player's
//       TC porch draining it, the TC read as UNSELECTABLE —
//       door and low tower-wall clicks resolved to the enemy
//       Scout instead of the building.
//   (2) With NO enemy anywhere near, a left-click on the TC's
//       ROOF art — far from any foe — produced "No selection":
//       the click deselected via the empty-ground fall-through
//       instead of selecting the TC whose art was clicked.
//
// ------------------------------------------------------------------
// STAGING (measured in this harness on pristine 43a16e5; the
// scenes drive the REAL plain-click handlers, sliced verbatim
// in the occorder20/21 style — canvas mousedown + window
// mouseup, the real selection state, selectionAt / tcAt /
// entityAt sliced whole; TC at the origin, camS 8, centre
// screen (960,540); pixels are SCREEN pixels on the TC art,
// converted by the harness's own s2w exactly as eventWorld
// converts a real click):
//
//   pixel (name, screen, ground world)
//     P1 door centre        (960,548)  ( 1.00,  1.00)
//     P2 tower-wall low R   (1004,524) ( 0.75, -4.75)
//     P3 tower top L        (916,496)  (-8.25, -2.75)
//     P4 roof               (960,468)  (-9.00, -9.00)
//     P5 roof top           (960,452)  (-11.00,-11.00)
//     P6 front corner gnd   (960,588)  ( 6.00,  6.00)
//
//   Scene A — TC ALONE (own Militia far east at (12,2)):
//     P1  selectionAt=null entityAt=null tcAt=0    -> TC (tcAt)
//     P2  selectionAt=null entityAt=null tcAt=0    -> TC (tcAt)
//     P3  selectionAt=null entityAt=null tcAt=null -> DESELECT
//     P4  selectionAt=null entityAt=null tcAt=null -> DESELECT
//     P5  selectionAt=null entityAt=null tcAt=null -> DESELECT
//     P6  selectionAt=null entityAt=null tcAt=0    -> TC (tcAt)
//
//   Scene B — TC + enemy Scout 28 AT THE CENTRE (0,0), the
//   documented draining position (occorder13/14/22 class A),
//   no INF-12 record this frame:
//     P1  selectionAt=28 (ground creator, d=1.41)  -> info foe
//     P2  selectionAt=28 (arena creator, d=4.81)   -> info foe
//     P3  selectionAt=null entityAt=null tcAt=null -> DESELECT
//     P4  selectionAt=null entityAt=null tcAt=null -> DESELECT
//     P5  selectionAt=null entityAt=null tcAt=null -> DESELECT
//     P6  selectionAt=28 (arena creator, centre    -> info foe
//                        disc: foe within 5 of centre)
//
// THE DEFECT (face 2, proven): the plain-click handler's
// building fallback is tcAt ALONE, and tcAt tests the click's
// GROUND point against the footprint box (pos ± tcHalf() on
// both axes). The TC's art rises straight up the screen from
// the footprint, so a click on the art above the ground plane
// converts to a ground point displaced north-west of the
// centre — screen-up is the (-d,-d) world direction, the
// tree-canopy / Ram-body family — up to ~11.5 wu out for the
// roof top, past the ±8 footprint edge on one or both axes
// (P3 misses by 0.25 on one axis; P4/P5 by 1 and 3). Those
// pixels name no unit (selectionAt null — staged above) and
// no building, so the handler falls through to its
// empty-ground deselect. selectionAt / the handler never
// reach ANY building reading for TC footprint pixels above
// the ground plane. In DE the building is the large object:
// clicking any visible part of your TC selects it.
//
// FACE 1, discriminated: the door / low tower-wall / front
// pixels that resolved to the Scout (P1/P2/P6 in scene B) do
// so through FIX #20's CREATORS inside selectionAt — the
// ground creator at the foe's feet (P1) and the arena creator
// on inside-footprint clicks past the ground radius (P2/P6).
// Those pixels genuinely name the hidden foe (the order pick
// agrees at every one: entityAt=28), and occorder22 pinned
// that reading as intended. They are PRESERVED here, by the
// task's own bound: this fix runs only where selectionAt has
// named nothing. What face 1 loses is its absoluteness: at
// the art pixels that name NO foe (P3/P4/P5 — tower top and
// roof, in scene B exactly as in scene A) the TC is
// selectable again, foe or no foe.
//
// ------------------------------------------------------------------
// THE FIX (FIX #22, client selection fallback only): a new
// tcArtAt(wx, wy) beside tcAt — the click decomposed against
// each TC's centre in screen space (ix = dxw - dyw,
// iy = (dxw + dyw) / 2, the w2s axes) and tested against the
// art's content bands read off drawTC's sprite contract
// (drawn width 17.92, bottom anchored +4.4) and the art's
// measured alpha content: main body |ix| <= 8.0 for
// iy in [-5.9, 1.75]; upperworks |ix| <= 4.5 for
// iy in [-11.5, 1.75]. The plain-click handler consults it
// ONLY where tcAt named nothing and every unit pick above
// named nothing, under the same owner/fog gate as tcAt.
// Every previously non-null reading is unchanged BY
// CONSTRUCTION: ground clicks keep tcAt, unit pixels keep
// selectionAt (FIX #20's creators included), and tcAt's
// other callers (orders / repair / garrison) are untouched.
// Raw picks, entityAt, orderAt and the sim are untouched;
// golden hash untouched by construction.
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
  };
  const factory = new Function(
    ...Object.keys(env),
    `${selStateSrc}\n${isOwnUnitSrc}\n${ownUnitIdsSrc}\n${selectedUnitsSrc}\n` +
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n${evClientSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${selectionAtSrc}\n${occSrc}\n${entityAtSrc}\n${smartOrderSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `${lDownSrc}\n${lMoveSrc}\n${lUpSrc}\n` +
    `return { selectUnits, selectBuilding, selectAnimal,
      getSelection: () => [...selection], getSelBuilding: () => selectedBuilding, getSelAnimal: () => selectedAnimal,
      selectionAt, entityAt, tcAt, tcArtAt: typeof tcArtAt === 'function' ? tcArtAt : null,
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

// Scene A: TC 0 alone (+ own Militia 4 far east). Scene B adds
// the enemy Scout 28 at the TC centre (0,0), no record.
function makeWorld(withFoe) {
  const st = blankState(29);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 4, 3, 0, 12, 2, 40);
  if (withFoe) put(st, 28, 2, 2, 0, 0, 45);
  const issued = [];
  const flashes = [];
  const records = [];
  const { fns, occEntries, canvas, winTarget } =
    makeHarness(st, records, issued, flashes, makeOccLog('occlog=1'));
  const leftClickScreen = (sx, sy) => {
    const ev = { button: 0, clientX: sx, clientY: sy, shiftKey: false, preventDefault() {} };
    fire(canvas, 'mousedown', ev);
    fire(winTarget, 'mouseup', ev);
  };
  return { ...fns, issued, flashes, occEntries, st, leftClickScreen };
}

const PIXELS = {
  door: [960, 548],
  towerWall: [1004, 524],
  towerTop: [916, 496],
  roof: [960, 468],
  roofTop: [960, 452],
  frontCorner: [960, 588],
};

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #22 wiring: tcArtAt exists beside tcAt and the handler consults it only on a tcAt miss, after the unit picks', () => {
  assert.match(appJs, /FIX #22 \(bot-loop tcsel/,
    'the fix is marked in app.js');
  const art = sliceFn(appJs, 'function tcArtAt(wx, wy) {', '\n  function nodeAt');
  assert.match(art, /iy < -11\.5/, 'the art test carries the measured upperworks top');
  assert.match(art, /Math\.abs\(ix\) <= 8\.0/, 'the art test carries the measured main-body half width');
  const body = sliceFn(appJs, '// Plain click.', "canvas.addEventListener('dblclick'");
  assert.ok(body.indexOf('const v = selectionAt(wx, wy);') >= 0
    && body.indexOf('const v = selectionAt(wx, wy);') < body.indexOf('tcArtAt(wx, wy)'),
    'the art fallback runs after selectionAt — unit pixels keep their pick');
  assert.ok(body.includes('const tcArt = tc === null ? tcArtAt(wx, wy) : null;'),
    'the art fallback is consulted only where tcAt named nothing — ground clicks keep their reading');
});

// ------------------------------------------------------------------
// Pick-level staging (the per-pixel table, asserted): on
// pristine the art pixels name NOTHING at any stage.

test('staging, scene A (TC alone): door/tower-wall/front name the TC by tcAt; tower-top/roof pixels name nothing at any stage', () => {
  const w = makeWorld(false);
  const at = (p) => w.s2w(p[0], p[1]);
  for (const key of ['door', 'towerWall', 'frontCorner']) {
    const [wx, wy] = at(PIXELS[key]);
    assert.equal(w.selectionAt(wx, wy), null, `${key}: no unit stage fires`);
    assert.equal(w.tcAt(wx, wy), 0, `${key}: the ground point lies inside the footprint — tcAt names the TC`);
  }
  for (const key of ['towerTop', 'roof', 'roofTop']) {
    const [wx, wy] = at(PIXELS[key]);
    assert.equal(w.selectionAt(wx, wy), null, `${key}: no unit stage fires (nothing to preserve, nothing to steal)`);
    assert.equal(w.entityAt(wx, wy, true), null, `${key}: the order pick agrees — nothing is named at this pixel`);
    assert.equal(w.tcAt(wx, wy), null, `${key}: the ground point lies OUTSIDE the footprint box — the pristine handler's only building reading misses`);
    assert.equal(w.tcArtAt && w.tcArtAt(wx, wy), 0, `${key}: post-fix the art test names the TC — on pristine tcArtAt does not exist`);
  }
});

test('staging, scene B (centre foe): the FIX #20 creator pixels name the foe in both picks; the art pixels name nothing', () => {
  const w = makeWorld(true);
  const at = (p) => w.s2w(p[0], p[1]);
  for (const key of ['door', 'towerWall', 'frontCorner']) {
    const [wx, wy] = at(PIXELS[key]);
    assert.equal(w.selectionAt(wx, wy), 28, `${key}: FIX #20's creators name the foe (ground at the door, arena at the wall/front)`);
    assert.equal(w.entityAt(wx, wy, true), 28, `${key}: the order pick agrees`);
  }
  for (const key of ['towerTop', 'roof', 'roofTop']) {
    const [wx, wy] = at(PIXELS[key]);
    assert.equal(w.selectionAt(wx, wy), null, `${key}: no creator fires — the click's ground point is outside the footprint, so the arena cannot run, and the foe is past every radius`);
    assert.equal(w.tcAt(wx, wy), null, `${key}: tcAt misses, as in scene A`);
    assert.equal(w.tcArtAt && w.tcArtAt(wx, wy), 0, `${key}: post-fix the art test names the TC even with the foe under the porch`);
  }
});

// ------------------------------------------------------------------
// Driven handler outcomes, scene A: the regression tests
// (towerTop/roof/roofTop fail on pristine — the click
// deselects via the empty-ground fall-through).

test('scene A driven: door, tower-wall and front-corner clicks select the TC (controls — pass pre/post)', () => {
  for (const key of ['door', 'towerWall', 'frontCorner']) {
    const w = makeWorld(false);
    w.leftClickScreen(...PIXELS[key]);
    assert.deepEqual(w.getSelBuilding(), { type: 'tc', id: 0, owner: 0 }, `${key}: the TC is selected`);
    assert.equal(w.getSelAnimal(), null, `${key}: no info selection`);
    assert.deepEqual(w.issued, [], `${key}: no order issued`);
  }
});

test('scene A driven: tower-top, roof and roof-top clicks select the TC (proof #452 face 2 — fail on pristine)', () => {
  for (const key of ['towerTop', 'roof', 'roofTop']) {
    const w = makeWorld(false);
    w.selectUnits(new Set([4])); // a standing selection, so the pristine deselect is observable
    w.leftClickScreen(...PIXELS[key]);
    assert.deepEqual(w.getSelBuilding(), { type: 'tc', id: 0, owner: 0 },
      `${key}: the click on the TC's art selects the TC — on pristine it cleared the selection ("No selection")`);
    assert.deepEqual(w.getSelection(), [], `${key}: the building selection replaces the unit selection, as for any TC click`);
    assert.deepEqual(w.issued, [], `${key}: no order issued`);
  }
});

// ------------------------------------------------------------------
// Driven handler outcomes, scene B: FIX #20 preservation and
// the art-pixel recovery.

test('scene B driven: door, tower-wall and front-corner clicks still info-select the hidden foe (FIX #20 preserved)', () => {
  for (const key of ['door', 'towerWall', 'frontCorner']) {
    const w = makeWorld(true);
    w.leftClickScreen(...PIXELS[key]);
    assert.equal(w.getSelAnimal(), 28, `${key}: the foe named by the creators is the information selection, exactly as on the pristine FIX #20 build`);
    assert.equal(w.getSelBuilding(), null, `${key}: the TC is NOT selected at a foe-named pixel — the art fallback never shadows a unit pick`);
    assert.deepEqual(w.issued, [], `${key}: no order issued`);
  }
});

test('scene B driven: roof and tower-top clicks select the TC even with the foe under the porch (the TC is selectable again)', () => {
  for (const key of ['towerTop', 'roof', 'roofTop']) {
    const w = makeWorld(true);
    w.leftClickScreen(...PIXELS[key]);
    assert.deepEqual(w.getSelBuilding(), { type: 'tc', id: 0, owner: 0 },
      `${key}: the art pixel names no foe, so the building fallback reaches the TC — on pristine this deselected`);
    assert.equal(w.getSelAnimal(), null, `${key}: the foe is not info-selected by an art pixel that does not name it`);
    assert.deepEqual(w.issued, [], `${key}: no order issued`);
  }
});

// ------------------------------------------------------------------
// Bounds: the art test claims nothing outside the art.

test('bounds: clicks above the art top, beside the art, and on far empty ground still deselect', () => {
  const w = makeWorld(false);
  w.selectUnits(new Set([4]));
  w.leftClickScreen(960, 436); // iy = -13: above the art's top (-11.5)
  assert.equal(w.getSelBuilding(), null, 'above the art: no TC selection');
  assert.deepEqual(w.getSelection(), [], 'above the art: the click still clears (empty sky/ground)');
  const w2 = makeWorld(false);
  w2.selectUnits(new Set([4]));
  w2.leftClickScreen(1056, 468); // ix = +12, iy = -9: beside the art's main body, above the footprint's right corner
  assert.equal(w2.getSelBuilding(), null, 'beside the art: no TC selection');
  assert.deepEqual(w2.getSelection(), [], 'beside the art: the click still clears');
  const w3 = makeWorld(false);
  w3.leftClickScreen(...(() => { const [sx, sy] = w3.worldToScreen(30, 30); return [sx, sy]; })());
  assert.equal(w3.getSelBuilding(), null, 'far empty ground: no TC selection');
});

test('bounds at the pick level: tcArtAt does not fire for a non-TC point, and the Militia body click is untouched', () => {
  const w = makeWorld(false);
  assert.equal(w.tcArtAt && w.tcArtAt(30, 30), null, 'far ground names no TC by art');
  assert.equal(w.tcArtAt && w.tcArtAt(-13, -13), null, 'above the art top names nothing');
  w.leftClickScreen(...(() => { const [sx, sy] = w.worldToScreen(11, 0.8); return [sx, sy]; })());
  assert.deepEqual(w.getSelection(), [4], 'a click on our Militia\'s drawn body still selects the Militia (selectionAt, untouched)');
  assert.equal(w.getSelBuilding(), null, 'the art fallback is not reached when a unit is named');
});
