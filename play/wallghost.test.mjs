// Wall-drag GHOST vs sim placement rules (bot-loop wallghost
// builder, 2026-11-19, base bfdc54f).
//
// BACKGROUND (buildprobe builder, run #431, measured in the
// sim's own buildprobe_tests): the sim's wall-line placement
// founds EXACTLY the placeable cells of a drag and charges
// exactly 3 wood per founded palisade segment — a cell that is
// unplaceable (tree, bushes, TC reserve, UNEXPLORED) is skipped
// WITHOUT charge (BLD-01: the sim refuses placement on
// unexplored terrain). The question for this file: does the
// CLIENT's drag ghost — `mirrorValid` in app.js, plus the drag
// preview's per-cell budget walk in draw() — predict that same
// per-cell verdict?
//
// METHOD: the real functions are sliced out of app.js and run
// against a staged stub sim (the occorder harness pattern), so
// every row of the rule table in the handoff
// (~/workspace/skirmish-study/handoffs/wallghost-build.md) is
// proven by a staged case here, not by reading code alone.
//
// SIM RULES for one wall-line cell (sim/src/lib.rs
// Sim::validate_build, generic path, walls are 1x1 Normal
// kinds; Sim::issue_build_line runs it per segment):
//   R1  in bounds (world_to_cell clamps; footprint inside grid)
//   R2  EXPLORED by the placing owner (BLD-01) — fog >= 1
//   R3  not blocked: deep water / live tree (node block) /
//       building dynamic layer (grid.is_blocked)
//   R4  not shallows (Normal kinds refuse shallows separately)
//   R5  no live resource node on the cell (depleted exempt)
//   R6  no standing building footprint overlap (registry;
//       a gate reads 1 wide there — its span is reserved by
//       R3's dynamic layer while CLOSED, not while open)
//   R7  no living Town Center's reserved 4x4 (BLD-16)
//   R8  standing units do NOT block walls (is_wall exemption)
//   R9  age: Stone Wall needs Feudal; Palisade is Dark
//   R10 no prerequisite building for either wall
//   R11 cost payable per segment, charged per founded segment
//       (resume of own incomplete segment is free and precedes
//       every placement check)
//   R12 cliff straddle: footprint-internal borders only — a
//       1x1 footprint has none, so the rule can never fire for
//       a wall segment (code-geometry row, no case staged)
//   R13 reachability: a ring cell must stand open AND the
//       builder must have an A* path to it (approach_point) —
//       KNOWN RESIDUAL, see the last test and the handoff.
//
// PRE-FIX MEASUREMENT (pristine bfdc54f, this file):
//   R2 unexplored ......... ghost GREEN, sim refuses (FAIL)
//   R4 shallows ........... ghost GREEN, sim refuses (FAIL)
//   R6 gate span (closed).. ghost GREEN, sim refuses (FAIL)
//   R7 dead TC ............ ghost RED, sim allows (FAIL)
//   R7 edge TC alias ...... ghost RED at the mirrored cell,
//                           sim allows (FAIL)
//   gate ghost explored ... gateOrientationAt GREEN over the
//                           black, sim gate_span_placeable
//                           refuses (FAIL)
//   drag label over a line crossing the black: 6 segments /
//   18 wood previewed, sim founds 5 / charges 15 (FAIL)
//   R1,R3,R5,R6(anchor),R7(living, interior),R8,R9,R10,R11,
//   R2(explored-not-visible is legal) — already correct.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { wallLineCells } from './walls.js';

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
// kind_footprints flat [w,h] per tag 0..26 (sim's BLD-06 table).
const FOOTPRINTS = [
  2, 2, 3, 3, 3, 3, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 1, 1,
  1, 1, 4, 4, 4, 4, 1, 1, 4, 4, 4, 4, 4, 4, 4, 1, 4, 1, 3, 3,
  4, 4, 5, 5, 3, 3, 1, 1, 1, 1, 1, 1,
];

