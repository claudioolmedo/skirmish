// Occluded-outline ORDER degradation, round 10 (bot-loop
// occorder10, 2026-11-19; follow-up to run #433, the directed
// live session on build 2a000f1/21d7bde — the first live
// session AFTER FIX #6/#7 published, in which the failure
// recurred in two distinct signatures).
//
// TRIAL 1 (run #433, verbatim fields): solo vs AI·Normal,
// attacker = own Scout Cavalry id 4 (selected, Aggressive),
// right-click on the red outline of an enemy hidden under the
// player's Town Center roof:
//   pick {selection:[4], picked:4, afterGate:4,
//     fromOutline:true, outline:4, outlineRect:4, records:5,
//     recordIds:[2,57,4,1,3], pickedKind:2, pickedOwner:0,
//     pickedFog:true}
//   issue {t:'garrison_tc', e:4, tc:0, ok:false}
//   issue {t:'move', e:4, ...}
// NO attack. Foe = enemy Scout id 57, present in recordIds.
// Contrast run #432's successful pick (recordIds [2,79,81,3],
// picked:79, the foe Spearman): there the foe's record was
// HIT at the click pixel, so FIX #6's foe preference fired.
//
// TRIAL 2 (run #433, verbatim fields): right-click with the
// same attacker while at least five enemy Militia stood under
// the TC roof:
//   pick {selection:[4], picked:null, afterGate:null,
//     fromOutline:false, outline:null, outlineRect:null,
//     records:9, recordIds:[2,71,72,69,7,68,70,57,3],
//     pickedKind:null, pickedOwner:null, pickedFog:null}
//   issue {t:'move', e:4, ...} only. NONE of the five Militia
// (ids 71,72,69,68,70 — all present in recordIds) was picked.
//
// ------------------------------------------------------------------
// DISCRIMINATION — TRIAL 1 (proven in World A below)
//
// The pick log alone proves three of the four candidate
// geometries did NOT resolve the foe at the click point:
// outline:4 means occludedPickAt over ALL records named our
// own Scout — so the foe's rect ∩ cover did NOT contain the
// point (else FIX #6's foe-only re-pick, which runs at the
// outline stage whenever the winner is ours, returns the foe:
// occorder7's capture-A staging). pickedFog:true is the OWN
// pick's fog (the log reads the picked unit's cell), and the
// fog gate never nulls an own unit anyway. What the log
// cannot show is the foe's RECT: occludedRectUnitAt returns
// only the FRONTMOST rect winner (4, our Scout, depth 7), so
// a foe rect covering the same point is invisible in the log.
//
// World A stages exactly that hidden case, with every logged
// field reproduced verbatim on the pristine base:
//   - our Scout 4's outline AND rect are hit at the pixel
//     (outline:4, outlineRect:4, pickedKind:2, pickedOwner:0);
//   - the foe Scout 57 holds a record (recordIds [2,57,4,1,3])
//     whose recorded RECT covers the pixel — the player sees
//     the foe's figure there (FIX #5's doctrine: for a unit
//     that HAS a record, its whole recorded rect is the click
//     target) — but whose outline intersection does not: the
//     foe is occluded by a neighbouring occluder whose cover
//     rect intersects the foe's rect east of the pixel, so
//     rect ∩ cover misses the pixel while the rect hits it;
//   - the foe's drawn BODY also misses at the point: the
//     click sits at body height h = 3.0 on the foe, above the
//     Scout's static body box (UNIT_BODY_BOX[2] tops at
//     2.81) but inside its recorded tall rect (h 4.05 — the
//     DIR attack pose of occorder4, h ≈ 3.55–3.9, is exactly
//     the geometry FIX #5 built the rect pick for), so
//     FIX #7's foeBodyAtPoint finds nothing either.
// Pristine entityAt then returns occ (4) at the outline
// stage's short-circuit (`if (occ !== null) return occ;`) —
// and the record-rect stage below, the ONE stage whose foe
// preference (FIX #6, second half) consults rect geometry,
// is never reached. orderAt degrades through the refused
// garrison_tc to a ground move: the trial-1 signature.
//
// THE FIX (FIX #8, entityAt outline stage only): when the
// outline winner is our own unit (same Ram / Transport host
// exemptions as FIX #2c/#6/#7), the foe preference now also
// consults a foe RECT candidate (foeRectAtPoint — a non-own
// record whose recorded rect covers the point, frontmost
// wins, alive / not garrisoned — the record-rect stage's own
// loop, restated at the stage that can actually be reached),
// ahead of FIX #7's body candidate (the measured rect is the
// authority, as FIX #5 held). No foe resolved by outline,
// rect or body — no preference: the garrison reading and
// every earlier bound survive (World A2 below pins the
// rect-miss case: a click resolving to NO foe geometry still
// names our own unit and degrades exactly as before, before
// and after the fix).
//
// HONESTY BOUND (trial 1): the live log cannot distinguish
// World A (foe rect hit, cover missed) from World A2 (foe
// rect also missed — a click on our own Scout's outline
// only). FIX #8 closes World A; World A2 is not a client
// defect and is deliberately left unchanged. What is proven
// is the mechanism and its signature, not which of the two
// worlds the live pixel inhabited.
//
// ------------------------------------------------------------------
// DISCRIMINATION — TRIAL 2 (proven in World B below)
//
// Trial 2 needs NO fix, and the capture itself disproves two
// of its candidate mechanisms:
//   - "No outline records generated for a dense group" is
//     disproven by recordIds itself: all five Militia
//     (71,72,69,68,70) AND the foe Scout 57 hold records in
//     that frame. World B proves the stronger point: staged
//     with the capture's record set, a click on a Militia's
//     actual outline pixel picks and attacks it — the dense
//     group's records are generated AND pickable.
//   - "pickedFog gate nulling" is disproven by the log's
//     shape: picked is null BEFORE the gate (afterGate ===
//     picked === null, pickedFog === null — the log reads fog
//     only for a non-null pick, and orderAt's gate only acts
//     on a non-null target). Nothing was named, so nothing
//     could be gated away.
// World B reproduces the trial-2 signature verbatim with the
// capture's record set: the click point (on the TC roof,
// screen-up from every hidden unit) lies outside EVERY pick
// geometry for EVERY unit — no record rect, no rect ∩ cover,
// no sprite-body box (the point sits ~11 body-heights above
// the nearest Militia), and > 2.2 wu of ground distance from
// every unit — so entityAt returns null and the click is a
// ground move. The outlines the player sees are painted
// within the units' own rects ∩ the TC's cover, lower on the
// roof; a click there picks (probe below). A click that
// resolves to nothing the client draws as a unit is a miss,
// in DE as here; redirecting it to a nearby hidden foe would
// be a design invention, not a defect fix. NO code change
// for trial 2.
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
// World A — trial 1. TC 0 at the origin; our starting villagers
// 1/2/3 and our Scout 4 occluded behind it; the enemy Scout 57
// at (6.5,-1), occluded by a neighbouring occluder east of the
// click pixel (its record's cover), its recorded rect extending
// west over the pixel. Records pushed in the capture's order
// [2,57,4,1,3]. Click pixel (1016,538) — world (3.25,-3.75).
function makeWorldA({ selKind = 2 } = {}) {
  const st = blankState(58);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 1, 0, 0, -3, -3, 50);
  put(st, 2, 0, 0, -5, 0, 50);
  put(st, 3, 0, 0, 0, -5, 50);
  put(st, 4, selKind, 0, 7.5, -0.5, selKind === 17 ? 150 : 45);
  put(st, 57, 2, 2, 6.5, -1, 45);
  const issued = [];
  const records = [];
  const { fns, occEntries } = makeHarness(st, records, issued, makeOccLog('occlog=1'));
  const [tsx, tsy] = fns.worldToScreen(0, 0);
  const tcCover = { x: tsx - 8.96 * 8, y: tsy - 13.52 * 8, w: 17.92 * 8, h: 17.92 * 8 };
  const eastCover = { x: 1028, y: 500, w: 44, h: 70 };
  const addRecord = (id, box, occluderRect) => {
    const [sx, sy] = fns.worldToScreen(st.pos[id * 2], st.pos[id * 2 + 1]);
    const rect = { x: sx + box.x0 * 8, y: sy + box.y0 * 8, w: box.w * 8, h: box.h * 8 };
    const depth = st.pos[id * 2] + st.pos[id * 2 + 1];
    const res = occlusionFor({ rect, depth, fogVisible: true }, [{ rect: occluderRect, depth: 8 }]);
    assert.ok(res.occluded, `staging A: unit ${id} is occluded (a record exists)`);
    return { id, rect, cover: res.cover.map((o) => o.rect), depth };
  };
  records.push(addRecord(2, VIL_RECT, tcCover));
  records.push(addRecord(57, TALL_RECT, eastCover));
  records.push(addRecord(4, TALL_RECT, tcCover));
  records.push(addRecord(1, VIL_RECT, tcCover));
  records.push(addRecord(3, VIL_RECT, tcCover));
  const capturePixel = [1016, 538];
  const captureWorld = fns.s2w(...capturePixel);
  return { ...fns, issued, records, occEntries, capturePixel, captureWorld, st };
}

