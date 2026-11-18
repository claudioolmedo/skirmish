// OCCORDER SWEEP — client-side diagnosis of the occluded-outline
// attack degradation (bot-loop run #421, queue item 1).
//
// stanceatk proved the SIM executes an explicit Attack identically
// in every stance. The live failure (Stand Ground Scout walks to
// the red outline and idles, twice; Aggressive/Militia engage)
// therefore sits in the CLIENT order path: on the failing attempts
// orderAt must be issuing a ground MOVE instead of {t:'attack'}.
//
// The existing pins (occludedorder.test.mjs) drive the REAL
// orderAt/entityAt/occludedUnitAt but with an IDENTITY projection
// (worldToScreen = (wx,wy)=>[wx,wy]) and hand-placed world clicks.
// This sweep drives the same real functions through the REAL
// isometric projection: clicks start as SCREEN points on the
// painted outline, convert through the real eventWorld math
// (s2w + the two-step elevation-lift solve), and occludedUnitAt
// re-projects through the real worldToScreen (w2s - lift). Any
// place where the round trip is not the identity — elevation
// steps between the hidden unit's cell and the click's ground
// cell — ejects the re-projected point from the outline region
// (rect ∩ cover), occludedUnitAt returns null, the ground pick
// (radius 2.2 from a ground point displaced up to ~4.3 wu NW by
// the sprite's height) misses, and the click degrades to a move.
//
// Scene (the live geometry, stanceatk Geometry A): our TC at the
// origin; an enemy Scout settled 2.97 wu from its centre, hidden
// under the roof, visible only as its INF-12 outline; our Scout
// selected, ~24.6 wu away. The outline record is built exactly as
// drawOccludedOutlines builds it: unit rect = the cavalry sprite's
// dest box (CAVALRY_BOX at worldToScreen(pos)), cover = the TC's
// rect, depth = the painter key.
//
// Run: node play/occordersweep.mjs   (diagnostic; prints tables)
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { occludedPickAt } from './occlusion.js';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  if (a < 0) throw new Error(`marker not found: ${startMarker}`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  if (b <= a) throw new Error(`end marker not found after: ${startMarker}`);
  return src.slice(a, b);
}

// Real client code, sliced verbatim from app.js.
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

// ------------------------------------------------------------------
// World constants (sim: CS = 4 wu cells; TC footprint 4x4 cells).
const CS = 4, HALF = 60, GW = 30, GH = 30;
const ELEV_LIFT = 1.1;
const CAVALRY_BOX = { x0: -1.4, y0: -3.05, w: 3.5, h: 3.2 }; // app.js:11418
const TC_POS = [0, 0];
const ATTACKER_POS = [-17.4, 17.4];

function heightsFor(mode) {
  const H = new Array(GW * GH).fill(0);
  if (mode === 'flat') return H;
  const lvl = mode === 'plateau2' ? 2 : 1;
  const inside = mode !== 'inv1';
  for (let gy = 0; gy < GH; gy++) {
    for (let gx = 0; gx < GW; gx++) {
      const cx = -HALF + (gx + 0.5) * CS;
      const cy = -HALF + (gy + 0.5) * CS;
      const inFoot = Math.abs(cx - TC_POS[0]) <= 8 && Math.abs(cy - TC_POS[1]) <= 8;
      H[gy * GW + gx] = (inFoot === inside) ? lvl : (inside ? 0 : lvl);
      if (mode === 'inv1') H[gy * GW + gx] = inFoot ? 0 : 1;
    }
  }
  return H;
}

