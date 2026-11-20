// Occluded-outline ORDER degradation, round 13 (bot-loop
// occorder13, 2026-11-19; follow-up to run #438, the directed
// live vigia on build a90a1a5 — FIX #10 live and served-
// verified, in which the failure recurred in a NEW geometry).
//
// THE CAPTURE (run #438, live, solo vs AI·Normal, hard
// reload): an enemy Scout sat under the player's Town Center
// roof AT THE TC CENTRE (visible from minute 1, red outline
// only), draining the TC. The player selected his own
// MILITIA (foot unit, stance confirmed Aggressive) and
// right-clicked LOW at the feet / bottom edge of the red
// outline — TWICE, with two different Militia: BOTH TIMES
// the Militia GARRISONED into the own TC ("Garrisoned."
// toast; TC panel "Garrisoned: 1/15", then "2/15"). No attack
// was issued. A left-click on the outline in the same
// geometry selected the player's own VILLAGER standing in
// front of the foe. The differences from every staged case
// so far: the staged foe stood at the TC's EAST EDGE (6,-1);
// this foe stood at/near the CENTRE of the footprint, and a
// friendly villager stood in front of it.
//
// ------------------------------------------------------------------
// DISCRIMINATION (measured in this harness; column scan +
// sweeps over the staged centre frame)
//
// Staged here: TC 0 at the origin; foe Scout 56 at (0,0),
// the footprint CENTRE, record held; villagers behind the
// TC; the selected Militia 4 east of the footprint; variant
// B adds our Villager 1 at (2,2), in front of the foe. A
// column scan at the foe's screen column (x = 960, 2 px
// steps from the figure's top to below the TC art) on the
// pristine FIX #10 base measures the fork exactly:
//
//   - On the foe's figure and for ~11 px below its feet the
//     pick already resolves the foe (outline / rect / body /
//     the 2.2 ground radius): attack at ground distance
//     d = 1.94 (screen y 551).
//   - The garrison band begins EXACTLY where the ground
//     distance passes 2.2 (garrison at d = 2.30, y 553) and
//     runs down the TC art to its bottom edge (y 575,
//     d = 6.19). There every foe geometry misses — the click
//     is screen-down from the foe (spriteBodyD2 h < 0), below
//     its recorded rect (bottom edge y 541) and outside the
//     ground radius — yet the click is still ON the Town
//     Center the foe hides under, inside its footprint
//     (tcAt = 0 throughout the band).
//   - In variant B the band's upper part (y 553–557) is won
//     by the front Villager's OUTLINE (its record extends
//     past the foe's feet): entityAt's outline gate consults
//     foeOcc / foeRect / foeBody / foeGround, all miss, and
//     returns the Villager → garrison_tc ok:TRUE. Below the
//     Villager's rect (y 559+) and everywhere in variant A,
//     NO stage names anything (entityAt = null) and
//     orderAt's tcAt garrison branch fires with no foe
//     arbitration anywhere in the order path.
//
// The candidate stages from the assignment, one by one:
//   (i) "the TC building's own record/body winning the pick"
//       — in shape, TRUE: the TC never enters entityAt (it is
//       kind 1, no body box, no record), but its tcAt branch
//       in orderAt is the branch that fires, and it fires
//       BECAUSE the pick named the front Villager or nothing
//       — the building wins by default inside its footprint.
//   (ii) "foeGroundAtPoint not consulted or out of radius"
//       — out of radius, measured: the band is d ∈ (2.2, 6.2]
//       in the foe's column. It IS consulted at every own-win
//       stage since FIX #10; the radius is what ends.
//   (iii) "the ground candidate suppressed inside the
//       footprint" — TRUE as the composing mechanism: inside
//       the footprint, a click that resolves no foe is read
//       as a garrison by construction, and no stage arbitrated
//       a foe BETWEEN the ground radius and the footprint's
//       own extent.
//   (iv) "the villager's record/body outranking the foe at
//       the outline stage" — TRUE for the band's upper part
//       (order) and, separately, for SELECTION (below).
//
// Why the east-edge stagings never showed the band: below
// the east foe's feet lay open ground OUTSIDE the footprint
// (a move, not a garrison — occorder12's attempt-1 bound),
// and our own defending unit's rect/body stood in the band
// carrying FIX #10's consultations. At the centre the band
// lies on the TC art itself.
//
// SELECTION (the capture's left-click): selectionAt has NO
// ground foe candidate at all — its body stage's ownership
// classes are its only foe arbitration, and its fallback is
// unitAt (own-only by contract). Measured at the foe's feet
// in variant B: points where the foe's body misses (h < 0),
// no outline winner exists, and the foe stands inside the
// 2.2 ground radius name our Villager instead — by its BODY
// (point S1, d = 0.88) and by its unitAt ground radius
// (point S2, d = 2.13, Villager 1.87 away). entityAt already
// attacks at both points; the selection layer alone fails.
//
// THE FIX (FIX #11, client pick layer only):
//   * entityAt gains the TC-arena ground candidate
//     (foeTcArenaAtPoint): when the click's ground point lies
//     inside OUR Town Center's footprint and a non-own unit
//     stands inside that SAME footprint within one
//     half-extent (tcHalf) of the click, that foe is resolved
//     at the point. Consulted LAST at the three own-win gates
//     (outline, record-rect, body — after foeGroundAtPoint)
//     and at the ground stage when the winner is our own
//     non-host unit or nothing. The half-extent limit keeps
//     the earlier bounds: occorder11's bound B (foe 12.25 wu
//     from the click) and occorder12's bound R (13.75 wu)
//     keep their shapes, and a click outside the footprint
//     never reaches the candidate. Fog: the candidate carries
//     the body candidate's pick-level fog rule (a foe whose
//     cell is not currently visible names nothing —
//     occorder8's pin), and a target named this way is not
//     tFromOutline, so orderAt's fog gate still applies.
//     Host exemptions (Ram / Transport) are untouched at
//     every site; the minimap path (screenPick === false)
//     never consults it.
//   * selectionAt gains the ground foe candidate
//     (foeGroundSelAtPoint, the 2.2 radius with the fog rule
//     restated — this pick has no post-pick gate), consulted
//     only to SHADOW an own winner: at the body stage when
//     the winner is our own class-2 unit, and at the unitAt
//     fallback when unitAt would name our own unit. It never
//     creates a selection from nothing, and the outline stage
//     keeps the frontmost-outline doctrine (occorder8).
// Raw picks (occludedUnitAt / occludedRectUnitAt), orderAt
// and unitAt are untouched; no sim change, golden hash
// untouched by construction.
//
// BOUNDS (pinned below): a foe inside the footprint but
// beyond one half-extent of the click does not shadow our
// own outline winner (the garrison stands); a TC-footprint
// click with no enemy present still garrisons; an
// open-ground click outside the footprint keeps the plain
// move; the minimap pick keeps its pre-fix shape; an own
// Ram / Transport Ship at the point keeps its host win.
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
const MIL_RECT = { x0: -0.9, y0: -2.6, w: 1.8, h: 2.75 };   // militia / transport staging box

