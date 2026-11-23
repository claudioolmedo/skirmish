// Right-click ORDER pick at Town Center centre pixels
// (bot-loop orderpick, 2026-11-21; follow-up to run #453's
// post-publish vigia on build 20ae0a0).
//
// THE CAPTURE (run #453 vigia, live, solo vs AI Normal,
// client ?occlog=1): an own Militia (selected) right-clicked
// the hidden enemy's red outline AT THE TC CENTRE. Result
// (vigia's paraphrase of the occLog lines): pick resolved
// NOTHING (picked:null, tFromOutline:false) and orderAt
// issued {t:'garrison_tc', e:69, tc:0} ok:true — the Militia
// garrisoned into the TC instead of attacking. Contrast
// runs #448/#451: right-clicks on the foe's OUTLINE pixels
// resolved pick {picked:<foe>, fromOutline:true} -> attack.
//
// VERDICT OF THIS STAGING (measured below, driven through
// the REAL orderAt + entityAt sliced verbatim from
// web-preview/app.js, in the occorder14/tcsel style): the
// capture does NOT reproduce at any centre pixel while the
// foe is in the state the capture describes — a foe the
// player can see outlined under their own TC. With the foe
// Scout at the TC CENTRE (0,0), fog-visible, with OR without
// an occlusion record that frame, EVERY staged pixel that
// names the foe class — its feet, its body centre, its
// outline tall band, the TC door centre, the TC art centre,
// and the footprint's far south corner — resolves the foe
// in entityAt and issues exactly one attack. The garrison
// precedence in orderAt is therefore CORRECT at centre
// pixels: it fires only when no enemy is named, and at
// centre pixels an enemy in the described state is always
// named (outline / record-rect / body / ground radius /
// FIX #11+#12 TC arena — the arena's centre-disc gate
// resolves a centre foe at ANY inside-footprint click).
// Grid sweeps behind this file (builder scratch, reported
// in the handoff): over the whole TC art bbox, zero
// garrison-on-figure pixels for any foe position inside
// the arena's +-10 membership box, records present or not,
// with own bystander units staged or not.
//
// The capture's signature (pick null + garrison at an
// on-figure pixel) reproduces in this harness ONLY under
// states the capture's own fields do not establish:
//   (a) FOG-HIDDEN foe cell: every foe creator gates on
//       cellVisibleW (body / arena) or on the record (which
//       itself requires a visible cell at draw time), so an
//       on-figure click past the 2.2 ground radius names
//       nothing (pixel C/E/F below); and a click INSIDE the
//       ground radius names the foe at the pick but orderAt's
//       fog gate drops it (afterGate:null) and the garrison
//       branch then fires on the nulled target (pixels A/B/D
//       below). BUT a fog-hidden foe draws NO outline and NO
//       body — there is nothing on screen to click — so this
//       shape requires the vigia's click to have landed on
//       a foe that was not actually visible that frame.
//   (b) Foe OUTSIDE the arena's +-10 membership (e.g. at
//       (0,11), south of the footprint): the recorded-rect
//       corner pixels of its figure whose ground points fall
//       inside the footprint garrison (bound below). That
//       foe stands in FRONT of the TC, not hidden at its
//       centre, and the pixels are rect-corner margin.
// The discriminating evidence for a future capture is the
// full pick line: picked / afterGate / pickedFog separate
// (a)-far (picked:null, pickedFog:false), (a)-near
// (picked:<foe>, afterGate:null, pickedFog:false) and (b)
// (picked:null, pickedFog:true). NO FIX is made here: per
// the builder brief, a scoping handoff is delivered instead
// of an invented fix. These tests pin the staged behaviour
// as regression protection for whichever layer a future
// proven capture indicts.
//
// PRESERVED BY CONSTRUCTION (pinned below): right-click on
// own TC with NO enemy named still garrisons (DE); empty
// ground still moves; the minimap pick keeps its plain
// ground shape (SEL-16); FIX #20/#22 left-click readings
// are untouched (no selection code is sliced differently —
// selectionAt is not modified by this build at all; there
// is no app.js change in this build).
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
const tcAtSrc = sliceFn(appJs, 'function tcAt(wx, wy) {', '\n  function nodeAt');
const ramBodySrc = sliceFn(appJs, 'function ramBodyD2(st, i, wx, wy) {', '\n  function unitAt');
const unitAtSrc = sliceFn(appJs, 'function unitAt(wx, wy) {', '\n  // Left-click selection pick');
const selectionAtSrc = sliceFn(appJs, 'function selectionAt(wx, wy) {', '\n  // Left-click pick for an animal');
const occSrc = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
const entityAtSrc = sliceFn(appJs, 'function entityAt(wx, wy, screenPick = false) {', '\n  // Placed-building hit test');
const orderAtSrc = sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting');
const issueSrc = sliceFn(appJs, 'function issue(cmd) {', '\n// M5: the mode decides');
const occLogSrc = sliceFn(appJs, 'const OCCLOG_ON', '\nconst canvas = document.getElementById');

