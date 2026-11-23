// Occluded-outline ORDER degradation, round 18 (bot-loop
// occorder18, 2026-11-20; follow-up to run #445, the directed
// live vigia on build f41c65b, solo vs AI, client ?occlog=1).
//
// THE CAPTURE (run #445, live, verbatim shape): the player
// right-clicked LOW at the feet of an alleged enemy outline
// under his own Town Center roof. The occlog recorded:
//   pick {picked:72, afterGate:72}
//   issue {t:'garrison_tc', e:73, tc:0} ok:true
// Entity 72 is an OWN unit standing in the TC footprint;
// entity 73 is the selected own Militia. The click RESOLVED
// to a unit — our own — yet the selected Militia GARRISONED
// into the TC instead of interacting with / moving to the
// named unit. The TC was left undefended by its defender
// entering it — run #427 attempt 2's shape, one pick class
// further down: there the click named the ENEMY and FIX #5's
// enemy guard stopped the reinterpretation; here the click
// names our OWN unit, which no guard covers.
//
// DISCRIMINATION (static trace of orderAt on f41c65b):
//  * tcT = tcAt(wx, wy) tests only whether the click POINT
//    lies inside the TC footprint — never what the pick
//    resolved to.
//  * The attack block fires only for Gaia / foe / sheep
//    targets; an own-unit target falls through it.
//  * The Ram / Transport / Heal blocks reuse an own-unit
//    click legitimately and run above; none applies to an
//    ordinary own foot unit with a Militia selected.
//  * FIX #5's enemy guard tests foeOwner() only — an
//    own-owner pick bypasses it by construction.
//  * The TC garrison branch then tests tcT alone:
//      if (tcT != null && st.owners[tcT] === localOwner)
//    so ANY click inside the footprint whose pick survived
//    to this line — an own unit included — is swallowed as
//    a garrison order for the whole selection.
// In AoE2 DE, garrison is ordered by right-clicking the
// BUILDING itself with eligible units selected; a right-click
// that resolves to a unit orders interaction with that unit
// (or, for an ordinary own unit, a move to its position) —
// never a garrison.
//
// THE FIX (FIX #15, client order layer only): the TC
// garrison branch is gated on the pick NOT naming a unit —
// it fires only when tgt is null (the click named no unit:
// entityAt skips Town Centers, so a click on the TC itself
// resolves to no unit) or when the pick IS the TC entity
// itself. A click that resolved to any other unit — own,
// in this round — falls through to the ordinary readings
// below (drop-off for a loaded villager, then the ground
// move). The building garrison branch (towers / Castle),
// the drop-off branch's own tcT reading, orderAt's other
// branches, the sim and the golden hash are untouched.
//
// SUPERSEDED BOUNDS: occorder12's bound R and occorder13's
// outline bound pinned "the garrison stands" for clicks that
// resolved to an own unit inside the footprint with no enemy
// resolved at the point. Run #445's capture is exactly that
// shape with no enemy present at all, and the player-visible
// outcome is the defect itself — those two bounds' garrison
// expectation is superseded by this round's DE reading and
// they are updated to the fall-through (a ground move).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { occludedPickAt } from './occlusion.js';

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

const CS = 4, HALF = 60, GW = 30, GH = 30;
// The capture's cast, ids kept: TC 0 at the origin; the own
// bystander 72 (a Militia) standing inside the footprint at
// (2,2); the selected own Militia 73 off to the south-west;
// the enemy Scout 7 at the east porch (7,-1) for pin (c).
const OWN_POS = [2, 2];
const ENEMY_POS = [7, -1];

