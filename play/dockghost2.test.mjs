// Dock / Fish Trap GHOST residuals #1–#3 (bot-loop dockghost2
// builder, 2026-11-19, base 7f8dbaa).
//
// BACKGROUND: the docktrap package (handoff docktrap-build.md)
// closed the BLD-01 explored hole in mirrorDockValid and
// mirrorTrapValid and pinned three further divergences as
// KNOWN RESIDUALS, measured but not fixed. This file stages
// those three against the sim and drives their closure:
//
//   #1 Dock standing units — the sim's generic path refuses a
//      Dock footprint with a living unit on ANY cell (the
//      ordering builder exempt); mirrorDockValid had no unit
//      check at all.
//   #2 Dock resume — the sim would Resume the owner's own
//      incomplete Dock (validate_build's resume loop precedes
//      every placement check, the order cell being ANY cell of
//      the standing footprint); mirrorDockValid had no resume
//      path, so its overlap loop refused the foundation itself.
//   #3 Fish Trap TC reserve — validate_build_fish_trap reads a
//      living Town Center's reserved 4x4 (BLD-16) exactly like
//      the generic path; mirrorTrapValid never consulted
//      tcReservedCells().
//
// METHOD: the docktrap harness pattern — the real functions
// sliced out of app.js and run against a staged stub sim, so
// every row is proven by a staged case here. The three
// residuals are ALSO pinned in docktrap.test.mjs; those pins
// are inverted in place by this package (dated justification
// there), and this file carries the deeper coverage.
//
// SIM RULES (sim/src/lib.rs Sim::validate_build):
//   - Resume loop first: an order whose cell is ANY cell of
//     the owner's own incomplete building of the same kind
//     returns Resume before the age gates and before either
//     branch (generic or fish-trap) runs.
//   - Generic path per-cell unit check: any living entity
//     other than the ordering builder standing on a footprint
//     cell refuses (any owner, Gaia included; Farms and walls
//     exempt — the Dock is neither).
//   - validate_build_fish_trap: after the node, building and
//     unit checks and before the price, a living Town Center's
//     reserved 4x4 (TC_FOOTPRINT anchored at the TC's cell
//     −(w/2 − 1)) refuses the trap cell.
//
// PRE-FIX MEASUREMENT (pristine 7f8dbaa, this file): the
// mismatch rows for #1, #2 and #3 FAIL (the ghost disagrees
// with the sim in the direction each residual documents);
// every control row and the one KNOWN RESIDUAL row (the
// whole-selection exemption, inherited from the square
// mirror) passes. Exact counts are in the handoff
// (~/workspace/skirmish-study/handoffs/dockghost2-build.md).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found in app.js`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}

const worldToCellSrc = sliceFn(appJs, 'function worldToCell(wx, wy) {', '\n// --- Placement ghost');
const nodeListSrc = sliceFn(appJs, 'function nodeList() {', '\n// Building data');
const buildingListSrc = sliceFn(appJs, 'function buildingList() {', '\n// One snapshot');
const kindSrc = sliceFn(appJs, 'let _kindFootprints = null;', '\nconst BUILD_ICON');
const mirrorSrc = sliceFn(appJs, 'function tcReservedCells() {', '\n// --- Click markers');
const costSrc = ['BUILD_COST', 'BUILD_COST_STONE', 'BUILD_COST_GOLD']
  .map((n) => appJs.match(new RegExp(`const ${n} = \\[[^\\]]*\\];`))[0]).join('\n');

// ------------------------------------------------------------------
// Staged world: 20x20 cells, CS 4, HALF 40 (the docktrap stage).
const GW = 20, GH = 20, CS = 4, HALF = 40;
const cellCenter = (x, y) => [-HALF + (x + 0.5) * CS, -HALF + (y + 0.5) * CS];
const FOOTPRINTS = [
  2, 2, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 1, 1,
  1, 1, 1, 1, 4, 4, 4, 4, 1, 1, 4, 4, 4, 4, 4, 4, 4, 1, 4, 1,
  3, 3, 4, 4, 5, 5, 3, 3, 1, 1, 1, 1, 1, 1,
];