function makeWorld() {
  const W = {
    OBST: new Uint8Array(GW * GH),
    TERRAIN: new Uint8Array(GW * GH), // 0 land, 1 deep, 2 shallows
    FOG: new Uint8Array(GW * GH).fill(2), // all visible by default
    age: 1, // Feudal (stone wall legal) unless a test lowers it
    stock: [999, 999, 999, 999], // food, wood, gold, stone
    tcs: [], // {x, y, hp} — TC entities (kind tag 1)
    buildings: [], // building records for the flat ABI
    nodes: [], // {x, y, kind, amount}
    gateStates: [], // flat [id, alongX, open]
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
    entityState: () => ({ count: 0, hp: [], pos: [], garrisoned: [] }),
    selectedUnits: () => [],
  };
  const factory = new Function(
    ...Object.keys(env),
    `${costSrc}\n${worldToCellSrc}\n${nodeListSrc}\n${buildingListSrc}\n${kindSrc}\n${mirrorSrc}\n` +
    'return { mirrorValid, gateOrientationAt, mirrorGateValid, buildingList, tcReservedCells };',
  );
  const fns = factory(...Object.values(env));
  return { W, ...fns };
}

function addBuilding(W, b) {
  W.buildings.push({ id: W.buildings.length + 1, complete: true, owner: 0, ...b });
  return W.buildings[W.buildings.length - 1];
}
const at = (W, x, y) => y * GW + x;

// ------------------------------------------------------------------
// Already-correct rows (pass before and after the fix).

test('R1 bounds: a wall cell outside the grid is refused', () => {
  const { mirrorValid } = makeWorld();
  assert.equal(mirrorValid(9, -1, 5), false);
  assert.equal(mirrorValid(9, GW, 5), false);
  assert.equal(mirrorValid(9, 5, GH), false);
});

test('R3 deep water and tree-blocked cells are refused (OBST layer)', () => {
  const { W, mirrorValid } = makeWorld();
  W.TERRAIN[at(W, 5, 5)] = 1; W.OBST[at(W, 5, 5)] = 1; // deep water
  W.OBST[at(W, 6, 5)] = 1; // live tree (node block)
  assert.equal(mirrorValid(9, 5, 5), false, 'deep water');
  assert.equal(mirrorValid(9, 6, 5), false, 'tree cell');
  assert.equal(mirrorValid(9, 7, 5), true, 'clean neighbour stands');
});

test('R5 a live node refuses; a depleted node frees the cell', () => {
  const { W, mirrorValid } = makeWorld();
  W.nodes.push({ x: 5, y: 5, kind: 1, amount: 100 }); // berry bush, OBST stays 0
  assert.equal(W.OBST[at(W, 5, 5)], 0, 'staging: the bush is not a grid block');
  assert.equal(mirrorValid(9, 5, 5), false, 'live bush refuses');
  W.nodes[0].amount = 0;
  assert.equal(mirrorValid(9, 5, 5), true, 'depleted bush frees the cell');
});

test('R6 a standing building footprint refuses (square overlap)', () => {
  const { W, mirrorValid } = makeWorld();
  addBuilding(W, { kind: 0, cellX: 5, cellY: 5, w: 2, h: 2 }); // House
  assert.equal(mirrorValid(9, 6, 6), false, 'inside the House footprint');
  assert.equal(mirrorValid(9, 7, 7), true, 'outside it stands');
});

test('R7 a LIVING Town Center reserve refuses its whole 4x4', () => {
  const { W, mirrorValid } = makeWorld();
  W.tcs.push({ x: 10, y: 10, hp: 2400 });
  assert.equal(mirrorValid(9, 9, 9), false, 'reserve corner (-1,-1)');
  assert.equal(mirrorValid(9, 12, 12), false, 'reserve corner (+2,+2)');
  assert.equal(mirrorValid(9, 13, 13), true, 'one past the reserve stands');
});

test('R9/R11 age and per-segment cost gates match the sim', () => {
  const { W, mirrorValid } = makeWorld();
  W.age = 0;
  assert.equal(mirrorValid(14, 5, 5), false, 'Stone Wall is Feudal-born');
  assert.equal(mirrorValid(9, 5, 5), true, 'Palisade is Dark Age');
  W.age = 1;
  W.stock[1] = 2; // wood
  assert.equal(mirrorValid(9, 5, 5), false, 'palisade needs 3 wood');
  W.stock[1] = 3;
  assert.equal(mirrorValid(9, 5, 5), true);
  W.stock[3] = 4; // stone
  assert.equal(mirrorValid(14, 5, 5), false, 'stone wall needs 5 stone');
  W.stock[3] = 5;
  assert.equal(mirrorValid(14, 5, 5), true);
});

