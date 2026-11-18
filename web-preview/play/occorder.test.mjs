// Occluded-outline ORDER degradation pins (bot-loop occorder,
// 2026-11-18; runs #420/#421 queue item 1).
//
// The live failure: a Stand Ground Scout ordered onto the red
// outline of an enemy Scout hidden under our Town Center walked
// to the outline and idled, twice, while an Aggressive Scout and
// a Militia engaged on the same kind of click. The stanceatk
// probe proved the SIM executes an explicit Attack identically
// in every stance (attack_auto=false bypasses the stance gates),
// so on the failing attempts the client must have issued a
// ground MOVE instead of {t:'attack'}.
//
// The occordersweep diagnostic (play/occordersweep.mjs) drove
// the REAL orderAt / entityAt / occludedUnitAt through the REAL
// isometric projection (real w2s/s2w, the two-step elevation
// solve in eventWorld, the lift in worldToScreen) over 6,104
// clicks on the painted outline — 4 settle angles × 4 height
// fields × zooms 8 and 24: with the INF-12 outline record
// present, 100% issued the attack. The projection round trip,
// the Ram-body precedence, the tcAt branch and the FIX #2b fog
// exemption are all clean. Degradation appears exactly when the
// record is ABSENT at click time: the ground pick (radius 2.2
// from a ground point displaced up to ~4.3 wu NW by the sprite's
// height) then decides alone, and 48.4% of the outline's area —
// its upper half — degrades to a move (100% when the enemy's
// cell is also fog-hidden). The minimap path (fromCanvas=false)
// is identical by construction (SEL-16: no outline pick there).
//
// So the live question is record CREATION. Two call-less draw
// paths could silently drop the record for exactly this scene,
// both the defect class the monk (2026-11-02) and fix2 closed
// for other kinds:
//  1. drawScout's vector fallback. The 2026-11-02 audit cleared
//     the Scout because it "routes through drawImage art paths"
//     — but BOTH can decline at once: the directional sheet is
//     the only sprite path while ATTACKING (the UNIT_ART set
//     carries scout gallop/idle frames and NO attack frames),
//     and an unavailable DIR sheet leaves pure vector strokes,
//     which recordFrameSprites cannot see: no outline painted,
//     no pick record, for the one unit in the one state this
//     defect is about.
//  2. The Town Center as occluder. Placed buildings carry
//     synthesized geometry (buildingOccGeo, fix2); the TC is an
//     ENTITY and its occ entry carried none — a vector-fallback
//     TC (the art chain's last resort) was a call-less occluder
//     the pass skips outright, so nothing behind it could be
//     outlined or picked at all.
//
// The fixes are the file's own patterns: the Scout figure is
// painted once into a cached sprite per cloth and blitted (the
// monk/cavalry treatment), and the TC entry spreads a
// synthesized tcOccGeo. Presentation only; sim and golden hash
// untouched. The behavioural pins below document the mechanism
// the record prevents (they pass before and after — the record
// logic itself never changed); the wiring pins fail on pristine.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { occludedPickAt } from './occlusion.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function fnBody(name) {
  const m = appJs.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n\\}`));
  assert.ok(m, `${name} found in app.js`);
  return m[0];
}

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found in app.js`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}

// ------------------------------------------------------------------
// The Scout unit-side recordability (fails on pristine).

test('drawScout blits a cached sprite through drawImage', () => {
  const body = fnBody('drawScout');
  assert.match(body, /scoutSprite\(cloth\)/,
    'drawScout obtains the cached scout sprite for its fallback');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'the fallback records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintScoutFigure\(ctx, sx, sy, u, cloth\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per scout draw (its own blit)');
});

