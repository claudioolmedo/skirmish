// Remaining-building GHOST vs sim placement rules (bot-loop
// ghostaudit builder, 2026-11-19, base a90a1a5).
//
// BACKGROUND (wallghost f7068f7, docktrap 579708a): the wall,
// Dock and Fish Trap ghosts were taught the sim's per-cell
// explored rule (BLD-01). The wallghost fix landed in the SHARED
// square mirror (`mirrorValid`), so every other building already
// inherited the explored / shallows / TC-reserve / closed-gate-span
// rows. The question for this file: does the square mirror predict
// `Sim::validate_build`'s verdict for the REMAINING placeable
// buildings — House, Barracks, Tower, Mill, the camps, Farm, and
// every other kind the dispatch routes to the generic path?
//
// METHOD: the real functions are sliced out of app.js and run
// against a staged stub sim (the wallghost / docktrap harness
// pattern), so every row below is proven by a staged case here,
// not by reading code alone. The sim side of each row is
// `Sim::validate_build` (sim/src/lib.rs, generic path) plus
// `BuildingKind::age_required` / `requires_building`
// (sim/src/build.rs).
//
// SIM RULES for one generic (Normal, square) placement:
//   G1  in bounds (footprint rect inside the grid)
//   G2  EXPLORED by the placing owner per cell (BLD-01, fog >= 1)
//   G3  not blocked / not shallows per cell (is_blocked ||
//       is_shallows — deep water, live tree, dynamic layer)
//   G4  no living Town Center's reserved 4x4 per cell (BLD-16)
//   G5  no live resource node per cell (depleted exempt)
//   G6  no standing-building footprint overlap (registry reads
//       the standing side's across edge; a CLOSED gate's span is
//       reserved by the dynamic layer)
//   G7  AGE + prerequisite gates (BLD-15): Feudal — Range,
//       Stable, Blacksmith, Watch Tower, Market, Stone Wall;
//       Castle — Town Center, Castle, University, Siege
//       Workshop, Guard Tower, Monastery; Imperial — Wonder,
//       Keep. Prerequisites: Farm <- Mill, Range/Stable <-
//       Barracks, Market <- Mill, Siege Workshop <- Blacksmith.
//       The Monument is unconstructible (always Reject).
//   G8  standing UNITS block per cell — any living entity but
//       the ordering builder; Farms and walls are exempt
//   G9  cliff straddle (MAP-07): no border BETWEEN footprint
//       cells may be a cliff (height step >= CLIFF_STEP 3);
//       1x1 footprints have no internal border and never fire
//   G10 cost payable (wood / stone / gold rows)
//   G11 resume: the owner's own incomplete building of the same
//       kind resumes from ANY footprint cell, before every
//       other check (SEL-11)
//   G12 reachability (approach_point) — KNOWN RESIDUAL, see the
//       last tests and the handoff.
//
// PRE-FIX MEASUREMENT (pristine a90a1a5, this file): the G7
// rows for kinds 10/12/13/25/26/21, the G8 unit rows, the G9
// cliff rows, the G11 any-cell resume rows and the wiring pin
// FAIL (ghost green where the sim refuses — red where the sim
// resumes); every already-correct row and every KNOWN
// RESIDUAL row passes — they are the discrimination, not
// filler. Exact counts are in the handoff
// (~/workspace/skirmish-study/handoffs/ghostaudit-build.md).
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
// sim/src/build.rs BuildingKind::footprint — House 2x2, Farm 3x3,
// Barracks 3x3, camps 2x2, Range/Stable/Blacksmith 3x3, Market
// 4x4, TC-building 4x4, Castle/University/Siege 4x4, Monastery
// 3x3, Monument 4x4, Wonder 5x5, towers/walls 1x1, gates 4x1).
const FOOTPRINTS = [
  2, 2, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 1, 1,
  1, 1, 1, 1, 4, 4, 4, 4, 1, 1, 4, 4, 4, 4, 4, 4, 4, 1, 4, 1,
  3, 3, 4, 4, 5, 5, 3, 3, 1, 1, 1, 1, 1, 1,
];

