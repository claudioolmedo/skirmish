// Occluded-outline ORDER degradation, round 12 (bot-loop
// occorder12, 2026-11-19; follow-up to run #437, the directed
// live vigia on build 579708a — after FIX #9 published).
//
// THE CAPTURE (run #437, live): solo vs AI, enemy Scout under
// the player's Town Center roof, red outline only. The player
// selected his own MILITIA (a FOOT unit — every earlier live
// capture had a MOUNTED Scout attacker) and right-clicked the
// outline. Attempt 1: the Militia moved adjacent and idled.
// Attempt 2: the Militia GARRISONED into the TC — "Garrisoned."
// toast, TC panel "Garrisoned: 1/15". No attack either time.
// With a mounted attacker the same pick failure degraded
// through a REFUSED garrison_tc (mounted units cannot garrison)
// to a ground move; with a foot attacker the garrison SUCCEEDS,
// so the fork is no longer a refused order — the defender
// leaves the field and enters the building it was defending.
//
// ------------------------------------------------------------------
// DISCRIMINATION (measured in this harness; sweep + probes)
//
// FIX #9 (occorder11) gave the OUTLINE stage the ground
// candidate, and its honesty bound named the one stage left
// unwidened: the record-rect stage (occ === null, occRect own
// winner, foe in ground range only), for lack of a live
// capture. This round swept the staged Militia-vs-Scout frame
// on the pristine FIX #9 base (2 px grid over the figures'
// screen region, 841 points) and measured EVERY point where
// entityAt returned our own unit while the foe stood inside
// the 2.2 ground pick radius: 32 points, ALL with occ === null
// — the outline stage is complete (no gap point has an outline
// winner, none lies inside the foe's rect). The 32 split by
// the stage that short-circuits:
//
//  (R) 25 points — RECORD-RECT stage: the point is inside our
//      own Militia's recorded rect but OUTSIDE every
//      rect ∩ cover (the visible part of our unit beside the
//      TC art's edge, so occludedUnitAt misses and the outline
//      stage's foe preference — ground included — never runs),
//      at the foe's feet: the foe's rect misses (the point is
//      just east of / below it), its body misses (the click is
//      screen-down from the foe's ground point, h < 0), and
//      this stage consults foeRect + foeBody but NOT
//      foeGroundAtPoint. entityAt returns our Militia;
//      orderAt reinterprets the click from its ground point
//      inside our own TC footprint: garrison_tc ok:TRUE.
//  (B) 7 points — BODY stage: the point is inside our own
//      Militia's drawn BODY box (wider in dx than its recorded
//      rect, so a band of body points reaches this stage with
//      occ === null AND occRect === null), again at the foe's
//      feet (foe body misses, h < 0), foe inside the ground
//      radius. The body stage's class system returns our own
//      unit (class 2 — no foe body hit to outrank it) without
//      consulting the ground candidate, short-circuiting the
//      ground stage's FIX #2c reading one stage early. Same
//      degradation: garrison_tc ok:TRUE.
//
// Both staged points below reproduce the capture verbatim on
// the pristine base (picked:4 / pickedOwner:0 -> garrison_tc
// ok:true, the Militia entering the TC), and both are LOW
// clicks at the foe's feet — the same click family as run
// #434's capture, one stage further down. A human aims low at
// the outline because the outline's feet are its lowest
// visible part, at the TC art's lower edge — exactly where
// our defending unit's rect/body, standing in front of the
// foe, extends past the cover.
//
// THE FIX (FIX #10, entityAt only): consult foeGroundAtPoint()
// at the two stages that could still return our own unit
// without it — LAST inside the record-rect stage's foe gate
// (after foeRect and foeBody, as at the outline stage), and
// at the body stage when the body winner is our own unit
// (class 2 only: an own Transport Ship's class-0 host win and
// a foe's class-1 win are untouched). Every stage that can
// name our own unit now arbitrates the ground candidate
// first — FIX #2c's documented reading, applied uniformly.
// Fog discipline is the ground candidate's own (FIX #9): a
// target named this way is not tFromOutline, so orderAt's fog
// gate still applies to it. Raw picks, selectionAt and
// orderAt are untouched; no sim change, golden hash untouched
// by construction.
//
// BOUNDS (pinned below): a foe OUTSIDE the ground radius (and
// outside every foe geometry) still does not shadow our own
// rect/body winner — the garrison reading survives there, and
// a TC-footprint click with no enemy resolved still garrisons
// a foot unit exactly as before. A click naming no foe
// geometry with the foe out of radius, south of the TC, keeps
// attempt 1's plain-move shape.
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
const MIL_RECT = { x0: -0.9, y0: -2.6, w: 1.8, h: 2.75 };   // militia figure, same frame class

