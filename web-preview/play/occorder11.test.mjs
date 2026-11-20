// Occluded-outline ORDER degradation, round 11 (bot-loop
// occorder11, 2026-11-19; follow-up to run #434, the directed
// live vigia on the FIX #8 build f20dcf8/2aaade6 — the first
// live session AFTER FIX #8 published, in which the failure
// recurred in a new configuration).
//
// THE CAPTURE (run #434, verbatim fields): solo vs AI, enemy
// Scout Cavalry hidden under the player's Town Center roof,
// visible only as a red outline, draining the TC ~0.5 HP/s.
// Left-click LOW on the outline selected the enemy correctly
// in both matches ("Scout Cavalry — HP 45/45 — enemy unit.
// Information only"). The player then selected his OWN Scout
// Cavalry (entity 4, Aggressive) and RIGHT-clicked LOW on the
// same red outline:
//   rclick {world:[-89.21,70.72]}
//   pick {selection:[4], picked:4, afterGate:4,
//     tFromOutline:true, outline:4, outlineRect:4, records:5,
//     recordIds:[2,56,4,1,3], pickedKind:2, pickedOwner:0,
//     pickedFog:true}
//   issue {t:'garrison_tc', e:4, tc:0, ok:false}
//   issue {t:'move', e:4, x:-89.21, y:70.72}
// NO attack. Foe = enemy Scout id 56, present in recordIds.
// Differences from the proven FIX #8 live case (run #433
// addendum, Militia 68 → Spearman 78, picked 78, attack
// ok:true): here BOTH units are Scout Cavalry, and the
// ATTACKER ITSELF stands at/under the TC — its own outline
// is the winner at the click point.
//
// ------------------------------------------------------------------
// DISCRIMINATION (measured in this harness; sweep + probes)
//
// The candidate mechanisms from the assignment, one by one:
//
// (b) "foe 56 has no recorded rect that frame" — DISPROVEN by
//     the capture itself: recordIds [2,56,4,1,3] is the same
//     lastOccludedOutlines array entityAt consulted (the log
//     reads it in the same orderAt call, no draw in between).
//     The foe HELD a record. (Its rect missing the point is
//     candidate (a), separately true — below.)
// (c) "the FIX #8 branch is not reached when the outline
//     winner is the SELECTED unit / winner==selection takes a
//     different path" — DISPROVEN in World A: staged with the
//     winner being the selected unit itself (selection [4],
//     outline winner 4), a click INSIDE the foe's rect still
//     resolves 56 on the pristine FIX #8 base (probe test
//     below). entityAt contains no selection check at all;
//     the gate's only conditions are screenPick, own winner,
//     and the Ram/Transport host exemptions.
// (e) "kind-based ordering / Ram-Transport exemptions prefer
//     the own Scout (kind 2)" — DISPROVEN by the same probe:
//     kind 2 is not an exempt kind (17 / 32 are), foeRect /
//     foeOcc carry no kind ordering, and the same-kind foe
//     wins wherever its rect or outline is hit. The Ram host
//     exemption itself is re-pinned below (World C).
// (d) "foeRectAtPoint returns 56 but a later stage
//     (afterGate / orderAt) overrides back to 4" — DISPROVEN
//     by the log's shape: afterGate is the value entityAt
//     returned, passed through orderAt's fog gate, which can
//     only KEEP a target or NULL it — it can never substitute
//     a different unit. afterGate:4 with picked:4 means
//     entityAt itself returned 4. (And pickedFog:true is the
//     OWN pick's fog — the log reads the picked unit's cell.)
// (a) "foe 56's recorded rect does not contain the click
//     point" — TRUE at the capture-consistent point, and it
//     is only half the mechanism. Measured at World A's
//     capture pixel (1016,562), world (6.25,-0.75):
//       - foe rect MISSES by +0.8 px below its bottom edge
//         (the click sits at the foe's feet — the LOW click);
//       - foe outline intersection MISSES (it is a subset of
//         the rect: rect ∩ cover);
//       - foe body box MISSES: the click is screen-down from
//         the foe's ground point (dxw +0.25, dyw +0.25), so
//         spriteBodyD2's height h = -0.25 < 0 — the body box
//         only covers screen-UP from the ground point;
//       - own outline AND own rect HIT (outline:4,
//         outlineRect:4, tFromOutline:true — all reproduced);
//       - BUT the click's ground point is 0.35 wu from the
//         foe (6,-1) — INSIDE entityAt's 2.2 ground pick
//         radius, and NEARER the foe than our own Scout
//         (0.79 wu). A sweep over the staged frame (4 px grid,
//         occorder11 probe) confirms this band — ~1–5 px below
//         the foe rect's bottom edge, spanning its width — is
//         the ONLY region where the own outline wins and every
//         foe geometry misses while a foe record is present:
//         everywhere the foe's rect is hit, the pick already
//         resolves 56 on the pristine base.
//
// THE MECHANISM: the outline stage's foe preference
// (FIX #6/#7/#8) consults three foe geometries — outline
// intersection, recorded rect, static body — and all three
// miss at the foe's feet. The fourth geometry, the ground
// pick radius, already holds the documented reading for
// exactly this point (FIX #2c, occorder2: "a right-click
// pick never names our OWN unit while a non-own unit is
// also in pick range" — the ground stage returns foeBest
// over an own best), but that stage is UNREACHABLE behind
// an own outline winner: `if (occ !== null) return occ;`
// short-circuits first, and no earlier foe candidate
// consults ground distance. Pristine entityAt therefore
// returns our own Scout 4 and orderAt degrades through the
// refused garrison_tc to a ground move — the capture
// signature, reproduced verbatim in World A below.
//
// THE FIX (FIX #9, entityAt outline stage only): inside the
// same own-winner gate, after foeRectAtPoint and
// foeBodyAtPoint, consult foeGroundAtPoint() — the ground
// stage's own loop restated (Town Centers skipped, alive,
// not garrisoned, nearest non-own unit within 2.2 wu). It
// runs LAST: the measured rect and the drawn body are
// tighter geometries and keep precedence. Fog discipline is
// the ground stage's own: a target named this way is not
// tFromOutline, so orderAt's fog gate still applies to it,
// exactly as for a ground-stage pick. Raw picks,
// selectionAt, orderAt and the record-rect stage are
// untouched.
//
// CONSEQUENCE FOR AN EARLIER PIN (explicit, per the loop's
// pin discipline): occorder10's "bound A2" staged a click on
// our own outline with no foe OUTLINE/RECT/BODY hit and
// pinned the pre-fix degradation (garrison_tc → move) as
// "not a defect". Its staged pixel, measured the same way,
// is 0.79 wu from the foe — inside the ground radius — so
// under FIX #9 that pin's expectation inverts (the foe is
// resolved by ground and attacked). Run #434 is the live
// evidence occorder10's own honesty bound said would
// decide the question ("if the next live vigia still shows
// trial 1's signature, the pixel was World A2 (or a geometry
// not staged here)"): the vigia on the FIX #8 build DID
// reproduce the signature, in the configuration where the
// foe is coincident at ground level. The A2 pin is updated
// in occorder10.test.mjs with this justification recorded
// there; every other occorder10 pin is unchanged. The bound
// that survives is World B below: a foe OUTSIDE the ground
// radius (and outside every foe geometry) still does not
// shadow our own outline winner.
//
// HONESTY BOUND: the live log cannot show the click's screen
// pixel (only its ground conversion), so World A's pixel is
// the capture-CONSISTENT point that reproduces every logged
// field verbatim — not a claim about the operator's exact
// pixel. What is proven: (i) on the FIX #8 base, the
// capture's signature requires a point where outline, rect
// and body all miss the foe; (ii) in the both-Scouts
// coincident staging, the only such region with a foe
// record present is the feet band inside the foe's ground
// radius; (iii) FIX #2c's documented doctrine already
// arbitrates that region for the ground stage. If a future
// vigia still shows this signature, the remaining
// unconsulted geometry would be the record-rect stage's
// identical ground gap (occ === null, occRect own winner) —
// deliberately NOT widened here: no live capture shows
// that configuration, and this fix stays minimal.
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