// ------------------------------------------------------------------
// World B — trial 2. TC 0 at the origin; five enemy Militia
// (68,69,70,71,72) and the enemy Scout 57 under its roof, all
// holding records; own villagers 2/3/7 likewise; the attacker
// (own Scout 4) holds NO record — exactly the capture's
// recordIds [2,71,72,69,7,68,70,57,3]. Trial-2 click pixel
// (990,470) — high on the TC roof, world (-6.875,-10.625).
function makeWorldB() {
  const st = blankState(73);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 2, 0, 0, -5, 0, 50);
  put(st, 3, 0, 0, 0, -5, 50);
  put(st, 7, 0, 0, -4, -4, 50);
  put(st, 4, 2, 0, 3, -4, 45);
  put(st, 68, 3, 2, 6, -3, 45);
  put(st, 69, 3, 2, 5, -2, 45);
  put(st, 70, 3, 2, 7, 0, 45);
  put(st, 71, 3, 2, 6, -1, 45);
  put(st, 72, 3, 2, 5, -3, 45);
  put(st, 57, 2, 2, 7, -1, 45);
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
    assert.ok(res.occluded, `staging B: unit ${id} is occluded by the TC art (a record exists)`);
    return { id, rect, cover: res.cover.map((o) => o.rect), depth };
  };
  records.push(addRecord(2, VIL_RECT));
  records.push(addRecord(71, MIL_RECT));
  records.push(addRecord(72, MIL_RECT));
  records.push(addRecord(69, MIL_RECT));
  records.push(addRecord(7, VIL_RECT));
  records.push(addRecord(68, MIL_RECT));
  records.push(addRecord(70, MIL_RECT));
  records.push(addRecord(57, TALL_RECT));
  records.push(addRecord(3, VIL_RECT));
  const capturePixel = [990, 470];
  const captureWorld = fns.s2w(...capturePixel);
  return { ...fns, issued, records, occEntries, capturePixel, captureWorld, st };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #8 wiring: the rect extension lives in entityAt\'s outline stage only', () => {
  assert.match(entityAtSrc, /FIX #8 \(bot-loop occorder10/,
    'the fix is marked in entityAt');
  assert.match(entityAtSrc, /const foeRectAtPoint = \(\) =>/,
    'the foe-rect candidate is defined in entityAt');
  assert.equal((entityAtSrc.match(/foeRectAtPoint\(\)/g) || []).length, 1,
    'only the outline stage consults it (the record-rect stage keeps its inline FIX #6 loop)');
  assert.match(entityAtSrc, /if \(occ !== null\) return occ;/,
    'the outline return line is preserved (earlier wiring pins index it)');
  assert.match(entityAtSrc, /if \(occRect !== null\) return occRect;/,
    'the record-rect return line is preserved');
  assert.equal((entityAtSrc.match(/foeBodyAtPoint\(\)/g) || []).length, 2,
    'FIX #7\'s body candidate is still consulted at exactly its two stages (occorder8 pin)');
  assert.ok(!/FIX #8|foeRectAtPoint/.test(occSrc),
    'the raw outline/record-rect picks carry no foe preference (selection parity)');
});

