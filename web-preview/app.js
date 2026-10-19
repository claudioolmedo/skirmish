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
// M4 adds the input & command completion: Shift+right-click queued
// orders (SEL-12), rally points set by right-clicking with the Town
// Center / Barracks selected, with the flag rendered (SEL-13),
// control groups on Ctrl/number keys (SEL-07), the idle villager /
// idle military keys and top-bar indicators (SEL-08/09, INF-03),
// H for the Town Center (SEL-10), the 60-unit selection cap
// (SEL-05), right-click orders on the minimap (SEL-16), and
// formation-keeping group moves.
// M13 adds the classic grid hotkeys (play/hotkeys.js): villager
// build pages on Q / W with the letter on each button placing it
// (Q, E raises a House), Q / W / E position keys on a selected
// building's panel, A for force drop-off, Ctrl+letter go-tos —
// and edge scrolling, the camera pan every other method shares
// the frame with.
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

// M10 load order (Claudio's direction): the WASM sim is NOT
// imported statically. The lobby — browser, region pings, rooms —
// must work without ever fetching the .wasm; the heavy module is
// pulled in by ensureGameLoaded() inside the loading phase, when
// a match actually starts. `WasmSim` below is assigned there.
let WasmSim = null;
import {
  createMinimap, renderMinimap, minimapToWorld, cameraCenteredOn,
} from './play/minimap.js';
import {
  collectProbeSignals, scoreProbe, resolveMode, levelForMode,
  settingsFor, createGovernor,
} from './play/quality.js';
import {
  createControlGroups, createTapTracker, nextInCycle,
} from './play/groups.js';
import {
  TURN_TICKS as NET_TURN_TICKS, REJOIN_GRACE_MS, LOAD_TIMEOUT_MS,
  applyCommand as netApplyCommand, tcIdOf as netTcIdOf,
  createSession, createRecorder, replayEvents, isReplay,
  createRelayClient, probeRegionPing, makeRoomId, buildPlayUrl,
  linkJoinRoom, pickFastestRegion, REGIONS as NET_REGIONS,
} from './play/netplay.js';
import {
  LOBBIES_REFRESH_MS, parseLobbies, lobbyRowModel, sortLobbyRows,
  joinTarget, filterLobbyRows, selectionSurvives, pingQuality,
} from './play/lobby.js';
import {
  checkPassword, peerAdmitted, guestAdmitted, canToggleReady,
  startGate, guestSlotOf, slotIsAi, slotSealed, slotAiMode,
  slotLabel, roomConfigText,
} from './play/room.js';
import {
  PAGE_KINDS, resolveHotkey, badgeForButton, edgePan,
} from './play/hotkeys.js';

const assetUrl = (rel) => new URL(rel, import.meta.url).href;

const params = new URLSearchParams(location.search);
const seed = parseInt(params.get('seed') || '7', 10);

const canvas = document.getElementById('view');
const ctx = canvas.getContext('2d');
// Interface chrome (play/index.html + play/ui.css). The command-grid
// buttons (#train / #build-0 / #build-1 / the research buttons) are
// REBUILT per selection context by renderCommands(), so they are
// reached through one delegated listener on #cmd-grid, never through
// stored refs.
const resFood = document.getElementById('res-food');
const resWood = document.getElementById('res-wood');
const resGold = document.getElementById('res-gold');
const resStone = document.getElementById('res-stone');
const resPop = document.getElementById('res-pop');
const resAge = document.getElementById('res-age'); // M6: the player's age
const pauseBtn = document.getElementById('pause-btn');
// Idle indicators (M4, INF-03): always-visible counts in the top bar;
// clicking one does what the '.' / ',' keys do (SEL-08/09).
const idleVilBtn = document.getElementById('idle-vil-btn');
const idleVilCount = document.getElementById('idle-vil-count');
const idleMilBtn = document.getElementById('idle-mil-btn');
const idleMilCount = document.getElementById('idle-mil-count');
const cmdGrid = document.getElementById('cmd-grid');
const selPortrait = document.getElementById('sel-portrait');
const selName = document.getElementById('sel-name');
const selInfo = document.getElementById('sel-info');
const selGrid = document.getElementById('sel-grid');
const minimapCanvas = document.getElementById('minimap');
const mmCtx = minimapCanvas.getContext('2d');
const toast = document.getElementById('toast');
const pauseMenu = document.getElementById('pause-menu');
const resumeBtn = document.getElementById('resume-btn');
const restartBtn = document.getElementById('restart-btn');
const helpBtn = document.getElementById('help-btn');
const helpOverlay = document.getElementById('help-overlay');
const helpClose = document.getElementById('help-close');
const menuClose = document.getElementById('menu-close');
const helpCloseX = document.getElementById('help-close-x');
// M5 match-end overlay + start-overlay mode radios.
const endOverlay = document.getElementById('end-overlay');
const endTitle = document.getElementById('end-title');
const endDetail = document.getElementById('end-detail');
const endRestartBtn = document.getElementById('end-restart-btn');
const endMenuBtn = document.getElementById('end-menu-btn');
const modeInputs = document.querySelectorAll('input[name="match-mode"]');

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
  4: 'Archer', 5: 'Sheep', 6: 'Boar', 7: 'Deer', 8: 'Man-at-Arms',
};
// Selection-panel portraits per unit kind (SVG symbol ids in
// play/index.html).
const KIND_ICON = {
  0: 'i-villager', 2: 'i-villager', 3: 'i-militia', 4: 'i-archer', 5: 'i-pop',
  8: 'i-militia',
};

// Building kind tags (must match the sim's BuildingKind::tag):
// 0 house, 1 farm, 2 barracks (M2.5/M3); 3 lumber camp, 4 mill,
// 5 mining camp, 6 archery range, 7 stable, 8 blacksmith (M6).
// Costs and footprints mirror the sim (MECHANICS-STUDY §5).
const BUILD_COST = [25, 60, 175, 100, 100, 100, 175, 175, 150];
const BUILD_NAME = ['House', 'Farm', 'Barracks', 'Lumber Camp', 'Mill',
  'Mining Camp', 'Archery Range', 'Stable', 'Blacksmith'];
const BUILD_SIZE = [2, 2, 3, 2, 2, 2, 3, 3, 3];
const BUILD_ICON = ['i-house', 'i-farm', 'i-tc', 'i-lumber', 'i-mill',
  'i-mining', 'i-range', 'i-stable', 'i-blacksmith'];
// Building max HP by kind tag (the sim's building table, study
// §5) — for the enemy-building info panel (M5).
const BUILD_MAXHP = [550, 480, 1200, 550, 550, 550, 1500, 1500, 1200];
// WASM build commands by kind tag (M6 completes the set).
const PUSH_BUILD = ['push_build_house', 'push_build_farm', 'push_build_barracks',
  'push_build_lumber_camp', 'push_build_mill', 'push_build_mining_camp',
  'push_build_archery_range', 'push_build_stable', 'push_build_blacksmith'];
const PUSH_BUILD_QUEUED = PUSH_BUILD.map((n) => `${n}_queued`);
// Own-building minimap colors by kind tag (M6); kinds without an
// entry keep the minimap's parchment default, the enemy's
// buildings are always ENEMY_COLOR.
const BUILDING_MINIMAP_COLOR = {
  3: '#8a6b42', 4: '#e5d9a8', 5: '#b9a24a', 6: '#c98f4e', 7: '#9a6a44', 8: '#6f6f76',
};

// Ages and technologies (M6). Age tags match the sim's Age::tag;
// tech tags match TechId::tag. Costs/times mirror the sim's table
// (study §8, OPENING-SPEC §2.6–2.7).
const AGE_NAMES = ['Dark Age', 'Feudal Age'];
const TECH_NAME = {
  0: 'Loom', 1: 'Feudal Age', 2: 'Fletching', 3: 'Scale Mail Armor',
  4: 'Padded Archer Armor', 5: 'Man-at-Arms',
};
// Research-block explanations, indexed by the sim's ResearchBlock
// tag (0 ok — unused, 1 funds, 2 queue full, 3 already done,
// 4 requirements).
const RESEARCH_BLOCK_TITLES = [
  '',
  'Not enough resources.',
  'The queue is full.',
  'Already researched.',
  'Requirements not met yet.',
];
// A queue entry's display name: unit codes are entity kind tags;
// research codes are 100 + tech tag.
const queueEntryName = (code) => (code < 100
  ? (KIND_NAME[code] || 'Unit')
  : (TECH_NAME[code - 100] || 'Research'));

// --- Game shell state (client-only; never sim input) -------------------
let playing = false; // the match is live: the sim ticks (set at init — no start screen)
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
  // SEL-05: one selection holds at most 60 units. Every path that
  // builds a selection (click, box, shift, double-click, control
  // groups, idle keys) funnels through here, so the cap lives here.
  selection = new Set([...set].slice(0, SELECTION_CAP));
  selectedBuilding = null;
  buildPage = null; // M13: a selection change closes any build page
  buildingGroup = null;
}
function selectBuilding(descOrNull) {
  selectedBuilding = descOrNull;
  selection = new Set();
  buildPage = null;
  buildingGroup = null;
}

// SEL-05: the selection cap (the rulebook's number).
const SELECTION_CAP = 60;

// Control groups (M4, SEL-07): membership by entity id, client-side
// only — the store (play/groups.js) is pure logic; the sim stays the
// truth about which members are alive. The tap tracker powers the
// double-tap-to-center gesture.
const controlGroups = createControlGroups();
const groupTaps = createTapTracker();
// Idle-key cycling memory (SEL-08/09): the last id each key handed
// out, so pressing again moves to the NEXT idle unit.
let idleCycleVil = -1;
let idleCycleMil = -1;

let mmGeom = null; // minimap geometry, created once the grid is known
const OWN_COLOR = '#4da3ff'; // our player blue on the minimap (villager tunic family)
// M5: the opponent's player color — red, the classic counterpart to
// the human's blue. Used on the minimap, the TC pennant, unit cloth
// in the vector draws, and the enemy sheep's brand mark.
const ENEMY_COLOR = '#e0483e';
const ENEMY_CLOTH = '#a02c22'; // scout/militia cloth, enemy
const ENEMY_ARCHER_CLOTH = '#8e3b2f'; // archer cloth, enemy
// Owner of the entity whose draw is running (set by the painter
// loop before each unit draw): how the draw functions pick the
// player color without changing every signature.
let drawOwner = 0;

// --- M5 match mode ------------------------------------------------------
// 'sandbox' (no opponent — the pre-M5 game) | 'easy' | 'normal'
// (the scripted AI at its two behavior profiles). Chosen on the
// start overlay, default AI Normal; ?mode= works like ?seed= and
// ?quality= (a support tool, never stored). MODE_TAG is the sim's
// MatchMode tag for WasmSim.new_match.
const MODE_TAG = { sandbox: 0, easy: 1, normal: 2, online: 3 };
let matchMode = ['sandbox', 'easy', 'normal'].includes(params.get('mode'))
  ? params.get('mode')
  : 'normal';
let builtMode = null; // the mode the current sim instance was built with
let matchEnded = false; // the victory/defeat overlay is up; the sim holds

// --- M8: lockstep multiplayer -------------------------------------------
// Send-a-link 1v1 over the dumb relay (play/netplay.js holds the
// whole protocol; this file integrates it). `net` is null in every
// local mode — the single-player path below is byte-for-byte the
// pre-M8 game; netplay activates only from the lobby or a room link.
// The seat this client plays: 0 = the first player (host), 2 = the
// second (guest). All "own/foe" checks and per-seat HUD reads go
// through localOwner/foeOwner() so the guest sees its own village.
const RELAY_HTTP = 'https://skirmish-relay.hoardserdure7.workers.dev';
const BUILD_VERSION = 'm8';
let localOwner = 0;
let myTcId = 0; // entity id of the LOCAL player's Town Center
const foeOwner = () => (localOwner === 0 ? 2 : 0);
let net = null; // netplay controller (see openLobby / startNetMatch)
let aiRoom = null; // M12: { mode, seed } while an AI-room match is the
// live game. The room dissolves the moment such a match begins (the
// match is local — see beginAiMatch), so this small marker is what
// lets Restart replay the same configuration and Back to menu find
// the Lobby Browser again, exactly like a local match's mode does.
let localRecorder = null; // replay recorder for LOCAL matches (M8)
let playback = null; // replay playback state while a replay runs

