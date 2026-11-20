// Occluded-outline ORDER degradation, round 19 (bot-loop
// occorder19, 2026-11-20; follow-up to occorder18's FIX #15
// residuals).
//
// FIX #15 (occorder18) gated the Town Center garrison branch
// on the pick: it fires only when the click names no unit (or
// the TC itself), because in the DE garrison is ordered by
// right-clicking the BUILDING itself, and a click that
// resolves to a unit is an order on that unit. The occorder18
// handoff flagged the SAME defect class in the two sibling
// branches of orderAt that still read only the click POINT:
//
//  (1) The building-garrison branch (garrison_b, towers /
//      Castle): it tests bT = buildingAt(wx, wy) — the point
//      lies inside the building's footprint — and never what
//      the pick resolved to. A right-click that resolves to
//      an OWN unit standing inside a tower footprint is
//      swallowed as a garrison of the whole selection: the
//      selected military walks into the tower instead of
//      moving to the named unit — run #445's shape, one
//      building kind over.
//  (2) The forced drop-off branch (force_dep): ownDrop tests
//      tcT / the camp bT — again the point alone. A LOADED
//      villager right-clicking an OWN unit inside the Town
//      Center footprint (or a camp footprint) drops its load
//      and returns to work instead of moving to the named
//      unit. (For the TC disjunct this is the fall-through
//      FIX #15 left behind: its gate stops the garrison, then
//      this branch re-reads the same point and drops off.)
//
// THE FIX (FIX #16 + FIX #17, client order layer only): each
// branch is gated on the pick naming no unit, in FIX #15's
// shape. The building branch admits tgt == null only:
// entityAt never names a placed building (towers, the
// Castle and the camps are not entities), so a click on the
// building itself IS the tgt == null case — FIX #15's second
// disjunct (tgt === tcT) exists only because the Town Center
// is an entity. The drop-off branch admits tgt == null or
// the TC entity itself, exactly as FIX #15's gate does. A
// click that resolved to any other unit falls through to
// the ordinary readings below (the ground move). Branches
// above that legitimately reuse an own-unit click (Ram
// garrison_u, Transport load_t, Monk heal) already returned
// and are untouched, as are FIX #5's enemy guard (an
// enemy-named click never reaches either branch) and the
// sim / golden hash.
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
const buildingListSrc = sliceFn(appJs, 'function buildingList() {', '\n// One snapshot of the entity arrays');
const buildingAtSrc = sliceFn(appJs, '  function buildingAt(wx, wy) {', '\n  // Town Center hit test');
const tcAtSrc = sliceFn(appJs, 'function tcAt(wx, wy) {', '\n  function nodeAt');
// Note: the ramBodyD2 slice spans UNIT_BODY_BOX + spriteBodyD2
// (they sit between ramBodyD2 and unitAt in app.js), so the
// body-pick helper entityAt consults is carried along.
const ramBodySrc = sliceFn(appJs, 'function ramBodyD2(st, i, wx, wy) {', '\n  function unitAt');
const occSrc = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
const entityAtSrc = sliceFn(appJs, 'function entityAt(wx, wy, screenPick = false) {', '\n  // Placed-building hit test');
const orderAtSrc = sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting');

const CS = 4, HALF = 60, GW = 30, GH = 30;

// A placed building in sim.buildings() flat form (stride 14:
// id, kind, x, y, progress, complete, cellX, cellY, w, h,
// queue, trainFrac, owner, hpFrac). w = h = 4, so buildingAt's
// half extent is size*CS/2 = 8 world units on both axes.
const flatBuilding = (id, kind, x, y) =>
  [id, kind, x, y, 1, 1, 0, 0, 4, 4, 0, 0, 0, 1];