function makeHarness(st, records, issued, occEntriesBundle) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = st.kinds;
  const { occLog, entries } = occEntriesBundle;
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
// World A — the capture configuration. TC 0 at the origin; our
// starting villagers 1/2/3 occluded behind it; the foe Scout
// 56 at (6,-1) under the TC; our OWN Scout 4 (the selected
// attacker) at (7,-0.5), also under the TC, FRONTMOST of the
// two (depth 6.5 > 5) — its outline wins wherever both are
// hit. Records pushed in the capture's order [2,56,4,1,3].
// Capture pixel (1016,562) — world (6.25,-0.75): 0.8 px below
// the foe rect's bottom edge, at the foe's feet.
function makeWorldA({ selKind = 2, foePos = [6, -1] } = {}) {
  const st = blankState(57);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 1, 0, 0, -3, -3, 50);
  put(st, 2, 0, 0, -5, 0, 50);
  put(st, 3, 0, 0, 0, -5, 50);
  put(st, 4, selKind, 0, 7, -0.5, selKind === 17 ? 150 : 45);
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
    assert.ok(res.occluded, `staging A: unit ${id} is occluded (a record exists)`);
    return { id, rect, cover: res.cover.map((o) => o.rect), depth };
  };
  records.push(addRecord(2, VIL_RECT));
  records.push(addRecord(56, TALL_RECT));
  records.push(addRecord(4, TALL_RECT));
  records.push(addRecord(1, VIL_RECT));
  records.push(addRecord(3, VIL_RECT));
  const capturePixel = [1016, 562];
  const captureWorld = fns.s2w(...capturePixel);
  return { ...fns, issued, records, occEntries, capturePixel, captureWorld, st };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #9 wiring: the ground extension lives in entityAt\'s outline stage only', () => {
  assert.match(entityAtSrc, /FIX #9 \(bot-loop occorder11/,
    'the fix is marked in entityAt');
  assert.match(entityAtSrc, /const foeGroundAtPoint = \(\) =>/,
    'the foe-ground candidate is defined in entityAt');
  assert.equal((entityAtSrc.match(/foeGroundAtPoint\(\)/g) || []).length, 3,
    'the outline stage consults it once (FIX #9); occorder12 (FIX #10) widened the count to 3 — ' +
    'the record-rect and body stages now consult it too, on run #437\'s live garrison capture, ' +
    'the evidence this suite\'s honesty bound said would decide the record-rect gap');
  assert.match(entityAtSrc, /if \(occ !== null\) return occ;/,
    'the outline return line is preserved (earlier wiring pins index it)');
  assert.match(entityAtSrc, /if \(occRect !== null\) return occRect;/,
    'the record-rect return line is preserved');
  assert.equal((entityAtSrc.match(/foeRectAtPoint\(\)/g) || []).length, 1,
    'FIX #8\'s rect candidate is still consulted exactly once (occorder10 pin)');
  assert.equal((entityAtSrc.match(/foeBodyAtPoint\(\)/g) || []).length, 2,
    'FIX #7\'s body candidate is still consulted at exactly its two stages (occorder8 pin)');
  assert.ok(!/FIX #9|foeGroundAtPoint/.test(occSrc),
    'the raw outline/record-rect picks carry no foe preference (selection parity)');
});