function makeHarness(st, records, issued, occEntriesBundle) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = st.kinds;
  const { occLog, entries } = occEntriesBundle;
  const applyLocalOrder = (cmd) => {
    let ok = true;
    if (cmd.t === 'garrison_tc' || cmd.t === 'garrison_u') {
      ok = kinds[cmd.e] === 0 || kinds[cmd.e] === 3; // foot kinds only — the Militia garrisons
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
    cellVisibleW: () => true,
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
    playback: null, net: null, playing: true, localRecorder: null,
    applyLocalOrder,
    occLog,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${occSrc}\n${entityAtSrc}\n${issueSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, occludedUnitAt, occludedRectUnitAt, tcAt, worldToScreen, spriteBodyD2, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
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

// ------------------------------------------------------------------
// The capture scene. TC 0 at the origin; villagers 1/2/3 behind
// it; the foe Scout 56 at (6,-1) under the TC (record held);
// our own MILITIA 4 (kind 3, the selected attacker) at (8,-1),
// standing in front of the foe at the TC art's east edge — its
// recorded rect extends past the cover's east edge and its body
// box past its rect. Records in the order [2,56,4,1,3].
//   Point R (1036,564) — world (7.75,-1.75): inside our
//     Militia's rect, outside every rect ∩ cover, 2.8 px below
//     the foe rect's bottom edge and 3.2 px east of its right
//     edge — the foe's feet, on our Militia's figure.
//   Point B (1040.4,560) — world (7.53,-2.53): outside every
//     rect, inside our Militia's body box (dx 1.05, past the
//     rect's 0.9 edge), at the foe's feet level.
function makeWorld({ selKind = 3, foePos = [6, -1] } = {}) {
  const st = blankState(57);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 1, 0, 0, -3, -3, 50);
  put(st, 2, 0, 0, -5, 0, 50);
  put(st, 3, 0, 0, 0, -5, 50);
  put(st, 4, selKind, 0, 8, -1, selKind === 17 ? 150 : 40);
  put(st, 56, 2, 2, foePos[0], foePos[1], 45);
  const issued = [];
  const records = [];
  const { fns, occEntries } = makeHarness(st, records, issued, makeOccLog('occlog=1'));
  const [tsx, tsy] = fns.worldToScreen(0, 0);
  const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
  const addRecord = (id, box) => {
    const [sx, sy] = fns.worldToScreen(st.pos[id * 2], st.pos[id * 2 + 1]);
    const rect = { x: sx + box.x0 * 8, y: sy + box.y0 * 8, w: box.w * 8, h: box.h * 8 };
    const depth = st.pos[id * 2] + st.pos[id * 2 + 1];
    const res = occlusionFor({ rect, depth, fogVisible: true }, [{ rect: tcCover, depth: 8 }]);
    assert.ok(res.occluded, `staging: unit ${id} is occluded (a record exists)`);
    return { id, rect, cover: res.cover.map((o) => o.rect), depth };
  };
  records.push(addRecord(2, VIL_RECT));
  records.push(addRecord(56, TALL_RECT));
  records.push(addRecord(4, MIL_RECT));
  records.push(addRecord(1, VIL_RECT));
  records.push(addRecord(3, VIL_RECT));
  const pointR = [1036, 564];
  const pointB = [1040.4, 560];
  return {
    ...fns, issued, records, occEntries, st, tcCover,
    pointR, worldR: fns.s2w(...pointR),
    pointB, worldB: fns.s2w(...pointB),
  };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #10 wiring: the ground extension lives at entityAt\'s rect and body stages', () => {
  assert.match(entityAtSrc, /FIX #10 \(bot-loop occorder12/,
    'the fix is marked in entityAt');
  assert.equal((entityAtSrc.match(/foeGroundAtPoint\(\)/g) || []).length, 3,
    'the ground candidate is now consulted at all three stages that can return our own unit (outline, record-rect, body)');
  assert.equal((entityAtSrc.match(/foeRectAtPoint\(\)/g) || []).length, 1,
    'FIX #8\'s rect candidate is still consulted exactly once (occorder10 pin)');
  assert.equal((entityAtSrc.match(/foeBodyAtPoint\(\)/g) || []).length, 2,
    'FIX #7\'s body candidate is still consulted at exactly its two stages (occorder8 pin)');
  assert.match(entityAtSrc, /if \(occRect !== null\) return occRect;/,
    'the record-rect return line is preserved (earlier wiring pins index it)');
  assert.ok(!/FIX #10|foeGroundAtPoint/.test(occSrc),
    'the raw outline/record-rect picks carry no foe preference (selection parity)');
});

// ------------------------------------------------------------------
// Staging R: the raw layer at point R (passes before and after
// the fix — the layer did not move). This is the discrimination
// for the record-rect half of the fork.

test('staging R: raw picks match the fork — no outline hit, our rect wins, foe geometries miss, foe in ground range', () => {
  const w = makeWorld({});
  const [wx, wy] = w.worldR;
  assert.deepEqual(w.records.map((r) => r.id), [2, 56, 4, 1, 3],
    'the foe holds a record this frame');
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'the outline pick misses: the point is outside every rect ∩ cover (our Militia\'s visible part, past the TC cover\'s east edge) — the outline stage\'s ground candidate never runs');
  assert.equal(w.occludedRectUnitAt(wx, wy), 4,
    'the raw record-rect pick names our own Militia — the stage that short-circuits');
  assert.equal(w.tcAt(wx, wy), 0,
    'the click\'s ground point lies inside our own TC footprint — the garrison reinterpretation is armed');
  const [sx, sy] = w.pointR;
  const foeRec = w.records.find((r) => r.id === 56);
  assert.ok(sy > foeRec.rect.y + foeRec.rect.h && sx > foeRec.rect.x + foeRec.rect.w,
    'the foe\'s recorded rect misses the point (below its bottom edge, east of its right edge — the foe\'s feet corner)');
  assert.equal(w.spriteBodyD2(w.st, 56, wx, wy), null,
    'the foe\'s body box misses too: the click is screen-down from its ground point (h < 0)');
  assert.ok(w.spriteBodyD2(w.st, 4, wx, wy) !== null,
    'our Militia\'s body is hit at the point — the click lands on its figure');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 1.90) < 0.01 && dFoe < 2.2,
    `but the foe stands ${dFoe.toFixed(2)} wu from the click's ground point — inside the 2.2 ground pick radius this stage never consulted`);
});

