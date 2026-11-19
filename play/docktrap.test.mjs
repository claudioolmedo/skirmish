// Dock / Fish Trap GHOST vs sim placement rules (bot-loop
// docktrap builder, 2026-11-19, base f7068f7).
//
// BACKGROUND (wallghost fix, main f7068f7): the wall-drag ghost
// was taught the sim's per-cell explored rule (BLD-01), but the
// same hole was left in the two water mirrors — the wallghost
// handoff flagged it as a same-class observation, out of its
// scope. The question for this file: do `mirrorDockValid` and
// `mirrorTrapValid` in app.js predict the sim's explored
// verdict for the Dock and the Fish Trap?
//
// METHOD: the real functions are sliced out of app.js and run
// against a staged stub sim (the wallghost / occorder harness
// pattern), so every row below is proven by a staged case here,
// not by reading code alone. The sim side of each row is the
// code cited in the row.
//
// SIM RULES (sim/src/lib.rs Sim::validate_build):
//   - The Dock (kind 23, Shoreline) goes through the GENERIC
//     path: after the shoreline predicate, EVERY footprint
//     cell must be explored by the builder owner
//     (cell_explored_by, BLD-01) — explored-but-fogged (1)
//     stays legal, the black (0) refuses.
//   - The Fish Trap (kind 24) has its own branch,
//     validate_build_fish_trap, whose FIRST check is the same
//     explored rule on its single cell.
//   - Resume precedes every placement check on BOTH paths:
//     validate_build's resume loop returns Resume for the
//     owner's own incomplete building of the same kind before
//     the age gates and before either branch runs — so a
//     resume verdict never reaches the explored check.
//
// PRE-FIX MEASUREMENT (pristine f7068f7, this file): the
// unexplored rows and the wiring pin FAIL (ghost green over
// the black, sim refuses); every already-correct row and
// every KNOWN RESIDUAL row passes — they are the
// discrimination, not filler. Exact counts are in the handoff
// (~/workspace/skirmish-study/handoffs/docktrap-build.md).
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
// Staged world: 20x20 cells, CS 4, HALF 40. Cell (x,y)'s center
// world point is (-40 + (x+.5)*4, -40 + (y+.5)*4).
const GW = 20, GH = 20, CS = 4, HALF = 40;
const cellCenter = (x, y) => [-HALF + (x + 0.5) * CS, -HALF + (y + 0.5) * CS];
// kind_footprints flat [w,h] per tag 0..26 (sim's BLD-06 table,
// sim/src/build.rs BuildingKind::footprint — Dock 3x3, Trap 1x1).
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
    age: 1, // Feudal (Fish Trap legal) unless a test lowers it
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

// A Dock footprint at anchor (5,5): the 3x3 spans (5..7, 5..7);
// its top row is deep water (OBST reads 1 there, as the real
// obstacles() layer does for deep water), the rest is land —
// a shoreline straddle, water side and land side both present.
function stageDock(W) {
  for (let x = 5; x <= 7; x++) {
    W.TERRAIN[at(W, x, 5)] = 1; // deep water
    W.OBST[at(W, x, 5)] = 1; // deep water is in the obstacle layer
  }
}
// A Fish Trap cell at (5,5): shallows — the water-side cell
// both the sim predicate and the mirror agree is placeable
// (deep water is a separate residual row below: the real
// obstacle layer reads 1 on deep, which the mirror's OBST
// check conflates with an obstruction).
function stageTrap(W) {
  W.TERRAIN[at(W, 5, 5)] = 2; // shallows, OBST stays 0
}

// ------------------------------------------------------------------
// Already-correct rows (pass before and after the fix).

test('Dock baseline: a shoreline straddle on explored ground is valid', () => {
  const { W, mirrorDockValid, mirrorValid } = makeWorld();
  stageDock(W);
  assert.equal(mirrorDockValid(5, 5), true);
  assert.equal(mirrorValid(23, 5, 5), true, 'the dispatch reaches the Dock mirror');
});

test('Dock legal side: explored-but-not-visible ground (fog 1) stays buildable', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  for (let y = 5; y <= 7; y++) for (let x = 5; x <= 7; x++) W.FOG[at(W, x, y)] = 1;
  assert.equal(mirrorDockValid(5, 5), true,
    'BLD-01 refuses only the black (fog 0); remembered ground is legal');
});

test('Dock terrain controls: all-land and all-water footprints refuse', () => {
  const { W, mirrorDockValid } = makeWorld();
  assert.equal(mirrorDockValid(5, 5), false, 'all land: no water side');
  for (let y = 5; y <= 7; y++) for (let x = 5; x <= 7; x++) W.TERRAIN[at(W, x, y)] = 1;
  assert.equal(mirrorDockValid(5, 5), false, 'all deep water: no land side');
});

