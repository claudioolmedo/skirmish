// Skirmish M2 debug preview — dependency-free Canvas 2D view of the sim.
//
// This is a THROWAWAY debug slice, not the M4 renderer. It renders the
// sim with a 2D isometric camera in the Age of Empires style: flat 2D
// sprites (programmer art with baked-in two-tone shading and ground
// shadows) on a fixed 2:1 dimetric projection, painted back-to-front.
// No real 3D, no rotation — the same technique as the classic engine.
// Its job is to make the M2 economy loop visible: stockpile HUD,
// resource nodes, the Town Center, villagers gathering/hauling/
// depositing, and the live state hash as a determinism proof.

import init, { WasmSim } from './pkg/skirmish_sim.js';

const params = new URLSearchParams(location.search);
const seed = parseInt(params.get('seed') || '7', 10);

const canvas = document.getElementById('view');
const ctx = canvas.getContext('2d');
const hud = document.getElementById('hud');
const trainBtn = document.getElementById('train');

const COLORS = [
  '#e74c3c', '#3498db', '#2ecc71', '#f1c40f',
  '#9b59b6', '#1abc9c', '#e67e22', '#ecf0f1',
  '#ff7f50', '#7bed9f', '#70a1ff', '#ffa502',
  '#eccc68', '#ff6b81', '#a4b0be', '#57606f',
];

// Node kind tags (must match the sim's NodeKind::tag): 0 tree, 1 berry,
// 2 gold, 3 stone. MAX_AMOUNT is each kind's starting amount in the sim
// (for the remaining-amount bar); CARRY_COLORS is the load dot shown
// above a hauling villager.
const NODE_MAX = [100, 125, 800, 350];
const CARRY_COLORS = ['#d9a05b', '#ff5da2', '#ffd700', '#c3c8cd'];
const KIND_TC = 1; // entity kind tag: 0 villager, 1 town center

// --- Isometric camera -------------------------------------------------
// Fixed angle, 2:1 dimetric. Pure functions of explicit parameters so
// the projection math can be unit-tested headlessly in Node.
const INITIAL_S = 5; // screen px per iso unit at load
const S_MIN = 1.2;
const S_MAX = 24;
let camS = INITIAL_S;
let camOx = 0;
let camOy = 0;

function w2s(wx, wy, s, ox, oy) {
  const ix = wx - wy;
  const iy = (wx + wy) / 2;
  return [ox + ix * s, oy + iy * s];
}
function s2w(sx, sy, s, ox, oy) {
  const ix = (sx - ox) / s;
  const iy = (sy - oy) / s;
  return [iy + ix / 2, iy - ix / 2];
}
function centerCamera(wx, wy, s, w, h) {
  return [w / 2 - (wx - wy) * s, h / 2 - ((wx + wy) / 2) * s];
}
const worldToScreen = (wx, wy) => w2s(wx, wy, camS, camOx, camOy);

let sim, HALF, GW, GH, CS, OBST;

// Node data is a flat array: [id, x, y, kind, amount_left] per node.
function nodeList() {
  const flat = sim.nodes();
  const out = [];
  for (let i = 0; i < flat.length; i += 5) {
    out.push({
      id: flat[i], x: flat[i + 1], y: flat[i + 2],
      kind: flat[i + 3], amount: flat[i + 4],
    });
  }
  return out;
}

function villagerIds() {
  const kinds = sim.entity_kinds();
  const ids = [];
  for (let i = 0; i < kinds.length; i++) {
    if (kinds[i] !== KIND_TC) ids.push(i); // entities are id-ordered
  }
  return ids;
}

// --- Carry indicator (client-side heuristic) --------------------------
// The frozen WASM ABI exposes no carry state, so the load dot is
// inferred: a villager seen at a node is marked with that node's kind;
// once it leaves the node's vicinity it is shown carrying that kind
// until it reaches the Town Center (deposit). This matches the sim's
// semantics — a Move override keeps the carry, only deposit clears it.
const AT_NODE_R2 = 2.6 * 2.6;
const DEPOSIT_R2 = 3.6 * 3.6;
const lastNodeKind = new Map();
const carryKind = new Map();

