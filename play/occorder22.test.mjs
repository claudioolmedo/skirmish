// Occluded-outline SELECTION divergence, round 22 (bot-loop
// occorder22, 2026-11-20; follow-up to run #450, live client
// ?occlog=1, solo vs AI).
//
// THE CAPTURE (run #450, live): an enemy unit (id 28) hidden
// under the player's Town Center roof drained the TC
// 2400 -> 1723. The diagnostic RIGHT-click attack with a
// Militia WORKED via the outline pick, verbatim:
//   pick {picked:28, afterGate:28, tFromOutline:true}
//   issue {t:'attack', ...} ok:true — the Militia engaged.
// But a LEFT-click select on the same red unit produced NO
// pick entry at all and the selection stayed on the Militia.
//
// ------------------------------------------------------------------
// WHAT "NO PICK ENTRY" MEANS (source fact, not a mechanism):
// the `pick` occLog line lives only in orderAt (the
// right-click path). The left-click selection path
// (selectionAt + the plain-click handler) logs NOTHING on a
// hit or a miss — so a failed left-click is invisible in the
// capture by construction. That observability gap is real
// but it is not the defect: the defect is that the selection
// stayed on the Militia, i.e. selectionAt never named 28.
//
// ------------------------------------------------------------------
// DISCRIMINATION (measured in this harness on the pristine
// build): selectionAt and entityAt are DIFFERENT pick
// functions, and selectionAt is missing three resolutions
// entityAt has — each one a pixel class where the order pick
// names the foe and the selection pick names NOTHING (null),
// so the plain-click handler falls through to its building /
// empty-ground readings and the foe is never info-selected:
//
//   R. THE RECORD-RECT STAGE. entityAt consults
//      occludedRectUnitAt (FIX #5): a unit that HAS an INF-12
//      record is named by a click anywhere on its recorded
//      rect — the measured sprite as drawn, pose included.
//      selectionAt NEVER calls occludedRectUnitAt (occorder8
//      already noted this stage "has no counterpart in
//      selectionAt"; FIX #7 closed the order side only).
//      Staged below: the foe's recorded rect's VISIBLE part
//      (outside rect n cover), at the tall attack pose's
//      height (body height 3.25, above the cached body box's
//      2.81 top — the static body pick misses), ground point
//      4.65 wu from the foe (the ground radius misses):
//      entityAt = 28, selectionAt = null on pristine.
//
//   G. THE GROUND CANDIDATE AS CREATOR. entityAt's ground
//      stage names a non-own unit inside the 2.2 pick radius
//      directly (its best loop includes foes). selectionAt
//      restated that candidate (foeGroundSelAtPoint, FIX
//      #11) ONLY as a shadow: it is consulted when an own
//      winner already exists (body class 2, or the unitAt
//      fallback) — "it never creates a selection from
//      nothing" (FIX #11's own words). Staged below: foe at
//      (1,1) inside the footprint, NO record that frame
//      (record absence is a normal per-frame event —
//      occorder8's cliff), click at its feet 1.34 wu away
//      (body missed with h < 0, no own unit near):
//      entityAt = 28, selectionAt = null on pristine.
//
//   A. THE TC ARENA AS CREATOR. Same shape one stage down:
//      entityAt's ground stage consults foeTcArenaAtPoint
//      when its ground winner is null (FIX #11), so a LOW
//      inside-footprint click past the 2.2 radius still
//      names a centre foe draining the TC. selectionAt's
//      arena restatement (FIX #12/#14) is likewise
//      shadow-only. Staged below: foe at the TC centre
//      (0,0), no record, click LOW at (3.4,3.4) — 4.81 wu
//      from the foe, inside the footprint, foe inside the
//      5.0 centre disc: entityAt = 28, selectionAt = null
//      on pristine.
//
// At the EXACT pixel of run #450's successful right-click
// (an outline hit — tFromOutline:true), selectionAt DOES
// name the foe: its outline stage runs first and returns
// the raw frontmost outline, exactly as entityAt's does
// (control below). The live left-click therefore landed in
// one of the three classes above — on the same red figure,
// a few pixels off the painted outline sliver: on its
// visible rect beside the roof, at its feet, or low on the
// TC art under it. All three are the same defect class:
// the selection pick resolves strictly less than the order
// pick at pixels a player reads as "clicking that unit".
//
// ------------------------------------------------------------------
// THE FIX (FIX #20, client selection pick only): the three
// candidates become CREATORS in selectionAt, consulted only
// after every existing stage has named nothing (Ram body,
// outline, body stage with its shadows, the unitAt fallback
// with its shadows — all byte-identical, in the same order):
// first the record-rect pick (raw frontmost, as in
// entityAt's own rect stage order — rect before ground),
// then the ground candidate, then the TC arena, each with
// the fog discipline its selection restatement already
// carries (a record exists only for a fog-visible unit;
// foeGroundSelAtPoint / foeTcArenaSelAtPoint gate on
// cellVisibleW — this pick has no post-pick fog gate, so
// the gate stays inside the candidates). Every previously
// non-null selection is unchanged BY CONSTRUCTION (the new
// block runs only where selectionAt returned null), and
// occorder8's pinned divergence is preserved: at a pixel
// inside a friendly rect on the foe's BODY, the body stage
// still fires first and names the foe — the rect stage
// never shadows a body winner, own or foe. Raw picks,
// entityAt, orderAt, tcAt and the sim are untouched;
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
const TALL_RECT = { x0: -1.4, y0: -3.9, w: 3.5, h: 4.05 }; // cavalry tall pose (occorder5/17)

