// Skirmish M1 debug preview — dependency-free Canvas 2D view of the sim.
//
// This is a THROWAWAY debug slice, not the M4 renderer. It renders the sim
// top-down (the final game uses a fixed isometric camera) purely to make M1
// movement visible: grid, walls, entities, active paths, and the live
// state hash as a determinism proof.

import init, { WasmSim } from './pkg/skirmish_sim.js';

const params = new URLSearchParams(location.search);
const seed = parseInt(params.get('seed') || '7', 10);

const canvas = document.getElementById('view');
const ctx = canvas.getContext('2d');
const hud = document.getElementById('hud');

const COLORS = [
  '#e74c3c', '#3498db', '#2ecc71', '#f1c40f',
  '#9b59b6', '#1abc9c', '#e67e22', '#ecf0f1',
];

let sim, HALF, COUNT, GW, GH, CS, OBST, C2W, W2C;

const worldToPx = (x, y) => [(x + HALF) * C2W, (y + HALF) * C2W];

async function main() {
  await init();
  sim = new WasmSim(seed);
  HALF = sim.world_half();
  COUNT = sim.entity_count();
  GW = sim.grid_width();
  GH = sim.grid_height();
  CS = sim.cell_size();
  OBST = sim.obstacles();

  const S = canvas.width;
  C2W = S / (2 * HALF);
  W2C = 1 / C2W;

  canvas.addEventListener('click', (ev) => {
    const r = canvas.getBoundingClientRect();
    const sx = (ev.clientX - r.left) * (canvas.width / r.width);
    const sy = (ev.clientY - r.top) * (canvas.height / r.height);
    const wx = sx * W2C - HALF;
    const wy = sy * W2C - HALF;
    for (let i = 0; i < COUNT; i++) sim.push_move(i, wx, wy);
  });

  // Opening demo order so something moves immediately.
  for (let i = 0; i < COUNT; i++) sim.push_move(i, 60, 60);

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

  // Active paths as thin lines.
  const pos = sim.positions();
  ctx.lineWidth = 1.5;
  for (let i = 0; i < COUNT; i++) {
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

  // Entities as colored circles with ids.
  for (let i = 0; i < COUNT; i++) {
    const [px, py] = worldToPx(pos[i * 2], pos[i * 2 + 1]);
    ctx.fillStyle = COLORS[i % COLORS.length];
    ctx.beginPath();
    ctx.arc(px, py, 7, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = '#000';
    ctx.font = '9px monospace';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(String(i), px, py);
  }

  // HUD: tick count + live state hash = visible determinism proof.
  hud.textContent = `tick ${sim.tick_count()} · hash ${sim.state_hash()} · seed ${seed}`;
}

main().catch((e) => {
  hud.textContent = 'failed to load WASM: ' + e;
  console.error(e);
});