// ------------------------------------------------------------------
// Staging: the raw layer and every foe geometry at the capture
// pixel, measured separately (passes before and after the fix —
// the layer did not move). This is the discrimination itself.

test('capture staging: raw picks match the live panel; all three foe geometries miss; the foe is in ground range', () => {
  const w = makeWorldA({});
  const [wx, wy] = w.captureWorld;
  assert.deepEqual(w.records.map((r) => r.id), [2, 56, 4, 1, 3],
    'the frame\'s records are the capture\'s recordIds, in order — the foe holds one (candidate (b) disproven)');
  assert.equal(w.occludedUnitAt(wx, wy), 4,
    'the raw outline pick names our own Scout — the selected attacker itself (the capture\'s outline:4)');
  assert.equal(w.occludedRectUnitAt(wx, wy), 4,
    'the raw record-rect pick names it too — frontmost rect at the point (outlineRect:4)');
  assert.equal(w.tcAt(wx, wy), 0,
    'the click\'s ground point lies inside our own TC footprint (tc:0 in the capture)');
  const [sx, sy] = w.capturePixel;
  const foeRec = w.records.find((r) => r.id === 56);
  // (a) the foe's recorded rect misses — by +0.8 px BELOW its
  // bottom edge, inside its x-span: the LOW click at its feet.
  assert.ok(sx >= foeRec.rect.x && sx < foeRec.rect.x + foeRec.rect.w,
    'the click is inside the foe rect\'s x-span');
  assert.ok(sy >= foeRec.rect.y + foeRec.rect.h
    && sy - (foeRec.rect.y + foeRec.rect.h) < 2,
    `the click is just below the foe rect's bottom edge (measured +${(sy - (foeRec.rect.y + foeRec.rect.h)).toFixed(1)} px) — FIX #8's foeRectAtPoint finds nothing`);
  assert.equal(occludedPickAt(w.records.filter((r) => r.id === 56), sx, sy), null,
    'the foe\'s outline intersection (rect ∩ cover, a subset of the rect) misses too — FIX #6\'s foeOcc finds nothing');
  assert.equal(w.spriteBodyD2(w.st, 56, wx, wy), null,
    'and the foe\'s static body box misses: the click is screen-down from its ground point (h = -0.25 < 0) — FIX #7\'s foeBody finds nothing');
  // The fourth geometry — the one no earlier foe candidate
  // consulted at this stage:
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  const dOwn = Math.hypot(w.st.pos[4 * 2] - wx, w.st.pos[4 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 0.35) < 0.01 && Math.abs(dOwn - 0.79) < 0.01,
    `measured ground distances: foe ${dFoe.toFixed(2)} wu, own Scout ${dOwn.toFixed(2)} wu`);
  assert.ok(dFoe < 2.2 && dFoe < dOwn,
    'the foe is inside entityAt\'s 2.2 ground pick radius and NEARER than our own Scout — FIX #2c\'s ground stage would name it, but that stage is unreachable behind the own outline winner');
});

