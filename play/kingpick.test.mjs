// King garrison pick defect (bot-loop #474, 2026-11-22;
// builder kingpick).
//
// THE CAPTURE (run #474, live, clean test — Regicide, no enemy
// anywhere near): the King (entity 53, kind 22) was selected at
// 00:16 and right-clicked on his own Town Center's footprint at
// ~00:34. The occlog recorded:
//   rclick world [-95.89, 64.05]
//   pick { selection:[53], picked:53, afterGate:53,
//          tFromOutline:true, records:4, recordIds:[53,2,1,3] }
//   issue { t:'move', e:53 }
// The King walked to the TC's south edge and stayed OUTSIDE;
// the TC panel read "Ungarrison: nobody is garrisoned inside."
// Town Bell garrisoned the villagers (3/15) in the same match,
// and the sim's own pins prove the King is foot-garrisonable
// (sim/src/cls06_tests.rs: EntityKind::King.is_foot_garrisonable(),
// garrison.rs: a TownCenter accepts foot units) — the failure
// is in the client's right-click pick/issue path.
//
// ROOT CAUSE (two layers, both by-design pieces composing into
// the defect):
//  1. The TC entity can NEVER appear in the pick records.
//     lastOccludedOutlines is filled by drawOccludedOutlines
//     only with draw entries whose occ.role is 'unit' and whose
//     occ.id is set; the Town Center's draw entry is
//     occ:{ role:'occluder', ...tcOccGeo } — the TC is an
//     occluder, never an occluded unit. entityAt likewise skips
//     KIND_TC in every candidate loop: the TC is named by the
//     separate tcAt() test, never by entityAt. So recordIds
//     [53,2,1,3] — the starting units standing under/around the
//     TC art — is the complete, expected record set.
//  2. The King SPAWNS inside the footprint: the sim's
//     KING_SPAWN_OFFSET is (-5,-5) from the TC centre, inside
//     the ±tcHalf (8) footprint, behind the TC art — so he
//     holds an occlusion record, and a click LOW on his feet /
//     outline inside the footprint resolves through
//     entityAt's outline stage to the King HIMSELF
//     (picked:53, tFromOutline:true). In orderAt, FIX #15's
//     TC-garrison gate — (tgt == null || tgt === tcT) — then
//     vetoes the garrison branch: the pick named a unit, and
//     FIX #15 (occorder18, run #445) reads any unit-named
//     click as an order ON that unit. The click falls through
//     the drop-off gate (same test) to the ground move.
//     But run #445's pick named a DIFFERENT own unit (picked
//     72, selected 73): an other-unit click. Here the pick
//     names a member of the very selection receiving the
//     order — a self-pick carries no other-unit interaction
//     reading (there is no second unit to interact with), so
//     the building reading of the point, which lies inside
//     our own TC's footprint, is the only one left: in DE,
//     right-clicking your Town Center with units selected
//     garrisons them, even when the click pixel lands on one
//     of the selected units standing inside it — for the
//     Regicide King that is his spawn position, so nearly
//     every NW-footprint click names him.
//
// THE FIX (FIX #24, client order layer only): FIX #15's gate
// on the TC garrison branch gains the selection-member
// disjunct — (tgt == null || tgt === tcT ||
// targets.includes(tgt)). Every other reading is preserved
// by construction: an enemy pick still dies at FIX #5's
// enemy guard above (an enemy is never in targets), an
// other-own-unit pick (occorder18's capture) still falls
// through to the ground move, and a self-pick outside any
// footprint still moves. The building-garrison (FIX #16) and
// drop-off (FIX #17) gates keep their exact shape — the same
// self-pick shape there is recorded as a residual, not fixed
// here (minimal scope: the TC branch the capture proves).
// Sim and golden hash untouched.
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
// The capture's cast, ids kept where the log gives them:
// TC 0 at the origin; the King 53 at his sim spawn offset
// (-5,-5) from the TC centre (KING_SPAWN_OFFSET, sim/src/
// lib.rs) — inside the ±8 footprint, behind the TC art;
// villagers 1/2/3 at their start offsets, the Scout 4 at
// (5,5); the selected Militia 73 off south-west and the own
// bystander Militia 72 inside the footprint for the #445
// bound; the enemy Scout 28 at the east porch for the #473
// bound.
const KING_POS = [-5, -5];