function makeHarness(st, records, issued, occEntriesBundle, foePos) {
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

// ------------------------------------------------------------------
// The capture scene, centre geometry. TC 0 at the origin; the
// foe Scout 56 at (0,0) — the CENTRE of the footprint (the
// east-edge stagings used (6,-1)); villagers 2/3 behind the
// TC; our MILITIA 4 (kind 3, the selected attacker) east of
// the footprint at (9,-1). Variant B (frontVil) puts our
// Villager 1 at (2,2), in front of the foe (depth 4 > 0); in
// variant A it stands behind the TC at (-3,-3) with the
// others. Records in the order [2,56,1,3] (+4 when the
// staged attacker is itself occluded, in the host tests).
//   Point O (960,555) — world (1.88,1.88): in variant B the
//     front Villager's outline/rect win; the foe is 2.65 wu
//     away — past the ground radius, inside the arena.
//   Point N (960,565) — world (3.13,3.13): in variant A no
//     stage names anything; the foe is 4.42 wu away.
//   Point S1 (952,543) — world (-0.13,0.88): selection by
//     the Villager's body, the foe 0.88 wu away.
//   Point S2 (944,549) — world (0.13,2.13): selection by
//     the Villager's unitAt ground radius, the foe 2.13 wu.
function makeWorld({ foePos = [0, 0], frontVil = false, selKind = 3, selPos = [9, -1], fogHidesFoe = false, foeDead = false } = {}) {
  makeHarness.fogHidesFoe = fogHidesFoe;
  const st = blankState(57);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 1, 0, 0, frontVil ? 2 : -3, frontVil ? 2 : -3, 50);
  put(st, 2, 0, 0, -5, 0, 50);
  put(st, 3, 0, 0, 0, -5, 50);
  put(st, 4, selKind, 0, selPos[0], selPos[1], selKind === 17 ? 150 : 40);
  put(st, 56, 2, 2, foePos[0], foePos[1], foeDead ? 0 : 45);
  const issued = [];
  const records = [];
  const { fns, occEntries } = makeHarness(st, records, issued, makeOccLog('occlog=1'), fogHidesFoe ? foePos : null);
  const [tsx, tsy] = fns.worldToScreen(0, 0);
  const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
  const addRecord = (id, box) => {
    const [sx, sy] = fns.worldToScreen(st.pos[id * 2], st.pos[id * 2 + 1]);
    const rect = { x: sx + box.x0 * 8, y: sy + box.y0 * 8, w: box.w * 8, h: box.h * 8 };
    const depth = st.pos[id * 2] + st.pos[id * 2 + 1];
    const res = occlusionFor({ rect, depth, fogVisible: true }, [{ rect: tcCover, depth: 8 }]);
    if (!res.occluded) return null;
    return { id, rect, cover: res.cover.map((o) => o.rect), depth };
  };
  for (const [id, box] of [[2, VIL_RECT], [56, TALL_RECT], [1, VIL_RECT], [3, VIL_RECT], [4, MIL_RECT]]) {
    const r = addRecord(id, box);
    if (r) records.push(r);
  }
  const pt = (sx, sy) => ({ screen: [sx, sy], world: fns.s2w(sx, sy) });
  return {
    ...fns, issued, records, occEntries, st, tcCover,
    pointO: pt(960, 555), pointN: pt(960, 565),
    pointS1: pt(952, 543), pointS2: pt(944, 549),
  };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #11 wiring: the arena lives in entityAt, the ground shadow in selectionAt; earlier counts unchanged', () => {
  assert.match(entityAtSrc, /FIX #11 \(bot-loop occorder13/,
    'the fix is marked in entityAt');
  assert.match(entityAtSrc, /const foeTcArenaAtPoint = \(\) =>/,
    'the TC-arena candidate is defined in entityAt');
  assert.equal((entityAtSrc.match(/foeTcArenaAtPoint\(\)/g) || []).length, 4,
    'the arena is consulted at the three own-win gates and at the ground stage');
  assert.equal((entityAtSrc.match(/foeGroundAtPoint\(\)/g) || []).length, 3,
    'FIX #9/#10\'s ground consultations are unchanged (occorder11/12 pins)');
  assert.equal((entityAtSrc.match(/foeRectAtPoint\(\)/g) || []).length, 1,
    'FIX #8\'s rect candidate is still consulted exactly once (occorder10 pin)');
  assert.equal((entityAtSrc.match(/foeBodyAtPoint\(\)/g) || []).length, 2,
    'FIX #7\'s body candidate is still consulted at exactly its two stages (occorder8 pin)');
  assert.match(selectionAtSrc, /FIX #11 \(bot-loop occorder13/,
    'the selection half of the fix is marked in selectionAt');
  assert.match(selectionAtSrc, /const foeGroundSelAtPoint = \(\) =>/,
    'the selection ground candidate is defined in selectionAt');
  assert.ok(!/FIX #11|foeTcArenaAtPoint|foeGroundSelAtPoint/.test(occSrc),
    'the raw outline/record-rect picks carry no foe preference (selection parity)');
  // AMENDMENT (occorder14, 2026-11-20): this pin originally
  // asserted selectionAt does NOT consult the TC arena (its
  // shadow was the 2.2 ground candidate only). Run #440's
  // Test B is the live capture proving that shape wrong at
  // the centre geometry: a left-click LOW at the centre
  // foe's feet selected the player's own Scout (body /
  // ground-radius winner, foe 4.24–7.07 away) while
  // entityAt resolved the foe. FIX #12 therefore gives
  // selectionAt its own arena candidate
  // (foeTcArenaSelAtPoint, consulted only to shadow an own
  // winner at the ground shadow's two sites); the pin now
  // asserts that FIX #12 state instead. Every other pin in
  // this test is unchanged, and this suite's behavioural
  // tests (S1/S2, sweeps, bounds, fog) pass unamended on
  // the FIX #12 build.
  assert.match(selectionAtSrc, /const foeTcArenaSelAtPoint = \(\) =>/,
    'selectionAt consults its own TC-arena candidate since FIX #12 (occorder14 amendment — see note above)');
  assert.ok(!/const foeTcArenaAtPoint = \(\) =>/.test(selectionAtSrc),
    'entityAt\'s arena candidate itself is not duplicated in selectionAt — the selection candidate is its own restatement');
});