function makeWorld() {
  const W = {
    OBST: new Uint8Array(GW * GH),
    TERRAIN: new Uint8Array(GW * GH), // 0 land, 1 deep, 2 shallows
    HEIGHTS: new Uint8Array(GW * GH), // 0..4, flat by default
    FOG: new Uint8Array(GW * GH).fill(2), // all visible by default
    age: 3, // Imperial (every age gate open) unless a test lowers it
    stock: [2000, 2000, 2000, 2000], // food, wood, gold, stone (the Wonder's 1000 rows included)
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
    OBST: W.OBST, TERRAIN: W.TERRAIN, HEIGHTS: W.HEIGHTS, FOG: W.FOG,
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
    'return { mirrorValid, buildingList, tcReservedCells };',
  );
  const fns = factory(...Object.values(env));
  return { W, ...fns };
}

function addBuilding(W, b) {
  W.buildings.push({ id: W.buildings.length + 1, complete: true, owner: 0, ...b });
  return W.buildings[W.buildings.length - 1];
}
// The completed prerequisite set, staged far from the (5,5)
// test area: a Mill (BLD-15 chain for Farm and Market), a
// Barracks (Range/Stable) and a Blacksmith (Siege Workshop).
function stagePrereqs(W) {
  addBuilding(W, { kind: 4, cellX: 17, cellY: 17, w: 2, h: 2 }); // Mill
  addBuilding(W, { kind: 2, cellX: 13, cellY: 17, w: 3, h: 3 }); // Barracks
  addBuilding(W, { kind: 8, cellX: 17, cellY: 13, w: 3, h: 3 }); // Blacksmith
}
const at = (W, x, y) => y * GW + x;

// ------------------------------------------------------------------
// Already-correct rows (pass before and after the fix).

test('baselines: every named remaining building is valid on clean explored land (gates open)', () => {
  const { W, mirrorValid } = makeWorld();
  stagePrereqs(W);
  assert.equal(mirrorValid(0, 5, 5), true, 'House 2x2');
  assert.equal(mirrorValid(2, 5, 5), true, 'Barracks 3x3');
  assert.equal(mirrorValid(4, 5, 5), true, 'Mill 2x2');
  assert.equal(mirrorValid(3, 5, 5), true, 'Lumber Camp 2x2');
  assert.equal(mirrorValid(5, 5, 5), true, 'Mining Camp 2x2');
  assert.equal(mirrorValid(1, 5, 5), true, 'Farm 3x3 (Mill standing)');
  assert.equal(mirrorValid(10, 5, 5), true, 'Watch Tower 1x1');
  assert.equal(mirrorValid(11, 5, 5), true, 'Outpost 1x1');
  assert.equal(mirrorValid(12, 5, 5), true, 'Market 4x4 (Mill standing)');
  assert.equal(mirrorValid(13, 5, 5), true, 'Town Center 4x4');
  assert.equal(mirrorValid(15, 5, 5), true, 'Castle 4x4');
  assert.equal(mirrorValid(20, 5, 5), true, 'Monastery 3x3');
  assert.equal(mirrorValid(22, 5, 5), true, 'Wonder 5x5');
  assert.equal(mirrorValid(25, 5, 5), true, 'Guard Tower 1x1');
  assert.equal(mirrorValid(26, 5, 5), true, 'Keep 1x1');
});

test('G2 explored: one black footprint cell refuses a House; fog 1 stays legal', () => {
  const { W, mirrorValid } = makeWorld();
  W.FOG[at(W, 6, 6)] = 0; // a corner of the 2x2 is black
  assert.equal(mirrorValid(0, 5, 5), false, 'the sim checks EVERY footprint cell (BLD-01)');
  W.FOG[at(W, 6, 6)] = 1; // explored memory, not currently visible
  assert.equal(mirrorValid(0, 5, 5), true, 'remembered ground is legal');
  W.FOG[at(W, 7, 7)] = 0; // a Barracks corner goes black
  assert.equal(mirrorValid(2, 5, 5), false, 'Barracks: same per-cell rule');
});