test('R11 resume: our own incomplete segment is valid even with an empty stockpile', () => {
  const { W, mirrorValid } = makeWorld();
  addBuilding(W, { kind: 9, cellX: 5, cellY: 5, w: 1, h: 1, complete: false });
  W.stock[1] = 0;
  assert.equal(mirrorValid(9, 5, 5), true,
    'the sim resumes before any placement/cost check — the ghost agrees');
});

test('R2 legal side: explored-but-not-visible ground (fog 1) stays buildable', () => {
  const { W, mirrorValid } = makeWorld();
  W.FOG[at(W, 5, 5)] = 1; // explored memory, not currently visible
  assert.equal(mirrorValid(9, 5, 5), true,
    'BLD-01 refuses only the black (fog 0); remembered ground is legal');
});

// ------------------------------------------------------------------
// Mismatches (fail on the pristine tree, pass after the fix).

test('R2 MISMATCH: an unexplored cell (fog 0) must read NOT buildable', () => {
  const { W, mirrorValid } = makeWorld();
  W.FOG[at(W, 5, 5)] = 0; // the black — clean land underneath
  assert.equal(mirrorValid(9, 5, 5), false,
    'the sim skips unexplored wall segments without charge (BLD-01); the ghost must not paint them green');
  assert.equal(mirrorValid(14, 5, 5), false, 'stone wall: same refusal');
});

test('R4 MISMATCH: a shallows cell must read NOT buildable for a wall', () => {
  const { W, mirrorValid } = makeWorld();
  W.TERRAIN[at(W, 5, 5)] = 2; // shallows — OBST stays 0 (land units wade)
  assert.equal(W.OBST[at(W, 5, 5)], 0, 'staging: shallows are not in the obstacle layer');
  assert.equal(mirrorValid(9, 5, 5), false,
    'the sim refuses Normal footprints on shallows (validate_build is_shallows check)');
});

test('R6 MISMATCH: a CLOSED gate span cell (non-anchor) must read NOT buildable', () => {
  const { W, mirrorValid } = makeWorld();
  const g = addBuilding(W, { kind: 18, cellX: 5, cellY: 5, w: 4, h: 1 }); // Palisade Gate, along-x
  W.gateStates = [g.id, 1, 0]; // alongX, closed
  assert.equal(mirrorValid(9, 5, 5), false, 'the gate anchor was already refused (square overlap)');
  assert.equal(mirrorValid(9, 7, 5), false,
    'span cell 3 of 4: the sim dynamic layer blocks it while the gate is closed');
  assert.equal(mirrorValid(9, 9, 5), true, 'one past the span stands');
});

test('R6 bound: an OPEN gate span cell (non-anchor) stays buildable, as in the sim', () => {
  const { W, mirrorValid } = makeWorld();
  const g = addBuilding(W, { kind: 18, cellX: 5, cellY: 5, w: 4, h: 1 });
  W.gateStates = [g.id, 1, 1]; // alongX, OPEN — the dynamic block lifts
  assert.equal(mirrorValid(9, 7, 5), true,
    'the sim is_blocked reads the dynamic layer, which an open gate does not write');
});

test('R7 MISMATCH: a DEAD Town Center reserves nothing', () => {
  const { W, mirrorValid } = makeWorld();
  W.tcs.push({ x: 10, y: 10, hp: 0 }); // razed — a tombstone entity
  assert.equal(mirrorValid(9, 11, 11), true,
    'the sim reserve loop skips dead TCs (!e.dead); the ghost must too');
});