// ------------------------------------------------------------------
// Staging O: the raw layer at point O in variant B (passes
// before and after the fix — the layer did not move). This is
// the discrimination for the outline-stage half of the fork.

test('staging O: raw picks match the fork — the front Villager wins the outline, every foe geometry misses, the foe is in arena range', () => {
  const w = makeWorld({ frontVil: true });
  const [wx, wy] = w.pointO.world;
  assert.deepEqual(w.records.map((r) => r.id), [2, 56, 1, 3],
    'the foe holds a record this frame, and the front Villager holds one in front of it');
  assert.equal(w.occludedUnitAt(wx, wy), 1,
    'the raw outline pick names our front Villager — the stage that short-circuits');
  assert.equal(w.occludedRectUnitAt(wx, wy), 1,
    'the raw record-rect pick names it too');
  assert.equal(w.tcAt(wx, wy), 0,
    'the click\'s ground point lies inside our own TC footprint — the garrison reinterpretation is armed');
  const [sx, sy] = w.pointO.screen;
  const foeRec = w.records.find((r) => r.id === 56);
  assert.ok(sy > foeRec.rect.y + foeRec.rect.h,
    'the foe\'s recorded rect misses the point (the point is below its bottom edge — the foe\'s feet band on the TC art)');
  assert.equal(occludedPickAt(w.records.filter((r) => r.id === 56), sx, sy), null,
    'the foe\'s outline intersection misses too (a subset of its rect)');
  assert.equal(w.spriteBodyD2(w.st, 56, wx, wy), null,
    'and the foe\'s body box misses: the click is screen-down from its ground point (h < 0)');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 2.65) < 0.01 && dFoe > 2.2 && dFoe < 8,
    `but the foe stands ${dFoe.toFixed(2)} wu from the click — past the 2.2 ground radius, within one TC half-extent, inside the same footprint`);
});