test('G3 terrain: shallows and deep water refuse Normal footprints', () => {
  const { W, mirrorValid } = makeWorld();
  W.TERRAIN[at(W, 5, 5)] = 2; // shallows — OBST stays 0
  assert.equal(mirrorValid(4, 5, 5), false, 'Mill on shallows: the sim refuses (is_shallows)');
  W.TERRAIN[at(W, 5, 5)] = 1; W.OBST[at(W, 5, 5)] = 1; // deep water
  assert.equal(mirrorValid(4, 5, 5), false, 'Mill on deep water');
  W.OBST[at(W, 6, 6)] = 1; // a live tree block, land terrain
  W.TERRAIN[at(W, 5, 5)] = 0; W.OBST[at(W, 5, 5)] = 0;
  assert.equal(mirrorValid(0, 5, 5), false, 'House over a tree cell');
});

test('G5/G6 nodes and overlap: a live node and a standing footprint refuse; a depleted node frees', () => {
  const { W, mirrorValid } = makeWorld();
  W.nodes.push({ x: 6, y: 5, kind: 2, amount: 100 }); // a gold mine cell
  assert.equal(mirrorValid(5, 5, 5), false, 'Mining Camp over a live node refuses');
  W.nodes[0].amount = 0;
  assert.equal(mirrorValid(5, 5, 5), true, 'depleted node frees the cell');
  addBuilding(W, { kind: 2, cellX: 5, cellY: 5, w: 3, h: 3 }); // a standing Barracks
  assert.equal(mirrorValid(0, 6, 6), false, 'House inside the Barracks footprint');
  assert.equal(mirrorValid(0, 8, 8), true, 'House clear of it stands');
  assert.equal(mirrorValid(1, 5, 5), false,
    'a standing Farm footprint refuses too (the sim registry reads every kind) — staged with no Mill, the overlap itself is what the ghost reads first here');
});

test('G4 TC reserve: a living Town Center reserve refuses a camp; a dead TC frees it', () => {
  const { W, mirrorValid } = makeWorld();
  W.tcs.push({ x: 10, y: 10, hp: 2400 });
  assert.equal(mirrorValid(3, 9, 9), false, 'Lumber Camp over the reserve corner (-1,-1)');
  assert.equal(mirrorValid(3, 12, 12), false, 'reserve corner (+2,+2)');
  assert.equal(mirrorValid(3, 13, 13), true, 'one past the reserve stands');
  W.tcs[0].hp = 0; // razed — a tombstone
  assert.equal(mirrorValid(3, 10, 10), true, 'a dead TC reserves nothing');
});

test('G1 edge of map: a footprint crossing the border refuses; flush inside stands', () => {
  const { mirrorValid } = makeWorld();
  assert.equal(mirrorValid(0, GW - 1, 5), false, 'House 2x2 hanging off the east edge');
  assert.equal(mirrorValid(0, GW - 2, 5), true, 'House flush with the edge');
  assert.equal(mirrorValid(2, 17, 17), true, 'Barracks 3x3 in the corner');
  assert.equal(mirrorValid(2, 18, 18), false, 'Barracks 3x3 past the corner');
  assert.equal(mirrorValid(0, -1, 5), false, 'negative anchor');
});

test('G10 cost: the wood and stone rows refuse short stockpiles', () => {
  const { W, mirrorValid } = makeWorld();
  W.stock[1] = 24;
  assert.equal(mirrorValid(0, 5, 5), false, 'House needs 25 wood');
  W.stock[1] = 25;
  assert.equal(mirrorValid(0, 5, 5), true);
  W.stock[1] = 999; // the Tower's 35 wood is covered; only its stone row is staged below
  W.stock[3] = 124;
  assert.equal(mirrorValid(10, 5, 5), false, 'Watch Tower needs 125 stone (35 wood is covered)');
  W.stock[3] = 125;
  assert.equal(mirrorValid(10, 5, 5), true);
});

