// Skirmish II play page — dependency-free Canvas 2D view of the sim.
// Loaded by play/index.html (../app.js); it is the game's fullscreen
// page. The landing page no longer embeds the game.
//
// Renders the sim with a 2D isometric camera in the Age of Empires
// style: flat 2D sprites on a fixed 2:1 dimetric projection, painted
// back-to-front. No real 3D, no rotation — the same technique as the
// classic engine. On top of the M2 economy view it adds the M2.5 shell:
// start overlay + pause menu, AoE2-style unit selection (individual,
// box, shift, double-click — orders go only to the selection), a
// placement ghost (client mirror of the sim's placement rules — the
// sim validates every order itself), construction rendering, and the
// AoE2-style interface layout: top resource bar, bottom-left command
// panel, bottom-center selection panel, bottom-right minimap.
// M3 adds the combat view: the starting Scout Cavalry, Gaia animals
// (sheep/boars/deer) with capture and carcasses, the Barracks with
// Militia/Archer training, right-click attack orders, HP bars, and
// arrow rendering — all read from the sim through the ABI (owners,
// HP, projectiles); the client never simulates any of it.
//
// The ADAPTIVE QUALITY governor (play/quality.js) sits over all of
// this: it probes the device, picks a level (sprite direction tier,
// canvas resolution cap, cosmetic draw passes), and adapts at
// runtime from measured FPS. It only ever changes how the game is
// DRAWN — the sim, its tick loop and every command are untouched,
// so a Chromebook and a high-end MacBook play the identical game.
//
// Asset URLs are resolved relative to THIS module (import.meta.url),
// so the page works from /play/, from the site root, and from the
// github.io project path alike.

import init, { WasmSim } from './pkg/skirmish_sim.js';
import {
  createMinimap, renderMinimap, minimapToWorld, cameraCenteredOn,
} from './play/minimap.js';
import {
  collectProbeSignals, scoreProbe, resolveMode, levelForMode,
  settingsFor, createGovernor,
} from './play/quality.js';

const assetUrl = (rel) => new URL(rel, import.meta.url).href;

const params = new URLSearchParams(location.search);
const seed = parseInt(params.get('seed') || '7', 10);

const canvas = document.getElementById('view');
const ctx = canvas.getContext('2d');
// Interface chrome (play/index.html + play/ui.css). The command-grid
// buttons (#train / #build-house / #build-farm) are REBUILT per
// selection context by renderCommands(), so they are reached through
// one delegated listener on #cmd-grid, never through stored refs.
const resFood = document.getElementById('res-food');
const resWood = document.getElementById('res-wood');
const resGold = document.getElementById('res-gold');
const resStone = document.getElementById('res-stone');
const resPop = document.getElementById('res-pop');
const gameClock = document.getElementById('game-clock');
const pauseBtn = document.getElementById('pause-btn');
const cmdGrid = document.getElementById('cmd-grid');
const selPortrait = document.getElementById('sel-portrait');
const selName = document.getElementById('sel-name');
const selInfo = document.getElementById('sel-info');
const selGrid = document.getElementById('sel-grid');
const minimapCanvas = document.getElementById('minimap');
const mmCtx = minimapCanvas.getContext('2d');
const toast = document.getElementById('toast');
const startOverlay = document.getElementById('start-overlay');
const playBtn = document.getElementById('play-btn');
const pauseMenu = document.getElementById('pause-menu');
const resumeBtn = document.getElementById('resume-btn');
const restartBtn = document.getElementById('restart-btn');

const COLORS = [
  '#e74c3c', '#3498db', '#2ecc71', '#f1c40f',
  '#9b59b6', '#1abc9c', '#e67e22', '#ecf0f1',
  '#ff7f50', '#7bed9f', '#70a1ff', '#ffa502',
  '#eccc68', '#ff6b81', '#a4b0be', '#57606f',
];

// Node kind tags (must match the sim's NodeKind::tag): 0 tree, 1 berry,
// 2 gold, 3 stone, 4 farm, 5 hunt carcass (boar/deer), 6 herd carcass
// (sheep). NODE_MAX is each kind's nominal full amount in the sim (for
// the remaining-amount bar / farm crop tint) — a carcass starts at what
// the animal was carrying, so its bar starts below full; CARRY_COLORS
// is the load dot shown above a hauling villager.
const NODE_MAX = [100, 125, 800, 350, 175, 340, 100];
const CARRY_COLORS = ['#d9a05b', '#ff5da2', '#ffd700', '#c3c8cd', '#ff5da2', '#ff5da2', '#ff5da2'];
// Entity kind tags (must match the sim's EntityKind::tag).
const KIND_TC = 1;
const KIND_SHEEP = 5;
const KIND_NAME = {
  0: 'Villager', 1: 'Town Center', 2: 'Scout Cavalry', 3: 'Militia',
  4: 'Archer', 5: 'Sheep', 6: 'Boar', 7: 'Deer',
};
// Selection-panel portraits per unit kind (SVG symbol ids in
// play/index.html).
const KIND_ICON = {
  0: 'i-villager', 2: 'i-villager', 3: 'i-militia', 4: 'i-archer', 5: 'i-pop',
};

// Building kind tags (must match the sim's BuildingKind::tag):
// 0 house, 1 farm, 2 barracks. Costs and footprints mirror the sim
// (MECHANICS-STUDY §5).
const BUILD_COST = [25, 60, 175];
const BUILD_NAME = ['House', 'Farm', 'Barracks'];
const BUILD_SIZE = [2, 2, 3];

// --- Game shell state (client-only; never sim input) -------------------
let playing = false; // Play pressed: the sim ticks
let paused = false; // pause menu open: the sim holds
let selection = new Set(); // selected unit ids — orders go ONLY to these (AoE2-style)
// The selected building, if any: { type: 'tc', id } for the Town
// Center (an entity) or { type: 'building', id, kind } for a placed
// building (House/Farm/Barracks). Null when units are selected.
let selectedBuilding = null;

// Selection invariants: the unit selection and the building selection
// are mutually exclusive — selecting one clears the other — and
// buildings never enter the unit Set (selectedUnits() also filters).
// Every selection change goes through these two helpers.
function selectUnits(set) {
  selection = set;
  selectedBuilding = null;
}
function selectBuilding(descOrNull) {
  selectedBuilding = descOrNull;
  selection = new Set();
}

let mmGeom = null; // minimap geometry, created once the grid is known
const OWN_COLOR = '#4da3ff'; // our player blue on the minimap (villager tunic family)
let lDrag = null; // left-button drag state, canvas px: { x0, y0, x1, y1, moved }
let placement = null; // { kind } while placing a building
let mouseWorld = null; // last cursor position, world coords
let flashText = '';
let flashUntil = 0;

function flash(msg) {
  flashText = msg;
  flashUntil = performance.now() + 2600;
}

// --- Placeholder sprites (CC0 test art) --------------------------------
// Temporary Kenney CC0 sprites so the preview already looks like a game;
// each is replaced by our own original art piece by piece (the villager
// and the house already are). Full source and license list:
// assets-placeholder/README.md. Loading is fire-and-forget: until an
// image is ready (or if it fails), the draw functions fall back to the
// original vector shapes below.
const SPRITES = {};
for (const [key, file] of Object.entries({
  villager: 'villager.png',
  tree: 'tree.png',
  berry: 'berry-bush.png',
  gold: 'gold.png',
  stone: 'stone.png',
  tc: 'town-center.png',
  house: 'house.png',
  barracks: 'barracks.png',
})) {
  const im = new Image();
  im.src = assetUrl('assets-placeholder/' + file);
  SPRITES[key] = im;
}
function spriteReady(im) {
  return !!im && im.complete && im.naturalWidth > 0;
}
// Draw `im` bottom-anchored at ground point (sx, sy + dyWorld*camS),
// sized by height (hWorld) or width (wWorld) in world units, keeping
// the image's aspect ratio. Returns false when not loaded yet.
function drawSpriteH(im, sx, sy, hWorld, dyWorld = 0) {
  if (!spriteReady(im)) return false;
  const h = hWorld * camS;
  const w = h * (im.naturalWidth / im.naturalHeight);
  ctx.drawImage(im, sx - w / 2, sy + dyWorld * camS - h, w, h);
  return true;
}
function drawSpriteW(im, sx, sy, wWorld, dyWorld = 0) {
  if (!spriteReady(im)) return false;
  const w = wWorld * camS;
  const h = w * (im.naturalHeight / im.naturalWidth);
  ctx.drawImage(im, sx - w / 2, sy + dyWorld * camS - h, w, h);
  return true;
}

// --- Original villager sprites (ours — sprite pipeline v1) -------------
// Generated by tools/sprite-pipeline/render.py from our own procedural
// low-poly model; see SPRITE-PIPELINE.md. Sheets: rows = facing
// directions (row count = the loaded quality tier: 8/16/32; dir 0
// faces +X world, increasing toward +Y), columns = frames. Which
// tier's sheets are loaded is the quality governor's call (see the
// quality block below) — only the active tier is ever fetched. The
// constants below mirror the pipeline and must be changed in both
// places together. Fallback chain per villager: original sheet
// -> CC0 placeholder -> vector circle (all in drawVillager).
const VIL_ORIGINAL = {
  cell: 128, ax: 64, ay: 110, pxPerUnit: 46, drawScale: 1.35,
  frames: { idle: 4, walk: 8, gather: 4, carry: 8 },
  dirs: 8, // rows in the currently loaded sheets (the active tier)
  img: {},
};
// --- Original house sprites (ours — sprite pipeline v1) --------------
// One image per construction stage, generated by
// tools/sprite-pipeline/render.py from our procedural cottage model;
// see SPRITE-PIPELINE.md (§8, replacement order #2) and
// assets-original/README.md. Canvas 896x704 with the footprint-center
// ground anchor at (448, 448), rendered at 46 px per world unit — the
// same anchor math as drawOriginalVillager, at true scale (1.0). The
// client picks the stage by build progress; the art replaces the old
// clip-reveal of the placeholder sprite. Fallback chain per house:
// original stage art -> CC0 placeholder (clip-reveal) -> vector.
const HOUSE_STAGES = ['stage0', 'stage1', 'stage2', 'complete'];
const HOUSE_ORIGINAL = {
  w: 896, h: 704, ax: 448, ay: 448, pxPerUnit: 46, drawScale: 1.0,
  img: {},
};
function drawOriginalHouse(b, sx, sy) {
  const stage = b.complete ? 'complete'
    : b.progress < 0.25 ? 'stage0'
    : b.progress < 0.55 ? 'stage1' : 'stage2';
  const im = HOUSE_ORIGINAL.img[stage];
  if (!spriteReady(im)) return false;
  const C = HOUSE_ORIGINAL;
  const k = (camS * C.drawScale) / C.pxPerUnit;
  ctx.drawImage(im, sx - C.ax * k, sy - C.ay * k, C.w * k, C.h * k);
  return true;
}
// Animation state per villager, advanced from sim positions each frame.
// The WASM ABI exposes no unit state, so — like the carry dot —
// movement is inferred client-side. Presentation only, never sim input.
const vilAnim = new Map();
const prevPos = new Map();