// ------------------------------------------------------------------
// Post-fix behaviour at point O (fails on pristine: pristine
// emits picked:1 -> garrison_tc ok:TRUE — run #438's signature
// verbatim, the Militia entering the TC it was defending).

test('capture O post-fix: the right-click issues Attack on the arena-resolved enemy, not a garrison (FIX #11)', () => {
  const w = makeWorld({ frontVil: true });
  const [wx, wy] = w.pointO.world;
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the enemy by the TC arena at the outline stage, not our front Villager');
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
  assert.equal(pick.outline, 1, 'the RAW outline pick still names our Villager — picked != outline proves the preference fired inside entityAt');
  assert.equal(pick.outlineRect, 1, 'the RAW record-rect pick still names our Villager');
  assert.deepEqual(pick.recordIds, [2, 56, 1, 3]);
  assert.equal(pick.pickedKind, 2, 'Scout Cavalry — the foe');
  assert.equal(pick.pickedOwner, 2, 'the foe seat — contrast the capture\'s garrison outcome');
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.cmd.tg, e.ok]), [['attack', 56, true]],
    'the issue log shows the single accepted attack — no garrison_tc at all');
});

// ------------------------------------------------------------------
// Capture N: the null band in variant A — no stage names
// anything on pristine, orderAt garrisons via tcAt alone
// (fails on pristine with the same garrison signature).

test('capture N post-fix: a click on the TC art below the centre foe\'s feet, naming nothing, also attacks (FIX #11, ground stage)', () => {
  const w = makeWorld({});
  const [wx, wy] = w.pointN.world;
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: the outline pick misses this point');
  assert.equal(w.occludedRectUnitAt(wx, wy), null,
    'staging: the record-rect pick misses too');
  assert.equal(w.tcAt(wx, wy), 0,
    'staging: the point is inside our own TC footprint — on pristine this click garrisons via tcAt with no pick at all');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 4.42) < 0.01 && dFoe > 2.2 && dFoe < 8,
    `staging: the foe is ${dFoe.toFixed(2)} wu away — past the ground radius, within the arena's half-extent`);
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the enemy by the TC arena at the ground stage, not nothing');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'exactly one command: {t:\'attack\', e:4, tg:56} — no garrison_tc, no move');
});