function makeWorld({ withOwn = true, withEnemy = false } = {}) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, 0, 0, 2400);
  if (withOwn) put(72, 3, 0, OWN_POS[0], OWN_POS[1], 40);
  put(73, 3, 0, -17.4, 17.4, 40);
  if (withEnemy) put(7, 2, 2, ENEMY_POS[0], ENEMY_POS[1], 45);
  const count = 74;
  const st = { kinds, owners, pos, hp, count, garrisoned: [] };
  const issued = [];
  const attempts = [];
  const env = {
    camS, camOx, camOy, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st,
    lastOccludedOutlines: [],
    occludedPickAt,
    cellVisibleW: () => true,
    cellExploredW: () => true,
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => [73],
    selectedAnimal: null,
    selectedBuilding: null,
    // Faithful sim verdicts for this scene: attacks and foot
    // garrisons are accepted (the capture's garrison was
    // ok:true — a Militia is garrison-eligible), moves too.
    issue: (cmd) => {
      attempts.push(cmd);
      issued.push(cmd);
      return true;
    },
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
    sim: { entity_kinds: () => kinds, positions: () => pos },
    OBST: new Array(GW * GH).fill(0),
    tcHalf: () => 8,
    KIND_RAM: 17, KIND_TC: 13, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${occSrc}\n${entityAtSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, tcAt };`,
  );
  const fns = factory(...Object.values(env));
  return { ...fns, issued, attempts, st };
}

// ------------------------------------------------------------------
// Wiring pin (fails on pristine).

test('FIX #15 wiring: the TC garrison branch is gated on the pick naming no unit (or the TC itself)', () => {
  assert.match(orderAtSrc,
    /if \(tcT != null && st\.owners\[tcT\] === localOwner && \(tgt == null \|\| tgt === tcT \|\| targets\.includes\(tgt\)\)\) \{/,
    'the garrison branch tests what the pick resolved to, not only where the point lies (FIX #24 adds the selection-member disjunct — this round\'s bystander 72 is not in the selection, so every behavioural pin below is unchanged)');
  const gate = orderAtSrc.indexOf('tgt === tcT');
  const garrison = orderAtSrc.indexOf("t: 'garrison_tc'");
  assert.ok(gate >= 0 && gate < garrison,
    'the gate sits on the TC garrison branch itself');
});

// ------------------------------------------------------------------
// Behavioural pins.

// (a) The capture, verbatim shape (fails on pristine: pristine
// issues exactly {t:'garrison_tc', e:73, tc:0}).
test('(a) a right-click resolving to an OWN unit inside the TC footprint does NOT garrison', () => {
  const w = makeWorld({});
  assert.equal(w.tcAt(OWN_POS[0], OWN_POS[1]), 0,
    'staging: the click point lies inside our own TC footprint');
  assert.equal(w.entityAt(OWN_POS[0], OWN_POS[1], true), 72,
    'staging: the pick resolves to our own bystander 72 — the capture\'s picked:72');
  w.orderAt(OWN_POS[0], OWN_POS[1], false, true);
  assert.ok(!w.attempts.some((c) => c.t === 'garrison_tc'),
    'no garrison_tc is even attempted (pristine issued it, ok:true)');
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'the click falls through to the ordinary ground move toward the named unit\'s position');
  assert.equal(w.issued[0].e, 73, 'the move comes from the selected Militia');
});

// (b) The intended garrison gesture survives: a click on the
// TC itself — no unit named (entityAt skips Town Centers) —
// with a garrison-eligible unit selected still garrisons.
test('(b) a right-click on the TC itself (no unit picked) still garrisons', () => {
  const w = makeWorld({});
  assert.equal(w.entityAt(-4, 4, true), null,
    'staging: deep inside the footprint, away from every unit, the pick names nothing');
  assert.equal(w.tcAt(-4, 4), 0, 'staging: the point is the TC');
  w.orderAt(-4, 4, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 73, 0]],
    'the ordinary garrison gesture is untouched');
});

test('(b2) a right-click at the TC centre still garrisons', () => {
  const w = makeWorld({ withOwn: false });
  w.orderAt(0, 0, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 73, 0]],
    'the TC-centre click keeps its DE meaning');
});

// (c) The enemy reading is unchanged (FIX #5's pins stay
// green in occorder4; this is the same scene class here).
test('(c) a right-click resolving to an ENEMY inside the footprint still attacks', () => {
  const w = makeWorld({ withEnemy: true });
  assert.equal(w.entityAt(ENEMY_POS[0], ENEMY_POS[1], true), 7,
    'staging: the pick resolves to the enemy');
  w.orderAt(ENEMY_POS[0], ENEMY_POS[1], false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 73, 7]],
    'exactly one order: the attack on the enemy — no garrison, no move');
});