// ------------------------------------------------------------------
// Trial 1 staging: the raw layer matches the #433 panel
// (passes before and after the fix — the layer did not move).

test('trial 1 staging: raw picks, records and geometries match the live panel', () => {
  const w = makeWorldA({});
  const [wx, wy] = w.captureWorld;
  assert.deepEqual(w.records.map((r) => r.id), [2, 57, 4, 1, 3],
    'the frame\'s records are trial 1\'s recordIds, in order — the foe holds one');
  assert.equal(w.occludedUnitAt(wx, wy), 4,
    'the raw outline pick names our own Scout (the capture\'s outline:4)');
  assert.equal(w.occludedRectUnitAt(wx, wy), 4,
    'the raw record-rect pick names it too — frontmost rect at the point (outlineRect:4)');
  assert.equal(w.tcAt(wx, wy), 0,
    'the click\'s ground point lies inside our own TC footprint (tc:0 in the capture)');
  // The three foe geometries at the point, measured separately:
  const [sx, sy] = w.capturePixel;
  const foeRec = w.records.find((r) => r.id === 57);
  assert.ok(sx >= foeRec.rect.x && sx < foeRec.rect.x + foeRec.rect.w
    && sy >= foeRec.rect.y && sy < foeRec.rect.y + foeRec.rect.h,
    'the foe\'s recorded RECT covers the click point — its figure is drawn there (FIX #5 geometry)');
  assert.equal(occludedPickAt(w.records.filter((r) => r.id === 57), sx, sy), null,
    'but the foe\'s outline intersection (rect ∩ its own occluder\'s cover) misses the point — FIX #6\'s foeOcc finds nothing');
  assert.equal(w.spriteBodyD2(w.st, 57, wx, wy), null,
    'and the foe\'s static body box misses too: the point sits at body height 3.0, above the Scout box top (2.81), inside the recorded tall rect (4.05) — FIX #7\'s foeBody finds nothing');
  // The foe record itself is sound: on its own outline pixel
  // the raw pick names it.
  assert.equal(w.occludedUnitAt(...w.s2w(1032, 548)), 57,
    'on the foe\'s actual outline pixel (inside rect ∩ cover) the raw pick names the foe — the record is not corrupt');
});