test('G7 already-mirrored gates: Range/Stable/Blacksmith age, Farm/Range prereqs, Castle/Wonder/Siege ages', () => {
  const { W, mirrorValid } = makeWorld();
  W.age = 0; // Dark
  assert.equal(mirrorValid(6, 5, 5), false, 'Archery Range is Feudal');
  assert.equal(mirrorValid(8, 5, 5), false, 'Blacksmith is Feudal');
  assert.equal(mirrorValid(1, 5, 5), false, 'Farm needs a Mill at any age');
  W.age = 1; // Feudal
  assert.equal(mirrorValid(6, 5, 5), false, 'Range still needs a Barracks');
  assert.equal(mirrorValid(15, 5, 5), false, 'Castle is Castle Age');
  assert.equal(mirrorValid(17, 5, 5), false, 'Siege Workshop is Castle Age');
  W.age = 2; // Castle
  assert.equal(mirrorValid(17, 5, 5), false, 'Siege Workshop still needs a Blacksmith');
  assert.equal(mirrorValid(22, 5, 5), false, 'Wonder is Imperial');
  stagePrereqs(W);
  assert.equal(mirrorValid(6, 5, 5), true, 'Range with Barracks + Feudal stands');
  assert.equal(mirrorValid(17, 5, 5), true, 'Siege Workshop with Blacksmith + Castle stands');
});

test('G11 resume at the anchor: our own incomplete House is valid with an empty stockpile', () => {
  const { W, mirrorValid } = makeWorld();
  addBuilding(W, { kind: 0, cellX: 5, cellY: 5, w: 2, h: 2, complete: false });
  W.stock = [0, 0, 0, 0];
  assert.equal(mirrorValid(0, 5, 5), true,
    'the sim resumes before any placement/cost check — the ghost agrees at the anchor');
});

test('G9 bound: a gentle height step (2) inside a footprint is NOT a cliff', () => {
  const { W, mirrorValid } = makeWorld();
  W.HEIGHTS[at(W, 6, 5)] = 2; W.HEIGHTS[at(W, 6, 6)] = 2; // right column up 2
  assert.equal(mirrorValid(0, 5, 5), true,
    'MAP-07 fires only at a step of CLIFF_STEP (3) or more; hills are buildable');
});

test('G9 bound: a 1x1 Tower beside a cliff face stands — the rule tests internal borders only', () => {
  const { W, mirrorValid } = makeWorld();
  W.HEIGHTS[at(W, 5, 5)] = 4; // the tower cell itself is high; every neighbour is 0
  assert.equal(mirrorValid(10, 5, 5), true,
    'a 1x1 footprint has no border between footprint cells, so the cliff rule can never fire for it');
});

test('G8 bounds: the ordering builder, the dead and the garrisoned do not block; Farms and walls are exempt', () => {
  const { W, mirrorValid } = makeWorld();
  stagePrereqs(W);
  W.units.push({ x: 5, y: 5, hp: 100 }); // our villager, standing on the House site
  W.selected = [0]; // it IS the ordering builder (attemptPlace picks a selected villager)
  assert.equal(mirrorValid(0, 5, 5), true, 'the sim exempts the ordering builder (e.id != entity)');
  W.selected = [];
  W.units[0].hp = 0;
  assert.equal(mirrorValid(0, 5, 5), true, 'a dead unit blocks nothing');
  W.units[0].hp = 100; W.units[0].garrisoned = 1;
  assert.equal(mirrorValid(0, 5, 5), true, 'a garrisoned unit is out of the field');
  W.units[0].garrisoned = 0;
  assert.equal(mirrorValid(1, 5, 5), true,
    'Farms are exempt from the unit check — the farmer stands on the plot by design');
  assert.equal(mirrorValid(9, 5, 5), true,
    'walls are exempt too (BLD-20): a line dragged through milling villagers founds every segment');
});

// ------------------------------------------------------------------
// Mismatches (fail on the pristine tree, pass after the fix).

test('G7 MISMATCH: the Watch Tower is Feudal — Dark Age must read NOT buildable', () => {
  const { W, mirrorValid } = makeWorld();
  W.age = 0;
  assert.equal(mirrorValid(10, 5, 5), false,
    'age_required reads Feudal for the Watch Tower (BLD-18); the mirror never checked kind 10');
  W.age = 1;
  assert.equal(mirrorValid(10, 5, 5), true, 'Feudal opens it');
});

test('G7 MISMATCH: the Market is Feudal AND needs a completed Mill', () => {
  const { W, mirrorValid } = makeWorld();
  stagePrereqs(W); // Mill standing — only the AGE can refuse below
  W.age = 0;
  assert.equal(mirrorValid(12, 5, 5), false,
    'age_required reads Feudal for the Market (BLD-19); the mirror never checked kind 12');
  const W2 = makeWorld();
  W2.W.age = 1; // Feudal, but NO Mill anywhere
  assert.equal(W2.mirrorValid(12, 5, 5), false,
    'requires_building reads Mill for the Market (BLD-15 chain); the mirror never checked it');
  W.age = 1;
  assert.equal(mirrorValid(12, 5, 5), true, 'Feudal + Mill opens it');
});

