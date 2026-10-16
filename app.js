// Skirmish M2 debug preview — dependency-free Canvas 2D view of the sim.
//
// This is a THROWAWAY debug slice, not the M4 renderer. It renders the
// sim top-down (the final game uses a fixed isometric camera) purely to
// make the M2 economy loop visible: stockpile HUD, resource nodes, the
// Town Center, villagers gathering/hauling/depositing, and the live
// state hash as a determinism proof.

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
// 2 gold, 3 stone.
const NODE_STYLE = [
  { color: '#2e8b3a', r: 7 },  // tree — green
  { color: '#d63384', r: 6 },  // berry bush — magenta
  { color: '#ffd700', r: 7 },  // gold mine — yellow
  { color: '#9aa0a6', r: 6 },  // stone mine — gray
];
const KIND_TC = 1; // entity kind tag: 0 villager, 1 town center

let sim, HALF, GW, GH, CS, OBST, C2W, W2C;

const worldToPx = (x, y) => [(x + HALF) * C2W, (y + HALF) * C2W];

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

async function main() {
  await init();
  sim = new WasmSim(seed);
  HALF = sim.world_half();
  GW = sim.grid_width();
  GH = sim.grid_height();
  CS = sim.cell_size();
  OBST = sim.obstacles();

  const S = canvas.width;
  C2W = S / (2 * HALF);
  W2C = 1 / C2W;

  trainBtn.addEventListener('click', () => {
    sim.push_train_villager();
  });

  canvas.addEventListener('click', (ev) => {
    const r = canvas.getBoundingClientRect();
    const sx = (ev.clientX - r.left) * (canvas.width / r.width);
    const sy = (ev.clientY - r.top) * (canvas.height / r.height);
    const wx = sx * W2C - HALF;
    const wy = sy * W2C - HALF;
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
    draw();
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

function draw() {
  const S = canvas.width;
  ctx.fillStyle = '#0b0f14';
  ctx.fillRect(0, 0, S, S);

  // Grid.
  const cellPx = CS * C2W;
  ctx.strokeStyle = 'rgba(255,255,255,0.05)';
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let i = 0; i <= GW; i++) {
    ctx.moveTo(i * cellPx, 0);
    ctx.lineTo(i * cellPx, S);
  }
  for (let j = 0; j <= GH; j++) {
    ctx.moveTo(0, j * cellPx);
    ctx.lineTo(S, j * cellPx);
  }
  ctx.stroke();

  // Obstacle cells.
  ctx.fillStyle = '#4a2f2f';
  for (let y = 0; y < GH; y++) {
    for (let x = 0; x < GW; x++) {
      if (OBST[y * GW + x]) ctx.fillRect(x * cellPx, y * cellPx, cellPx, cellPx);
    }
  }

  // Resource nodes; depleted ones vanish.
  ctx.textAlign = 'center';
  for (const n of nodeList()) {
    if (n.amount <= 0) continue;
    const st = NODE_STYLE[n.kind];
    const [px, py] = worldToPx(n.x, n.y);
    ctx.fillStyle = st.color;
    ctx.beginPath();
    ctx.arc(px, py, st.r, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = 'rgba(255,255,255,0.55)';
    ctx.font = '8px monospace';
    ctx.textBaseline = 'top';
    ctx.fillText(String(Math.floor(n.amount)), px, py + st.r + 1);
  }

  const kinds = sim.entity_kinds();
  const pos = sim.positions();
  const count = sim.entity_count();

  // Active paths as thin lines (villagers only — the TC never moves).
  ctx.lineWidth = 1.5;
  for (let i = 0; i < count; i++) {
    if (kinds[i] === KIND_TC) continue;
    const path = sim.entity_path(i);
    if (path.length >= 4) {
      ctx.strokeStyle = COLORS[i % COLORS.length] + '88';
      ctx.beginPath();
      const [ex, ey] = worldToPx(pos[i * 2], pos[i * 2 + 1]);
      ctx.moveTo(ex, ey);
      for (let k = 0; k < path.length; k += 2) {
        const [px, py] = worldToPx(path[k], path[k + 1]);
        ctx.lineTo(px, py);
      }
      ctx.stroke();
    }
  }

  // Entities: Town Center as a big square, villagers as colored circles.
  for (let i = 0; i < count; i++) {
    const [px, py] = worldToPx(pos[i * 2], pos[i * 2 + 1]);
    if (kinds[i] === KIND_TC) {
      ctx.fillStyle = '#c9a227';
      ctx.fillRect(px - 13, py - 13, 26, 26);
      ctx.strokeStyle = '#fff2b0';
      ctx.lineWidth = 1.5;
      ctx.strokeRect(px - 13, py - 13, 26, 26);
      ctx.fillStyle = '#1a1405';
      ctx.font = 'bold 10px monospace';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText('TC', px, py);
    } else {
      ctx.fillStyle = COLORS[i % COLORS.length];
      ctx.beginPath();
      ctx.arc(px, py, 6, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = '#000';
      ctx.font = '8px monospace';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(String(i), px, py);
    }
  }

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