function updateCarry(nodes, tcPos, pos, kinds) {
  for (let i = 0; i < kinds.length; i++) {
    if (kinds[i] === KIND_TC) continue;
    const wx = pos[i * 2];
    const wy = pos[i * 2 + 1];
    let atKind = null;
    for (const n of nodes) {
      if (n.amount <= 0) continue;
      const dx = n.x - wx;
      const dy = n.y - wy;
      if (dx * dx + dy * dy <= AT_NODE_R2) {
        atKind = n.kind;
        break;
      }
    }
    if (atKind !== null) {
      lastNodeKind.set(i, atKind);
    } else if (lastNodeKind.has(i) && !carryKind.has(i)) {
      carryKind.set(i, lastNodeKind.get(i)); // left the node with a load
    }
    if (tcPos) {
      const dx = tcPos[0] - wx;
      const dy = tcPos[1] - wy;
      if (dx * dx + dy * dy <= DEPOSIT_R2) {
        carryKind.delete(i);
        lastNodeKind.delete(i);
      }
    }
  }
}

// --- Input: pan / zoom -------------------------------------------------
const keys = new Set();
addEventListener('keydown', (e) => {
  if (e.code.startsWith('Arrow')) e.preventDefault();
  keys.add(e.code);
});
addEventListener('keyup', (e) => keys.delete(e.code));

function applyKeys() {
  const P = 14; // screen px per frame while a pan key is held
  if (keys.has('KeyA') || keys.has('ArrowLeft')) camOx += P;
  if (keys.has('KeyD') || keys.has('ArrowRight')) camOx -= P;
  if (keys.has('KeyW') || keys.has('ArrowUp')) camOy += P;
  if (keys.has('KeyS') || keys.has('ArrowDown')) camOy -= P;
}

async function main() {
  await init();
  sim = new WasmSim(seed);
  HALF = sim.world_half();
  GW = sim.grid_width();
  GH = sim.grid_height();
  CS = sim.cell_size();
  OBST = sim.obstacles();

  // Initial camera: centered on the Town Center.
  {
    const kinds = sim.entity_kinds();
    const pos = sim.positions();
    const tc = kinds.indexOf(KIND_TC);
    const [ox, oy] = centerCamera(
      pos[tc * 2], pos[tc * 2 + 1], camS, canvas.width, canvas.height);
    camOx = ox;
    camOy = oy;
  }

  trainBtn.addEventListener('click', () => {
    sim.push_train_villager();
  });

  canvas.addEventListener('click', (ev) => {
    const r = canvas.getBoundingClientRect();
    const sx = (ev.clientX - r.left) * (canvas.width / r.width);
    const sy = (ev.clientY - r.top) * (canvas.height / r.height);
    const [wx, wy] = s2w(sx, sy, camS, camOx, camOy);
    // Click near a live node (within 4 world units): gather it with all
    // villagers. Otherwise: plain move order for all villagers.
    let best = null;
    let bestD2 = 4 * 4;
    for (const n of nodeList()) {
      if (n.amount <= 0) continue;
      const d2 = (n.x - wx) * (n.x - wx) + (n.y - wy) * (n.y - wy);
      if (d2 < bestD2) {
        bestD2 = d2;
        best = n;
      }
    }
    for (const id of villagerIds()) {
      if (best) sim.push_gather(id, best.id);
      else sim.push_move(id, wx, wy);
    }
  });

  // Zoom toward the cursor with the mouse wheel.
  canvas.addEventListener('wheel', (ev) => {
    ev.preventDefault();
    const r = canvas.getBoundingClientRect();
    const mx = (ev.clientX - r.left) * (canvas.width / r.width);
    const my = (ev.clientY - r.top) * (canvas.height / r.height);
    const ns = Math.min(S_MAX, Math.max(S_MIN, camS * Math.exp(-ev.deltaY * 0.0012)));
    camOx = mx - (mx - camOx) * (ns / camS);
    camOy = my - (my - camOy) * (ns / camS);
    camS = ns;
  }, { passive: false });

  // Pan by dragging with the right mouse button.
  let drag = null;
  canvas.addEventListener('mousedown', (e) => {
    if (e.button === 2) drag = [e.clientX, e.clientY];
  });
  addEventListener('mousemove', (e) => {
    if (!drag) return;
    const r = canvas.getBoundingClientRect();
    const f = canvas.width / r.width;
    camOx += (e.clientX - drag[0]) * f;
    camOy += (e.clientY - drag[1]) * f;
    drag = [e.clientX, e.clientY];
  });
  addEventListener('mouseup', (e) => {
    if (e.button === 2) drag = null;
  });
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());

  // Opening demo orders so the economy starts moving immediately:
  // two villagers on the nearest berries, one on the nearest tree.
  const nodes = nodeList();
  const berry = nodes.find((n) => n.kind === 1);
  const tree = nodes.find((n) => n.kind === 0);
  const vils = villagerIds();
  if (berry && vils.length > 1) {
    sim.push_gather(vils[0], berry.id);
    sim.push_gather(vils[1], berry.id);
  }
  if (tree && vils.length > 2) {
    sim.push_gather(vils[2], tree.id);
  }

  // Fixed 60 Hz tick loop decoupled from rAF, with a max-ticks-per-frame
  // guard: on a slow machine the preview slows down visually, never
  // logically (same rule as the real game).
  let acc = 0;
  let last = performance.now();
  const STEP = 1000 / 60;
  function frame(now) {
    acc += Math.min(now - last, 250);
    last = now;
    let n = 0;
    while (acc >= STEP && n < 5) {
      sim.tick();
      acc -= STEP;
      n++;
    }
    if (n === 5) acc = 0;
    applyKeys();
    draw();
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

// --- Drawing helpers ----------------------------------------------------
function poly(pts, fill) {
  ctx.fillStyle = fill;
  ctx.beginPath();
  ctx.moveTo(pts[0][0], pts[0][1]);
  for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]);
  ctx.closePath();
  ctx.fill();
}