test('G7 MISMATCH: the Town Center building is Castle Age', () => {
  const { W, mirrorValid } = makeWorld();
  W.age = 1; // Feudal
  assert.equal(mirrorValid(13, 5, 5), false,
    'age_required reads Castle for the Town Center (Castle P1); the mirror never checked kind 13 (the build PANEL gates it — the ghost did not)');
  W.age = 2;
  assert.equal(mirrorValid(13, 5, 5), true, 'Castle Age opens it');
});

test('G7 MISMATCH: the Guard Tower is Castle Age and the Keep is Imperial', () => {
  const { W, mirrorValid } = makeWorld();
  W.age = 1; // Feudal
  assert.equal(mirrorValid(25, 5, 5), false, 'Guard Tower: age_required reads Castle (BLD-18)');
  W.age = 2; // Castle
  assert.equal(mirrorValid(25, 5, 5), true, 'Castle Age opens the Guard Tower');
  assert.equal(mirrorValid(26, 5, 5), false, 'Keep: age_required reads Imperial (BLD-18)');
  W.age = 3;
  assert.equal(mirrorValid(26, 5, 5), true, 'Imperial opens the Keep');
});

test('G7 MISMATCH: the Monument is unconstructible at any age', () => {
  const { mirrorValid } = makeWorld(); // Imperial, rich, clean land
  assert.equal(mirrorValid(21, 5, 5), false,
    'validate_build rejects the Monument outright (Package I) — its cost rows read 0, so without the gate the ghost paints a free 4x4 green');
});

test('G8 MISMATCH: a living unit standing on the footprint refuses a House', () => {
  const { W, mirrorValid } = makeWorld();
  W.units.push({ x: 6, y: 6, hp: 100 }); // a living unit, NOT selected — not the builder
  assert.equal(mirrorValid(0, 5, 5), false,
    'the sim refuses any footprint cell a living unit stands on (any owner, Gaia included)');
  assert.equal(mirrorValid(0, 8, 8), true, 'a House clear of the unit stands');
});

test('G8 MISMATCH: a living unit refuses a Barracks and a Tower too', () => {
  const { W, mirrorValid } = makeWorld();
  W.units.push({ x: 7, y: 7, hp: 100 }); // a corner cell of the 3x3
  assert.equal(mirrorValid(2, 5, 5), false, 'Barracks corner cell occupied');
  assert.equal(mirrorValid(10, 7, 7), false,
    'the Tower is 1x1 and NOT exempt — only Farms and walls are');
  assert.equal(mirrorValid(10, 8, 8), true, 'a Tower on a free cell stands');
});

test('G9 MISMATCH: a cliff border BETWEEN footprint cells refuses a House', () => {
  const { W, mirrorValid } = makeWorld();
  W.HEIGHTS[at(W, 6, 5)] = 3; W.HEIGHTS[at(W, 6, 6)] = 3; // right column up exactly CLIFF_STEP
  assert.equal(mirrorValid(0, 5, 5), false,
    'MAP-07: a footprint may not straddle a cliff border (step >= 3 between its own cells)');
});

test('G9 MISMATCH: a cliff border between footprint cells refuses a Barracks', () => {
  const { W, mirrorValid } = makeWorld();
  for (let x = 5; x <= 7; x++) W.HEIGHTS[at(W, x, 7)] = 4; // bottom row on a shelf, step 4
  assert.equal(mirrorValid(2, 5, 5), false,
    'the internal border between rows y=6 and y=7 is a cliff — the sim rejects the whole footprint');
  assert.equal(mirrorValid(2, 5, 8), true, 'the same Barracks wholly on the shelf stands');
});

