// Occluded-outline ORDER degradation, round 14 (bot-loop
// occorder14, 2026-11-20; follow-up to run #440, the directed
// live vigia on build 776b649 — FIX #11 live and served-
// verified, whose SE-pillar test had PASSED live in run #439
// but whose CENTRE geometry was never manually tested until
// #440).
//
// THE CAPTURE (run #440, live, solo vs AI·Normal, mouse-only):
// an enemy unit stood at the CENTRE of the player's Town
// Center (~0.5 tile east of centre, ~1 tile north of the
// south edge; red figure + HP bar visible through the roof),
// draining the TC 2400 → 2047, then held. The player's
// Aggressive Militia stood ~1 tile south of the TC centre
// (at the south edge), ~0.5–1 tile SW of the enemy —
// centre-distance between the units ~2–6 wu, far inside LoS.
//   TEST A (manual order): right-click LOW at the bottom
//     tip/feet of the red outline → "Garrisoned." flash, the
//     Militia vanished into the TC ("Garrisoned: 1/15", pop
//     10 → 9), enemy HP unchanged — the run #438 signature
//     recurring on the FIX #11 build.
//   TEST B (selection): left-click LOW at the outline's feet
//     selected the player's own Scout standing in front /
//     south of the TC — not the enemy.
//
// ------------------------------------------------------------------
// DISCRIMINATION (measured in this harness; column scans +
// sweeps over every centre staging on the pristine FIX #11
// build — foe at (0,0), (2,2), (4,0) and the acqprobe real
// AI settle spot (0.7,-2.9); attacker east of the footprint
// or at its south edge; front Villager / front Scout):
//
//   - At the foe's figure and below its feet, the pick
//     resolves the foe (outline / rect / body / ground /
//     FIX #11's arena) until the click's ground distance
//     from the foe reaches EXACTLY tcHalf (8.0): attack at
//     d = 7.74–7.78 (dy +44 px in the foe's column), then
//     garrison from d = 8.45–8.49 (dy +48 px) down to the
//     footprint's edge (d = 11.31 at the south corner for a
//     centre foe). The transition is the arena's own radius
//     in EVERY staging — no other gate can fail while a
//     garrison fires: every own-win stage (outline, record-
//     rect, body, ground) consults the arena (FIX #11), the
//     garrison branch itself proves tcAt(click) names the
//     own TC, a foe draining the TC entity is inside the
//     footprint and fog-visible (its outline record exists
//     only for a visible cell), so the click-distance cap
//     is the one remaining gate.
//   - At the SE-pillar geometry (#439's live PASS) the same
//     band is harmless: it lies on the footprint's far NW
//     sliver, while a LOW click near an edge foe is either
//     within the radius or outside the footprint (a move).
//     At the centre the band covers the footprint's whole
//     southern half — the TC art's lower region, where a
//     LOW click at the outline's bottom tip lands.
//   - Selection (Test B): selectionAt has NO arena at all
//     (FIX #11 gave it only the 2.2 ground shadow). At the
//     centre foe's feet past 2.2, with no outline winner,
//     our front Scout wins by its BODY (point S1 below,
//     foe 4.24 away) and by its unitAt ground radius
//     (point S2, foe 7.07 away) while entityAt resolves
//     the foe — the selection layer alone diverges, the
//     same shape FIX #11 closed at 2.2 for the Villager.
//
// THE FIX (FIX #12, client pick layer only):
//   * entityAt's TC-arena candidate gains the CENTRE-DISC
//     gate: a foe inside the footprint AND within 5.0 of
//     the TC centre is resolved at ANY click inside the
//     same footprint. A foe draining the TC entity stands
//     within melee reach of the centre (the engageprobe /
//     acqprobe disc, r <= 3), so 5.0 is that disc plus a
//     half-tile margin. The click-distance gate (one
//     half-extent) is kept as the second gate. The earlier
//     bounds keep their shapes because their foe fails
//     BOTH gates: the bounds' foe at (-6,-1) is 6.08 from
//     the centre (outside the disc) with clicks 8.39–13.75
//     away (outside the radius) — restaged below.
//   * selectionAt gains the same arena candidate
//     (foeTcArenaSelAtPoint), consulted only to SHADOW an
//     own winner — at the body stage when the winner is
//     our own class-2 unit, and at the unitAt fallback
//     when unitAt would name our own unit — exactly the
//     FIX #11 ground shadow's two sites, with the fog rule
//     restated (this pick has no post-pick gate). It never
//     creates a selection from nothing, and the outline
//     stage keeps the frontmost-outline doctrine.
// Raw picks (occludedUnitAt / occludedRectUnitAt), orderAt
// and unitAt are untouched; no sim change, golden hash
// untouched by construction.
//
// BOUNDS (pinned below): a foe inside the footprint but
// outside BOTH gates (the (-6,-1) west-edge foe, click on
// our front Villager's outline at d = 8.39) does not shadow
// our own outline winner — the garrison stands; a TC-
// footprint click with no enemy present still garrisons;
// an open-ground click outside the footprint keeps the
// plain move; the minimap pick keeps its pre-fix shape;
// an own Ram / Transport Ship at the point keeps its host
// win; a fog-hidden centre foe names nothing in the arena.
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
// The #440 capture scene. TC 0 at the origin; foe Scout 56
// at foePos (the centre class: (2,2) by default — within
// the 5.0 centre disc, 2.83 from the centre); our own Scout
// 1 in front/south at scoutPos (default (6,6), the Test B
// bystander); villagers 2/3 behind the TC; the selected
// Militia 4 east of the footprint at (9,-1).
//   Point C1 (960,604) — world (8,8), the footprint's south
//     corner: foe (2,2) is 8.49 away — past the FIX #11
//     arena radius, inside the footprint, on the TC art's
//     bottom tip. On pristine: entityAt null → garrison.
//   Point C2 — world (6,6) against the acqprobe settle foe
//     (0.7,-2.9): foe 10.36 away, inside the footprint.
//   Point S1 — world (5,5): selection by our Scout's body,
//     the foe 4.24 away (past the 2.2 ground shadow).
//   Point S2 — world (7,7): selection by our Scout's
//     unitAt ground radius, the foe 7.07 away.
function makeWorld({ foePos = [2, 2], scoutPos = [6, 6], frontVil = false, selKind = 3, selPos = [9, -1], fogHidesFoe = false, foeDead = false } = {}) {
  makeHarness.fogHidesFoe = fogHidesFoe;
  const st = blankState(57);
  put(st, 0, 13, 0, 0, 0, 2400);
  put(st, 1, frontVil ? 0 : 2, 0, scoutPos[0], scoutPos[1], 50);
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
  for (const [id, box] of [[2, VIL_RECT], [56, TALL_RECT], [1, frontVil ? VIL_RECT : TALL_RECT], [3, VIL_RECT], [4, MIL_RECT]]) {
    const r = addRecord(id, box);
    if (r) records.push(r);
  }
  const pt = (sx, sy) => ({ screen: [sx, sy], world: fns.s2w(sx, sy) });
  return {
    ...fns, issued, records, occEntries, st, tcCover,
    pointC1: pt(960, 604),
    pointO: pt(960, 555),
  };
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #12 wiring: the centre-disc gate lives in the arena candidates; earlier counts unchanged', () => {
  assert.match(entityAtSrc, /FIX #12 \(bot-loop occorder14/,
    'the fix is marked in entityAt');
  assert.equal((entityAtSrc.match(/foeTcArenaAtPoint\(\)/g) || []).length, 4,
    'the arena is still consulted at the three own-win gates and at the ground stage (occorder13 pin)');
  assert.equal((entityAtSrc.match(/foeGroundAtPoint\(\)/g) || []).length, 3,
    'FIX #9/#10\'s ground consultations are unchanged (occorder11/12 pins)');
  assert.match(entityAtSrc, /c2 > 5\.0 \* 5\.0/,
    'the centre-disc gate (foe within 5.0 of the TC centre) is in the arena candidate');
  assert.match(selectionAtSrc, /FIX #12 \(bot-loop occorder14/,
    'the selection half of the fix is marked in selectionAt');
  assert.match(selectionAtSrc, /const foeTcArenaSelAtPoint = \(\) =>/,
    'the selection TC-arena candidate is defined in selectionAt');
  assert.equal((selectionAtSrc.match(/foeTcArenaSelAtPoint\(\)/g) || []).length, 2,
    'the selection arena is consulted at the body stage and at the unitAt fallback — the ground shadow\'s two sites');
  assert.ok(!/foeTcArenaSelAtPoint|foeTcArenaAtPoint/.test(occSrc),
    'the raw outline/record-rect picks carry no foe preference (selection parity)');
});