function makeHarness(st, records, issued, occEntriesBundle, foePos) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = st.kinds;
  const { occLog, entries } = occEntriesBundle;
  const applyLocalOrder = (cmd) => {
    issued.push({ ...cmd, ok: true });
    return true;
  };
  const env = {
    camS, camOx, camOy, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    lastOccludedOutlines: records,
    occludedPickAt,
    cellVisibleW: (x, y) => !(foePos && x === foePos[0] && y === foePos[1] && makeHarness.fogHidesFoe),
    cellExploredW: () => true,
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => [4],
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
    sim: { entity_kinds: () => kinds, positions: () => st.pos, tick_count: () => 0 },
    OBST: new Array(GW * GH).fill(0),
    tcHalf: () => 8,
    KIND_RAM: 17, KIND_TC: 13, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
    isOwnUnit: (s, i) => s.owners[i] === 0 && s.kinds[i] !== 13 && s.hp[i * 2] >= 1
      && !(s.garrisoned && s.garrisoned[i]),
    playback: null, net: null, playing: true, localRecorder: null,
    applyLocalOrder,
    occLog,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${selectionAtSrc}\n${occSrc}\n${entityAtSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, selectionAt, unitAt, occludedUnitAt, occludedRectUnitAt, tcAt, worldToScreen, spriteBodyD2, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
  );
  const fns = factory(...Object.values(env));
  return { fns, occEntries: entries };
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

// The #450 scene, TC-relative (harness TC at the origin, as
// in every occorder harness): TC 0; own Militia 4 (kind 3,
// the selected unit) far east at (12,2); foe 28 (kind 2,
// owner 2, full HP) at foePos under/behind the TC.
function makeWorld({ foePos = [-9, 0], withRecord = true, fogHidesFoe = false, foeDead = false } = {}) {
  makeHarness.fogHidesFoe = fogHidesFoe;
  const st = blankState(29);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 4, 3, 0, 12, 2, 40);
  put(st, 28, 2, 2, foePos[0], foePos[1], foeDead ? 0 : 45);
  const issued = [];
  const records = [];
  const { fns, occEntries } = makeHarness(st, records, issued, makeOccLog('occlog=1'), fogHidesFoe ? foePos : null);
  let foeRec = null;
  if (withRecord) {
    const [tsx, tsy] = fns.worldToScreen(0, 0);
    const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
    const [sx, sy] = fns.worldToScreen(foePos[0], foePos[1]);
    const rect = { x: sx + TALL_RECT.x0 * 8, y: sy + TALL_RECT.y0 * 8, w: TALL_RECT.w * 8, h: TALL_RECT.h * 8 };
    const depth = foePos[0] + foePos[1];
    const res = occlusionFor({ rect, depth, fogVisible: true }, [{ rect: tcCover, depth: 8 }]);
    if (res.occluded) {
      foeRec = { id: 28, rect, cover: res.cover.map((o) => o.rect), depth };
      records.push(foeRec);
    }
  }
  const pt = (sx, sy) => ({ screen: [sx, sy], world: fns.s2w(sx, sy) });
  return { ...fns, issued, records, occEntries, st, foeRec, pt };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #20 wiring: selectionAt consults the record-rect pick; the handler routing is unchanged', () => {
  assert.match(selectionAtSrc, /FIX #20 \(bot-loop occorder22/,
    'the fix is marked in selectionAt');
  assert.match(selectionAtSrc, /occludedRectUnitAt\(wx, wy\)/,
    'selectionAt now consults the record-rect pick (FIX #5\'s stage, previously order-only)');
  assert.ok(appJs.includes('const v = selectionAt(wx, wy);'),
    'context pin: the plain-click handler still takes its candidate from selectionAt');
  assert.ok(appJs.includes('selectAnimal(v);'),
    'context pin: a non-own selectionAt winner is routed to the information selection (selectAnimal) — a foe named by the new creators becomes the info selection, exactly as an outline-named foe already did');
});