// ------------------------------------------------------------------
// Post-fix behaviour at point R (fails on pristine: pristine
// emits picked:4 -> garrison_tc ok:TRUE — run #437 attempt 2's
// signature verbatim, the Militia entering the TC).

test('capture R post-fix: the right-click issues Attack on the ground-resolved enemy, not a garrison (FIX #10)', () => {
  const w = makeWorld({});
  const [wx, wy] = w.worldR;
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the enemy by ground proximity at the record-rect stage, not our own Militia');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'exactly one command: {t:\'attack\', e:4, tg:56} — no garrison_tc, no move');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick, 'occLog recorded the pick resolution (instrument intact)');
  assert.deepEqual(pick.selection, [4]);
  assert.equal(pick.picked, 56, 'the pick now resolves to the enemy');
  assert.equal(pick.afterGate, 56, 'the fog gate keeps it (its cell is visible)');
  assert.equal(pick.tFromOutline, false,
    'the target was NOT named by the raw outline pick — orderAt\'s fog gate therefore applied to it, as for a ground-stage pick');
  assert.equal(pick.outline, null, 'the RAW outline pick still misses — picked != outline proves the preference fired inside entityAt');
  assert.equal(pick.outlineRect, 4, 'the RAW record-rect pick still names our Militia — the foe is resolved inside entityAt, not by moving the raw picks');
  assert.deepEqual(pick.recordIds, [2, 56, 4, 1, 3]);
  assert.equal(pick.pickedKind, 2, 'Scout Cavalry — the foe');
  assert.equal(pick.pickedOwner, 2, 'the foe seat — contrast the capture\'s pickedOwner:0');
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.cmd.tg, e.ok]), [['attack', 56, true]],
    'the issue log shows the single accepted attack — no garrison_tc at all');
});