test('Dock price: insufficient wood refuses', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  W.stock[1] = 149; // the Dock costs 150 wood
  assert.equal(mirrorDockValid(5, 5), false);
  W.stock[1] = 150;
  assert.equal(mirrorDockValid(5, 5), true);
});

test('Dock nodes: a live non-fish node refuses; a fish shoal does not', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  W.nodes.push({ x: 6, y: 6, kind: 0, amount: 100 }); // a tree-side node on a land cell
  assert.equal(mirrorDockValid(5, 5), false, 'live non-fish node refuses, as in the sim');
  W.nodes[0].kind = 7; // shore fish shoal — the 2026-11-16 DE exception
  assert.equal(mirrorDockValid(5, 5), true, 'a fish shoal stays under the Dock, as in DE');
  W.nodes[0].kind = 8; // deep fish shoal, on the water row
  W.nodes[0].x = 5; W.nodes[0].y = 5;
  assert.equal(mirrorDockValid(5, 5), true, 'a deep fish shoal is exempt too');
});

test('Dock obstruction: a tree block on a land footprint cell refuses', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  W.OBST[at(W, 6, 6)] = 1; // live tree on a land cell (node block)
  assert.equal(mirrorDockValid(5, 5), false);
});

test('Trap baseline: an explored shallows cell is valid', () => {
  const { W, mirrorTrapValid, mirrorValid } = makeWorld();
  stageTrap(W);
  assert.equal(mirrorTrapValid(5, 5), true);
  assert.equal(mirrorValid(24, 5, 5), true, 'the dispatch reaches the Trap mirror');
});

test('Trap legal side: explored-but-not-visible ground (fog 1) stays buildable', () => {
  const { W, mirrorTrapValid } = makeWorld();
  stageTrap(W);
  W.FOG[at(W, 5, 5)] = 1;
  assert.equal(mirrorTrapValid(5, 5), true,
    'BLD-01 refuses only the black (fog 0); remembered ground is legal');
});

test('Trap controls: land, obstruction, Dark Age and price all refuse', () => {
  const { W, mirrorTrapValid } = makeWorld();
  assert.equal(mirrorTrapValid(5, 5), false, 'land cell: water-side only');
  stageTrap(W);
  W.OBST[at(W, 5, 5)] = 1;
  assert.equal(mirrorTrapValid(5, 5), false, 'an obstructed cell refuses');
  W.OBST[at(W, 5, 5)] = 0;
  W.age = 0;
  assert.equal(mirrorTrapValid(5, 5), false, 'the Fish Trap is Feudal-born');
  W.age = 1;
  W.stock[1] = 99; // the trap costs 100 wood
  assert.equal(mirrorTrapValid(5, 5), false);
  W.stock[1] = 100;
  assert.equal(mirrorTrapValid(5, 5), true);
});

test('Trap nodes and units: a live node and another unit refuse; the ordering ship is exempt', () => {
  const { W, mirrorTrapValid } = makeWorld();
  stageTrap(W);
  W.nodes.push({ x: 5, y: 5, kind: 7, amount: 100 });
  assert.equal(mirrorTrapValid(5, 5), false, 'a trap never shares its cell with a live node');
  W.nodes[0].amount = 0;
  assert.equal(mirrorTrapValid(5, 5), true, 'a depleted node frees the cell');
  W.units.push({ x: 5, y: 5, hp: 100 }); // a living unit that is not the builder
  assert.equal(mirrorTrapValid(5, 5), false, 'another living unit on the cell refuses');
  W.selected = [0]; // the same unit, now the ordering ship itself
  assert.equal(mirrorTrapValid(5, 5), true, 'the ordering ship may stand on its site');
});

test('Trap resume: our own incomplete trap is valid even on unexplored ground', () => {
  const { W, mirrorTrapValid } = makeWorld();
  stageTrap(W);
  addBuilding(W, { kind: 24, cellX: 5, cellY: 5, w: 1, h: 1, complete: false });
  W.FOG[at(W, 5, 5)] = 0; // the black — a fresh placement would refuse
  W.stock[1] = 0; // and the price is not payable either
  assert.equal(mirrorTrapValid(5, 5), true,
    'the sim resume loop precedes the age, price and explored checks (validate_build) — the mirror agrees');
});

// ------------------------------------------------------------------
// Mismatches (fail on the pristine tree, pass after the fix).

test('Dock MISMATCH: one unexplored LAND footprint cell must read NOT buildable', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  W.FOG[at(W, 7, 7)] = 0; // a land corner of the footprint is black
  assert.equal(mirrorDockValid(5, 5), false,
    'the sim checks EVERY footprint cell explored (generic path, BLD-01); the ghost must not paint over the black');
});