// ------------------------------------------------------------------
// Capture C1: the south-corner LOW click against a centre
// foe — the band the scans measured (fails on pristine:
// entityAt null, orderAt issues garrison_tc — run #440's
// signature verbatim).

test('capture C1 post-fix: a LOW click at the TC art bottom tip inside the footprint attacks the centre foe (FIX #12, centre disc)', () => {
  const w = makeWorld({});
  const [wx, wy] = w.pointC1.world;
  assert.deepEqual([wx, wy], [8, 8], 'staging: the point is the footprint\'s south corner');
  assert.equal(w.tcAt(wx, wy), 0,
    'staging: the click\'s ground point lies inside our own TC footprint — the garrison reinterpretation is armed');
  assert.equal(w.occludedUnitAt(wx, wy), null, 'staging: no outline winner at the art\'s bottom tip');
  assert.equal(w.occludedRectUnitAt(wx, wy), null, 'staging: no record-rect winner either');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 8.49) < 0.01 && dFoe > 8,
    `staging: the foe is ${dFoe.toFixed(2)} wu from the click — past FIX #11's arena radius (the measured band)`);
  const cFoe = Math.hypot(w.st.pos[56 * 2], w.st.pos[56 * 2 + 1]);
  assert.ok(Math.abs(cFoe - 2.83) < 0.01 && cFoe <= 5.0,
    `staging: the foe stands ${cFoe.toFixed(2)} wu from the TC centre — inside the centre disc (a TC-draining foe's stand)`);
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the enemy by the TC arena\'s centre-disc gate, not nothing');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'exactly one command: {t:\'attack\', e:4, tg:56} — no garrison_tc, no move');
});