// ------------------------------------------------------------------
// Class R: the record-rect-only pixel (fails on pristine:
// selectionAt returns null while entityAt returns 28).

test('class R post-fix: a click on the foe\'s recorded rect (visible part, tall pose) selects it — selection parity with the order pick', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  assert.ok(w.foeRec, 'staging: the west foe holds an INF-12 record — its outline is painted through the TC art');
  const { world: [wx, wy] } = w.pt(880, 478);
  assert.equal(w.occludedRectUnitAt(wx, wy), 28, 'staging: the point is inside the foe\'s recorded rect');
  assert.equal(w.occludedUnitAt(wx, wy), null, 'staging: the point is OUTSIDE rect n cover — no outline winner (the visible part beside the roof)');
  assert.equal(w.spriteBodyD2(w.st, 28, wx, wy), null,
    'staging: the foe\'s static body box misses — the click sits at body height 3.25, above the cached box top 2.81 (the tall attack pose, FIX #5\'s case)');
  const dFoe = Math.hypot(w.st.pos[28 * 2] - wx, w.st.pos[28 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2, `staging: the foe is ${dFoe.toFixed(2)} wu from the click's ground point — the ground radius misses too`);
  assert.equal(w.entityAt(wx, wy, true), 28,
    'the ORDER pick names the foe at this pixel (record-rect stage) — the run #450 asymmetry in miniature');
  assert.equal(w.selectionAt(wx, wy), 28,
    'post-fix the SELECTION pick names the same unit — on pristine it returned null and the handler fell through to its building/empty-ground readings');
});

test('class R order parity: the same pixel right-clicked attacks (already true on pristine — the asymmetry is selection-only)', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  const { world: [wx, wy] } = w.pt(880, 478);
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg, c.ok]), [['attack', 4, 28, true]],
    'the Militia attacks 28 from this pixel — what run #450\'s right-click did through the outline, this pixel does through the rect');
});

// ------------------------------------------------------------------
// Class G: the ground-only pixel, no record, no own winner
// (fails on pristine: selectionAt returns null).

test('class G post-fix: a click at a record-less foe\'s feet (inside the 2.2 ground radius, no own winner) selects it', () => {
  const w = makeWorld({ foePos: [1, 1], withRecord: false });
  const [wx, wy] = [2.2, 1.6];
  assert.equal(w.records.length, 0, 'staging: no records this frame (record absence is a normal per-frame event — occorder8)');
  assert.equal(w.spriteBodyD2(w.st, 28, wx, wy), null,
    'staging: the foe\'s body box misses — the click is screen-down from its ground point (h < 0)');
  assert.equal(w.unitAt(wx, wy), null, 'staging: no own unit is named at the point — the shadow guards have nothing to shadow');
  const dFoe = Math.hypot(w.st.pos[28 * 2] - wx, w.st.pos[28 * 2 + 1] - wy);
  assert.ok(dFoe < 2.2, `staging: the foe is ${dFoe.toFixed(2)} wu away — inside the ground pick radius entityAt has arbitrated since FIX #2c`);
  assert.equal(w.entityAt(wx, wy, true), 28, 'the ORDER pick names the foe by ground proximity');
  assert.equal(w.selectionAt(wx, wy), 28,
    'post-fix the SELECTION pick creates the same selection — on pristine the ground candidate was shadow-only and this returned null');
});

// ------------------------------------------------------------------
// Class A: the arena-only pixel (fails on pristine).