// ------------------------------------------------------------------
// Candidates (c) and (e), disproven on the pristine base's own
// terms: in this same world (winner = the selected unit, foe
// the same kind), a point inside the foe's rect already
// resolves the foe — the FIX #8 gate IS reached and kind 2 is
// not exempt. (Passes before and after the fix.)

test('discrimination probe: inside the foe rect, the same-kind foe already wins over the selected own winner', () => {
  const w = makeWorldA({});
  // A pixel inside the foe's rect ∩ cover, also inside our own
  // Scout's outline (the raw winner is ours there too):
  const [wx, wy] = w.s2w(1018, 545);
  assert.equal(w.occludedUnitAt(wx, wy), 4,
    'staging: the raw outline winner at this pixel is still our own selected Scout');
  assert.equal(w.entityAt(wx, wy, true), 56,
    'yet entityAt resolves the foe — the FIX #6/#8 gate is reached with winner == selection, and Scout kind 2 carries no exemption');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'the attack issues — the failure is specific to the feet band, not to this configuration in general');
});

// ------------------------------------------------------------------
// Post-fix behaviour on the capture (fails on pristine:
// pristine emits picked:4 -> garrison_tc ok:false -> move,
// verbatim the live signature).

test('capture post-fix: the right-click issues Attack on the ground-resolved enemy (FIX #9)', () => {
  const w = makeWorldA({});
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the enemy by its ground proximity, not our own frontmost Scout');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'exactly one command: {t:\'attack\', e:4, tg:56} — no garrison_tc, no move');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick, 'occLog recorded the pick resolution (instrument intact)');
  assert.deepEqual(pick.selection, [4]);
  assert.equal(pick.picked, 56, 'the pick now resolves to the enemy');
  assert.equal(pick.afterGate, 56,
    'the fog gate keeps it (its cell is visible) — candidate (d) disproven in shape: the gate keeps or nulls, it never substitutes our own unit');
  assert.equal(pick.tFromOutline, false,
    'the target was NOT named by the raw outline pick — orderAt\'s fog gate therefore applied to it, as it does to a ground-stage pick');
  assert.equal(pick.outline, 4, 'the RAW outline pick still names our Scout — picked != outline proves the preference fired');
  assert.equal(pick.outlineRect, 4, 'the RAW record-rect pick still names the frontmost rect — the foe is resolved inside entityAt, not by moving the raw picks');
  assert.deepEqual(pick.recordIds, [2, 56, 4, 1, 3]);
  assert.equal(pick.pickedKind, 2, 'Scout Cavalry — the foe is the attacker\'s own kind');
  assert.equal(pick.pickedOwner, 2, 'the foe seat — contrast the capture\'s pickedOwner:0');
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.cmd.tg, e.ok]), [['attack', 56, true]],
    'the issue log shows the single accepted attack');
});

