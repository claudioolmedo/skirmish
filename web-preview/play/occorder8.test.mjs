// Occluded-outline ORDER degradation, round 8 (bot-loop
// occorder8, 2026-11-19; follow-up to run #431's addendum —
// the LIVE PROOF of FIX #6 on the published build f71bb9d).
//
// CAPTURE A (run #431 diagnostic, pre-FIX#6): the foe's record
// 56 WAS in the frame's record set, hit at the click pixel
// behind our own frontmost Scout 4 — FIX #6 (foe preference in
// entityAt's record picks) closed exactly that layer and its
// harness passes (occorder7).
//
// CAPTURE B (run #431 proof session, POST-FIX#6 build): at
// 03:28 a LEFT-click on the red outline under the TC selected
// the enemy Scout Cavalry (45/45). At ~04:32 a RIGHT-click on
// the red outline produced verbatim (live map offset aside):
//   pick {selection:[4], picked:1, afterGate:1,
//     tFromOutline:true, outline:1, outlineRect:1, records:4,
//     recordIds:[2,4,1,3], pickedKind:0, pickedOwner:0,
//     pickedFog:true}
//   issue {garrison_tc, e:4, tc:0, ok:false} -> {move, e:4, ...}
// picked:1 is a friendly VILLAGER. The foe's record is ABSENT
// from recordIds — every record hit was a friendly starting
// unit. No engagement followed; the enemy later left on its
// own (TC HP froze at 2292, outline gone by 06:47).
//
// THE DISCRIMINATED MECHANISM (this harness, pristine base):
//  1. At capture B's pixel and record set, the selection pick
//     and the order pick AGREE: selectionAt === entityAt === 1.
//     There is no left/right geometry divergence AT THAT PIXEL
//     (both consult occludedUnitAt first and it names the
//     Villager). The 03:28 left-click success was a different
//     frame — one where the foe held a record (capture A's
//     frame class) or no friendly outline overlapped.
//     (A second, real divergence class DOES exist one stage
//     down — entityAt's record-RECT stage has no counterpart
//     in selectionAt, so a pixel inside a friendly rect but
//     outside every outline, on the foe's body, selected the
//     foe while the order named the friendly unit. Pinned
//     below; FIX #7 closes it at the same place.)
//  2. The foe's record can be absent while the foe stands in
//     exactly the same place: a record exists only while
//     occlusionFor's covered fraction of the unit's
//     alpha-tight rect stays >= OCCLUDE_MIN_FRACTION (0.15)
//     against an occluder drawn strictly in front. At the TC
//     art's east edge that fraction falls off a cliff within
//     ~1 wu of the attacking scout's own repositioning (and
//     its animation frames change the alpha-tight rect every
//     frame) — the sweep below measures 0.39 -> 0.16 -> 0
//     across the TC's front-depth line. Record absence is a
//     normal per-frame event at this geometry, not an anomaly.
//  3. The foe was nevertheless RESOLVED at capture B's point
//     by the client's own stable geometry: the click pixel
//     lies inside the enemy's sprite-body box
//     (spriteBodyD2(56) = 0.47, a hit), and with the records
//     emptied BOTH picks name 56 through the body stage. What
//     kept entityAt from reaching that stage is the own record
//     winner's short-circuit (`if (occ !== null) return occ;`)
//     combined with FIX #6 searching only foe RECORDS. It is
//     not capture-B-as-(c): the foe did not leave — it kept
//     draining the TC after the click and left minutes later;
//     its body was at the click point throughout.
//
// THE FIX (FIX #7, entityAt only): when a record stage's
// winner is our own unit (same Ram/Transport host exemptions
// as FIX #2c/#6), the foe preference now also considers a
// non-own unit resolved by its drawn BODY at the point
// (foeBodyAtPoint — spriteBodyD2 over non-own units, the body
// stage's own fog rule, closest body centre wins), at both
// the outline stage and the record-rect stage. No record, no
// body — no preference: the garrison reading survives
// whenever no enemy is resolved at the point by either
// geometry. Raw picks, selectionAt and orderAt are untouched.
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

const CS = 4, HALF = 60, GW = 30, GH = 30;
const TALL_RECT = { x0: -1.4, y0: -3.9, w: 3.5, h: 4.05 }; // cavalry tall pose (occorder5)
const VIL_RECT = { x0: -0.9, y0: -2.6, w: 1.8, h: 2.75 };   // villager box (occorder5)

