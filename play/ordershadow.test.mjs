// orderAt i18n-shadowing regression pins (bot-loop; flagged as
// an adjacent observation in the FIX #2b builder's handoff,
// fix2b-build.md).
//
// Inside orderAt, the picked target was held in a local named
// `t`:
//
//   let t = entityAt(wx, wy, fromCanvas);
//
// `t` is also the i18n function app.js imports from i18n.js,
// and a function-scoped `let` shadows it for the WHOLE body of
// orderAt. Two consequences, both live on pristine code:
//
//  1. Every success flash AFTER the pick — the Ram-garrison,
//     transport-load (both directions), TC-garrison and
//     building-garrison blocks end in
//     flash(t('flash.garrisoned')) / flash(t('flash.loaded'))
//     — called the entity id (a number, or null on the TC /
//     building paths where entityAt names nothing) as a
//     function: TypeError, thrown AFTER the order was issued.
//     The order stands; the click handler aborts with a
//     console error and the confirmation flash never shows.
//  2. The no-selection flash at the TOP of orderAt (before the
//     `let` line executes) hit the binding in its temporal
//     dead zone: right-clicking with nothing selected threw
//     ReferenceError instead of showing "select a unit first".
//
// The fix renames the local to `tgt`; the flash sites are
// untouched and resolve to the i18n function again. No order,
// gate or fall-through changes.
//
// Staging choice: these tests slice the REAL orderAt out of
// app.js (the occludedorder.test.mjs pattern) and drive its
// success paths directly. entityAt / tcAt / buildingAt are
// staged collaborators — the defect lives entirely inside
// orderAt's own scope, so the pick functions are stubbed to
// name the staged host, exactly as a real pick would. The
// i18n stub is the identity function, so a flash that reaches
// it records the message KEY; on pristine code the same call
// throws before flash is ever invoked.
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

const orderAtSrc = sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting');