// Diamond centered at (cx, cy) with half-width hw and half-height hh.
function diamond(cx, cy, hw, hh, fill) {
  poly([[cx, cy - hh], [cx + hw, cy], [cx, cy + hh], [cx - hw, cy]], fill);
}

function shadow(sx, sy, rx) {
  ctx.fillStyle = 'rgba(0,0,0,0.28)';
  ctx.beginPath();
  ctx.ellipse(sx, sy, rx, rx / 2, 0, 0, Math.PI * 2);
  ctx.fill();
}

const up = (p, h) => [p[0], p[1] - h];

// Raised stone block on a blocked cell (AoE-style wall sprite: one flat
// shape — light top, medium left face, dark right face).
const BLOCK_H = 2; // screen px of height per unit of camS
function drawBlock(wx0, wy0) {
  const t = worldToScreen(wx0, wy0);
  const r = worldToScreen(wx0 + CS, wy0);
  const b = worldToScreen(wx0 + CS, wy0 + CS);
  const l = worldToScreen(wx0, wy0 + CS);
  const h = BLOCK_H * camS;
  poly([l, b, up(b, h), up(l, h)], '#7d838d'); // left face
  poly([b, r, up(r, h), up(b, h)], '#5b6067'); // right face
  poly([up(t, h), up(r, h), up(b, h), up(l, h)], '#b9bdc4'); // top
}

// --- Resource node sprites ----------------------------------------------
function amountBar(sx, sy, frac) {
  const w = 3 * camS;
  const h = Math.max(2, camS * 0.55);
  const x = sx - w / 2;
  const y = sy + 0.6 * camS;
  ctx.fillStyle = 'rgba(0,0,0,0.55)';
  ctx.fillRect(x - 1, y - 1, w + 2, h + 2);
  ctx.fillStyle = '#20261c';
  ctx.fillRect(x, y, w, h);
  ctx.fillStyle = frac > 0.3 ? '#7dd87d' : '#e0a13c';
  ctx.fillRect(x, y, w * Math.max(0, Math.min(1, frac)), h);
}

