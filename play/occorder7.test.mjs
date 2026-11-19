// Occluded-outline ORDER degradation, round 7 (bot-loop
// occorder7, 2026-11-19; follow-up to run #431, the first
// directed live session that READ the ?occlog=1 panel on the
// occorder6 build).
//
// THE CAPTURE (run #431, session 2, build fc67d06): enemy Scout
// Cavalry outline-only under the player's Town Center (east
// porch), TC draining 2400 -> 2281. Left-click SELECTED the
// enemy (45/45). Own Scout set to Aggressive; ONE right-click
// on the red outline produced verbatim:
//   {"ev":"rclick","screen":[960.004194010694,441.26229147687593],
//    "world":[-89.98570143972213,69.95127430894112],"shift":false}
//   {"ev":"pick","wx":-89.98570143972213,"wy":69.95127430894112,
//    "fromCanvas":true,"selection":[4],"picked":4,"afterGate":4,
//    "tFromOutline":true,"outline":4,"outlineRect":4,"records":5,
//    "recordIds":[2,1,56,4,3],"pickedKind":2,"pickedOwner":0,
//    "pickedFog":true}
//   {"ev":"issue","cmd":{"t":"garrison_tc","e":4,"tc":0},"ok":false}
//   {"ev":"issue","cmd":{"t":"move","e":4,
//    "x":-89.98570143972213,"y":69.95127430894112}}
// NO attack command, NO enemy-guard entry. The Scout walked to
// the point and stood idle ~115 s; the TC kept draining.
//
// THE RESOLVED ANSWER to the capture's open question (picked:4
// enemy or friendly?) — FRIENDLY, proven three ways:
//  1. pickedOwner is 0, which is localOwner in local play (the
//     foe seat is 2, Gaia is 1); pickedKind 2 is Scout Cavalry.
//     selection is [4]: picked:4 is the selected Scout ITSELF —
//     its own INF-12 record is the frontmost hit at the click
//     pixel (its painter depth exceeds the enemy's, both units
//     standing on the TC's east porch), so occludedUnitAt and
//     occludedRectUnitAt both name it. The enemy's record (56,
//     the one id in recordIds outside the player's starting
//     set 1-4) is present AND hit at the same pixel, behind it.
//     This is occorder5's bound 2 (frontmost-record over-claim
//     by an own unit), with the over-claiming unit being the
//     attacker itself.
//  2. The alternative (afterGate named the enemy, then the TC
//     garrison branch pre-empted the attack branch) is
//     impossible in orderAt as written: the attack block AND
//     the FIX #5 enemy guard both run BEFORE the garrison
//     block, so a foe-named click can never emit garrison_tc —
//     and the guard would have logged an enemy-guard entry.
//     The capture shows garrison_tc and NO enemy-guard, so the
//     target at guard time was not the foe.
//  3. The exact degrading branch: with tgt naming an OWN unit,
//     no earlier branch matches (attack needs foe/Gaia/sheep;
//     heal needs wounded + Monk; Ram/Transport/relic/convert
//     miss), the enemy guard checks foe-only, and the click is
//     reinterpreted from its raw ground point: tcAt(point) is
//     the player's own TC (id 0), so orderAt's TC garrison
//     block issues {t:'garrison_tc', e:4, tc:0} — the sim
//     correctly refuses a mounted Scout (ok:false, `any` stays
//     false, no return) — and the click falls through to the
//     ground-move tail: {t:'move', e:4, x, y}.
//
// PRE-FIX REPRODUCTION (this harness's staging, run on the
// pristine base fc67d06 before the fix): the occLog entries
// were, verbatim in every discriminating field,
//   pick {selection:[4], picked:4, afterGate:4,
//     tFromOutline:true, outline:4, outlineRect:4, records:5,
//     recordIds:[2,1,56,4,3], pickedKind:2, pickedOwner:0,
//     pickedFog:true}
//   issue {cmd:{t:'garrison_tc', e:4, tc:0}, ok:false}
//   issue {cmd:{t:'move', e:4, ...}, ok:true}
// (wx/wy differ from the capture only because the harness map
// is centred on the origin; the live map's origin is offset.)
//
// THE FIX (FIX #6, entityAt only — the right-click pick):
// FIX #2c's foe preference, extended into the two record picks
// as occorder5's bound 2 anticipated. When the outline pick or
// the record-rect pick names an OWN unit (and it is not a
// Battering Ram / Transport Ship host, which keeps its win as
// in FIX #2c), a NON-OWN record also hit at the same point
// wins instead: a right-click is an order, and in DE
// right-clicking an enemy unit orders an attack on it even
// when it stands inside a friendly building's footprint; the
// garrison reading applies only when no enemy is resolved at
// the point. The raw picks (occludedUnitAt /
// occludedRectUnitAt), occlusion.js's frontmost doctrine and
// the left-click selection path are untouched — selection
// keeps naming the frontmost outline, orders now name the foe
// behind it. No sim change; the garrison_tc ok:false in the
// capture was the sim correctly refusing — the bug was what
// the client chose to emit.
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