// ------------------------------------------------------------------
// Post-fix behaviour on trial 1 (fails on pristine: pristine
// emits picked:4 -> garrison_tc ok:false -> move, verbatim the
// live signature).

test('trial 1 post-fix: the right-click issues Attack on the rect-resolved enemy (FIX #8)', () => {
  const w = makeWorldA({});
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 57,
    'entityAt resolves the enemy by its recorded rect, not our own frontmost Scout');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 57]],
    'exactly one command: {t:\'attack\', e:4, tg:57} — no garrison_tc, no move');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick, 'occLog recorded the pick resolution (instrument intact)');
  assert.deepEqual(pick.selection, [4]);
  assert.equal(pick.picked, 57, 'the pick now resolves to the enemy');
  assert.equal(pick.afterGate, 57, 'the fog gate keeps it (its cell is visible)');
  assert.equal(pick.outline, 4, 'the RAW outline pick still names our Scout — picked != outline proves the preference fired');
  assert.equal(pick.outlineRect, 4, 'the RAW record-rect pick still names the frontmost rect — the foe rect is resolved inside entityAt, not by moving the raw picks');
  assert.deepEqual(pick.recordIds, [2, 57, 4, 1, 3]);
  assert.equal(pick.pickedKind, 2, 'Scout Cavalry');
  assert.equal(pick.pickedOwner, 2, 'the foe seat — contrast trial 1\'s pickedOwner:0');
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.cmd.tg, e.ok]), [['attack', 57, true]],
    'the issue log shows the single accepted attack');
});