// ------------------------------------------------------------------
// Staging + post-fix at point B: the body-stage half of the
// fork (fails on pristine with the same garrison signature).

test('capture B post-fix: a foe at the feet of our own body winner also attacks (FIX #10, body stage)', () => {
  const w = makeWorld({});
  const [wx, wy] = w.worldB;
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: the outline pick misses this point');
  assert.equal(w.occludedRectUnitAt(wx, wy), null,
    'staging: the record-rect pick misses too — the point is past our Militia\'s rect edge (dx 1.05 > 0.9)');
  assert.ok(w.spriteBodyD2(w.st, 4, wx, wy) !== null,
    'staging: our Militia\'s drawn body IS hit — the body stage is the one that short-circuits');
  assert.equal(w.spriteBodyD2(w.st, 56, wx, wy), null,
    'staging: the foe\'s body misses (the click is screen-down from its ground point)');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(dFoe < 2.2, `staging: the foe is ${dFoe.toFixed(2)} wu away — inside the ground radius`);
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the enemy by ground proximity at the body stage, not our own Militia');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'exactly one command: {t:\'attack\', e:4, tg:56} — no garrison_tc, no move');
});

// ------------------------------------------------------------------
// The sweep: over the staged frame, NO point with the foe
// inside the ground radius resolves to our own unit any more
// (32 such points on the pristine base — 25 rect-stage,
// 7 body-stage, all occ === null).

test('sweep: no click point with the foe in ground range resolves to our own unit', () => {
  const w = makeWorld({});
  const gaps = [];
  for (let sx = 996; sx <= 1052; sx += 2) {
    for (let sy = 524; sy <= 580; sy += 2) {
      const [wx, wy] = w.s2w(sx, sy);
      const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
      if (dFoe >= 2.2) continue;
      const ent = w.entityAt(wx, wy, true);
      if (ent !== 56 && ent !== null && w.st.owners[ent] === 0) gaps.push([sx, sy, ent]);
    }
  }
  assert.deepEqual(gaps, [],
    'every point whose ground conversion lands within the foe\'s pick radius resolves the foe (or nothing), never our own unit');
});

// ------------------------------------------------------------------
// Bounds: the garrison reading survives wherever NO enemy is
// resolved at the point.

test('bound R: a foe outside the ground radius still does not shadow our own rect winner — the garrison stands', () => {
  const w = makeWorld({ foePos: [-6, -1] });
  const [wx, wy] = w.worldR;
  assert.equal(w.occludedRectUnitAt(wx, wy), 4, 'staging: only our Militia\'s rect is hit');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2, `staging: the foe is ${dFoe.toFixed(2)} wu away — outside the ground radius`);
  assert.equal(w.entityAt(wx, wy, true), 4,
    'with no foe outline, rect, body or ground hit at the point, the preference does not fire');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 4, 0]],
    'legitimate garrison preserved: the foot Militia enters its own TC exactly as before');
});

test('bound: a TC-footprint click with no enemy present still garrisons a foot unit', () => {
  const w = makeWorld({});
  w.st.hp[56 * 2] = 0; // the foe is gone — nothing to resolve anywhere
  w.orderAt(-4, 4, false, true); // deep inside the footprint, away from every unit/record
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 4, 0]],
    'a foot unit garrisons into its own TC as before');
});

test('bound: attempt 1\'s shape survives — a click naming no foe geometry, foe out of radius, keeps the plain move', () => {
  const w = makeWorld({});
  const [wx, wy] = w.s2w(1020, 575); // south of the TC art, on open ground
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2, `staging: the foe is ${dFoe.toFixed(2)} wu away`);
  assert.equal(w.tcAt(wx, wy), null, 'staging: the point is outside the TC footprint too');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'no enemy resolved and no building named: a ground move, exactly as before');
});

test('host exemption: an own Transport Ship body winner keeps its win over a ground-resolved foe', () => {
  const w = makeWorld({ selKind: 32 });
  const [wx, wy] = w.worldB;
  assert.ok(w.spriteBodyD2(w.st, 4, wx, wy) !== null,
    'staging: the Transport\'s body is hit at the body-stage point');
  assert.equal(w.entityAt(wx, wy, true), 4,
    'the own Transport (class 0, a load target) is never shadowed by the ground foe preference');
});