function updateVilAnim(i, wx, wy, now) {
  let a = vilAnim.get(i);
  if (!a) {
    // Default facing: +Y (screen down-left) — row dirs/4 at any tier.
    a = { dir: VIL_ORIGINAL.dirs / 4, phase: 0, still: 0 };
    vilAnim.set(i, a);
  }
  const p = prevPos.get(i);
  prevPos.set(i, [wx, wy]);
  let moving = false;
  if (p) {
    const dx = wx - p[0];
    const dy = wy - p[1];
    const dist = Math.hypot(dx, dy);
    if (dist > 0.004) {
      moving = true;
      a.still = 0;
      a.phase += dist / 1.5; // ~1.5 world units travelled per walk cycle
      if (dist > 0.02) {
        // Quantize the facing to the loaded tier's direction rows.
        const dirs = VIL_ORIGINAL.dirs;
        const d = Math.round(Math.atan2(dy, dx) / ((Math.PI * 2) / dirs));
        a.dir = ((d % dirs) + dirs) % dirs;
      }
    } else if (++a.still > 3) {
      a.phase = 0;
    }
  }
  let state = 'idle';
  if (moving) state = carryKind.has(i) ? 'carry' : 'walk';
  else if (atNodeNow.has(i)) state = 'gather';
  let frame;
  if (state === 'walk' || state === 'carry') {
    frame = Math.floor((a.phase % 1) * VIL_ORIGINAL.frames[state]);
  } else if (state === 'gather') {
    frame = Math.floor(now / 140) % VIL_ORIGINAL.frames.gather;
  } else {
    frame = Math.floor(now / 400) % VIL_ORIGINAL.frames.idle;
  }
  return { state, dir: a.dir, frame };
}

function drawOriginalVillager(anim, sx, sy) {
  const im = VIL_ORIGINAL.img[anim.state];
  if (!spriteReady(im)) return false;
  const C = VIL_ORIGINAL.cell;
  const k = (camS * VIL_ORIGINAL.drawScale) / VIL_ORIGINAL.pxPerUnit;
  ctx.drawImage(im,
    anim.frame * C, anim.dir * C, C, C,
    sx - VIL_ORIGINAL.ax * k, sy - VIL_ORIGINAL.ay * k, C * k, C * k);
  return true;
}

// --- Adaptive quality governor -----------------------------------------
// Doctrine (ARCHITECTURE.md, "Adaptive quality"): the sim is
// bit-identical for everyone; ONLY the picture scales. This block
// owns the client's quality level end-to-end:
//   probe -> mode (auto / manual override) -> governor (runtime FPS
//   adaptation) -> applyQuality (sprite tier, DPR cap, cosmetics).
// The sim tick loop below never reads any of this. On a slow machine
// the frame loop already drops RENDERED frames via its max-ticks
// guard (the sim slows visually, never logically); the governor's
// job is to make each rendered frame cheaper so the guard rarely
// has to bite.
const QUALITY_STORE_KEY = 'skirmish2.quality';
const probeLevel = scoreProbe(collectProbeSignals());
let storedQuality = null;
try { storedQuality = localStorage.getItem(QUALITY_STORE_KEY); } catch { /* private mode */ }
let qualityMode = resolveMode({
  param: params.get('quality'), stored: storedQuality,
});
const governor = createGovernor({
  initial: levelForMode(qualityMode, probeLevel),
  max: probeLevel, // auto-adaptation may never exceed the probe
});
if (qualityMode !== 'auto') governor.setManual(qualityMode);
let Q = settingsFor(governor.level); // active level's settings
let dprScale = 1; // canvas backing px per CSS px (set by applyQuality)

// Point the sprite loaders at one tier's folder and fetch that
// tier's sheets. Lazy by construction: sheets for other tiers are
// never requested. A tier switch swaps the Image objects; until a
// new sheet arrives, drawVillager/drawHouse fall back down their
// chains (placeholder -> vector), so a switch never blanks a unit.
function loadTierSprites(st) {
  const prevDirs = VIL_ORIGINAL.dirs;
  VIL_ORIGINAL.dirs = st.tier;
  const base = st.tierDir ? `assets-original/${st.tierDir}/` : 'assets-original/';
  for (const state of Object.keys(VIL_ORIGINAL.frames)) {
    const im = new Image();
    im.src = assetUrl(base + `villager-${state}.png`);
    VIL_ORIGINAL.img[state] = im;
  }
  for (const stage of HOUSE_STAGES) {
    const im = new Image();
    if (st.tierDir) {
      // The house art is tier-independent; if a tier copy ever
      // fails to load, fall back to the root (LOW) file.
      im.onerror = () => {
        im.onerror = null;
        im.src = assetUrl(`assets-original/house-${stage}.png`);
      };
    }
    im.src = assetUrl(base + `house-${stage}.png`);
    HOUSE_ORIGINAL.img[stage] = im;
  }
  // Facings were quantized on the old tier's rows; rescale them so
  // standing units don't snap to a wrong facing on a tier switch.
  if (prevDirs !== st.tier) {
    for (const a of vilAnim.values()) {
      a.dir = Math.round((a.dir * st.tier) / prevDirs) % st.tier;
    }
  }
}

const qualityLabel = document.getElementById('quality-label');
const qualityModeBtns = {
  auto: document.getElementById('q-auto'),
  high: document.getElementById('q-high'),
  medium: document.getElementById('q-medium'),
  low: document.getElementById('q-low'),
};

function updateQualityUi() {
  const name = governor.level[0].toUpperCase() + governor.level.slice(1);
  if (qualityLabel) {
    qualityLabel.textContent = qualityMode === 'auto'
      ? `Quality: ${name} · Auto`
      : `Quality: ${name}`;
  }
  for (const [mode, btn] of Object.entries(qualityModeBtns)) {
    if (btn) btn.classList.toggle('active', qualityMode === mode);
  }
}

// Apply a level end-to-end. `announce` shows the one subtle toast
// the spec allows per automatic change; the initial application at
// load is silent (the top-bar label shows the level instead).
function applyQuality(level, { announce = false } = {}) {
  Q = settingsFor(level);
  loadTierSprites(Q);
  dprScale = Math.min(window.devicePixelRatio || 1, Q.dprCap);
  fitCanvas();
  updateQualityUi();
  if (announce) {
    const name = level[0].toUpperCase() + level.slice(1);
    flash(`Quality set to ${name} — adjusted for your device. The game is the same at every level.`);
  }
}

// Pause-menu override. A manual pick pins the governor; Auto hands
// control back (adaptation resumes from the current level, still
// capped at the probe). The ?quality= URL parameter wins for the
// session but is never written to storage (it is a support tool).
function setQualityMode(mode) {
  qualityMode = mode;
  try { localStorage.setItem(QUALITY_STORE_KEY, mode); } catch { /* private mode */ }
  if (mode === 'auto') {
    governor.setManual(null);
    applyQuality(governor.level);
  } else {
    governor.setManual(mode);
    applyQuality(mode);
  }
}

// Boot: fetch the initial tier's sheets and set the DPR scale now;
// main()'s first fitCanvas() call picks dprScale up (the full
// applyQuality path is for post-boot changes — it also resizes the
// canvas, which must wait until the camera state exists).
loadTierSprites(Q);
dprScale = Math.min(window.devicePixelRatio || 1, Q.dprCap);
updateQualityUi();

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

// Building data is a flat array of 11 per building (M3):
// [id, kind, x, y, progress, complete, cell_x, cell_y, size,
//  queue_len, train_progress].
function buildingList() {
  const flat = sim.buildings();
  const out = [];
  for (let i = 0; i < flat.length; i += 11) {
    out.push({
      id: flat[i], kind: flat[i + 1], x: flat[i + 2], y: flat[i + 3],
      progress: flat[i + 4], complete: flat[i + 5] > 0.5,
      cellX: flat[i + 6], cellY: flat[i + 7], size: flat[i + 8],
      queue: flat[i + 9], trainFrac: flat[i + 10],
    });
  }
  return out;
}

// One snapshot of the entity arrays (entity index === entity id —
// the sim never reorders or removes entities; the dead are
// tombstones with hp < 1). `hp` is flat [hp, max_hp] pairs.
function entityState() {
  return {
    kinds: sim.entity_kinds(),
    owners: sim.entity_owners(),
    hp: sim.entity_hp(),
    pos: sim.positions(),
    count: sim.entity_count(),
  };
}
// A living unit of ours: player-owned, not the Town Center, hp >= 1
// (the sim's death rule is hp < 1).
const isOwnUnit = (st, i) =>
  st.owners[i] === 0 && st.kinds[i] !== KIND_TC && st.hp[i * 2] >= 1;

// Every selectable unit of ours (for box select / double-click).
function ownUnitIds() {
  const st = entityState();
  const ids = [];
  for (let i = 0; i < st.count; i++) {
    if (isOwnUnit(st, i)) ids.push(i);
  }
  return ids;
}

// The current selection as an id list, restricted to our living
// units (buildings are never in the selection — the filter keeps it
// true even if a future caller slips one in; the dead drop out the
// frame they die).
function selectedUnits() {
  if (!sim) return [];
  const st = entityState();
  return [...selection].filter((i) => isOwnUnit(st, i));
}

function worldToCell(wx, wy) {
  const cx = Math.max(0, Math.min(GW - 1, Math.floor((wx + HALF) / CS)));
  const cy = Math.max(0, Math.min(GH - 1, Math.floor((wy + HALF) / CS)));
  return [cx, cy];
}

