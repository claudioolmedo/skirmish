// SELECTION SWEEP — left-click selection parity diagnostic
// (bot-loop selpick, 2026-11-18). Companion to
// play/selpick.test.mjs: sweeps a 2 px grid over the enemy
// Scout's full dest rect (182 clicks, the occorder sweeps'
// grid) and classifies the plain-click handler's outcome —
// select our coincident Scout (units 1), info-select the enemy
// (animal 7), or anything else (TC / clear) — through the REAL
// sliced picks (unitAt / animalAt / occludedUnitAt / tcAt, and
// selectionAt when the tree carries FIX #4) and the REAL
// isometric projection. Run it against a pristine tree for the
// "before" numbers and against the fixed tree for "after":
//
//   node play/selpicksweep.mjs
//
// Scenes (enemy Scout id 7 at the TC east porch (6,-6), our TC
// at the origin): A record present + friendly coincident (the
// occorder2 H1 scene, measured 44.0% own / 56.0% enemy there);
// B no record + friendly coincident; C no record + friendly far.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { occludedPickAt } from './occlusion.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(src, a, b) {
  const i = src.indexOf(a);
  if (i < 0) throw new Error(`marker not found: ${a}`);
  const j = src.indexOf(b, i + a.length);
  if (j <= i) throw new Error(`end marker not found after: ${a}`);
  return src.slice(i, j);
}
function trySlice(src, a, b) {
  const i = src.indexOf(a);
  if (i < 0) return null;
  const j = src.indexOf(b, i + a.length);
  return j > i ? src.slice(i, j) : null;
}

const w2sSrc = sliceFn(appJs, 'function w2s(wx, wy, s, ox, oy) {', '\nfunction s2w');
const s2wSrc = sliceFn(appJs, 'function s2w(sx, sy, s, ox, oy) {', '\nfunction centerCamera');
const worldToCellSrc = sliceFn(appJs, 'function worldToCell(wx, wy) {', '\n// --- Placement ghost');
const heightAtWorldSrc = sliceFn(appJs, 'function heightAtWorld(wx, wy) {', '\nfunction fogAtWorld');
const worldToScreenSrc = sliceFn(appJs, 'const worldToScreen = (wx, wy) => {', '\n};') + '\n};';
const tcAtSrc = sliceFn(appJs, 'function tcAt(wx, wy) {', '\n  function nodeAt');
const ramBodySrc = sliceFn(appJs, 'function ramBodyD2(st, i, wx, wy) {', '\n  function unitAt');
const unitAtSrc = sliceFn(appJs, 'function unitAt(wx, wy) {', '\n  // Left-click pick for an animal');
const animalAtSrc = sliceFn(appJs, 'function animalAt(wx, wy) {', '\n  // Occluded-outline pick');
const occSrc = sliceFn(appJs, 'function occludedUnitAt(wx, wy) {', '\n  // Right-click target pick');
const selectionAtSrc = trySlice(appJs, 'function selectionAt(wx, wy) {', '\n  // Left-click pick for an animal');

const CS = 4, HALF = 60, GW = 30, GH = 30;
const CAVALRY_BOX = { x0: -1.4, y0: -3.05, w: 3.5, h: 3.2 };
const ENEMY = [6, -6];
const FAR = [-17.4, 17.4];
const KIND_TC = 1, KIND_RAM = 17, KIND_SHEEP = 5, KIND_RELIC = 21;