// World builder, three stagings:
//  - 'B' (capture B): TC 0 at the origin; villagers 2/3 behind
//    it; villager 1 at (5.5,-2.5), behind the TC near the foe,
//    its record hit at the click pixel; own Scout 4 at (3,-4),
//    its record present but its rect missing the pixel; the
//    enemy Scout 56 at the east porch (7,-1) with NO record —
//    capture B's defining fact. Records pushed in capture B's
//    order [2,4,1,3]. Click pixel: screen (1024,548) — inside
//    villager 1's rect ∩ cover AND inside the foe's body box.
//  - 'A' (capture A, occorder7's staging): the foe DOES hold
//    a record, hit at the pixel behind our frontmost Scout 4.
//  - 'R' (record-rect divergence): villager 1 at (8,-1), its
//    recorded rect extending east past the TC art's rect edge;
//    the click pixel (1036,556) is inside that rect but
//    outside every rect ∩ cover, and on the foe's body.
function makeWorld(mode, { selKind = 2, fogHidesFoe = false } = {}) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, 0, 0, 2400);
  if (mode === 'A') {
    put(1, 0, 0, -3, -3, 50);
    put(2, 0, 0, -5, 0, 50);
    put(3, 0, 0, 0, -5, 50);
    put(4, selKind, 0, 7.5, -0.5, 45);
  } else if (mode === 'R') {
    put(1, 0, 0, 8, -1, 50);
    put(2, 0, 0, -5, 0, 50);
    put(3, 0, 0, 0, -5, 50);
    put(4, selKind, 0, 3, -4, 45);
  } else { // 'B'
    put(1, 0, 0, 5.5, -2.5, 50);
    put(2, 0, 0, -5, 0, 50);
    put(3, 0, 0, 0, -5, 50);
    put(4, selKind, 0, 3, -4, selKind === 17 ? 150 : 45);
  }
  put(56, 2, 2, 7, -1, mode === 'G' ? 0 : 45); // 'G': the foe is dead — nothing to resolve
  const count = 57;
  const st = { kinds, owners, pos, hp, count, garrisoned: [] };
  const issued = [];
  const records = [];
  const { occLog, entries } = makeOccLog('occlog=1');
  const applyLocalOrder = (cmd) => {
    let ok = true;
    if (cmd.t === 'garrison_tc' || cmd.t === 'garrison_u') {
      ok = kinds[cmd.e] === 0 || kinds[cmd.e] === 3; // foot kinds only
    }
    if (ok) issued.push(cmd);
    return ok;
  };
  const env = {
    camS, camOx, camOy, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    lastOccludedOutlines: records,
    occludedPickAt,
    cellVisibleW: fogHidesFoe
      ? (x, y) => !(x === 7 && y === -1)
      : () => true,
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
    sim: { entity_kinds: () => kinds, positions: () => pos, tick_count: () => 0 },
    OBST: new Array(GW * GH).fill(0),
    tcHalf: () => 8,
    KIND_RAM: 17, KIND_TC: 13, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
    isOwnUnit: (s, i) => s.owners[i] === 0 && s.hp[i * 2] >= 1,
    playback: null, net: null, playing: true, localRecorder: null,
    applyLocalOrder,
    occLog,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${selectionAtSrc}\n${occSrc}\n${entityAtSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, selectionAt, occludedUnitAt, occludedRectUnitAt, tcAt, worldToScreen, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy), spriteBodyD2 };`,
  );
  const fns = factory(...Object.values(env));
  const [tsx, tsy] = fns.worldToScreen(0, 0);
  const cover = { x: tsx - 8.96 * camS, y: tsy - 13.52 * camS, w: 17.92 * camS, h: 17.92 * camS };
  const addRecord = (id, box) => {
    const [sx, sy] = fns.worldToScreen(pos[id * 2], pos[id * 2 + 1]);
    const rect = { x: sx + box.x0 * camS, y: sy + box.y0 * camS, w: box.w * camS, h: box.h * camS };
    const depth = pos[id * 2] + pos[id * 2 + 1];
    const res = occlusionFor({ rect, depth, fogVisible: true }, [{ rect: cover, depth: 8 }]);
    assert.ok(res.occluded, `staging: unit ${id} is occluded by the TC art (a record exists)`);
    return { id, rect, cover: res.cover.map((o) => o.rect), depth };
  };
  let capturePixel;
  if (mode === 'A') {
    records.push(addRecord(2, VIL_RECT));
    records.push(addRecord(1, VIL_RECT));
    records.push(addRecord(56, TALL_RECT));
    records.push(addRecord(4, TALL_RECT));
    records.push(addRecord(3, VIL_RECT));
    const [esx, esy] = fns.worldToScreen(7, -1);
    capturePixel = [esx + 4, esy - 8];
  } else if (mode === 'R') {
    records.push(addRecord(2, VIL_RECT));
    records.push(addRecord(4, TALL_RECT));
    records.push(addRecord(1, VIL_RECT));
    records.push(addRecord(3, VIL_RECT));
    capturePixel = [1036, 556];
  } else {
    records.push(addRecord(2, VIL_RECT));
    records.push(addRecord(4, mode === 'B' && selKind === 17 ? VIL_RECT : TALL_RECT));
    records.push(addRecord(1, VIL_RECT));
    records.push(addRecord(3, VIL_RECT));
    capturePixel = [1024, 548];
  }
  const captureWorld = fns.s2w(...capturePixel);
  return { ...fns, issued, records, occEntries: entries, capturePixel, captureWorld, st, cover };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #7 wiring: the body extension lives in entityAt record stages only', () => {
  assert.match(entityAtSrc, /FIX #7 \(bot-loop occorder8/,
    'the fix is marked in entityAt');
  assert.match(entityAtSrc, /const foeBodyAtPoint = \(\) =>/,
    'the foe-body candidate is defined in entityAt');
  assert.equal((entityAtSrc.match(/foeBodyAtPoint\(\)/g) || []).length, 2,
    'both record stages consult it (outline + record-rect)');
  assert.match(entityAtSrc, /if \(occ !== null\) return occ;/,
    'the outline return line is preserved (earlier wiring pins index it)');
  assert.match(entityAtSrc, /if \(occRect !== null\) return occRect;/,
    'the record-rect return line is preserved');
  assert.ok(!/FIX #7|foeBodyAtPoint/.test(occSrc),
    'the raw outline/record-rect picks carry no foe preference (selection parity)');
  assert.ok(!/foeBodyAtPoint/.test(selectionAtSrc),
    'the left-click selection pick is untouched');
});

// ------------------------------------------------------------------
// Capture B staging: the raw layer matches the live panel
// (passes before and after the fix — the layer did not move).

test('capture B staging: raw picks and records match the live panel', () => {
  const w = makeWorld('B');
  const [wx, wy] = w.captureWorld;
  assert.deepEqual(w.records.map((r) => r.id), [2, 4, 1, 3],
    'the frame\'s records are capture B\'s recordIds, in order — the foe holds none');
  assert.equal(w.occludedUnitAt(wx, wy), 1,
    'the raw outline pick names our Villager (the capture\'s outline:1)');
  assert.equal(w.occludedRectUnitAt(wx, wy), 1,
    'the raw record-rect pick names it too (outlineRect:1)');
  assert.equal(w.tcAt(wx, wy), 0,
    'the click\'s ground point lies inside our own TC footprint (tc:0 in the capture)');
});

// ------------------------------------------------------------------
// The discrimination (measured on the pristine base; these
// layer facts are asserted so a future change must move them
// consciously).

test('discrimination: at capture B\'s pixel the two picks agree — and the foe is body-resolved there', () => {
  const w = makeWorld('B');
  const [wx, wy] = w.captureWorld;
  assert.equal(w.selectionAt(wx, wy), 1,
    'the LEFT pick names the Villager too — no selection/order divergence at this pixel; ' +
    'the 03:28 left-click success was a different frame (capture A\'s class)');
  const bd2 = w.spriteBodyD2(w.st, 56, wx, wy);
  assert.ok(bd2 !== null && bd2 < 0.5,
    `the foe IS resolved at the point by its drawn body (spriteBodyD2(56) = ${bd2}) — ` +
    'entityAt simply never reached the body stage behind the own record winner');
  // With the records emptied, both picks reach the body stage
  // and name the foe at the very same pixel.
  const saved = w.records.splice(0, w.records.length);
  assert.equal(w.selectionAt(wx, wy), 56, 'records emptied: the selection pick names the foe by body');
  assert.equal(w.entityAt(wx, wy, true), 56, 'records emptied: the order pick names the foe by body');
  w.records.push(...saved);
});

test('discrimination: record existence falls off a cliff at the TC art edge (occlusionFor sweep)', () => {
  const w = makeWorld('B');
  const foeAt = (fx, shiftU = 0) => {
    const [sx, sy] = w.worldToScreen(fx, -1);
    const rect = {
      x: sx + (TALL_RECT.x0 + shiftU) * 8, y: sy + TALL_RECT.y0 * 8,
      w: TALL_RECT.w * 8, h: TALL_RECT.h * 8,
    };
    return occlusionFor({ rect, depth: fx - 1, fogVisible: true }, [{ rect: w.cover, depth: 8 }]);
  };
  const f85 = foeAt(8.5);
  assert.ok(f85.occluded && f85.fraction > 0.2,
    `deeper behind the art the foe is recorded (fraction ${f85.fraction.toFixed(3)})`);
  const f88 = foeAt(8.8);
  assert.ok(f88.occluded && f88.fraction >= 0.15 && f88.fraction < 0.2,
    `at the edge the record barely survives (fraction ${f88.fraction.toFixed(3)})`);
  const f88pose = foeAt(8.8, 0.4);
  assert.ok(!f88pose.occluded,
    `the same unit, its frame's alpha content shifted 0.4 wu east (an animation pose), ` +
    `loses the record (fraction ${f88pose.fraction.toFixed(3)} < 0.15) — standing in the same place`);
  const f90 = foeAt(9.0);
  assert.ok(!f90.occluded && f90.fraction === 0,
    'past the TC\'s front-depth line no occluder is in front at all — no record, by painter\'s order');
});