// ------------------------------------------------------------------
// The staged scene (indices are entity ids, as in the client):
//   0   our Town Center (entity) at (100, 100)
//   5   our Battering Ram (kind 17) at (50, 50)
//   6   our Transport Ship (kind 32) at (60, 60)
//   10  our foot unit (kind 1) at (40, 40) — the usual selection
// Each scenario names its host through exactly one pick, the way
// the real pick functions would (entityAt skips Town Centers, so
// the TC scenario picks through tcAt and entityAt names null).
function makeWorld({ selected = [10], pick = null, tcPick = null, buildingPick = null } = {}) {
  const kinds = [];
  const owners = [];
  const pos = [];
  const hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y;
    hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, 100, 100, 2400); // KIND_TC = 13
  put(5, 17, 0, 50, 50, 200); // Battering Ram
  put(6, 32, 0, 60, 60, 100); // Transport Ship
  put(10, 1, 0, 40, 40, 40); // foot unit
  const st = { kinds, owners, pos, hp, count: 11, garrisoned: [] };
  const issued = [];
  const flashes = [];
  const env = {
    entityState: () => st,
    // staged picks: each scenario's host, as the real pick
    // functions would name it
    entityAt: () => pick,
    tcAt: () => tcPick,
    buildingAt: () => buildingPick,
    occludedUnitAt: () => null,
    cellVisibleW: () => true,
    cellExploredW: () => true,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    // selection + order sink: every staged order is accepted
    selectedUnits: () => selected,
    selectedAnimal: null,
    selectedBuilding: null,
    issue: (cmd) => { issued.push(cmd); return true; },
    // the i18n function the flash sites must resolve to
    t: (k) => k,
    flash: (msg) => { flashes.push(msg); },
    // misc orderAt plumbing
    localOwner: 0,
    foeOwner: () => 2,
    formationApplies: () => false,
    formationChoice: { tag: () => 0 },
    ackClassForKinds: () => null,
    playAck: () => {},
    carryKind: new Set(),
    clickMarkers: { add: () => {} },
    firstGarrisonedRelic: () => null,
    sim: {},
    OBST: [],
    GW: 0,
    worldToCell: () => [0, 0],
    KIND_SHEEP: 5,
    KIND_RELIC: 21,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${orderAtSrc}\nreturn { orderAt };`,
  );
  const fns = factory(...Object.values(env));
  return { ...fns, issued, flashes };
}

test('Ram garrison success: the garrisoned flash reaches the i18n function', () => {
  const w = makeWorld({ pick: 5 });
  w.orderAt(50, 50, false, false);
  assert.deepEqual(w.issued, [{ t: 'garrison_u', e: 10, host: 5 }],
    'the garrison order is issued to the Ram');
  assert.deepEqual(w.flashes, ['flash.garrisoned'],
    'the success flash resolves through the i18n t() instead of throwing on the entity id');
});

test('transport load success: the loaded flash reaches the i18n function', () => {
  const w = makeWorld({ pick: 6 });
  w.orderAt(60, 60, false, false);
  assert.deepEqual(w.issued, [{ t: 'load_t', e: 10, ship: 6 }],
    'the load order is issued to the Transport Ship');
  assert.deepEqual(w.flashes, ['flash.loaded'],
    'the success flash resolves through the i18n t() instead of throwing on the entity id');
});

test('reverse load (ship selected, unit clicked): the loaded flash reaches the i18n function', () => {
  const w = makeWorld({ selected: [6], pick: 10 });
  w.orderAt(40, 40, false, false);
  assert.deepEqual(w.issued, [{ t: 'load_t', e: 10, ship: 6 }],
    'the load order names the clicked unit and the selected ship');
  assert.deepEqual(w.flashes, ['flash.loaded'],
    'the success flash resolves through the i18n t() instead of throwing on the entity id');
});

test('TC garrison success: the garrisoned flash reaches the i18n function (pick is null here)', () => {
  // entityAt skips Town Centers, so on this path the picked
  // local is null — on pristine code the flash called null.
  const w = makeWorld({ pick: null, tcPick: 0 });
  w.orderAt(100, 100, false, false);
  assert.deepEqual(w.issued, [{ t: 'garrison_tc', e: 10, tc: 0 }],
    'the garrison order is issued to our Town Center');
  assert.deepEqual(w.flashes, ['flash.garrisoned'],
    'the success flash resolves through the i18n t() instead of throwing on null');
});

test('building garrison success: the garrisoned flash reaches the i18n function (pick is null here)', () => {
  const tower = { id: 3, owner: 0, complete: true, kind: 10, hpFrac: 1, x: 50, y: 50 };
  const w = makeWorld({ pick: null, tcPick: null, buildingPick: tower });
  w.orderAt(50, 50, false, false);
  assert.deepEqual(w.issued, [{ t: 'garrison_b', e: 10, b: 3 }],
    'the garrison order is issued to the tower');
  assert.deepEqual(w.flashes, ['flash.garrisoned'],
    'the success flash resolves through the i18n t() instead of throwing on null');
});

test('no selection: the select-a-unit flash reaches the i18n function (no TDZ throw)', () => {
  // This flash sits BEFORE the picked-target declaration in
  // orderAt; with the local named t it read the binding in its
  // temporal dead zone and threw ReferenceError.
  const w = makeWorld({ selected: [] });
  w.orderAt(50, 50, false, false);
  assert.deepEqual(w.issued, [], 'no order is issued with no selection');
  assert.deepEqual(w.flashes, ['flash.selectUnitFirst'],
    'the hint flash resolves through the i18n t() instead of throwing');
});

test('wiring: orderAt declares no local named t, and the flash call sites still call t(...)', () => {
  assert.ok(!/\b(?:let|const|var)\s+t\s*=/.test(orderAtSrc),
    'no declaration in orderAt shadows the i18n t function');
  assert.ok(!/\bst\.(?:owners|kinds|hp|pos)\[t\b/.test(orderAtSrc),
    'no state array in orderAt is indexed by a bare t');
  assert.equal(orderAtSrc.split("flash(t('flash.garrisoned'))").length - 1, 3,
    'the three garrisoned flash sites (Ram, TC, building) still call the i18n t()');
  assert.equal(orderAtSrc.split("flash(t('flash.loaded'))").length - 1, 2,
    'the two loaded flash sites (transport, reverse load) still call the i18n t()');
});