function makeWorld({ selected = [53], withEnemy = false, kingPos = KING_POS } = {}) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 1, 0, 0, 0, 2400); // the Town Center (KIND_TC = 1)
  put(1, 0, 0, 5, 0, 50);
  put(2, 0, 0, -5, 0, 50);
  put(3, 0, 0, 0, 5, 50);
  put(4, 2, 0, 5, 5, 45);
  put(53, 22, 0, kingPos[0], kingPos[1], 75); // the King
  put(72, 3, 0, 2, 2, 40);
  put(73, 3, 0, -17.4, 17.4, 40);
  if (withEnemy) put(28, 2, 2, 7, -1, 45);
  const count = 74;
  const st = { kinds, owners, pos, hp, count, garrisoned: [] };
  const issued = [];
  const attempts = [];
  const records = [];
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
    selectedUnits: () => selected.slice(),
    selectedAnimal: null,
    selectedBuilding: null,
    // Faithful sim verdicts for this scene: the King is
    // foot-garrisonable and the TC accepts him (cls06 /
    // garrison.rs), moves and Militia attacks are accepted —
    // and the King's ATTACK is refused, as live (run #473:
    // the King has no attack; push_attack returns false).
    issue: (cmd) => {
      attempts.push(cmd);
      const ok = !(cmd.t === 'attack' && cmd.e === 53);
      if (ok) issued.push(cmd);
      return ok;
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
    KIND_RAM: 17, KIND_TC: 1, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${occSrc}\n${entityAtSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, occludedUnitAt, tcAt, worldToScreen, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy), heightAtWorld };`,
  );
  const fns = factory(...Object.values(env));
  // The frame's occlusion records, mirroring the capture's
  // recordIds [53,2,1,3]: the starting units the TC art
  // occludes. Each record's rect is the unit's alpha-tight
  // screen rect; the cover is the TC's occluder rect (the
  // tcOccGeo span, as in occorder's staging). The TC itself
  // (entity 0) holds NO record — it is the occluder.
  const tcCover = [{ x: camOx - 16 * camS, y: camOy - 10.2 * camS, w: 32 * camS, h: 18.2 * camS }];
  const addRecord = (id, x, y, x0, x1, hgt) => {
    const [sx, sy] = fns.worldToScreen(x, y);
    records.push({
      id,
      rect: { x: sx + x0 * camS, y: sy - hgt * camS, w: (x1 - x0) * camS, h: hgt * camS },
      cover: tcCover,
      depth: x + y,
    });
  };
  addRecord(53, kingPos[0], kingPos[1], -0.66, 0.95, 2.85); // King body box
  addRecord(2, -5, 0, -1.1, 1.1, 3.1);
  addRecord(1, 5, 0, -1.1, 1.1, 3.1);
  addRecord(3, 0, 5, -1.1, 1.1, 3.1);
  const eventWorld = (sx, sy) => {
    let [wx, wy] = fns.s2w(sx, sy);
    const h = fns.heightAtWorld(wx, wy);
    if (h > 0) {
      [wx, wy] = fns.s2w(sx, sy + h * 1.1 * camS);
      const h2 = fns.heightAtWorld(wx, wy);
      if (h2 !== h) [wx, wy] = fns.s2w(sx, sy + h2 * 1.1 * camS);
    }
    return [wx, wy];
  };
  return { ...fns, eventWorld, issued, attempts, records };
}

// The capture's click: LOW on the King's outline at his feet
// — his ground screen point, 2 px up. Its ground conversion
// sits just NW of the King, inside the TC footprint.
function kingClick(w) {
  const [sx, sy] = w.worldToScreen(KING_POS[0], KING_POS[1]);
  return w.eventWorld(sx, sy - 2);
}

// ------------------------------------------------------------------
// Wiring pins (fail on pristine).

test('FIX #24 wiring: the TC garrison gate admits a pick naming a member of the selection itself', () => {
  assert.match(orderAtSrc,
    /if \(tcT != null && st\.owners\[tcT\] === localOwner && \(tgt == null \|\| tgt === tcT \|\| targets\.includes\(tgt\)\)\) \{/,
    'the garrison branch tests the pick against the selection, not only against null / the TC');
  const gate = orderAtSrc.indexOf('targets.includes(tgt)');
  const garrison = orderAtSrc.indexOf("t: 'garrison_tc'");
  assert.ok(gate >= 0 && gate < garrison,
    'the disjunct sits on the TC garrison branch itself');
});

test('the Town Center is an occluder, never a pick record (the capture\'s recordIds shape)', () => {
  assert.match(appJs, /occ: \{ role: 'occluder', \.\.\.tcOccGeo\(wx, wy\) \}/,
    'the TC draw entry is an occluder entry');
  const occPass = sliceFn(appJs, 'function drawOccludedOutlines(items) {', '\nfunction draw()');
  assert.match(occPass, /if \(it\.occ\.role === 'unit'\) units\.push\(entry\);\s*else occluders\.push\(entry\);/,
    'records are built from unit entries only — an occluder can never be recorded');
  const w = makeWorld({});
  assert.deepEqual(w.records.map((r) => r.id), [53, 2, 1, 3],
    'staging mirrors the capture: recordIds [53,2,1,3], the TC (0) absent by construction');
});

// ------------------------------------------------------------------
// Behavioural pins.

// (a) The capture, verbatim shape (fails on pristine:
// pristine issues exactly {t:'move', e:53}).
test('(a) the selected King right-clicked LOW on his own outline inside the TC footprint garrisons', () => {
  const w = makeWorld({});
  const [wx, wy] = kingClick(w);
  assert.equal(w.tcAt(wx, wy), 0,
    'staging: the click point lies inside our own TC footprint (the capture\'s tcT)');
  assert.equal(w.occludedUnitAt(wx, wy), 53,
    'staging: the outline pick names the King himself (the capture\'s tFromOutline:true)');
  assert.equal(w.entityAt(wx, wy, true), 53,
    'staging: the pick resolves to the selected King — picked:53, afterGate:53');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 53, 0]],
    'exactly one order: garrison_tc for the King into his own TC — no move');
});

// (b) The group form: the pick names ONE member of a larger
// selection — the whole selection garrisons, as a click on
// the TC itself would order.
test('(b) a group selection whose member is picked inside the footprint garrisons as a group', () => {
  const w = makeWorld({ selected: [53, 73] });
  const [wx, wy] = kingClick(w);
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]),
    [['garrison_tc', 53, 0], ['garrison_tc', 73, 0]],
    'both selected units are ordered into the TC');
});

// (c) FIX #15's bound is preserved (occorder18's capture):
// the pick names an own unit that is NOT in the selection —
// no garrison, the click falls through to the ground move.
test('(c) a pick naming an OTHER own unit inside the footprint still does NOT garrison (#445 bound)', () => {
  const w = makeWorld({ selected: [73] });
  assert.equal(w.entityAt(2, 2, true), 72,
    'staging: the pick resolves to our own bystander 72, who is not selected');
  w.orderAt(2, 2, false, true);
  assert.ok(!w.attempts.some((c) => c.t === 'garrison_tc'),
    'no garrison_tc is even attempted');
  assert.deepEqual(w.issued.map((c) => [c.t, c.e]), [['move', 73]],
    'the selected Militia moves toward the named unit, exactly as after FIX #15');
});

// (d) The enemy reading is preserved (#473 / FIX #5): a
// click resolving to the enemy Scout inside the footprint
// is an attack reading — with the King selected his refused
// attack dies at the enemy guard; no garrison, no move.
test('(d) a pick naming the ENEMY inside the footprint still attacks / enemy-guards — never garrisons', () => {
  const w = makeWorld({ selected: [53], withEnemy: true });
  assert.equal(w.entityAt(7, -1, true), 28,
    'staging: the pick resolves to the enemy Scout 28');
  w.orderAt(7, -1, false, true);
  assert.deepEqual(w.attempts.map((c) => [c.t, c.e, c.tg]), [['attack', 53, 28]],
    'the only attempted order is the King\'s attack on the enemy');
  assert.ok(!w.attempts.some((c) => c.t === 'garrison_tc' || c.t === 'move'),
    'no garrison and no move: FIX #5\'s enemy guard returns first');
});

test('(d2) a Militia selected, the same enemy pick issues the attack', () => {
  const w = makeWorld({ selected: [73], withEnemy: true });
  w.orderAt(7, -1, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tg]), [['attack', 73, 28]],
    'exactly one order: the attack — clicking an enemy still attacks');
});

// (e) The intended garrison gesture survives unchanged: a
// click on the TC itself (no unit named) garrisons the King.
test('(e) a right-click on the TC itself (no unit picked) still garrisons the King', () => {
  const w = makeWorld({});
  assert.equal(w.entityAt(-4, 4, true), null,
    'staging: deep inside the footprint, away from every unit, the pick names nothing');
  assert.equal(w.tcAt(-4, 4), 0, 'staging: the point is the TC');
  w.orderAt(-4, 4, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e, c.tc]), [['garrison_tc', 53, 0]],
    'the ordinary garrison gesture is untouched');
});

// (f) No overreach: a self-pick OUTSIDE any TC footprint is
// still a plain move — the disjunct only fires where tcT
// already names our own TC.
test('(f) the King picked outside any footprint still just moves', () => {
  const w = makeWorld({ kingPos: [20, 20] });
  const [sx, sy] = w.worldToScreen(20, 20);
  const [wx, wy] = w.eventWorld(sx, sy - 2);
  assert.equal(w.tcAt(wx, wy), null, 'staging: no Town Center at the click');
  assert.equal(w.entityAt(wx, wy, true), 53, 'staging: the pick names the King');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => [c.t, c.e]), [['move', 53]],
    'a self-pick on open ground keeps its ordinary move reading');
});