function makeWorld({ entities, buildings = [], selected, carry = [] }) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  let count = 0;
  for (const [i, kind, owner, x, y, h] of entities) {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
    count = Math.max(count, i + 1);
  }
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
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => selected,
    selectedAnimal: null,
    selectedBuilding: null,
    // Faithful sim verdicts: every staged order is accepted
    // (as the capture's garrison was, ok:true).
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
    carryKind: new Set(carry),
    clickMarkers: { add: () => {} },
    firstGarrisonedRelic: () => null,
    sim: {
      entity_kinds: () => kinds, positions: () => pos,
      buildings: () => buildings,
    },
    OBST: new Array(GW * GH).fill(0),
    tcHalf: () => 8,
    KIND_RAM: 17, KIND_TC: 13, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${buildingListSrc}\n${buildingAtSrc}\n${tcAtSrc}\n${ramBodySrc}\n${occSrc}\n${entityAtSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, tcAt, buildingAt };`,
  );
  const fns = factory(...Object.values(env));
  return { ...fns, issued, attempts, st };
}

// The tower scene: our TC 0 at the origin (far from the
// action); tower building 3 (kind 10) centred (30,30),
// footprint 22..38 on both axes; our bystander Militia 72
// inside it at (32,32); the selected own Militia 73 away
// south-west; optionally the enemy Scout 7 at (36,30), also
// inside the footprint.
const towerWorld = ({ withEnemy = false } = {}) => makeWorld({
  entities: [
    [0, 13, 0, 0, 0, 2400],
    [72, 3, 0, 32, 32, 40],
    [73, 3, 0, -17.4, 17.4, 40],
    ...(withEnemy ? [[7, 2, 2, 36, 30, 45]] : []),
  ],
  buildings: flatBuilding(3, 10, 30, 30),
  selected: [73],
});

// The TC drop-off scene (occorder18's cast plus the loaded
// villager): TC 0 at the origin; our bystander Militia 72
// inside the footprint at (2,2); the selected own Villager
// 74 away south-west, LOADED (carryKind holds it).
const tcDropWorld = () => makeWorld({
  entities: [
    [0, 13, 0, 0, 0, 2400],
    [72, 3, 0, 2, 2, 40],
    [74, 0, 0, -17.4, 17.4, 25],
  ],
  selected: [74],
  carry: [74],
});

// The camp drop-off scene: TC 0 at the origin (out of the
// way); Lumber Camp building 4 (kind 3) centred (30,-30),
// footprint x 22..38, y -38..-22; our bystander Militia 75
// inside it at (32,-32); the selected loaded Villager 74.
const campDropWorld = () => makeWorld({
  entities: [
    [0, 13, 0, 0, 0, 2400],
    [75, 3, 0, 32, -32, 40],
    [74, 0, 0, -17.4, -17.4, 25],
  ],
  buildings: flatBuilding(4, 3, 30, -30),
  selected: [74],
  carry: [74],
});

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #16 wiring: the building-garrison branch is gated on the pick naming no unit', () => {
  assert.match(orderAtSrc,
    /\(bT\.kind === 10 \|\| bT\.kind === 25 \|\| bT\.kind === 26 \|\| bT\.kind === 15\) && tgt == null\) \{/,
    'the garrison_b branch tests what the pick resolved to, not only where the point lies');
  const gate = orderAtSrc.indexOf('tgt == null');
  const garrison = orderAtSrc.indexOf("t: 'garrison_b'");
  assert.ok(gate >= 0 && gate < garrison,
    'a pick gate sits on the building-garrison branch itself');
});

test('FIX #17 wiring: the forced drop-off branch is gated on the pick naming no unit (or the TC itself)', () => {
  assert.match(orderAtSrc,
    /if \(ownDrop && \(tgt == null \|\| tgt === tcT\)\) \{/,
    'the force_dep branch tests what the pick resolved to, not only where the point lies');
  const gate = orderAtSrc.indexOf('tgt === tcT', orderAtSrc.indexOf('const ownDrop'));
  const drop = orderAtSrc.indexOf("t: 'force_dep'");
  assert.ok(gate >= 0 && gate < drop,
    'the gate sits on the forced drop-off branch itself');
});

// ------------------------------------------------------------------
// (1) The building-garrison branch.

// (a) The defect (fails on pristine: pristine issues exactly
// {t:'garrison_b', e:73, b:3} for the own-unit pick).
test('(a) a right-click resolving to an OWN unit inside a tower footprint does NOT garrison', () => {
  const w = towerWorld();
  assert.equal(w.buildingAt(32, 32)?.id, 3,
    'staging: the click point lies inside our own tower footprint');
  assert.equal(w.entityAt(32, 32, true), 72,
    'staging: the pick resolves to our own bystander 72');
  w.orderAt(32, 32, false, true);
  assert.ok(!w.attempts.some((c) => c.t === 'garrison_b'),
    'no garrison_b is even attempted (pristine issued it)');
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'the click falls through to the ordinary ground move toward the named unit\'s position');
  assert.equal(w.issued[0].e, 73, 'the move comes from the selected Militia');
});

// (a2) The intended garrison gesture survives: a click on
// the tower itself — no unit named (entityAt names no placed
// building) — with a garrison-eligible unit selected.
test('(a2) a right-click on the tower itself (no unit picked) still garrisons', () => {
  const w = towerWorld();
  assert.equal(w.entityAt(25, 25, true), null,
    'staging: inside the footprint, away from every unit, the pick names nothing');
  assert.equal(w.buildingAt(25, 25)?.id, 3, 'staging: the point is the tower');
  w.orderAt(25, 25, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.b]), [['garrison_b', 73, 3]],
    'the ordinary building-garrison gesture is untouched');
});

// (a3) The enemy reading is unchanged: an enemy named inside
// the footprint is attacked by the block far above — the
// garrison branch is never reached, before or after.
test('(a3) a right-click resolving to an ENEMY inside the tower footprint still attacks', () => {
  const w = towerWorld({ withEnemy: true });
  assert.equal(w.entityAt(36, 30, true), 7,
    'staging: the pick resolves to the enemy');
  w.orderAt(36, 30, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 73, 7]],
    'exactly one order: the attack on the enemy — no garrison, no move');
});

// ------------------------------------------------------------------
// (2) The forced drop-off branch.

// (b) The defect, TC disjunct (fails on pristine: pristine
// issues exactly {t:'force_dep', e:74} for the own-unit pick
// — FIX #15's gate stops the garrison, then this branch
// re-reads the point and drops off).
test('(b) a loaded villager right-clicking an OWN unit inside the TC footprint does NOT drop off', () => {
  const w = tcDropWorld();
  assert.equal(w.tcAt(2, 2), 0,
    'staging: the click point lies inside our own TC footprint');
  assert.equal(w.entityAt(2, 2, true), 72,
    'staging: the pick resolves to our own bystander 72');
  w.orderAt(2, 2, false, true);
  assert.ok(!w.attempts.some((c) => c.t === 'force_dep'),
    'no force_dep is even attempted (pristine issued it)');
  assert.ok(!w.attempts.some((c) => c.t === 'garrison_tc'),
    'no garrison_tc either (FIX #15 already gates that branch)');
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'the click falls through to the ordinary ground move toward the named unit\'s position');
  assert.equal(w.issued[0].e, 74, 'the move comes from the selected loaded Villager');
});

// (b2) The intended drop-off gesture survives: a loaded
// villager clicking the TC itself (no unit named) still
// drops off — the TC garrison branch skips loaded villagers
// precisely so they land here.
test('(b2) a loaded villager right-clicking the TC itself (no unit picked) still drops off', () => {
  const w = tcDropWorld();
  assert.equal(w.entityAt(-4, 4, true), null,
    'staging: deep inside the footprint, away from every unit, the pick names nothing');
  assert.equal(w.tcAt(-4, 4), 0, 'staging: the point is the TC');
  w.orderAt(-4, 4, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e]), [['force_dep', 74]],
    'the ordinary forced drop-off gesture is untouched');
});

// (c) The defect, camp disjunct (fails on pristine the same
// way, through the camp half of ownDrop).
test('(c) a loaded villager right-clicking an OWN unit inside a camp footprint does NOT drop off', () => {
  const w = campDropWorld();
  assert.equal(w.buildingAt(32, -32)?.id, 4,
    'staging: the click point lies inside our own Lumber Camp footprint');
  assert.equal(w.entityAt(32, -32, true), 75,
    'staging: the pick resolves to our own bystander 75');
  w.orderAt(32, -32, false, true);
  assert.ok(!w.attempts.some((c) => c.t === 'force_dep'),
    'no force_dep is even attempted (pristine issued it)');
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'the click falls through to the ordinary ground move toward the named unit\'s position');
  assert.equal(w.issued[0].e, 74, 'the move comes from the selected loaded Villager');
});

// (c2) The camp drop-off gesture survives on the building
// itself (no unit named).
test('(c2) a loaded villager right-clicking the camp itself (no unit picked) still drops off', () => {
  const w = campDropWorld();
  assert.equal(w.entityAt(25, -25, true), null,
    'staging: inside the footprint, away from every unit, the pick names nothing');
  assert.equal(w.buildingAt(25, -25)?.id, 4, 'staging: the point is the camp');
  w.orderAt(25, -25, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e]), [['force_dep', 74]],
    'the ordinary camp drop-off gesture is untouched');
});