// ------------------------------------------------------------------
// Post-fix behaviour on capture B (fails on pristine: pristine
// emits picked:1 -> garrison_tc ok:false -> move, verbatim the
// live signature).

test('capture B post-fix: the right-click issues Attack on the record-less enemy (FIX #7)', () => {
  const w = makeWorld('B');
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the enemy by its body, not our Villager\'s outline');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'exactly one command: {t:\'attack\', e:4, tg:56} — no garrison_tc, no move');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick, 'occLog recorded the pick resolution (instrument intact)');
  assert.deepEqual(pick.selection, [4]);
  assert.equal(pick.picked, 56);
  assert.equal(pick.afterGate, 56, 'the fog gate keeps it (its cell is visible)');
  assert.equal(pick.outline, 1, 'the RAW outline pick still names our Villager — picked != outline proves the preference fired');
  assert.equal(pick.outlineRect, 1);
  assert.deepEqual(pick.recordIds, [2, 4, 1, 3], 'the foe still holds no record — the body, not a record, resolved it');
  assert.equal(pick.pickedKind, 2, 'Scout Cavalry');
  assert.equal(pick.pickedOwner, 2, 'the foe seat — contrast capture B\'s pickedOwner:0');
  assert.equal(pick.tFromOutline, false,
    'the FIX #2b exemption names the raw outline winner only; the gate passes on the cell\'s own visibility');
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.cmd.tg, e.ok]), [['attack', 56, true]]);
  assert.equal(w.selectionAt(wx, wy), 1,
    'left-click selection is unchanged: at this pixel it still names the frontmost outline (the Villager)');
});