// ------------------------------------------------------------------
// The surviving bound: a foe outside EVERY geometry — ground
// radius included — does not shadow our own outline winner
// (passes before and after the fix — FIX #9 must not widen
// past FIX #2c's own radius).
//
// AMENDMENT (occorder12, 2026-11-19): the honesty bound below
// left the record-rect stage's identical ground gap unwidened
// for lack of a live capture. Run #437 supplied it — a FOOT
// attacker (Militia) GARRISONED into the TC on the second
// click — and occorder12's sweep measured the gap at BOTH the
// record-rect and the body stages. FIX #10 widens the ground
// consultation to those two stages; this suite's wiring pin
// count for foeGroundAtPoint() therefore moves 1 -> 3 (in
// place, above). Every other pin in this suite is unchanged.

test('bound B: a foe outside the ground radius still does not shadow our own outline winner', () => {
  const w = makeWorldA({ foePos: [-6, -1] });
  const [wx, wy] = w.captureWorld;
  assert.equal(w.occludedUnitAt(wx, wy), 4, 'staging: only our Scout\'s outline is hit');
  assert.equal(w.occludedRectUnitAt(wx, wy), 4, 'staging: only our Scout\'s rect is hit');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(dFoe > 2.2, `staging: the foe is ${dFoe.toFixed(2)} wu away — outside the ground radius`);
  assert.equal(w.entityAt(wx, wy, true), 4,
    'with no foe outline, rect, body or ground hit at the point, the preference does not fire');
  w.orderAt(wx, wy, false, true);
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  // occorder18 (FIX #15) supersedes the order half of this
  // bound: the pick still stands (our Scout 4, no foe
  // preference), but an own-unit pick no longer reaches the
  // TC garrison branch — the click is simply the ground move.
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.ok]), [['move', true]],
    'occorder18: no garrison_tc is attempted on an own-unit pick — the ground move only');
});

test('host exemption: an own Battering Ram outline winner keeps its win over a ground-resolved foe', () => {
  const w = makeWorldA({ selKind: 17 });
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 4,
    'the own Ram at the point is a garrison target, never shadowed by the foe preference — ground candidate included');
});