test('the scout sprite is cached per cloth colour', () => {
  const body = fnBody('scoutSprite');
  assert.match(body, /scoutSpriteCache\.has\(cloth\)/,
    'cache is consulted before painting');
  assert.match(body, /scoutSpriteCache\.set\(cloth, cv\)/,
    'painted sprite is stored per cloth colour');
  assert.match(body, /paintScoutFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintScoutFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintScoutFigure');
  for (const colour of ['#5d4630', '#8a6844', '#77572f', '#e8c39a', '#d8d8d8']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (legs, horse, head, rider head, spear)`);
  }
  assert.match(body, /g\.ellipse\(sx, sy - 0\.95 \* u, 1\.15 \* u, 0\.55 \* u/,
    'the horse body keeps its geometry');
});

test('UNIT_ART has no scout attack frames (DIR is the only sprite path while attacking)', () => {
  // The fact that makes gap 1 reachable: unitArtImage can never
  // answer an attacking Scout, so drawDirUnit or the vector
  // fallback draws every attack frame.
  const files = sliceFn(appJs, 'const UNIT_ART_FILES = {', '\n};');
  assert.ok(!/scoutAttack/.test(files), 'no scoutAttack entries in UNIT_ART_FILES');
  assert.match(files, /scoutGallop1/, 'gallop frames exist (the moving Scout is covered)');
  assert.match(files, /scoutIdle1/, 'the idle frame exists');
  const spec = sliceFn(appJs, 'const DIR_ART_SPEC = {', '\n};');
  assert.match(spec, /scout: \{ states: \['idle', 'gallop', 'attack'\]/,
    'the DIR spec does carry a scout attack sheet — when it is drawable');
});

// ------------------------------------------------------------------
// The Town Center occluder-side recordability (fails on pristine).

test('the Town Center draw entry carries synthesized occluder geometry', () => {
  assert.match(appJs,
    /occ: \{ role: 'occluder', \.\.\.tcOccGeo\(wx, wy\) \}/,
    'the TC draw entry spreads tcOccGeo into its occ record');
  const geo = fnBody('tcOccGeo');
  assert.match(geo, /worldToScreen\(wx - a, wy - a\)/,
    'geometry corners go through the same projection as the draw');
  assert.match(geo, /const a = tcHalf\(\);/,
    'the footprint half is the TC\'s own');
  assert.match(geo, /2\.2 \* camS/,
    'height mirrors drawTC\'s vector wall height');
  assert.match(geo, /poly = \[up\(t\), up\(r\), r, bt, l, up\(l\)\]/,
    'the silhouette polygon is the raised footprint diamond');
});

test('tcOccGeo geometry: the rect is the footprint diamond raised by the wall height', () => {
  const src = sliceFn(appJs, 'function tcOccGeo(wx, wy) {', '\n}\n') + '\n}';
  const camS = 8;
  const worldToScreen = (wx, wy) => [480 + (wx - wy) * camS, 270 + ((wx + wy) / 2) * camS];
  const tcOccGeo = new Function('worldToScreen', 'tcHalf', 'camS',
    `${src}\nreturn tcOccGeo;`)(worldToScreen, () => 8, camS);
  const { rect, poly } = tcOccGeo(0, 0);
  // Diamond corners at (±8,±8): screen x span ±16*camS, top
  // corner y -8*camS raised a further 2.2*camS of wall.
  assert.equal(rect.w, 32 * camS);
  assert.equal(rect.y, 270 - 8 * camS - 2.2 * camS);
  assert.equal(rect.h, 16 * camS + 2.2 * camS);
  assert.equal(poly.length, 6);
});

// ------------------------------------------------------------------
// The mechanism the record prevents (behavioural; the REAL
// orderAt / entityAt / occludedUnitAt through the REAL
// projection, staged as in occordersweep.mjs: our TC at the
// origin, the enemy Scout settled 2.97 wu NW under its roof,
// our Scout selected).

const CS = 4, HALF = 60, GW = 30, GH = 30;
const ENEMY = [-2.1, -2.1];

function makeWorld({ record }) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, 0, 0, 2400);
  put(1, 2, 0, -17.4, 17.4, 45);
  put(7, 2, 2, ENEMY[0], ENEMY[1], 45);
  const st = { kinds, owners, pos, hp, count: 8, garrisoned: [] };
  const issued = [];
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
    selectedUnits: () => [1],
    selectedAnimal: null,
    selectedBuilding: null,
    issue: (cmd) => {
      const ok = cmd.t !== 'garrison_tc' && cmd.t !== 'garrison_u';
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
    KIND_RAM: 17, KIND_TC: 13, KIND_SHEEP: 5, KIND_RELIC: 21,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${sliceFn(appJs, 'function w2s(wx, wy, s, ox, oy) {', '\nfunction s2w')}\n` +
    `${sliceFn(appJs, 'function s2w(sx, sy, s, ox, oy) {', '\nfunction centerCamera')}\n` +
    `${sliceFn(appJs, 'function worldToCell(wx, wy) {', '\n// --- Placement ghost')}\n` +
    `${sliceFn(appJs, 'function heightAtWorld(wx, wy) {', '\nfunction fogAtWorld')}\n` +
    `${sliceFn(appJs, 'const worldToScreen = (wx, wy) => {', '\n};')}\n};\n` +
    `${sliceFn(appJs, 'function tcAt(wx, wy) {', '\n  function nodeAt')}\n` +
    `${sliceFn(appJs, 'function ramBodyD2(st, i, wx, wy) {', '\n  function unitAt')}\n` +
    `${sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick')}\n` +
    `${sliceFn(appJs, 'function entityAt(wx, wy, screenPick = false) {', '\n  // Placed-building hit test')}\n` +
    `${sliceFn(appJs, 'function orderAt(wx, wy, queued = false, fromCanvas = false) {', '\n  // Repair targeting')}\n` +
    `return { orderAt, occludedUnitAt, worldToScreen, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy), heightAtWorld };`,
  );
  const fns = factory(...Object.values(env));
  const [esx, esy] = fns.worldToScreen(ENEMY[0], ENEMY[1]);
  const unitRect = { x: esx - 1.4 * camS, y: esy - 3.05 * camS, w: 3.5 * camS, h: 3.2 * camS };
  if (record) {
    records.push({
      id: 7, rect: unitRect,
      cover: [{ x: 960 - 16 * camS, y: 540 - 10.2 * camS, w: 32 * camS, h: 18.2 * camS }],
      depth: ENEMY[0] + ENEMY[1],
    });
  }
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
  return { ...fns, eventWorld, issued, unitRect };
}

test('mechanism: with the record present, a click on the outline TOP issues the attack', () => {
  const w = makeWorld({ record: true });
  const sx = w.unitRect.x + w.unitRect.w / 2;
  const sy = w.unitRect.y + 1; // the rider's head — the farthest point from the ground anchor
  const [wx, wy] = w.eventWorld(sx, sy);
  assert.ok(Math.hypot(wx - ENEMY[0], wy - ENEMY[1]) > 2.2,
    'staging: the ground conversion sits outside the ground-pick radius (the record is the only path)');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['attack']);
  assert.equal(w.issued[0].tg, 7);
});

test('mechanism: with the record absent, the same click degrades to a ground move', () => {
  // This is the #421 failure shape, reproduced on the real
  // orderAt: the ordered unit walks to the click point and —
  // stance-gated auto-acquisition being the sim's side of the
  // story — a Stand Ground unit then idles there.
  const w = makeWorld({ record: false });
  const sx = w.unitRect.x + w.unitRect.w / 2;
  const sy = w.unitRect.y + 1;
  const [wx, wy] = w.eventWorld(sx, sy);
  assert.equal(w.occludedUnitAt(wx, wy), null, 'no record, no outline pick');
  w.orderAt(wx, wy, false, true);
  assert.deepEqual(w.issued.map((c) => c.t), ['move'],
    'the attack silently degrades to a ground move to the click point');
});