function makeWorld() {
  const W = {
    OBST: new Uint8Array(GW * GH),
    TERRAIN: new Uint8Array(GW * GH), // 0 land, 1 deep, 2 shallows
    FOG: new Uint8Array(GW * GH).fill(2), // all visible by default
    age: 1, // Feudal unless a test lowers it
    stock: [999, 999, 999, 999], // food, wood, gold, stone
    tcs: [], // {x, y, hp} — TC entities (kind tag 1)
    buildings: [], // building records for the flat ABI
    nodes: [], // {x, y, kind, amount}
    gateStates: [], // flat [id, alongX, open]
    units: [], // {x, y, hp, garrisoned} — for entityState()
    selected: [], // entity ids, for selectedUnits()
  };
  const sim = {
    entity_kinds: () => W.tcs.map(() => 1),
    positions: () => W.tcs.flatMap((tc) => cellCenter(tc.x, tc.y)),
    entity_hp: () => W.tcs.flatMap((tc) => [tc.hp, tc.hp]),
    buildings: () => W.buildings.flatMap((b) => [
      b.id, b.kind, ...cellCenter(b.cellX, b.cellY),
      b.complete ? 1 : 0.5, b.complete ? 1 : 0,
      b.cellX, b.cellY, b.w, b.h, 0, 0, b.owner ?? 0, 1,
    ]),
    nodes: () => W.nodes.flatMap((n, i) => [...cellCenter(n.x, n.y), n.kind, n.amount]
      .flatMap((v, j) => (j === 0 ? [i + 1, v] : [v]))),
    gate_states: () => W.gateStates,
    tick_count: () => 0,
    kind_footprints: () => FOOTPRINTS,
  };
  const env = {
    sim, GW, GH, CS, HALF,
    OBST: W.OBST, TERRAIN: W.TERRAIN, FOG: W.FOG,
    localOwner: 0, KIND_TC: 1,
    myAge: () => W.age,
    myStock: () => W.stock,
    entityState: () => ({
      count: W.units.length,
      hp: W.units.flatMap((u) => [u.hp ?? 100, 100]),
      pos: W.units.flatMap((u) => cellCenter(u.x, u.y)),
      garrisoned: W.units.map((u) => (u.garrisoned ? 1 : 0)),
    }),
    selectedUnits: () => W.selected,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${costSrc}\n${worldToCellSrc}\n${nodeListSrc}\n${buildingListSrc}\n${kindSrc}\n${mirrorSrc}\n` +
    'return { mirrorValid, mirrorDockValid, mirrorTrapValid, buildingList, tcReservedCells };',
  );
  const fns = factory(...Object.values(env));
  return { W, ...fns };
}

function addBuilding(W, b) {
  W.buildings.push({ id: W.buildings.length + 1, complete: true, owner: 0, ...b });
  return W.buildings[W.buildings.length - 1];
}
const at = (W, x, y) => y * GW + x;

// The docktrap Dock stage: footprint at anchor (5,5), top row
// deep water, the rest land — a shoreline straddle.
function stageDock(W) {
  for (let x = 5; x <= 7; x++) {
    W.TERRAIN[at(W, x, 5)] = 1; // deep water
    W.OBST[at(W, x, 5)] = 1; // deep water is in the obstacle layer
  }
}
// A Fish Trap cell: shallows (the mirror-honest water cell).
function stageTrap(W, x = 5, y = 5) {
  W.TERRAIN[at(W, x, y)] = 2; // shallows, OBST stays 0
}

// ------------------------------------------------------------------
// Residual #1 — Dock standing units (sim: generic path refuses
// a footprint with a living unit on ANY cell, builder exempt).

test('#1 Dock MISMATCH: a living unit on a LAND footprint cell must read NOT buildable', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  W.units.push({ x: 6, y: 6, hp: 100 }); // a living unit on a land footprint cell
  assert.equal(mirrorDockValid(5, 5), false,
    'the generic sim path refuses a footprint with a living unit on it (builder exempt); the ghost must agree');
});

test('#1 Dock MISMATCH: a living unit on a WATER footprint cell must read NOT buildable', () => {
  const { W, mirrorDockValid, mirrorValid } = makeWorld();
  stageDock(W);
  W.units.push({ x: 5, y: 5, hp: 100 }); // a ship standing on the deep-water row
  assert.equal(mirrorDockValid(5, 5), false,
    'the sim unit check runs on every footprint cell, water cells included');
  assert.equal(mirrorValid(23, 5, 5), false, 'the same refusal through the dispatch');
});

test('#1 Dock controls: dead, garrisoned and off-footprint units do not refuse', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  W.units.push({ x: 6, y: 6, hp: 0 }); // dead (the sim hp < 1 rule)
  assert.equal(mirrorDockValid(5, 5), true, 'a dead unit reserves nothing');
  W.units[0].hp = 100;
  W.units[0].garrisoned = true; // out of the field
  assert.equal(mirrorDockValid(5, 5), true, 'a garrisoned unit is not standing on the cell');
  W.units[0].garrisoned = false;
  W.units[0].x = 8; W.units[0].y = 8; // alive, but off the footprint
  assert.equal(mirrorDockValid(5, 5), true, 'a unit beside the footprint does not refuse');
});

test('#1 Dock control: the ordering builder standing on the footprint is exempt', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  W.units.push({ x: 6, y: 6, hp: 100 });
  W.selected = [0]; // the same unit, now the ordering builder itself
  assert.equal(mirrorDockValid(5, 5), true,
    'the sim exempts the ordering builder — it steps to the ring to work');
});

// ------------------------------------------------------------------
// Residual #2 — Dock resume (sim: the resume loop precedes
// every placement check; the order cell may be ANY cell of
// the owner's own incomplete Dock's footprint).

test('#2 Dock MISMATCH: resuming our own incomplete Dock at its anchor must read buildable', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  addBuilding(W, { kind: 23, cellX: 5, cellY: 5, w: 3, h: 3, complete: false });
  assert.equal(mirrorDockValid(5, 5), true,
    'the sim resume loop returns Resume before the overlap check can refuse the foundation itself');
});

test('#2 Dock MISMATCH: an order landing on ANY footprint cell of our foundation resumes it', () => {
  const { W, mirrorDockValid, mirrorValid } = makeWorld();
  stageDock(W);
  addBuilding(W, { kind: 23, cellX: 5, cellY: 5, w: 3, h: 3, complete: false });
  assert.equal(mirrorDockValid(6, 6), true,
    'SEL-11: the order cell may be any cell of the standing footprint, not only its anchor');
  assert.equal(mirrorValid(23, 7, 5), true, 'the same resume through the dispatch, on a water-row cell');
});

test('#2 Dock MISMATCH: resume precedes the explored and price checks', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  addBuilding(W, { kind: 23, cellX: 5, cellY: 5, w: 3, h: 3, complete: false });
  W.FOG[at(W, 7, 7)] = 0; // a fresh placement would refuse (BLD-01)
  W.stock[1] = 0; // and the price is not payable either
  assert.equal(mirrorDockValid(5, 5), true,
    'validate_build returns Resume before the age, explored and cost checks — the mirror agrees');
});

test('#2 Dock controls: no resume for the enemy foundation, a completed Dock, or another kind', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  addBuilding(W, { kind: 23, cellX: 5, cellY: 5, w: 3, h: 3, complete: false, owner: 1 });
  assert.equal(mirrorDockValid(5, 5), false,
    'M5: only the owner resumes their own foundation — the enemy Dock still refuses by overlap');
  W.buildings[0].owner = 0;
  W.buildings[0].complete = true;
  assert.equal(mirrorDockValid(5, 5), false, 'a completed Dock is not resumed; it refuses by overlap');
  W.buildings[0].complete = false;
  W.buildings[0].kind = 2; // an incomplete Barracks on the same cells
  assert.equal(mirrorDockValid(5, 5), false, 'a foundation of another kind is not a Dock resume');
});

// ------------------------------------------------------------------
// Residual #3 — Fish Trap TC reserve (sim:
// validate_build_fish_trap reads a living TC's reserved 4x4,
// BLD-16, after the unit check and before the price).

test('#3 Trap MISMATCH: a trap cell on a living Town Center cell must read NOT buildable', () => {
  const { W, mirrorTrapValid } = makeWorld();
  stageTrap(W);
  W.tcs.push({ x: 5, y: 5, hp: 2400 }); // the trap cell is the TC cell itself — inside its 4x4 reserve
  assert.equal(mirrorTrapValid(5, 5), false,
    'validate_build_fish_trap reads the TC reserve (BLD-16) exactly like the generic path; the ghost must agree');
});

test('#3 Trap MISMATCH: the reserve edge cells refuse; one cell past the reserve is free', () => {
  const { W, mirrorTrapValid, mirrorValid } = makeWorld();
  W.tcs.push({ x: 5, y: 5, hp: 2400 }); // reserve spans (4..7, 4..7)
  stageTrap(W, 7, 7);
  assert.equal(mirrorTrapValid(7, 7), false, 'the far corner of the 4x4 reserve refuses');
  assert.equal(mirrorValid(24, 7, 7), false, 'the same refusal through the dispatch');
  stageTrap(W, 4, 4);
  assert.equal(mirrorTrapValid(4, 4), false, 'the near corner of the 4x4 reserve refuses');
  stageTrap(W, 8, 5);
  assert.equal(mirrorTrapValid(8, 5), true, 'one cell past the reserve edge is ordinary shallows');
});

test('#3 Trap control: a dead Town Center reserves nothing', () => {
  const { W, mirrorTrapValid } = makeWorld();
  stageTrap(W);
  W.tcs.push({ x: 5, y: 5, hp: 0 }); // razed — the sim reserve loop skips dead entities
  assert.equal(mirrorTrapValid(5, 5), true,
    'tcReservedCells reads only a LIVING Town Center (the wallghost HP rule)');
});

test('#3 Trap control: resuming our own incomplete trap precedes the reserve read', () => {
  const { W, mirrorTrapValid } = makeWorld();
  stageTrap(W);
  W.tcs.push({ x: 5, y: 5, hp: 2400 });
  addBuilding(W, { kind: 24, cellX: 5, cellY: 5, w: 1, h: 1, complete: false });
  assert.equal(mirrorTrapValid(5, 5), true,
    'validate_build returns Resume before the fish-trap branch runs at all — the reserve never gets a say');
});

// ------------------------------------------------------------------
// Wiring pins for the three fixes.

test('wiring: the Dock mirror runs the unit check and a resume return ahead of its standing checks', () => {
  const dockSrc = appJs.slice(appJs.indexOf('function mirrorDockValid'), appJs.indexOf('function mirrorTrapValid'));
  assert.match(dockSrc, /entityState\(\)/,
    'mirrorDockValid consults the unit layer (residual #1)');
  assert.match(dockSrc, /!b\.complete && b\.owner === localOwner && b\.kind === kind/,
    'mirrorDockValid resumes the owner\'s own incomplete Dock (residual #2)');
  assert.ok(dockSrc.indexOf('return true') < dockSrc.indexOf('waterSide'),
    'the Dock resume return precedes the shoreline cell loop, as in the sim');
});

test('wiring: the trap mirror consults the TC reserve after its resume return', () => {
  const trapSrc = appJs.slice(appJs.indexOf('function mirrorTrapValid'), appJs.indexOf('function mirrorValid'));
  assert.match(trapSrc, /tcReservedCells\(\)/,
    'mirrorTrapValid consults tcReservedCells (residual #3)');
  assert.ok(trapSrc.indexOf('tcReservedCells()') > trapSrc.indexOf('return true'),
    'the reserve read follows the resume early-return, as in the sim (the resume loop precedes the branch)');
});

// ------------------------------------------------------------------
// KNOWN RESIDUAL (inherited, measured, NOT fixed): the mirror
// exempts the WHOLE current selection from the unit check,
// while the sim exempts only the ONE ordering builder entity.
// This is the square mirror's documented sub-case (pinned in
// ghostaudit.test.mjs): mirrorValid receives only the anchor
// cell, not the raw click point attemptPlace uses to name the
// one exempt builder, so the mirror cannot name it either.
// Inverted by whichever front gives the mirrors the ordering
// builder's identity.

test('KNOWN RESIDUAL Dock units: a second SELECTED unit on the footprint still previews valid (sim refuses)', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  W.units.push({ x: 6, y: 6, hp: 100 }); // the ordering builder
  W.units.push({ x: 7, y: 7, hp: 100 }); // another selected villager on the footprint
  W.selected = [0, 1];
  assert.equal(mirrorDockValid(5, 5), true,
    'RESIDUAL: the sim exempts only the ordering builder entity (e.id == entity); the mirror exempts the whole selection, as the square and trap mirrors do');
});