/// Show the victory/defeat overlay (M5). Called by the frame loop
/// the first tick the sim reports a decided match; the sim itself
/// would keep ticking harmlessly, but the picture freezes at the
/// decision — Restart and Back to menu are the exits.
function showMatchEnd(ms) {
  matchEnded = true;
  const secs = Math.floor(sim.tick_count() / 60);
  const mm = String(Math.floor(secs / 60)).padStart(2, '0');
  const ss = String(secs % 60).padStart(2, '0');
  endTitle.textContent = ms === 1 ? 'Victory!' : 'Defeat';
  endDetail.textContent = ms === 1
    ? `The enemy has been eliminated. Match time ${mm}:${ss}.`
    : `Your village has fallen. Match time ${mm}:${ss}.`;
  endOverlay.style.display = 'flex';
}
let lDrag = null; // left-button drag state, canvas px: { x0, y0, x1, y1, moved }
let placement = null; // { kind } while placing a building
// M13 hotkey state (client-only, like placement): the armed
// villager build page ('eco' | 'mil' | null), the go-to cycle
// memory (building kind -> last id handed out), and the building
// id list when a Ctrl+Shift go-to selected a whole type at once
// (selectedBuilding stays the primary; the panel addresses it).
let buildPage = null;
let gotoCycleMem = {};
let buildingGroup = null;
// M13 edge scrolling: the last pointer position in client px and
// whether the pointer currently rides over the game canvas (not
// the UI bars, not an overlay). Fed by the pointer listeners in
// main(); consumed by applyEdgeScroll() on the frame loop.
let mouseClientPt = null;
let mouseOverCanvas = false;
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
let placeholderSpritesLoaded = false;
function loadPlaceholderSprites() {
  // M10 load order: artwork loads with the game (loading phase),
  // never at page open — the lobby must stay light.
  if (placeholderSpritesLoaded) return;
  placeholderSpritesLoaded = true;
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

// --- Original building art (Base44 test set, batch 1) ------------------
// One PNG per building kind, AI-generated on Claudio's Base44 account
// from this project's standardized prompts (original designs after
// real medieval architecture — clean-room; provenance:
// assets-original/buildings/README.md). Wired in as a TEST set so the
// look can be judged in a real match before more batches are made.
// Draw contract (same as the placeholder sprites it replaces): the
// image is scaled to a per-kind width in world units with aspect
// preserved and anchored bottom-center, its base planted `dyWorld`
// toward the footprint's front edge. While a building is under
// construction the image is clip-revealed by progress over the
// standard foundation diamond — the same construction feedback the
// placeholder sprites always used. Exception: the House has
// dedicated construction stage frames (house-stage1..3.png, one
// generated strip sliced into thirds of the build) drawn full-size
// instead of the clip-reveal; every building kind now has its own
// stage strip (see buildingStageImage). Fallback chain per building: this
// art -> whatever the renderer used before (original stage art / CC0
// placeholder / vector). One image serves both players for the test
// (no per-owner tint yet). Loading starts inside the M10 loading
// phase, and ensureGameLoaded() AWAITS the whole set settling
// (loaded or errored, capped — see whenBuildingArtSettled) before
// any match may start: a match that opened on unsettled art drew
// the CC0 placeholder first and popped to the real art when it
// landed (Claudio's bug report, 2026-10-18).
const BUILDING_ART = { img: {}, settled: {}, loaded: false };
const BUILDING_ART_FILES = {
  tc: 'town-center.png',
  house: 'house.png',
  houseStage1: 'house-stage1.png',
  houseStage2: 'house-stage2.png',
  houseStage3: 'house-stage3.png',
  barracks: 'barracks.png',
  barracksStage1: 'barracks-stage1.png',
  barracksStage2: 'barracks-stage2.png',
  barracksStage3: 'barracks-stage3.png',
  lumber: 'lumber-camp.png',
  lumberStage1: 'lumber-camp-stage1.png',
  lumberStage2: 'lumber-camp-stage2.png',
  lumberStage3: 'lumber-camp-stage3.png',
  mill: 'mill.png',
  millStage1: 'mill-stage1.png',
  millStage2: 'mill-stage2.png',
  millStage3: 'mill-stage3.png',
  mining: 'mining-camp.png',
  miningStage1: 'mining-camp-stage1.png',
  miningStage2: 'mining-camp-stage2.png',
  miningStage3: 'mining-camp-stage3.png',
  range: 'archery-range.png',
  rangeStage1: 'archery-range-stage1.png',
  rangeStage2: 'archery-range-stage2.png',
  rangeStage3: 'archery-range-stage3.png',
  stable: 'stable.png',
  stableStage1: 'stable-stage1.png',
  stableStage2: 'stable-stage2.png',
  stableStage3: 'stable-stage3.png',
  blacksmith: 'blacksmith.png',
  blacksmithStage1: 'blacksmith-stage1.png',
  blacksmithStage2: 'blacksmith-stage2.png',
  blacksmithStage3: 'blacksmith-stage3.png',
  farm: 'farm.png',
  farmState1: 'farm-state1.png',
  farmState2: 'farm-state2.png',
  farmState3: 'farm-state3.png',
  farmState4: 'farm-state4.png',
};
// The construction-stage frame for a building kind at a build
// progress (thirds, the House rule): stage 1 foundation + posts,
// stage 2 walls + scaffolding, stage 3 structure complete with roof
// beams. Null for kinds without stage art (the Town Center, the
// Farm) — drawArtImage(null) is false, so callers just fall through
// to their existing chain.
function buildingStageImage(kind, progress) {
  const key = BUILDING_ART_KEY[kind];
  if (!key || kind === 0) return null; // the House has its own path
  const n = progress < 1 / 3 ? 1 : progress < 2 / 3 ? 2 : 3;
  return BUILDING_ART.img[key + 'Stage' + n] || null;
}
// Building kind (sim ABI order) -> BUILDING_ART key, for the kinds
// drawn by drawBuilding / drawNewBuilding.
const BUILDING_ART_KEY = {
  0: 'house', 2: 'barracks', 3: 'lumber', 4: 'mill', 5: 'mining',
  6: 'range', 7: 'stable', 8: 'blacksmith',
};
// The settle cap: the loading gate waits for the art, but a hung
// image (stalled connection) must never block a match longer
// than this. Whatever is still in flight past the cap keeps the
// in-draw fallback chain (with the pending guard in drawTC /
// drawBarracks covering the pop until it lands).
const BUILDING_ART_SETTLE_TIMEOUT_MS = 8000;
let buildingArtSettledPromise = null;
function loadBuildingArt() {
  if (BUILDING_ART.loaded) return;
  BUILDING_ART.loaded = true;
  const pending = [];
  for (const [key, file] of Object.entries(BUILDING_ART_FILES)) {
    const im = new Image();
    BUILDING_ART.img[key] = im;
    BUILDING_ART.settled[key] = false;
    pending.push(new Promise((resolve) => {
      const done = () => { BUILDING_ART.settled[key] = true; resolve(); };
      im.addEventListener('load', done);
      im.addEventListener('error', done);
      im.src = assetUrl('assets-original/buildings/' + file);
      // A cached image may already be complete (loaded or broken)
      // the moment src is set — that counts as settled too.
      if (im.complete) done();
    }));
  }
  const cap = new Promise((r) => { setTimeout(r, BUILDING_ART_SETTLE_TIMEOUT_MS); });
  buildingArtSettledPromise = Promise.race([Promise.all(pending), cap]);
}
// Resolves once every building-art image has settled (loaded or
// errored), or after BUILDING_ART_SETTLE_TIMEOUT_MS — whichever
// comes first. Resolves immediately if art was never kicked off.
function whenBuildingArtSettled() {
  return buildingArtSettledPromise || Promise.resolve();
}
// True while a building-art image is still in flight (not loaded,
// not errored). False for keys that never started loading.
function buildingArtPending(key) {
  return BUILDING_ART.settled[key] === false;
}
// --- Original node + animal art (Base44 sets) -----------------------
// Same provenance and loading phase as the building art (see
// BUILDING_ART above; provenance: assets-original/nodes/README.md
// and assets-original/animals/README.md). Nodes: one image per
// resource kind, preferred over the CC0 placeholder sprites on the
// SAME size/anchor contract the placeholders draw with. Animals:
// four poses per animal from one generated strip (1 idle, 2/3 walk
// strides, 4 graze/root/browse), drawn bottom-anchored at the
// entity, flipped horizontally when it faces right (the poses face
// left). Both sets join the loading phase's readiness set alongside
// the building art — same settle semantics (loaded or errored,
// capped), so a match never opens on unsettled art for them either.
function loadArtSet(art, files, dir) {
  if (art.loaded) return art.settledPromise;
  art.loaded = true;
  const pending = [];
  for (const [key, file] of Object.entries(files)) {
    const im = new Image();
    art.img[key] = im;
    art.settled[key] = false;
    pending.push(new Promise((resolve) => {
      const done = () => { art.settled[key] = true; resolve(); };
      im.addEventListener('load', done);
      im.addEventListener('error', done);
      im.src = assetUrl(dir + file);
      if (im.complete) done();
    }));
  }
  const cap = new Promise((r) => { setTimeout(r, BUILDING_ART_SETTLE_TIMEOUT_MS); });
  art.settledPromise = Promise.race([Promise.all(pending), cap]);
  return art.settledPromise;
}
const NODE_ART = { img: {}, settled: {}, loaded: false, settledPromise: null };
const NODE_ART_FILES = {
  tree: 'tree.png', berry: 'berry-bush.png', gold: 'gold.png', stone: 'stone.png',
};
function loadNodeArt() {
  return loadArtSet(NODE_ART, NODE_ART_FILES, 'assets-original/nodes/');
}
function whenNodeArtSettled() {
  return NODE_ART.settledPromise || Promise.resolve();
}
const ANIMAL_ART = { img: {}, settled: {}, loaded: false, settledPromise: null };
const ANIMAL_ART_FILES = {
  sheep1: 'sheep-pose1.png', sheep2: 'sheep-pose2.png',
  sheep3: 'sheep-pose3.png', sheep4: 'sheep-pose4.png',
  boar1: 'boar-pose1.png', boar2: 'boar-pose2.png',
  boar3: 'boar-pose3.png', boar4: 'boar-pose4.png',
  deer1: 'deer-pose1.png', deer2: 'deer-pose2.png',
  deer3: 'deer-pose3.png', deer4: 'deer-pose4.png',
};
function loadAnimalArt() {
  return loadArtSet(ANIMAL_ART, ANIMAL_ART_FILES, 'assets-original/animals/');
}
function whenAnimalArtSettled() {
  return ANIMAL_ART.settledPromise || Promise.resolve();
}

// Draw building art bottom-anchored like the placeholder sprites:
// full image when frac >= 1, otherwise clip-revealed to the bottom
// `frac` of the image (the building visibly rises). False when the
// image is not loaded yet.
function drawArtImage(im, sx, sy, wWorld, dyWorld, frac) {
  if (!spriteReady(im)) return false;
  const w = wWorld * camS;
  const h = w * (im.naturalHeight / im.naturalWidth);
  const base = sy + dyWorld * camS;
  if (frac >= 1) {
    ctx.drawImage(im, sx - w / 2, base - h, w, h);
  } else {
    const hh = Math.max(1, h * frac);
    ctx.save();
    ctx.beginPath();
    ctx.rect(sx - w / 2, base - hh, w, hh);
    ctx.clip();
    ctx.drawImage(im, sx - w / 2, base - h, w, h);
    ctx.restore();
  }
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
  imgRed: {}, // M5: red palette-swap variants, built per sheet load
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
  // M5: the opponent's villagers draw from the red variant of the
  // same sheets (generated at load in loadTierSprites) — one sprite
  // pipeline, two player colors.
  const red = drawOwner === foeOwner() ? VIL_ORIGINAL.imgRed[anim.state] : null;
  const im = red || VIL_ORIGINAL.img[anim.state];
  if (!im || !(im.naturalWidth > 0 || im.width > 0)) return false;
  const C = VIL_ORIGINAL.cell;
  const k = (camS * VIL_ORIGINAL.drawScale) / VIL_ORIGINAL.pxPerUnit;
  ctx.drawImage(im,
    anim.frame * C, anim.dir * C, C, C,
    sx - VIL_ORIGINAL.ax * k, sy - VIL_ORIGINAL.ay * k, C * k, C * k);
  return true;
}

/// The enemy-red variant of a villager sheet (M5): a copy of the
/// sheet with its blue-dominant pixels channel-swapped to red —
/// the tunic (and only the tunic family) changes color, exactly
/// like a palette swap in the reference pipeline. Built once per
/// sheet load, client-side; the sim never sees it.
function makeRedSheet(im) {
  const c = document.createElement('canvas');
  c.width = im.naturalWidth;
  c.height = im.naturalHeight;
  const g = c.getContext('2d');
  g.drawImage(im, 0, 0);
  const d = g.getImageData(0, 0, c.width, c.height);
  const px = d.data;
  for (let p = 0; p < px.length; p += 4) {
    const r = px[p];
    const gg = px[p + 1];
    const b = px[p + 2];
    if (b > r + 24 && b > gg + 12) {
      px[p] = b;
      px[p + 2] = r;
    }
  }
  g.putImageData(d, 0, 0);
  return c;
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
  VIL_ORIGINAL.imgRed = {}; // rebuilt as each new sheet lands
  for (const state of Object.keys(VIL_ORIGINAL.frames)) {
    const im = new Image();
    im.src = assetUrl(base + `villager-${state}.png`);
    VIL_ORIGINAL.img[state] = im;
    // M5: the opponent's red variant of this sheet (palette swap).
    im.addEventListener('load', () => {
      VIL_ORIGINAL.imgRed[state] = makeRedSheet(im);
    });
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

const qualityModeBtns = {
  auto: document.getElementById('q-auto'),
  high: document.getElementById('q-high'),
  medium: document.getElementById('q-medium'),
  low: document.getElementById('q-low'),
};

function updateQualityUi() {
  // No quality readout in the top bar (Claudio, 2026-10-17 — it took
  // space for nothing); this only marks the active menu button.
  for (const [mode, btn] of Object.entries(qualityModeBtns)) {
    if (btn) btn.classList.toggle('active', qualityMode === mode);
  }
}

// Apply a level end-to-end. `announce` shows the one subtle toast
// the spec allows per automatic change; the initial application at
// load is silent (the level is not displayed anywhere by default).
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

// Boot: set the DPR scale now; main()'s first fitCanvas() call
// picks dprScale up. The initial tier's sheets are NOT fetched
// here anymore (M10 load order): ensureGameLoaded() loads them
// inside the loading phase, when a match actually starts.
// (applyQuality's loadTierSprites call is the post-boot path —
// it also resizes the canvas, which must wait until the camera
// state exists.)
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
// M7: the projection lifts elevated ground — screen y drops by
// (height level × ELEV_LIFT) per world unit of zoom, so units,
// buildings and overlays standing on a hill render on the hill.
// One projection point keeps every sprite consistent.
const ELEV_LIFT = 1.1;
const worldToScreen = (wx, wy) => {
  const [sx, sy] = w2s(wx, wy, camS, camOx, camOy);
  return [sx, sy - heightAtWorld(wx, wy) * ELEV_LIFT * camS];
};

let sim, HALF, GW, GH, CS, OBST;
// M7 world layers (flat row-major, one byte per cell, indexed
// gy * GW + gx): terrain (0 land / 1 water), heights (0..=4), and
// the LOCAL player's fog grid (0 unexplored / 1 explored /
// 2 visible) — the only knowledge this client renders from.
let TERRAIN = null, HEIGHTS = null, FOG = null;
// Land cells touching water — sand-colored shorelines (computed
// when a match's layers load; terrain never changes mid-match).
let SHORE = new Set();
// Last-known enemy buildings (VIS-06): id -> snapshot of the
// building record as last seen visible. Drawn dimmed while their
// ground is explored-but-not-visible; dropped on re-look if gone.
const seenBuildings = new Map();

function heightAtCell(gx, gy) {
  if (!HEIGHTS || gx < 0 || gy < 0 || gx >= GW || gy >= GH) return 0;
  return HEIGHTS[gy * GW + gx];
}
function heightAtWorld(wx, wy) {
  if (!HEIGHTS) return 0;
  const [gx, gy] = worldToCell(wx, wy);
  return HEIGHTS[gy * GW + gx];
}
function fogAtWorld(wx, wy) {
  if (!FOG) return 2;
  const [gx, gy] = worldToCell(wx, wy);
  return FOG[gy * GW + gx];
}
const cellVisibleW = (wx, wy) => fogAtWorld(wx, wy) === 2;
const cellExploredW = (wx, wy) => fogAtWorld(wx, wy) >= 1;

// Mix a #rrggbb color toward a pale tint by t (0..1) — the hill
// lightening for terrain rendering.
function shade(hex, t) {
  const n = parseInt(hex.slice(1), 16);
  const r = (n >> 16) & 255, g = (n >> 8) & 255, b = n & 255;
  const mix = (c) => Math.round(c + (224 - c) * t);
  return `rgb(${mix(r)},${mix(g)},${mix(b)})`;
}

// Pull the static world layers for the current sim and compute the
// shoreline set. Terrain/heights are fixed per match; fog and
// obstacles refresh per frame in draw().
function refreshWorldLayers() {
  GW = sim.grid_width();
  GH = sim.grid_height();
  CS = sim.cell_size();
  HALF = sim.world_half();
  TERRAIN = sim.terrain();
  HEIGHTS = sim.heights();
  OBST = sim.obstacles();
  FOG = myFog();
  SHORE = new Set();
  for (let gy = 0; gy < GH; gy++) {
    for (let gx = 0; gx < GW; gx++) {
      const i = gy * GW + gx;
      if (TERRAIN[i] === 1) continue;
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const nx = gx + dx, ny = gy + dy;
        if (nx >= 0 && ny >= 0 && nx < GW && ny < GH && TERRAIN[ny * GW + nx] === 1) {
          SHORE.add(i);
          break;
        }
      }
    }
  }
}

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

// Building data is a flat array of 13 per building (M5):
// [id, kind, x, y, progress, complete, cell_x, cell_y, size,
//  queue_len, train_progress, owner, hp]. `owner` is the entity
// owner tag (0 ours, 2 the opponent's); `hp` is the remaining-HP
// fraction 0..1 (buildings take damage since M5).
function buildingList() {
  const flat = sim.buildings();
  const out = [];
  for (let i = 0; i < flat.length; i += 13) {
    out.push({
      id: flat[i], kind: flat[i + 1], x: flat[i + 2], y: flat[i + 3],
      progress: flat[i + 4], complete: flat[i + 5] > 0.5,
      cellX: flat[i + 6], cellY: flat[i + 7], size: flat[i + 8],
      queue: flat[i + 9], trainFrac: flat[i + 10],
      owner: flat[i + 11], hpFrac: flat[i + 12],
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
  st.owners[i] === localOwner && st.kinds[i] !== KIND_TC && st.hp[i * 2] >= 1;

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

// --- Placement ghost: client mirror of the sim's build validation ----
// Mirrors Sim::validate_build (lib.rs) using only ABI-visible state, so
// the ghost is honest. The sim re-validates every order itself — this
// mirror is a courtesy, never the authority.
function mirrorValid(kind, cx, cy) {
  const size = BUILD_SIZE[kind];
  if (cx < 0 || cy < 0 || cx + size > GW || cy + size > GH) return false;
  const buildings = buildingList();
  // Resuming an incomplete building of the same kind at this anchor
  // (M5: only our OWN — the sim refuses to resume the enemy's).
  for (const b of buildings) {
    if (!b.complete && b.owner === localOwner && b.kind === kind && b.cellX === cx && b.cellY === cy) {
      return true;
    }
  }
  // M6 gates (mirrors of the sim's validate_build): Farms need a
  // completed Mill of ours; the Feudal buildings need the Feudal
  // Age; the Range and the Stable need a completed Barracks.
  const ownDone = buildings.filter((b) => b.owner === localOwner && b.complete);
  if (kind === 1 && !ownDone.some((b) => b.kind === 4)) return false;
  if (kind >= 6 && myAge() < 1) return false;
  if ((kind === 6 || kind === 7) && !ownDone.some((b) => b.kind === 2)) return false;
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
  // Every Town Center's cell is off-limits (M5: both players').
  const tcCells = [];
  {
    const kinds = sim.entity_kinds();
    const pos = sim.positions();
    for (let i = 0; i < kinds.length; i++) {
      if (kinds[i] === KIND_TC) tcCells.push(worldToCell(pos[i * 2], pos[i * 2 + 1]));
    }
  }
  const nodes = nodeList();
  for (const [x, y] of cells) {
    if (OBST[y * GW + x]) return false; // static wall
    for (const [tx, ty] of tcCells) {
      if (x === tx && y === ty) return false; // a Town Center
    }
    for (const n of nodes) { // a live node on this cell
      if (n.amount <= 0) continue;
      const nc = worldToCell(n.x, n.y);
      if (nc[0] === x && nc[1] === y) return false;
    }
  }
  if (myStock()[1] < BUILD_COST[kind]) return false;
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
    if (helpOverlay.style.display !== 'none') {
      helpOverlay.style.display = 'none';
    } else if (placement) {
      placement = null;
    } else if (buildPage) {
      buildPage = null; // M13: Esc backs out of an armed build page
    } else if (playing) {
      setPaused(!paused);
    }
    return;
  }
  // M4 game hotkeys (control groups, idle keys, go-to TC): consumed
  // keys never reach the pan-key set below.
  if (playing && sim && handleGameKey(e)) return;
  keys.add(e.code);
});
addEventListener('keyup', (e) => keys.delete(e.code));

// --- M4 hotkeys ---------------------------------------------------------
// Control groups (SEL-07), idle keys (SEL-08/09), go-to Town Center
// (SEL-10). Returns true when the key was consumed. Everything here
// is client-side selection/camera work on top of the sim's queries —
// the only sim writes are the orders the player then issues.

/// Center the camera on a world point (keeps the current zoom).
function centerOn(wx, wy) {
  const [ox, oy] = centerCamera(wx, wy, camS, canvas.width, canvas.height);
  camOx = ox;
  camOy = oy;
}

/// Group n's members that are still ours and alive (the sim is the
/// truth; group membership is just remembered ids).
function liveGroupMembers(n) {
  const st = entityState();
  return controlGroups.recall(n, (id) => id < st.count && isOwnUnit(st, id));
}

/// The idle key (SEL-08/09): select + center the NEXT idle unit of
/// the kind ('villager' | 'military'), cycling; with `selectAll`,
/// select every idle one instead (the Shift variant). Shared by the
/// '.' / ',' keys and the top-bar idle indicators.
function cycleIdle(kind, selectAll) {
  const ids = kind === 'villager' ? [...myIdleVillagers()] : [...myIdleMilitary()];
  if (!ids.length) {
    flash(kind === 'villager' ? 'No idle villagers.' : 'No idle military units.');
    return;
  }
  if (selectAll) {
    selectUnits(new Set(ids));
    return;
  }
  const id = nextInCycle(ids, kind === 'villager' ? idleCycleVil : idleCycleMil);
  if (kind === 'villager') idleCycleVil = id;
  else idleCycleMil = id;
  selectUnits(new Set([id]));
  const pos = sim.positions();
  centerOn(pos[id * 2], pos[id * 2 + 1]);
}

function handleGameKey(e) {
  // Control groups: Ctrl+1–9 assigns the selection, 1–9 recalls it,
  // a double-tap centers the camera on it, Shift+number adds the
  // group to the current selection (SEL-07).
  const digit = /^Digit([1-9])$/.exec(e.code);
  if (digit) {
    const n = Number(digit[1]);
    if (e.altKey) return false; // the Alt layer (groups 11–20) is not implemented
    e.preventDefault();
    if (e.repeat) return true;
    if (e.ctrlKey || e.metaKey) {
      const sel = selectedUnits();
      if (sel.length) controlGroups.assign(n, sel);
      return true;
    }
    if (e.shiftKey) {
      const next = new Set(selection);
      for (const id of liveGroupMembers(n)) next.add(id);
      selectUnits(next);
      return true;
    }
    const members = liveGroupMembers(n);
    if (!members.length) return true;
    selectUnits(new Set(members));
    if (groupTaps.tap(n, performance.now()) === 'center') {
      const pos = sim.positions();
      let sx = 0;
      let sy = 0;
      for (const id of members) {
        sx += pos[id * 2];
        sy += pos[id * 2 + 1];
      }
      centerOn(sx / members.length, sy / members.length);
    }
    return true;
  }
  switch (e.code) {
    case 'Period': // '.' — next idle villager (SEL-08)
      e.preventDefault();
      if (!e.repeat) cycleIdle('villager', e.shiftKey);
      return true;
    case 'Comma': // ',' — next idle military unit (SEL-09)
      e.preventDefault();
      if (!e.repeat) cycleIdle('military', e.shiftKey);
      return true;
    case 'KeyH': { // 'H' — go to the Town Center (SEL-10)
      if (e.ctrlKey || e.metaKey || e.altKey) return false;
      e.preventDefault();
      if (e.repeat) return true;
      const tc = ownTcIndex();
      if (tc >= 0) {
        selectBuilding({ type: 'tc', id: tc, owner: localOwner });
        const pos = sim.positions();
        centerOn(pos[tc * 2], pos[tc * 2 + 1]);
      }
      return true;
    }
    default:
      return handleHotkey(e); // M13: the classic grid (below)
  }
}

// --- M13 hotkeys ----------------------------------------------------------
// The classic grid (play/hotkeys.js holds the tables and the pure
// resolution; this layer owns the gates and the doing). A resolved
// key runs the SAME code a panel click runs (runCommandButton) or
// the same order path a right-click runs — never a parallel one.

/// The renderCommands() context for the current selection,
/// computed the same way the panel computes it.
function panelContext() {
  const sel = selectedUnits();
  if (sel.length) {
    const st = entityState();
    return sel.some((id) => st.kinds[id] === 0) ? 'build' : 'units';
  }
  if (!selectedBuilding || selectedBuilding.owner === foeOwner()) {
    return selectedBuilding ? 'foe' : 'none';
  }
  if (selectedBuilding.type === 'tc') return 'tc';
  const bk = buildingList().find((b) => b.id === selectedBuilding.id);
  return (bk && { 2: 'barracks', 6: 'range', 7: 'stable', 8: 'blacksmith' }[bk.kind]) || 'none';
}

/// Force drop-off for the selection (DE's villager A; the M6
/// ForceDeposit order, exactly as the right-click drop-off path
/// issues it — the carry heuristic decides who is loaded, the
/// sim re-validates each order itself).
function forceDropOffSelected() {
  const st = entityState();
  let any = false;
  for (const id of selectedUnits()) {
    if (st.kinds[id] === 0 && carryKind.has(id) && issue({ t: 'force_dep', e: id })) any = true;
  }
  if (!any) flash('No loaded villagers selected — nothing to drop off.');
}

/// Go-to (Ctrl+letter): cycle our completed buildings of a kind,
/// selecting and centering each in turn; with `selectAll`, take
/// the whole type at once (no centering unless there is one).
function goToBuildingKind(kind, selectAll) {
  const list = buildingList().filter(
    (b) => b.owner === localOwner && b.kind === kind && b.complete);
  if (!list.length) {
    flash(`No ${BUILD_NAME[kind]} yet.`);
    return;
  }
  if (selectAll && list.length > 1) {
    selectBuilding({ type: 'building', id: list[0].id, kind, owner: localOwner });
    buildingGroup = list.map((b) => b.id);
    return;
  }
  const id = nextInCycle(list.map((b) => b.id), gotoCycleMem[kind] ?? -1);
  gotoCycleMem[kind] = id;
  const b = list.find((x) => x.id === id);
  selectBuilding({ type: 'building', id: b.id, kind: b.kind, owner: b.owner });
  centerOn(b.x, b.y);
}

function handleHotkey(e) {
  if (e.altKey) return false; // the Alt layer is not ours
  // Never while typing: the lobby search, the room password, any
  // text field owns its letters outright.
  const t = e.target;
  if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA'
    || t.tagName === 'SELECT' || t.isContentEditable)) return false;
  // Never over a modal state: the pause menu, the guide, a
  // finished match, a replay. (The M4 keys above keep their
  // older, wider gate — selection keys are harmless there.)
  if (paused || matchEnded || playback) return false;
  if (helpOverlay.style.display !== 'none') return false;
  const act = resolveHotkey({
    context: panelContext(), page: buildPage, code: e.code,
    ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey,
  });
  if (!act) return false;
  e.preventDefault();
  if (e.repeat) return true; // a held key is one press, as in DE
  switch (act.type) {
    case 'arm':
      buildPage = act.page;
      renderCommands();
      return true;
    case 'button': {
      const btn = [...cmdGrid.children].find((b) => b.id === act.id);
      // A gated building's button stands disabled in the panel;
      // the key is still consumed (it must not leak into camera
      // panning) but does nothing, like the button itself.
      if (btn && !btn.disabled) {
        runCommandButton(act.id);
        renderCommands();
      }
      return true;
    }
    case 'drop-off':
      forceDropOffSelected();
      return true;
    case 'goto':
      goToBuildingKind(act.kind, act.all);
      return true;
    default:
      return false;
  }
}

function applyKeys(now) {
  const P = 14; // screen px per frame while a pan key is held
  if (keys.has('KeyA') || keys.has('ArrowLeft')) camOx += P;
  if (keys.has('KeyD') || keys.has('ArrowRight')) camOx -= P;
  if (keys.has('KeyW') || keys.has('ArrowUp')) camOy += P;
  if (keys.has('KeyS') || keys.has('ArrowDown')) camOy -= P;
  applyEdgeScroll(now);
}

// --- M13: edge scrolling --------------------------------------------------
// The reference game's camera: rest the pointer against a screen
// edge and the view pans that way, corners diagonally. It shares
// the pan keys' speed (14 canvas px per 60 Hz frame — screen-
// relative, so the world speed follows the zoom on its own) and
// their place in the frame: every branch that pans by keys pans
// by edges. The gates are the pointer's own: it must ride over
// the game canvas (never the top/bottom UI bars, whose elements
// sit above the canvas and take the pointer events instead),
// inside the window, in a live match view with no modal up.
const EDGE_MARGIN_PX = 6; // CSS px from the viewport edge
const EDGE_PX_PER_FRAME = 14; // the pan keys' P, at 60 Hz
let lastEdgeAt = null;

function applyEdgeScroll(now) {
  if (lastEdgeAt == null) lastEdgeAt = now;
  const dt = Math.min(Math.max(now - lastEdgeAt, 0), 100);
  lastEdgeAt = now;
  if (!mouseClientPt || !mouseOverCanvas) return;
  if (paused || matchEnded || playback) return;
  if (!document.body || !document.body.classList.contains('in-match')) return;
  if (helpOverlay.style.display !== 'none' || pauseMenu.style.display !== 'none') return;
  const r = canvas.getBoundingClientRect();
  if (!r.width || !r.height) return;
  const v = edgePan(
    mouseClientPt.x - r.left, mouseClientPt.y - r.top,
    r.width, r.height, EDGE_MARGIN_PX);
  if (!v.x && !v.y) return;
  const step = EDGE_PX_PER_FRAME * (dt / (1000 / 60)) * (canvas.width / r.width);
  camOx += v.x * step;
  camOy += v.y * step;
}

function setPaused(v) {
  if (!playing) return;
  paused = v;
  pauseMenu.style.display = paused ? 'flex' : 'none';
  // M8: connection details live in the pause menu (the top bar
  // stays clean) — refresh them whenever the menu opens online.
  if (v && net) {
    updateNetStatus();
    refreshNetPing();
  }
}

// --- M8: per-seat reads ---------------------------------------------------
// In local play localOwner is 0 and every helper below takes the
// original seat-0 ABI call — the pre-M8 behavior, unchanged. In an
// online match (or a replay viewed from the second seat) they take
// the per-seat forms, so the guest's HUD, panels and fog are its own.
const seatAware = () => !!net || localOwner !== 0;
const myStock = () => (seatAware() ? sim.stockpile_for(localOwner) : sim.stockpile());
const myAge = () => (seatAware() ? sim.age_for(localOwner) : sim.age());
const myTechMask = () => (seatAware() ? sim.tech_mask_for(localOwner) : sim.tech_mask());
const myPop = () => (seatAware() ? sim.population_for(localOwner) : sim.population());
const myPopCap = () => (seatAware() ? sim.population_cap_for(localOwner) : sim.population_cap());
const myTrainBlock = () => (seatAware() ? sim.train_block_reason_at(myTcId) : sim.train_block_reason());
const myResearchBlockTc = (t) => (seatAware() ? sim.research_block_tc_at(myTcId, t) : sim.research_block_tc(t));
const myTcQueue = () => (seatAware() ? sim.tc_queue_for(myTcId) : sim.tc_queue());
const myTcProgress = () => (seatAware() ? sim.tc_progress_for(myTcId) : sim.tc_progress());
const myTrainQueueLen = () => myTcQueue().length;
const myIdleVillagers = () => (seatAware() ? sim.idle_villagers_for(localOwner) : sim.idle_villagers());
const myIdleMilitary = () => (seatAware() ? sim.idle_military_for(localOwner) : sim.idle_military());
const myFog = () => (seatAware() ? sim.fog_for(localOwner) : sim.fog());

/// The entity index of the LOCAL player's Town Center.
function ownTcIndex() {
  const kinds = sim.entity_kinds();
  const owners = sim.entity_owners();
  for (let i = 0; i < kinds.length; i++) {
    if (kinds[i] === KIND_TC && owners[i] === localOwner) return i;
  }
  return -1;
}

// --- M8: the order choke point --------------------------------------------
// Every order the local player gives funnels through issue(). Local
// play applies it to the sim immediately, exactly as the pre-M8 call
// sites did (applyLocalOrder is those very calls), and records it
// for the replay. Netplay stamps it into the lockstep session
// instead — the sim receives it later, at its execution turn, on
// both clients identically (netplay.js's applyCommand is the mirror
// of applyLocalOrder with the per-seat Town Center forms).

function applyLocalOrder(cmd) {
  switch (cmd.t) {
    case 'move': return cmd.q
      ? sim.push_move_queued(cmd.e, cmd.x, cmd.y)
      : sim.push_move(cmd.e, cmd.x, cmd.y);
    case 'gather': return cmd.q
      ? sim.push_gather_queued(cmd.e, cmd.n)
      : sim.push_gather(cmd.e, cmd.n);
    case 'attack': return cmd.q
      ? sim.push_attack_queued(cmd.e, cmd.tg)
      : sim.push_attack(cmd.e, cmd.tg);
    case 'attackb': return sim.push_attack_building(cmd.e, cmd.b);
    case 'build': return cmd.q
      ? sim[PUSH_BUILD_QUEUED[cmd.k]](cmd.e, cmd.x, cmd.y)
      : sim[PUSH_BUILD[cmd.k]](cmd.e, cmd.x, cmd.y);
    case 'train_at':
      return cmd.k === 2 ? sim.push_train_scout(cmd.b)
        : cmd.k === 3 ? sim.push_train_militia(cmd.b)
        : sim.push_train_archer(cmd.b);
    case 'train_vil': return sim.push_train_villager();
    case 'research_tc': return cmd.tech === 0
      ? sim.push_research_loom()
      : sim.push_research_feudal();
    case 'research_at': return sim.push_research_at(cmd.b, cmd.tech);
    case 'rally_tc': return sim.push_set_rally_tc(cmd.x, cmd.y);
    case 'rally_b': return sim.push_set_rally_building(cmd.b, cmd.x, cmd.y);
    case 'force_dep': return sim.push_force_deposit(cmd.e);
    default: return false;
  }
}

function issue(cmd) {
  if (playback) return false;
  if (net) {
    if (!net.session || net.phase !== 'playing') return false;
    return net.session.localCommand(cmd) >= 0;
  }
  const ok = applyLocalOrder(cmd);
  if (ok && playing && localRecorder) {
    localRecorder.addLocal(sim.tick_count(), cmd);
  }
  return ok;
}
// M5: the mode decides the construction — Sandbox is the plain
// constructor (no opponent at all); the AI modes go through
// WasmSim.new_match and spawn the mirrored opponent + its script.
function resetSim(mode = matchMode) {
  matchMode = mode;
  builtMode = mode;
  matchEnded = false;
  endOverlay.style.display = 'none';
  localOwner = 0; // local modes always play the first seat (M8)
  playback = null;
  if (typeof netUi !== 'undefined' && netUi && netUi.playbackBar) {
    netUi.playbackBar.hidden = true;
  }
  sim = mode === 'sandbox'
    ? new WasmSim(seed)
    : WasmSim.new_match(seed, MODE_TAG[mode]);
  // M8: every local match records its command stream — the end
  // overlay (and the pause menu) can download it as a replay.
  localRecorder = createRecorder({
    seed, modeTag: MODE_TAG[mode], seats: [0], view: 0,
  });
  seenBuildings.clear(); // M7: last-known enemy buildings are per-match
  refreshWorldLayers();
  vilAnim.clear();
  prevPos.clear();
  lastNodeKind.clear();
  carryKind.clear();
  atNodeNow.clear();
  selectUnits(new Set()); // also clears any building selection
  placement = null;
  // M4 client state resets with the match: control groups and the
  // idle-key cycling memory belong to the old game.
  controlGroups.clear();
  groupTaps.reset();
  idleCycleVil = -1;
  idleCycleMil = -1;

  // Camera: centered on the Town Center.
  const kinds = sim.entity_kinds();
  const pos = sim.positions();
  const tc = kinds.indexOf(KIND_TC);
  myTcId = tc;
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

// --- M8: netplay controller ----------------------------------------------
// The lobby, the room handshake, rejoin, telemetry, and replay
// playback. Protocol + session live in play/netplay.js; the relay
// is a dumb pipe, so presence (hello messages), roles (how you
// entered), and the handshake all happen here, client-side.

let netUi = null; // DOM refs, filled by wireNetUi() in main()
let regionList = NET_REGIONS; // replaced by the relay's /regions when fetched

// --- M9: open-games browser state -----------------------------------
// The directory (relay GET /lobbies) is polled while the Lobby
// Browser is open; the shaping/sorting lives in play/lobby.js.
// M11: the controls row's state — search text, the Password toggle
// and the Server filter — decided purely by lobby.js.
let lobbyPings = {}; // region id -> measured ms (shares loadRegionsAndPings' probes)
let lobbyGames = { open: [], inProgress: 0 };
let lobbyTimer = null;
let lobbySelectedRoom = null;
let lobbyFilters = { search: '', showPassword: true, region: '' };

const regionName = (id) => (regionList.find((r) => r.id === id) || {}).name || id;

function lobbyStatus(text) {
  if (netUi) netUi.lobbyStatus.textContent = text;
}

function showOverlay(el, show) {
  if (el) el.style.display = show ? 'flex' : 'none';
}

/// THE pre-match state (M11 addendum, Claudio's rule): before a
/// match is entered there is no game on screen at all — play.css
/// hides the canvas and every HUD element unless body carries
/// .in-match, and the lobby sits on the plain page background.
/// This one toggle is flipped at the exact entry points where a
/// match starts (solo start, online 'go', rejoin resume, replay
/// playback) and at every return (openLobby, leaving a match,
/// back-to-menu) — nowhere else.
function setInMatch(on) {
  if (document.body) document.body.classList.toggle('in-match', !!on);
}

/// Adopt a freshly built sim as THE match (start of an online game,
/// or the rejoiner's rebuilt one): the same per-match client resets
/// resetSim performs, for an explicit seat.
function adoptSim(newSim, owner) {
  sim = newSim;
  localOwner = owner;
  builtMode = 'online';
  matchEnded = false;
  endOverlay.style.display = 'none';
  seenBuildings.clear();
  refreshWorldLayers();
  vilAnim.clear();
  prevPos.clear();
  lastNodeKind.clear();
  carryKind.clear();
  atNodeNow.clear();
  selectUnits(new Set());
  placement = null;
  controlGroups.clear();
  groupTaps.reset();
  idleCycleVil = -1;
  idleCycleMil = -1;
  localRecorder = null;
  myTcId = ownTcIndex();
  const pos = sim.positions();
  camS = INITIAL_S;
  const [ox, oy] = centerCamera(
    pos[myTcId * 2], pos[myTcId * 2 + 1], camS, canvas.width, canvas.height);
  camOx = ox;
  camOy = oy;
}

function makeNetSession() {
  return createSession({
    localSlot: net.role === 'host' ? 0 : 1,
    send: (m) => net.client.send(m),
    applyBundle: (slot, cmds) => {
      const o = slot === 0 ? 0 : 2;
      for (const c of cmds) netApplyCommand(sim, c, o, (oo) => netTcIdOf(sim, oo));
    },
    tickOnce: () => {
      sim.tick();
      return sim.state_hash();
    },
    onEvent: (e) => {
      if (!net) return;
      if (e.type === 'desync') onDesync(e);
      if (e.type === 'waiting') onNetWaiting(e.waiting);
    },
    onTurn: (turn, c0, c1) => {
      if (!net) return;
      net.recorder.addTurn(turn, c0, c1);
      const ms = sim.match_state();
      if (ms !== 0 && net.phase === 'playing') {
        // match_state() speaks for seat 0; invert for the guest.
        netMatchEnd(localOwner === 0 ? ms : (ms === 1 ? 2 : 1), {});
      }
    },
  });
}

let netWaitTimer = null;
function onNetWaiting(waiting) {
  // Show the stall pill only for stalls that outlast a blink.
  if (!netUi) return;
  if (waiting) {
    if (!netWaitTimer) {
      netWaitTimer = setTimeout(() => {
        if (netUi) netUi.netWait.hidden = false;
      }, 500);
    }
  } else {
    clearTimeout(netWaitTimer);
    netWaitTimer = null;
    netUi.netWait.hidden = true;
  }
}

function postTelemetry(result) {
  if (!net || net.telemetryPosted) return;
  net.telemetryPosted = true;
  try {
    fetch(RELAY_HTTP + '/telemetry', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        region: net.region,
        durationTicks: sim ? sim.tick_count() : 0,
        result,
        desyncCount: net.desyncs || 0,
        buildVersion: BUILD_VERSION,
      }),
    }).catch(() => {});
  } catch { /* telemetry never breaks the game */ }
}