test('G11 MISMATCH: resuming our own incomplete building works from ANY footprint cell', () => {
  const { W, mirrorValid } = makeWorld();
  addBuilding(W, { kind: 0, cellX: 5, cellY: 5, w: 2, h: 2, complete: false });
  W.stock = [0, 0, 0, 0]; // resume is free and precedes the cost check
  assert.equal(mirrorValid(0, 6, 6), true,
    'the sim resume test reads footprint_cells().contains(order cell) (SEL-11) — a click on the foundation\'s far cell resumes; the mirror only recognised the anchor and painted the rest red via its own overlap loop');
  const W2 = makeWorld();
  addBuilding(W2.W, { kind: 2, cellX: 5, cellY: 5, w: 3, h: 3, complete: false });
  assert.equal(W2.mirrorValid(2, 7, 7), true, 'Barracks: the far corner cell resumes too');
});

test('G11 bound: an ENEMY incomplete building never resumes — its cells stay refused', () => {
  const { W, mirrorValid } = makeWorld();
  addBuilding(W, { kind: 0, cellX: 5, cellY: 5, w: 2, h: 2, complete: false, owner: 2 });
  assert.equal(mirrorValid(0, 6, 6), false,
    'only the owner\'s own villagers resume a foundation (M5); the enemy foundation is an overlap');
  assert.equal(mirrorValid(0, 5, 5), false, 'its anchor cell too');
});

// ------------------------------------------------------------------
// Wiring pins for the fix itself.

test('wiring: mirrorValid carries the full age table, the Market chain, the unit and cliff checks', () => {
  const mv = mirrorSrc.slice(mirrorSrc.indexOf('function mirrorValid'));
  assert.match(mv, /kind === 10 && myAge\(\) < 1/, 'Watch Tower Feudal gate');
  assert.match(mv, /kind === 12 && myAge\(\) < 1/, 'Market Feudal gate');
  assert.match(mv, /kind === 13 && myAge\(\) < 2/, 'Town Center Castle gate');
  assert.match(mv, /kind === 25 && myAge\(\) < 2/, 'Guard Tower Castle gate');
  assert.match(mv, /kind === 26 && myAge\(\) < 3/, 'Keep Imperial gate');
  assert.match(mv, /kind === 21\) return false/, 'Monument unconstructible gate');
  assert.match(mv, /kind === 12 && !ownDone\.some\(\(b\) => b\.kind === 4\)/,
    'Market requires a completed Mill');
  assert.match(mv, /CLIFF_STEP = 3/, 'the cliff threshold is the sim\'s CLIFF_STEP');
  assert.match(mv, /unitCells/, 'the standing-unit check is staged per footprint cell');
});

// ------------------------------------------------------------------
// KNOWN RESIDUALS (measured, NOT fixed — see the handoff):
// divergences between the square mirror and the sim that this
// package deliberately leaves alone. Each test pins the
// CURRENT (residual) behaviour and must be inverted by
// whichever front closes that gap.

test('KNOWN RESIDUAL G12 reachability: a ring-enclosed House site still previews green (sim approach_point would reject)', () => {
  const { W, mirrorValid } = makeWorld();
  for (let y = 4; y <= 7; y++) for (let x = 4; x <= 7; x++) {
    if (x >= 5 && x <= 6 && y >= 5 && y <= 6) continue; // the footprint itself stays clean
    W.OBST[at(W, x, y)] = 1; // the whole ring around the 2x2 is blocked
  }
  assert.equal(mirrorValid(0, 5, 5), true,
    'RESIDUAL: no ring cell stands open, so no builder can ever stand beside this House — the sim rejects, the ghost (no per-builder A*) cannot yet see it. Same class as wallghost R13 / docktrap reachability.');
});

test('KNOWN RESIDUAL G8 multi-select: a second SELECTED villager on the footprint still previews green (sim refuses)', () => {
  const { W, mirrorValid } = makeWorld();
  W.units.push({ x: 5, y: 5, hp: 100 }, { x: 6, y: 5, hp: 100 }); // two of our villagers on the site
  W.selected = [0, 1]; // both selected — the mirror exempts the selection, like mirrorTrapValid
  assert.equal(mirrorValid(0, 5, 5), true,
    'RESIDUAL: the sim exempts only THE ordering builder (the nearest selected villager, chosen by attemptPlace from the raw click point) — the second villager still blocks in the sim. mirrorValid receives only the anchor cell, not the click point, so it cannot name the one exempt builder; exempting the whole selection is the trap mirror\'s precedent and leaves exactly this sub-case divergent.');
});