// The capture scene: our TC (id 0) at the origin; our starting
// villagers 1/2/3 and our Scout 4 occluded behind it; the
// enemy Scout 56 at the east porch (7,-1), inside the TC
// footprint. Our Scout 4 stands at (7.5,-0.5), slightly toward
// the viewer (painter depth 7 > the enemy's 6), so ITS record
// is frontmost at the shared pixel. Records are built through
// the REAL occlusionFor against the TC art's rect and pushed
// in the capture's order [2, 1, 56, 4, 3]. `selKind` is the
// kind of the selected unit id 4; issue() is the REAL one over
// a stub applyLocalOrder with the sim's faithful verdicts for
// this scene (attack accepted; garrison_tc admits foot kinds
// only — a mounted Scout / a Ram is refused, as in the
// capture).
function makeWorld({ selKind = 2, withEnemy = true } = {}) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, 0, 0, 2400);
  put(1, 0, 0, -3, -3, 50);
  put(2, 0, 0, -5, 0, 50);
  put(3, 0, 0, 0, -5, 50);
  put(4, selKind, 0, 7.5, -0.5, selKind === 17 ? 150 : 45);
  if (withEnemy) put(56, 2, 2, 7, -1, 45);
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
    sim: { entity_kinds: () => kinds, positions: () => pos, tick_count: () => 0 },
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
    `return { orderAt, entityAt, occludedUnitAt, occludedRectUnitAt, tcAt, worldToScreen, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
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
  // The capture's record order: [2, 1, 56, 4, 3].
  records.push(addRecord(2, VIL_RECT));
  records.push(addRecord(1, VIL_RECT));
  if (withEnemy) records.push(addRecord(56, TALL_RECT));
  records.push(addRecord(4, selKind === 0 ? VIL_RECT : TALL_RECT));
  records.push(addRecord(3, VIL_RECT));
  // The capture click: the enemy anchor's screen point nudged
  // (4,-8) — inside BOTH 56's and 4's rect ∩ cover.
  const [esx, esy] = fns.worldToScreen(7, -1);
  const capturePixel = [esx + 4, esy - 8];
  const captureWorld = fns.s2w(...capturePixel);
  return { ...fns, issued, records, occEntries: entries, capturePixel, captureWorld, st };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #6 wiring: foe preference lives in entityAt record picks only', () => {
  assert.match(entityAtSrc, /FIX #6 \(bot-loop occorder7/,
    'the fix is marked in entityAt');
  assert.match(entityAtSrc, /occludedPickAt\(\s*lastOccludedOutlines\.filter/,
    'the outline stage re-picks over the non-own records at the point');
  assert.match(entityAtSrc, /st\.kinds\[occ\] !== KIND_RAM && st\.kinds\[occ\] !== 32/,
    'the outline stage keeps the Ram / Transport host exemption (FIX #2c)');
  assert.match(entityAtSrc, /st\.kinds\[occRect\] !== KIND_RAM && st\.kinds\[occRect\] !== 32/,
    'the record-rect stage keeps the same host exemption');
  // The pinned return lines of the earlier rounds keep their
  // shape (occludedpick / occorder3 / occorder4 index them).
  assert.match(entityAtSrc, /if \(occ !== null\) return occ;/,
    'the outline return line is preserved');
  assert.match(entityAtSrc, /if \(occRect !== null\) return occRect;/,
    'the record-rect return line is preserved');
  // The raw picks are untouched: no foe preference inside
  // occludedUnitAt / occludedRectUnitAt (selection parity —
  // the left click keeps the frontmost outline).
  assert.ok(!/FIX #6|foeOcc|foeRect/.test(occSrc),
    'the raw outline/record-rect picks carry no foe preference');
});

// ------------------------------------------------------------------
// Capture staging: the raw pick layer matches run #431 exactly
// (passes before and after the fix — the layer did not move).

test('capture staging: the raw picks and records match the #431 panel', () => {
  const w = makeWorld({});
  const [wx, wy] = w.captureWorld;
  assert.equal(w.occludedUnitAt(wx, wy), 4,
    'the raw outline pick names our own Scout — frontmost at the pixel (the capture\'s outline:4)');
  assert.equal(w.occludedRectUnitAt(wx, wy), 4,
    'the raw record-rect pick names it too (the capture\'s outlineRect:4)');
  assert.deepEqual(w.records.map((r) => r.id), [2, 1, 56, 4, 3],
    'the frame\'s records are the capture\'s recordIds, in order');
  assert.equal(w.tcAt(wx, wy), 0,
    'the click\'s ground point lies inside our own TC footprint (tc:0 in the capture)');
  // The enemy IS hit at the same pixel: re-picking over only
  // the non-own records names 56 — the candidate FIX #6 uses.
  const [sx, sy] = w.worldToScreen(wx, wy);
  const foeOnly = occludedPickAt(w.records.filter((r) => w.st.owners[r.id] !== 0), sx, sy);
  assert.equal(foeOnly, 56, 'the enemy record is also hit at the capture pixel, behind ours');
});

// ------------------------------------------------------------------
// Post-fix behaviour on the capture scene (fails on pristine:
// pristine emits the garrison_tc -> move signature above).

test('capture scene: the right-click now issues Attack on the enemy (FIX #6)', () => {
  const w = makeWorld({});
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the enemy at the point, not our own frontmost Scout');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'exactly one command: {t:\'attack\', e:4, tg:56} — no garrison_tc, no move');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick, 'occLog recorded the pick resolution (instrument intact)');
  assert.deepEqual(pick.selection, [4]);
  assert.equal(pick.picked, 56, 'the pick now resolves to the enemy');
  assert.equal(pick.afterGate, 56, 'the fog gate keeps it (its cell is visible)');
  assert.equal(pick.outline, 4, 'the RAW outline pick still names our Scout — picked != outline proves the preference fired');
  assert.equal(pick.outlineRect, 4);
  assert.deepEqual(pick.recordIds, [2, 1, 56, 4, 3]);
  assert.equal(pick.pickedKind, 2, 'Scout Cavalry');
  assert.equal(pick.pickedOwner, 2, 'the foe seat — contrast the capture\'s pickedOwner:0');
  assert.equal(pick.tFromOutline, false,
    'the FIX #2b exemption names the raw outline winner only; the gate passes on the cell\'s own visibility, as in the capture scene');
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.cmd.tg, e.ok]), [['attack', 56, true]],
    'the issue log shows the single accepted attack');
  assert.ok(!w.occEntries.some((e) => e.ev === 'enemy-guard'),
    'no enemy-guard entry: the attack was accepted, the guard never needed');
});

test('record-rect stage: a click inside both rects but outside the outline also attacks (FIX #6)', () => {
  const w = makeWorld({});
  // Screen point east of the TC art's rect edge: inside both
  // cavalry rects, outside every rect ∩ cover (no outline hit).
  const [esx, esy] = w.worldToScreen(7, -1);
  const [wx, wy] = w.s2w(esx + 11, esy - 14);
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: the outline pick misses this pixel');
  assert.equal(w.occludedRectUnitAt(wx, wy), 4,
    'staging: the raw record-rect pick names our own frontmost Scout');
  assert.equal(w.entityAt(wx, wy, true), 56,
    'the record-rect foe preference resolves the enemy');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]]);
});

// ------------------------------------------------------------------
// No regression: the garrison interpretation survives wherever
// NO enemy is resolved at the point.

test('no foe hit at the point: our own outline still names itself and the click degrades as before', () => {
  const w = makeWorld({});
  // A pixel low on our Scout's outline, below the enemy's
  // rect: only record 4 is hit there.
  const [esx, esy] = w.worldToScreen(7.5, -0.5);
  const [wx, wy] = w.s2w(esx - 9, esy - 1);
  assert.equal(w.occludedUnitAt(wx, wy), 4, 'staging: only our Scout\'s outline is hit');
  assert.equal(w.entityAt(wx, wy, true), 4,
    'with no non-own record hit at the point, the preference does not fire');
  w.orderAt(wx, wy, false, true);
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.ok]), [['garrison_tc', false], ['move', true]],
    'the pre-fix shape is preserved exactly when no enemy is resolved: refused garrison, then a ground move');
});

test('TC footprint with NO enemy present still garrisons a foot unit (and a Scout still degrades to move)', () => {
  const mil = makeWorld({ selKind: 3, withEnemy: false });
  mil.orderAt(-4, 4, false, true); // deep inside the footprint, away from every unit/record
  assert.deepEqual(mil.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 4, 0]],
    'a foot unit garrisons into its own TC as before');
  const sco = makeWorld({ selKind: 2, withEnemy: false });
  sco.orderAt(-4, 4, false, true);
  const iss = sco.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.ok]), [['garrison_tc', false], ['move', true]],
    'a mounted Scout is refused by the sim and walks — unchanged');
});

test('visible enemy with no records: the right-click still attacks (unchanged path)', () => {
  const w = makeWorld({ withEnemy: true });
  w.records.length = 0; // no occlusion records this frame
  // The enemy in the open: click its own ground point from far
  // outside the TC footprint by moving the click to the enemy
  // after relocating it in the staged state.
  w.st.pos[56 * 2] = 24; w.st.pos[56 * 2 + 1] = -24;
  w.orderAt(24, -24, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'the ordinary visible-enemy attack is untouched');
});

test('host exemption: an own Battering Ram record over the enemy keeps its win', () => {
  const w = makeWorld({ selKind: 17 });
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 4,
    'the own Ram at the point is a garrison target, never shadowed by the foe preference');
});