test('Dock MISMATCH: one unexplored WATER footprint cell must read NOT buildable', () => {
  const { W, mirrorDockValid, mirrorValid } = makeWorld();
  stageDock(W);
  W.FOG[at(W, 5, 5)] = 0; // a deep-water corner of the footprint is black
  assert.equal(mirrorDockValid(5, 5), false,
    'the shoreline straddle does not exempt water cells from the explored check');
  assert.equal(mirrorValid(23, 5, 5), false, 'the same refusal through the dispatch');
});

test('Trap MISMATCH: an unexplored cell must read NOT buildable', () => {
  const { W, mirrorTrapValid, mirrorValid } = makeWorld();
  stageTrap(W);
  W.FOG[at(W, 5, 5)] = 0; // clean shallows underneath, never explored
  assert.equal(mirrorTrapValid(5, 5), false,
    'validate_build_fish_trap checks the explored rule first (BLD-01); the ghost must agree');
  assert.equal(mirrorValid(24, 5, 5), false, 'the same refusal through the dispatch');
});

// ------------------------------------------------------------------
// Wiring pins for the fix itself.

test('wiring: both mirrors consult the fog layer; the trap check sits after the resume return', () => {
  const dockSrc = appJs.slice(appJs.indexOf('function mirrorDockValid'), appJs.indexOf('function mirrorTrapValid'));
  const trapSrc = appJs.slice(appJs.indexOf('function mirrorTrapValid'), appJs.indexOf('function mirrorValid'));
  assert.match(dockSrc, /FOG\[y \* GW \+ x\] < 1/,
    'mirrorDockValid refuses unexplored footprint cells (BLD-01)');
  assert.match(trapSrc, /FOG\[cy \* GW \+ cx\] < 1/,
    'mirrorTrapValid refuses an unexplored cell (BLD-01)');
  assert.ok(trapSrc.indexOf('FOG[cy * GW + cx]') > trapSrc.indexOf('return true'),
    'the trap explored check follows the resume early-return, as in the sim');
});

// ------------------------------------------------------------------
// KNOWN RESIDUALS (measured, NOT fixed — see the handoff):
// divergences between these two mirrors and the sim that this
// package deliberately leaves alone (explored hole only).
// Each test pins the CURRENT (residual) behaviour and must be
// inverted by whichever front closes that gap.

test('KNOWN RESIDUAL Dock units: a living unit on a footprint cell still previews valid (sim refuses)', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  W.units.push({ x: 6, y: 6, hp: 100 }); // a living unit on a land footprint cell
  assert.equal(mirrorDockValid(5, 5), true,
    'RESIDUAL: the generic sim path refuses a footprint with a living unit on it (builder exempt); the Dock mirror has no unit check at all');
});

test('KNOWN RESIDUAL Dock resume: our own incomplete Dock still previews invalid (sim would Resume)', () => {
  const { W, mirrorDockValid } = makeWorld();
  stageDock(W);
  addBuilding(W, { kind: 23, cellX: 5, cellY: 5, w: 3, h: 3, complete: false });
  assert.equal(mirrorDockValid(5, 5), false,
    'RESIDUAL: the sim resume loop precedes every placement check, but the Dock mirror has no resume path — its building-overlap loop refuses the foundation itself');
});

test('KNOWN RESIDUAL Trap TC reserve: a trap cell inside a living Town Center reserve still previews valid (sim refuses)', () => {
  const { W, mirrorTrapValid } = makeWorld();
  stageTrap(W);
  W.tcs.push({ x: 5, y: 5, hp: 2400 }); // the trap cell is the TC cell itself — inside its 4x4 reserve
  assert.equal(mirrorTrapValid(5, 5), true,
    'RESIDUAL: validate_build_fish_trap reads the TC reserve (BLD-16) exactly like the generic path; the trap mirror never consults tcReservedCells');
});

test('KNOWN RESIDUAL Trap deep water: a deep-water cell still previews invalid (sim allows)', () => {
  const { W, mirrorTrapValid } = makeWorld();
  W.TERRAIN[at(W, 5, 5)] = 1; // deep water
  W.OBST[at(W, 5, 5)] = 1; // the real obstacles() layer reads 1 on deep water
  assert.equal(mirrorTrapValid(5, 5), false,
    'RESIDUAL: fish_trap_placement_ok admits deep water (its obstruction is the dynamic + node-block layers only), but the mirror reads the land obstacle layer, which counts deep water itself as blocked — shallows are the mirror-honest water cell');
});