test('capture C2 post-fix: the same band against the acqprobe settle foe (0.7,-2.9) attacks', () => {
  const w = makeWorld({ foePos: [0.7, -2.9] });
  const [wx, wy] = [6, 6];
  assert.equal(w.tcAt(wx, wy), 0, 'staging: the point is inside our own TC footprint');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 10.36) < 0.05 && dFoe > 8,
    `staging: the foe is ${dFoe.toFixed(2)} wu from the click — deep in the measured band`);
  const cFoe = Math.hypot(w.st.pos[56 * 2], w.st.pos[56 * 2 + 1]);
  assert.ok(cFoe <= 5.0, `staging: the settled foe is ${cFoe.toFixed(2)} wu from the centre — inside the disc`);
  assert.equal(w.entityAt(wx, wy, true), 56,
    'entityAt resolves the settled centre foe by the centre-disc gate');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 4, 56]],
    'exactly one command: {t:\'attack\', e:4, tg:56} — no garrison_tc, no move');
});

// ------------------------------------------------------------------
// Selection S1 / S2: run #440's Test B (both fail on
// pristine: selectionAt returns our Scout 1).

test('selection S1 post-fix: a LOW click at the centre foe\'s feet on our Scout\'s body selects the foe (FIX #12, body stage)', () => {
  const w = makeWorld({});
  const [wx, wy] = [5, 5];
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: no outline winner at this point — the selection is decided by the body stage');
  assert.equal(w.spriteBodyD2(w.st, 56, wx, wy), null,
    'staging: the foe\'s body misses (the click is screen-down from its ground point)');
  assert.ok(w.spriteBodyD2(w.st, 1, wx, wy) !== null,
    'staging: our front Scout\'s body IS hit — the stage that named it on pristine');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 4.24) < 0.01 && dFoe > 2.2,
    `staging: the foe is ${dFoe.toFixed(2)} wu away — past the 2.2 ground shadow selectionAt had`);
  assert.equal(w.selectionAt(wx, wy), 56,
    'selectionAt now resolves the foe by the TC arena over our Scout\'s body win');
});

test('selection S2 post-fix: a LOW click at the centre foe\'s feet in our Scout\'s ground radius selects the foe (FIX #12, ground fallback)', () => {
  const w = makeWorld({});
  const [wx, wy] = [7, 7];
  assert.equal(w.occludedUnitAt(wx, wy), null,
    'staging: no outline winner at this point');
  assert.equal(w.spriteBodyD2(w.st, 1, wx, wy), null,
    'staging: our Scout\'s body misses too — the selection is decided by the unitAt ground fallback');
  assert.equal(w.unitAt(wx, wy), 1,
    'staging: the raw own-only fallback names our Scout (its ground radius) — and unitAt itself is unchanged by the fix');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  assert.ok(Math.abs(dFoe - 7.07) < 0.01 && dFoe > 2.2,
    `staging: the foe is ${dFoe.toFixed(2)} wu away — past the ground shadow, inside the arena`);
  assert.equal(w.selectionAt(wx, wy), 56,
    'selectionAt shadows the own ground fallback with the arena-resolved foe');
});

// ------------------------------------------------------------------
// Sweep: over the staged centre frame, EVERY inside-footprint
// click now resolves the centre foe (fails on pristine in
// the d >= 8 band).