function netMatchEnd(localMs, opts) {
  if (!net || net.phase === 'ended') return;
  net.phase = 'ended';
  if (net.session) net.session.stop();
  if (netUi) netUi.netWait.hidden = true;
  showOverlay(netUi && netUi.peerOverlay, false);
  const result = opts.desync ? 'desync'
    : opts.selfResign ? 'resigned'
    : opts.resignedByPeer ? 'victory'
    : localMs === 1 ? 'victory' : 'defeat';
  net.finalReplay = net.recorder
    ? net.recorder.finish({ result, ticks: sim.tick_count() })
    : null;
  postTelemetry(result);
  updateNetStatus();
  if (opts.desync) {
    if (netUi) {
      netUi.desyncDetail.textContent =
        `The two clients' simulations diverged at turn ${opts.desyncTurn}. ` +
        'The match cannot continue — download the replay to help diagnose it.';
    }
    showOverlay(netUi && netUi.desyncOverlay, true);
    return;
  }
  showMatchEnd(localMs);
}

function onDesync(e) {
  if (!net) return;
  net.desyncs = (net.desyncs || 0) + 1;
  netMatchEnd(2, { desync: true, desyncTurn: e.turn });
}

/// Tear the whole netplay state down (the caller resets the local
/// game afterwards when appropriate).
function teardownNet() {
  stopLobbyBrowser();
  if (net && net.loadTimer) clearTimeout(net.loadTimer);
  if (net && net.client) net.client.close();
  net = null;
  if (netUi) {
    showOverlay(netUi.onlineOverlay, false);
    showOverlay(netUi.loadingOverlay, false);
    showOverlay(netUi.desyncOverlay, false);
    showOverlay(netUi.peerOverlay, false);
    netUi.netWait.hidden = true;
    netUi.netStatus.hidden = true;
    if (netUi.resignBtn) netUi.resignBtn.hidden = true;
  }
}

/// Leave an online match back to local play (pause-menu Restart,
/// Leave match, desync close): telemetry when a match was live,
/// then a fresh local game with the pause menu over it.
function leaveNetMatch(result) {
  if (net && (net.phase === 'playing' || net.phase === 'peer-left')) {
    postTelemetry(result || 'abandoned');
  }
  teardownNet();
  setInMatch(false); // back to the menu side: chrome hides again
  resetSim(matchMode);
  playing = true;
  setPaused(true);
}

function updateNetStatus() {
  if (!netUi) return;
  if (!net || !net.room) {
    netUi.netStatus.hidden = true;
    if (netUi && netUi.resignBtn) netUi.resignBtn.hidden = true;
    return;
  }
  const stateTxt = {
    lobby: 'in the lobby',
    starting: 'loading…',
    playing: 'match in progress',
    'peer-left': 'opponent disconnected',
    rebuilding: 'rebuilding the match…',
    ended: 'match over',
    desync: 'desynchronized',
  }[net.phase] || net.phase;
  netUi.netStatus.hidden = false;
  netUi.netStatusText.textContent =
    `Online · ${regionName(net.region)} · you are the ${net.role === 'host' ? 'host (blue)' : 'guest (red)'} · ${stateTxt}` +
    (net.pingMs != null ? ` · ping ${net.pingMs} ms` : '');
  if (netUi.resignBtn) {
    netUi.resignBtn.hidden = !(net.phase === 'playing' || net.phase === 'peer-left');
  }
}

function refreshNetPing() {
  if (!net || !net.region) return;
  probeRegionPing({ relayHttp: RELAY_HTTP, region: net.region }).then((r) => {
    if (r && net) {
      net.pingMs = r.ms;
      updateNetStatus();
    }
  }).catch(() => {});
}

// --- M10: lazy game load + the loading phase -----------------------
//
// The heavy game — the WASM binary, the sprite tiers, the world
// constants — loads ONLY here, once a match is actually starting
// (both players ready online, or Play solo). Until then /play is
// a light lobby: no .wasm request, no artwork fetch. The load
// reports its step to the loading screen, so what the player
// watches is the real load, not a spinner over nothing.

let gameLoadPromise = null;

function ensureGameLoaded(onStep) {
  if (!gameLoadPromise) {
    gameLoadPromise = (async () => {
      if (onStep) onStep('Downloading the game…');
      const mod = await import('./pkg/skirmish_sim.js');
      await mod.default(); // fetch + instantiate the .wasm
      WasmSim = mod.WasmSim;
      if (onStep) onStep('Loading the artwork…');
      loadPlaceholderSprites();
      loadBuildingArt();
      loadNodeArt();
      loadAnimalArt();
      loadTierSprites(Q);
      // The local "loaded" readiness includes the building-art
      // set: this promise must not resolve while a building PNG
      // is still in flight, or the match's first frames draw the
      // CC0 placeholder and pop to the real art when it lands.
      // Settled means loaded OR errored (a failed image keeps the
      // fallback chain), capped at BUILDING_ART_SETTLE_TIMEOUT_MS
      // so a hung image can never block a match. Every entry
      // funnels through here — solo start, the host's start, the
      // guest's start-ack (the M10 loaded signal), AI rooms,
      // rejoin and replay — so gating here gates them all. The
      // node and animal sets settle under the same rule and join
      // the same await.
      await Promise.all([whenBuildingArtSettled(), whenNodeArtSettled(),
        whenAnimalArtSettled()]);
      // The world constants are the same for every match (fixed
      // world size); read them off a throwaway probe sim so no
      // match code ever runs before they exist.
      const probe = new WasmSim(1);
      HALF = probe.world_half();
      GW = probe.grid_width();
      GH = probe.grid_height();
      CS = probe.cell_size();
      OBST = probe.obstacles();
      mmGeom = createMinimap({
        worldHalf: HALF, width: minimapCanvas.width, height: minimapCanvas.height,
      });
      if (onStep) onStep('Building the world…');
    })();
    // A failed load (offline, blocked fetch) may be retried by
    // the next attempt to play — don't cache the rejection.
    gameLoadPromise.catch(() => { gameLoadPromise = null; });
  }
  return gameLoadPromise;
}

/// Whose seat is loaded in the loading model: seats are fixed —
/// host is Blue (slot 0), guest is Red (slot 1).
function loadingModel() {
  if (!net) return null;
  const selfLoaded = !!net.selfLoaded;
  const peerLoaded = !!net.peerLoaded;
  return net.role === 'host'
    ? { blueLoaded: selfLoaded, redLoaded: peerLoaded }
    : { blueLoaded: peerLoaded, redLoaded: selfLoaded };
}

function renderLoading() {
  if (!netUi || !netUi.loadingOverlay) return;
  const m = loadingModel();
  if (!m) return;
  netUi.loadSlotGuest.style.display = '';
  const row = (label, loaded) =>
    `<span class="slot-name">${label}</span>` +
    `<span class="slot-state">${loaded ? 'loaded ✓' : 'loading…'}</span>`;
  netUi.loadSlotHost.innerHTML = row('Blue — Host', m.blueLoaded);
  // M12: with an AI guest slot the red seat names the AI (it is
  // this same client's sim — it shows loaded the moment the
  // host's build verifies).
  const guestLabel = slotIsAi(net.guestSlot)
    ? `Red — ${slotLabel(net.guestSlot)}` : 'Red — Guest';
  netUi.loadSlotGuest.innerHTML = row(guestLabel, m.redLoaded);
}

function setLoadingStatus(text) {
  if (netUi && netUi.loadingStatus) netUi.loadingStatus.textContent = text;
}

/// Enter the loading phase (online): the room screen gives way to
/// the loading screen, both loaded flags start down, and the
/// cancellation timer starts ticking.
function enterLoading() {
  net.phase = 'starting';
  net.selfLoaded = false;
  net.peerLoaded = false;
  showOverlay(netUi.onlineOverlay, false);
  showOverlay(netUi.loadingOverlay, true);
  renderLoading();
  setLoadingStatus('Loading…');
  clearTimeout(net.loadTimer);
  net.loadTimer = setTimeout(() => cancelLoading('timeout'), LOAD_TIMEOUT_MS);
  updateNetStatus();
}

function hideLoading() {
  if (netUi) showOverlay(netUi.loadingOverlay, false);
}

/// This client finished loading (sim built, construction hash
/// verified): raise my flag on the loading screen.
function markSelfLoaded() {
  if (!net) return;
  net.selfLoaded = true;
  renderLoading();
}

/// The one cancellation message (addendum rule): a player left or
/// failed to load — the match is cancelled for everyone.
const MATCH_CANCELLED_MSG = 'Match cancelled — a player left or failed to load.';

/// Cancel a match that is still loading and return to the Lobby
/// Browser. `notify` tells the peer (its own cancel — or its
/// disconnect — needs no echo). Never a desync overlay, never a
/// half-started sim: the pending sim is simply dropped.
function cancelLoading(notify) {
  if (!net || net.phase !== 'starting') return;
  clearTimeout(net.loadTimer);
  if (notify && net.client) {
    try { net.client.send({ type: 'cancel' }); } catch { /* socket already gone */ }
  }
  hideLoading();
  openLobby(null);
  lobbyStatus(MATCH_CANCELLED_MSG);
}

// --- room-link consume-once --------------------------------------------------
// A room link is consumed exactly once per tab session: the boot
// that follows it joins the room; a RELOAD of the same tab must
// land on the Lobby Browser (the front door) — the game loads
// only when a match is actually entered. sessionStorage is the
// mark store: it survives reloads in the same tab and dies with
// the tab, which is exactly the wanted semantics.
const ROOM_LINK_MARK_KEY = 'skirmish2.roomLinkConsumed';

function readRoomLinkMark() {
  try { return sessionStorage.getItem(ROOM_LINK_MARK_KEY); } catch { return null; }
}

function writeRoomLinkMark(room) {
  try { sessionStorage.setItem(ROOM_LINK_MARK_KEY, room); } catch { /* storage off — the link just stays re-joinable */ }
}

/// Drop the room params (room/region/private) from the address bar
/// without navigating. From the moment a room exists it lives in
/// memory (net.room); the invite link is built on demand by the
/// room screen's copy action, never read back from the URL — so
/// the address bar can never re-trigger a join on reload.
function cleanPlayUrl() {
  try {
    const p = new URLSearchParams(location.search);
    if (!p.has('room') && !p.has('region') && !p.has('private')) return;
    p.delete('room');
    p.delete('region');
    p.delete('private');
    const qs = p.toString();
    history.replaceState(null, '', location.pathname + (qs ? '?' + qs : ''));
  } catch { /* non-http(s) context — nothing to clean */ }
}

// --- the handshake ---------------------------------------------------------

async function hostStartMatch() {
  // Fired by the both-ready gate (M10), never by a button: the
  // host's client starts the handshake the moment both players
  // are ready. The heavy load happens first, on the loading
  // screen; only a loaded host sends 'start'.
  if (!net || net.role !== 'host' || !net.peerPresent) return;
  if (net.phase !== 'lobby') return;
  enterLoading();
  const mine = net;
  try {
    await ensureGameLoaded((t) => { if (net === mine) setLoadingStatus(t); });
  } catch {
    if (net === mine) cancelLoading(true);
    return;
  }
  if (net !== mine || net.phase !== 'starting') return; // cancelled mid-load
  net.seed = (Math.random() * 0x7fffffff) >>> 0;
  net.pendingSim = WasmSim.new_match(net.seed, MODE_TAG.online);
  net.client.send({
    type: 'start', seed: net.seed, hash0: net.pendingSim.state_hash(),
  });
  // The host's own build IS its verification — it is loaded.
  markSelfLoaded();
  setLoadingStatus('Waiting for your opponent to load…');
  updateNetStatus();
}

async function guestHandleStart(msg) {
  // The host started: enter the loading phase and do the real
  // load here (WASM + artwork + world), then build the sim and
  // verify its construction hash against the host's.
  enterLoading();
  const mine = net;
  try {
    await ensureGameLoaded((t) => { if (net === mine) setLoadingStatus(t); });
  } catch {
    if (net === mine) {
      net.client.send({ type: 'start-abort', reason: 'load-failed' });
      cancelLoading(false);
    }
    return;
  }
  if (net !== mine || net.phase !== 'starting') return; // cancelled mid-load
  const candidate = WasmSim.new_match(msg.seed, MODE_TAG.online);
  if (candidate.state_hash() !== msg.hash0) {
    // Same seed, different construction hash: the two builds
    // disagree — lockstep would desync on turn 0. That is a
    // failed load: refuse and cancel cleanly for everyone.
    net.client.send({ type: 'start-abort', reason: 'build-mismatch' });
    cancelLoading(false);
    return;
  }
  net.seed = msg.seed;
  net.pendingSim = candidate;
  markSelfLoaded();
  // start-ack IS the loaded signal (M10): the sim is built and
  // the construction hash verified. The host sends 'go' only
  // once both loaded signals are in.
  net.client.send({ type: 'start-ack' });
  setLoadingStatus('Loaded — waiting for the host…');
}

// --- M12: the AI guest slot --------------------------------------
//
// With the guest seat filled by an AI, no second client exists —
// so there is no handshake and no lockstep session. The room
// screen and the loading screen are genuinely the online ones
// (that is the point: the host runs the full flow alone), and
// the match itself is a LOCAL match against the in-sim scripted
// AI at the chosen difficulty, recorded like a local match.

async function hostStartAiMatch() {
  // Fired by the host-only ready gate when the slot is an AI.
  if (!net || net.role !== 'host' || net.phase !== 'lobby') return;
  const mode = slotAiMode(net.guestSlot);
  if (!mode) return;
  enterLoading();
  const mine = net;
  try {
    await ensureGameLoaded((t) => { if (net === mine) setLoadingStatus(t); });
  } catch {
    if (net === mine) cancelLoading(true);
    return;
  }
  if (net !== mine || net.phase !== 'starting') return; // cancelled mid-load
  net.seed = (Math.random() * 0x7fffffff) >>> 0;
  net.pendingSim = WasmSim.new_match(net.seed, MODE_TAG[mode]);
  // The host's own build is its verification — and the AI seat
  // counts as loaded the moment that build verifies: there is
  // no second load to wait for.
  markSelfLoaded();
  net.peerLoaded = true;
  renderLoading();
  setLoadingStatus('Starting the match…');
  beginAiMatch(mode);
}

/// The loading phase ends: the room dissolves (its socket
/// closes, the directory entry goes with it) and the verified
/// sim becomes a local match under the aiRoom marker.
function beginAiMatch(mode) {
  const seed = net.seed;
  const candidate = net.pendingSim;
  net.pendingSim = null;
  // The room's life is over: mark it ended BEFORE teardown
  // closes its socket, so the close event can never be read as
  // a peer dropping out of the loading phase.
  net.phase = 'ended';
  teardownNet(); // clears the load timer, closes the room socket
  aiRoom = { mode, seed };
  adoptAiMatch(candidate, mode, seed);
}

/// Put an AI-room match on screen: the same per-match resets an
/// online match gets (adoptSim), then the local corrections —
/// the frame loop must run the AI modes' Conquest check (not
/// the online path), and the replay records like a local match
/// (one seat, the AI difficulty's mode tag).
function adoptAiMatch(candidate, mode, seed) {
  adoptSim(candidate, 0);
  builtMode = mode;
  localRecorder = createRecorder({
    seed, modeTag: MODE_TAG[mode], seats: [0], view: 0,
  });
  setInMatch(true); // the match begins: the game appears
  setPaused(false);
  playing = true;
}

/// Pause-menu / end-overlay Restart for an AI-room match: the
/// same configuration again — same seed, same AI difficulty —
/// exactly what Restart means for a local match.
function restartAiMatch() {
  if (!aiRoom) return;
  const { mode, seed } = aiRoom;
  adoptAiMatch(WasmSim.new_match(seed, MODE_TAG[mode]), mode, seed);
}

function beginOnlineMatch() {
  // Both loaded signals are in and 'go' has flown: the loading
  // phase ends here, for both roles.
  clearTimeout(net.loadTimer);
  hideLoading();
  setInMatch(true); // 'go': the game appears
  const owner = net.role === 'host' ? 0 : 2;
  adoptSim(net.pendingSim, owner);
  net.pendingSim = null;
  net.recorder = createRecorder({
    seed: net.seed, modeTag: MODE_TAG.online, region: net.region,
    seats: [0, 2], view: owner,
  });
  net.session = makeNetSession();
  net.phase = 'playing';
  stopLobbyBrowser();
  showOverlay(netUi.onlineOverlay, false);
  setPaused(false);
  playing = true;
  updateNetStatus();
  refreshNetPing();
  return net.session;
}

// --- M10: the room screen --------------------------------------------
//
// Two seats, their ready states, one Start button per player
// (toggle ready). The pure gates live in play/room.js; this is
// the DOM + state half.

/// Admission from THIS client's seat (host: the peer's; guest:
/// my own) — the value the ready gate consumes.
function myAdmitted() {
  if (!net) return false;
  return net.role === 'host'
    ? peerAdmitted({ privacy: net.privacy, peerPwOk: net.peerPwOk })
    : guestAdmitted({ isPrivate: net.privacy === 'private', pwState: net.pwState });
}