// ------------------------------------------------------------------
// The bound: a click resolving to NO foe geometry is unchanged
// (passes before and after the fix — FIX #8 must not widen).

test('bound A2: our own outline with no foe geometry at the point still degrades as before', () => {
  const w = makeWorldA({});
  // A pixel low on our Scout's outline, below the foe's rect:
  // only record 4's geometries are hit there.
  const [wx, wy] = w.s2w(1016, 566);
  assert.equal(w.occludedUnitAt(wx, wy), 4, 'staging: only our Scout\'s outline is hit');
  assert.equal(w.occludedRectUnitAt(wx, wy), 4, 'staging: only our Scout\'s rect is hit');
  assert.equal(w.entityAt(wx, wy, true), 4,
    'with no foe outline, rect or body hit at the point, the preference does not fire');
  w.orderAt(wx, wy, false, true);
  const iss = w.occEntries.filter((e) => e.ev === 'issue');
  assert.deepEqual(iss.map((e) => [e.cmd.t, e.ok]), [['garrison_tc', false], ['move', true]],
    'the pre-fix shape is preserved exactly when no enemy is resolved: refused garrison, then a ground move');
});

test('host exemption: an own Battering Ram outline winner keeps its win over a rect-resolved foe', () => {
  const w = makeWorldA({ selKind: 17 });
  const [wx, wy] = w.captureWorld;
  assert.equal(w.entityAt(wx, wy, true), 4,
    'the own Ram at the point is a garrison target, never shadowed by the foe preference');
});

// ------------------------------------------------------------------
// Trial 2: the signature, reproduced (passes before and after
// the fix — trial 2 is a discrimination result, not a fix).

test('trial 2 staging: the capture signature reproduces with the full dense record set', () => {
  const w = makeWorldB();
  const [wx, wy] = w.captureWorld;
  assert.deepEqual(w.records.map((r) => r.id), [2, 71, 72, 69, 7, 68, 70, 57, 3],
    'the frame\'s records are trial 2\'s recordIds, in order — all five Militia hold records');
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'no outline (rect ∩ cover) contains the click point (the capture\'s outline:null)');
  assert.equal(w.occludedRectUnitAt(wx, wy), null,
    'no recorded rect contains it either (outlineRect:null)');
  assert.equal(w.entityAt(wx, wy, true), null,
    'no body box and no ground-radius candidate either — entityAt names nothing (picked:null)');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'the click is a plain ground move — the capture\'s only issue');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.ok(pick, 'occLog recorded the pick resolution');
  assert.equal(pick.picked, null);
  assert.equal(pick.afterGate, null,
    'afterGate === picked: the fog gate changed nothing — it only acts on a non-null target, so pickedFog gate nulling is not this trial\'s mechanism');
  assert.equal(pick.pickedFog, null, 'fog is logged only for a non-null pick (the capture\'s pickedFog:null)');
  assert.deepEqual(pick.recordIds, [2, 71, 72, 69, 7, 68, 70, 57, 3]);
});

test('trial 2 probe: the dense group\'s records ARE pickable at their actual outline pixels', () => {
  const w = makeWorldB();
  // Militia 70's outline pixel: inside its rect ∩ the TC
  // cover, frontmost among the foe records hit there.
  const [wx, wy] = w.s2w(1016, 560);
  assert.equal(w.occludedUnitAt(wx, wy), 70,
    'the raw outline pick names Militia 70 at its painted outline');
  assert.equal(w.entityAt(wx, wy, true), 70);
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 70]],
    'a click on the painted outline attacks — record generation for the dense group is not the defect');
  const pick = w.occEntries.find((e) => e.ev === 'pick');
  assert.equal(pick.picked, 70);
  assert.equal(pick.afterGate, 70, 'the fog gate keeps a pick made at the outline — same world, same fog as the trial-2 click');
});