test('sweep (order): every inside-footprint click resolves the centre foe', () => {
  const w = makeWorld({});
  const gaps = [];
  for (let sx = 896; sx <= 1024; sx += 2) {
    for (let sy = 476; sy <= 604; sy += 2) {
      const [wx, wy] = w.s2w(sx, sy);
      if (w.tcAt(wx, wy) !== 0) continue;
      const ent = w.entityAt(wx, wy, true);
      if (ent !== 56) gaps.push([sx, sy, ent]);
    }
  }
  assert.deepEqual(gaps, [],
    'inside the footprint, with a foe in the centre disc, the foe is always the pick — never our own unit, never nothing');
});

// ------------------------------------------------------------------
// Bounds: both gates closed — the arena does not fire.

test('bound: a west-edge foe outside both gates does not shadow our outline winner — the garrison stands (occorder13 bound, restaged)', () => {
  const w = makeWorld({ foePos: [-6, -1], frontVil: true, scoutPos: [2, 2] });
  const [wx, wy] = w.pointO.world;
  assert.equal(w.occludedUnitAt(wx, wy), 1, 'staging: only our Villager\'s outline is hit');
  const dFoe = Math.hypot(w.st.pos[56 * 2] - wx, w.st.pos[56 * 2 + 1] - wy);
  const cFoe = Math.hypot(w.st.pos[56 * 2], w.st.pos[56 * 2 + 1]);
  assert.ok(Math.abs(dFoe - 8.39) < 0.01 && dFoe > 8 && cFoe > 5.0,
    `staging: the foe is ${dFoe.toFixed(2)} wu from the click and ${cFoe.toFixed(2)} from the centre — outside BOTH gates`);
  assert.equal(w.entityAt(wx, wy, true), 1,
    'with no foe outline, rect, body, ground or arena hit at the point, the preference does not fire');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 4, 0]],
    'legitimate garrison preserved: the foot Militia enters its own TC exactly as before');
});

test('bound: a TC-footprint click with no enemy present still garrisons a foot unit', () => {
  const w = makeWorld({ foeDead: true });
  const [wx, wy] = w.pointC1.world;
  assert.equal(w.tcAt(wx, wy), 0, 'staging: the point is inside the footprint');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 4, 0]],
    'a foot unit garrisons into its own TC as before — the arena needs a living foe inside the footprint');
});

test('bound: an open-ground click outside the footprint keeps the plain move', () => {
  const w = makeWorld({});
  const [wx, wy] = w.s2w(1020, 575); // south-east of the TC art, on open ground
  assert.equal(w.tcAt(wx, wy), null, 'staging: the point is outside the TC footprint');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'no enemy resolved and no building named: a ground move, exactly as before — the arena is footprint-scoped');
});

test('bound: the minimap pick is untouched — no arena, no record stages (SEL-16)', () => {
  const w = makeWorld({});
  const [wx, wy] = w.pointC1.world;
  assert.equal(w.entityAt(wx, wy, false), null,
    'with screenPick off, the pick is the plain ground pick (nothing within 2.2 of the corner) — the arena never consults');
  w.orderAt(wx, wy, false, false);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 4, 0]],
    'the minimap order at the same world point keeps its pre-fix shape');
});

test('host exemptions: an own Transport Ship / Battering Ram in front of the foe keeps its win over the arena', () => {
  const wt = makeWorld({ selKind: 32, selPos: [2, 2], foePos: [0, 0], scoutPos: [-3, -3] });
  const [tx, ty] = wt.pointO.world;
  assert.equal(wt.occludedUnitAt(tx, ty), 4,
    'staging: the Transport\'s outline wins the point (it stands in front of the foe, where the Villager stood in variant B)');
  assert.equal(wt.entityAt(tx, ty, true), 4,
    'the own Transport (a load host) at the point is never shadowed by the arena');
  const wr = makeWorld({ selKind: 17, selPos: [2, 2], foePos: [0, 0], scoutPos: [-3, -3] });
  const [rx, ry] = wr.pointO.world;
  assert.equal(wr.entityAt(rx, ry, true), 4,
    'the own Battering Ram (a garrison host) at the point is never shadowed by the arena');
});

test('fog discipline: a centre foe whose cell is not visible names nothing in the arena or the selection shadow', () => {
  const w = makeWorld({ fogHidesFoe: true });
  const [wx, wy] = w.pointC1.world;
  assert.equal(w.entityAt(wx, wy, true), null,
    'the foe\'s cell reads fog-hidden, so the arena names nothing at the corner — the click falls through as before');
  const [sx1, sy1] = [5, 5];
  assert.equal(w.selectionAt(sx1, sy1), 1,
    'the selection arena shadow carries the same fog rule — a fog-hidden foe does not shadow our Scout');
});