function renderRoom() {
  if (!netUi || !netUi.roomSection) return;
  const inRoom = !!(net && net.room);
  netUi.roomSection.hidden = !inRoom;
  if (!inRoom) return;
  const isHost = net.role === 'host';
  const priv = net.privacy === 'private';
  const slot = net.guestSlot || 'open';
  const aiSlot = isHost && slotIsAi(slot);
  const hostReady = isHost ? net.selfReady : net.peerReady;
  // M12: the AI seat always counts as ready.
  const guestReady = aiSlot ? true : (isHost ? net.peerReady : net.selfReady);
  let hostState;
  let guestState;
  if (isHost) hostState = 'You';
  else hostState = (net.peerPresent || net.hostSeen) ? 'Here' : 'Waiting…';
  if (!isHost) {
    guestState = priv && net.pwState !== 'ok' ? 'You — password required' : 'You';
  } else if (aiSlot) {
    guestState = slotLabel(slot);
  } else if (slot === 'closed') {
    guestState = 'Closed — no guest will join';
  } else if (net.peerPwOk || net.peerPresent) {
    guestState = 'Here';
  } else if (net.doorGuest) {
    guestState = 'At the door — password';
  } else {
    guestState = 'Waiting for a guest…';
  }
  const badge = (r) => (r ? ' · <span class="slot-ready">Ready ✓</span>' : '');
  netUi.slotHost.innerHTML =
    '<span class="slot-dot dot-blue"></span>' +
    '<span class="slot-name">Host — Blue</span>' +
    `<span class="slot-state">${hostState}${badge(hostReady)}</span>`;
  netUi.slotGuest.innerHTML =
    '<span class="slot-dot dot-red"></span>' +
    '<span class="slot-name">Guest — Red</span>' +
    `<span class="slot-state">${guestState}${badge(guestReady)}</span>`;
  // M12: the configuration summary — the game's fixed facts
  // plus the guest seat, from the viewer's own side of it.
  if (netUi.roomConfigLine) {
    netUi.roomConfigLine.textContent = roomConfigText({
      guestSlot: slot, selfIsGuest: !isHost,
    });
  }
  // M12: the guest-seat selector is the host's alone, and it
  // locks while a human guest is present (or at the door): the
  // seat cannot be reconfigured out from under a person.
  if (netUi.slotControlRow) {
    netUi.slotControlRow.hidden = !isHost;
    if (netUi.roomGuestSlot) {
      netUi.roomGuestSlot.value = slot;
      netUi.roomGuestSlot.disabled = net.phase !== 'lobby'
        || !!net.peerPresent || !!net.doorGuest;
    }
  }
  // The listed lines describe an OPEN seat; a sealed seat is
  // explained by the configuration line instead.
  netUi.roomListedLine.hidden = priv || slotSealed(slot);
  netUi.roomPrivateLine.hidden = !priv || slotSealed(slot);
  // The password prompt belongs to a guest of a private room who
  // is not in yet.
  const showPw = !isHost && priv && net.pwState !== 'ok';
  netUi.pwRow.hidden = !showPw;
  const showPwNote = showPw && net.pwState === 'wrong';
  netUi.pwNote.hidden = !showPwNote;
  if (showPwNote) netUi.pwNote.textContent = 'Incorrect password — try again.';
  // The Start toggle: live only for an admitted player with an
  // admitted, present peer — or, with an AI seat, for the host
  // alone (the AI always counts as ready).
  const can = canToggleReady({
    phase: net.phase, peerPresent: net.peerPresent, admitted: myAdmitted(),
    guestSlot: slot,
  });
  netUi.readyBtn.disabled = !can;
  netUi.readyBtn.textContent = net.selfReady ? '✓ Ready — tap to cancel' : 'Start';
  netUi.readyBtn.classList.toggle('active', !!net.selfReady);
}

/// The both-ready rule (host side only — startGate enforces it):
/// when both flags are up, the host's client initiates the start
/// handshake by itself. Un-readying before that simply keeps the
/// gate closed. M12: with an AI guest slot the gate is the host's
/// ready alone, and firing it starts the AI match instead — no
/// handshake exists to send.
function maybeStartMatch() {
  if (!net) return;
  if (startGate({
    role: net.role,
    phase: net.phase,
    peerPresent: net.peerPresent,
    admitted: myAdmitted(),
    selfReady: net.selfReady,
    peerReady: net.peerReady,
    guestSlot: net.guestSlot,
  })) {
    if (slotIsAi(net.guestSlot)) hostStartAiMatch();
    else hostStartMatch();
  }
}

function toggleReady() {
  if (!net || net.phase !== 'lobby') return;
  if (!canToggleReady({
    phase: net.phase, peerPresent: net.peerPresent, admitted: myAdmitted(),
    guestSlot: net.guestSlot,
  })) return;
  net.selfReady = !net.selfReady;
  net.client.send({ type: 'ready', ready: net.selfReady });
  renderRoom();
  maybeStartMatch();
}

/// Guest: send the typed password to the host for checking. The
/// verdict comes back as pw-ok / pw-no; until pw-ok this client
/// is not present, cannot ready, and the host cannot start.
function submitPassword() {
  if (!net || !netUi.pwInput) return;
  const pw = netUi.pwInput.value;
  if (!pw) return;
  netUi.pwInput.value = '';
  net.pwState = 'sent';
  net.client.send({ type: 'pw', pw });
  lobbyStatus('Checking the password…');
  renderRoom();
}

// --- rejoin ------------------------------------------------------------------

async function rejoinFromResync(msg) {
  // The remaining peer transferred the full recorded stream: this
  // client rebuilds the sim from the seed and fast-forwards it,
  // chunked so the page stays alive, then both sides resume from
  // the frontier via startAt/rebase.
  // A rejoin can land on a client whose game never loaded in this
  // page (fresh tab on the same link): go through the same load
  // gate as every other entry — WasmSim and the building art
  // (settled, capped) — before rebuilding.
  try {
    await ensureGameLoaded();
  } catch {
    lobbyStatus('The game failed to load — check your connection and try rejoining again.');
    return;
  }
  net.phase = 'rebuilding';
  net.seed = msg.seed;
  const slot = typeof msg.yourSlot === 'number' ? msg.yourSlot : 1;
  net.role = slot === 0 ? 'host' : 'guest';
  const owner = slot === 0 ? 0 : 2;
  lobbyStatus('Rebuilding the match from the recorded game…');
  showOverlay(netUi.onlineOverlay, true);
  const fresh = WasmSim.new_match(msg.seed, MODE_TAG.online);
  adoptSim(fresh, owner);
  const byTick = new Map();
  for (const e of msg.events || []) byTick.set(e.tick, e);
  const total = msg.ticks || 0;
  for (let t = 0; t < total; t++) {
    const e = byTick.get(t);
    if (e) {
      for (const c of e.s0) netApplyCommand(sim, c, 0, (o) => netTcIdOf(sim, o));
      if (e.s1) for (const c of e.s1) netApplyCommand(sim, c, 2, (o) => netTcIdOf(sim, o));
    }
    sim.tick();
    if (t % 600 === 599) await new Promise((r) => setTimeout(r, 0));
  }
  net.recorder = createRecorder({
    seed: net.seed, modeTag: MODE_TAG.online, region: net.region,
    seats: [0, 2], view: owner,
  });
  net.recorder.seed(msg.events || []);
  net.session = makeNetSession();
  net.resyncDelay = msg.delay || 4;
  net.client.send({ type: 'resync-done' });
  lobbyStatus('Rebuilt — resuming…');
}

function sendResync() {
  if (!net || !net.session || !net.recorder) return;
  const snapshot = net.recorder.finish({ result: 'ongoing', ticks: sim.tick_count() });
  net.client.send({
    type: 'resync',
    seed: net.seed,
    ticks: sim.tick_count(),
    delay: net.session.state().delay,
    yourSlot: net.role === 'host' ? 1 : 0,
    events: snapshot.events,
  });
}

function onPeerGone() {
  if (!net) return;
  if (net.phase === 'playing') {
    net.phase = 'peer-left';
    net.peerLeftAt = Date.now();
    updateNetStatus();
    showOverlay(netUi.peerOverlay, true);
    if (netUi.claimBtn) netUi.claimBtn.disabled = true;
    clearInterval(net.graceTimer);
    net.graceTimer = setInterval(() => {
      if (!net || net.phase !== 'peer-left') {
        clearInterval(net.graceTimer);
        return;
      }
      const left = Math.max(0, Math.ceil(
        (REJOIN_GRACE_MS - (Date.now() - net.peerLeftAt)) / 1000));
      if (netUi.peerDetail) {
        netUi.peerDetail.textContent = left > 0
          ? `Your opponent's connection dropped. If they reopen the same link within ${left} s, the match resumes where it stopped.`
          : 'The rejoin window has closed. You may claim the victory, or leave the match.';
      }
      if (left <= 0 && netUi.claimBtn) netUi.claimBtn.disabled = false;
    }, 250);
  } else if (net.phase === 'starting') {
    // M10: a disconnect during LOADING cancels the match for
    // everyone — no dead screen, no half-started sim; the
    // remaining player lands back in the Lobby Browser with the
    // cancellation message.
    cancelLoading(false);
  } else if (net.phase === 'lobby') {
    net.peerPresent = false;
    net.peerGone = false;
    // M10: a guest who leaves takes both ready flags down — the
    // host must press Start again for the next guest.
    net.peerReady = false;
    net.selfReady = false;
    net.peerPwOk = false;
    net.doorGuest = false;
    lobbyStatus('Your opponent left the room.');
    renderRoom();
    updateNetStatus();
  }
}

function onNetMessage(msg) {
  if (!net) return;
  if (net.session && net.session.receive(msg)) return;
  switch (msg.type) {
    case 'hello': {
      // M12: a sealed guest seat (AI / Closed) admits no human —
      // a late hello on a stale link is told the room is full,
      // exactly like a third opener of a two-human room.
      if (net.role === 'host' && net.phase === 'lobby'
        && slotSealed(net.guestSlot)) {
        net.client.send({ type: 'room-full' });
        break;
      }
      if (!net.helloReplied) {
        net.helloReplied = true;
        net.client.send({ type: 'hello', v: 1, role: net.role });
      }
      // M10 private rooms: a guest at the door of a private room
      // does not count as present until the password check passes
      // (host side). The host asks for the password instead; the
      // door state shows in the guest slot, nothing more.
      if (net.role === 'host' && net.phase === 'lobby'
        && net.privacy === 'private' && !net.peerPwOk) {
        net.doorGuest = true;
        net.client.send({ type: 'pw-req' });
        lobbyStatus('A guest is at the door — waiting for the password.');
        renderRoom();
        updateNetStatus();
        break;
      }
      // Guest side of the same door: the host exists, but in a
      // private room this client is not IN yet — presence waits
      // for 'pw-ok'.
      const guestAtDoor = net.role === 'guest' && net.phase === 'lobby'
        && net.privacy === 'private' && net.pwState !== 'ok';
      const wasGone = net.peerGone;
      net.hellosSeen = (net.hellosSeen || 0) + 1;
      if (guestAtDoor) {
        net.hostSeen = true;
        lobbyStatus('This room is private — enter the password to join.');
        renderRoom();
        updateNetStatus();
        break;
      }
      const isNewPresence = !net.peerPresent;
      net.peerPresent = true;
      net.peerGone = false;
      // A fresh arrival starts unready on both sides of the gate.
      if (isNewPresence && net.phase === 'lobby') net.peerReady = false;
      if (net.phase === 'lobby' || net.phase === 'starting') {
        if (net.role === 'host') {
          lobbyStatus('Your opponent is here. Press Start when you are both ready.');
        } else {
          lobbyStatus('Connected. Press Start when you are ready — the match begins when both players are ready.');
        }
        renderRoom();
      } else if (net.phase === 'peer-left' || (net.phase === 'playing' && wasGone)) {
        // The opponent rejoined on the same link: transfer the
        // recorded match so they can rebuild it.
        showOverlay(netUi.peerOverlay, false);
        net.phase = 'playing';
        sendResync();
      } else if (net.phase === 'playing' && !wasGone && net.hellosSeen > 2) {
        // The real peer accounts for at most two hellos (its own
        // plus its reply to mine); a third hello is a third client
        // opening the link mid-match.
        net.client.send({ type: 'room-full' });
      }
      updateNetStatus();
      break;
    }
    case 'room-full':
      lobbyStatus('This room already has two players.');
      break;
    case 'start':
      if (net.role === 'guest' && net.phase === 'lobby') guestHandleStart(msg);
      break;
    case 'start-ack':
      // The guest's loaded signal (M10): its sim is built and its
      // construction hash verified. The host's own loaded flag
      // went up when it sent 'start' — with both in, 'go' flies.
      if (net.role === 'host' && net.phase === 'starting' && net.pendingSim) {
        net.peerLoaded = true;
        renderLoading();
        const t0 = performance.now() + 600;
        net.client.send({ type: 'go', inMs: 600 });
        beginOnlineMatch().start(t0);
      }
      break;
    case 'start-abort':
      // The guest failed to load (build mismatch or a failed
      // download): the match is cancelled for everyone.
      if (net.phase === 'starting') cancelLoading(false);
      break;
    case 'cancel':
      // The peer cancelled during loading (or, for a guest still
      // in the room, the host's load failed before 'start'): the
      // match is off; land back in the Lobby Browser with the
      // cancellation message. A cancel never echoes back.
      if (net.phase === 'starting') cancelLoading(false);
      else if (net.phase === 'lobby' && net.room) {
        openLobby(null);
        lobbyStatus(MATCH_CANCELLED_MSG);
      }
      break;
    case 'go':
      if (net.role === 'guest' && net.pendingSim) {
        net.peerLoaded = true; // 'go' means the host is loaded too
        renderLoading();
        beginOnlineMatch().start(performance.now() + (msg.inMs || 600));
      }
      break;
    case 'ready':
      // M10: the peer's ready flag. Presence is already gated on
      // admission (a private-room guest counts as present only
      // after pw-ok), so a ready from a present peer is a ready
      // that counts. The host's gate runs on every change.
      if (net.phase === 'lobby' && net.peerPresent) {
        net.peerReady = !!msg.ready;
        renderRoom();
        maybeStartMatch();
      }
      break;
    case 'pw-req':
      // The host's room is private (a stripped link param can't
      // hide it): ask for the password before presence counts.
      if (net.role === 'guest' && net.phase === 'lobby') {
        net.privacy = 'private';
        net.hostSeen = true;
        if (net.pwState === 'na') net.pwState = 'needed';
        lobbyStatus('This room is private — enter the password to join.');
        renderRoom();
      }
      break;
    case 'pw':
      // Host side of the courtesy lock: compare against the
      // password held in THIS client's memory (set at create
      // time; never sent anywhere, never stored server-side).
      if (net.role === 'host' && net.phase === 'lobby'
        && net.privacy === 'private') {
        if (checkPassword(net.password, msg.pw)) {
          net.peerPwOk = true;
          net.doorGuest = false;
          net.peerPresent = true;
          net.peerReady = false;
          net.client.send({ type: 'pw-ok' });
          lobbyStatus('Your opponent is here. Press Start when you are both ready.');
        } else {
          net.client.send({ type: 'pw-no' });
          lobbyStatus('A guest entered the wrong password.');
        }
        renderRoom();
        updateNetStatus();
      }
      break;
    case 'pw-ok':
      if (net.role === 'guest' && net.phase === 'lobby') {
        net.pwState = 'ok';
        net.peerPresent = true;
        net.peerReady = false;
        lobbyStatus('You are in. Press Start when you are ready — the match begins when both players are ready.');
        renderRoom();
        updateNetStatus();
      }
      break;
    case 'pw-no':
      if (net.role === 'guest' && net.phase === 'lobby') {
        net.pwState = 'wrong';
        lobbyStatus('Incorrect password — try again.');
        renderRoom();
      }
      break;
    case 'resign':
      if (net.phase === 'playing' || net.phase === 'peer-left') {
        showOverlay(netUi.peerOverlay, false);
        netMatchEnd(1, { resignedByPeer: true });
      }
      break;
    case 'resync':
      if (net.phase === 'lobby' || net.phase === 'peer-left') {
        showOverlay(netUi.peerOverlay, false);
        rejoinFromResync(msg);
      }
      break;
    case 'resync-done':
      if (net.session && (net.phase === 'playing')) {
        const st = net.session.state();
        const atTurn = st.turn + st.delay + 2;
        net.client.send({ type: 'resume', atTurn });
        net.session.rebase(atTurn, performance.now() + 700);
        flash('Opponent reconnected — match resumed.');
      }
      break;
    case 'resume':
      if (net.session && net.phase === 'rebuilding') {
        net.session.startAt(msg.atTurn, performance.now() + 700,
          net.resyncDelay || undefined);
        net.phase = 'playing';
        showOverlay(netUi.onlineOverlay, false);
        setInMatch(true); // the resumed match is on screen again
        setPaused(false);
        playing = true;
        updateNetStatus();
        flash('Reconnected — match resumed.');
      }
      break;
    default:
      break;
  }
}

/// M12: tell the room object whether this room belongs in the
/// open-games directory right now. The seat is the host's to
/// configure, so only the host's client sends it — a control
/// frame the room object honors from the host's connection only
/// and never forwards (see the relay's slot control). Sealed
/// (AI / Closed) delists; Open relists.
function sendSlotListed(listed) {
  if (!net || net.role !== 'host' || !net.client) return;
  try { net.client.send({ type: 'slot', listed: !!listed }); } catch { /* socket gone */ }
}

/// Host: set the guest seat (room screen selector). The selector
/// is disabled while a human guest is present — this is only
/// ever called from an enabled control — and the directory
/// follows the seat: sealed seats leave the open list, Open
/// returns to it (under the M10 public/private rules).
function setGuestSlot(value) {
  if (!net || net.role !== 'host') return;
  const slot = guestSlotOf(value);
  if (slot === net.guestSlot) { renderRoom(); return; }
  net.guestSlot = slot;
  sendSlotListed(!slotSealed(slot));
  renderRoom();
}

function connectToRoom() {
  // M10: the HOST's create connection carries the private flag —
  // the room registers in the directory as locked (listed, like
  // the reference game's passworded rooms; only the flag travels,
  // never the password).
  const privFlag = net.role === 'host' && net.privacy === 'private'
    ? '&private=1' : '';
  // M12: the host's connection also presents host=1 (only it may
  // steer the listing later, via the slot control) and, when the
  // seat was already filled in the create panel, listed=0 — the
  // room then never enters the open list at all.
  const hostFlag = net.role === 'host' ? '&host=1' : '';
  const listedFlag = net.role === 'host' && slotSealed(net.guestSlot)
    ? '&listed=0' : '';
  const wsUrl = RELAY_HTTP.replace(/^http/, 'ws')
    + `/room/${encodeURIComponent(net.room)}?region=${encodeURIComponent(net.region)}${privFlag}${hostFlag}${listedFlag}`;
  net.client = createRelayClient({
    onOpen: () => {
      net.client.send({ type: 'hello', v: 1, role: net.role });
      if (net.role === 'host') {
        // M12: a sealed seat changes what "created" means — the
        // room is not in the public list and needs no guest.
        if (slotIsAi(net.guestSlot)) {
          lobbyStatus(`Room created — the guest seat is ${slotLabel(net.guestSlot)}. Press Start when you are ready.`);
        } else if (net.guestSlot === 'closed') {
          lobbyStatus('Room created — the guest seat is closed. Reopen it to let a player join.');
        } else if (net.privacy === 'private') {
          lobbyStatus('Private room created. Share the invite link — guests also need the password.');
        } else {
          lobbyStatus('Room created — it is listed in the Lobby Browser for anyone to join.');
        }
      } else if (net.privacy === 'private') {
        lobbyStatus('This room is private — enter the password to join.');
      } else {
        lobbyStatus('Joining the room…');
      }
      renderRoom();
    },
    onMessage: (m) => onNetMessage(m),
    onClose: () => {
      if (net && net.client) {
        net.peerGone = true;
        onPeerGone();
      }
    },
  });
  net.client.connect(wsUrl);
}

/// Open the lobby overlay — since M9 this is the Lobby Browser:
/// the open-games table over the create view. With `join`
/// ({room, region}) the client skips the browser entirely, enters
/// as the guest and connects at once (the link flow).
function openLobby(join) {
  setInMatch(false); // the lobby is up: no game on screen
  aiRoom = null; // entering the lobby dissolves any AI-room match
  if (net) teardownNet();
  net = {
    role: join ? 'guest' : 'host',
    room: join ? join.room : null,
    region: join ? (join.region || 'wnam') : null,
    client: null, session: null, phase: 'lobby',
    peerPresent: false, peerGone: false, helloReplied: false,
    seed: null, pendingSim: null, recorder: null,
    desyncs: 0, telemetryPosted: false, pingMs: null,
    selectedRegion: null,
    guestSlot: 'open', // M12: the second seat — Open / AI / Closed
    // M10 room model. The password lives ONLY in the host
    // client's memory (net.password), set at create time; it is
    // never persisted and never sent anywhere — guests send
    // theirs to the host for comparison, over the room pipe.
    privacy: join && join.private ? 'private' : 'public',
    password: null,
    peerPwOk: false, doorGuest: false, hostSeen: false,
    selfReady: false, peerReady: false,
    pwState: join && join.private ? 'needed' : 'na',
    roomLinkValue: '',
    selfLoaded: false, peerLoaded: false, loadTimer: null,
  };
  showOverlay(netUi.onlineOverlay, true);
  hideLoading();
  if (netUi.createBtn) netUi.createBtn.disabled = !!join;
  // Browsing shows the games browser; the create view is a panel
  // the action bar's Create Lobby opens (M11). Joining enters the
  // room screen directly (and the waiting-view ad sidebar).
  if (netUi.gamesSection) netUi.gamesSection.hidden = !!join;
  if (netUi.createSection) netUi.createSection.hidden = true;
  lobbyStatus(join ? 'Joining the room…' : 'Pick a server, then create the game.');
  loadRegionsAndPings();
  if (join) {
    stopLobbyBrowser();
    connectToRoom();
  } else {
    startLobbyBrowser();
  }
  renderRoom();
  updateNetStatus();
}

async function loadRegionsAndPings() {
  const mine = net;
  try {
    const res = await fetch(RELAY_HTTP + '/regions');
    if (res.ok) {
      const data = await res.json();
      if (Array.isArray(data.regions) && data.regions.length) {
        regionList = data.regions.map((r) => ({ id: r.id, name: r.label || r.name || r.id }));
      }
    }
  } catch { /* keep the built-in mirror */ }
  if (!net || net !== mine) return;
  renderRegionList({});
  renderRegionFilter();
  renderLobbyGames();
  renderConnQuality();
  const pings = {};
  for (const r of regionList) {
    const res = await probeRegionPing({ relayHttp: RELAY_HTTP, region: r.id })
      .catch(() => null);
    if (!net || net !== mine) return;
    pings[r.id] = res ? res.ms : null;
    lobbyPings[r.id] = pings[r.id];
    renderRegionList(pings);
    renderLobbyGames();
    renderConnQuality();
  }
  // Pre-highlight the nearest server; the choice stays the player's.
  if (net && net === mine && !net.selectedRegion) {
    const fastest = pickFastestRegion(pings);
    if (fastest) selectRegion(fastest);
  }
}

function selectRegion(id) {
  if (!net) return;
  net.selectedRegion = id;
  if (net.role === 'host' && net.phase === 'lobby' && !net.room) {
    net.region = id;
  }
  if (netUi) {
    for (const row of netUi.regionList.querySelectorAll('.region-row')) {
      row.classList.toggle('selected', row.dataset.region === id);
    }
  }
}

function renderRegionList(pings) {
  if (!netUi) return;
  netUi.regionList.innerHTML = '';
  for (const r of regionList) {
    const row = document.createElement('button');
    row.type = 'button';
    row.className = 'region-row'
      + (net && net.selectedRegion === r.id ? ' selected' : '')
      + (net && net.region === r.id && net.room ? ' locked' : '');
    row.dataset.region = r.id;
    const ms = pings[r.id];
    row.innerHTML = `<span class="region-name">${r.name}</span>`
      + `<span class="region-ms">${typeof ms === 'number' ? ms + ' ms' : '…'}</span>`;
    row.addEventListener('click', () => {
      if (net && net.room) return; // region is fixed once the room exists
      selectRegion(r.id);
    });
    netUi.regionList.append(row);
  }
}