// Build one scenario world: real sliced functions + scenario env.
function makeWorld({ camS, heightsMode, enemyPos, recordMode, enemyVisible }) {
  const camOx = 960, camOy = 540; // centerCamera(0,0) on 1920x1080
  const HEIGHTS = heightsFor(heightsMode);
  const kinds = [], owners = [], pos = [], hp = [];
  const put = (i, kind, owner, x, y, h) => {
    kinds[i] = kind; owners[i] = owner;
    pos[i * 2] = x; pos[i * 2 + 1] = y; hp[i * 2] = h; hp[i * 2 + 1] = h;
  };
  put(0, 13, 0, TC_POS[0], TC_POS[1], 2400);
  put(1, 2, 0, ATTACKER_POS[0], ATTACKER_POS[1], 45);
  put(7, 2, 2, enemyPos[0], enemyPos[1], 45);
  const st = { kinds, owners, pos, hp, count: 8, garrisoned: [] };
  const issued = []; // ACCEPTED commands only (issue() returned true)
  const attempts = []; // every command orderAt tried to issue
  const records = []; // mutated in place; the factory closes over it

  const env = {
    // projection env (real sliced functions read these)
    camS, camOx, camOy, ELEV_LIFT, HEIGHTS, GW, GH, HALF, CS,
    // scene state
    entityState: () => st,
    lastOccludedOutlines: records,
    occludedPickAt,
    cellVisibleW: (wx, wy) => (wx === enemyPos[0] && wy === enemyPos[1] ? enemyVisible : true),
    cellExploredW: () => true,
    buildingAt: () => null,
    nodeAt: () => null,
    treeCanopyAt: () => null,
    selectedUnits: () => [1],
    selectedAnimal: null,
    selectedBuilding: null,
    // Faithful sim verdicts for THIS scene (stanceatk probe +
    // garrison rules): the attack is accepted; a Scout Cavalry is
    // mounted and a TC admits foot kinds only, so garrison_tc is
    // refused and the click falls through — as in the live game.
    issue: (cmd) => {
      attempts.push(cmd);
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
    `${w2sSrc}\n${s2wSrc}\n${worldToCellSrc}\n${heightAtWorldSrc}\n${worldToScreenSrc}\n` +
    `${tcAtSrc}\n${ramBodySrc}\n${occSrc}\n${entityAtSrc}\n${orderAtSrc}\n` +
    `return { orderAt, entityAt, occludedUnitAt, worldToScreen, heightAtWorld, worldToCell, s2w: (sx, sy) => s2w(sx, sy, camS, camOx, camOy) };`,
  );
  const fns = factory(...Object.values(env));

  // The real eventWorld math (app.js eventWorld, DOM part removed:
  // it works in canvas device px, exactly our screen points).
  const eventWorld = (sx, sy) => {
    let [wx, wy] = fns.s2w(sx, sy);
    const h = fns.heightAtWorld(wx, wy);
    if (h > 0) {
      const s2 = (x, y) => fns.s2w(x, y);
      [wx, wy] = s2(sx, sy + h * ELEV_LIFT * camS);
      const h2 = fns.heightAtWorld(wx, wy);
      if (h2 !== h) [wx, wy] = s2(sx, sy + h2 * ELEV_LIFT * camS);
    }
    return [wx, wy];
  };

  // The outline record, built as drawOccludedOutlines builds it.
  const [esx, esy] = fns.worldToScreen(enemyPos[0], enemyPos[1]);
  const unitRect = {
    x: esx + CAVALRY_BOX.x0 * camS, y: esy + CAVALRY_BOX.y0 * camS,
    w: CAVALRY_BOX.w * camS, h: CAVALRY_BOX.h * camS,
  };
  const [tsx, tsy] = fns.worldToScreen(TC_POS[0], TC_POS[1]);
  // TC cover rect: the vector TC's bounding box (the larger of the
  // two draw paths — art sprite 17.92 wu wide vs vector diamond
  // 32 wu wide). Staging asserts the unit rect is fully inside
  // either, so rect ∩ cover == unit rect and the choice is inert.
  const coverRect = { x: tsx - 16 * camS, y: tsy - 10.2 * camS, w: 32 * camS, h: 18.2 * camS };
  if (recordMode === 'present') {
    records.push({ id: 7, rect: unitRect, cover: [coverRect], depth: enemyPos[0] + enemyPos[1] });
  }
  return { ...fns, eventWorld, issued, attempts, unitRect, coverRect, records, enemyPos };
}

function classify(issued) {
  if (!issued.length) return 'none';
  const atk = issued.find((c) => c.t === 'attack');
  if (atk) return atk.tg === 7 ? 'attack7' : `attack${atk.tg}`;
  return issued[0].t; // move / garrison_tc / ...
}

// Sweep A: screen clicks on a grid over the painted outline
// region (unit rect ∩ cover == unit rect, asserted in staging).
function sweepOutline(w, { fromCanvas = true } = {}) {
  const r = w.unitRect;
  const step = Math.max(2, wStep(r));
  function wStep() { return 2; }
  const rows = [];
  const tally = {};
  let n = 0;
  for (let sy = r.y + step / 2; sy < r.y + r.h; sy += step) {
    for (let sx = r.x + step / 2; sx < r.x + r.w; sx += step) {
      n++;
      const [wx, wy] = w.eventWorld(sx, sy);
      const ent = w.entityAt(wx, wy, fromCanvas);
      const occ = w.occludedUnitAt(wx, wy);
      w.issued.length = 0;
      w.attempts.length = 0;
      w.orderAt(wx, wy, false, fromCanvas);
      const cmd = classify([...w.issued]);
      tally[cmd] = (tally[cmd] || 0) + 1;
      rows.push({
        sx, sy, wx, wy, ent, occ,
        tFromOutline: ent != null && fromCanvas && occ === ent,
        groundDist: Math.hypot(wx - w.enemyPos[0], wy - w.enemyPos[1]),
        cmd, issued: [...w.issued],
      });
    }
  }
  return { n, tally, rows };
}

// Sweep B: clicks aimed at the enemy's GROUND disc (screen point
// = worldToScreen of the ground point) — clicking the TC centre
// area / the enemy's feet through the roof.
function sweepDisc(w) {
  const tally = {};
  const bandIn = {}; // ground points within entityAt's 2.2 radius
  const bandOut = {}; // ground points beyond it
  let n = 0;
  for (let dy = -2.6; dy <= 2.6; dy += 0.4) {
    for (let dx = -2.6; dx <= 2.6; dx += 0.4) {
      if (Math.hypot(dx, dy) > 2.6) continue;
      const gx = w.enemyPos[0] + dx, gy = w.enemyPos[1] + dy;
      const [sx, sy] = w.worldToScreen(gx, gy);
      const [wx, wy] = w.eventWorld(sx, sy);
      w.issued.length = 0;
      w.attempts.length = 0;
      w.orderAt(wx, wy, false, true);
      const cmd = classify([...w.issued]);
      tally[cmd] = (tally[cmd] || 0) + 1;
      const band = Math.hypot(dx, dy) <= 2.2 ? bandIn : bandOut;
      band[cmd] = (band[cmd] || 0) + 1;
      n++;
    }
  }
  return { n, tally, bandIn, bandOut };
}

const ANGLES = { '225 (live NW)': [-2.1, -2.1], '45 (SE)': [2.1, 2.1], '0 (E)': [2.97, 0], '90 (S)': [0, 2.97] };
const summary = [];

// Staging sanity on the primary scenario.
{
  const w = makeWorld({ camS: 8, heightsMode: 'flat', enemyPos: ANGLES['225 (live NW)'], recordMode: 'present', enemyVisible: true });
  const r = w.unitRect, c = w.coverRect;
  const inside = r.x >= c.x && r.y >= c.y && r.x + r.w <= c.x + c.w && r.y + r.h <= c.y + c.h;
  console.log(`STAGING unitRect=${JSON.stringify(r)} fullyInsideCover=${inside}`);
  const [cx, cy] = [r.x + r.w / 2, r.y + r.h / 2];
  console.log(`STAGING centre click occ=${w.occludedUnitAt(...w.eventWorld(cx, cy))} (want 7)`);
}

for (const camS of [8, 24]) {
  for (const heightsMode of ['flat', 'plateau1', 'plateau2', 'inv1']) {
    for (const [aname, epos] of Object.entries(ANGLES)) {
      if (camS === 24 && heightsMode !== 'flat' && heightsMode !== 'plateau1') continue;
      if (camS === 24 && aname !== '225 (live NW)') continue;
      const w = makeWorld({ camS, heightsMode, enemyPos: epos, recordMode: 'present', enemyVisible: true });
      const { n, tally, rows } = sweepOutline(w);
      const moves = rows.filter((x) => x.cmd === 'move');
      const causes = {};
      for (const m of moves) {
        const cause = m.occ === null
          ? (m.ent === null ? 'occMiss+groundMiss' : m.ent === 7 ? 'occMiss+groundHit7(fogGate?)' : `occMiss+groundOther(${m.ent})`)
          : 'occHitButMove';
        causes[cause] = (causes[cause] || 0) + 1;
      }
      summary.push({ scen: `record=present camS=${camS} h=${heightsMode} ang=${aname}`, n, tally, causes });
      if (moves.length) {
        const m = moves[0];
        summary[summary.length - 1].firstMove = {
          screen: [m.sx.toFixed(1), m.sy.toFixed(1)], ground: [m.wx.toFixed(2), m.wy.toFixed(2)],
          groundDist: m.groundDist.toFixed(2), ent: m.ent, occ: m.occ,
          reprojErr: (() => { const [rx, ry] = w.worldToScreen(m.wx, m.wy); return [+(rx - m.sx).toFixed(1), +(ry - m.sy).toFixed(1)]; })(),
        };
      }
    }
  }
}

// Record absent (stale/missing frame) + minimap + disc sweeps.
for (const heightsMode of ['flat', 'plateau1']) {
  const w = makeWorld({ camS: 8, heightsMode, enemyPos: ANGLES['225 (live NW)'], recordMode: 'absent', enemyVisible: true });
  const { n, tally } = sweepOutline(w);
  summary.push({ scen: `record=ABSENT camS=8 h=${heightsMode} ang=225`, n, tally });
}
{
  const w = makeWorld({ camS: 8, heightsMode: 'flat', enemyPos: ANGLES['225 (live NW)'], recordMode: 'absent', enemyVisible: false });
  const { n, tally } = sweepOutline(w);
  summary.push({ scen: 'record=ABSENT camS=8 h=flat ang=225 enemyFog=hidden', n, tally });
}
{
  const w = makeWorld({ camS: 8, heightsMode: 'flat', enemyPos: ANGLES['225 (live NW)'], recordMode: 'present', enemyVisible: true });
  const { n, tally } = sweepOutline(w, { fromCanvas: false });
  summary.push({ scen: 'MINIMAP path (fromCanvas=false) record=present h=flat', n, tally });
}
for (const heightsMode of ['flat', 'plateau1']) {
  const w = makeWorld({ camS: 8, heightsMode, enemyPos: ANGLES['225 (live NW)'], recordMode: 'present', enemyVisible: true });
  const { n, tally, bandIn, bandOut } = sweepDisc(w);
  summary.push({ scen: `DISC sweep record=present h=${heightsMode} (inR=${JSON.stringify(bandIn)} outR=${JSON.stringify(bandOut)})`, n, tally });
}

for (const s of summary) {
  const pct = (k) => (s.tally[k] ? `${((100 * s.tally[k]) / s.n).toFixed(1)}%` : '—');
  console.log(`\n${s.scen}\n  n=${s.n} attack7=${pct('attack7')} move=${pct('move')} other=${JSON.stringify(s.tally)}`);
  if (s.causes && Object.keys(s.causes).length) console.log(`  move causes: ${JSON.stringify(s.causes)}`);
  if (s.firstMove) console.log(`  first move: ${JSON.stringify(s.firstMove)}`);
}