test('record-rect stage: a foe body under a friendly rect (outside every outline) also attacks (FIX #7)', () => {
  const w = makeWorld('R');
  const [wx, wy] = w.captureWorld;
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: the outline pick misses this pixel (outside every rect ∩ cover)');
  assert.equal(w.occludedRectUnitAt(wx, wy), 1,
    'staging: the raw record-rect pick names our Villager');
  assert.equal(w.selectionAt(wx, wy), 56,
    'the selection pick names the foe here (it has no record-rect stage; its body pick\'s foe class wins) — ' +
    'the one genuine selection/order geometry divergence, closed by FIX #7 at the rect stage');
  assert.equal(w.entityAt(wx, wy, true), 56,
    'the order pick now resolves the foe by body at the rect stage too');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]]);
});

// ------------------------------------------------------------------
// Regressions: capture A, preservation, hosts, fog.

test('capture A regression: the foe with a record still wins through FIX #6 (FIX #7 does not shadow it)', () => {
  const w = makeWorld('A');
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 56);
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'capture A still resolves to the foe and attacks, as published');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.deepEqual(pick.recordIds, [2, 1, 56, 4, 3]);
});

test('no enemy resolved at the point: capture B geometry without the foe still degrades as before', () => {
  const w = makeWorld('B');
  w.st.hp[56 * 2] = 0; // the foe is gone — no record, no body, nothing resolved
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 1,
    'with no enemy resolved at the point, our own outline winner stands');
  w.orderAt(wx, wy, false, true);
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.ok]), [['garrison_tc', false], ['move', true]],
    'the pre-fix shape is preserved exactly: refused garrison, then a ground move');
});

test('TC footprint with NO enemy present still garrisons a foot unit', () => {
  const w = makeWorld('G', { selKind: 3 });
  w.orderAt(-4, 4, false, true); // deep inside the footprint, away from every unit/record
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 4, 0]],
    'a foot unit garrisons into its own TC as before');
});

test('host exemption: an own Battering Ram at the point keeps its win over a body-resolved foe', () => {
  const w = makeWorld('B', { selKind: 17 });
  // Move the Ram (id 4) onto villager 1's spot so its record is
  // the one hit at the capture pixel; the foe's body is hit
  // there too — the Ram must still win.
  w.st.pos[4 * 2] = 5.5; w.st.pos[4 * 2 + 1] = -2.5;
  w.st.kinds[4] = 17;
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 4,
    'the own Ram at the point is a garrison target, never shadowed by the foe preference');
});

test('fog discipline: a body-resolved foe whose cell is not visible does not fire the preference', () => {
  const w = makeWorld('B', { fogHidesFoe: true });
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 1,
    'the foe\'s cell reads fog-hidden, so its body names nothing — our outline winner stands ' +
    '(the body stage\'s own fog rule, restated in foeBodyAtPoint)');
});