function createGame() {
  if (!net || net.room) return;
  // M10: the privacy choice (Public default / Private + password).
  const privEl = document.querySelector('input[name="room-privacy"]:checked');
  net.privacy = privEl && privEl.value === 'private' ? 'private' : 'public';
  if (net.privacy === 'private') {
    const pw = netUi.roomPassword ? netUi.roomPassword.value : '';
    if (!pw) {
      lobbyStatus('Set a password for the private room first.');
      if (netUi.roomPassword) netUi.roomPassword.focus();
      return;
    }
    net.password = pw; // host memory only — never persisted
    if (netUi.roomPassword) netUi.roomPassword.value = '';
  }
  // M12: the guest seat may be configured right here, in the
  // create panel (the same control the room screen carries). A
  // seat filled here rides the create connection as listed=0.
  net.guestSlot = guestSlotOf(netUi.createGuestSlot
    ? netUi.createGuestSlot.value : 'open');
  net.region = net.selectedRegion || net.region || regionList[0].id;
  net.room = makeRoomId();
  // The room lives in memory from here — the address bar stays
  // room-free (a reload must land on the Lobby Browser), and the
  // invite link is built on demand by the copy action.
  cleanPlayUrl();
  net.roomLinkValue = buildPlayUrl(
    location.origin + location.pathname, net.room, net.region,
    { private: net.privacy === 'private' });
  if (netUi.createBtn) netUi.createBtn.disabled = true;
  // The room screen takes over from the browser + create view.
  stopLobbyBrowser();
  if (netUi.gamesSection) netUi.gamesSection.hidden = true;
  if (netUi.createSection) netUi.createSection.hidden = true;
  renderRegionList({});
  renderRoom();
  connectToRoom();
}

// --- M9: the open-games browser ---------------------------------------

function startLobbyBrowser() {
  stopLobbyBrowser();
  refreshLobbyGames();
  lobbyTimer = setInterval(refreshLobbyGames, LOBBIES_REFRESH_MS);
}

function stopLobbyBrowser() {
  if (lobbyTimer) {
    clearInterval(lobbyTimer);
    lobbyTimer = null;
  }
}

async function refreshLobbyGames() {
  try {
    const res = await fetch(RELAY_HTTP + '/lobbies');
    if (!res.ok) return;
    const parsed = parseLobbies(await res.json());
    if (!parsed) return;
    lobbyGames = parsed;
    // A selection that filled up or vanished is dropped; one
    // that still exists survives the refresh untouched (M11).
    if (!selectionSurvives(lobbySelectedRoom, parsed.open)) {
      lobbySelectedRoom = null;
    }
    renderLobbyGames();
  } catch { /* directory unreachable — the last list stands */ }
}

// The colored square beside a ping: green under 120 ms, yellow
// under 220 ms, red above (pingQuality in play/lobby.js); hollow
// while the region is still unmeasured.
function qualitySquare(quality) {
  return `<span class="q-square q-${quality || 'none'}" aria-hidden="true"></span>`;
}

function renderLobbyGames() {
  if (!netUi || !netUi.gamesList) return;
  const models = lobbyGames.open.map((e) => lobbyRowModel(e, {
    regionName: regionName(e.region),
    pingMs: lobbyPings[e.region],
  }));
  // Lobby order (oldest game first), then the controls row's
  // filters decide what is visible (M11).
  const rows = filterLobbyRows(sortLobbyRows(models, 'created', 1), lobbyFilters);
  const list = netUi.gamesList;
  list.innerHTML = '';
  const head = document.createElement('div');
  head.className = 'game-row game-head';
  head.innerHTML = '<span class="g-title">Game Name</span>'
    + '<span class="g-players">Players</span>'
    + '<span class="g-map">Map</span>'
    + '<span class="g-loc">Location</span>'
    + '<span class="g-server">Server</span>';
  list.append(head);
  if (!rows.length) {
    const empty = document.createElement('p');
    empty.className = 'games-empty';
    empty.textContent = lobbyGames.open.length
      ? 'No games match — clear the search or filters.'
      : 'No open games right now — create one with Create Lobby.';
    list.append(empty);
  }
  for (const m of rows) {
    const row = document.createElement('div');
    row.className = 'game-row' + (m.room === lobbySelectedRoom ? ' selected' : '');
    row.dataset.room = m.room;
    row.innerHTML = `<span class="g-title">${m.locked ? '<span class="g-lock" title="Private — password required">🔒</span> ' : ''}${m.title}</span>`
      + `<span class="g-players">${m.playersText}</span>`
      + `<span class="g-map">${m.mapText}</span>`
      + `<span class="g-loc">${m.server}</span>`
      + `<span class="g-server">${qualitySquare(m.quality)}${m.pingText}</span>`;
    row.addEventListener('click', () => {
      lobbySelectedRoom = m.room;
      renderLobbyGames();
    });
    row.addEventListener('dblclick', () => joinListedGame(m.room));
    list.append(row);
  }
  if (netUi.joinLobbyBtn) {
    // Join needs a selection that is actually on screen: one the
    // filters hide cannot be joined from here.
    netUi.joinLobbyBtn.disabled = !selectionSurvives(lobbySelectedRoom, rows);
  }
  if (netUi.gamesNote) {
    const n = lobbyGames.inProgress;
    netUi.gamesNote.hidden = !(n > 0);
    netUi.gamesNote.textContent = n > 0
      ? `${n} match${n === 1 ? '' : 'es'} in progress`
      : '';
  }
}

/// The controls row's Server dropdown: Any + every region. The
/// options follow regionList (the relay's /regions replaces the
/// built-in mirror); the current filter value is preserved.
function renderRegionFilter() {
  const sel = netUi && netUi.gamesRegionFilter;
  if (!sel) return;
  sel.innerHTML = '';
  const any = document.createElement('option');
  any.value = '';
  any.textContent = 'Any';
  sel.append(any);
  for (const r of regionList) {
    const opt = document.createElement('option');
    opt.value = r.id;
    opt.textContent = r.name;
    sel.append(opt);
  }
  sel.value = lobbyFilters.region;
}

/// The sidebar's Connection Quality panel (M11): every region
/// with the player's live ping from the probe cycle and its
/// quality square — the reference browser's panel, our regions.
function renderConnQuality() {
  if (!netUi || !netUi.connQualityList) return;
  const list = netUi.connQualityList;
  list.innerHTML = '';
  for (const r of regionList) {
    const ms = lobbyPings[r.id];
    const row = document.createElement('div');
    row.className = 'cq-row';
    row.innerHTML = `<span class="cq-name">${r.name}</span>`
      + `<span class="cq-ms">${typeof ms === 'number' ? Math.round(ms) : '…'}</span>`
      + qualitySquare(pingQuality(typeof ms === 'number' ? ms : null));
    list.append(row);
  }
}

function joinListedGame(room) {
  const entry = lobbyGames.open.find((g) => g.room === room);
  const target = joinTarget(entry);
  if (!target) return;
  lobbySelectedRoom = null;
  openLobby(target);
}

/// The quiet way out of the browser: into a solo match. M10:
/// solo passes through the loading screen too — but the screen
/// lasts exactly as long as the real load takes (the WASM +
/// artwork fetch on first play; near-instant after that). No
/// artificial delay, ever. From an online match it leaves first.
async function playSoloFromLobby() {
  stopLobbyBrowser();
  if (net && (net.phase === 'playing' || net.phase === 'peer-left')) {
    leaveNetMatch('abandoned');
    setPaused(false);
    playing = true;
    return;
  }
  if (net) teardownNet();
  showOverlay(netUi.onlineOverlay, false);
  // Solo loading screen: one seat — you, loading, then loaded.
  if (netUi) {
    showOverlay(netUi.loadingOverlay, true);
    netUi.loadSlotHost.innerHTML =
      '<span class="slot-name">Blue — You</span>' +
      '<span class="slot-state">loading…</span>';
    netUi.loadSlotGuest.style.display = 'none';
    setLoadingStatus('Loading…');
  }
  try {
    await ensureGameLoaded((t) => setLoadingStatus(t));
  } catch {
    hideLoading();
    openLobby(null);
    lobbyStatus('The game failed to load — check your connection and try again.');
    return;
  }
  resetSim(matchMode);
  hideLoading();
  setInMatch(true); // solo start: the game appears
  setPaused(false);
  playing = true;
}

// --- replay playback -----------------------------------------------------------

async function startPlayback(rec) {
  if (net) teardownNet();
  // A replay re-simulates the match, so it needs the game loaded
  // (M10 load order) — a first-ever Watch click loads it here.
  try {
    await ensureGameLoaded();
  } catch {
    flash('The game failed to load — check your connection and try again.');
    return;
  }
  const events = replayEvents(rec);
  if (!events) {
    flash('That file is not a Skirmish II replay.');
    return;
  }
  const owners = Array.isArray(rec.seats) && rec.seats.length >= 2
    ? rec.seats : [0, 2];
  sim = rec.modeTag === 0
    ? new WasmSim(rec.seed)
    : WasmSim.new_match(rec.seed, rec.modeTag);
  playback = {
    rec, events, idx: 0, owners,
    view: typeof rec.view === 'number' ? rec.view : owners[0],
    speed: 1, playing: true, acc: 0, last: 0, done: false,
    totalTicks: rec.ticks || 0,
  };
  adoptSim(sim, playback.view);
  setInMatch(true); // playback entry: the game appears
  // adoptSim replaced `sim` with itself and cleared playback? No —
  // adoptSim does not touch playback; but it DOES reset local state.
  playback.last = performance.now();
  if (netUi && netUi.playbackBar) {
    netUi.playbackBar.hidden = false;
    netUi.pbLabel.textContent = 'Replay — ' + (rec.region ? regionName(rec.region) + ' · ' : '')
      + `seed ${rec.seed}`;
    netUi.pbToggle.textContent = 'Pause';
    netUi.pbSpeed.textContent = '1×';
  }
  playing = true;
  setPaused(false);
  flash('Playing the replay. Orders are disabled; camera is free.');
}

function exitPlayback() {
  playback = null;
  if (netUi && netUi.playbackBar) netUi.playbackBar.hidden = true;
  resetSim(matchMode);
  playing = true;
  setPaused(false);
}

function stepReplayTick() {
  const t = sim.tick_count();
  while (playback.idx < playback.events.length
      && playback.events[playback.idx].tick <= t) {
    const e = playback.events[playback.idx++];
    for (const c of e.s0) netApplyCommand(sim, c, playback.owners[0], (o) => netTcIdOf(sim, o));
    if (e.s1) {
      for (const c of e.s1) netApplyCommand(sim, c, playback.owners[1], (o) => netTcIdOf(sim, o));
    }
  }
  sim.tick();
  if (playback.totalTicks && sim.tick_count() >= playback.totalTicks) {
    playback.done = true;
    playback.playing = false;
    if (netUi) netUi.pbToggle.textContent = 'Play';
    flash('Replay ended.');
  }
}

function drivePlayback(now) {
  if (!playback.playing) {
    playback.last = now;
    return;
  }
  if (!playback.last) playback.last = now;
  playback.acc += Math.min(now - playback.last, 250) * playback.speed;
  playback.last = now;
  const STEP = 1000 / 60;
  let n = 0;
  while (playback.acc >= STEP && n < 24 && !playback.done) {
    stepReplayTick();
    playback.acc -= STEP;
    n++;
  }
  if (n === 24) playback.acc = 0;
}