function makeWorld({ friendlyPos, record }) {
  const camS = 8, camOx = 960, camOy = 540;
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, KIND_TC, 0, 0, 0, 1909);
  put(1, 2, 0, friendlyPos[0], friendlyPos[1], 45);
  put(7, 2, 2, ENEMY[0], ENEMY[1], 45);
  const st = { kinds, owners, pos, hp, count: 8, garrisoned: [] };
  const records = [];
  const isOwnUnit = (s, i) =>
    s.owners[i] === 0 && s.kinds[i] !== KIND_TC && s.hp[i * 2] >= 1
    && !(s.garrisoned && s.garrisoned[i]);
  const env = {
    camS, camOx, camOy, ELEV_LIFT: 1.1,
    HEIGHTS: new Array(GW * GH).fill(0), GW, GH, HALF, CS,
    entityState: () => st, isOwnUnit,
    lastOccludedOutlines: records, occludedPickAt,
    cellVisibleW: () => true, localOwner: 0,
    sim: { entity_kinds: () => kinds, positions: () => pos },
    tcHalf: () => 8, KIND_RAM, KIND_TC, KIND_SHEEP, KIND_RELIC,
    RAM_BODY_H: 3.0, RAM_BODY_HALF_W: 1.6,
  };
  const factory = new Function(
    ...Object.keys(env),
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${unitAtSrc}\n${animalAtSrc}\n${occSrc}\n${selectionAtSrc || ''}\n` +
    `return { unitAt, animalAt, occludedUnitAt, tcAt, worldToScreen, ` +
    `selectionAt: typeof selectionAt === 'function' ? selectionAt : null, ` +
    `s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
  );
  const fns = factory(...Object.values(env));
  if (record) {
    const [esx, esy] = fns.worldToScreen(ENEMY[0], ENEMY[1]);
    const [tsx, tsy] = fns.worldToScreen(0, 0);
    records.push({
      id: 7,
      rect: { x: esx + CAVALRY_BOX.x0 * camS, y: esy + CAVALRY_BOX.y0 * camS, w: CAVALRY_BOX.w * camS, h: CAVALRY_BOX.h * camS },
      cover: [{ x: tsx - 16 * camS, y: tsy - 10.2 * camS, w: 32 * camS, h: 18.2 * camS }],
      depth: ENEMY[0] + ENEMY[1],
    });
  }
  const leftClick = (wx, wy) => {
    if (fns.selectionAt) {
      const v = fns.selectionAt(wx, wy);
      if (v != null && isOwnUnit(st, v)) return { type: 'units', id: v };
      if (v != null) return { type: 'animal', id: v };
    } else {
      const v = fns.unitAt(wx, wy);
      if (v != null) return { type: 'units', id: v };
    }
    const occ = fns.occludedUnitAt(wx, wy);
    if (occ != null) return isOwnUnit(st, occ) ? { type: 'units', id: occ } : { type: 'animal', id: occ };
    const a = fns.animalAt(wx, wy);
    if (a != null) return { type: 'animal', id: a };
    const tc = fns.tcAt(wx, wy);
    if (tc != null) return { type: 'tc', id: tc };
    return { type: 'none' };
  };
  return { ...fns, leftClick };
}

const hasFix = !!selectionAtSrc;
console.log(`tree: ${hasFix ? 'FIXED (selectionAt present)' : 'PRISTINE (no selectionAt)'}`);
for (const [name, opts] of [
  ['A record present, friendly COINCIDENT', { friendlyPos: ENEMY, record: true }],
  ['B no record,      friendly COINCIDENT', { friendlyPos: ENEMY, record: false }],
  ['C no record,      friendly FAR      ', { friendlyPos: FAR, record: false }],
]) {
  const w = makeWorld(opts);
  const [esx, esy] = w.worldToScreen(ENEMY[0], ENEMY[1]);
  const tally = { own: 0, enemy: 0, other: 0 };
  let n = 0;
  for (let px = 0; px < CAVALRY_BOX.w * 8; px += 2) {
    for (let py = 0; py < CAVALRY_BOX.h * 8; py += 2) {
      const sx = esx + CAVALRY_BOX.x0 * 8 + px;
      const sy = esy + CAVALRY_BOX.y0 * 8 + py;
      const [wx, wy] = w.s2w(sx, sy);
      const r = w.leftClick(wx, wy);
      n++;
      if (r.type === 'units' && r.id === 1) tally.own++;
      else if (r.type === 'animal' && r.id === 7) tally.enemy++;
      else tally.other++;
    }
  }
  const pct = (k) => `${(100 * tally[k] / n).toFixed(1)}%`;
  console.log(`${name}: n=${n}  enemy info ${pct('enemy')}  own select ${pct('own')}  other (TC/clear) ${pct('other')}`);
}