function drawTree(sx, sy) {
  const u = camS;
  shadow(sx, sy, 1.5 * u);
  ctx.fillStyle = '#6b4a2b'; // trunk
  ctx.fillRect(sx - 0.18 * u, sy - 1.5 * u, 0.36 * u, 1.5 * u);
  ctx.fillStyle = '#256b2e'; // canopy, dark side
  ctx.beginPath();
  ctx.arc(sx - 0.55 * u, sy - 1.85 * u, 1.15 * u, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#2e8b3a'; // canopy, main
  ctx.beginPath();
  ctx.arc(sx + 0.35 * u, sy - 2.1 * u, 1.35 * u, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#3aa544'; // canopy, lit top
  ctx.beginPath();
  ctx.arc(sx + 0.05 * u, sy - 2.55 * u, 0.8 * u, 0, Math.PI * 2);
  ctx.fill();
}

function drawBerry(sx, sy) {
  const u = camS;
  shadow(sx, sy, 1.4 * u);
  const blob = (dx, dy, rw, rh, fill) => {
    ctx.fillStyle = fill;
    ctx.beginPath();
    ctx.ellipse(sx + dx, sy + dy, rw, rh, 0, 0, Math.PI * 2);
    ctx.fill();
  };
  blob(-0.7 * u, -0.4 * u, 0.95 * u, 0.6 * u, '#a1235e');
  blob(0.7 * u, -0.35 * u, 0.9 * u, 0.55 * u, '#b52a68');
  blob(0, -0.7 * u, 1.0 * u, 0.62 * u, '#d63384');
  ctx.fillStyle = '#ff8fc0'; // berries
  for (const [dx, dy] of [[-0.5, -0.75], [0.3, -0.95], [0.75, -0.5]]) {
    ctx.beginPath();
    ctx.arc(sx + dx * u, sy + dy * u, 0.16 * u, 0, Math.PI * 2);
    ctx.fill();
  }
}

function drawGold(sx, sy) {
  const u = camS;
  shadow(sx, sy, 1.3 * u);
  // Angular crystals: small diamonds in three gold tones.
  diamond(sx - 0.7 * u, sy - 0.45 * u, 0.5 * u, 0.55 * u, '#c78f00');
  diamond(sx + 0.1 * u, sy - 0.8 * u, 0.65 * u, 0.85 * u, '#ffd700');
  diamond(sx + 0.8 * u, sy - 0.4 * u, 0.45 * u, 0.5 * u, '#e0a800');
  diamond(sx + 0.1 * u, sy - 1.05 * u, 0.22 * u, 0.3 * u, '#fff3a0');
}

function drawStone(sx, sy) {
  const u = camS;
  shadow(sx, sy, 1.35 * u);
  poly([
    [sx - 0.95 * u, sy], [sx - 0.55 * u, sy - 0.8 * u],
    [sx + 0.1 * u, sy - 1.0 * u], [sx + 0.55 * u, sy - 0.55 * u],
    [sx + 0.7 * u, sy],
  ], '#8a8f98');
  poly([
    [sx + 0.25 * u, sy], [sx + 0.6 * u, sy - 0.6 * u],
    [sx + 1.05 * u, sy - 0.45 * u], [sx + 1.15 * u, sy],
  ], '#a7adb5');
  poly([
    [sx - 0.55 * u, sy - 0.8 * u], [sx + 0.1 * u, sy - 1.0 * u],
    [sx + 0.3 * u, sy - 0.7 * u], [sx - 0.25 * u, sy - 0.62 * u],
  ], '#c3c8cd');
}

const NODE_DRAW = [drawTree, drawBerry, drawGold, drawStone];

// --- Building / unit sprites ----------------------------------------------
const TC_HALF = 3.6; // world units: TC footprint half-extent
function drawTC(wx, wy) {
  const a = TC_HALF;
  const t = worldToScreen(wx - a, wy - a);
  const r = worldToScreen(wx + a, wy - a);
  const b = worldToScreen(wx + a, wy + a);
  const l = worldToScreen(wx - a, wy + a);
  const [sx, sy] = worldToScreen(wx, wy);
  shadow(sx, sy, (r[0] - l[0]) / 2);
  const h = 2.2 * camS; // wall height
  poly([l, b, up(b, h), up(l, h)], '#b09468'); // front-left wall
  poly([b, r, up(r, h), up(b, h)], '#8a6f45'); // front-right wall
  poly([up(t, h), up(r, h), up(b, h), up(l, h)], '#9c4a2f'); // roof
  // Raised roof cap, for a hip-roof hint.
  const cx = (t[0] + b[0]) / 2;
  const cy = (t[1] + b[1]) / 2 - h - 0.7 * camS;
  diamond(cx, cy, (r[0] - l[0]) / 4, (r[0] - l[0]) / 8, '#b85c38');
  ctx.fillStyle = '#ffe9c9';
  ctx.font = `bold ${Math.max(9, Math.round(1.1 * camS))}px monospace`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText('TC', cx, cy);
}

function drawVillager(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 0.95 * u);
  const by = sy - 1.15 * u; // body floats just above its ground point
  ctx.fillStyle = COLORS[i % COLORS.length];
  ctx.beginPath();
  ctx.arc(sx, by, 0.85 * u, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = 'rgba(0,0,0,0.4)';
  ctx.lineWidth = 1;
  ctx.stroke();
  const ck = carryKind.get(i);
  if (ck !== undefined) {
    ctx.fillStyle = CARRY_COLORS[ck];
    ctx.beginPath();
    ctx.arc(sx, sy - 2.35 * u, 0.4 * u, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = 'rgba(0,0,0,0.35)';
    ctx.stroke();
  }
  ctx.fillStyle = '#101010';
  ctx.font = `${Math.max(7, Math.round(u))}px monospace`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText(String(i), sx, by);
}

// --- Frame ------------------------------------------------------------------
function draw() {
  const W = canvas.width;
  const H = canvas.height;
  ctx.fillStyle = '#0b0f14';
  ctx.fillRect(0, 0, W, H);

  // Ground: the whole grid as diamond tiles, two alternating greens.
  const hw = CS * camS;
  const hh = hw / 2;
  for (let gy = 0; gy < GH; gy++) {
    for (let gx = 0; gx < GW; gx++) {
      const [cx, cy] = worldToScreen(-HALF + (gx + 0.5) * CS, -HALF + (gy + 0.5) * CS);
      if (cx < -hw || cx > W + hw || cy < -hh || cy > H + hh) continue;
      diamond(cx, cy, hw, hh, (gx + gy) % 2 ? '#3f7a37' : '#457f3b');
    }
  }
  ctx.strokeStyle = 'rgba(0,0,0,0.10)';
  ctx.lineWidth = 1;
  for (let gy = 0; gy < GH; gy++) {
    for (let gx = 0; gx < GW; gx++) {
      const [cx, cy] = worldToScreen(-HALF + (gx + 0.5) * CS, -HALF + (gy + 0.5) * CS);
      if (cx < -hw || cx > W + hw || cy < -hh || cy > H + hh) continue;
      ctx.beginPath();
      ctx.moveTo(cx, cy - hh);
      ctx.lineTo(cx + hw, cy);
      ctx.lineTo(cx, cy + hh);
      ctx.lineTo(cx - hw, cy);
      ctx.closePath();
      ctx.stroke();
    }
  }

  const nodes = nodeList();
  const kinds = sim.entity_kinds();
  const pos = sim.positions();
  const count = sim.entity_count();

  let tcPos = null;
  for (let i = 0; i < count; i++) {
    if (kinds[i] === KIND_TC) tcPos = [pos[i * 2], pos[i * 2 + 1]];
  }
  updateCarry(nodes, tcPos, pos, kinds);

  // Active paths as thin ground lines (villagers only — the TC never moves).
  ctx.lineWidth = 1.5;
  for (let i = 0; i < count; i++) {
    if (kinds[i] === KIND_TC) continue;
    const path = sim.entity_path(i);
    if (path.length >= 4) {
      ctx.strokeStyle = COLORS[i % COLORS.length] + '88';
      ctx.beginPath();
      const [ex, ey] = worldToScreen(pos[i * 2], pos[i * 2 + 1]);
      ctx.moveTo(ex, ey);
      for (let k = 0; k < path.length; k += 2) {
        const [px, py] = worldToScreen(path[k], path[k + 1]);
        ctx.lineTo(px, py);
      }
      ctx.stroke();
    }
  }

  // Painter's algorithm: every sprite-like drawable sorted by wx + wy,
  // drawn back-to-front so nearer things overlap farther ones.
  const draws = [];
  for (let gy = 0; gy < GH; gy++) {
    for (let gx = 0; gx < GW; gx++) {
      if (!OBST[gy * GW + gx]) continue;
      const wx0 = -HALF + gx * CS;
      const wy0 = -HALF + gy * CS;
      draws.push({ key: wx0 + wy0 + CS, fn: () => drawBlock(wx0, wy0) });
    }
  }
  for (const n of nodes) {
    if (n.amount <= 0) continue; // depleted nodes vanish
    draws.push({
      key: n.x + n.y,
      fn: () => {
        const [sx, sy] = worldToScreen(n.x, n.y);
        NODE_DRAW[n.kind](sx, sy);
        amountBar(sx, sy, n.amount / NODE_MAX[n.kind]);
      },
    });
  }
  for (let i = 0; i < count; i++) {
    const wx = pos[i * 2];
    const wy = pos[i * 2 + 1];
    if (kinds[i] === KIND_TC) {
      // Sort by the TC's base/front edge so units in front overlap it.
      draws.push({ key: wx + wy + TC_HALF, fn: () => drawTC(wx, wy) });
    } else {
      draws.push({ key: wx + wy, fn: () => drawVillager(i, wx, wy) });
    }
  }
  draws.sort((a, b) => a.key - b.key);
  for (const d of draws) d.fn();

  // HUD: resources + villagers + tick + live state hash.
  const [food, wood, gold, stone] = sim.stockpile();
  const queue = sim.train_queue_len();
  hud.textContent =
    `food ${Math.floor(food)} · wood ${Math.floor(wood)} · gold ${Math.floor(gold)} · stone ${Math.floor(stone)}` +
    ` · villagers ${sim.villager_count()}${queue ? ` (+${queue} training)` : ''}` +
    ` · tick ${sim.tick_count()} · hash ${sim.state_hash()} · seed ${seed}`;
}

main().catch((e) => {
  hud.textContent = 'failed to load WASM: ' + e;
  console.error(e);
});