/// Download a replay object as a .json file.
function downloadReplay(replayObj) {
  if (!replayObj) {
    flash('No replay recorded yet.');
    return;
  }
  const blob = new Blob([JSON.stringify(replayObj)], { type: 'application/json' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = `skirmish2-replay-seed${replayObj.seed}.json`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 4000);
}

function currentReplay() {
  if (playback) return playback.rec;
  if (net && net.recorder) {
    return net.finalReplay || net.recorder.finish({
      result: 'ongoing', ticks: sim.tick_count(),
    });
  }
  if (localRecorder) {
    return localRecorder.finish({
      result: matchEnded ? 'finished' : 'ongoing', ticks: sim.tick_count(),
    });
  }
  return null;
}

async function main() {
  // M10 load order: NO WASM init and NO sim at boot. The lobby is
  // light — the game loads inside the loading phase, when a
  // match actually starts (ensureGameLoaded). HALF/GW/GH/CS/OBST
  // and the minimap geometry are set there, from a probe sim.
  HALF = null;
  fitCanvas();
  addEventListener('resize', () => {
    fitCanvas();
  });

  // --- Menus -----------------------------------------------------------
  // No start screen (Claudio's direction, 2026-10-17): online is
  // the front door (the Lobby Browser below); solo starts from
  // it in one tap. The mode radios live in the pause menu;
  // Restart applies the checked one.
  for (const inp of modeInputs) inp.checked = inp.value === matchMode;
  function chosenMode() {
    const chosen = document.querySelector('input[name="match-mode"]:checked');
    return chosen ? chosen.value : matchMode;
  }
  playing = false; // no match exists until one is entered
  paused = false;
  pauseBtn.addEventListener('click', () => setPaused(true));
  // Idle indicators double as buttons: the same cycle the '.' / ','
  // keys run (SEL-08/09).
  idleVilBtn.addEventListener('click', () => { if (sim) cycleIdle('villager', false); });
  idleMilBtn.addEventListener('click', () => { if (sim) cycleIdle('military', false); });
  resumeBtn.addEventListener('click', () => setPaused(false));
  menuClose.addEventListener('click', () => setPaused(false));
  helpCloseX.addEventListener('click', () => { helpOverlay.style.display = 'none'; });
  helpBtn.addEventListener('click', () => { helpOverlay.style.display = 'flex'; });
  helpClose.addEventListener('click', () => { helpOverlay.style.display = 'none'; });
  helpOverlay.addEventListener('click', (e) => {
    if (e.target === helpOverlay) helpOverlay.style.display = 'none';
  });
  restartBtn.addEventListener('click', () => {
    // Restart begins a fresh match in the mode checked in the menu.
    // In an online match, Restart leaves the match (it cannot be
    // restarted locally — the opponent is a person, not a script).
    if (net) {
      leaveNetMatch('abandoned');
      return;
    }
    // M12: an AI-room match restarts like a local match — the
    // same configuration (same seed, same AI difficulty).
    if (aiRoom) {
      restartAiMatch();
      return;
    }
    resetSim(chosenMode());
    setInMatch(true); // a fresh live match is on screen
    setPaused(false);
    playing = true;
  });
  // Match end (M5): Restart runs the menu's checked mode again from
  // the same seed; Back to menu resets a fresh match and opens the
  // pause menu over it (there is no start screen anymore).
  endRestartBtn.addEventListener('click', () => {
    if (net) {
      // The finished online match cannot restart; leave it and
      // begin a fresh local match in the mode checked in the menu.
      leaveNetMatch('abandoned');
      resetSim(chosenMode());
      setInMatch(true);
      setPaused(false);
      return;
    }
    // M12: an AI-room match restarts with its own configuration.
    if (aiRoom) {
      restartAiMatch();
      return;
    }
    resetSim(chosenMode());
    setInMatch(true);
    playing = true;
    paused = false;
  });
  endMenuBtn.addEventListener('click', () => {
    if (net) {
      leaveNetMatch('abandoned');
      return;
    }
    // M12: from an AI-room match, Back to menu means the Lobby
    // Browser (the room was the way in).
    if (aiRoom) {
      endOverlay.style.display = 'none';
      openLobby(null);
      return;
    }
    setInMatch(false); // back to the menu: chrome hides again
    resetSim(builtMode || matchMode);
    playing = true;
    setPaused(true);
  });

  // --- M8 wiring: lobby, online menu entries, replay ----------------
  netUi = {
    onlineOverlay: document.getElementById('online-overlay'),
    onlineClose: document.getElementById('online-close'),
    gamesSection: document.getElementById('games-section'),
    createSection: document.getElementById('create-section'),
    gamesList: document.getElementById('games-list'),
    gamesNote: document.getElementById('games-in-progress'),
    gamesSearch: document.getElementById('games-search'),
    gamesShowLocked: document.getElementById('games-show-locked'),
    gamesRegionFilter: document.getElementById('games-region-filter'),
    refreshGamesBtn: document.getElementById('btn-refresh-games'),
    joinLobbyBtn: document.getElementById('btn-join-lobby'),
    createOpenBtn: document.getElementById('btn-create-open'),
    createBackBtn: document.getElementById('btn-create-back'),
    connQualityList: document.getElementById('conn-quality-list'),
    soloBtn: document.getElementById('btn-solo'),
    regionList: document.getElementById('region-list'),
    createBtn: document.getElementById('btn-create-game'),
    roomPassword: document.getElementById('room-password'),
    pwSetRow: document.getElementById('pw-set-row'),
    createGuestSlot: document.getElementById('create-guest-slot'),
    roomSection: document.getElementById('room-section'),
    slotHost: document.getElementById('slot-host'),
    slotGuest: document.getElementById('slot-guest'),
    roomConfigLine: document.getElementById('room-config-line'),
    slotControlRow: document.getElementById('slot-control-row'),
    roomGuestSlot: document.getElementById('guest-slot-select'),
    roomListedLine: document.getElementById('room-listed-line'),
    roomPrivateLine: document.getElementById('room-private-line'),
    pwRow: document.getElementById('pw-row'),
    pwInput: document.getElementById('pw-input'),
    pwEnterBtn: document.getElementById('btn-pw-enter'),
    pwNote: document.getElementById('pw-note'),
    readyBtn: document.getElementById('btn-ready'),
    copyLinkBtn: document.getElementById('btn-copy-link'),
    lobbyStatus: document.getElementById('lobby-status'),
    loadingOverlay: document.getElementById('loading-overlay'),
    loadSlotHost: document.getElementById('load-slot-host'),
    loadSlotGuest: document.getElementById('load-slot-guest'),
    loadingStatus: document.getElementById('loading-status'),
    netStatus: document.getElementById('net-status'),
    netStatusText: document.getElementById('net-status-text'),
    resignBtn: document.getElementById('btn-resign'),
    netWait: document.getElementById('net-wait'),
    desyncOverlay: document.getElementById('desync-overlay'),
    desyncDetail: document.getElementById('desync-detail'),
    desyncReplayBtn: document.getElementById('btn-desync-replay'),
    desyncCloseBtn: document.getElementById('btn-desync-close'),
    peerOverlay: document.getElementById('peer-overlay'),
    peerDetail: document.getElementById('peer-detail'),
    claimBtn: document.getElementById('btn-claim-victory'),
    leaveBtn: document.getElementById('btn-leave-match'),
    playbackBar: document.getElementById('playback-bar'),
    pbLabel: document.getElementById('pb-label'),
    pbToggle: document.getElementById('pb-toggle'),
    pbSpeed: document.getElementById('pb-speed'),
    pbExit: document.getElementById('pb-exit'),
  };
  const onlineBtn = document.getElementById('btn-online');
  if (onlineBtn) onlineBtn.addEventListener('click', () => openLobby(null));
  if (netUi.onlineClose) {
    netUi.onlineClose.addEventListener('click', () => {
      stopLobbyBrowser();
      showOverlay(netUi.onlineOverlay, false);
    });
  }
  if (netUi.refreshGamesBtn) {
    netUi.refreshGamesBtn.addEventListener('click', () => refreshLobbyGames());
  }
  // M11 controls row: search / Password toggle / Server filter —
  // each keystroke or change re-renders through lobby.js' pure
  // filter; the directory poll keeps running underneath.
  if (netUi.gamesSearch) {
    netUi.gamesSearch.addEventListener('input', () => {
      lobbyFilters.search = netUi.gamesSearch.value;
      renderLobbyGames();
    });
  }
  if (netUi.gamesShowLocked) {
    netUi.gamesShowLocked.addEventListener('change', () => {
      lobbyFilters.showPassword = !!netUi.gamesShowLocked.checked;
      renderLobbyGames();
    });
  }
  if (netUi.gamesRegionFilter) {
    netUi.gamesRegionFilter.addEventListener('change', () => {
      lobbyFilters.region = netUi.gamesRegionFilter.value;
      renderLobbyGames();
    });
  }
  if (netUi.joinLobbyBtn) {
    netUi.joinLobbyBtn.addEventListener('click', () => {
      if (lobbySelectedRoom) joinListedGame(lobbySelectedRoom);
    });
  }
  // M11 action bar: Create Lobby opens the create panel; the
  // panel's quiet back line returns to the browser.
  if (netUi.createOpenBtn) {
    netUi.createOpenBtn.addEventListener('click', () => {
      if (netUi.gamesSection) netUi.gamesSection.hidden = true;
      if (netUi.createSection) netUi.createSection.hidden = false;
    });
  }
  if (netUi.createBackBtn) {
    netUi.createBackBtn.addEventListener('click', () => {
      if (netUi.createSection) netUi.createSection.hidden = true;
      if (netUi.gamesSection) netUi.gamesSection.hidden = false;
    });
  }
  if (netUi.soloBtn) {
    netUi.soloBtn.addEventListener('click', playSoloFromLobby);
  }
  if (netUi.createBtn) netUi.createBtn.addEventListener('click', createGame);
  // M12: the room screen's guest-seat selector (host only; the
  // create panel's twin is read by createGame itself).
  if (netUi.roomGuestSlot) {
    netUi.roomGuestSlot.addEventListener('change', () => {
      setGuestSlot(netUi.roomGuestSlot.value);
    });
  }
  // M10: the room screen's one button toggles ready; the host's
  // client starts the match itself when both players are ready.
  if (netUi.readyBtn) netUi.readyBtn.addEventListener('click', toggleReady);
  if (netUi.pwEnterBtn) netUi.pwEnterBtn.addEventListener('click', submitPassword);
  if (netUi.pwInput) {
    netUi.pwInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') submitPassword();
    });
  }
  // The privacy choice reveals the password field (private rooms).
  for (const radio of document.querySelectorAll('input[name="room-privacy"]')) {
    radio.addEventListener('change', () => {
      if (netUi.pwSetRow) {
        netUi.pwSetRow.hidden = !(radio.checked && radio.value === 'private');
      }
    });
  }
  if (netUi.copyLinkBtn) {
    netUi.copyLinkBtn.addEventListener('click', () => {
      // The invite link is built on demand from in-memory room
      // state — never read from the address bar, which is kept
      // room-free so a reload lands on the Lobby Browser.
      if (net && net.room) {
        net.roomLinkValue = buildPlayUrl(
          location.origin + location.pathname, net.room, net.region,
          { private: net.privacy === 'private' });
        if (navigator.clipboard) {
          navigator.clipboard.writeText(net.roomLinkValue);
        }
      }
      flash('Link copied — send it to your opponent.');
    });
  }
  if (netUi.resignBtn) {
    netUi.resignBtn.addEventListener('click', () => {
      if (!net) return;
      net.client.send({ type: 'resign' });
      showOverlay(netUi.peerOverlay, false);
      netMatchEnd(2, { selfResign: true });
      setPaused(false);
    });
  }
  if (netUi.claimBtn) {
    netUi.claimBtn.addEventListener('click', () => {
      if (!net) return;
      showOverlay(netUi.peerOverlay, false);
      netMatchEnd(1, {});
    });
  }
  if (netUi.leaveBtn) {
    netUi.leaveBtn.addEventListener('click', () => leaveNetMatch('abandoned'));
  }
  if (netUi.desyncReplayBtn) {
    netUi.desyncReplayBtn.addEventListener('click', () => downloadReplay(currentReplay()));
  }
  if (netUi.desyncCloseBtn) {
    netUi.desyncCloseBtn.addEventListener('click', () => leaveNetMatch('desync'));
  }
  const endReplayBtn = document.getElementById('end-replay-btn');
  if (endReplayBtn) {
    endReplayBtn.addEventListener('click', () => downloadReplay(currentReplay()));
  }
  const dlReplayBtn = document.getElementById('btn-download-replay');
  if (dlReplayBtn) {
    dlReplayBtn.addEventListener('click', () => downloadReplay(currentReplay()));
  }
  const replayFile = document.getElementById('replay-file');
  if (replayFile) {
    replayFile.addEventListener('change', async () => {
      const f = replayFile.files && replayFile.files[0];
      replayFile.value = '';
      if (!f) return;
      try {
        const rec = JSON.parse(await f.text());
        if (!isReplay(rec)) throw new Error('not a replay');
        setPaused(false);
        startPlayback(rec);
      } catch {
        flash('That file is not a Skirmish II replay.');
      }
    });
  }
  if (netUi.pbToggle) {
    netUi.pbToggle.addEventListener('click', () => {
      if (!playback || playback.done) return;
      playback.playing = !playback.playing;
      netUi.pbToggle.textContent = playback.playing ? 'Pause' : 'Play';
    });
  }
  if (netUi.pbSpeed) {
    netUi.pbSpeed.addEventListener('click', () => {
      if (!playback) return;
      const speeds = [0.5, 1, 2, 4];
      const next = speeds[(speeds.indexOf(playback.speed) + 1) % speeds.length];
      playback.speed = next;
      netUi.pbSpeed.textContent = next + '×';
    });
  }
  if (netUi.pbExit) netUi.pbExit.addEventListener('click', exitPlayback);

  // M9: the play page OPENS on the Lobby Browser (online-first) —
  // the local match waits behind the panel and "Play solo instead"
  // starts it in one tap. Arriving by link skips the browser and
  // joins that room directly; ?online=1 lands on the browser too.
  // Reload rule: the link is consumed ONCE per tab — the mark is
  // written and the address bar cleaned BEFORE joining, so any
  // later reload of this tab lands on the Lobby Browser.
  {
    const join = linkJoinRoom(location.search, readRoomLinkMark());
    setPaused(true);
    if (join) {
      writeRoomLinkMark(join.room);
      cleanPlayUrl();
      openLobby({ room: join.room, region: join.region, private: join.private });
    } else {
      cleanPlayUrl(); // a consumed link's params leave the bar too
      openLobby(null);
    }
  }
  // Quality override (pause menu): Auto / High / Medium / Low.
  for (const [mode, btn] of Object.entries(qualityModeBtns)) {
    if (btn) btn.addEventListener('click', () => setQualityMode(mode));
  }
  updateQualityUi();

  // Command panel: ONE delegated listener — renderCommands() rebuilds
  // the buttons per selection context, so per-button refs would go
  // stale. The behavior lives in runCommandButton() (top level), so
  // the M13 hotkeys run the very same code a click runs.
  cmdGrid.addEventListener('click', (ev) => {
    const btn = ev.target.closest('button');
    if (!btn || btn.disabled) return;
    runCommandButton(btn.id);
  });

  // Minimap camera control: click or drag to jump the main camera.
  // The bottom bar floats above #view, so these gestures never reach
  // the canvas selection/pan handlers. Client px are converted to
  // canvas px through the bounding rect — the CSS display size
  // differs from the attribute size on narrow screens.
  // M4 (SEL-16): a RIGHT-click on the minimap instead issues the
  // smart order (or sets a rally point) at that world point.
  let mmDrag = false;
  function minimapPointerWorld(ev) {
    if (!mmGeom) return null;
    const r = minimapCanvas.getBoundingClientRect();
    const mx = (ev.clientX - r.left) * (minimapCanvas.width / r.width);
    const my = (ev.clientY - r.top) * (minimapCanvas.height / r.height);
    return minimapToWorld(mmGeom, mx, my);
  }
  function minimapJump(ev) {
    const w = minimapPointerWorld(ev);
    if (!w) return;
    [camOx, camOy] = cameraCenteredOn(w[0], w[1], camS, canvas.width, canvas.height);
  }
  minimapCanvas.addEventListener('pointerdown', (ev) => {
    if (ev.button === 2) {
      const w = minimapPointerWorld(ev);
      if (w) smartOrderAt(w[0], w[1], ev.shiftKey);
      return;
    }
    mmDrag = true;
    minimapCanvas.setPointerCapture(ev.pointerId);
    minimapJump(ev);
  });
  minimapCanvas.addEventListener('pointermove', (ev) => {
    if (mmDrag) minimapJump(ev);
  });
  minimapCanvas.addEventListener('pointerup', () => { mmDrag = false; });
  minimapCanvas.addEventListener('pointercancel', () => { mmDrag = false; });
  minimapCanvas.addEventListener('contextmenu', (ev) => ev.preventDefault());

  // --- Pointer ---------------------------------------------------------
  function eventWorld(ev) {
    const r = canvas.getBoundingClientRect();
    const sx = (ev.clientX - r.left) * (canvas.width / r.width);
    const sy = (ev.clientY - r.top) * (canvas.height / r.height);
    let [wx, wy] = s2w(sx, sy, camS, camOx, camOy);
    // M7: the projection lifts elevated ground (see worldToScreen);
    // picking must undo the lift — solve with the height at the
    // flat estimate, then once more with the refined one.
    const h = heightAtWorld(wx, wy);
    if (h > 0) {
      [wx, wy] = s2w(sx, sy + h * ELEV_LIFT * camS, camS, camOx, camOy);
      const h2 = heightAtWorld(wx, wy);
      if (h2 !== h) [wx, wy] = s2w(sx, sy + h2 * ELEV_LIFT * camS, camS, camOx, camOy);
    }
    return [wx, wy];
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

  // The smart order at a point, from the main view OR the minimap
  // (SEL-16): with the Town Center or a Barracks selected, a
  // right-click sets that building's rally point (SEL-13); with
  // units selected it is the context order below. `queued` is the
  // Shift form (SEL-12): the order lines up behind each unit's
  // current one instead of replacing it.
  function smartOrderAt(wx, wy, queued = false) {
    if (selectedBuilding) {
      // M5: rally points belong to OUR buildings only — an enemy
      // building selected for information takes no orders at all;
      // the click falls through to the unit-order path (whose
      // selection is empty, so it just flashes the hint).
      const foe = selectedBuilding.owner === foeOwner();
      if (!foe && selectedBuilding.type === 'tc') {
        issue({ t: 'rally_tc', x: wx, y: wy });
        return;
      }
      if (!foe && selectedBuilding.type === 'building' && [2, 6, 7].includes(selectedBuilding.kind)) {
        issue({ t: 'rally_b', b: selectedBuilding.id, x: wx, y: wy });
        return;
      }
    }
    orderAt(wx, wy, queued);
  }

  // Context order at a point (M3): an entity there (Gaia animal or
  // our own sheep) means Attack for everyone who can fight; a node
  // means Gather for villagers and Move for the rest; ground means
  // Move. Goes ONLY to the current selection (AoE2-style) — with an
  // empty selection the order does nothing but flash the hint.
  function orderAt(wx, wy, queued = false) {
    const targets = selectedUnits();
    if (targets.length === 0) {
      flash(selectedBuilding != null
        ? (selectedBuilding.owner === foeOwner()
          ? 'That is the enemy’s — look, but the orders are not yours.'
          : selectedBuilding.type === 'tc'
            ? 'The Town Center trains villagers from the Commands panel.'
            : 'Train units from the Commands panel.')
        : 'Select a unit first.');
      return;
    }
    const st = entityState();
    // Group-move shape: with 2+ units ordered onto open ground, each
    // keeps its offset from the group's centroid (clamped to 6
    // units), so a group arrives in the shape it left in instead of
    // stacking onto one point. An offset that would land inside a
    // static wall falls back to the plain shared target.
    let moveDest = null;
    if (targets.length >= 2) {
      let cx = 0;
      let cy = 0;
      for (const id of targets) {
        cx += st.pos[id * 2];
        cy += st.pos[id * 2 + 1];
      }
      cx /= targets.length;
      cy /= targets.length;
      moveDest = (id) => {
        let dx = st.pos[id * 2] - cx;
        let dy = st.pos[id * 2 + 1] - cy;
        const len = Math.hypot(dx, dy);
        if (len > 6) {
          dx *= 6 / len;
          dy *= 6 / len;
        }
        const tx = wx + dx;
        const ty = wy + dy;
        const [ccx, ccy] = worldToCell(tx, ty);
        return OBST[ccy * GW + ccx] ? [wx, wy] : [tx, ty];
      };
    }
    const sendMove = (id) => {
      const [mx, my] = moveDest ? moveDest(id) : [wx, wy];
      issue({ t: 'move', e: id, x: mx, y: my, q: queued || undefined });
    };
    // The enemy Town Center (M5): entityAt skips Town Centers, so
    // it gets its own pick — an attack order for everyone who can
    // fight. Our own TC is never a target: clicks on it fall
    // through to the ordinary logic below.
    const tcT = tcAt(wx, wy);
    // M7: an order can only name what the player knows — the
    // enemy Town Center once its ground is explored (VIS-06).
    if (tcT != null && st.owners[tcT] === foeOwner()
        && cellExploredW(st.pos[tcT * 2], st.pos[tcT * 2 + 1])) {
      let any = false;
      for (const id of targets) {
        if (st.kinds[id] === KIND_SHEEP) continue;
        if (issue({ t: 'attack', e: id, tg: tcT, q: queued || undefined })) any = true;
      }
      if (any) return;
    }
    let t = entityAt(wx, wy);
    // M7: a non-own unit is a target only while currently visible
    // — in explored memory or the black it cannot be clicked
    // (the click falls through to a ground move, as a miss does).
    if (t != null && st.owners[t] !== localOwner
        && !cellVisibleW(st.pos[t * 2], st.pos[t * 2 + 1])) t = null;
    if (t != null && (st.owners[t] === 1 || st.owners[t] === foeOwner() || st.kinds[t] === KIND_SHEEP)) {
      let any = false;
      for (const id of targets) {
        if (st.kinds[id] === KIND_SHEEP) continue; // sheep don't fight
        if (issue({ t: 'attack', e: id, tg: t, q: queued || undefined })) any = true;
      }
      if (any) return;
      // Nobody could attack (e.g. only sheep selected): fall through
      // to a plain move toward the point.
    }
    // An enemy building (M5): attack it via the building order.
    // (No queued form in M5 — the order replaces, like a fresh
    // right-click should.)
    const bT = buildingAt(wx, wy);
    if (bT && bT.owner === foeOwner() && cellExploredW(bT.x, bT.y)) {
      let any = false;
      for (const id of targets) {
        if (st.kinds[id] === KIND_SHEEP) continue;
        if (issue({ t: 'attackb', e: id, b: bT.id })) any = true;
      }
      if (any) return;
    }
    // Force drop-off (M6, ECO-07): the click lands on OUR Town
    // Center or one of our camps — every selected villager the
    // carry heuristic sees loaded hauls its load in now, then
    // returns to its work on its own. The heuristic is the client's
    // carry view; the sim re-validates each order itself.
    const ownDrop = (tcT != null && st.owners[tcT] === localOwner)
      || (bT && bT.owner === localOwner && bT.kind >= 3 && bT.kind <= 5);
    if (ownDrop) {
      let any = false;
      for (const id of targets) {
        if (st.kinds[id] === 0 && carryKind.has(id) && issue({ t: 'force_dep', e: id })) any = true;
      }
      if (any) return;
    }
    let n = nodeAt(wx, wy);
    // M7: a node on unexplored ground cannot be ordered — the
    // click is a ground move instead.
    if (n && !cellExploredW(n.x, n.y)) n = null;
    for (const id of targets) {
      if (n && st.kinds[id] === 0) {
        issue({ t: 'gather', e: id, n: n.id, q: queued || undefined });
      } else {
        sendMove(id);
      }
    }
  }

  function attemptPlace(wx, wy, queued = false) {
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
    // Queued (Shift) placement: the build lines up behind the
    // builder's current order and placement mode STAYS armed, so a
    // row of buildings can be queued with repeated Shift+clicks.
    // (The queued form validates at execution — see Sim::can_queue —
    // so its acceptance here only proves the builder can build.)
    const ok = issue({
      t: 'build', e: builder, k: placement.kind, x: px, y: py,
      q: queued || undefined,
    });
    if (ok) {
      if (!queued) placement = null;
    } else {
      flash(`Cannot build a ${BUILD_NAME[placement.kind]} there.`);
    }
  }

  canvas.addEventListener('mousemove', (ev) => {
    mouseWorld = eventWorld(ev);
    // M13 edge scrolling rides on this: the pointer is over the
    // game canvas itself (the UI bars and overlays are separate
    // elements above it and report themselves as targets below).
    mouseClientPt = { x: ev.clientX, y: ev.clientY };
    mouseOverCanvas = true;
  });
  addEventListener('mousemove', (e) => {
    if (e.target !== canvas) mouseOverCanvas = false;
  });
  document.addEventListener('mouseleave', () => {
    mouseClientPt = null;
    mouseOverCanvas = false;
  });
  addEventListener('blur', () => {
    mouseClientPt = null;
    mouseOverCanvas = false;
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
      if (!d.moved) attemptPlace(wx, wy, ev.shiftKey); // a placement click, not a drag
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
      // ground clears both selections. M5: the enemy's TC and
      // buildings select too — for information only (their owner
      // rides along in the descriptor; the panels gate on it).
      const tc = tcAt(wx, wy);
      const st = entityState();
      // M7: information selection obeys fog too — an enemy Town
      // Center or building selects only once its ground is
      // explored; in the black the click is plain empty ground.
      if (tc != null && (st.owners[tc] === localOwner
          || cellExploredW(st.pos[tc * 2], st.pos[tc * 2 + 1]))) {
        selectBuilding({ type: 'tc', id: tc, owner: st.owners[tc] });
      } else {
        const b = buildingAt(wx, wy);
        if (b && (b.owner !== foeOwner() || cellExploredW(b.x, b.y))) {
          selectBuilding({ type: 'building', id: b.id, kind: b.kind, owner: b.owner });
        } else selectUnits(new Set());
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

  // Pinch zoom (touch, Claudio 2026-10-17): two fingers on the
  // canvas zoom the CAMERA toward the pinch midpoint — fast, direct
  // ratio — while the page itself never zooms (touch-action:none on
  // the canvas, viewport maximum-scale, and the preventDefaults
  // here; iOS Safari also gets the gesturestart block below).
  let pinchD = 0;
  const pinchDist = (e) => Math.hypot(
    e.touches[0].clientX - e.touches[1].clientX,
    e.touches[0].clientY - e.touches[1].clientY);
  canvas.addEventListener('touchstart', (e) => {
    if (e.touches.length === 2) { pinchD = pinchDist(e); e.preventDefault(); }
  }, { passive: false });
  canvas.addEventListener('touchmove', (e) => {
    if (e.touches.length === 2 && pinchD > 0) {
      e.preventDefault();
      const d = pinchDist(e);
      const r = canvas.getBoundingClientRect();
      const mx = ((e.touches[0].clientX + e.touches[1].clientX) / 2 - r.left) * (canvas.width / r.width);
      const my = ((e.touches[0].clientY + e.touches[1].clientY) / 2 - r.top) * (canvas.height / r.height);
      const ns = Math.min(S_MAX, Math.max(S_MIN, camS * (d / pinchD)));
      camOx = mx - (mx - camOx) * (ns / camS);
      camOy = my - (my - camOy) * (ns / camS);
      camS = ns;
      pinchD = d;
    }
  }, { passive: false });
  const pinchEnd = (e) => { if (e.touches.length < 2) pinchD = 0; };
  canvas.addEventListener('touchend', pinchEnd);
  canvas.addEventListener('touchcancel', pinchEnd);
  document.addEventListener('gesturestart', (e) => e.preventDefault());

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
    if (buildPage) {
      buildPage = null; // M13: a right-click backs out of a page too
      return;
    }
    const [wx, wy] = eventWorld(e);
    smartOrderAt(wx, wy, e.shiftKey);
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
    // M10 load order: before any match is entered there is no
    // sim at all — the lobby sits over an empty canvas. Paint the
    // bare backdrop and wait.
    if (!sim) {
      ctx.fillStyle = '#0b0f14';
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      prevFrameAt = now;
      last = now;
      requestAnimationFrame(frame);
      return;
    }
    // M8: in an online match the lockstep session owns the tick
    // loop — it applies both players' bundles at their execution
    // turns and ticks there; the free-running accumulator below is
    // bypassed (never both). The pause menu does NOT hold an online
    // match: the opponent plays on, so the session pumps regardless
    // of `paused`. In replay playback the replay driver ticks.
    if (net && net.session) {
      if (net.phase === 'playing' || net.phase === 'peer-left') {
        net.session.pump(now);
      }
      if (prevFrameAt != null) {
        const step = governor.sample(now - prevFrameAt, now);
        if (step.changed) applyQuality(step.level, { announce: true });
      }
      prevFrameAt = now;
      last = now;
      applyKeys(now);
      draw();
      requestAnimationFrame(frame);
      return;
    }
    if (playback) {
      drivePlayback(now);
      prevFrameAt = now;
      last = now;
      applyKeys(now);
      draw();
      requestAnimationFrame(frame);
      return;
    }
    if (playing && !paused && !matchEnded) {
      acc += Math.min(now - last, 250);
      let n = 0;
      while (acc >= STEP && n < 5) {
        sim.tick();
        acc -= STEP;
        n++;
      }
      if (n === 5) acc = 0;
      // M5: Conquest check — the first decided tick raises the
      // end overlay and holds the sim (see showMatchEnd).
      if (builtMode !== 'sandbox') {
        const ms = sim.match_state();
        if (ms !== 0) showMatchEnd(ms);
      }
      if (prevFrameAt != null) {
        const step = governor.sample(now - prevFrameAt, now);
        if (step.changed) applyQuality(step.level, { announce: true });
      }
    } else {
      acc = 0;
    }
    prevFrameAt = now;
    last = now;
    applyKeys(now);
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

// (M7: the arena's drawBlock wall sprite is retired — there are no
// wall cells anymore; blocked ground is water or trees, rendered
// as terrain and nodes.)

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
  if (drawSpriteH(NODE_ART.img.tree, sx, sy, 4.4)) return;
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
  if (drawSpriteH(NODE_ART.img.berry, sx, sy, 1.9)) return;
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
  if (drawSpriteW(NODE_ART.img.gold, sx, sy, 2.6)) return;
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
  if (drawSpriteW(NODE_ART.img.stone, sx, sy, 2.8)) return;
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
  // Base44 art: the farm plot image fitted to the field's
  // footprint diamond (width matched to the diamond, aspect kept,
  // centered on it). The image is one of four growth states picked
  // by the food fraction remaining (frac > 0.66 ripe, > 0.4 green,
  // > 0.15 shoots, else planted), so a field visibly winds down as
  // it is harvested; farm.png itself is the ripe frame. The
  // depletion cue survives as a bare-soil tint that strengthens as
  // the crop runs out. Fallback: the vector plot.
  // (A farm UNDER CONSTRUCTION still draws the bare vector plot via
  // drawBuilding/drawFarmPlot — the construction feedback.)
  const stateIm = frac > 0.66 ? BUILDING_ART.img.farmState4
    : frac > 0.4 ? BUILDING_ART.img.farmState3
    : frac > 0.15 ? BUILDING_ART.img.farmState2
    : BUILDING_ART.img.farmState1;
  const im = spriteReady(stateIm) ? stateIm : BUILDING_ART.img.farm;
  if (spriteReady(im)) {
    const t = worldToScreen(wx - CS, wy - CS);
    const r = worldToScreen(wx + CS, wy - CS);
    const b = worldToScreen(wx + CS, wy + CS);
    const l = worldToScreen(wx - CS, wy + CS);
    const w = r[0] - l[0];
    const h = w * (im.naturalHeight / im.naturalWidth);
    const cx = (t[0] + b[0]) / 2;
    const cy = (t[1] + b[1]) / 2;
    ctx.drawImage(im, cx - w / 2, cy - h / 2, w, h);
    if (frac < 1) {
      poly([t, r, b, l], `rgba(107, 74, 44, ${((1 - frac) * 0.4).toFixed(3)})`);
    }
    return;
  }
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
// The Town Center's footprint is 4x4 cells (the DE study; the
// prompt standard composes the TC art for exactly that). The art is
// drawn footprint-based like the other Base44 buildings
// (drawNewBuilding): width = footprint x 1.12, base planted
// half-footprint x 0.45 toward the front corner — NOT the old
// 9.5 / +1.6 placeholder contract, which left the compound small
// inside its 4x4 footprint diamond with empty space at the sides.
const TC_ART_CELLS = 4; // TC footprint edge, in cells
function drawTC(wx, wy, owner = 0) {
  const a = TC_HALF;
  const t = worldToScreen(wx - a, wy - a);
  const r = worldToScreen(wx + a, wy - a);
  const b = worldToScreen(wx + a, wy + a);
  const l = worldToScreen(wx - a, wy + a);
  const [sx, sy] = worldToScreen(wx, wy);
  shadow(sx, sy, (r[0] - l[0]) / 2);
  // Building sprite, footprint-based like drawNewBuilding: width =
  // 4 cells x CS x 1.12 (= 17.92 world units at CS = 4), base planted
  // The Base44 art is normalized in processing so its measured
  // ground-contact diamond sits on the canonical diamond (width
  // 0.893 x canvas, equator at canvas height - 0.2457 x canvas
  // width): drawn at footprint x 1.12 wide with the anchor 4.4 wu
  // above the sprite's bottom edge (half-footprint x 0.55), its
  // base corners land exactly on the 4x4 footprint diamond.
  // The CC0 placeholder is the fallback (same draw contract).
  const tcArtHalf = (TC_ART_CELLS * CS) / 2;
  if (drawSpriteW(BUILDING_ART.img.tc, sx, sy,
    TC_ART_CELLS * CS * 1.12, tcArtHalf * 0.55)) return;
  // While the real art is still in flight — reachable only when
  // the loading gate's settle cap let the match start first —
  // fall through to the vector Town Center below instead of the
  // CC0 placeholder photo, so the eventual pop is from our own
  // shapes, not a photo swap. Once the art has settled the normal
  // chain resumes: an errored image keeps the placeholder.
  if (!buildingArtPending('tc') &&
    drawSpriteW(SPRITES.tc, sx, sy,
      TC_ART_CELLS * CS * 1.12, tcArtHalf * 0.55)) return;
  const h = 2.2 * camS; // wall height
  poly([l, b, up(b, h), up(l, h)], '#b09468'); // front-left wall
  poly([b, r, up(r, h), up(b, h)], '#8a6f45'); // front-right wall
  poly([up(t, h), up(r, h), up(b, h), up(l, h)], '#9c4a2f'); // roof
  // Raised roof cap, for a hip-roof hint.
  const cx = (t[0] + b[0]) / 2;
  const cy = (t[1] + b[1]) / 2 - h - 0.7 * camS;
  diamond(cx, cy, (r[0] - l[0]) / 4, (r[0] - l[0]) / 8, '#b85c38');
  // M5: the owner's pennant — a small player-color triangle above
  // the roof cap, so the two Town Centers read apart at a glance
  // (blue ours, red the enemy's).
  ctx.fillStyle = owner === foeOwner() ? ENEMY_COLOR : OWN_COLOR;
  ctx.beginPath();
  ctx.moveTo(cx, cy - 1.15 * camS);
  ctx.lineTo(cx + 1.05 * camS, cy - 0.82 * camS);
  ctx.lineTo(cx, cy - 0.49 * camS);
  ctx.closePath();
  ctx.fill();
  ctx.fillStyle = '#ffe9c9';
  ctx.font = `bold ${Math.max(9, Math.round(1.1 * camS))}px monospace`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillText('TC', cx, cy);
}

// House rendering. First choice is the Base44 art (see
// BUILDING_ART above): while under construction it draws dedicated
// construction stage frames (one generated strip, sliced into
// thirds of the build progress) over a foundation diamond, on the
// finished art's draw contract so completion has no design jump;
// the clip-reveal of the finished art is the fallback if a stage
// frame is not loaded. Fallback is our original stage
// art (see HOUSE_ORIGINAL above): the sprite carries its own
// foundation and the construction stage follows build progress.
// Last resorts: the CC0 placeholder sprite with the same clip-reveal,
// and finally a vector block that grows with progress.
const HOUSE_W = 8.6; // world units wide (placeholder draw contract)
const HOUSE_DY = 2.0; // base planted toward the footprint's front edge
function drawHouse(b) {
  const [sx, sy] = worldToScreen(b.x, b.y);
  shadow(sx, sy, 3.6 * camS);
  if (spriteReady(BUILDING_ART.img.house)) {
    const t = worldToScreen(b.x - CS, b.y - CS);
    const r = worldToScreen(b.x + CS, b.y - CS);
    const bt = worldToScreen(b.x + CS, b.y + CS);
    const l = worldToScreen(b.x - CS, b.y + CS);
    poly([t, r, bt, l], b.complete ? 'rgba(0,0,0,0.12)' : '#8d8578');
    if (b.complete) {
      drawArtImage(BUILDING_ART.img.house, sx, sy, HOUSE_W, HOUSE_DY, 1);
    } else {
      // Construction stages: dedicated per-stage frames drawn
      // full-size on the finished house's draw contract, so the
      // build visibly grows stage by stage (foundation + posts ->
      // walls + scaffolding -> roof beams) and completion lands on
      // the same design. If the needed stage frame is not loaded
      // yet, fall back to clip-revealing the finished art.
      const stage = b.progress < 1 / 3 ? BUILDING_ART.img.houseStage1
        : b.progress < 2 / 3 ? BUILDING_ART.img.houseStage2
        : BUILDING_ART.img.houseStage3;
      if (!drawArtImage(stage, sx, sy, HOUSE_W, HOUSE_DY, 1)) {
        drawArtImage(BUILDING_ART.img.house, sx, sy, HOUSE_W, HOUSE_DY,
          b.progress);
      }
    }
  } else if (!drawOriginalHouse(b, sx, sy)) {
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

// Barracks rendering (M3): while under construction, the Base44
// construction stage frames over a 3-cell foundation diamond (the
// clip-reveal of the finished art is the fallback), full-size when
// done; the CC0 placeholder keep sprite
// (assets-placeholder/barracks.png) with the same treatment is the
// fallback, and a stone-block vector the last resort. The footprint
// is 3×3 cells.
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
  if (!b.complete &&
    drawArtImage(buildingStageImage(2, b.progress), sx, sy, 13.5, 2.6, 1)) {
    // Construction stage frame drawn (the House rule: dedicated
    // per-stage frames on the finished art's draw contract).
  } else if (drawArtImage(BUILDING_ART.img.barracks, sx, sy, 13.5, 2.6,
    b.complete ? 1 : b.progress)) {
    // Base44 art drawn (full, or clip-revealed by progress).
  } else if (spriteReady(im) && !buildingArtPending('barracks')) {
    // Same pending guard as drawTC: while the real art is still
    // in flight, grow the vector block below rather than the CC0
    // placeholder photo; a settled (errored) image restores it.
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
  } else if (b.kind >= 3) {
    drawNewBuilding(b);
  } else {
    drawHouse(b);
  }
  // M5: a damaged building shows its HP above the roof — buildings
  // are attackable now, and their remaining health is information,
  // never stripped by the quality governor.
  if (b.hpFrac < 0.999) {
    const [sx, sy] = worldToScreen(b.x, b.y);
    hpBar(sx, sy - (b.size === 3 ? 7.3 : 5.8) * camS, b.hpFrac);
  }
}

// The M6 buildings (Lumber Camp, Mill, Mining Camp, Archery Range,
// Stable, Blacksmith): the Base44 art over the foundation
// diamond (full when complete; while building, the construction
// stage frames, with the clip-reveal of the finished art as the
// fallback), sized to the footprint width × 1.12 like the Barracks
// placeholder contract. Fallback: the original vector draws in the
// Barracks' family — a block growing with progress in a per-kind
// palette, a small feature on completion, and the owner's pennant
// (red for the enemy, the M5 treatment; the art path draws the same
// image for both players, so no pennant there).
const NEW_BUILDING_STYLE = {
  3: { wall: '#8a6b42', wall2: '#6f5533', roof: '#5d4630' }, // Lumber Camp: timber
  4: { wall: '#b09a68', wall2: '#8f7a4e', roof: '#c9a86a' }, // Mill: pale timber
  5: { wall: '#9a8f6a', wall2: '#7a7050', roof: '#6b6248' }, // Mining Camp
  6: { wall: '#a3763f', wall2: '#82592c', roof: '#7a4a35' }, // Archery Range
  7: { wall: '#96684a', wall2: '#77503a', roof: '#5d4630' }, // Stable
  8: { wall: '#6f6f76', wall2: '#54545c', roof: '#3f3f45' }, // Blacksmith: dark stone
};

function drawNewBuilding(b) {
  const [sx, sy] = worldToScreen(b.x, b.y);
  const half = (b.size * CS) / 2;
  shadow(sx, sy, half * 1.45 * camS);
  const t = worldToScreen(b.x - half, b.y - half);
  const r = worldToScreen(b.x + half, b.y - half);
  const bt = worldToScreen(b.x + half, b.y + half);
  const l = worldToScreen(b.x - half, b.y + half);
  poly([t, r, bt, l], b.complete ? 'rgba(0,0,0,0.12)' : '#8d8578');
  // Under construction: the dedicated stage frame on the finished
  // art's draw contract (the House rule); the clip-reveal below is
  // the fallback when a stage frame is not loaded.
  let artDrawn = false;
  if (!b.complete) {
    artDrawn = drawArtImage(buildingStageImage(b.kind, b.progress), sx, sy,
      b.size * CS * 1.12, half * 0.45, 1);
  }
  if (!artDrawn &&
    !drawArtImage(BUILDING_ART.img[BUILDING_ART_KEY[b.kind]], sx, sy,
      b.size * CS * 1.12, half * 0.45, b.complete ? 1 : b.progress)) {
    const st = NEW_BUILDING_STYLE[b.kind];
    const hgt = (b.size === 3 ? 3.2 : 2.1) * (b.complete ? 1 : b.progress) * camS;
    poly([l, bt, up(bt, hgt), up(l, hgt)], st.wall);
    poly([bt, r, up(r, hgt), up(bt, hgt)], st.wall2);
    if (b.complete) {
      poly([up(t, hgt), up(r, hgt), up(bt, hgt), up(l, hgt)], st.roof);
      drawBuildingFeature(b.kind, sx, sy, hgt);
      if (b.owner === foeOwner()) {
        // The enemy's pennant above the roof (M5's owner treatment).
        poly([[sx, sy - hgt - 1.9 * camS], [sx + 1.15 * camS, sy - hgt - 1.55 * camS],
          [sx, sy - hgt - 1.2 * camS]], ENEMY_COLOR);
        ctx.fillStyle = '#3a2c1c';
        ctx.fillRect(sx - 0.06 * camS, sy - hgt - 1.9 * camS, 0.12 * camS, 1.9 * camS);
      }
    }
  }
  if (!b.complete) progressBar(sx, sy - (b.size === 3 ? 6.4 : 4.9) * camS, b.progress);
}

// One small identifying feature per M6 building kind, drawn on the
// roof point (sx, sy - hgt): logs for the Lumber Camp, the Mill's
// sail cross, ore flecks for the Mining Camp, a target for the
// Range, a fence line for the Stable, a chimney with an ember for
// the Blacksmith.
function drawBuildingFeature(kind, sx, sy, hgt) {
  const u = camS;
  const top = sy - hgt;
  ctx.lineWidth = Math.max(1, 0.14 * u);
  if (kind === 3) {
    ctx.fillStyle = '#6f4f2a';
    for (const dx of [-1.5, -0.5, 0.5]) {
      ctx.beginPath();
      ctx.ellipse(sx + dx * u, top + 0.5 * u, 0.42 * u, 0.22 * u, 0, 0, Math.PI * 2);
      ctx.fill();
    }
  } else if (kind === 4) {
    ctx.strokeStyle = '#e8dcc0';
    for (const [dx, dy] of [[-1, -1], [1, -1], [-1, 1], [1, 1]]) {
      ctx.beginPath();
      ctx.moveTo(sx, top - 0.9 * u);
      ctx.lineTo(sx + dx * 1.05 * u, top - 0.9 * u + dy * 1.05 * u);
      ctx.stroke();
    }
    ctx.fillStyle = '#5d4630';
    ctx.beginPath();
    ctx.arc(sx, top - 0.9 * u, 0.18 * u, 0, 0, Math.PI * 2);
    ctx.fill();
  } else if (kind === 5) {
    ctx.fillStyle = '#ffd75e';
    for (const [dx, dy] of [[-1.1, 0.3], [0.1, 0.55], [1.15, 0.25]]) {
      ctx.beginPath();
      ctx.arc(sx + dx * u, top + dy * u, 0.17 * u, 0, 0, Math.PI * 2);
      ctx.fill();
    }
  } else if (kind === 6) {
    ctx.strokeStyle = '#e8e8e8';
    ctx.beginPath();
    ctx.arc(sx + 1.5 * u, top + 0.4 * u, 0.55 * u, 0, 0, Math.PI * 2);
    ctx.stroke();
    ctx.fillStyle = '#b03a2e';
    ctx.beginPath();
    ctx.arc(sx + 1.5 * u, top + 0.4 * u, 0.28 * u, 0, 0, Math.PI * 2);
    ctx.fill();
  } else if (kind === 7) {
    ctx.strokeStyle = '#5d4630';
    for (const dy of [0.15, 0.6]) {
      ctx.beginPath();
      ctx.moveTo(sx - 1.9 * u, top + dy * u);
      ctx.lineTo(sx + 1.9 * u, top + dy * u);
      ctx.stroke();
    }
  } else if (kind === 8) {
    ctx.fillStyle = '#2e2e33';
    ctx.fillRect(sx + 0.9 * u, top - 0.95 * u, 0.5 * u, 0.95 * u);
    ctx.fillStyle = '#ff9a3c';
    ctx.beginPath();
    ctx.arc(sx - 0.8 * u, top + 0.45 * u, 0.2 * u, 0, 0, Math.PI * 2);
    ctx.fill();
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
    ctx.fillStyle = drawOwner === foeOwner() ? ENEMY_COLOR : COLORS[i % COLORS.length];
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
  ctx.fillStyle = drawOwner === foeOwner() ? ENEMY_CLOTH : '#31579e'; // rider torso (player color)
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
  ctx.fillStyle = drawOwner === foeOwner() ? ENEMY_CLOTH : '#31579e'; // tunic (player color)
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
  ctx.fillStyle = drawOwner === foeOwner() ? ENEMY_ARCHER_CLOTH : '#4a6e46'; // tunic (archer green / enemy red)
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

// --- Animal pose art (Base44 sets, see ANIMAL_ART above) -----------
// The renderer gets fresh positions every frame but no velocity, so
// movement and facing are derived per entity from the position
// delta between draws: still = the idle pose (with an occasional
// graze/root/browse pose 4 on a slow per-animal phase), moving =
// walk strides 2/3 alternating on a tick phase. Facing is
// screen-space: in the dimetric projection, moving right on screen
// means the world delta has (dx - dy) > 0. The generated poses face
// left, so rightward movement draws them flipped. Renderer-only
// state; the vector draws below stay as the fallback.
const ANIMAL_POSE_STATE = {};
function animalPoseFor(i, wx, wy) {
  const st = ANIMAL_POSE_STATE[i] ||
    (ANIMAL_POSE_STATE[i] = { x: wx, y: wy, face: -1 });
  const dx = wx - st.x;
  const dy = wy - st.y;
  const moving = Math.abs(dx) + Math.abs(dy) > 0.0001;
  if (moving) {
    const sdx = dx - dy;
    if (sdx > 0.0001) st.face = 1;
    else if (sdx < -0.0001) st.face = -1;
  }
  st.x = wx;
  st.y = wy;
  return { moving, face: st.face };
}
// The pose image for one animal this frame, plus its flip flag.
// kind is the ANIMAL_ART key prefix: 'sheep' | 'boar' | 'deer'.
function animalPoseImage(kind, i, wx, wy) {
  const { moving, face } = animalPoseFor(i, wx, wy);
  const tick = sim.tick_count();
  let n;
  if (moving) n = (Math.floor(tick / 8) + i) % 2 === 0 ? 2 : 3;
  else n = (Math.floor(tick / 240) + i) % 5 === 0 ? 4 : 1;
  return { im: ANIMAL_ART.img[kind + n], flip: face > 0 };
}
// Draw an animal pose bottom-anchored at (sx, sy), hWorld tall,
// flipped horizontally when flip is set. False when not loaded.
function drawAnimalPose(im, sx, sy, hWorld, flip) {
  if (!spriteReady(im)) return false;
  const h = hWorld * camS;
  const w = h * (im.naturalWidth / im.naturalHeight);
  if (flip) {
    ctx.save();
    ctx.translate(sx, 0);
    ctx.scale(-1, 1);
    ctx.drawImage(im, -w / 2, sy - h, w, h);
    ctx.restore();
  } else {
    ctx.drawImage(im, sx - w / 2, sy - h, w, h);
  }
  return true;
}

function drawSheep(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 0.9 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.2);
  const sheepPose = animalPoseImage('sheep', i, wx, wy);
  if (drawAnimalPose(sheepPose.im, sx, sy, 2.0, sheepPose.flip)) {
    // The enemy brand (see the vector draw below) still applies.
    if (drawOwner === foeOwner()) {
      ctx.fillStyle = ENEMY_COLOR;
      ctx.beginPath();
      ctx.arc(sx + 0.05 * u, sy - 1.35 * u, 0.26 * u, 0, Math.PI * 2);
      ctx.fill();
    }
    return;
  }
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
  // M5: a captured sheep wears its owner's mark — a red brand dot
  // for the enemy's herd (our own sheep stay unmarked, as before).
  if (drawOwner === foeOwner()) {
    ctx.fillStyle = ENEMY_COLOR;
    ctx.beginPath();
    ctx.arc(sx + 0.05 * u, sy - 1.35 * u, 0.26 * u, 0, Math.PI * 2);
    ctx.fill();
  }
}

function drawBoar(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 1.25 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.55);
  const boarPose = animalPoseImage('boar', i, wx, wy);
  if (drawAnimalPose(boarPose.im, sx, sy, 2.1, boarPose.flip)) return;
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
  const deerPose = animalPoseImage('deer', i, wx, wy);
  if (drawAnimalPose(deerPose.im, sx, sy, 2.9, deerPose.flip)) return;
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

let lastIdleVil = -1;
let lastIdleMil = -1;
let lastAge = -1; // M6: the last age tag shown in the top bar

function updateTopBar() {
  const [food, wood, gold, stone] = myStock();
  resFood.textContent = Math.floor(food);
  resWood.textContent = Math.floor(wood);
  resGold.textContent = Math.floor(gold);
  resStone.textContent = Math.floor(stone);
  // M6: the age chip — the player's current age, always visible.
  const age = myAge();
  if (age !== lastAge) {
    lastAge = age;
    resAge.textContent = AGE_NAMES[age] || 'Dark Age';
  }
  const pop = myPop();
  const cap = myPopCap();
  resPop.textContent = `${pop} / ${cap}`;
  // Warn tint when housed (reason 3) or simply at the cap.
  resPop.classList.toggle('warn', myTrainBlock() === 3 || pop >= cap);
  // Idle indicators (INF-03): live counts from the sim's idle
  // queries; the buttons light up while anyone is standing free.
  const nVil = myIdleVillagers().length;
  if (nVil !== lastIdleVil) {
    lastIdleVil = nVil;
    idleVilCount.textContent = String(nVil);
    idleVilBtn.classList.toggle('has-idle', nVil > 0);
  }
  const nMil = myIdleMilitary().length;
  if (nMil !== lastIdleMil) {
    lastIdleMil = nMil;
    idleMilCount.textContent = String(nMil);
    idleMilBtn.classList.toggle('has-idle', nMil > 0);
  }
}

/// Run one command-panel button by id — the single implementation
/// behind the panel's delegated click listener AND the M13
/// hotkeys, so a key press and a click can never drift apart.
/// Callers gate on the button not being disabled (the listener
/// checks the element; the hotkey path checks it too).
function runCommandButton(id) {
  if (id === 'train') {
    if (!issue({ t: 'train_vil' })) {
      const reason = myTrainBlock();
      flash(['', 'Not enough food for a villager.',
        'The training queue is full.', 'Housed — build a House first.'][reason] || 'Cannot train right now.');
    }
  } else if (id.startsWith('build-')) {
    const kind = Number(id.slice(6));
    placement = placement && placement.kind === kind ? null : { kind };
    buildPage = null; // M13: entering placement closes the page
  } else if (id === 'research-loom' || id === 'research-feudal') {
    // M6: Town Center research (shared queue with training).
    const tag = id === 'research-loom' ? 0 : 1;
    const ok = issue({ t: 'research_tc', tech: tag });
    if (!ok) {
      const rb = myResearchBlockTc(tag);
      flash(tag === 1 && rb === 4
        ? 'Feudal Age needs 2 completed Dark-Age buildings first (Houses and Farms do not count).'
        : RESEARCH_BLOCK_TITLES[rb] || 'Cannot research right now.');
    }
  } else if (id.startsWith('research-')) {
    // M6: building research — Blacksmith lines + Man-at-Arms.
    const tag = {
      'research-fletching': 2, 'research-scalemail': 3,
      'research-padded': 4, 'research-maa': 5,
    }[id];
    if (tag != null && selectedBuilding && selectedBuilding.type === 'building') {
      if (!issue({ t: 'research_at', b: selectedBuilding.id, tech: tag })) {
        flash(RESEARCH_BLOCK_TITLES[sim.research_block_at(selectedBuilding.id, tag)]
          || 'Cannot research right now.');
      }
    }
  } else if (id === 'train-scout') {
    const ok = selectedBuilding && selectedBuilding.type === 'building'
      ? issue({ t: 'train_at', b: selectedBuilding.id, k: 2 })
      : false;
    if (!ok && selectedBuilding) {
      const reason = sim.building_train_block(selectedBuilding.id, 2);
      flash(['', 'Not enough resources for that unit.',
        'The training queue is full.', 'Housed — build a House first.'][reason] || 'Cannot train right now.');
    }
  } else if (id === 'train-militia' || id === 'train-archer') {
    const kindTag = id === 'train-militia' ? 3 : 4;
    const ok = selectedBuilding && selectedBuilding.type === 'building'
      ? issue({ t: 'train_at', b: selectedBuilding.id, k: kindTag })
      : false;
    if (!ok && selectedBuilding) {
      const reason = sim.building_train_block(selectedBuilding.id, kindTag);
      flash(['', 'Not enough resources for that unit.',
        'The training queue is full.', 'Housed — build a House first.'][reason] || 'Cannot train right now.');
    }
  }
}

function cmdButton({ id, icon, label, costIcon, cost, title, disabled, active, badge, key }) {
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
  if (key) html += `<span class="cmd-key">${key}</span>`;
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
  const wood = myStock()[1];
  const reason = myTrainBlock();
  const queue = myTrainQueueLen();
  const bk = selectedBuilding && selectedBuilding.type === 'building'
    ? buildingList().find((b) => b.id === selectedBuilding.id) || null
    : null;
  // M5: an enemy building/TC selection is information-only — its
  // context renders a hint, never command buttons.
  const foeSel = !!selectedBuilding && selectedBuilding.owner === foeOwner();
  // M6: each production/research building gets its own context.
  const BUILDING_CONTEXT = { 2: 'barracks', 6: 'range', 7: 'stable', 8: 'blacksmith' };
  const context = selUnits.length > 0
    ? (hasVillager ? 'build' : 'units')
    : selectedBuilding
      ? (foeSel ? 'foe'
        : selectedBuilding.type === 'tc' ? 'tc'
        : (bk && BUILDING_CONTEXT[bk.kind]) || 'none')
      : 'none';
  const age = myAge();
  const techMask = myTechMask();
  const blockM = bk && context === 'barracks' ? sim.building_train_block(bk.id, 3) : -1;
  const blockA = bk && context === 'range' ? sim.building_train_block(bk.id, 4) : -1;
  const blockS = bk && context === 'stable' ? sim.building_train_block(bk.id, 2) : -1;
  // Research blocks: the TC pair (Loom 0, Feudal 1) and the
  // building techs (Fletching 2, Scale Mail 3, Padded 4, MAA 5).
  const rbTc = context === 'tc'
    ? [myResearchBlockTc(0), myResearchBlockTc(1)] : [-1, -1];
  const rbBk = bk && (context === 'blacksmith' || context === 'barracks')
    ? [2, 3, 4, 5].map((t) => sim.research_block_at(bk.id, t)) : [];
  const tcQueueCodes = context === 'tc' ? myTcQueue().join('.') : '';
  const bkQueueCodes = bk ? sim.building_queue(bk.id).join('.') : '';
  const sig = [context, Math.floor(wood), Math.floor(myStock()[0]), Math.floor(myStock()[2]),
    reason, queue, bk ? bk.id : -1, bk ? bk.queue : -1, bk && bk.complete ? 1 : 0,
    blockM, blockA, blockS, age, techMask, rbTc.join('.'), rbBk.join('.'),
    tcQueueCodes, bkQueueCodes, buildPage || '',
    placement ? placement.kind : -1].join('|');
  if (sig === cmdSig) return;
  cmdSig = sig;
  cmdGrid.innerHTML = '';
  if (context === 'build') {
    // M6: the full building set. Availability gates mirror the sim:
    // Farms need a completed Mill; the Feudal buildings show locked
    // (with their requirement in the title) until the age advances;
    // the Range and the Stable also need a completed Barracks.
    // M13: with a build page armed (Q / W), the panel shows that
    // page's buildings only, each badged with its hotkey letter.
    const all = buildingList();
    const hasMill = all.some((b) => b.owner === localOwner && b.kind === 4 && b.complete);
    const hasBarracks = all.some((b) => b.owner === localOwner && b.kind === 2 && b.complete);
    const buildBtn = (kind) => {
      let gate = null;
      if (kind === 1 && !hasMill) gate = 'Requires a Mill';
      else if (kind >= 6 && age < 1) gate = 'Requires Feudal Age';
      else if ((kind === 6 || kind === 7) && !hasBarracks) gate = 'Requires a Barracks';
      const lacksWood = wood < BUILD_COST[kind];
      return cmdButton({
        id: `build-${kind}`, icon: BUILD_ICON[kind], label: BUILD_NAME[kind],
        costIcon: 'i-wood', cost: BUILD_COST[kind],
        disabled: !!gate || lacksWood,
        active: !!placement && placement.kind === kind,
        key: badgeForButton({ context, page: buildPage, id: `build-${kind}` }),
        title: gate
          ? `${BUILD_NAME[kind]} — ${gate}.`
          : lacksWood
            ? `${BUILD_NAME[kind]} — needs ${BUILD_COST[kind]} wood`
            : `${BUILD_NAME[kind]} — ${BUILD_COST[kind]} wood`,
      });
    };
    const hint = document.createElement('p');
    hint.className = 'ui-cmd-hint';
    if (buildPage) {
      cmdGrid.append(...PAGE_KINDS[buildPage].map(buildBtn));
      hint.textContent = buildPage === 'eco'
        ? 'Economic buildings — the letter on a button places it. Esc goes back.'
        : 'Military buildings — the letter on a button places it. Esc goes back.';
    } else {
      const buttons = [];
      for (let kind = 0; kind <= 8; kind++) buttons.push(buildBtn(kind));
      cmdGrid.append(...buttons);
      hint.textContent =
        'Hotkeys: Q economic buildings · W military buildings · A drop off resources.';
    }
    cmdGrid.append(hint);
  } else if (context === 'tc') {
    // M6: the Town Center also researches — Loom and the Feudal
    // advancement share its queue with villager training (TEC-03).
    const loomDone = (techMask & 1) !== 0;
    const feudalDone = age >= 1;
    cmdGrid.append(
      cmdButton({
        id: 'train', icon: 'i-train', label: 'Villager',
        costIcon: 'i-food', cost: 50,
        disabled: reason === 2, // queue full; other blocks flash on click
        badge: queue > 0 ? `+${queue}` : null,
        key: badgeForButton({ context, id: 'train', index: 0 }),
        title: TRAIN_BLOCK_TITLES[reason]
          ? `Train Villager — ${TRAIN_BLOCK_TITLES[reason]}`
          : 'Train a villager at the Town Center (50 food).',
      }),
      cmdButton({
        id: 'research-loom', icon: 'i-loom', label: 'Loom',
        costIcon: 'i-gold', cost: 50,
        disabled: loomDone || rbTc[0] === 2 || rbTc[0] === 3,
        key: badgeForButton({ context, id: 'research-loom', index: 1 }),
        title: loomDone
          ? 'Loom — researched. Villagers: +15 HP, +1 melee / +2 pierce armor.'
          : `Research Loom — 50 gold · 25 s. Villagers gain +15 HP and armor.${RESEARCH_BLOCK_TITLES[rbTc[0]] ? ' ' + RESEARCH_BLOCK_TITLES[rbTc[0]] : ''}`,
      }),
      cmdButton({
        id: 'research-feudal', icon: 'i-feudal', label: 'Feudal Age',
        costIcon: 'i-food', cost: 500,
        disabled: feudalDone || rbTc[1] === 2 || rbTc[1] === 3,
        key: badgeForButton({ context, id: 'research-feudal', index: 2 }),
        title: feudalDone
          ? 'Feudal Age — you stand in the Feudal Age.'
          : rbTc[1] === 4
            ? 'Advance to the Feudal Age — needs 2 completed Dark-Age buildings first (Houses and Farms do not count).'
            : `Advance to the Feudal Age — 500 food · 130 s. Unlocks the Archery Range, Stable and Blacksmith.${RESEARCH_BLOCK_TITLES[rbTc[1]] ? ' ' + RESEARCH_BLOCK_TITLES[rbTc[1]] : ''}`,
      }),
    );
  } else if (context === 'barracks') {
    const underConstruction = !bk.complete;
    const maaDone = (techMask & 32) !== 0;
    cmdGrid.append(
      cmdButton({
        id: 'train-militia', icon: 'i-militia', label: 'Militia',
        costIcon: 'i-food', cost: 60,
        disabled: underConstruction || blockM === 2,
        badge: bk.queue > 0 ? `+${bk.queue}` : null,
        key: badgeForButton({ context, id: 'train-militia', index: 0 }),
        title: underConstruction
          ? 'Militia — the Barracks is under construction.'
          : `Train Militia — 60 food · 20 gold.${TRAIN_BLOCK_TITLES[blockM] ? ' ' + TRAIN_BLOCK_TITLES[blockM] : ''}`,
      }),
      cmdButton({
        id: 'research-maa', icon: 'i-maa', label: 'Man-at-Arms',
        costIcon: 'i-food', cost: 100,
        disabled: underConstruction || maaDone || rbBk[3] === 2 || rbBk[3] === 3,
        key: badgeForButton({ context, id: 'research-maa', index: 1 }),
        title: underConstruction
          ? 'Man-at-Arms — the Barracks is under construction.'
          : maaDone
            ? 'Man-at-Arms — researched. Your Militia fight as Men-at-Arms.'
            : `Upgrade to Man-at-Arms — 100 food · 40 gold · 40 s. Every Militia becomes a Man-at-Arms.${RESEARCH_BLOCK_TITLES[rbBk[3]] ? ' ' + RESEARCH_BLOCK_TITLES[rbBk[3]] : ''}`,
      }),
    );
  } else if (context === 'range') {
    const underConstruction = !bk.complete;
    cmdGrid.append(cmdButton({
      id: 'train-archer', icon: 'i-archer', label: 'Archer',
      costIcon: 'i-wood', cost: 25,
      disabled: underConstruction || blockA === 2,
      badge: bk.queue > 0 ? `+${bk.queue}` : null,
      key: badgeForButton({ context, id: 'train-archer', index: 0 }),
      title: underConstruction
        ? 'Archer — the Archery Range is under construction.'
        : `Train Archer — 25 wood · 45 gold.${TRAIN_BLOCK_TITLES[blockA] ? ' ' + TRAIN_BLOCK_TITLES[blockA] : ''}`,
    }));
  } else if (context === 'stable') {
    const underConstruction = !bk.complete;
    cmdGrid.append(cmdButton({
      id: 'train-scout', icon: 'i-villager', label: 'Scout Cavalry',
      costIcon: 'i-food', cost: 80,
      disabled: underConstruction || blockS === 2,
      badge: bk.queue > 0 ? `+${bk.queue}` : null,
      key: badgeForButton({ context, id: 'train-scout', index: 0 }),
      title: underConstruction
        ? 'Scout Cavalry — the Stable is under construction.'
        : `Train Scout Cavalry — 80 food · 30 s.${TRAIN_BLOCK_TITLES[blockS] ? ' ' + TRAIN_BLOCK_TITLES[blockS] : ''}`,
    }));
  } else if (context === 'blacksmith') {
    // M6: the Blacksmith's three Feudal researches.
    const underConstruction = !bk.complete;
    const techBtn = (id, tag, icon, label, title) => {
      const done = (techMask & (1 << tag)) !== 0;
      const rb = rbBk[tag - 2];
      return cmdButton({
        id, icon, label, costIcon: 'i-food', cost: 100,
        key: badgeForButton({ context, id, index: tag - 2 }),
        disabled: underConstruction || done || rb === 2 || rb === 3,
        title: underConstruction
          ? `${label} — the Blacksmith is under construction.`
          : done
            ? `${label} — researched.`
            : `${title}${RESEARCH_BLOCK_TITLES[rb] ? ' ' + RESEARCH_BLOCK_TITLES[rb] : ''}`,
      });
    };
    cmdGrid.append(
      techBtn('research-fletching', 2, 'i-fletching', 'Fletching',
        'Research Fletching — 100 food · 50 gold · 30 s. Archers: +1 attack, +1 range.'),
      techBtn('research-scalemail', 3, 'i-scalemail', 'Scale Mail',
        'Research Scale Mail Armor — 100 food · 40 s. Infantry: +1/+1 armor.'),
      techBtn('research-padded', 4, 'i-padded', 'Padded Armor',
        'Research Padded Archer Armor — 100 food · 40 s. Archers: +1/+1 armor.'),
    );
  } else if (context === 'units') {
    const hint = document.createElement('p');
    hint.className = 'ui-cmd-hint';
    hint.textContent = 'Right-click an animal or enemy to attack, ground to move.';
    cmdGrid.append(hint);
  } else if (context === 'foe') {
    const hint = document.createElement('p');
    hint.className = 'ui-cmd-hint';
    hint.textContent = 'Enemy — information only. Select your own units to give orders.';
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
  const queue = myTrainQueueLen();
  const reason = myTrainBlock();
  const bk = selectedBuilding && selectedBuilding.type === 'building'
    ? buildingList().find((b) => b.id === selectedBuilding.id) || null
    : null;
  // M5: enemy selections get their own info-only contexts.
  const foeSel = !!selectedBuilding && selectedBuilding.owner === foeOwner();
  // M6: the production/research buildings share one info shape.
  const PROD_KINDS = [2, 6, 7, 8];
  const context = selUnits.length > 0 ? 'units'
    : selectedBuilding
      ? (selectedBuilding.type === 'tc'
        ? (foeSel ? 'foe-tc' : 'tc')
        : foeSel ? 'foe-building'
        : bk && PROD_KINDS.includes(bk.kind) ? 'production' : 'building')
      : 'none';
  const singleHp = selUnits.length === 1 ? Math.floor(st.hp[selUnits[0] * 2]) : -1;
  const TRAINS_TAG = { 2: 3, 6: 4, 7: 2 };
  const bkBlock = bk && TRAINS_TAG[bk.kind] != null
    ? sim.building_train_block(bk.id, TRAINS_TAG[bk.kind]) : -1;
  const tcQueueCodes = myTcQueue().join('.');
  const tcProgBucket = Math.floor(myTcProgress() * 20);
  const bkQueueCodes = bk ? sim.building_queue(bk.id).join('.') : '';
  // The selected TC's HP joins the signature (M5): the enemy TC
  // panel must tick down live while it is being razed. Same for a
  // selected building's HP fraction.
  const selTcHp = selectedBuilding && selectedBuilding.type === 'tc'
    ? Math.floor(st.hp[selectedBuilding.id * 2]) : -1;
  const sig = [context, selUnits.join(','), JSON.stringify(selectedBuilding),
    queue, reason, singleHp, bk ? bk.queue : -1, bkBlock,
    bk && bk.complete ? 1 : 0, selTcHp, tcQueueCodes, bkQueueCodes, tcProgBucket,
    myAge(), myTechMask(), (buildingGroup || []).join(','),
    bk ? Math.round(bk.hpFrac * 40) : -1].join('|');
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
    const codes = myTcQueue();
    if (reason === 3) {
      selInfo.textContent = 'Housed — training stalls until a House stands.';
      selInfo.classList.add('warn');
    } else if (codes.length > 0) {
      const head = codes[0];
      selInfo.textContent = head >= 100
        ? `Researching ${queueEntryName(head)}… ${Math.round(myTcProgress() * 100)}% · Queue ${codes.length}`
        : `Training a villager… · Queue ${codes.length}`;
    } else {
      selInfo.textContent = 'Train villagers and research from the Commands panel. Right-click the map to set a rally point.';
    }
  } else if (context === 'production') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML =
      `<svg class="ui-icon"><use href="#${BUILD_ICON[bk.kind]}"/></svg>`;
    // M13: a Ctrl+Shift go-to selects a whole type at once; the
    // name carries the count (commands address the first).
    selName.textContent = buildingGroup && buildingGroup.length > 1
      ? `${BUILD_NAME[bk.kind]} ×${buildingGroup.length}`
      : BUILD_NAME[bk.kind];
    const codes = sim.building_queue(bk.id);
    if (!bk.complete) {
      selInfo.textContent = 'Under construction…';
    } else if (bkBlock === 3) {
      selInfo.textContent = 'Housed — training stalls until a House stands.';
      selInfo.classList.add('warn');
    } else if (codes.length > 0) {
      const head = codes[0];
      selInfo.textContent = head >= 100
        ? `Researching ${queueEntryName(head)}… · Queue ${codes.length}`
        : `Training ${queueEntryName(head)}… · Queue ${codes.length}`;
    } else {
      selInfo.textContent = bk.kind === 8
        ? 'Research technologies from the Commands panel.'
        : 'Train from the Commands panel. Right-click the map to set a rally point.';
    }
  } else if (context === 'foe-tc') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = '<svg class="ui-icon"><use href="#i-tc"/></svg>';
    selName.textContent = 'Enemy Town Center';
    const id = selectedBuilding.id;
    selInfo.textContent =
      `HP ${Math.max(0, Math.ceil(st.hp[id * 2]))} / ${st.hp[id * 2 + 1]} — destroy it, its buildings and its units to win the match.`;
  } else if (context === 'foe-building') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML =
      `<svg class="ui-icon"><use href="#${(bk && BUILD_ICON[bk.kind]) || 'i-house'}"/></svg>`;
    selName.textContent = bk ? `Enemy ${BUILD_NAME[bk.kind]}` : 'Enemy Building';
    selInfo.textContent = bk
      ? `HP ${Math.round(bk.hpFrac * BUILD_MAXHP[bk.kind])} / ${BUILD_MAXHP[bk.kind]}${bk.complete ? '' : ' · under construction'} — information only.`
      : '';
  } else if (context === 'building') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML =
      `<svg class="ui-icon"><use href="#${(bk && BUILD_ICON[bk.kind]) || 'i-house'}"/></svg>`;
    selName.textContent = bk
      ? (buildingGroup && buildingGroup.length > 1
        ? `${BUILD_NAME[bk.kind]} ×${buildingGroup.length}`
        : BUILD_NAME[bk.kind])
      : 'Building';
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
/// The rally flag (SEL-13): a dashed gold line from the building
/// to its rally point, a small flag planted there, and a gold ring
/// when the point snapped to a resource node. Extracted (M8) so
/// the per-seat Town Center rally draws the identical flag.
function drawRallyFlag(from, rx, ry, nodeId) {
  const [fx, fy] = worldToScreen(from[0], from[1]);
  const [tx, ty] = worldToScreen(rx, ry);
  ctx.save();
  ctx.strokeStyle = 'rgba(255, 207, 77, 0.75)';
  ctx.fillStyle = '#ffcf4d';
  ctx.lineWidth = 1.5;
  ctx.setLineDash([5, 4]);
  ctx.beginPath();
  ctx.moveTo(fx, fy);
  ctx.lineTo(tx, ty);
  ctx.stroke();
  ctx.setLineDash([]);
  // The flag: a pole and a triangular pennant at the point.
  const pole = 2.4 * camS;
  ctx.beginPath();
  ctx.moveTo(tx, ty);
  ctx.lineTo(tx, ty - pole);
  ctx.stroke();
  ctx.beginPath();
  ctx.moveTo(tx, ty - pole);
  ctx.lineTo(tx + 1.1 * camS, ty - pole + 0.38 * camS);
  ctx.lineTo(tx, ty - pole + 0.76 * camS);
  ctx.closePath();
  ctx.fill();
  if (nodeId >= 0) {
    ctx.beginPath();
    ctx.arc(tx, ty, 0.9 * camS, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.restore();
}

function draw() {
  const W = canvas.width;
  const H = canvas.height;
  ctx.fillStyle = '#0b0f14';
  ctx.fillRect(0, 0, W, H);

  // M7: fog and obstacles change as the match runs (sight moves,
  // trees fall) — refresh both flat layers every frame, like every
  // other sim read here. Terrain and heights are static per match
  // (refreshWorldLayers).
  FOG = myFog();
  OBST = sim.obstacles();

  // Ground (M7): the generated terrain as diamond tiles — land in
  // the two alternating greens, lightened per elevation level,
  // with a darker step skirt under each raised cell; water in two
  // blues; sand on shorelines. Unexplored cells are the black
  // shroud (VIS-01) — nothing about them is drawn at all.
  const hw = CS * camS;
  const hh = hw / 2;
  for (let gy = 0; gy < GH; gy++) {
    for (let gx = 0; gx < GW; gx++) {
      const fi = gy * GW + gx;
      const wx = -HALF + (gx + 0.5) * CS;
      const wy = -HALF + (gy + 0.5) * CS;
      const [cx, cy] = worldToScreen(wx, wy);
      if (cx < -hw || cx > W + hw || cy < -hh || cy > H + hh) continue;
      if (FOG[fi] === 0) {
        diamond(cx, cy, hw, hh, '#05070a');
        continue;
      }
      const hgt = HEIGHTS[fi];
      let base;
      if (TERRAIN[fi] === 1) base = (gx + gy) % 2 ? '#2a639c' : '#2e6da8';
      else if (SHORE.has(fi)) base = '#b8a06a';
      else base = (gx + gy) % 2 ? '#3f7a37' : '#457f3b';
      if (hgt > 0) {
        // The step: the level below shows as a darker tread.
        diamond(cx, cy + ELEV_LIFT * camS, hw, hh, shade(base, -0.25));
        diamond(cx, cy, hw, hh, shade(base, hgt * 0.09));
      } else {
        diamond(cx, cy, hw, hh, base);
      }
    }
  }
  // Ground grid strokes: a second full-grid stroke pass — pure
  // cosmetics, the first thing LOW drops after shadows. Only
  // currently-visible ground is stroked (fog is information; the
  // stroke grid is not).
  if (Q.groundStrokes) {
    ctx.strokeStyle = 'rgba(0,0,0,0.10)';
    ctx.lineWidth = 1;
    for (let gy = 0; gy < GH; gy++) {
      for (let gx = 0; gx < GW; gx++) {
        if (FOG[gy * GW + gx] !== 2) continue;
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
  // M7 (VIS-06): the buildings this frame may DRAW — ours always;
  // the enemy's live record while visible (refreshing the
  // last-known snapshot), the snapshot while merely explored, and
  // nothing while unexplored. A snapshot whose ground is visible
  // again with no building standing is dropped — re-seeing updates
  // memory, including to "razed".
  const renderBuildings = [];
  for (const b of buildings) {
    if (b.owner !== foeOwner()) {
      renderBuildings.push(b);
      continue;
    }
    const f = fogAtWorld(b.x, b.y);
    if (f === 2) {
      seenBuildings.set(b.id, { ...b });
      renderBuildings.push(b);
    } else if (f === 1) {
      const snap = seenBuildings.get(b.id);
      if (snap) renderBuildings.push(snap);
    }
  }
  for (const [id, snap] of seenBuildings) {
    if (fogAtWorld(snap.x, snap.y) === 2 && !buildings.some((b) => b.id === id)) {
      seenBuildings.delete(id);
    }
  }
  const st = entityState();
  const kinds = st.kinds;
  const pos = st.pos;
  const count = st.count;

  // The carry heuristic's deposit point is OUR Town Center (M5:
  // with two TCs on the map, the first owner-0 one — never the
  // enemy's, or our haulers would "deposit" by walking past it).
  let tcPos = null;
  for (let i = 0; i < count; i++) {
    if (kinds[i] === KIND_TC && st.owners[i] === localOwner) {
      tcPos = [pos[i * 2], pos[i * 2 + 1]];
      break;
    }
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
  // (M7: the old arena's stone blocks are gone — obstacles are now
  // terrain: water renders in the ground pass, trees as nodes.)
  for (const n of nodes) {
    // Fog (VIS-01/06): a node draws only on explored ground. A
    // depleted node still draws while its ground is not currently
    // visible (memory says it stands); once re-seen empty it
    // vanishes for good. Amount bars are live information — only
    // in visible cells.
    const f = fogAtWorld(n.x, n.y);
    if (f === 0) continue;
    if (n.amount <= 0 && f === 2) continue;
    const known = n.amount > 0 ? n.amount / NODE_MAX[n.kind] : 1;
    draws.push({
      key: n.x + n.y,
      fn: () => {
        const [sx, sy] = worldToScreen(n.x, n.y);
        NODE_DRAW[n.kind](sx, sy, known, n.x, n.y);
        if (f === 2) amountBar(sx, sy, n.amount / NODE_MAX[n.kind]);
      },
    });
  }
  for (const b of renderBuildings) {
    if (b.kind === 1 && b.complete) continue; // drawn as a farm node
    draws.push({ key: b.x + b.y + CS, fn: () => drawBuilding(b) });
  }
  // Unit draws dispatch by kind (M3); the dead are skipped (they
  // are tombstones in the sim, invisible here). A damaged unit gets
  // an HP bar above it.
  const UNIT_DRAW = [drawVillager, null, drawScout, drawMilitia, drawArcher,
    drawSheep, drawBoar, drawDeer, drawMilitia]; // 8: Man-at-Arms (M6) — the Militia draw for now
  for (let i = 0; i < count; i++) {
    const wx = pos[i * 2];
    const wy = pos[i * 2 + 1];
    if (kinds[i] === KIND_TC) {
      // Sort by the TC's base/front edge so units in front overlap it.
      const owner = st.owners[i];
      // M7 (VIS-06): the enemy Town Center is a building for
      // knowledge — drawn once its ground is explored, but its HP
      // is live information, shown only while visible.
      const f = owner === localOwner ? 2 : fogAtWorld(wx, wy);
      if (f === 0) continue;
      const hp = st.hp[i * 2];
      const maxHp = st.hp[i * 2 + 1];
      draws.push({
        key: wx + wy + TC_HALF,
        fn: () => {
          drawTC(wx, wy, owner);
          // M5: a Town Center under attack shows its HP like any
          // unit — razing it is how matches end.
          if (f === 2 && hp < maxHp) {
            const [sx, sy] = worldToScreen(wx, wy);
            hpBar(sx, sy - 5.4 * camS, hp / maxHp);
          }
        },
      });
    } else if (st.hp[i * 2] >= 1) {
      // M7 (VIS-01): enemy and Gaia UNITS draw only in currently
      // visible cells — explored memory remembers buildings, never
      // units (VIS-06).
      if (st.owners[i] !== localOwner && !cellVisibleW(wx, wy)) continue;
      const drawFn = UNIT_DRAW[kinds[i]] || drawVillager;
      const hp = st.hp[i * 2];
      const maxHp = st.hp[i * 2 + 1];
      const owner = st.owners[i];
      draws.push({
        key: wx + wy,
        fn: () => {
          drawOwner = owner; // player color for this draw (M5)
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
      // M7 (VIS-01): an arrow is a unit for knowledge — drawn only
      // while its cell is currently visible.
      if (!cellVisibleW(wx, wy)) continue;
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

  // Fog dim (M7, VIS-01): explored-but-not-visible ground draws
  // under a dim veil — terrain, nodes and last-known buildings
  // were all painted at full brightness above, and this pass is
  // what makes memory read as memory. (Unexplored cells were never
  // painted — the ground pass left them black.)
  for (let gy = 0; gy < GH; gy++) {
    for (let gx = 0; gx < GW; gx++) {
      if (FOG[gy * GW + gx] !== 1) continue;
      const [cx, cy] = worldToScreen(-HALF + (gx + 0.5) * CS, -HALF + (gy + 0.5) * CS);
      if (cx < -hw || cx > W + hw || cy < -hh || cy > H + hh) continue;
      diamond(cx, cy, hw, hh, 'rgba(6, 8, 12, 0.48)');
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
      const b = renderBuildings.find((bb) => bb.id === selectedBuilding.id);
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

  // Rally flag (SEL-13): when the Town Center or a Barracks is
  // selected and holds a rally point, a dashed gold line runs from
  // the building to the point, with a small flag planted there. A
  // rally that snapped to a resource node shows a gold ring on the
  // node as well. Only the selected building's flag shows. M5:
  // never for an enemy selection — the opponent's rally is its own
  // business, and entry 0 is always OUR Town Center's rally.
  if (selectedBuilding && selectedBuilding.owner !== foeOwner()) {
    const rallies = sim.rally_points();
    let entry = -1;
    let from = null;
    if (selectedBuilding.type === 'tc' && seatAware()) {
      // M8: the per-seat TC rally, shaped like one rally_points entry.
      const r = sim.tc_rally_of(selectedBuilding.id);
      entry = -2;
      from = [pos[selectedBuilding.id * 2], pos[selectedBuilding.id * 2 + 1]];
      if (r[3] > 0.5) {
        drawRallyFlag(from, r[0], r[1], r[2]);
      }
    } else if (selectedBuilding.type === 'tc') {
      entry = 0;
      from = [pos[selectedBuilding.id * 2], pos[selectedBuilding.id * 2 + 1]];
    } else if (selectedBuilding.type === 'building') {
      const bi = buildings.findIndex((b) => b.id === selectedBuilding.id);
      if (bi >= 0) {
        entry = 1 + bi;
        from = [buildings[bi].x, buildings[bi].y];
      }
    }
    if (entry >= 0 && rallies[entry * 4 + 3] > 0.5) {
      drawRallyFlag(from, rallies[entry * 4], rallies[entry * 4 + 1], rallies[entry * 4 + 2]);
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
      // M7 (VIS-10): the minimap obeys the same fog as the main
      // view — our units always; the enemy Town Center once
      // explored; every other non-own unit only while visible.
      const ex = pos[i * 2], ey = pos[i * 2 + 1];
      if (st.owners[i] !== localOwner) {
        const f = fogAtWorld(ex, ey);
        if (kinds[i] === KIND_TC ? f < 1 : f !== 2) continue;
      }
      mmEntities.push({
        x: ex, y: ey, kind: kinds[i],
        // M5: three minimap families — Gaia's own colors, our blue,
        // the enemy's red.
        color: st.owners[i] === 1
          ? (GAIA_COLORS[kinds[i]] || '#9a9a9a')
          : st.owners[i] === foeOwner() ? ENEMY_COLOR : OWN_COLOR,
        selected: selection.has(i),
      });
    }
    renderMinimap(mmCtx, mmGeom, {
      gridW: GW, gridH: GH, cellSize: CS, obstacles: OBST,
      // M7: terrain + heights + fog — the minimap's ground IS the
      // world as the local player knows it (VIS-10).
      terrain: TERRAIN, heights: HEIGHTS, fog: FOG,
      // M7: nodes plot only on explored ground.
      nodes: nodes.filter((n) => cellExploredW(n.x, n.y)),
      // M5: enemy buildings plot in enemy red. M6: our buildings
      // plot in their per-kind colors (camps/range/stable/smith);
      // House/Farm/Barracks keep the parchment default. M7: the
      // enemy's entries are the fog-filtered render list
      // (last-known snapshots included).
      buildings: renderBuildings.map((b) => (b.owner === foeOwner()
        ? { ...b, color: ENEMY_COLOR }
        : { ...b, color: BUILDING_MINIMAP_COLOR[b.kind] })),
      entities: mmEntities,
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