test('class A post-fix: a LOW inside-footprint click past the ground radius selects the centre foe draining the TC', () => {
  const w = makeWorld({ foePos: [0, 0], withRecord: false });
  const [wx, wy] = [3.4, 3.4];
  assert.equal(w.tcAt(wx, wy), 0, 'staging: the click lies inside our own TC footprint');
  assert.equal(w.spriteBodyD2(w.st, 28, wx, wy), null, 'staging: the foe\'s body box misses (h < 0 below its feet)');
  assert.equal(w.unitAt(wx, wy), null, 'staging: no own winner to shadow');
  const dFoe = Math.hypot(w.st.pos[28 * 2] - wx, w.st.pos[28 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2 && dFoe < 8, `staging: the foe is ${dFoe.toFixed(2)} wu away — past the ground radius, inside the arena's click gate, and at the TC centre (the draining disc)`);
  assert.equal(w.entityAt(wx, wy, true), 28, 'the ORDER pick names the foe by the TC arena (its ground stage consults the arena on a null winner)');
  assert.equal(w.selectionAt(wx, wy), 28,
    'post-fix the SELECTION pick names it too — on pristine the arena was shadow-only and this returned null (the handler then selected the TC itself)');
});

// ------------------------------------------------------------------
// Control: the exact outline pixel of run #450's successful
// right-click — both picks already agree (regression pin,
// passes pre- and post-fix).

test('control: a click ON the painted outline selects and attacks the foe (run #450\'s right-click pixel class)', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  const { world: [wx, wy] } = w.pt(896, 490);
  assert.equal(w.occludedUnitAt(wx, wy), 28, 'staging: the pixel is on the painted outline (rect n cover)');
  assert.equal(w.selectionAt(wx, wy), 28, 'selectionAt names the foe through its outline stage — the left-click shape that worked in runs #444/#450');
  assert.equal(w.entityAt(wx, wy, true), 28, 'entityAt names the foe through the outline stage');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg, c.ok]), [['attack', 4, 28, true]],
    'an outline click attacks, as run #450\'s right-click did (picked 28, fromOutline)');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick && pick.picked === 28 && pick.afterGate === 28 && pick.tFromOutline === true,
    'the occLog pick line matches run #450\'s verbatim shape: picked:28 / afterGate:28 / tFromOutline:true');
});

// ------------------------------------------------------------------
// Preservation: every previously non-null selection is
// unchanged (the creators run only on a null fall-through).

test('preservation: an own unit\'s body click still selects it; empty ground far from any foe still selects nothing', () => {
  const w = makeWorld({ foePos: [-9, 0] });
  assert.equal(w.selectionAt(11, 0.8), 4, 'a click on our Militia\'s drawn body still selects the Militia (body stage, untouched)');
  assert.equal(w.selectionAt(30, 30), null, 'far empty ground names nothing — no creator fires without a foe resolved at the point');
  assert.equal(w.entityAt(30, 30, true), null, 'the order pick agrees at that point');
});

test('preservation (occorder8 doctrine): a foe body under a friendly rect still wins selection by BODY — the rect creator never shadows a body winner', () => {
  // Foe at (-9,0) with a record; our Villager-class own unit
  // is not staged here — instead the foe's own body pixel
  // inside its rect: the body stage (class 1) names it
  // before any creator runs.
  const w = makeWorld({ foePos: [-9, 0] });
  const { world: [wx, wy] } = w.pt(884, 496);
  assert.ok(w.spriteBodyD2(w.st, 28, wx, wy) !== null, 'staging: the foe\'s body box IS hit at this pixel');
  assert.equal(w.selectionAt(wx, wy), 28, 'the body stage names the foe, as before the fix');
});

// ------------------------------------------------------------------
// Bounds: the creators inherit their candidates' gates.

test('bound: a far inside-footprint click past a non-centre foe selects nothing — both distance gates stand', () => {
  const w = makeWorld({ foePos: [-6, -1], withRecord: false });
  const [wx, wy] = [7, 7];
  const dFoe = Math.hypot(w.st.pos[28 * 2] - wx, w.st.pos[28 * 2 + 1] - wy);
  const cFoe = Math.hypot(w.st.pos[28 * 2], w.st.pos[28 * 2 + 1]);
  assert.ok(dFoe >= 8 && cFoe > 5.0, `staging: foe ${dFoe.toFixed(2)} wu from the click, ${cFoe.toFixed(2)} from the centre — outside BOTH arena gates`);
  assert.equal(w.entityAt(wx, wy, true), null, 'the order pick names nothing either');
  assert.equal(w.selectionAt(wx, wy), null, 'the arena creator does not fire — a deliberate TC click at distance still selects the TC via the handler');
});

test('fog discipline: a ground-resolved foe whose cell is not visible is NOT created as a selection', () => {
  const w = makeWorld({ foePos: [1, 1], withRecord: false, fogHidesFoe: true });
  assert.equal(w.selectionAt(2.2, 1.6), null,
    'the ground candidate\'s fog gate (cellVisibleW) stands inside the creator — this pick has no post-pick gate to catch it later');
});

test('bound: a dead foe\'s stale record names nothing in the rect creator (revalidation stands)', () => {
  const w = makeWorld({ foePos: [-9, 0], foeDead: true });
  assert.ok(w.foeRec, 'staging: the stale record is still in the frame\'s record set');
  const { world: [wx, wy] } = w.pt(880, 478);
  assert.equal(w.occludedRectUnitAt(wx, wy), null, 'staging: the raw rect pick revalidates against the live state — the dead foe names nothing');
  assert.equal(w.selectionAt(wx, wy), null, 'the rect creator inherits that revalidation');
});