function tcCell() {
  const kinds = sim.entity_kinds();
  const pos = sim.positions();
  const tc = kinds.indexOf(KIND_TC);
  if (tc < 0) return null;
  return worldToCell(pos[tc * 2], pos[tc * 2 + 1]);
}

// --- Placement ghost: client mirror of the sim's build validation ----
// Mirrors Sim::validate_build (lib.rs) using only ABI-visible state, so
// the ghost is honest. The sim re-validates every order itself — this
// mirror is a courtesy, never the authority.
function mirrorValid(kind, cx, cy) {
  const size = BUILD_SIZE[kind];
  if (cx < 0 || cy < 0 || cx + size > GW || cy + size > GH) return false;
  const buildings = buildingList();
  // Resuming an incomplete building of the same kind at this anchor.
  for (const b of buildings) {
    if (!b.complete && b.kind === kind && b.cellX === cx && b.cellY === cy) {
      return true;
    }
  }
  // Any standing building's footprint (square range overlap).
  for (const b of buildings) {
    const bs = b.size || BUILD_SIZE[b.kind] || 2;
    if (cx < b.cellX + bs && b.cellX < cx + size &&
        cy < b.cellY + bs && b.cellY < cy + size) return false;
  }
  const cells = [];
  for (let dx = 0; dx < size; dx++) {
    for (let dy = 0; dy < size; dy++) cells.push([cx + dx, cy + dy]);
  }
  const tc = tcCell();
  const nodes = nodeList();
  for (const [x, y] of cells) {
    if (OBST[y * GW + x]) return false; // static wall
    if (tc && x === tc[0] && y === tc[1]) return false; // Town Center
    for (const n of nodes) { // a live node on this cell
      if (n.amount <= 0) continue;
      const nc = worldToCell(n.x, n.y);
      if (nc[0] === x && nc[1] === y) return false;
    }
  }
  if (sim.wood() < BUILD_COST[kind]) return false;
  return true;
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
const atNodeNow = new Set(); // villagers currently within gather range of a node

function updateCarry(nodes, tcPos, pos, kinds, hp) {
  for (let i = 0; i < kinds.length; i++) {
    if (kinds[i] !== 0) continue; // villagers only — animals/military carry nothing
    if (hp && hp[i * 2] < 1) { // the dead drop out of the heuristic
      carryKind.delete(i);
      lastNodeKind.delete(i);
      atNodeNow.delete(i);
      continue;
    }
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
    if (atKind !== null) atNodeNow.add(i);
    else atNodeNow.delete(i);
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
  if (e.code === 'Escape') {
    if (placement) {
      placement = null;
    } else if (playing) {
      setPaused(!paused);
    }
    return;
  }
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

function setPaused(v) {
  if (!playing) return;
  paused = v;
  pauseMenu.style.display = paused ? 'flex' : 'none';
}

// (Re)create the simulation and all client-side mirrors. Restart is a
// fresh WasmSim with the same seed: bit-identical starting state.
function resetSim() {
  sim = new WasmSim(seed);
  vilAnim.clear();
  prevPos.clear();
  lastNodeKind.clear();
  carryKind.clear();
  atNodeNow.clear();
  selectUnits(new Set()); // also clears any building selection
  placement = null;

  // Camera: centered on the Town Center.
  const kinds = sim.entity_kinds();
  const pos = sim.positions();
  const tc = kinds.indexOf(KIND_TC);
  camS = INITIAL_S;
  const [ox, oy] = centerCamera(
    pos[tc * 2], pos[tc * 2 + 1], camS, canvas.width, canvas.height);
  camOx = ox;
  camOy = oy;

  // No opening orders: the villagers start idle beside the Town
  // Center, and the player's clicks are the first orders of the
  // match — the AoE2 standard start (OPENING-SPEC.md §2.1). The
  // client never auto-plays the opening.
}

// Size the canvas backing store: CSS size x dprScale, where
// dprScale is the quality level's DPR cap (quality block). All
// rendering and pointer math work in backing px (pointer handlers
// scale by canvas.width / rect.width), so a higher cap simply buys
// sharpness on strong hardware. On a resize the world point at the
// screen center is kept put by shifting the camera origin by half
// the size delta.
function fitCanvas() {
  const w = Math.max(1, Math.round(window.innerWidth * dprScale));
  const h = Math.max(1, Math.round(window.innerHeight * dprScale));
  camOx += (w - canvas.width) / 2;
  camOy += (h - canvas.height) / 2;
  canvas.width = w;
  canvas.height = h;
}

async function main() {
  await init();
  HALF = null; // set from the first sim instance below
  fitCanvas();
  addEventListener('resize', () => {
    fitCanvas();
  });

  resetSim();
  HALF = sim.world_half();
  GW = sim.grid_width();
  GH = sim.grid_height();
  CS = sim.cell_size();
  OBST = sim.obstacles();
  mmGeom = createMinimap({
    worldHalf: HALF, width: minimapCanvas.width, height: minimapCanvas.height,
  });
  // Camera centering in resetSim ran before HALF/GW were known-good;
  // the values come straight from the sim, so just re-center cleanly.
  {
    const kinds = sim.entity_kinds();
    const pos = sim.positions();
    const tc = kinds.indexOf(KIND_TC);
    const [ox, oy] = centerCamera(
      pos[tc * 2], pos[tc * 2 + 1], camS, canvas.width, canvas.height);
    camOx = ox;
    camOy = oy;
  }

  // --- Menus -----------------------------------------------------------
  playBtn.addEventListener('click', () => {
    startOverlay.style.display = 'none';
    playing = true;
    paused = false;
  });
  pauseBtn.addEventListener('click', () => setPaused(true));
  resumeBtn.addEventListener('click', () => setPaused(false));
  restartBtn.addEventListener('click', () => {
    resetSim();
    setPaused(false);
    playing = true;
  });
  // Quality override (pause menu): Auto / High / Medium / Low.
  for (const [mode, btn] of Object.entries(qualityModeBtns)) {
    if (btn) btn.addEventListener('click', () => setQualityMode(mode));
  }
  updateQualityUi();

  // Command panel: ONE delegated listener — renderCommands() rebuilds
  // the buttons per selection context, so per-button refs would go
  // stale. The sim-facing behavior is unchanged from the old buttons.
  cmdGrid.addEventListener('click', (ev) => {
    const btn = ev.target.closest('button');
    if (!btn || btn.disabled) return;
    if (btn.id === 'train') {
      if (!sim.push_train_villager()) {
        const reason = sim.train_block_reason();
        flash(['', 'Not enough food for a villager.',
          'The training queue is full.', 'Housed — build a House first.'][reason] || 'Cannot train right now.');
      }
    } else if (btn.id === 'build-house') {
      placement = placement && placement.kind === 0 ? null : { kind: 0 };
    } else if (btn.id === 'build-farm') {
      placement = placement && placement.kind === 1 ? null : { kind: 1 };
    } else if (btn.id === 'build-barracks') {
      placement = placement && placement.kind === 2 ? null : { kind: 2 };
    } else if (btn.id === 'train-militia' || btn.id === 'train-archer') {
      const kindTag = btn.id === 'train-militia' ? 3 : 4;
      const ok = selectedBuilding && selectedBuilding.type === 'building'
        ? (kindTag === 3
          ? sim.push_train_militia(selectedBuilding.id)
          : sim.push_train_archer(selectedBuilding.id))
        : false;
      if (!ok && selectedBuilding) {
        const reason = sim.building_train_block(selectedBuilding.id, kindTag);
        flash(['', 'Not enough resources for that unit.',
          'The training queue is full.', 'Housed — build a House first.'][reason] || 'Cannot train right now.');
      }
    }
  });

  // Minimap camera control: click or drag to jump the main camera.
  // The bottom bar floats above #view, so these gestures never reach
  // the canvas selection/pan handlers. Client px are converted to
  // canvas px through the bounding rect — the CSS display size
  // differs from the attribute size on narrow screens.
  let mmDrag = false;
  function minimapJump(ev) {
    if (!mmGeom) return;
    const r = minimapCanvas.getBoundingClientRect();
    const mx = (ev.clientX - r.left) * (minimapCanvas.width / r.width);
    const my = (ev.clientY - r.top) * (minimapCanvas.height / r.height);
    const [wx, wy] = minimapToWorld(mmGeom, mx, my);
    [camOx, camOy] = cameraCenteredOn(wx, wy, camS, canvas.width, canvas.height);
  }
  minimapCanvas.addEventListener('pointerdown', (ev) => {
    mmDrag = true;
    minimapCanvas.setPointerCapture(ev.pointerId);
    minimapJump(ev);
  });
  minimapCanvas.addEventListener('pointermove', (ev) => {
    if (mmDrag) minimapJump(ev);
  });
  minimapCanvas.addEventListener('pointerup', () => { mmDrag = false; });
  minimapCanvas.addEventListener('pointercancel', () => { mmDrag = false; });

  // --- Pointer ---------------------------------------------------------
  function eventWorld(ev) {
    const r = canvas.getBoundingClientRect();
    const sx = (ev.clientX - r.left) * (canvas.width / r.width);
    const sy = (ev.clientY - r.top) * (canvas.height / r.height);
    return s2w(sx, sy, camS, camOx, camOy);
  }

  function eventScreen(ev) {
    const r = canvas.getBoundingClientRect();
    return [
      (ev.clientX - r.left) * (canvas.width / r.width),
      (ev.clientY - r.top) * (canvas.height / r.height),
    ];
  }

  // Left-click pick: one of OUR living units (villagers, Scout,
  // military, captured sheep) — Gaia's animals are never selected.
  function unitAt(wx, wy) {
    const st = entityState();
    let best = null;
    let bestD2 = 1.9 * 1.9;
    for (let i = 0; i < st.count; i++) {
      if (!isOwnUnit(st, i)) continue;
      const d2 = (st.pos[i * 2] - wx) ** 2 + (st.pos[i * 2 + 1] - wy) ** 2;
      if (d2 < bestD2) {
        bestD2 = d2;
        best = i;
      }
    }
    return best;
  }

  // Right-click target pick: ANY living entity (ours or Gaia's —
  // the attack-target test). Slightly more generous radius: boars
  // are big, and a missed click should still read as an attack.
  function entityAt(wx, wy) {
    const st = entityState();
    let best = null;
    let bestD2 = 2.2 * 2.2;
    for (let i = 0; i < st.count; i++) {
      if (st.kinds[i] === KIND_TC || st.hp[i * 2] < 1) continue;
      const d2 = (st.pos[i * 2] - wx) ** 2 + (st.pos[i * 2 + 1] - wy) ** 2;
      if (d2 < bestD2) {
        bestD2 = d2;
        best = i;
      }
    }
    return best;
  }

  // Placed-building hit test (Barracks selection): the building
  // whose footprint box contains the point.
  function buildingAt(wx, wy) {
    for (const b of buildingList()) {
      const half = (b.size * CS) / 2;
      if (Math.abs(b.x - wx) <= half && Math.abs(b.y - wy) <= half) return b;
    }
    return null;
  }

  // Town Center hit test: the TC whose footprint box (pos ± TC_HALF
  // on both axes) contains the point — deliberately more generous
  // than unitAt's pick radius, since the TC is a big building.
  function tcAt(wx, wy) {
    const kinds = sim.entity_kinds();
    const pos = sim.positions();
    for (let i = 0; i < kinds.length; i++) {
      if (kinds[i] !== KIND_TC) continue;
      if (Math.abs(pos[i * 2] - wx) <= TC_HALF &&
          Math.abs(pos[i * 2 + 1] - wy) <= TC_HALF) return i;
    }
    return null;
  }

  function nodeAt(wx, wy) {
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
    return best;
  }

  // Context order at a point (M3): an entity there (Gaia animal or
  // our own sheep) means Attack for everyone who can fight; a node
  // means Gather for villagers and Move for the rest; ground means
  // Move. Goes ONLY to the current selection (AoE2-style) — with an
  // empty selection the order does nothing but flash the hint.
  function orderAt(wx, wy) {
    const targets = selectedUnits();
    if (targets.length === 0) {
      flash(selectedBuilding != null
        ? (selectedBuilding.type === 'tc'
          ? 'The Town Center trains villagers from the Commands panel.'
          : 'Train units from the Commands panel.')
        : 'Select a unit first.');
      return;
    }
    const st = entityState();
    const t = entityAt(wx, wy);
    if (t != null && (st.owners[t] === 1 || st.kinds[t] === KIND_SHEEP)) {
      let any = false;
      for (const id of targets) {
        if (st.kinds[id] === KIND_SHEEP) continue; // sheep don't fight
        if (sim.push_attack(id, t)) any = true;
      }
      if (any) return;
      // Nobody could attack (e.g. only sheep selected): fall through
      // to a plain move toward the point.
    }
    const n = nodeAt(wx, wy);
    for (const id of targets) {
      if (n && st.kinds[id] === 0) sim.push_gather(id, n.id);
      else sim.push_move(id, wx, wy);
    }
  }

  function attemptPlace(wx, wy) {
    // The builder is the selected VILLAGER nearest the placement
    // point: with a group selected, the closest pair of hands does
    // the work (the sim's Build command names a single builder, and
    // only villagers build).
    const st = entityState();
    const pos = st.pos;
    let builder = null;
    let bestD2 = Infinity;
    for (const id of selectedUnits()) {
      if (st.kinds[id] !== 0) continue;
      const d2 = (pos[id * 2] - wx) ** 2 + (pos[id * 2 + 1] - wy) ** 2;
      if (d2 < bestD2) {
        bestD2 = d2;
        builder = id;
      }
    }
    if (builder == null) {
      placement = null;
      flash('Select a villager to build.');
      return;
    }
    const [cx, cy] = worldToCell(wx, wy);
    // Send a point strictly inside the anchor cell; the sim anchors
    // the footprint at the cell containing the point.
    const px = -HALF + cx * CS + 0.01;
    const py = -HALF + cy * CS + 0.01;
    const ok = placement.kind === 0
      ? sim.push_build_house(builder, px, py)
      : placement.kind === 1
        ? sim.push_build_farm(builder, px, py)
        : sim.push_build_barracks(builder, px, py);
    if (ok) {
      placement = null;
    } else {
      flash(`Cannot build a ${BUILD_NAME[placement.kind]} there.`);
    }
  }

  canvas.addEventListener('mousemove', (ev) => {
    mouseWorld = eventWorld(ev);
  });

  // ---- AoE2-style selection (left button) ----
  // Click a unit: select only it. Click empty ground: deselect.
  // Drag: selection box — every own unit whose ground point falls
  // inside joins the selection. Shift+click toggles one unit;
  // shift+drag ADDS the boxed units. Double-click a unit: select all
  // units of the same kind visible on screen. Left-click NEVER issues
  // an order — orders are the right button's job (or the build menu).
  canvas.addEventListener('mousedown', (ev) => {
    if (ev.button !== 0) return;
    const [sx, sy] = eventScreen(ev);
    lDrag = { x0: sx, y0: sy, x1: sx, y1: sy, moved: false };
  });
  window.addEventListener('mousemove', (ev) => {
    if (!lDrag) return;
    const [sx, sy] = eventScreen(ev);
    lDrag.x1 = sx;
    lDrag.y1 = sy;
    if (Math.abs(sx - lDrag.x0) + Math.abs(sy - lDrag.y0) > 6) lDrag.moved = true;
  });
  window.addEventListener('mouseup', (ev) => {
    if (ev.button !== 0 || !lDrag) return;
    const d = lDrag;
    lDrag = null;
    const [wx, wy] = eventWorld(ev);
    if (placement) {
      if (!d.moved) attemptPlace(wx, wy); // a placement click, not a drag
      return;
    }
    if (d.moved) {
      // Selection box (screen space): ground point inside the rect.
      const x0 = Math.min(d.x0, d.x1);
      const x1 = Math.max(d.x0, d.x1);
      const y0 = Math.min(d.y0, d.y1);
      const y1 = Math.max(d.y0, d.y1);
      const pos = sim.positions();
      const boxed = [];
      for (const id of ownUnitIds()) {
        const [sx, sy] = worldToScreen(pos[id * 2], pos[id * 2 + 1]);
        if (sx >= x0 && sx <= x1 && sy >= y0 && sy <= y1) boxed.push(id);
      }
      if (ev.shiftKey) {
        const next = new Set(selection);
        for (const id of boxed) next.add(id);
        selectUnits(next);
      } else {
        selectUnits(new Set(boxed));
      }
      return;
    }
    // Plain click.
    const v = unitAt(wx, wy);
    if (v != null) {
      if (ev.shiftKey) {
        const next = new Set(selection);
        if (next.has(v)) next.delete(v);
        else next.add(v);
        selectUnits(next);
      } else {
        selectUnits(new Set([v]));
      }
    } else if (!ev.shiftKey) {
      // No unit hit: the Town Center and placed buildings are
      // selections of their own (kept OUT of the unit Set); empty
      // ground clears both selections.
      const tc = tcAt(wx, wy);
      if (tc != null) {
        selectBuilding({ type: 'tc', id: tc });
      } else {
        const b = buildingAt(wx, wy);
        if (b) selectBuilding({ type: 'building', id: b.id, kind: b.kind });
        else selectUnits(new Set());
      }
    }
  });
  canvas.addEventListener('dblclick', (ev) => {
    const [wx, wy] = eventWorld(ev);
    const v = unitAt(wx, wy);
    if (v == null) return;
    const kinds = sim.entity_kinds();
    const kind = kinds[v];
    const pos = sim.positions();
    const same = [];
    for (const id of ownUnitIds()) {
      if (kinds[id] !== kind) continue;
      const [sx, sy] = worldToScreen(pos[id * 2], pos[id * 2 + 1]);
      if (sx >= 0 && sx <= canvas.width && sy >= 0 && sy <= canvas.height) same.push(id);
    }
    if (ev.shiftKey) {
      const next = new Set(selection);
      for (const id of same) next.add(id);
      selectUnits(next);
    } else {
      selectUnits(new Set(same));
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

  // Pan by dragging with the right mouse button; a right *click*
  // (no drag) issues a context order without changing the selection —
  // or cancels placement mode.
  let drag = null; // [clientX, clientY, moved]
  canvas.addEventListener('mousedown', (e) => {
    if (e.button === 2) drag = [e.clientX, e.clientY, false];
  });
  addEventListener('mousemove', (e) => {
    if (!drag) return;
    const r = canvas.getBoundingClientRect();
    const f = canvas.width / r.width;
    if (Math.abs(e.clientX - drag[0]) + Math.abs(e.clientY - drag[1]) > 3) {
      drag[2] = true;
    }
    camOx += (e.clientX - drag[0]) * f;
    camOy += (e.clientY - drag[1]) * f;
    drag = [e.clientX, e.clientY, drag[2]];
  });
  addEventListener('mouseup', (e) => {
    if (e.button !== 2 || !drag) return;
    const wasClick = !drag[2];
    drag = null;
    if (!wasClick) return;
    if (placement) {
      placement = null;
      return;
    }
    const [wx, wy] = eventWorld(e);
    orderAt(wx, wy);
  });
  canvas.addEventListener('contextmenu', (e) => e.preventDefault());

  // Fixed 60 Hz tick loop decoupled from rAF, with a max-ticks-per-frame
  // guard: on a slow machine the preview slows down visually, never
  // logically (same rule as the real game). The sim only advances while
  // playing and unpaused — it starts paused behind the start overlay.
  // The quality governor samples the RENDERED frame rate on the same
  // loop (only while playing — a paused scene proves nothing) and
  // steps the visual level down/up; it cannot touch the tick logic
  // above: worst case on weak hardware stays "fewer, cheaper frames
  // of the identical simulation".
  let acc = 0;
  let last = performance.now();
  let prevFrameAt = null;
  const STEP = 1000 / 60;
  function frame(now) {
    if (playing && !paused) {
      acc += Math.min(now - last, 250);
      let n = 0;
      while (acc >= STEP && n < 5) {
        sim.tick();
        acc -= STEP;
        n++;
      }
      if (n === 5) acc = 0;
      if (prevFrameAt != null) {
        const step = governor.sample(now - prevFrameAt, now);
        if (step.changed) applyQuality(step.level, { announce: true });
      }
    } else {
      acc = 0;
    }
    prevFrameAt = now;
    last = now;
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
  if (!Q.shadows) return; // cosmetic: LOW sheds shadows first
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

// Construction progress bar, drawn at an explicit anchor (above the
// building), gold fill — same visual family as amountBar.
function progressBar(sx, sy, frac) {
  const w = 5 * camS;
  const h = Math.max(3, camS * 0.6);
  const x = sx - w / 2;
  ctx.fillStyle = 'rgba(0,0,0,0.55)';
  ctx.fillRect(x - 1, sy - 1, w + 2, h + 2);
  ctx.fillStyle = '#20261c';
  ctx.fillRect(x, sy, w, h);
  ctx.fillStyle = '#d9a441';
  ctx.fillRect(x, sy, w * Math.max(0, Math.min(1, frac)), h);
}

function drawTree(sx, sy) {
  const u = camS;
  shadow(sx, sy, 1.5 * u);
  if (drawSpriteH(SPRITES.tree, sx, sy, 4.4)) return;
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
  if (drawSpriteH(SPRITES.berry, sx, sy, 1.9)) return;
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
  if (drawSpriteW(SPRITES.gold, sx, sy, 2.6)) return;
  // Angular crystals: small diamonds in three gold tones.
  diamond(sx - 0.7 * u, sy - 0.45 * u, 0.5 * u, 0.55 * u, '#c78f00');
  diamond(sx + 0.1 * u, sy - 0.8 * u, 0.65 * u, 0.85 * u, '#ffd700');
  diamond(sx + 0.8 * u, sy - 0.4 * u, 0.45 * u, 0.5 * u, '#e0a800');
  diamond(sx + 0.1 * u, sy - 1.05 * u, 0.22 * u, 0.3 * u, '#fff3a0');
}

function drawStone(sx, sy) {
  const u = camS;
  shadow(sx, sy, 1.35 * u);
  if (drawSpriteW(SPRITES.stone, sx, sy, 2.8)) return;
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

// A farm field: a tilled-soil diamond spanning the farm's 2x2 cells
// around its center point, with furrow lines and a crop tint whose
// strength follows the food remaining (full field = lush, nearly
// depleted = mostly bare soil). `cropFrac` null draws bare soil
// (under construction).
function drawFarmPlot(wx, wy, cropFrac) {
  const t = worldToScreen(wx - CS, wy - CS);
  const r = worldToScreen(wx + CS, wy - CS);
  const b = worldToScreen(wx + CS, wy + CS);
  const l = worldToScreen(wx - CS, wy + CS);
  poly([t, r, b, l], '#6b4a2c'); // tilled soil
  // Furrows: three lines running one way across the field.
  ctx.strokeStyle = 'rgba(0,0,0,0.28)';
  ctx.lineWidth = Math.max(1, camS * 0.18);
  for (const f of [0.25, 0.5, 0.75]) {
    const x1 = t[0] + (l[0] - t[0]) * f;
    const y1 = t[1] + (l[1] - t[1]) * f;
    const x2 = r[0] + (b[0] - r[0]) * f;
    const y2 = r[1] + (b[1] - r[1]) * f;
    ctx.beginPath();
    ctx.moveTo(x1, y1);
    ctx.lineTo(x2, y2);
    ctx.stroke();
  }
  if (cropFrac != null) {
    const a = 0.12 + 0.5 * Math.max(0, Math.min(1, cropFrac));
    poly([t, r, b, l], `rgba(96, 165, 62, ${a.toFixed(3)})`);
  }
}

function drawFarm(sx, sy, frac, wx, wy) {
  shadow(sx, sy, 3.4 * camS);
  drawFarmPlot(wx, wy, frac);
}

// A carcass (M3): a fallen animal — meat mound in two red tones with
// pale bone ticks. Same painter family as the other node vectors; the
// amount bar above it (drawn by the caller) shows the meat left, and
// it decays visibly over time. `frac` is amount / nominal full.
function drawCarcass(sx, sy, frac) {
  const u = camS;
  shadow(sx, sy, 1.5 * u);
  ctx.fillStyle = '#7c3f2a';
  ctx.beginPath();
  ctx.ellipse(sx, sy - 0.35 * u, 1.25 * u, 0.62 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#a2542f';
  ctx.beginPath();
  ctx.ellipse(sx - 0.25 * u, sy - 0.55 * u, 0.85 * u, 0.45 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#e8ddc8';
  ctx.lineWidth = Math.max(1, 0.22 * u);
  for (const [dx, dy, a] of [[-0.8, -0.5, -0.5], [0.1, -0.85, 0.2], [0.8, -0.45, 0.6]]) {
    ctx.beginPath();
    ctx.moveTo(sx + dx * u, sy + dy * u);
    ctx.lineTo(sx + (dx + Math.sin(a) * 0.45) * u, sy + (dy - 0.45) * u);
    ctx.stroke();
  }
}

const NODE_DRAW = [drawTree, drawBerry, drawGold, drawStone, drawFarm, drawCarcass, drawCarcass];

// --- Building sprites -----------------------------------------------------
const TC_HALF = 3.6; // world units: TC footprint half-extent
function drawTC(wx, wy) {
  const a = TC_HALF;
  const t = worldToScreen(wx - a, wy - a);
  const r = worldToScreen(wx + a, wy - a);
  const b = worldToScreen(wx + a, wy + a);
  const l = worldToScreen(wx - a, wy + a);
  const [sx, sy] = worldToScreen(wx, wy);
  shadow(sx, sy, (r[0] - l[0]) / 2);
  // Placeholder building sprite, base planted toward the footprint's
  // front edge (dy +1.6 world units from the center point).
  if (drawSpriteW(SPRITES.tc, sx, sy, 9.5, 1.6)) return;
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

// House rendering. First choice is our original stage art (see
// HOUSE_ORIGINAL above): the sprite carries its own foundation and
// the construction stage follows build progress. Fallback is the
// CC0 placeholder sprite with a clip-reveal (only the bottom `frac`
// shown over a foundation diamond, so the building visibly rises),
// and finally a vector block that grows with progress.
const HOUSE_W = 8.6; // world units wide (placeholder draw contract)
const HOUSE_DY = 2.0; // base planted toward the footprint's front edge
function drawHouse(b) {
  const [sx, sy] = worldToScreen(b.x, b.y);
  shadow(sx, sy, 3.6 * camS);
  if (!drawOriginalHouse(b, sx, sy)) {
    // Foundation: the footprint as a flat stone diamond.
    const t = worldToScreen(b.x - CS, b.y - CS);
    const r = worldToScreen(b.x + CS, b.y - CS);
    const bt = worldToScreen(b.x + CS, b.y + CS);
    const l = worldToScreen(b.x - CS, b.y + CS);
    poly([t, r, bt, l], b.complete ? 'rgba(0,0,0,0.12)' : '#8d8578');
    const im = SPRITES.house;
    if (spriteReady(im)) {
      const w = HOUSE_W * camS;
      const h = w * (im.naturalHeight / im.naturalWidth);
      const base = sy + HOUSE_DY * camS;
      if (b.complete) {
        ctx.drawImage(im, sx - w / 2, base - h, w, h);
      } else {
        const hh = Math.max(1, h * b.progress);
        ctx.save();
        ctx.beginPath();
        ctx.rect(sx - w / 2, base - hh, w, hh);
        ctx.clip();
        ctx.drawImage(im, sx - w / 2, base - h, w, h);
        ctx.restore();
      }
    } else {
      // Vector fallback: a low wall block that grows with progress,
      // plus a roof slab when complete.
      const hh = (b.complete ? 2.0 : 2.0 * b.progress) * camS;
      poly([l, bt, up(bt, hh), up(l, hh)], '#a08b62');
      poly([bt, r, up(r, hh), up(bt, hh)], '#7d6845');
      if (b.complete) {
        poly([up(t, hh), up(r, hh), up(bt, hh), up(l, hh)], '#9c4a2f');
      }
    }
  }
  if (!b.complete) progressBar(sx, sy - 4.9 * camS, b.progress);
}

// Barracks rendering (M3): the CC0 placeholder keep sprite
// (assets-placeholder/barracks.png) clip-revealed over a 3-cell
// foundation diamond while under construction, full-size when done;
// a stone-block vector is the fallback. The footprint is 3×3 cells.
function drawBarracks(b) {
  const [sx, sy] = worldToScreen(b.x, b.y);
  const half = (b.size * CS) / 2; // footprint half-extent, world units
  shadow(sx, sy, half * 1.45 * camS);
  const t = worldToScreen(b.x - half, b.y - half);
  const r = worldToScreen(b.x + half, b.y - half);
  const bt = worldToScreen(b.x + half, b.y + half);
  const l = worldToScreen(b.x - half, b.y + half);
  poly([t, r, bt, l], b.complete ? 'rgba(0,0,0,0.12)' : '#8d8578');
  const im = SPRITES.barracks;
  if (spriteReady(im)) {
    const w = 13.5 * camS;
    const h = w * (im.naturalHeight / im.naturalWidth);
    const base = sy + 2.6 * camS;
    if (b.complete) {
      ctx.drawImage(im, sx - w / 2, base - h, w, h);
    } else {
      const hh = Math.max(1, h * b.progress);
      ctx.save();
      ctx.beginPath();
      ctx.rect(sx - w / 2, base - hh, w, hh);
      ctx.clip();
      ctx.drawImage(im, sx - w / 2, base - h, w, h);
      ctx.restore();
    }
  } else {
    // Vector fallback: a wide stone block that grows with progress,
    // with a banner slit when complete.
    const hh = (b.complete ? 3.4 : 3.4 * b.progress) * camS;
    poly([l, bt, up(bt, hh), up(l, hh)], '#8f9299');
    poly([bt, r, up(r, hh), up(bt, hh)], '#6d7076');
    if (b.complete) {
      poly([up(t, hh), up(r, hh), up(bt, hh), up(l, hh)], '#7a4a35');
      ctx.fillStyle = '#b03a2e';
      const cx = (t[0] + bt[0]) / 2;
      ctx.fillRect(cx - 0.3 * camS, (t[1] + bt[1]) / 2 - hh, 0.6 * camS, 1.6 * camS);
    }
  }
  if (!b.complete) progressBar(sx, sy - 6.4 * camS, b.progress);
}

function drawBuilding(b) {
  if (b.kind === 1) {
    // Farm under construction: bare tilled plot + progress bar.
    // (A completed farm is drawn by the node renderer instead.)
    const [sx, sy] = worldToScreen(b.x, b.y);
    shadow(sx, sy, 3.4 * camS);
    drawFarmPlot(b.x, b.y, null);
    progressBar(sx, sy - 3.4 * camS, b.progress);
  } else if (b.kind === 2) {
    drawBarracks(b);
  } else {
    drawHouse(b);
  }
}

// Gold selection ring under a unit (shared by every unit draw).
function drawSelRing(sx, sy, rWorld) {
  ctx.strokeStyle = '#ffcf4d';
  ctx.lineWidth = Math.max(1.5, camS * 0.22);
  ctx.beginPath();
  ctx.ellipse(sx, sy, rWorld * camS, (rWorld / 2) * camS, 0, 0, Math.PI * 2);
  ctx.stroke();
}

// Thin HP bar (green → red as HP falls), drawn above a damaged unit.
function hpBar(sx, sy, frac) {
  const w = 2.6 * camS;
  const h = Math.max(2, camS * 0.42);
  const x = sx - w / 2;
  const f = Math.max(0, Math.min(1, frac));
  ctx.fillStyle = 'rgba(0,0,0,0.55)';
  ctx.fillRect(x - 1, sy - 1, w + 2, h + 2);
  ctx.fillStyle = '#20261c';
  ctx.fillRect(x, sy, w, h);
  ctx.fillStyle = f > 0.5 ? '#7dd87d' : f > 0.25 ? '#e0a13c' : '#d84a35';
  ctx.fillRect(x, sy, w * f, h);
}

function drawVillager(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 0.95 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.25);
  const anim = updateVilAnim(i, wx, wy, performance.now());
  const orig = drawOriginalVillager(anim, sx, sy);
  const spr = orig || drawSpriteH(SPRITES.villager, sx, sy, 2.4);
  const by = sy - 1.15 * u; // fallback body center / label anchor
  if (!spr) {
    ctx.fillStyle = COLORS[i % COLORS.length];
    ctx.beginPath();
    ctx.arc(sx, by, 0.85 * u, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = 'rgba(0,0,0,0.4)';
    ctx.lineWidth = 1;
    ctx.stroke();
  }
  const ck = carryKind.get(i);
  if (ck !== undefined) {
    // With our carry sheet the load is drawn on the unit itself, so the
    // kind-colored dot rides above the bundle; otherwise above the head.
    const dotH = orig ? (anim.state === 'carry' ? 3.28 : 2.6) : spr ? 2.8 : 2.35;
    ctx.fillStyle = CARRY_COLORS[ck];
    ctx.beginPath();
    ctx.arc(sx, sy - dotH * u, 0.4 * u, 0, Math.PI * 2);
    ctx.fill();
    ctx.strokeStyle = 'rgba(0,0,0,0.35)';
    ctx.stroke();
  }
  // Unit id label: debug-grade detail — MEDIUM/HIGH only.
  if (Q.idLabels) {
    ctx.fillStyle = '#101010';
    ctx.font = `${Math.max(7, Math.round(u))}px monospace`;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillText(String(i), sx, spr ? sy + 0.5 * u : by);
  }
}

// --- M3 unit drawings (v1: clean original vectors) --------------------
// Flat shapes in the same painter family as the node vectors, drawn
// from the unit's ground point (sx, sy); the sprite pipeline replaces
// them piece by piece like it did the villager. No HP bar here — the
// painter loop adds it above any damaged unit.

function legs(sx, sy, u, dxs, color) {
  ctx.strokeStyle = color;
  ctx.lineWidth = Math.max(1, 0.22 * u);
  for (const dx of dxs) {
    ctx.beginPath();
    ctx.moveTo(sx + dx * u, sy - 0.55 * u);
    ctx.lineTo(sx + dx * u, sy);
    ctx.stroke();
  }
}

function drawScout(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 1.2 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.5);
  legs(sx, sy, u, [-0.75, -0.3, 0.45, 0.85], '#5d4630');
  ctx.fillStyle = '#8a6844'; // horse body
  ctx.beginPath();
  ctx.ellipse(sx, sy - 0.95 * u, 1.15 * u, 0.55 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#8a6844'; // neck
  ctx.lineWidth = 0.5 * u;
  ctx.beginPath();
  ctx.moveTo(sx + 0.85 * u, sy - 1.2 * u);
  ctx.lineTo(sx + 1.3 * u, sy - 1.75 * u);
  ctx.stroke();
  ctx.fillStyle = '#77572f'; // head
  ctx.beginPath();
  ctx.ellipse(sx + 1.45 * u, sy - 1.8 * u, 0.38 * u, 0.22 * u, 0.5, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#31579e'; // rider torso (player blue)
  ctx.beginPath();
  ctx.ellipse(sx - 0.05 * u, sy - 1.85 * u, 0.42 * u, 0.55 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#e8c39a'; // rider head
  ctx.beginPath();
  ctx.arc(sx - 0.05 * u, sy - 2.55 * u, 0.26 * u, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#d8d8d8'; // spear, held diagonal
  ctx.lineWidth = Math.max(1, 0.14 * u);
  ctx.beginPath();
  ctx.moveTo(sx + 0.25 * u, sy - 2.5 * u);
  ctx.lineTo(sx + 1.05 * u, sy - 1.15 * u);
  ctx.stroke();
}

function drawMilitia(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 0.95 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.25);
  ctx.fillStyle = '#31579e'; // tunic (player blue)
  ctx.beginPath();
  ctx.ellipse(sx, sy - 0.85 * u, 0.62 * u, 0.8 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#e8c39a'; // head
  ctx.beginPath();
  ctx.arc(sx, sy - 1.95 * u, 0.34 * u, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#7a4a21'; // shield
  ctx.beginPath();
  ctx.arc(sx - 0.75 * u, sy - 1.0 * u, 0.42 * u, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#e8e8e8'; // sword
  ctx.lineWidth = Math.max(1, 0.16 * u);
  ctx.beginPath();
  ctx.moveTo(sx + 0.55 * u, sy - 1.5 * u);
  ctx.lineTo(sx + 1.15 * u, sy - 2.2 * u);
  ctx.stroke();
}

function drawArcher(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 0.95 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.25);
  ctx.fillStyle = '#4a6e46'; // tunic (archer green)
  ctx.beginPath();
  ctx.ellipse(sx, sy - 0.85 * u, 0.58 * u, 0.8 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#e8c39a'; // head
  ctx.beginPath();
  ctx.arc(sx, sy - 1.95 * u, 0.34 * u, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#8a6844'; // bow arc
  ctx.lineWidth = Math.max(1, 0.16 * u);
  ctx.beginPath();
  ctx.arc(sx + 0.75 * u, sy - 1.35 * u, 0.62 * u, -1.1, 1.1);
  ctx.stroke();
  ctx.strokeStyle = '#e8e8e8'; // bowstring
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(sx + 1.12 * u, sy - 1.92 * u);
  ctx.lineTo(sx + 1.12 * u, sy - 0.78 * u);
  ctx.stroke();
}

function drawSheep(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 0.9 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.2);
  legs(sx, sy, u, [-0.5, 0.5], '#4a4038');
  ctx.fillStyle = '#e9e6df'; // wool: three overlapping puffs
  for (const [dx, dy, r] of [[-0.45, -0.8, 0.5], [0.35, -0.85, 0.55], [0, -1.1, 0.5]]) {
    ctx.beginPath();
    ctx.arc(sx + dx * u, sy + dy * u, r * u, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.fillStyle = '#4a4038'; // head
  ctx.beginPath();
  ctx.ellipse(sx + 0.95 * u, sy - 0.8 * u, 0.3 * u, 0.22 * u, 0.4, 0, Math.PI * 2);
  ctx.fill();
}

function drawBoar(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 1.25 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.55);
  legs(sx, sy, u, [-0.7, -0.25, 0.5, 0.9], '#3a2a20');
  ctx.fillStyle = '#5d4030'; // bulky body
  ctx.beginPath();
  ctx.ellipse(sx, sy - 0.85 * u, 1.3 * u, 0.68 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = '#4c3424'; // head wedge
  ctx.beginPath();
  ctx.ellipse(sx + 1.25 * u, sy - 0.7 * u, 0.55 * u, 0.42 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#f2ead8'; // tusks
  ctx.lineWidth = Math.max(1, 0.18 * u);
  for (const dy of [-0.55, -0.85]) {
    ctx.beginPath();
    ctx.moveTo(sx + 1.55 * u, sy + dy * u);
    ctx.lineTo(sx + 1.8 * u, sy + (dy - 0.3) * u);
    ctx.stroke();
  }
  ctx.strokeStyle = '#3a2a20'; // bristle ridge
  ctx.lineWidth = Math.max(1, 0.2 * u);
  ctx.beginPath();
  ctx.moveTo(sx - 1.0 * u, sy - 1.35 * u);
  ctx.lineTo(sx + 0.6 * u, sy - 1.5 * u);
  ctx.stroke();
}

function drawDeer(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 1.1 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.4);
  legs(sx, sy, u, [-0.65, -0.25, 0.45, 0.8], '#8a6a42');
  ctx.fillStyle = '#b98d54'; // body
  ctx.beginPath();
  ctx.ellipse(sx, sy - 1.05 * u, 1.05 * u, 0.5 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#b98d54'; // neck
  ctx.lineWidth = 0.42 * u;
  ctx.beginPath();
  ctx.moveTo(sx + 0.8 * u, sy - 1.3 * u);
  ctx.lineTo(sx + 1.2 * u, sy - 1.95 * u);
  ctx.stroke();
  ctx.fillStyle = '#a87f49'; // head
  ctx.beginPath();
  ctx.ellipse(sx + 1.35 * u, sy - 2.05 * u, 0.34 * u, 0.2 * u, 0.4, 0, Math.PI * 2);
  ctx.fill();
  ctx.strokeStyle = '#7a5c33'; // antlers
  ctx.lineWidth = Math.max(1, 0.13 * u);
  for (const s of [-1, 1]) {
    ctx.beginPath();
    ctx.moveTo(sx + 1.3 * u, sy - 2.2 * u);
    ctx.lineTo(sx + (1.3 + 0.3 * s) * u, sy - 2.65 * u);
    ctx.stroke();
  }
}

// --- Interface panels (top bar / commands / selection / toast) ------
// updateHud() was split per the AoE2-style layout: each panel updates
// from draw(), and the two panels that rebuild DOM (commands,
// selection) are signature-gated so they only touch the DOM when
// something they display actually changed — never at 60 fps.

function updateTopBar() {
  const [food, wood, gold, stone] = sim.stockpile();
  resFood.textContent = Math.floor(food);
  resWood.textContent = Math.floor(wood);
  resGold.textContent = Math.floor(gold);
  resStone.textContent = Math.floor(stone);
  const pop = sim.population();
  const cap = sim.population_cap();
  resPop.textContent = `${pop} / ${cap}`;
  // Warn tint when housed (reason 3) or simply at the cap.
  resPop.classList.toggle('warn', sim.train_block_reason() === 3 || pop >= cap);
  const secs = Math.floor(sim.tick_count() / 60); // the sim ticks at 60 Hz
  const mm = String(Math.floor(secs / 60)).padStart(2, '0');
  const ss = String(secs % 60).padStart(2, '0');
  gameClock.textContent = `${mm}:${ss}`;
}

function cmdButton({ id, icon, label, costIcon, cost, title, disabled, active, badge }) {
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.id = id;
  btn.className = 'cmd-btn' + (active ? ' active' : '');
  btn.disabled = !!disabled;
  btn.title = title;
  let html =
    `<svg class="cmd-icon"><use href="#${icon}"/></svg>` +
    `<span class="cmd-label">${label}</span>` +
    `<span class="cmd-cost"><svg class="ui-icon"><use href="#${costIcon}"/></svg>${cost}</span>`;
  if (badge) html += `<span class="cmd-badge" id="train-badge">${badge}</span>`;
  btn.innerHTML = html;
  return btn;
}

let cmdSig = ''; // signature of the last-rendered command context

// Train-block explanations, indexed by the sim's TrainBlock tag
// (shared shape for the Town Center and production buildings).
const TRAIN_BLOCK_TITLES = [
  '',
  'Not enough resources.',
  'The training queue is full.',
  'Housed: population is at capacity — training cannot finish until a House stands.',
];

// Contextual commands, bottom-left: units selected (with a villager
// among them) -> build buttons (driving the existing placement
// flow); Town Center selected -> train Villager; Barracks selected
// -> train Militia/Archer; nothing -> a hint line. Button ids are
// stable across rebuilds; clicks are handled by the one delegated
// listener on #cmd-grid in main().
function renderCommands() {
  const selUnits = selectedUnits();
  const st = entityState();
  const hasVillager = selUnits.some((id) => st.kinds[id] === 0);
  const wood = sim.wood();
  const reason = sim.train_block_reason();
  const queue = sim.train_queue_len();
  const bk = selectedBuilding && selectedBuilding.type === 'building'
    ? buildingList().find((b) => b.id === selectedBuilding.id) || null
    : null;
  const context = selUnits.length > 0
    ? (hasVillager ? 'build' : 'units')
    : selectedBuilding
      ? (selectedBuilding.type === 'tc' ? 'tc'
        : bk && bk.kind === 2 ? 'barracks' : 'none')
      : 'none';
  const blockM = bk && context === 'barracks' ? sim.building_train_block(bk.id, 3) : -1;
  const blockA = bk && context === 'barracks' ? sim.building_train_block(bk.id, 4) : -1;
  const sig = [context, Math.floor(wood), Math.floor(sim.food()), Math.floor(sim.gold()),
    reason, queue, bk ? bk.id : -1, bk ? bk.queue : -1, bk && bk.complete ? 1 : 0,
    blockM, blockA, placement ? placement.kind : -1].join('|');
  if (sig === cmdSig) return;
  cmdSig = sig;
  cmdGrid.innerHTML = '';
  if (context === 'build') {
    cmdGrid.append(
      cmdButton({
        id: 'build-house', icon: 'i-house', label: BUILD_NAME[0],
        costIcon: 'i-wood', cost: BUILD_COST[0],
        disabled: wood < BUILD_COST[0],
        active: !!placement && placement.kind === 0,
        title: wood < BUILD_COST[0]
          ? `House — needs ${BUILD_COST[0]} wood`
          : 'House — 25 wood, +5 population room',
      }),
      cmdButton({
        id: 'build-farm', icon: 'i-farm', label: BUILD_NAME[1],
        costIcon: 'i-wood', cost: BUILD_COST[1],
        disabled: wood < BUILD_COST[1],
        active: !!placement && placement.kind === 1,
        title: wood < BUILD_COST[1]
          ? `Farm — needs ${BUILD_COST[1]} wood`
          : 'Farm — 60 wood, 175 food',
      }),
      cmdButton({
        id: 'build-barracks', icon: 'i-tc', label: BUILD_NAME[2],
        costIcon: 'i-wood', cost: BUILD_COST[2],
        disabled: wood < BUILD_COST[2],
        active: !!placement && placement.kind === 2,
        title: wood < BUILD_COST[2]
          ? `Barracks — needs ${BUILD_COST[2]} wood`
          : 'Barracks — 175 wood, trains Militia and Archers',
      }),
    );
  } else if (context === 'tc') {
    cmdGrid.append(cmdButton({
      id: 'train', icon: 'i-train', label: 'Villager',
      costIcon: 'i-food', cost: 50,
      disabled: reason === 2, // queue full; other blocks flash on click
      badge: queue > 0 ? `+${queue}` : null,
      title: TRAIN_BLOCK_TITLES[reason]
        ? `Train Villager — ${TRAIN_BLOCK_TITLES[reason]}`
        : 'Train a villager at the Town Center (50 food).',
    }));
  } else if (context === 'barracks') {
    const underConstruction = !bk.complete;
    cmdGrid.append(
      cmdButton({
        id: 'train-militia', icon: 'i-militia', label: 'Militia',
        costIcon: 'i-food', cost: 60,
        disabled: underConstruction || blockM === 2,
        badge: bk.queue > 0 ? `+${bk.queue}` : null,
        title: underConstruction
          ? 'Militia — the Barracks is under construction.'
          : `Train Militia — 60 food · 20 gold.${TRAIN_BLOCK_TITLES[blockM] ? ' ' + TRAIN_BLOCK_TITLES[blockM] : ''}`,
      }),
      cmdButton({
        id: 'train-archer', icon: 'i-archer', label: 'Archer',
        costIcon: 'i-wood', cost: 25,
        disabled: underConstruction || blockA === 2,
        title: underConstruction
          ? 'Archer — the Barracks is under construction.'
          : `Train Archer — 25 wood · 45 gold.${TRAIN_BLOCK_TITLES[blockA] ? ' ' + TRAIN_BLOCK_TITLES[blockA] : ''}`,
      }),
    );
  } else if (context === 'units') {
    const hint = document.createElement('p');
    hint.className = 'ui-cmd-hint';
    hint.textContent = 'Right-click an animal or enemy to attack, ground to move.';
    cmdGrid.append(hint);
  } else {
    const hint = document.createElement('p');
    hint.className = 'ui-cmd-hint';
    hint.textContent = 'Select a villager to build, or the Town Center to train.';
    cmdGrid.append(hint);
  }
}

let selSig = ''; // signature of the last-rendered selection panel

// Selection panel, bottom-center: single unit -> portrait + name +
// HP + hint; group -> count + one cell per unit (capped at 12 +
// "+k"); Town Center / Barracks -> portrait + queue state; nothing
// -> guidance.
function updateSelPanel() {
  const selUnits = selectedUnits();
  const st = entityState();
  const queue = sim.train_queue_len();
  const reason = sim.train_block_reason();
  const bk = selectedBuilding && selectedBuilding.type === 'building'
    ? buildingList().find((b) => b.id === selectedBuilding.id) || null
    : null;
  const context = selUnits.length > 0 ? 'units'
    : selectedBuilding
      ? (selectedBuilding.type === 'tc' ? 'tc'
        : bk && bk.kind === 2 ? 'barracks' : 'building')
      : 'none';
  const singleHp = selUnits.length === 1 ? Math.floor(st.hp[selUnits[0] * 2]) : -1;
  const bkBlock = bk ? sim.building_train_block(bk.id, 3) : -1;
  const sig = [context, selUnits.join(','), JSON.stringify(selectedBuilding),
    queue, reason, singleHp, bk ? bk.queue : -1, bkBlock,
    bk && bk.complete ? 1 : 0].join('|');
  if (sig === selSig) return;
  selSig = sig;

  selGrid.innerHTML = '';
  selInfo.classList.remove('warn');
  if (context === 'units') {
    const kind = st.kinds[selUnits[0]];
    const kindName = KIND_NAME[kind] || 'Unit';
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML =
      `<svg class="ui-icon"><use href="#${KIND_ICON[kind] || 'i-villager'}"/></svg>`;
    if (selUnits.length === 1) {
      const id = selUnits[0];
      const hpTxt = `HP ${Math.ceil(st.hp[id * 2])} / ${st.hp[id * 2 + 1]}`;
      selName.textContent = kindName;
      if (kind === 0) {
        selInfo.textContent =
          `${hpTxt} — right-click a node to gather, ground to move, an animal to attack.`;
      } else if (kind === KIND_SHEEP) {
        selInfo.textContent = `${hpTxt} — your sheep. Right-click the ground to move it.`;
      } else {
        selInfo.textContent = `${hpTxt} — right-click an animal or enemy to attack.`;
      }
    } else {
      selName.textContent = `${selUnits.length} · ${kindName}`;
      selInfo.textContent = 'Right-click to order the group.';
      const shown = selUnits.slice(0, 12);
      for (let i = 0; i < shown.length; i++) {
        const cell = document.createElement('span');
        cell.className = 'sel-cell';
        cell.innerHTML = '<svg class="ui-icon"><use href="#i-villager"/></svg>';
        selGrid.append(cell);
      }
      if (selUnits.length > shown.length) {
        const more = document.createElement('span');
        more.className = 'sel-cell more';
        more.textContent = `+${selUnits.length - shown.length}`;
        selGrid.append(more);
      }
    }
  } else if (context === 'tc') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = '<svg class="ui-icon"><use href="#i-tc"/></svg>';
    selName.textContent = 'Town Center';
    if (reason === 3) {
      selInfo.textContent = 'Housed — training stalls until a House stands.';
      selInfo.classList.add('warn');
    } else if (queue > 0) {
      selInfo.textContent = `Training a villager… · Queue ${queue}`;
    } else {
      selInfo.textContent = 'Train villagers from the Commands panel.';
    }
  } else if (context === 'barracks') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = '<svg class="ui-icon"><use href="#i-tc"/></svg>';
    selName.textContent = 'Barracks';
    if (!bk.complete) {
      selInfo.textContent = 'Under construction…';
    } else if (bkBlock === 3) {
      selInfo.textContent = 'Housed — training stalls until a House stands.';
      selInfo.classList.add('warn');
    } else if (bk.queue > 0) {
      selInfo.textContent = `Training… · Queue ${bk.queue}`;
    } else {
      selInfo.textContent = 'Train Militia or Archers from the Commands panel.';
    }
  } else if (context === 'building') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = '<svg class="ui-icon"><use href="#i-house"/></svg>';
    selName.textContent = bk ? BUILD_NAME[bk.kind] : 'Building';
    selInfo.textContent = bk && !bk.complete ? 'Under construction…' : '';
  } else {
    selPortrait.classList.add('empty');
    selPortrait.innerHTML = '<svg class="ui-icon"><use href="#i-villager"/></svg>';
    selName.textContent = 'No selection';
    selInfo.textContent =
      'Click a unit, drag to select a group, or click the Town Center.';
  }
}

// Transient toast (placement rejections, training blocks).
function updateToast() {
  if (flashText && performance.now() < flashUntil) {
    toast.textContent = flashText;
    toast.style.display = 'block';
  } else {
    toast.style.display = 'none';
  }
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
  // Ground grid strokes: a second full-grid stroke pass — pure
  // cosmetics, the first thing LOW drops after shadows.
  if (Q.groundStrokes) {
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
  }

  const nodes = nodeList();
  const buildings = buildingList();
  const st = entityState();
  const kinds = st.kinds;
  const pos = st.pos;
  const count = st.count;

  let tcPos = null;
  for (let i = 0; i < count; i++) {
    if (kinds[i] === KIND_TC) tcPos = [pos[i * 2], pos[i * 2 + 1]];
  }
  updateCarry(nodes, tcPos, pos, kinds, st.hp);

  // Active paths as thin ground lines (our living units only — the
  // TC never moves, and Gaia's routes stay hidden). Cosmetic:
  // MEDIUM/HIGH only.
  if (Q.pathLines) {
    ctx.lineWidth = 1.5;
    for (let i = 0; i < count; i++) {
      if (!isOwnUnit(st, i)) continue;
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
        NODE_DRAW[n.kind](sx, sy, n.amount / NODE_MAX[n.kind], n.x, n.y);
        amountBar(sx, sy, n.amount / NODE_MAX[n.kind]);
      },
    });
  }
  for (const b of buildings) {
    if (b.kind === 1 && b.complete) continue; // drawn as a farm node
    draws.push({ key: b.x + b.y + CS, fn: () => drawBuilding(b) });
  }
  // Unit draws dispatch by kind (M3); the dead are skipped (they
  // are tombstones in the sim, invisible here). A damaged unit gets
  // an HP bar above it.
  const UNIT_DRAW = [drawVillager, null, drawScout, drawMilitia, drawArcher,
    drawSheep, drawBoar, drawDeer];
  for (let i = 0; i < count; i++) {
    const wx = pos[i * 2];
    const wy = pos[i * 2 + 1];
    if (kinds[i] === KIND_TC) {
      // Sort by the TC's base/front edge so units in front overlap it.
      draws.push({ key: wx + wy + TC_HALF, fn: () => drawTC(wx, wy) });
    } else if (st.hp[i * 2] >= 1) {
      const drawFn = UNIT_DRAW[kinds[i]] || drawVillager;
      const hp = st.hp[i * 2];
      const maxHp = st.hp[i * 2 + 1];
      draws.push({
        key: wx + wy,
        fn: () => {
          drawFn(i, wx, wy);
          if (hp < maxHp) {
            const [sx, sy] = worldToScreen(wx, wy);
            hpBar(sx, sy - 3.3 * camS, hp / maxHp);
          }
        },
      });
    }
  }
  draws.sort((a, b) => a.key - b.key);
  for (const d of draws) d.fn();

  // Arrows in flight (M3): interpolate along the sim's fixed
  // trajectory. MEDIUM/HIGH draw the cosmetic arc + tail line; LOW
  // draws a plain dot at the same interpolated position — the
  // arrow's existence and position are information, the arc is not.
  {
    const proj = sim.projectiles();
    const now = sim.tick_count();
    if (Q.projTrail) ctx.strokeStyle = '#e8ddc8';
    if (Q.projTrail) ctx.lineWidth = Math.max(1, camS * 0.16);
    else ctx.fillStyle = '#e8ddc8';
    for (let p = 0; p < proj.length; p += 6) {
      const span = Math.max(1, proj[p + 5] - proj[p + 4]);
      const t = Math.max(0, Math.min(1, (now - proj[p + 4]) / span));
      const wx = proj[p] + (proj[p + 2] - proj[p]) * t;
      const wy = proj[p + 1] + (proj[p + 3] - proj[p + 1]) * t;
      const [sx, sy] = worldToScreen(wx, wy);
      if (!Q.projTrail) {
        ctx.beginPath();
        ctx.arc(sx, sy, Math.max(1.2, camS * 0.28), 0, Math.PI * 2);
        ctx.fill();
        continue;
      }
      const lift = Math.sin(Math.PI * t) * 1.2 * camS;
      const dx = proj[p + 2] - proj[p];
      const dy = proj[p + 3] - proj[p + 1];
      const dl = Math.hypot(dx, dy) || 1;
      const tail = 0.9 * camS;
      ctx.beginPath();
      ctx.moveTo(sx - (dx / dl) * tail, sy - lift - (dy / dl) * tail * 0.5);
      ctx.lineTo(sx, sy - lift);
      ctx.stroke();
    }
  }

  // Building selection: a gold outline around the footprint — the
  // building counterpart of the unit selection ring.
  {
    let outline = null; // [centerX, centerY, halfExtent]
    if (selectedBuilding && selectedBuilding.type === 'tc') {
      const id = selectedBuilding.id;
      outline = [pos[id * 2], pos[id * 2 + 1], TC_HALF];
    } else if (selectedBuilding && selectedBuilding.type === 'building') {
      const b = buildings.find((bb) => bb.id === selectedBuilding.id);
      if (b) outline = [b.x, b.y, (b.size * CS) / 2];
    }
    if (outline) {
      const [bx, by, half] = outline;
      const pts = [
        worldToScreen(bx - half, by - half),
        worldToScreen(bx + half, by - half),
        worldToScreen(bx + half, by + half),
        worldToScreen(bx - half, by + half),
      ];
      poly(pts, 'rgba(255, 207, 77, 0.10)');
      ctx.strokeStyle = 'rgba(255, 207, 77, 0.95)';
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.moveTo(pts[0][0], pts[0][1]);
      for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]);
      ctx.closePath();
      ctx.stroke();
    }
  }

  // Placement ghost: the footprint under the cursor, green when the
  // mirrored sim rules allow building there, red when they don't.
  if (placement && mouseWorld) {
    const [cx, cy] = worldToCell(mouseWorld[0], mouseWorld[1]);
    const ok = mirrorValid(placement.kind, cx, cy);
    const size = BUILD_SIZE[placement.kind];
    const x0 = -HALF + cx * CS;
    const y0 = -HALF + cy * CS;
    const pts = [
      worldToScreen(x0, y0),
      worldToScreen(x0 + size * CS, y0),
      worldToScreen(x0 + size * CS, y0 + size * CS),
      worldToScreen(x0, y0 + size * CS),
    ];
    poly(pts, ok ? 'rgba(90, 230, 120, 0.30)' : 'rgba(240, 70, 55, 0.30)');
    ctx.strokeStyle = ok ? 'rgba(140, 255, 170, 0.9)' : 'rgba(255, 110, 95, 0.9)';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(pts[0][0], pts[0][1]);
    for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]);
    ctx.closePath();
    ctx.stroke();
  }

  // Selection box while left-dragging: translucent gold rect.
  if (lDrag && lDrag.moved) {
    const x = Math.min(lDrag.x0, lDrag.x1);
    const y = Math.min(lDrag.y0, lDrag.y1);
    const w = Math.abs(lDrag.x1 - lDrag.x0);
    const h = Math.abs(lDrag.y1 - lDrag.y0);
    ctx.fillStyle = 'rgba(255, 207, 77, 0.12)';
    ctx.fillRect(x, y, w, h);
    ctx.strokeStyle = 'rgba(255, 207, 77, 0.85)';
    ctx.lineWidth = 1.5;
    ctx.strokeRect(x, y, w, h);
  }

  updateTopBar();
  renderCommands();
  updateSelPanel();
  updateToast();

  // Minimap: reuses this frame's nodes/buildings/entity state, so
  // it costs no extra ABI calls. Gaia animals get their own dot
  // colors; the dead are skipped.
  if (mmGeom) {
    const GAIA_COLORS = { 5: '#cfd4d8', 6: '#a2532f', 7: '#c9a35f' };
    const mmEntities = [];
    for (let i = 0; i < count; i++) {
      if (kinds[i] !== KIND_TC && st.hp[i * 2] < 1) continue;
      mmEntities.push({
        x: pos[i * 2], y: pos[i * 2 + 1], kind: kinds[i],
        color: st.owners[i] === 1 ? (GAIA_COLORS[kinds[i]] || '#9a9a9a') : OWN_COLOR,
        selected: selection.has(i),
      });
    }
    renderMinimap(mmCtx, mmGeom, {
      gridW: GW, gridH: GH, cellSize: CS, obstacles: OBST,
      nodes, buildings, entities: mmEntities,
      cam: { s: camS, ox: camOx, oy: camOy, viewW: W, viewH: H },
    });
  }
}

main().catch((e) => {
  // No #hud text line anymore: load failures surface on the toast.
  toast.textContent = 'Failed to load the game (WASM): ' + e;
  toast.style.display = 'block';
  console.error(e);
});