// ------------------------------------------------------------------
// Selection S1 / S2: the capture's left-click failure (both
// fail on pristine: selectionAt returns our Villager 1).

test('selection S1 post-fix: a LOW click at the foe\'s feet on our Villager\'s body selects the foe (FIX #11, body stage)', () => {
  const w = makeWorld({ frontVil: true });
  const [wx, wy] = w.pointS1.world;
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: no outline winner at this point — the selection is decided by the body stage');
  assert.equal(w.spriteBodyD2(w.st, 56, wx, wy), null,
    'staging: the foe\'s body misses (the click is screen-down from its ground point)');
  assert.ok(w.spriteBodyD2(w.st, 1, wx, wy) !== null,
    'staging: our front Villager\'s body IS hit — the stage that named it on pristine');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 0.88) < 0.01 && dFoe < 2.2,
    `staging: the foe is ${dFoe.toFixed(2)} wu away — inside the ground pick radius selectionAt never consulted`);
  assert.equal(w.entityAt(wx, wy, true), 56,
    'the order pick already resolved the foe at this point before FIX #11 — the divergence was selection-only');
  assert.equal(w.selectionAt(wx, wy), 56,
    'selectionAt now resolves the foe by ground proximity over our Villager\'s body win — an information selection of the enemy the player clicked');
});

test('selection S2 post-fix: a LOW click at the foe\'s feet in our Villager\'s ground radius selects the foe (FIX #11, ground fallback)', () => {
  const w = makeWorld({ frontVil: true });
  const [wx, wy] = w.pointS2.world;
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: no outline winner at this point');
  assert.equal(w.spriteBodyD2(w.st, 1, wx, wy), null,
    'staging: our Villager\'s body misses too — the selection is decided by the unitAt ground fallback');
  assert.equal(w.unitAt(wx, wy), 1,
    'staging: the raw own-only fallback names our Villager (its ground radius) — and unitAt itself is unchanged by the fix');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 2.13) < 0.01 && dFoe < 2.2,
    `staging: the foe is ${dFoe.toFixed(2)} wu away — inside the ground pick radius`);
  assert.equal(w.selectionAt(wx, wy), 56,
    'selectionAt shadows the own ground fallback with the ground-resolved foe');
});

// ------------------------------------------------------------------
// Sweeps: over the staged centre frame, no inside-footprint
// click within the arena resolves to our own unit or nothing;
// and no foe-feet point selects the front Villager.

test('sweep (order): every inside-footprint click within one half-extent of the centre foe resolves the foe', () => {
  const w = makeWorld({ frontVil: true });
  const gaps = [];
  for (let sx = 896; sx <= 1024; sx += 2) {
    for (let sy = 476; sy <= 604; sy += 2) {
      const [wx, wy] = w.s2w(sx, sy);
      if (w.tcAt(wx, wy) !== 0) continue;
      const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
      if (dFoe >= 8) continue; // the arena radius is strict, like every pick radius in the family
      const ent = w.entityAt(wx, wy, true);
      if (ent !== 56) gaps.push([sx, sy, ent]);
    }
  }
  assert.deepEqual(gaps, [],
    'inside the footprint, within the arena, the foe is always the pick — never our own unit, never nothing');
});

test('sweep (selection): no foe-feet point with no outline winner selects the front Villager', () => {
  const w = makeWorld({ frontVil: true });
  const gaps = [];
  for (let sx = 940; sx <= 980; sx += 2) {
    for (let sy = 530; sy <= 560; sy += 2) {
      const [wx, wy] = w.s2w(sx, sy);
      if (w.occludedUnitAt(wx, wy) !== null) continue; // the outline stage keeps its doctrine
      const foeBody = w.spriteBodyD2(w.st, 56, wx, wy) !== null;
      const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
      if (!foeBody && dFoe >= 2.2) continue;
      if (w.selectionAt(wx, wy) === 1) gaps.push([sx, sy]);
    }
  }
  assert.deepEqual(gaps, [],
    'where the foe is body- or ground-resolved and no outline winner exists, the selection never names our front Villager');
});