function makeOccLog(search) {
  const env = { params: new URLSearchParams(search), window: {}, performance: { now: () => 0 } };
  const factory = new Function(...Object.keys(env), `${occLogSrc}\nreturn { occLog, occLogEntries };`);
  const r = factory(...Object.values(env));
  return { occLog: r.occLog, entries: r.occLogEntries };
}

const CS = 4, HALF = 60, GW = 30, GH = 30;
const TALL_RECT = { x0: -1.4, y0: -3.9, w: 3.5, h: 4.05 }; // cavalry tall pose (occorder5)

// Scene: own TC 0 (real entity kind tag 1) at the origin;
// selected own Militia 69 east of the footprint at (14,0)
// (the capture's garrisoning unit); foe Scout 56 at foePos
// (default the TC CENTRE (0,0) — the draining position).
function makeWorld({ foePos = [0, 0], withRecords = true, fogHidesFoe = false, foeAlive = true } = {}) {
  const st = {
    kinds: new Array(70).fill(undefined),
    owners: new Array(70).fill(undefined),
    pos: new Array(140).fill(0),
    hp: new Array(140).fill(0),
    count: 70, garrisoned: [],
  };
  const put = (i, k, o, x, y, h) => {
    st.kinds[i] = k; st.owners[i] = o;
    st.pos[i * 2] = x; st.pos[i * 2 + 1] = y; st.hp[i * 2] = h; st.hp[i * 2 + 1] = h;
  };
  put(0, 1, 0, 0, 0, 2400);
  put(69, 3, 0, 14, 0, 40);
  put(56, 2, 2, foePos[0], foePos[1], foeAlive ? 45 : 0);
  const issued = [];
  const records = [];
  const { occLog, entries } = makeOccLog('');
  const env = {
    camS: 8, camOx: 960, camOy: 540, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    lastOccludedOutlines: records,
    occludedPickAt,
    cellVisibleW: (x, y) => !(fogHidesFoe && x === foePos[0] && y === foePos[1]),
    cellExploredW: () => true,
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => [69],
    selectedAnimal: null,
    selectedBuilding: null,
    localOwner: 0,
    foeOwner: () => 2,
    formationApplies: () => false,
    formationChoice: { tag: () => 0 },
    ackClassForKinds: () => null,
    playAck: () => {},
    playAudioCue: () => {},
    flash: () => {},
    t: (k) => k,
    carryKind: new Set(),
    clickMarkers: { add: () => {} },
    firstGarrisonedRelic: () => null,
    sim: { entity_kinds: () => st.kinds, positions: () => st.pos, tick_count: () => 0 },
    OBST: new Array(GW * GH).fill(0),
    tcHalf: () => 8,
    KIND_RAM: 17, KIND_TC: 1, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
    isOwnUnit: (s, i) => s.owners[i] === 0 && s.kinds[i] !== 1 && s.hp[i * 2] >= 1
      && !(s.garrisoned && s.garrisoned[i]),
    playback: null, net: null, playing: true, localRecorder: null,
    applyLocalOrder: (cmd) => { issued.push(cmd); return true; },
    occLog,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${selectionAtSrc}\n${occSrc}\n${entityAtSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, selectionAt, occludedUnitAt, occludedRectUnitAt, tcAt, worldToScreen, s2w: (sx, sy) => s2w(sx, sy, 8, 960, 540) };`,
  );
  const fns = factory(...Object.values(env));
  if (withRecords && foeAlive) {
    const [tsx, tsy] = fns.worldToScreen(0, 0);
    const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
    const [sx, sy] = fns.worldToScreen(foePos[0], foePos[1]);
    const rect = { x: sx + TALL_RECT.x0 * 8, y: sy + TALL_RECT.y0 * 8, w: TALL_RECT.w * 8, h: TALL_RECT.h * 8 };
    const res = occlusionFor({ rect, depth: foePos[0] + foePos[1], fogVisible: !fogHidesFoe }, [{ rect: tcCover, depth: 8 }]);
    if (res.occluded) records.push({ id: 56, rect, cover: res.cover.map((o) => o.rect), depth: foePos[0] + foePos[1] });
  }
  return { ...fns, issued, records, occEntries: entries, st };
}

// The staged pixels (screen, camS 8, TC centre at (960,540)).
const PIXELS = {
  A: { label: 'foe feet (screen of the foe position)', screen: [960, 540] },
  B: { label: 'foe body centre', screen: [960, 529] },
  C: { label: 'foe outline tall band (rect n cover)', screen: [960, 516] },
  D: { label: 'TC door centre', screen: [960, 548] },
  E: { label: 'TC footprint south corner (away from foe)', screen: [960, 604] },
  F: { label: 'TC art centre', screen: [960, 486] },
  G: { label: 'empty ground SE of the TC', screen: [1100, 620] },
};
function drive(w, key) {
  const [sx, sy] = PIXELS[key].screen;
  const [wx, wy] = w.s2w(sx, sy);
  const ent = w.entityAt(wx, wy, true);
  w.issued.length = 0;
  w.orderAt(wx, wy, false, true);
  return { wx, wy, ent, issued: w.issued.map((c) => c.t) };
}

// ------------------------------------------------------------------
// Wiring pins: the layers this staging exercises are the
// published ones (FIX #11/#12 arena + FIX #15 garrison gate).

test('wiring: entityAt carries the TC arena and orderAt carries the FIX #15 pick gate on the garrison branch', () => {
  assert.equal((entityAtSrc.match(/foeTcArenaAtPoint\(\)/g) || []).length, 4,
    'the arena is consulted at the three own-win gates and at the ground stage (occorder13/14 pin)');
  assert.match(orderAtSrc, /FIX #15 \(bot-loop occorder18/,
    'the garrison branch is pick-gated (FIX #15 is marked in orderAt)');
  assert.match(orderAtSrc, /tcT != null && st\.owners\[tcT\] === localOwner && \(tgt == null \|\| tgt === tcT \|\| targets\.includes\(tgt\)\)/,
    'garrison fires only when the pick named no unit, the TC itself, or a member of the selection (FIX #24, kingpick)');
});

// ------------------------------------------------------------------
// The staging table: foe at the TC centre, fog-visible.

for (const withRecords of [true, false]) {
  test(`staging (records=${withRecords}): a centre foe is named and attacked at every centre/door/footprint pixel`, () => {
    const w = makeWorld({ withRecords });
    if (withRecords) assert.equal(w.records.length, 1, 'staging: the centre foe holds a TC-occlusion record');
    for (const key of ['A', 'B', 'C', 'D', 'E', 'F']) {
      const r = drive(w, key);
      assert.equal(r.ent, 56, `${key} (${PIXELS[key].label}): entityAt names the centre foe`);
      assert.deepEqual(r.issued, ['attack'], `${key}: exactly one attack — no garrison_tc, no move`);
    }
    const g = drive(w, 'G');
    assert.equal(g.ent, null, 'G: empty ground names nothing');
    assert.deepEqual(g.issued, ['move'], 'G: a plain ground move');
  });
}

test('staging: the outline pixel resolves through the record (the #448/#451 shape)', () => {
  const w = makeWorld({ withRecords: true });
  const [sx, sy] = PIXELS.C.screen;
  const [wx, wy] = w.s2w(sx, sy);
  assert.equal(w.occludedUnitAt(wx, wy), 56, 'staging: pixel C is on the foe\'s painted outline (rect n cover)');
  assert.equal(w.entityAt(wx, wy, true), 56, 'the order pick names the foe at its outline pixel');
});

// ------------------------------------------------------------------
// DE preservation: no enemy named -> garrison stands.

test('preservation: with no living foe, door/footprint/art pixels still garrison and empty ground still moves', () => {
  const w = makeWorld({ foeAlive: false, withRecords: false });
  for (const key of ['A', 'B', 'C', 'D', 'E', 'F']) {
    const r = drive(w, key);
    assert.equal(r.ent, null, `${key}: nothing named with no living foe`);
    assert.deepEqual(r.issued, ['garrison_tc'], `${key}: the DE garrison gesture is intact`);
  }
  assert.deepEqual(drive(w, 'G').issued, ['move'], 'G: empty ground still moves');
});

test('preservation: the minimap pick keeps its plain ground shape (SEL-16)', () => {
  const w = makeWorld({ withRecords: true });
  const [sx, sy] = PIXELS.C.screen;
  const [wx, wy] = w.s2w(sx, sy);
  assert.equal(w.entityAt(wx, wy, false), null,
    'with screenPick off, the tall-band pixel names nothing (no record/body/arena stages)');
  const d = drive(w, 'D');
  assert.equal(d.ent, 56, 'canvas pick at the door names the foe (contrast)');
  w.issued.length = 0;
  const [dwx, dwy] = w.s2w(...PIXELS.D.screen);
  w.orderAt(dwx, dwy, false, false);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack'],
    'the door pixel is inside the 2.2 ground radius, so even the minimap-shaped pick names the foe there');
});

// ------------------------------------------------------------------
// Bounds: the only configurations whose signature matches
// the capture (pick null / dropped + garrison on an
// on-figure pixel). Pinned as documented bounds, NOT fixed
// — neither is established by the #453 capture.

test('bound (a): a fog-hidden centre foe names nothing past the ground radius, and the fog gate drops it inside it', () => {
  const w = makeWorld({ withRecords: false, fogHidesFoe: true });
  for (const key of ['C', 'E', 'F']) {
    const r = drive(w, key);
    assert.equal(r.ent, null, `${key}: every foe creator gates on cellVisibleW — nothing is named`);
    assert.deepEqual(r.issued, ['garrison_tc'], `${key}: the nulled pick falls through to the garrison branch`);
  }
  for (const key of ['A', 'B', 'D']) {
    const r = drive(w, key);
    assert.equal(r.ent, 56, `${key}: the ground stage names the foe (it carries no visibility gate)`);
    assert.deepEqual(r.issued, ['garrison_tc'],
      `${key}: orderAt's fog gate drops the named foe (afterGate:null) and the garrison branch fires — the (a)-near shape`);
  }
  // The bound's own limit: a fog-hidden foe draws nothing —
  // no body, no outline record (occlusionFor's fairness gate
  // requires fogVisible) — so there is no visible foe figure
  // to click in this state.
  const w2 = makeWorld({ withRecords: true, fogHidesFoe: true });
  assert.equal(w2.records.length, 0, 'a fog-hidden foe holds no occlusion record — nothing is painted to click');
});

test('bound (b): a foe outside the arena membership (0,11) garrisons only at its rect-corner margin pixels', () => {
  const w = makeWorld({ foePos: [0, 11], withRecords: false });
  // The foe's recorded-rect top-left corner pixel found by
  // the builder's grid sweep: screen (862,554), ground
  // (-4.38, 7.88) — inside the footprint, 5.4+ wu from the
  // foe, above its body box (h = 3.75 > 2.81).
  const [wx, wy] = w.s2w(862, 554);
  assert.equal(w.tcAt(wx, wy), 0, 'staging: the click ground point is inside our TC footprint');
  assert.equal(w.entityAt(wx, wy, true), null,
    'the arena membership (+-10) excludes the foe at (0,11); body/ground/rect all miss at this corner pixel');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['garrison_tc'],
    'the corner pixel garrisons — a foe in FRONT of the TC (not hidden at its centre), at a rect-margin pixel');
  // Its feet still name it:
  const [fx, fy] = w.s2w(...w.worldToScreen(0, 11));
  assert.equal(w.entityAt(fx, fy, true), 56, 'the same foe at its feet is named by the ground pick');
});