test('R7 MISMATCH: a Town Center at the map edge must not alias its reserve onto the far side', () => {
  const { W, mirrorValid } = makeWorld();
  W.tcs.push({ x: 19, y: 10, hp: 2400 }); // east edge of the 20-wide grid
  // The reserve's out-of-grid cells (x = 20, 21) index y*GW+x,
  // which aliases (0, y+1) and (1, y+1) in a flat set.
  assert.equal(mirrorValid(9, 0, 11), true,
    'cell (0,11) is nowhere near the TC — only the flat-index alias touches it');
  assert.equal(mirrorValid(9, 18, 10), false, 'the in-grid reserve still refuses');
});

test('gate ghost MISMATCH: a gate span over unexplored ground resolves no orientation', () => {
  const { W, gateOrientationAt } = makeWorld();
  for (let x = 5; x <= 8; x++) W.FOG[at(W, x, 15)] = 0; // the along-x span is black
  for (let y = 15; y <= 18; y++) W.FOG[at(W, 5, y)] = 0; // and the along-y span too
  assert.equal(gateOrientationAt(18, 5, 15), null,
    'sim gate_span_placeable refuses every unexplored span cell (BLD-01)');
});

test('drag preview: the segment/cost label counts only cells the sim will found', () => {
  const { W, mirrorValid, buildingList } = makeWorld();
  W.FOG[at(W, 5, 2)] = 0; // one black cell inside the drag
  // The draw() walk, replicated line for line (app.js placement
  // ghost): per cell, mirrorValid decides green; a green
  // non-resume cell spends segCost from the budget; `placeable`
  // is the label's segment count and cost = placeable * segCost.
  const kind = 9, segCost = 3;
  const cells = wallLineCells(2, 2, 7, 2);
  const buildings = buildingList();
  let budget = W.stock[1];
  let placeable = 0;
  for (const [wx2, wy2] of cells) {
    const resume = buildings.some((b) => !b.complete && b.owner === 0
      && b.kind === kind && b.cellX === wx2 && b.cellY === wy2);
    let ok = mirrorValid(kind, wx2, wy2);
    if (ok && !resume) {
      if (budget >= segCost) { budget -= segCost; placeable++; } else ok = false;
    } else if (ok) placeable++;
  }
  assert.equal(cells.length, 6, 'staging: the drag spans 6 cells');
  assert.equal(placeable, 5,
    'the sim founds the 5 explored cells and charges 15 — the label must preview exactly that');
  assert.equal(placeable * segCost, 15);
});

// ------------------------------------------------------------------
// Wiring pins for the fix itself.

test('wiring: mirrorValid consults the fog and terrain layers; tcReservedCells reads living TCs in-grid', () => {
  assert.match(mirrorSrc, /FOG\[y \* GW \+ x\] < 1/,
    'mirrorValid refuses unexplored cells (BLD-01)');
  assert.match(mirrorSrc, /TERRAIN\[y \* GW \+ x\] !== 0/,
    'mirrorValid refuses water-side terrain for Normal kinds (shallows included)');
  const tcSrc = mirrorSrc.slice(0, mirrorSrc.indexOf('function gateSpanCells'));
  assert.match(tcSrc, /entity_hp/, 'tcReservedCells skips dead Town Centers');
  assert.match(tcSrc, /rx < GW && ry < GH/, 'tcReservedCells keeps the reserve in-grid');
});

// ------------------------------------------------------------------
// KNOWN RESIDUAL (R13, measured, NOT fixed — see the handoff):
// a wall cell whose whole ring is blocked can never be
// approached, so the sim's approach_point rejects it, yet the
// ghost (which has no per-builder A*) still paints it green.
// This test pins the gap's existence and shape so it is a
// measured fact, not a suspicion; it asserts the CURRENT
// (residual) behaviour and must be inverted by whichever front
// mirrors reachability.

test('KNOWN RESIDUAL R13: a ring-enclosed cell still previews green (sim approach_point would reject)', () => {
  const { W, mirrorValid } = makeWorld();
  for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
    if (dx === 0 && dy === 0) continue;
    W.OBST[at(W, 10 + dx, 5 + dy)] = 1; // the whole 8-ring is tree/water-blocked
  }
  assert.equal(mirrorValid(9, 10, 5), true,
    'RESIDUAL: no ring cell stands open, so no builder can ever stand beside this segment — the sim rejects, the ghost cannot yet see it');
});