// ------------------------------------------------------------------
// Bounds: the arena is limited — by distance, by the
// footprint, by the enemy's presence, by the pick surface.

test('bound: a foe inside the footprint but beyond one half-extent of the click does not shadow our outline winner — the pick stands, the order falls through (occorder18)', () => {
  // occorder18 (FIX #15) supersedes this bound's order half:
  // the outline pick still stands (our Villager 1, no arena
  // preference), but a click naming an own unit no longer
  // garrisons — run #445's capture is this shape.
  const w = makeWorld({ frontVil: true, foePos: [-6, -1] });
  const [wx, wy] = w.pointO.world;
  assert.equal(w.occludedUnitAt(wx, wy), 1, 'staging: only our Villager\'s outline is hit');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 8.39) < 0.01 && dFoe > 8,
    `staging: the foe is ${dFoe.toFixed(2)} wu away — inside the footprint but beyond the arena's half-extent`);
  assert.equal(w.entityAt(wx, wy, true), 1,
    'with no foe outline, rect, body, ground or arena hit at the point, the preference does not fire');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['move', 4, undefined]],
    'occorder18: an own-unit pick inside the footprint issues a move, never a garrison');
});

test('bound: a TC-footprint click with no enemy present still garrisons a foot unit', () => {
  const w = makeWorld({ foeDead: true });
  const [wx, wy] = w.pointN.world;
  assert.equal(w.tcAt(wx, wy), 0, 'staging: the point is inside the footprint');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 4, 0]],
    'a foot unit garrisons into its own TC as before — the arena needs a living foe inside the footprint');
});

test('bound: an open-ground click outside the footprint keeps the plain move (attempt-1 shape)', () => {
  const w = makeWorld({});
  const [wx, wy] = w.s2w(1020, 575); // south-east of the TC art, on open ground
  assert.equal(w.tcAt(wx, wy), null, 'staging: the point is outside the TC footprint');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2, `staging: the foe is ${dFoe.toFixed(2)} wu away`);
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'no enemy resolved and no building named: a ground move, exactly as before — the arena is footprint-scoped');
});

test('bound: the minimap pick is untouched — no arena, no record stages (SEL-16)', () => {
  const w = makeWorld({ frontVil: true });
  const [wx, wy] = w.pointO.world;
  assert.equal(w.entityAt(wx, wy, false), 1,
    'with screenPick off, the pick is the plain ground pick (our Villager beside the point) — the arena never consults');
  w.orderAt(wx, wy, false, false);
  // occorder18 (FIX #15): the minimap PICK above is untouched
  // (SEL-16), but the order follows the same rule — the pick
  // named our Villager, so the click is not a TC garrison.
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['move', 4, undefined]],
    'occorder18: the minimap pick is unchanged, the order is a move — an own-unit pick never garrisons');
});

test('host exemptions: an own Transport Ship / Battering Ram in front of the foe keeps its win over the arena', () => {
  const wt = makeWorld({ selKind: 32, selPos: [2, 2] });
  const [tx, ty] = wt.pointO.world;
  assert.equal(wt.occludedUnitAt(tx, ty), 4,
    'staging: the Transport\'s outline wins the point (it stands in front of the foe, where the Villager stood in variant B)');
  assert.equal(wt.entityAt(tx, ty, true), 4,
    'the own Transport (a load host) at the point is never shadowed by the arena');
  const wr = makeWorld({ selKind: 17, selPos: [2, 2] });
  const [rx, ry] = wr.pointO.world;
  assert.equal(wr.entityAt(rx, ry, true), 4,
    'the own Battering Ram (a garrison host) at the point is never shadowed by the arena');
});

test('fog discipline: a foe whose cell is not visible names nothing in the arena or the selection shadow', () => {
  const w = makeWorld({ frontVil: true, fogHidesFoe: true });
  const [wx, wy] = w.pointO.world;
  assert.equal(w.entityAt(wx, wy, true), 1,
    'the foe\'s cell reads fog-hidden, so the arena names nothing — our outline winner stands (the body candidate\'s fog rule, carried by the arena)');
  const [sx1, sy1] = w.pointS1.world;
  assert.equal(w.selectionAt(sx1, sy1), 1,
    'the selection shadow carries the same fog rule — a fog-hidden foe does not shadow our Villager');
});
