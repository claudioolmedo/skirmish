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
// — the letter IS the button's place in the physical keyboard
// grid (rows Q W E / A S D / Z X C, so Q, Q raises a House) —
// slot letters on a selected building's panel, A for force
// drop-off, Ctrl+letter go-tos — and edge scrolling, the camera
// pan every other method shares the frame with.
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
  GAME_SPEED, TICK_MS, advanceTicks,
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
import { diffFeedEvents } from './play/feed.js';
import { createAudioEngine, cuesForSnapshotDiff } from './play/audio.js';
import { ClickMarkers, markerHeads } from './play/clickmarker.js';
import { pennantsForSelection, pennantShape } from './play/pennants.js';
import {
  t, getLocale, setLocale, costText, genderOf,
} from './play/i18n.js';

const assetUrl = (rel) => new URL(rel, import.meta.url).href;

// --- Generated audio (2026-10-20, Claudio's request) -------------
// The engine (play/audio.js) synthesizes every cue with the Web
// Audio API — no files, no samples. It is fed ONLY from UI
// actions below and from own-seat state transitions (the event
// feed's snapshots, HP/idle/housed diffs in the top-bar update):
// never from the sim's internals, and it never writes back —
// lockstep determinism is untouched. The context stays locked
// until the first user gesture (see the unlock listeners in
// main()); cues asked for before that are dropped by design.
const gameAudio = createAudioEngine();
function playAudioCue(cue) {
  try { return gameAudio.play(cue); } catch { return false; }
}

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
// Fullscreen toggle (Claudio, 2026-10-19): a top-bar button and
// a pause-menu entry, both driving the one toggleFullscreen().
const fullscreenBtn = document.getElementById('fullscreen-btn');
const pauseFullscreenBtn = document.getElementById('pause-fullscreen-btn');
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
const feedEl = document.getElementById('event-feed');
const minimapCanvas = document.getElementById('minimap');
const mmCtx = minimapCanvas.getContext('2d');
const toast = document.getElementById('toast');
const pauseMenu = document.getElementById('pause-menu');
// Generated-audio controls (pause menu): the on/off toggle and
// the volume slider; wired in main(), voiced by applyStaticStrings.
const audioEnabledInput = document.getElementById('audio-enabled');
const audioVolumeInput = document.getElementById('audio-volume');
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
// End-of-match feedback block (2026-10-19): refs + state. The
// send goes to the relay's /feedback; once it succeeds the form
// never shows again (localStorage skirmish2.feedbackDone) — the
// GitHub star line stays. The ✕ hides the block for the rest of
// the page session only.
const feedbackBlock = document.getElementById('end-feedback');
const feedbackForm = document.getElementById('feedback-form');
const feedbackQuestion = document.getElementById('feedback-q');
const feedbackStarsEl = document.getElementById('feedback-stars');
const feedbackText = document.getElementById('feedback-text');
const feedbackError = document.getElementById('feedback-error');
const feedbackThanks = document.getElementById('feedback-thanks');
const feedbackSendBtn = document.getElementById('feedback-send');
const feedbackCloseBtn = document.getElementById('feedback-close');
const feedbackStarBtns = [1, 2, 3, 4, 5]
  .map((n) => document.getElementById(`feedback-star-${n}`));
let feedbackStars = 0; // 0 = no rating chosen
let feedbackCtx = null; // { result, durationTicks } of the ended match
let feedbackDismissed = false; // session-only (the ✕)

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
// i18n: the display-name tables below are rebuilt from
// play/i18n.js on every locale switch (refreshNames) — the
// semantic ids index the string table (unit.<id>, building.<id>).
const KIND_I18N = ['villager', 'townCenter', 'scout', 'militia', 'archer',
  'sheep', 'boar', 'deer', 'manAtArms'];
let KIND_NAME = {
  0: 'Villager', 1: 'Town Center', 2: 'Scout Cavalry', 3: 'Militia',
  4: 'Archer', 5: 'Sheep', 6: 'Boar', 7: 'Deer', 8: 'Man-at-Arms',
};
// Selection-panel portraits per unit kind (SVG symbol ids in
// play/index.html).
const KIND_ICON = {
  0: 'i-villager', 2: 'i-villager', 3: 'i-militia', 4: 'i-archer', 5: 'i-pop',
  8: 'i-militia',
};

// Original PNG icon set (2026-10-19, AI generations via Base44 on
// Claudio's account — provenance in play/ui-art/README.md):
// full-bleed 128px squares with their own dark walnut background
// and thin gold frame, drawn in the reference game's icon
// language (a single large, clear pictogram per button) without
// copying any of its icons. Where a command button or selection
// portrait has a PNG below it replaces the older SVG symbol; the
// symbols remain for everything unmapped (resources, animals).
// Kind tags here match KIND_NAME / the sim's entity kinds.
const KIND_PNG = {
  0: 'villager.png', 2: 'scout.png', 3: 'militia.png', 4: 'archer.png',
  8: 'man-at-arms.png',
};
// An <img> glyph occupying the same slot (and classes) as the
// SVG icons, so every sizing rule applies unchanged.
function pngIcon(png, cls) {
  return `<img class="${cls}" src="ui-art/icons/${png}" alt="">`;
}

// Building kind tags (must match the sim's BuildingKind::tag):
// 0 house, 1 farm, 2 barracks (M2.5/M3); 3 lumber camp, 4 mill,
// 5 mining camp, 6 archery range, 7 stable, 8 blacksmith (M6).
// Costs and footprints mirror the sim (MECHANICS-STUDY §5).
const BUILD_COST = [25, 60, 175, 100, 100, 100, 175, 175, 150];
const BUILD_I18N = ['house', 'farm', 'barracks', 'lumber', 'mill',
  'mining', 'range', 'stable', 'blacksmith'];
let BUILD_NAME = ['House', 'Farm', 'Barracks', 'Lumber Camp', 'Mill',
  'Mining Camp', 'Archery Range', 'Stable', 'Blacksmith'];
const BUILD_SIZE = [2, 2, 3, 2, 2, 2, 3, 3, 3];
const BUILD_ICON = ['i-house', 'i-farm', 'i-tc', 'i-lumber', 'i-mill',
  'i-mining', 'i-range', 'i-stable', 'i-blacksmith'];
// The PNG set above, per building kind tag (same order as
// BUILD_ICON): command buttons and building portraits.
const BUILD_PNG = ['house.png', 'farm.png', 'barracks.png', 'lumber-camp.png',
  'mill.png', 'mining-camp.png', 'archery-range.png', 'stable.png',
  'blacksmith.png'];
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
let AGE_NAMES = ['Dark Age', 'Feudal Age'];
let TECH_NAME = {
  0: 'Loom', 1: 'Feudal Age', 2: 'Fletching', 3: 'Scale Mail Armor',
  4: 'Padded Archer Armor', 5: 'Man-at-Arms',
};
// Research-block explanations, indexed by the sim's ResearchBlock
// tag (0 ok — unused, 1 funds, 2 queue full, 3 already done,
// 4 requirements).
let RESEARCH_BLOCK_TITLES = [
  '',
  'Not enough resources.',
  'The queue is full.',
  'Already researched.',
  'Requirements not met yet.',
];
// i18n: rebuild every display-name table from the string table
// in the active locale. Called once at load and on each switch.
function refreshNames() {
  const kn = {};
  KIND_I18N.forEach((id, tag) => { kn[tag] = t('unit.' + id); });
  KIND_NAME = kn;
  BUILD_NAME = BUILD_I18N.map((id) => t('building.' + id));
  AGE_NAMES = [t('age.dark'), t('age.feudal')];
  TECH_NAME = {
    0: t('tech.loom'), 1: t('age.feudal'), 2: t('tech.fletching'),
    3: t('tech.scalemail'), 4: t('tech.padded'), 5: t('tech.maa'),
  };
  RESEARCH_BLOCK_TITLES = [
    '', t('block.noResources'), t('block.queueFullTrain'),
    t('block.alreadyDone'), t('block.requirements'),
  ];
  TRAIN_BLOCK_TITLES = [
    '', t('block.noResources'), t('block.queueFull'), t('block.housedLong'),
  ];
}
// (First refreshNames() runs at the top of main() — the tables'
// later declarations must be initialized first.)
// A queue entry's display name: unit codes are entity kind tags;
// research codes are 100 + tech tag.
const queueEntryName = (code) => (code < 100
  ? (KIND_NAME[code] || t('unit.fallback'))
  : (TECH_NAME[code - 100] || t('research.fallback')));

// --- Game shell state (client-only; never sim input) -------------------
let playing = false; // the match is live: the sim ticks (set at init — no start screen)
let paused = false; // pause menu open: the sim holds
let selection = new Set(); // selected unit ids — orders go ONLY to these (AoE2-style)
// The selected building, if any: { type: 'tc', id } for the Town
// Center (an entity) or { type: 'building', id, kind } for a placed
// building (House/Farm/Barracks). Null when units are selected.
let selectedBuilding = null;
// The info-selected animal, if any: the entity id of a living
// animal that is NOT ours (wild Gaia sheep/boar/deer, or the
// enemy's sheep). Information only — it takes no orders; the
// selection panel shows its portrait and hit points, as the
// reference does for an animal you click but do not own.
// Mutually exclusive with the unit/building selections.
let selectedAnimal = null;

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
  selectedAnimal = null;
  buildPage = null; // M13: a selection change closes any build page
  buildingGroup = null;
}
function selectBuilding(descOrNull) {
  selectedBuilding = descOrNull;
  selection = new Set();
  selectedAnimal = null;
  buildPage = null;
  buildingGroup = null;
}
function selectAnimal(id) {
  selectedAnimal = id;
  selection = new Set();
  selectedBuilding = null;
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
  endTitle.textContent = ms === 1 ? t('end.victory') : t('end.defeat');
  endDetail.textContent = ms === 1
    ? t('end.detailVictory', { time: `${mm}:${ss}` })
    : t('end.detailDefeat', { time: `${mm}:${ss}` });
  feedbackCtx = {
    result: ms === 1 ? 'victory' : 'defeat',
    durationTicks: sim.tick_count(),
  };
  playAudioCue(ms === 1 ? 'victory' : 'defeat');
  renderEndFeedback();
  endOverlay.style.display = 'flex';
}

/// Paint the feedback block for a freshly ended match: hidden
/// for the session after the ✕; the bare GitHub star line once
/// feedback was sent in a past session; otherwise the form, with
/// any unsent text kept (a failed send must never eat it).
function renderEndFeedback() {
  if (!feedbackBlock) return;
  feedbackBlock.style.display = feedbackDismissed ? 'none' : '';
  let done = false;
  try {
    done = localStorage.getItem('skirmish2.feedbackDone') === '1';
  } catch { /* storage unavailable — treat as not sent */ }
  if (feedbackForm) feedbackForm.style.display = done ? 'none' : '';
  if (!done) {
    // Undo a previous thanks state (reachable when the send
    // succeeded but localStorage refused the flag).
    if (feedbackQuestion) feedbackQuestion.hidden = false;
    if (feedbackStarsEl) feedbackStarsEl.hidden = false;
    if (feedbackText) feedbackText.hidden = false;
    if (feedbackSendBtn) feedbackSendBtn.hidden = false;
  }
  if (feedbackError) feedbackError.hidden = true;
  if (feedbackThanks) feedbackThanks.hidden = true;
  if (feedbackSendBtn) feedbackSendBtn.disabled = false;
  paintFeedbackStars();
}

function paintFeedbackStars() {
  feedbackStarBtns.forEach((btn, i) => {
    if (!btn) return;
    const on = i < feedbackStars;
    btn.textContent = on ? '★' : '☆';
    btn.classList.toggle('lit', on);
  });
}

function setFeedbackStars(n) {
  feedbackStars = n;
  paintFeedbackStars();
}

/// POST the feedback to the relay. Fire-and-forget by design:
/// success swaps the form for a thanks line and retires it for
/// good; failure keeps the text and says so quietly. The end
/// card's own buttons never wait on this.
async function sendFeedback() {
  if (!feedbackCtx || feedbackSendBtn.disabled) return;
  feedbackSendBtn.disabled = true;
  if (feedbackError) feedbackError.hidden = true;
  const payload = {
    stars: feedbackStars > 0 ? feedbackStars : null,
    text: feedbackText ? feedbackText.value.slice(0, 1000) : '',
    result: feedbackCtx.result,
    durationTicks: feedbackCtx.durationTicks,
    buildVersion: BUILD_VERSION,
  };
  try {
    const resp = await fetch(RELAY_HTTP + '/feedback', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    if (!resp || !resp.ok) throw new Error('rejected');
    try {
      localStorage.setItem('skirmish2.feedbackDone', '1');
    } catch { /* the thanks state below still applies */ }
    // The thanks state: the form's inputs retire, the line stays.
    if (feedbackQuestion) feedbackQuestion.hidden = true;
    if (feedbackStarsEl) feedbackStarsEl.hidden = true;
    if (feedbackText) feedbackText.hidden = true;
    if (feedbackSendBtn) feedbackSendBtn.hidden = true;
    if (feedbackThanks) feedbackThanks.hidden = false;
  } catch {
    if (feedbackError) {
      feedbackError.textContent = t('feedback.error');
      feedbackError.hidden = false;
    }
    if (feedbackSendBtn) feedbackSendBtn.disabled = false;
  }
}
let lDrag = null; // left-button drag state, canvas px: { x0, y0, x1, y1, moved }
let placement = null; // { kind } while placing a building
let repairArm = false; // Repair targeting armed (villager root panel, E)
// M13 hotkey state (client-only, like placement): the armed
// villager build page ('eco' | 'mil' | null), the go-to cycle
// memory (building kind -> last id handed out), and the building
// id list when a Ctrl+Shift go-to selected a whole type at once
// (selectedBuilding stays the primary; the panel addresses it).
let buildPage = null;
let gotoCycleMem = {};
let buildingGroup = null;
// M13 edge scrolling: the last pointer position in VIEWPORT
// client px, tracked on the window whatever element the pointer
// rides over (the 2026-10-19 fix: canvas-only tracking meant the
// top resource bar swallowed the top edge band — the pointer over
// the bar never reached the canvas listener, so the top edge
// never panned). Fed by the pointer listeners in main(); consumed
// by applyEdgeScroll() on the frame loop.
let mouseClientPt = null;
let mouseWorld = null; // last cursor position, world coords
let flashText = '';
let flashUntil = 0;

function flash(msg) {
  flashText = msg;
  flashUntil = performance.now() + 2600;
}

// --- Event notification feed (2026-10-19 study fix, gap 3) --------
// DE's top-left event log ("House Built", "Villager Created",
// "Sheep Found", "Loom Research Complete"...): how a player
// tracks the game while looking somewhere else. Own-player
// events only, detected by diffing snapshots of the state the
// client already reads every frame for its own seat (completed
// buildings, living units by kind, researched techs, age, owned
// sheep) — nothing here observes the opponent, so the feed is
// fog-fair by construction. The diff -> message mapping is the
// pure play/feed.js; this layer owns the snapshot and the DOM.
// No audio (that is a separate, later gap).

const FEED_MAX_LINES = 5;
let feedPrev = null; // previous snapshot; null until a match seeds it

function ownFeedSnapshot() {
  const built = {};
  for (const b of buildingList()) {
    if (b.owner === localOwner && b.complete) built[b.id] = b.kind;
  }
  const st = entityState();
  const units = {};
  let sheep = 0;
  for (let i = 0; i < st.count; i++) {
    if (st.owners[i] !== localOwner || st.hp[i * 2] < 1) continue;
    const k = st.kinds[i];
    if (k === KIND_SHEEP) sheep++;
    else if (k !== KIND_TC) units[k] = (units[k] || 0) + 1;
  }
  const mask = myTechMask();
  return {
    built, units, sheep,
    age: myAge(),
    techs: {
      loom: (mask & 1) !== 0,
      fletching: (mask & 4) !== 0,
      scalemail: (mask & 8) !== 0,
      padded: (mask & 16) !== 0,
      maa: (mask & 32) !== 0,
    },
  };
}

function pushFeedLine(text) {
  if (!feedEl) return;
  const line = document.createElement('div');
  line.className = 'feed-line';
  line.textContent = text;
  feedEl.prepend(line); // newest on top
  while (feedEl.children.length > FEED_MAX_LINES) {
    feedEl.removeChild(feedEl.children[feedEl.children.length - 1]);
  }
  const fade = setTimeout(() => line.classList.add('fade'), 4000);
  const drop = setTimeout(() => line.remove(), 5000);
  if (fade.unref) fade.unref();
  if (drop.unref) drop.unref();
}

/// The feed's localized formatters (play/feed.js takes them as
/// an argument; its own defaults are the English originals). The
/// participles agree with the entity's gender in the active
/// locale (pt 'Casa construída', es 'Milicia creada').
function feedFmt() {
  const loc = getLocale();
  return {
    built: (name, kind) => t('feed.built.' + genderOf(loc, 'building', BUILD_I18N[kind]), { name }),
    created: (name, kind) => t('feed.created.' + genderOf(loc, 'unit', KIND_I18N[kind]), { name }),
    research: (name) => t('feed.research', { name }),
    feudal: t('feed.feudal'),
    sheep: t('feed.sheep'),
  };
}

/// Diff the current own-player state against the last frame's
/// and post any events. Called once per frame from the live
/// branches of the frame loop (never during replay playback:
/// the feed reports the match being played, not watched).
function updateEventFeed() {
  if (!sim || playback) return;
  const snap = ownFeedSnapshot();
  if (!feedPrev) { feedPrev = snap; return; }
  const prev = feedPrev;
  const lines = diffFeedEvents(prev, snap, {
    buildingName: (k) => BUILD_NAME[k],
    unitName: (k) => KIND_NAME[k],
    techName: (key) => TECH_NAME[{ loom: 0, feudal: 1, fletching: 2, scalemail: 3, padded: 4, maa: 5 }[key]],
  }, feedFmt());
  feedPrev = snap;
  for (const text of lines) pushFeedLine(text);
  // Generated audio: the same own-seat transition the feed just
  // voiced earns its cue (construction complete, a villager
  // trained, research complete, the age fanfare). The mapping
  // is play/audio.js's pure cuesForSnapshotDiff; the engine's
  // gate throttles repeats.
  for (const cue of cuesForSnapshotDiff(prev, snap)) playAudioCue(cue);
}

/// Per-match reset (resetSim / adoptSim): the next snapshot
/// seeds silently, and stale lines from the last match go.
function resetEventFeed() {
  feedPrev = null;
  if (feedEl) feedEl.innerHTML = '';
}

/// Per-match reset of the audio world-state diffs (the HP map
/// behind the under-attack cue, the housed edge, the idle-count
/// edge): the next updateTopBar seeds them silently, so a fresh
/// match never "takes damage" from the last one's numbers.
function resetAudioWorldState() {
  audioPrevHp = null;
  audioHousedPrev = false;
  lastIdleVil = -1;
  lastIdleMil = -1;
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
  tree: 'tree.png', treeDmg1: 'tree-dmg1.png', treeDmg2: 'tree-dmg2.png',
  berry: 'berry-bush.png', gold: 'gold.png', stone: 'stone.png',
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
// --- Original unit animation art (generated sets, 2026-10-19) -------
// Side-view action frames per unit (provenance:
// assets-original/units/README.md), drawn with the ANIMAL_ART model:
// one action strip per verb, frames sliced onto a shared
// bottom-aligned canvas per unit, poses face LEFT and the renderer
// flips them horizontally when the unit moves right on screen.
// This is side-view cycles + flip, NOT 8-direction sheets. The state
// -> action mapping lives in unitArtImage() (beside the animal pose
// code below); every state without art falls back to the previous
// drawing (procedural sheets / vector), so no state regresses.
// Player color: every human unit's cloth is the player blue, and
// the enemy draws a red palette-swap variant built at load with
// makeRedSheet — the same swap the villager sheets use (M5).
const UNIT_ART = { img: {}, imgRed: {}, settled: {}, loaded: false, settledPromise: null };
const UNIT_ART_FILES = {
  villagerWalk1: 'villager/walk-1.png', villagerWalk2: 'villager/walk-2.png',
  villagerWalk3: 'villager/walk-3.png', villagerWalk4: 'villager/walk-4.png',
  villagerGather1: 'villager/gather-1.png', villagerGather2: 'villager/gather-2.png',
  villagerGather3: 'villager/gather-3.png',
  villagerCarry1: 'villager/carry-1.png', villagerCarry2: 'villager/carry-2.png',
  villagerBuild1: 'villager/build-1.png', villagerBuild2: 'villager/build-2.png',
  villagerIdle1: 'villager/idle-1.png',
  archerWalk1: 'archer/walk-1.png', archerWalk2: 'archer/walk-2.png',
  archerWalk3: 'archer/walk-3.png', archerWalk4: 'archer/walk-4.png',
  archerShoot1: 'archer/shoot-1.png', archerShoot2: 'archer/shoot-2.png',
  archerShoot3: 'archer/shoot-3.png',
  archerIdle1: 'archer/idle-1.png',
  militiaWalk1: 'militia/walk-1.png', militiaWalk2: 'militia/walk-2.png',
  militiaWalk3: 'militia/walk-3.png', militiaWalk4: 'militia/walk-4.png',
  militiaAttack1: 'militia/attack-1.png', militiaAttack2: 'militia/attack-2.png',
  militiaAttack3: 'militia/attack-3.png',
  militiaIdle1: 'militia/idle-1.png',
  maaWalk1: 'maa/walk-1.png', maaWalk2: 'maa/walk-2.png',
  maaWalk3: 'maa/walk-3.png', maaWalk4: 'maa/walk-4.png',
  maaAttack1: 'maa/attack-1.png', maaAttack2: 'maa/attack-2.png',
  maaAttack3: 'maa/attack-3.png',
  maaIdle1: 'maa/idle-1.png',
  scoutGallop1: 'scout/gallop-1.png', scoutGallop2: 'scout/gallop-2.png',
  scoutGallop3: 'scout/gallop-3.png', scoutGallop4: 'scout/gallop-4.png',
  scoutIdle1: 'scout/idle-1.png',
};
function loadUnitArt() {
  const p = loadArtSet(UNIT_ART, UNIT_ART_FILES, 'assets-original/units/');
  if (!UNIT_ART.redHooked) {
    UNIT_ART.redHooked = true;
    for (const [key, im] of Object.entries(UNIT_ART.img)) {
      const build = () => { UNIT_ART.imgRed[key] = makeRedSheet(im); };
      if (im.complete && im.naturalWidth > 0) build();
      else im.addEventListener('load', build);
    }
  }
  return p;
}
function whenUnitArtSettled() {
  return UNIT_ART.settledPromise || Promise.resolve();
}

// --- Directional sprite sheets (ours — sprite pipeline, §10) --------
// The directional milestone: sheets rendered from the procedural
// models in tools/sprite-pipeline/models/ — rows = 8 facing
// directions (row 0 faces +X world, increasing toward +Y, the same
// quantization updateVilAnim uses), columns = frames, cell/anchor
// per unit below, drawn at the pipeline's 1.35 readability scale.
// These sheets sit ABOVE the generated side-view sets (UNIT_ART /
// ANIMAL_ART) in every draw chain; the side-view sets remain the
// fallback beneath them. Player color: sheets with a -pcmask twin
// (mask gray = the posterized shade of the player-color pixels)
// get a true palette-remapped red variant at load (buildMaskRed) —
// the reference engine's palette-remap mechanism, with PNGs.
const DIR_ART_SPEC = {
  archer: { states: ['idle', 'walk', 'shoot'], cell: 128, ax: 64, ay: 110, mask: true },
  militia: { states: ['idle', 'walk', 'attack'], cell: 128, ax: 64, ay: 110, mask: true },
  maa: { states: ['idle', 'walk', 'attack'], cell: 128, ax: 64, ay: 110, mask: true },
  scout: { states: ['idle', 'gallop'], cell: 160, ax: 80, ay: 138, mask: true },
  sheep: { states: ['idle', 'walk', 'graze'], cell: 128, ax: 64, ay: 110, mask: false },
  boar: { states: ['idle', 'walk', 'graze'], cell: 128, ax: 64, ay: 110, mask: false },
  deer: { states: ['idle', 'walk', 'graze'], cell: 128, ax: 64, ay: 110, mask: false },
};
const DIR_ART = { img: {}, mask: {}, red: {}, settled: {}, loaded: false, settledPromise: null };
const DIR_PC_RED = [0xd2, 0x34, 0x2c]; // red player's base (blue #2e6fd8 is native)

// The red variant of a sheet from its player-color mask: masked
// pixels become the red base at the mask's shade; everything else
// copies the color sheet untouched. Null when shapes disagree.
function buildMaskRed(img, mask) {
  const w = img.naturalWidth, h = img.naturalHeight;
  if (!w || !mask || mask.naturalWidth !== w || mask.naturalHeight !== h) return null;
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  const g = c.getContext('2d');
  g.drawImage(img, 0, 0);
  const mc = document.createElement('canvas');
  mc.width = w; mc.height = h;
  const mg = mc.getContext('2d');
  mg.drawImage(mask, 0, 0);
  const id = g.getImageData(0, 0, w, h);
  const md = mg.getImageData(0, 0, w, h);
  const d = id.data, m = md.data;
  for (let p = 0; p < d.length; p += 4) {
    const s = m[p];
    if (s > 8 && m[p + 3] > 0) {
      d[p] = (DIR_PC_RED[0] * s) / 255;
      d[p + 1] = (DIR_PC_RED[1] * s) / 255;
      d[p + 2] = (DIR_PC_RED[2] * s) / 255;
    }
  }
  g.putImageData(id, 0, 0);
  return c;
}

function loadDirArt() {
  if (DIR_ART.loaded) return DIR_ART.settledPromise;
  DIR_ART.loaded = true;
  const pending = [];
  const track = (store, key, file) => {
    const im = new Image();
    store[key] = im;
    DIR_ART.settled[key] = false;
    pending.push(new Promise((resolve) => {
      const done = () => { DIR_ART.settled[key] = true; resolve(); };
      im.addEventListener('load', done);
      im.addEventListener('error', done);
      im.src = assetUrl('assets-original/' + file);
      if (im.complete) done();
    }));
    return im;
  };
  for (const [unit, spec] of Object.entries(DIR_ART_SPEC)) {
    for (const state of spec.states) {
      const key = `${unit}-${state}`;
      const im = track(DIR_ART.img, key, `${key}.png`);
      if (spec.mask) {
        const mk = track(DIR_ART.mask, key, `${key}-pcmask.png`);
        const build = () => {
          if (spriteReady(im) && spriteReady(mk)) {
            DIR_ART.red[key] = buildMaskRed(im, mk);
          }
        };
        im.addEventListener('load', build);
        mk.addEventListener('load', build);
      }
    }
  }
  const cap = new Promise((r) => { setTimeout(r, BUILDING_ART_SETTLE_TIMEOUT_MS); });
  DIR_ART.settledPromise = Promise.race([Promise.all(pending), cap]);
  return DIR_ART.settledPromise;
}
function whenDirArtSettled() {
  return DIR_ART.settledPromise || Promise.resolve();
}

// The sheet frame for an action at the current sim tick (the
// sheets' frame counts are the pipeline's: walk/carry 8, gallop 8,
// the work cycles 4, idle 4 slow).
function dirSheetFrame(act) {
  const tick = sim.tick_count();
  switch (act.action) {
    case 'walk': case 'carry': return Math.floor(tick / 7) % 8;
    case 'gallop': return Math.floor(tick / 6) % 8;
    case 'gather': return Math.floor(tick / 8) % 4;
    case 'build': return Math.floor(tick / 6) % 4;
    case 'attack': return Math.floor(tick / 7) % 4;
    case 'shoot': return Math.min(3, Math.floor((tick - (act.shotTick || 0)) / 5));
    case 'graze': return Math.floor(tick / 10) % 4;
    default: return Math.floor(tick / 30) % 4; // idle
  }
}

// Draw one directional-sheet cell for `unit` in the state/direction
// `act` ({action, dir}) at ground point (sx, sy). False when the
// sheet (or its frame) is not drawable — the caller falls back.
function drawDirUnit(unit, act, sx, sy) {
  const spec = DIR_ART_SPEC[unit];
  if (!spec || !act) return false;
  const key = `${unit}-${act.action}`;
  const red = drawOwner === foeOwner() ? DIR_ART.red[key] : null;
  const im = red || DIR_ART.img[key];
  if (!unitArtReady(im)) return false;
  const C = spec.cell;
  const rows = Math.max(1, Math.round((im.naturalHeight || im.height) / C));
  const frames = Math.max(1, Math.round((im.naturalWidth || im.width) / C));
  const d = ((Math.floor((act.dir * rows) / 8) % rows) + rows) % rows;
  const f = ((dirSheetFrame(act) % frames) + frames) % frames;
  const k = (camS * 1.35) / 46;
  ctx.drawImage(im, f * C, d * C, C, C,
    sx - spec.ax * k, sy - spec.ay * k, C * k, C * k);
  return true;
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
  frames: { idle: 4, walk: 8, gather: 4, carry: 8, build: 4 },
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
  else if (atBuildNow.has(i)) state = 'build';
  // Facing (forest readability pass): a stationary gatherer turns
  // to FACE its node for the whole harvest — movement facing used
  // to strand it sideways to the tree, one of the reasons the chop
  // read as "standing by the tree".
  const gTarget = state === 'gather' ? atNodeTarget.get(i) : null;
  if (gTarget) {
    const dirs = VIL_ORIGINAL.dirs;
    const d = Math.round(Math.atan2(gTarget.y - wy, gTarget.x - wx) / ((Math.PI * 2) / dirs));
    a.dir = ((d % dirs) + dirs) % dirs;
  }
  let frame;
  if (state === 'walk' || state === 'carry') {
    frame = Math.floor((a.phase % 1) * VIL_ORIGINAL.frames[state]);
  } else if (state === 'gather' || state === 'build') {
    frame = Math.floor(now / 140) % VIL_ORIGINAL.frames[state];
  } else {
    frame = Math.floor(now / 400) % VIL_ORIGINAL.frames.idle;
  }
  // Hit feedback: frame 2 of the gather cycle is the axe's impact
  // frame (sprite pipeline pose_gather). A villager gathering WOOD
  // lands one hit per cycle on its tree — the tree renderer turns
  // that into a wobble + wood chips. Deduped per tree per cycle.
  if (state === 'gather' && frame === GATHER_IMPACT_FRAME
      && gTarget && gTarget.kind === 0) {
    noteTreeHit(gTarget, wx, wy, now);
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
    // Directional milestone: at the LOW tier the pipeline also
    // ships a player-color mask per sheet — prefer the true
    // mask remap (buildMaskRed) and fall back to the channel swap.
    im.addEventListener('load', () => {
      if (st.tierDir) {
        VIL_ORIGINAL.imgRed[state] = makeRedSheet(im);
        return;
      }
      const mk = new Image();
      mk.addEventListener('load', () => {
        VIL_ORIGINAL.imgRed[state] = buildMaskRed(im, mk) || makeRedSheet(im);
      });
      mk.addEventListener('error', () => {
        VIL_ORIGINAL.imgRed[state] = makeRedSheet(im);
      });
      mk.src = assetUrl(`assets-original/villager-${state}-pcmask.png`);
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
    flash(t('flash.qualitySet', { name: t('quality.' + level) }));
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

// --- Click markers (INF-09) --------------------------------------
// Transient order-confirmation markers for GROUND orders only;
// see play/clickmarker.js for the DE evidence and the mapping.
// Spawned by the order paths below (main view + minimap share
// smartOrderAt, so both are covered by construction).
const clickMarkers = new ClickMarkers();

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
// The nearest node per villager (forest readability pass): the
// gather presentation faces THIS node and lands its hit feedback
// on it — position-keyed tree state lives with the tree renderer.
const atNodeTarget = new Map(); // villager idx -> {id, x, y, kind}

function updateCarry(nodes, tcPos, pos, kinds, hp) {
  for (let i = 0; i < kinds.length; i++) {
    if (kinds[i] !== 0) continue; // villagers only — animals/military carry nothing
    if (hp && hp[i * 2] < 1) { // the dead drop out of the heuristic
      carryKind.delete(i);
      lastNodeKind.delete(i);
      atNodeNow.delete(i);
      atNodeTarget.delete(i);
      continue;
    }
    const wx = pos[i * 2];
    const wy = pos[i * 2 + 1];
    let atKind = null;
    let atNode = null;
    let atD2 = AT_NODE_R2;
    for (const n of nodes) {
      if (n.amount <= 0) continue;
      const dx = n.x - wx;
      const dy = n.y - wy;
      const d2 = dx * dx + dy * dy;
      if (d2 <= atD2) {
        atD2 = d2;
        atKind = n.kind;
        atNode = n;
      }
    }
    if (atKind !== null) {
      atNodeNow.add(i);
      atNodeTarget.set(i, { id: atNode.id, x: atNode.x, y: atNode.y, kind: atNode.kind });
    } else {
      atNodeNow.delete(i);
      atNodeTarget.delete(i);
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
  if (e.code === 'Escape') {
    if (helpOverlay.style.display !== 'none') {
      helpOverlay.style.display = 'none';
    } else if (placement) {
      placement = null;
    } else if (repairArm) {
      repairArm = false;
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
    flash(kind === 'villager' ? t('flash.noIdleVil') : t('flash.noIdleMil'));
    return;
  }
  if (selectAll) {
    selectUnits(new Set(ids));
    playAudioCue('select');
    return;
  }
  const id = nextInCycle(ids, kind === 'villager' ? idleCycleVil : idleCycleMil);
  if (kind === 'villager') idleCycleVil = id;
  else idleCycleMil = id;
  selectUnits(new Set([id]));
  const pos = sim.positions();
  centerOn(pos[id * 2], pos[id * 2 + 1]);
  playAudioCue('select');
}

function handleGameKey(e) {
  // Typing or picking in a form control is never a game key
  // (the pause menu's locale select, the end-card textarea).
  // (Guarded by capability, not DOM constructors: the headless
  // smoke harness loads this file without a full DOM.)
  if (e.target && typeof e.target.closest === 'function'
    && e.target.closest('input, textarea, select, [contenteditable="true"]')) {
    return false;
  }
  // Control groups (SEL-07, wiki Hotkey page — the full DE
  // form): Ctrl+<Number> assigns groups 1–10 (Digit0 is group
  // 10), Ctrl+Alt+<Number> assigns 11–20; <Number> alone (or
  // with Alt for 11–20) selects, a double-tap centers;
  // Shift+<Number> APPENDS the selection to the group;
  // Ctrl+Shift+<Number> selects the group and snaps the view.
  const digit = /^Digit([0-9])$/.exec(e.code);
  if (digit) {
    const base = Number(digit[1]) || 10; // Digit0 → group 10
    const n = base + (e.altKey ? 10 : 0); // the Alt layer: 11–20
    e.preventDefault();
    if (e.repeat) return true;
    if ((e.ctrlKey || e.metaKey) && e.shiftKey) {
      const members = liveGroupMembers(n);
      if (members.length) {
        selectUnits(new Set(members));
        const pos = sim.positions();
        let sx = 0;
        let sy = 0;
        for (const id of members) {
          sx += pos[id * 2];
          sy += pos[id * 2 + 1];
        }
        centerOn(sx / members.length, sy / members.length);
        playAudioCue('select');
      }
      return true;
    }
    if (e.ctrlKey || e.metaKey) {
      const sel = selectedUnits();
      if (sel.length) controlGroups.assign(n, sel);
      return true;
    }
    if (e.shiftKey) {
      // Append the SELECTION to the group (SEL-07's corrected
      // reading — the group grows; the selection is unchanged).
      const sel = selectedUnits();
      if (sel.length) {
        controlGroups.addToGroup(n, sel);
        playAudioCue('select');
      }
      return true;
    }
    const members = liveGroupMembers(n);
    if (!members.length) return true;
    selectUnits(new Set(members));
    playAudioCue('select');
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
        playAudioCue('select');
      }
      return true;
    }
    case 'Delete': { // SEL-14 (wiki Hotkey page): Delete = one
      // selected unit (or the selected own building);
      // Shift+Delete = all selected units.
      if (e.ctrlKey || e.metaKey || e.altKey) return false;
      e.preventDefault();
      if (e.repeat) return true;
      if (selectedBuilding && selectedBuilding.owner === localOwner) {
        deleteSelectedBuilding();
      } else {
        deleteSelectedUnits(e.shiftKey);
      }
      return true;
    }
    case 'Space': { // SEL-10: center the view on the selection
      // (the classic binding the wiki's Hotkey page lists).
      if (e.ctrlKey || e.metaKey || e.altKey) return false;
      if (e.target && e.target.tagName === 'BUTTON') return false;
      e.preventDefault();
      if (e.repeat) return true;
      const ids = selectedUnits();
      const pos = sim.positions();
      if (ids.length) {
        let sx = 0;
        let sy = 0;
        for (const id of ids) {
          sx += pos[id * 2];
          sy += pos[id * 2 + 1];
        }
        centerOn(sx / ids.length, sy / ids.length);
        playAudioCue('select');
      } else if (selectedBuilding) {
        if (selectedBuilding.type === 'tc') {
          const tc = selectedBuilding.id;
          centerOn(pos[tc * 2], pos[tc * 2 + 1]);
          playAudioCue('select');
        } else {
          const bk = buildingList().find((b) => b.id === selectedBuilding.id);
          if (bk) {
            centerOn(bk.x, bk.y);
            playAudioCue('select');
          }
        }
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

/// AutoExplore toggle (SEL-17): flips the mode on the LOCAL
/// player's Scout Cavalry in the current selection — ANY scout,
/// starting or trained (current DE, update 111772; Claudio's
/// 2026-10-20 ruling). The sim is the eligibility truth (living
/// Scout Cavalry owned by a player); the client mirrors it per
/// entity through `entity_can_auto_explore`, and reads the
/// on/off state from the sim's flag array. With several scouts
/// selected the toggle is uniform: if any selected scout is off,
/// all turn on; if all are on, all turn off — each scout still
/// explores independently in the sim. Shared by the
/// command-panel button and the G key, so both run the very same
/// order. Returns whether any order was issued.
function toggleAutoExplore() {
  const flags = sim.entity_auto_explore();
  const scouts = selectedUnits().filter((id) => sim.entity_can_auto_explore(id));
  if (!scouts.length) return false;
  const on = !scouts.every((id) => flags[id] === 1);
  let issued = false;
  for (const id of scouts) {
    if ((flags[id] === 1) !== on && issue({ t: 'auto_explore', e: id, on })) {
      issued = true;
    }
  }
  if (!issued) return false;
  playAudioCue('orderMilitary');
  return true;
}

/// SEL-14 Stop (wiki Hotkey page): halt every selected unit
/// where it stands — the sim clears its orders and task; a
/// villager keeps its carry.
function stopSelectedUnits() {
  let any = false;
  for (const id of selectedUnits()) {
    if (issue({ t: 'stop', e: id })) any = true;
  }
  if (any) playAudioCue('click');
  return any;
}

/// SEL-14 Delete (wiki Hotkey page): Delete destroys ONE
/// selected unit; Shift+Delete (and the panel button) destroys
/// ALL selected. The sim kills through its ordinary death
/// path, so population frees and animals leave carcasses.
function deleteSelectedUnits(all) {
  const ids = selectedUnits();
  if (!ids.length) return false;
  const targets = all ? ids : [ids[0]];
  let any = false;
  for (const id of targets) {
    if (issue({ t: 'delete', e: id })) any = true;
  }
  if (any) {
    selectUnits(new Set(ids.filter((id) => !targets.includes(id))));
    playAudioCue('click');
  }
  return any;
}

/// SEL-14 + BLD-02: delete the selected own building — a
/// foundation refunds its full cost in the sim; a completed
/// building refunds nothing.
function deleteSelectedBuilding() {
  if (!selectedBuilding || selectedBuilding.owner !== localOwner) return false;
  const ok = selectedBuilding.type === 'tc'
    ? issue({ t: 'delete', e: selectedBuilding.id })
    : issue({ t: 'deleteb', b: selectedBuilding.id });
  if (ok) {
    selectBuilding(null);
    playAudioCue('click');
  }
  return ok;
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
  if (!any) flash(t('flash.noLoadedVil'));
  else playAudioCue('orderVillager');
}

/// Go-to (Ctrl+letter): cycle our completed buildings of a kind,
/// selecting and centering each in turn; with `selectAll`, take
/// the whole type at once (no centering unless there is one).
function goToBuildingKind(kind, selectAll) {
  const list = buildingList().filter(
    (b) => b.owner === localOwner && b.kind === kind && b.complete);
  if (!list.length) {
    flash(t('flash.noBuildingYet', { name: BUILD_NAME[kind] }));
    return;
  }
  if (selectAll && list.length > 1) {
    selectBuilding({ type: 'building', id: list[0].id, kind, owner: localOwner });
    buildingGroup = list.map((b) => b.id);
    playAudioCue('select');
    return;
  }
  const id = nextInCycle(list.map((b) => b.id), gotoCycleMem[kind] ?? -1);
  gotoCycleMem[kind] = id;
  const b = list.find((x) => x.id === id);
  selectBuilding({ type: 'building', id: b.id, kind: b.kind, owner: b.owner });
  centerOn(b.x, b.y);
  playAudioCue('select');
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
  const ctx = panelContext();
  const act = resolveHotkey({
    context: ctx, page: buildPage, code: e.code,
    ctrl: e.ctrlKey || e.metaKey, shift: e.shiftKey,
  });
  if (!act) {
    // SEL-17: G toggles AutoExplore — but ONLY in the units
    // context (a military selection: the scout's panel lives
    // there), only unmodified, and only after every existing
    // binding had its say: the build pages' G alias (Mining Camp,
    // on the armed villager eco page) resolves above and keeps
    // the key, and Ctrl+G (go-to Mining Camp) resolves to its
    // goto action above too. Anywhere else G stays unbound — no
    // invented global behaviour.
    if (e.code === 'KeyG' && !e.ctrlKey && !e.metaKey && !e.altKey
        && ctx === 'units') {
      e.preventDefault();
      if (!e.repeat) {
        toggleAutoExplore();
        renderCommands();
      }
      return true;
    }
    return false;
  }
  e.preventDefault();
  if (e.repeat) return true; // a held key is one press, as in DE
  switch (act.type) {
    case 'arm':
      buildPage = act.page;
      repairArm = false;
      renderCommands();
      playAudioCue('click');
      return true;
    case 'button': {
      const btn = [...cmdGrid.children].find((b) => b.id === act.id);
      // A gated building's button stands disabled in the panel;
      // the key is still consumed (it must not leak into camera
      // panning) but does nothing, like the button itself.
      if (btn && !btn.disabled) {
        // Gap 4: Shift held on a train hotkey queues five, like
        // a Shift+click (resolveHotkey already consumed the key;
        // the modifier only scales the train repeat).
        runCommandButton(act.id, e.shiftKey ? SHIFT_TRAIN_COUNT : 1);
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
// by edges. The position is read in VIEWPORT coordinates: the
// pointer counts at any of the four window edges whether it
// rides over the canvas, the top resource bar or the bottom bar
// (the first build read it canvas-only, and the top bar — whose
// elements sit above the canvas and take its pointer events —
// permanently covered the top band, so the top edge never
// panned; Claudio, 2026-10-19). The remaining gates are the
// view's own: inside the window (leaving it clears the point), a
// live match with no modal up — the lobby, room, loading and end
// card all sit outside body.in-match, paused or ended matches.
const EDGE_MARGIN_PX = 6; // CSS px from the viewport edge
const EDGE_PX_PER_FRAME = 14; // the pan keys' P, at 60 Hz
let lastEdgeAt = null;

function applyEdgeScroll(now) {
  if (lastEdgeAt == null) lastEdgeAt = now;
  const dt = Math.min(Math.max(now - lastEdgeAt, 0), 100);
  lastEdgeAt = now;
  if (!mouseClientPt) return;
  if (paused || matchEnded || playback) return;
  if (!document.body || !document.body.classList.contains('in-match')) return;
  if (helpOverlay.style.display !== 'none' || pauseMenu.style.display !== 'none') return;
  const vw = window.innerWidth;
  const vh = window.innerHeight;
  if (!vw || !vh) return;
  const v = edgePan(mouseClientPt.x, mouseClientPt.y, vw, vh, EDGE_MARGIN_PX);
  if (!v.x && !v.y) return;
  const r = canvas.getBoundingClientRect();
  const step = EDGE_PX_PER_FRAME * (dt / (1000 / 60))
    * (r.width ? canvas.width / r.width : 1);
  camOx += v.x * step;
  camOy += v.y * step;
}

// --- Fullscreen (2026-10-19) -----------------------------------------------
// One toggle for the top-bar button and the pause-menu entry:
// fullscreen keeps the pointer inside the game (with viewport-
// edge scrolling it can ride the screen edges without slipping
// out of the window). The glyph/title track the real state via
// fullscreenchange, so Esc-out-of-fullscreen stays in sync too.
function isFullscreen() {
  return !!(document.fullscreenElement || document.webkitFullscreenElement);
}

function syncFullscreenUi() {
  const fs = isFullscreen();
  if (fullscreenBtn) {
    fullscreenBtn.textContent = fs ? '⤢' : '⛶';
    fullscreenBtn.title = fs ? t('top.exitFullscreen') : t('top.fullscreen');
  }
  if (pauseFullscreenBtn) {
    pauseFullscreenBtn.textContent = fs ? t('top.exitFullscreen') : t('top.fullscreen');
  }
}

function toggleFullscreen() {
  if (isFullscreen()) {
    const exit = document.exitFullscreen || document.webkitExitFullscreen;
    if (exit) exit.call(document);
    return;
  }
  const el = document.documentElement;
  if (el.requestFullscreen) {
    const p = el.requestFullscreen();
    if (p && p.catch) p.catch(() => {});
  } else if (el.webkitRequestFullscreen) {
    el.webkitRequestFullscreen();
  }
}

function setPaused(v) {
  if (!playing) return;
  const wasPaused = paused;
  paused = v;
  pauseMenu.style.display = paused ? 'flex' : 'none';
  // Generated audio: the menu's own open/close cues.
  if (wasPaused !== paused) playAudioCue(paused ? 'menuOpen' : 'menuClose');
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
    case 'repair': return cmd.q
      ? sim.push_repair_queued(cmd.e, cmd.b)
      : sim.push_repair(cmd.e, cmd.b);
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
    case 'auto_explore': return sim.push_auto_explore(cmd.e, cmd.on);
    case 'stop': return sim.push_stop(cmd.e);
    case 'delete': return sim.push_delete(cmd.e);
    case 'deleteb': return sim.push_delete_building(cmd.b);
    case 'cancel_tc': return sim.push_cancel_queue_tc(cmd.tc, cmd.i);
    case 'cancel_b': return sim.push_cancel_queue_building(cmd.b, cmd.i);
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
  resetEventFeed();
  resetAudioWorldState();
  refreshWorldLayers();
  vilAnim.clear();
  prevPos.clear();
  lastNodeKind.clear();
  carryKind.clear();
  atNodeNow.clear();
  archerShoot.clear();
  seenProjKeys.clear();
  for (const k of Object.keys(UNIT_POSE_STATE)) delete UNIT_POSE_STATE[k];
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
  resetEventFeed();
  resetAudioWorldState();
  refreshWorldLayers();
  vilAnim.clear();
  prevPos.clear();
  lastNodeKind.clear();
  carryKind.clear();
  atNodeNow.clear();
  archerShoot.clear();
  seenProjKeys.clear();
  for (const k of Object.keys(UNIT_POSE_STATE)) delete UNIT_POSE_STATE[k];
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
        t('desync.detail', { turn: opts.desyncTurn });
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
  const stateKey = {
    lobby: 'lobby', starting: 'starting', playing: 'playing',
    'peer-left': 'peerLeft', rebuilding: 'rebuilding',
    ended: 'ended', desync: 'desync',
  }[net.phase];
  const stateTxt = stateKey ? t('net.state.' + stateKey) : net.phase;
  netUi.netStatus.hidden = false;
  netUi.netStatusText.textContent =
    `${t('net.online')} · ${regionName(net.region)} · ${net.role === 'host' ? t('net.youHost') : t('net.youGuest')} · ${stateTxt}` +
    (net.pingMs != null ? ` · ${t('net.ping', { ms: net.pingMs })}` : '');
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
      if (onStep) onStep(t('load.stepDownload'));
      const mod = await import('./pkg/skirmish_sim.js');
      await mod.default(); // fetch + instantiate the .wasm
      WasmSim = mod.WasmSim;
      if (onStep) onStep(t('load.stepArtwork'));
      loadPlaceholderSprites();
      loadBuildingArt();
      loadNodeArt();
      loadAnimalArt();
      loadUnitArt();
      loadDirArt();
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
        whenAnimalArtSettled(), whenUnitArtSettled(), whenDirArtSettled()]);
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
      if (onStep) onStep(t('load.stepWorld'));
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
    `<span class="slot-state">${loaded ? t('load.loaded') : t('load.loadingSeat')}</span>`;
  netUi.loadSlotHost.innerHTML = row(t('load.blueHost'), m.blueLoaded);
  // M12: with an AI guest slot the red seat names the AI (it is
  // this same client's sim — it shows loaded the moment the
  // host's build verifies).
  const guestLabel = slotIsAi(net.guestSlot)
    ? t('load.redAi', { slot: slotLabel(net.guestSlot, slotStrings()) }) : t('load.redGuest');
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
  setLoadingStatus(t('load.status'));
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
const matchCancelledMsg = () => t('load.matchCancelled');

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
  lobbyStatus(matchCancelledMsg());
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
  setLoadingStatus(t('load.waitOpponent'));
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
  setLoadingStatus(t('load.waitHost'));
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
  setLoadingStatus(t('load.starting'));
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

/// i18n: the localized label sets the pure room/lobby modules
/// take (their own defaults are the English originals).
function slotStrings() {
  return {
    open: t('slot.open'), closed: t('slot.closed'),
    aiEasy: t('mode.aiEasy'), aiNormal: t('mode.aiNormal'),
  };
}
function roomConfigStrings() {
  return {
    slot: slotStrings(),
    you: t('room.you'),
    format: (guest) => t('room.config', { guest }),
  };
}
function lobbyStrings() {
  return {
    unknownServer: t('lobby.unknownServer'),
    titleFmt: (region) => t('lobby.titleAuto', { region }),
    mapName: t('map.arabia'),
  };
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
  if (isHost) hostState = t('room.you');
  else hostState = (net.peerPresent || net.hostSeen) ? t('room.here') : t('room.waiting');
  if (!isHost) {
    guestState = priv && net.pwState !== 'ok' ? t('room.youPw') : t('room.you');
  } else if (aiSlot) {
    guestState = slotLabel(slot, slotStrings());
  } else if (slot === 'closed') {
    guestState = t('room.closedState');
  } else if (net.peerPwOk || net.peerPresent) {
    guestState = t('room.here');
  } else if (net.doorGuest) {
    guestState = t('room.atDoor');
  } else {
    guestState = t('room.waitingGuest');
  }
  const badge = (r) => (r ? ` · <span class="slot-ready">${t('room.readyBadge')}</span>` : '');
  netUi.slotHost.innerHTML =
    '<span class="slot-dot dot-blue"></span>' +
    `<span class="slot-name">${t('room.hostSeat')}</span>` +
    `<span class="slot-state">${hostState}${badge(hostReady)}</span>`;
  netUi.slotGuest.innerHTML =
    '<span class="slot-dot dot-red"></span>' +
    `<span class="slot-name">${t('room.guestSeat')}</span>` +
    `<span class="slot-state">${guestState}${badge(guestReady)}</span>`;
  // M12: the configuration summary — the game's fixed facts
  // plus the guest seat, from the viewer's own side of it.
  if (netUi.roomConfigLine) {
    netUi.roomConfigLine.textContent = roomConfigText({
      guestSlot: slot, selfIsGuest: !isHost,
    }, roomConfigStrings());
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
  if (showPwNote) netUi.pwNote.textContent = t('status.incorrectPw');
  // The Start toggle: live only for an admitted player with an
  // admitted, present peer — or, with an AI seat, for the host
  // alone (the AI always counts as ready).
  const can = canToggleReady({
    phase: net.phase, peerPresent: net.peerPresent, admitted: myAdmitted(),
    guestSlot: slot,
  });
  netUi.readyBtn.disabled = !can;
  netUi.readyBtn.textContent = net.selfReady ? t('room.readyCancel') : t('room.start');
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
  lobbyStatus(t('status.checkingPw'));
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
    lobbyStatus(t('status.rebuildFail'));
    return;
  }
  net.phase = 'rebuilding';
  net.seed = msg.seed;
  const slot = typeof msg.yourSlot === 'number' ? msg.yourSlot : 1;
  net.role = slot === 0 ? 'host' : 'guest';
  const owner = slot === 0 ? 0 : 2;
  lobbyStatus(t('status.rebuilding'));
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
  lobbyStatus(t('status.rebuilt'));
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
          ? t('peer.detailGrace', { secs: left })
          : t('peer.detailClosed');
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
    lobbyStatus(t('status.opponentLeft'));
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
        lobbyStatus(t('status.guestAtDoor'));
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
        lobbyStatus(t('status.privateEnter'));
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
          lobbyStatus(t('status.opponentHere'));
        } else {
          lobbyStatus(t('status.connected'));
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
      lobbyStatus(t('status.roomFull'));
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
        lobbyStatus(matchCancelledMsg());
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
        lobbyStatus(t('status.privateEnter'));
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
          lobbyStatus(t('status.opponentHere'));
        } else {
          net.client.send({ type: 'pw-no' });
          lobbyStatus(t('status.wrongPwGuest'));
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
        lobbyStatus(t('status.youAreIn'));
        renderRoom();
        updateNetStatus();
      }
      break;
    case 'pw-no':
      if (net.role === 'guest' && net.phase === 'lobby') {
        net.pwState = 'wrong';
        lobbyStatus(t('status.incorrectPw'));
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
        flash(t('flash.reconnectedPeer'));
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
        flash(t('flash.reconnectedSelf'));
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
          lobbyStatus(t('status.roomAi', { slot: slotLabel(net.guestSlot, slotStrings()) }));
        } else if (net.guestSlot === 'closed') {
          lobbyStatus(t('status.roomClosed'));
        } else if (net.privacy === 'private') {
          lobbyStatus(t('status.roomPrivate'));
        } else {
          lobbyStatus(t('status.roomPublic'));
        }
      } else if (net.privacy === 'private') {
        lobbyStatus(t('status.privateEnter'));
      } else {
        lobbyStatus(t('status.joining'));
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
  lobbyStatus(join ? t('status.joining') : t('status.pickServer'));
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
      lobbyStatus(t('status.setPwFirst'));
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
    strings: lobbyStrings(),
  }));
  // Lobby order (oldest game first), then the controls row's
  // filters decide what is visible (M11).
  const rows = filterLobbyRows(sortLobbyRows(models, 'created', 1), lobbyFilters);
  const list = netUi.gamesList;
  list.innerHTML = '';
  const head = document.createElement('div');
  head.className = 'game-row game-head';
  head.innerHTML = `<span class="g-title">${t('lobby.colGame')}</span>`
    + `<span class="g-players">${t('lobby.colPlayers')}</span>`
    + `<span class="g-map">${t('lobby.colMap')}</span>`
    + `<span class="g-loc">${t('lobby.colLocation')}</span>`
    + `<span class="g-server">${t('lobby.colServer')}</span>`;
  list.append(head);
  if (!rows.length) {
    const empty = document.createElement('p');
    empty.className = 'games-empty';
    empty.textContent = lobbyGames.open.length
      ? t('lobby.emptyFiltered')
      : t('lobby.emptyNone');
    list.append(empty);
  }
  for (const m of rows) {
    const row = document.createElement('div');
    row.className = 'game-row' + (m.room === lobbySelectedRoom ? ' selected' : '');
    row.dataset.room = m.room;
    row.innerHTML = `<span class="g-title">${m.locked ? `<span class="g-lock" title="${t('lobby.lockTitle')}">🔒</span> ` : ''}${m.title}</span>`
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
      ? t(n === 1 ? 'lobby.inProgressOne' : 'lobby.inProgressOther', { n })
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
  any.textContent = t('lobby.any');
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
      `<span class="slot-name">${t('load.blueYou')}</span>` +
      `<span class="slot-state">${t('load.loadingSeat')}</span>`;
    netUi.loadSlotGuest.style.display = 'none';
    setLoadingStatus(t('load.status'));
  }
  try {
    await ensureGameLoaded((t) => setLoadingStatus(t));
  } catch {
    hideLoading();
    openLobby(null);
    lobbyStatus(t('status.loadFail'));
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
    flash(t('status.loadFail'));
    return;
  }
  const events = replayEvents(rec);
  if (!events) {
    flash(t('flash.notReplay'));
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
    netUi.pbLabel.textContent = t('pb.line', {
      detail: (rec.region ? regionName(rec.region) + ' · ' : '') + t('pb.seedPart', { seed: rec.seed }),
    });
    netUi.pbToggle.textContent = t('pb.pause');
    netUi.pbSpeed.textContent = '1×';
  }
  playing = true;
  setPaused(false);
  flash(t('flash.playingReplay'));
}

function exitPlayback() {
  playback = null;
  if (netUi && netUi.playbackBar) netUi.playbackBar.hidden = true;
  resetSim(matchMode);
  playing = true;
  setPaused(false);
}

function stepReplayTick() {
  const tickNow = sim.tick_count();
  while (playback.idx < playback.events.length
      && playback.events[playback.idx].tick <= tickNow) {
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
    if (netUi) netUi.pbToggle.textContent = t('pb.play');
    flash(t('flash.replayEnded'));
  }
}

function drivePlayback(now) {
  if (!playback.playing) {
    playback.last = now;
    return;
  }
  if (!playback.last) playback.last = now;
  // Replay playback follows the same pacing as live play — the
  // advanceTicks conversion at GAME_SPEED — times the viewer's
  // own speed setting. (playback.acc is a game-ms remainder.)
  const adv = advanceTicks(
    playback.acc, Math.min(now - playback.last, 250) * playback.speed);
  playback.last = now;
  playback.acc = adv.accMs;
  let n = 0;
  while (n < adv.ticks && n < 24 && !playback.done) {
    stepReplayTick();
    n++;
  }
  if (adv.ticks > n) playback.acc = 0; // shed the backlog, as the old cap did
}

/// Download a replay object as a .json file.
function downloadReplay(replayObj) {
  if (!replayObj) {
    flash(t('flash.noReplayYet'));
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

// --- i18n: static strings + the language switch -------------------
// applyStaticStrings re-voices every static element of the page
// from the string table (play/i18n.js). applyLocale is the ONE
// switch path (both language selects call it): persist the
// choice, rebuild the name tables, re-voice the statics, and
// re-run the live renderers so the current screen changes
// language immediately, without a reload.
function setText(id, key, vars) {
  const e = document.getElementById(id);
  if (e) e.textContent = t(key, vars);
}
function setAttr(id, attr, key) {
  const e = document.getElementById(id);
  if (e && e.setAttribute) e.setAttribute(attr, t(key));
}
function setTitle(id, key) {
  const e = document.getElementById(id);
  if (e) e.title = t(key);
}
function setPlaceholder(id, key) {
  const e = document.getElementById(id);
  if (e) e.placeholder = t(key);
}
function setOptions(id, pairs) {
  const sel = document.getElementById(id);
  if (!sel || !sel.options) return;
  for (const opt of sel.options) {
    const k = pairs[opt.value];
    if (k) opt.textContent = t(k);
  }
}

function applyStaticStrings() {
  document.title = t('title.play');
  if (document.documentElement) document.documentElement.lang = getLocale();
  // Top bar (the age chip / idle titles; the fullscreen button's
  // title is syncFullscreenUi's, called by applyLocale).
  setAttr('res-group', 'aria-label', 'top.resources');
  setTitle('res-age-chip', 'top.currentAge');
  setTitle('idle-vil-btn', 'top.idleVillagers');
  setTitle('idle-mil-btn', 'top.idleMilitary');
  setTitle('pause-btn', 'top.menuTitle');
  // The label lives in its own span (play/index.html): writing
  // EVERY text node of the button — the old loop — also rewrote
  // the whitespace node before the banner icon, which painted
  // the word twice ("Menu [banner] Menu", Claudio 2026-10-20).
  setText('pause-btn-label', 'top.menu');
  setAttr('cmd-panel', 'aria-label', 'panel.commands');
  setAttr('sel-panel', 'aria-label', 'panel.selection');
  setAttr('minimap-wrap', 'aria-label', 'panel.minimap');
  setTitle('page-eco', 'cmd.ecoTitle');
  setTitle('page-mil', 'cmd.milTitle');
  // Pause menu.
  setText('pause-title', 'pause.title');
  setAttr('menu-close', 'aria-label', 'pause.closeMenu');
  setText('resume-btn', 'pause.resume');
  setText('help-btn', 'pause.howToPlay');
  setText('restart-btn', 'pause.restart');
  setText('btn-online', 'pause.playOnline');
  setText('net-note', 'pause.netNote');
  setText('btn-resign', 'pause.resign');
  setText('btn-download-replay', 'end.downloadReplay');
  setText('btn-watch-replay', 'pause.watchReplay');
  setAttr('mode-row', 'aria-label', 'pause.matchMode');
  setText('mode-sandbox-txt', 'mode.sandbox');
  setText('mode-easy-txt', 'mode.aiEasy');
  setText('mode-normal-txt', 'mode.aiNormal');
  setText('mode-note', 'pause.modeNote');
  setAttr('quality-row', 'aria-label', 'pause.visualQuality');
  setText('quality-title', 'pause.quality');
  setText('q-auto', 'quality.auto');
  setText('q-high', 'quality.high');
  setText('q-medium', 'quality.medium');
  setText('q-low', 'quality.low');
  setText('quality-note', 'pause.qualityNote');
  setAttr('audio-row', 'aria-label', 'audio.title');
  setText('audio-title', 'audio.title');
  setText('audio-enabled-txt', 'audio.enabled');
  setText('audio-volume-txt', 'audio.volume');
  setText('audio-note', 'audio.note');
  setText('test-note', 'pause.testBuildNote');
  setText('back-site-link', 'pause.backToSite');
  setText('lang-label-lobby', 'lang.label');
  setText('lang-label-pause', 'lang.label');
  // Help overlay (the items carry <strong> markup — innerHTML).
  setText('help-title', 'help.title');
  setAttr('help-close-x', 'aria-label', 'help.closeGuide');
  setText('help-close', 'help.close');
  setText('help-esc-note', 'help.escNote');
  if (document.querySelectorAll) {
    document.querySelectorAll('.howto li').forEach((li, i) => {
      li.innerHTML = t('help.i' + (i + 1));
    });
  }
  // End card + net overlays.
  setText('end-restart-btn', 'end.restart');
  setText('end-menu-btn', 'end.backToMenu');
  setText('end-replay-btn', 'end.downloadReplay');
  // End-of-match feedback block (its GitHub line is the anchor's
  // text; the stub DOMs have no querySelector hit and skip it).
  setText('feedback-q', 'feedback.question');
  setAttr('feedback-close', 'aria-label', 'feedback.dismiss');
  setAttr('feedback-stars', 'aria-label', 'feedback.rateMatch');
  setPlaceholder('feedback-text', 'feedback.placeholder');
  setText('feedback-send', 'feedback.send');
  setText('feedback-thanks', 'feedback.thanks');
  for (const btn of feedbackStarBtns) {
    if (btn && btn.setAttribute) {
      const n = Number(btn.dataset.stars);
      btn.setAttribute('aria-label', n === 1 ? t('feedback.rateOne') : t('feedback.rateMany', { n }));
    }
  }
  if (document.querySelector) {
    const ghLink = document.querySelector('#feedback-gh-line a');
    if (ghLink) ghLink.textContent = t('feedback.starGithub');
  }
  setText('net-wait', 'netwait.text');
  setText('desync-title', 'desync.title');
  setText('btn-desync-replay', 'end.downloadReplay');
  setText('btn-desync-close', 'end.backToMenu');
  setText('peer-title', 'peer.title');
  setText('peer-detail', 'peer.detailDefault');
  setText('btn-claim-victory', 'peer.claim');
  setText('btn-leave-match', 'peer.leave');
  // Lobby browser + create panel + room screen.
  setAttr('online-close', 'aria-label', 'lobby.close');
  setText('lobby-title-h', 'lobby.title');
  setText('lobby-sub', 'lobby.sub');
  setText('lbl-search', 'lobby.search');
  setPlaceholder('games-search', 'lobby.searchPlaceholder');
  setText('btn-refresh-games', 'lobby.refresh');
  setText('lbl-password', 'lobby.password');
  setText('lbl-server', 'lobby.server');
  setText('btn-solo', 'lobby.playSolo');
  setText('btn-join-lobby', 'lobby.join');
  setText('btn-create-open', 'lobby.create');
  setText('create-title-h', 'create.title');
  setText('btn-create-back', 'create.back');
  setText('create-note', 'create.note');
  setAttr('privacy-row', 'aria-label', 'create.privacyLabel');
  setText('priv-public-txt', 'create.public');
  setText('priv-private-txt', 'create.private');
  setPlaceholder('room-password', 'create.pwPlaceholder');
  setText('settings-title', 'set.title');
  setText('set-map-name', 'set.map');
  setText('set-map-value', 'map.arabia');
  setText('set-victory-name', 'set.victory');
  setText('set-victory-value', 'set.victoryValue');
  setText('set-players-name', 'set.players');
  setText('set-guest-name', 'set.guestSlot');
  const slotOptions = {
    open: 'slot.openOption', 'ai-easy': 'mode.aiEasy',
    'ai-normal': 'mode.aiNormal', closed: 'slot.closedOption',
  };
  setOptions('create-guest-slot', slotOptions);
  setOptions('guest-slot-select', slotOptions);
  setText('btn-create-game', 'lobby.create');
  setText('room-title-h', 'room.title');
  setText('slot-control-txt', 'set.guestSlot');
  setText('room-listed-line', 'room.listedLine');
  setText('room-private-line', 'room.privateLine');
  setPlaceholder('pw-input', 'room.pwPlaceholder');
  setText('btn-pw-enter', 'room.enter');
  setText('invite-txt', 'room.invite');
  setText('btn-copy-link', 'room.copy');
  setText('conn-title-h', 'conn.title');
  setText('loading-title-h', 'load.title');
  // Ad slots: the caption + note (the slot bodies belong to ads.js).
  if (document.querySelectorAll) {
    document.querySelectorAll('.ad-label').forEach((e) => { e.textContent = t('ad.label'); });
    document.querySelectorAll('.ad-note').forEach((e) => { e.textContent = t('ad.note'); });
  }
  // Playback bar (its label is startPlayback's while one runs).
  setText('pb-exit', 'pb.exit');
  if (!playback) setText('pb-label', 'pb.label');
}

function applyLocale(loc) {
  setLocale(loc);
  refreshNames();
  applyStaticStrings();
  syncFullscreenUi();
  cmdSig = '';
  selSig = '';
  lastAge = -1;
  for (const id of ['lang-select', 'lang-select-pause']) {
    const s = document.getElementById(id);
    if (s) s.value = getLocale();
  }
  if (netUi) {
    renderLobbyGames();
    renderRegionFilter();
    renderConnQuality();
    renderRegionList(lobbyPings);
    renderRoom();
    updateNetStatus();
    // In the browser (no room yet) the status line is the
    // standing pick-a-server prompt — re-voice it too. In a
    // room it carries event history and is left alone.
    if (net && !net.room) lobbyStatus(t('status.pickServer'));
    if (playback && netUi.pbToggle) {
      netUi.pbToggle.textContent = playback.playing ? t('pb.pause') : t('pb.play');
    }
  }
  syncAudioUi();
}

/// Reflect the audio engine's persisted settings in the pause
/// menu's controls (called on boot, on every locale switch —
/// applyStaticStrings re-voices the labels — and after a change).
function syncAudioUi() {
  const s = gameAudio.settings();
  if (audioEnabledInput) audioEnabledInput.checked = s.enabled;
  if (audioVolumeInput) audioVolumeInput.value = String(Math.round(s.volume * 100));
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
  if (fullscreenBtn) fullscreenBtn.addEventListener('click', toggleFullscreen);
  if (pauseFullscreenBtn) pauseFullscreenBtn.addEventListener('click', toggleFullscreen);
  document.addEventListener('fullscreenchange', syncFullscreenUi);
  document.addEventListener('webkitfullscreenchange', syncFullscreenUi);
  syncFullscreenUi();
  // Idle indicators double as buttons: the same cycle the '.' / ','
  // keys run (SEL-08/09).
  idleVilBtn.addEventListener('click', () => { if (sim) cycleIdle('villager', false); });
  idleMilBtn.addEventListener('click', () => { if (sim) cycleIdle('military', false); });
  // Generated audio: the first pointer/key gesture unlocks the
  // context (autoplay policy — capture phase, so it runs before
  // the gesture's own handlers and their cues can sound).
  addEventListener('pointerdown', () => { gameAudio.unlock(); }, true);
  addEventListener('keydown', () => { gameAudio.unlock(); }, true);
  // The pause menu's sound controls: persisted on/off + volume.
  if (audioEnabledInput) {
    audioEnabledInput.addEventListener('change', () => {
      gameAudio.setEnabled(!!audioEnabledInput.checked);
      if (audioEnabledInput.checked) {
        gameAudio.unlock();
        playAudioCue('click');
      }
    });
  }
  if (audioVolumeInput) {
    audioVolumeInput.addEventListener('input', () => {
      gameAudio.setVolume(Number(audioVolumeInput.value) / 100);
    });
    audioVolumeInput.addEventListener('change', () => playAudioCue('click'));
  }
  syncAudioUi();
  // The generic UI click: one delegated listener voices ordinary
  // chrome clicks. Surfaces with their own cue (the command grid,
  // the canvas, the menu open/close buttons, the idle pills) are
  // skipped — they speak for themselves.
  document.addEventListener('click', (ev) => {
    const t = ev.target;
    if (!t || !t.closest) return;
    if (t.closest('#cmd-grid, #view, #pause-btn, #resume-btn, #menu-close, #idle-vil-btn, #idle-mil-btn')) return;
    if (t.closest('button, select, input, .game-row')) playAudioCue('click');
  });
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

  // End-of-match feedback: stars paint on click, Send posts to
  // the relay (sendFeedback owns the thanks/error states), and
  // the ✕ hides the block for the rest of the page session.
  feedbackStarBtns.forEach((btn, i) => {
    if (btn) btn.addEventListener('click', () => setFeedbackStars(i + 1));
  });
  if (feedbackSendBtn) feedbackSendBtn.addEventListener('click', () => { sendFeedback(); });
  if (feedbackCloseBtn) {
    feedbackCloseBtn.addEventListener('click', () => {
      feedbackDismissed = true;
      if (feedbackBlock) feedbackBlock.style.display = 'none';
    });
  }

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
      flash(t('flash.linkCopied'));
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
        flash(t('flash.notReplay'));
      }
    });
  }
  if (netUi.pbToggle) {
    netUi.pbToggle.addEventListener('click', () => {
      if (!playback || playback.done) return;
      playback.playing = !playback.playing;
      netUi.pbToggle.textContent = playback.playing ? t('pb.pause') : t('pb.play');
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
  // i18n: voice the whole page in the stored/detected locale
  // before the lobby opens, and wire the two language selects
  // (lobby header + pause menu) to the one switch path.
  refreshNames();
  applyLocale(getLocale());
  for (const id of ['lang-select', 'lang-select-pause']) {
    const sel = document.getElementById(id);
    if (sel) sel.addEventListener('change', () => applyLocale(sel.value));
  }
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
    // Gap 4: Shift+click on a train button queues five.
    runCommandButton(btn.id, ev.shiftKey ? SHIFT_TRAIN_COUNT : 1);
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
  // The sheep is the smallest sprite in the game and its wool
  // sits well above its ground point, so it gets a wider pick
  // radius: clicks on the visible body must select it (the
  // 1.9 wu standard radius only covers clicks near its feet).
  function unitAt(wx, wy) {
    const st = entityState();
    let best = null;
    let bestD2 = 1.9 * 1.9;
    for (let i = 0; i < st.count; i++) {
      if (!isOwnUnit(st, i)) continue;
      const r = st.kinds[i] === KIND_SHEEP ? 2.75 : 1.9;
      const d2 = (st.pos[i * 2] - wx) ** 2 + (st.pos[i * 2 + 1] - wy) ** 2;
      if (d2 < r * r && (best === null || d2 < bestD2)) {
        bestD2 = d2;
        best = i;
      }
    }
    return best;
  }

  // Left-click pick for an animal that is NOT ours (a wild Gaia
  // sheep/boar/deer — kind tags 5/6/7 — or the enemy's sheep): an
  // information selection only. The reference lets you click any
  // animal to inspect it; in earlier builds the click silently
  // cleared the selection, which read as "sheep can't be
  // selected" (Claudio, 2026-10-20). Fog rule matches enemy
  // targets: only what is currently visible can be picked.
  function animalAt(wx, wy) {
    const st = entityState();
    let best = null;
    let bestD2 = Infinity;
    for (let i = 0; i < st.count; i++) {
      const k = st.kinds[i];
      if (k !== KIND_SHEEP && k !== 6 && k !== 7) continue;
      if (st.owners[i] === localOwner || st.hp[i * 2] < 1) continue;
      if (!cellVisibleW(st.pos[i * 2], st.pos[i * 2 + 1])) continue;
      const r = k === KIND_SHEEP ? 2.75 : 2.2;
      const d2 = (st.pos[i * 2] - wx) ** 2 + (st.pos[i * 2 + 1] - wy) ** 2;
      if (d2 < r * r && d2 < bestD2) {
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
        if (issue({ t: 'rally_tc', x: wx, y: wy })) {
          playAudioCue('orderBuilding');
          clickMarkers.add('ground', wx, wy, performance.now());
        }
        return;
      }
      if (!foe && selectedBuilding.type === 'building' && [2, 6, 7].includes(selectedBuilding.kind)) {
        if (issue({ t: 'rally_b', b: selectedBuilding.id, x: wx, y: wy })) {
          playAudioCue('orderBuilding');
          clickMarkers.add('ground', wx, wy, performance.now());
        }
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
      flash(selectedAnimal != null
        ? t('flash.notYourAnimal')
        : selectedBuilding != null
        ? (selectedBuilding.owner === foeOwner()
          ? t('flash.enemyOrders')
          : selectedBuilding.type === 'tc'
            ? t('flash.tcTrains')
            : t('flash.trainFromPanel'))
        : t('flash.selectUnitFirst'));
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
    let groundMoveSent = false;
    const sendMove = (id) => {
      const [mx, my] = moveDest ? moveDest(id) : [wx, wy];
      const sent = issue({ t: 'move', e: id, x: mx, y: my, q: queued || undefined });
      if (sent) groundMoveSent = true;
      return sent;
    };
    // The order acknowledgement's voice: military if any soldier
    // is among the ordered, else the villager's — one cue per
    // order, however many units it names (generated audio).
    const orderCue = () => (targets.some((id) => [2, 3, 4, 8].includes(st.kinds[id]))
      ? 'orderMilitary'
      : targets.some((id) => st.kinds[id] === 0) ? 'orderVillager' : null);
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
      if (any) { playAudioCue(orderCue()); return; }
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
      if (any) { playAudioCue(orderCue()); return; }
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
      if (any) { playAudioCue(orderCue()); return; }
    }
    // Repair (BLD-17): an own, complete, damaged building takes
    // every selected villager as a repairer. One exception: a
    // loaded villager clicking a drop-off building still drops
    // off first (the block below) — hauling home is the older,
    // load-bearing gesture, and the Repair button covers camps.
    if (bT && bT.owner === localOwner && bT.kind !== 1 && bT.complete
        && bT.hpFrac < 0.999) {
      const isDrop = bT.kind >= 3 && bT.kind <= 5;
      const anyLoaded = targets.some(
        (id) => st.kinds[id] === 0 && carryKind.has(id));
      if (!(isDrop && anyLoaded)) {
        let any = false;
        for (const id of targets) {
          if (st.kinds[id] !== 0) continue;
          if (issue({ t: 'repair', e: id, b: bT.id, q: queued || undefined })) any = true;
        }
        if (any) { playAudioCue('orderVillager'); return; }
      }
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
      if (any) { playAudioCue('orderVillager'); return; }
    }
    let n = nodeAt(wx, wy);
    // M7: a node on unexplored ground cannot be ordered — the
    // click is a ground move instead.
    if (n && !cellExploredW(n.x, n.y)) n = null;
    let any = false;
    for (const id of targets) {
      if (n && st.kinds[id] === 0) {
        if (issue({ t: 'gather', e: id, n: n.id, q: queued || undefined })) any = true;
      } else if (sendMove(id)) {
        any = true;
      }
    }
    if (any) playAudioCue(orderCue());
    // INF-09 click marker: the click named a NODE (an object) —
    // no ground marker, exactly as in DE. A pure ground move
    // (nobody was sent to an object) gets the marker at the
    // clicked point. Object orders above (attack / repair /
    // drop-off) returned earlier and never reach this line.
    if (!n && groundMoveSent) {
      clickMarkers.add('ground', wx, wy, performance.now());
    }
  }

  // Repair targeting (BLD-17): the armed click picks the building;
  // every selected villager is ordered onto it. An invalid pick
  // flashes and STAYS armed, exactly like a rejected placement.
  function attemptRepair(wx, wy, queued = false) {
    const b = buildingAt(wx, wy);
    if (!b) {
      flash(t('flash.repairArmOwn'));
      return;
    }
    if (b.owner !== localOwner) {
      flash(t('flash.repairOwnOnly'));
      return;
    }
    if (b.kind === 1) {
      flash(t('flash.repairFarms'));
      return;
    }
    if (!b.complete) {
      flash(t('flash.repairConstruction'));
      return;
    }
    if (b.hpFrac >= 0.999) {
      flash(t('flash.repairUndamaged'));
      return;
    }
    const st = entityState();
    let any = false;
    for (const id of selectedUnits()) {
      if (st.kinds[id] !== 0) continue;
      if (issue({ t: 'repair', e: id, b: b.id, q: queued || undefined })) any = true;
    }
    if (!any) {
      repairArm = false;
      flash(t('flash.repairSelectVil'));
      return;
    }
    if (!queued) repairArm = false;
    playAudioCue('orderVillager');
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
      flash(t('flash.buildSelectVil'));
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
      playAudioCue('buildingPlaced'); // the wood knock
    } else {
      flash(t('flash.cannotBuildThere', { name: BUILD_NAME[placement.kind] }));
    }
  }

  canvas.addEventListener('mousemove', (ev) => {
    mouseWorld = eventWorld(ev);
  });
  // M13 edge scrolling reads the pointer's viewport position
  // from the window: it counts at the screen edges over ANY
  // element — canvas, top bar, bottom bar — because the bars
  // float over the canvas and would otherwise swallow their
  // edge bands (the top bar did exactly that to the top edge).
  // Overlays still rest the pan via applyEdgeScroll's own gates.
  addEventListener('mousemove', (e) => {
    mouseClientPt = { x: e.clientX, y: e.clientY };
  });
  document.addEventListener('mouseleave', () => {
    mouseClientPt = null;
  });
  addEventListener('blur', () => {
    mouseClientPt = null;
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
    if (repairArm) {
      if (!d.moved) attemptRepair(wx, wy, ev.shiftKey); // a targeting click, not a drag
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
      if (boxed.length) playAudioCue('select');
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
      playAudioCue('select');
    } else if (!ev.shiftKey) {
      // No unit hit: an animal that is not ours selects for
      // information (see animalAt); the Town Center and placed
      // buildings are selections of their own (kept OUT of the
      // unit Set); empty ground clears both selections. M5: the
      // enemy's TC and buildings select too — for information
      // only (their owner rides along in the descriptor; the
      // panels gate on it).
      const a = animalAt(wx, wy);
      if (a != null) {
        selectAnimal(a);
        return;
      }
      const tc = tcAt(wx, wy);
      const st = entityState();
      // M7: information selection obeys fog too — an enemy Town
      // Center or building selects only once its ground is
      // explored; in the black the click is plain empty ground.
      if (tc != null && (st.owners[tc] === localOwner
          || cellExploredW(st.pos[tc * 2], st.pos[tc * 2 + 1]))) {
        selectBuilding({ type: 'tc', id: tc, owner: st.owners[tc] });
        playAudioCue('select');
      } else {
        const b = buildingAt(wx, wy);
        if (b && (b.owner !== foeOwner() || cellExploredW(b.x, b.y))) {
          selectBuilding({ type: 'building', id: b.id, kind: b.kind, owner: b.owner });
          playAudioCue('select');
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
    if (same.length) playAudioCue('select');
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
    if (repairArm) {
      repairArm = false;
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
  let acc = 0; // game-ms remainder (advanceTicks, play/netplay.js)
  let last = performance.now();
  let prevFrameAt = null;
  // Per-frame tick cap: a long hitch sheds its backlog instead of
  // spiralling (the pre-speed-fix loop capped at 5 ticks of 1.0x
  // time; 9 ticks is the same catch-up window in game time at
  // GAME_SPEED = 1.7).
  const MAX_TICKS_PER_FRAME = 9;
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
      updateEventFeed();
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
      // Game speed (gap 2): the accumulator converts wall time
      // to ticks at GAME_SPEED (DE Normal, 1.7x) — advanceTicks
      // is the one place that conversion lives, shared with the
      // netplay pacing and replay playback.
      const adv = advanceTicks(acc, Math.min(now - last, 250));
      acc = adv.accMs;
      const n = Math.min(adv.ticks, MAX_TICKS_PER_FRAME);
      if (adv.ticks > n) acc = 0; // shed the backlog, as the old cap did
      for (let i = 0; i < n; i++) sim.tick();
      // M5: Conquest check — the first decided tick raises the
      // end overlay and holds the sim (see showMatchEnd).
      if (builtMode !== 'sandbox') {
        const ms = sim.match_state();
        if (ms !== 0) showMatchEnd(ms);
      }
      updateEventFeed();
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


// --- Trees: damage stages, hit feedback, falling, stumps --------
// Forest readability pass (2026-10-20, Claudio's live-play
// feedback). Everything here is renderer-only presentation over
// the node's remaining-wood fraction — the same channel farm
// growth stages use. The sim's gather rate, wood per tree and
// node lifecycle are untouched; nothing here feeds back into a
// command or the hash.
//
//   frac > 2/3   intact widened pine (canopies overlap into a
//                forest mass; per-tree flip/scale/tint from a
//                position hash, so no two neighbours stamp alike)
//   1/3 .. 2/3   tree-dmg1 art (notch, ~30% foliage gone) + lean
//   frac < 1/3   tree-dmg2 art (deep notch, ~60% gone) + lean
//   depletion    client-side tip-over (~0.5 s, away from the last
//                cutter when known) leaving a fading stump
//                (~18 s, the rubble-decal pattern)
//   each axe hit the tree wobbles briefly and sheds wood chips
//                (pooled particles; LOW sheds fewer, never none
//                of the wobble)
const GATHER_IMPACT_FRAME = 2; // pose_gather's strike frame
const TREE_H = 5.6; // world units: widened art is ~0.76 aspect,
                    // so neighbours 4 wu apart overlap canopies
const FALL_MS = 500;
const STUMP_FADE_MS = 18000;
const treeKey = (x, y) => `${Math.round(x * 8)},${Math.round(y * 8)}`;
const treeWobble = new Map(); // key -> hit time (ms)
const treeLastHit = new Map(); // key -> {x, y} last cutter pos
const treePrev = new Map(); // key -> {x, y, frac} last frame
const fallingTrees = []; // {x, y, stage, ang, t0, stumpDone}
const stumpSites = []; // {x, y, t0}
const woodChips = []; // {x, y, vx, vy, t0, life, rot}
let treeSim = null;

function treeHash(x, y, salt) {
  let h = (Math.imul(Math.round(x * 8) + salt * 1013904223, 374761393)
    + Math.imul(Math.round(y * 8), 668265263)) >>> 0;
  h = (h ^ (h >> 13)) >>> 0;
  h = Math.imul(h, 1274126177) >>> 0;
  return ((h ^ (h >> 16)) >>> 0) / 4294967296;
}

// Fall direction (screen angle) for a tree at (x, y): away from
// its last cutter when a hit is on record, else a fixed
// per-position direction.
function treeFallAngle(x, y) {
  const hit = treeLastHit.get(treeKey(x, y));
  if (hit) {
    const dx = x - hit.x;
    const dy = y - hit.y;
    if (dx * dx + dy * dy > 0.01) return Math.atan2((dx + dy) / 2, dx - dy);
  }
  return treeHash(x, y, 7) * Math.PI * 2;
}

// One axe hit on a tree node (from updateVilAnim's impact frame).
// Deduped per tree per 140 ms gather-cycle step.
const treeHitCycle = new Map(); // key -> last cycle index fed
function noteTreeHit(node, vx, vy, now) {
  const key = treeKey(node.x, node.y);
  const cycle = Math.floor(now / 140);
  if (treeHitCycle.get(key) === cycle) return;
  treeHitCycle.set(key, cycle);
  treeWobble.set(key, now);
  treeLastHit.set(key, { x: vx, y: vy });
  // Wood chips from the trunk at axe height, flung away from the
  // cutter and up. Pooled + capped; LOW spawns fewer (the wobble
  // above is the information and is never tier-gated).
  const low = governor.level === 'low';
  const n = low ? 2 : 4;
  const dx = node.x - vx;
  const dy = node.y - vy;
  const len = Math.hypot(dx, dy) || 1;
  const ux = dx / len;
  const uy = dy / len;
  for (let k = 0; k < n; k++) {
    if (woodChips.length >= 240) woodChips.shift();
    const sp = 2.2 + treeHash(node.x + k, node.y, k + 3) * 2.4;
    woodChips.push({
      x: node.x + ux * 0.3, y: node.y + uy * 0.3, z: 1.15,
      vx: ux * sp + (treeHash(node.x, node.y + k, 11) - 0.5) * 1.6,
      vy: uy * sp + (treeHash(node.x, node.y + k, 13) - 0.5) * 1.6,
      vz: 2.6 + treeHash(node.x, node.y + k, 17) * 1.8,
      t0: now, life: low ? 380 : 520,
      rot: treeHash(node.x, node.y + k, 19) * 6.28,
    });
  }
}

// Per-frame tree bookkeeping: remember live trees; a tree that
// was here and is gone (depleted) starts its tip-over — but only
// on explored ground (VIS-06: memory, never invention).
function updateTreeTracking(nodes, now) {
  if (treeSim !== sim) {
    treeSim = sim;
    treePrev.clear();
    treeWobble.clear();
    treeLastHit.clear();
    treeHitCycle.clear();
    fallingTrees.length = 0;
    stumpSites.length = 0;
    woodChips.length = 0;
  }
  const live = new Set();
  for (const n of nodes) {
    if (n.kind !== 0 || n.amount <= 0) continue;
    const key = treeKey(n.x, n.y);
    live.add(key);
    treePrev.set(key, { x: n.x, y: n.y, frac: n.amount / NODE_MAX[0] });
  }
  for (const [key, prev] of treePrev) {
    if (live.has(key)) continue;
    treePrev.delete(key);
    if (fogAtWorld(prev.x, prev.y) === 0) continue;
    fallingTrees.push({
      x: prev.x, y: prev.y,
      stage: prev.frac > 2 / 3 ? 0 : prev.frac > 1 / 3 ? 1 : 2,
      ang: treeFallAngle(prev.x, prev.y), t0: now, stumpDone: false,
    });
  }
  // Finished falls become stumps.
  for (let k = fallingTrees.length - 1; k >= 0; k--) {
    const f = fallingTrees[k];
    if (now - f.t0 > FALL_MS && !f.stumpDone) {
      f.stumpDone = true;
      stumpSites.push({ x: f.x, y: f.y, t0: now });
    }
    if (now - f.t0 > FALL_MS + 120) fallingTrees.splice(k, 1);
  }
  for (let k = stumpSites.length - 1; k >= 0; k--) {
    if (now - stumpSites[k].t0 > STUMP_FADE_MS) stumpSites.splice(k, 1);
  }
}

// Tinted variants of a tree image (deterministic per-tree tint
// jitter, baked once into offscreen canvases — no per-frame
// filter cost). Buckets: 0 neutral, 1 warm, 2 cool.
const treeTintCache = new Map();
function treeTinted(im, bucket) {
  if (bucket === 0 || !spriteReady(im)) return im;
  let per = treeTintCache.get(im);
  if (!per) {
    per = [null, null, null];
    treeTintCache.set(im, per);
  }
  if (!per[bucket]) {
    const c = document.createElement('canvas');
    c.width = im.naturalWidth;
    c.height = im.naturalHeight;
    const g = c.getContext('2d');
    g.drawImage(im, 0, 0);
    g.globalCompositeOperation = 'source-atop';
    g.fillStyle = bucket === 1 ? 'rgba(255, 176, 84, 0.10)' : 'rgba(110, 150, 255, 0.10)';
    g.fillRect(0, 0, c.width, c.height);
    per[bucket] = c;
  }
  return per[bucket];
}

function treeStageImage(frac) {
  if (frac > 2 / 3) return NODE_ART.img.tree;
  if (frac > 1 / 3) return spriteReady(NODE_ART.img.treeDmg1) ? NODE_ART.img.treeDmg1 : NODE_ART.img.tree;
  return spriteReady(NODE_ART.img.treeDmg2) ? NODE_ART.img.treeDmg2 : NODE_ART.img.tree;
}

// Draw one tree image with the pass's transforms: base planted at
// (sx, sy) exactly (the trunk anchor never moves), variation
// scale/flip/lean applied around it, optional extra rotation for
// the falling animation.
function drawTreeImage(im, sx, sy, wx, wy, opts = {}) {
  const h = TREE_H * camS * (opts.scale ?? 1);
  const w = h * (im.naturalWidth / im.naturalHeight);
  ctx.save();
  ctx.translate(sx + (opts.ox || 0), sy);
  if (opts.rot) ctx.rotate(opts.rot);
  if (opts.flip) ctx.scale(-1, 1);
  if (opts.alpha != null) ctx.globalAlpha = opts.alpha;
  ctx.drawImage(im, -w / 2, -h, w, h);
  ctx.restore();
}

function drawTree(sx, sy, frac = 1, wx = 0, wy = 0) {
  const u = camS;
  shadow(sx, sy, 1.5 * u);
  const key = treeKey(wx, wy);
  const stage = frac > 2 / 3 ? 0 : frac > 1 / 3 ? 1 : 2;
  // Deterministic per-tree variation from the position hash.
  const flip = treeHash(wx, wy, 1) < 0.5;
  const scale = 0.92 + treeHash(wx, wy, 2) * 0.16; // ±8%
  const bucket = Math.floor(treeHash(wx, wy, 3) * 3);
  // Damage lean: grows with the stage, toward the fall side.
  const lean = (stage === 1 ? 0.028 : stage === 2 ? 0.062 : 0)
    * (Math.cos(treeFallAngle(wx, wy)) >= 0 ? 1 : -1);
  // Hit wobble: a short decaying shake after the last axe impact.
  let ox = 0;
  let wob = 0;
  const hitAt = treeWobble.get(key);
  if (hitAt != null) {
    const age = performance.now() - hitAt;
    if (age < 240) {
      const d = 1 - age / 240;
      ox = Math.sin(age / 22) * 0.09 * u * d;
      wob = Math.sin(age / 22) * 0.02 * d;
    }
  }
  const im = treeStageImage(frac);
  if (spriteReady(im)) {
    drawTreeImage(treeTinted(im, bucket), sx, sy, wx, wy,
      { flip, scale, rot: lean + wob, ox });
    return;
  }
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

// A falling tree mid-tip-over (spawned by updateTreeTracking).
function drawFallingTree(f, now) {
  const im = treeStageImage(f.stage === 0 ? 1 : f.stage === 1 ? 0.5 : 0.2);
  if (!spriteReady(im)) return;
  const [sx, sy] = worldToScreen(f.x, f.y);
  const p = Math.min(1, (now - f.t0) / FALL_MS);
  const ease = p * p * (3 - 2 * p);
  // Rotate the tree's up-vector toward the fall direction.
  let target = f.ang + Math.PI / 2;
  while (target > Math.PI) target -= Math.PI * 2;
  while (target < -Math.PI) target += Math.PI * 2;
  const rot = target * ease * 0.94;
  const alpha = p > 0.6 ? 1 - (p - 0.6) / 0.4 * 0.55 : 1;
  const flip = treeHash(f.x, f.y, 1) < 0.5;
  const scale = 0.92 + treeHash(f.x, f.y, 2) * 0.16;
  drawTreeImage(im, sx, sy, f.x, f.y, { flip, scale, rot, alpha });
}

// The stump a felled tree leaves: a short trunk stub with a pale
// fresh-cut top, drawn in the node family's vector language.
function drawStump(site, alpha) {
  const [sx, sy] = worldToScreen(site.x, site.y);
  const u = camS;
  ctx.save();
  ctx.globalAlpha = alpha;
  shadow(sx, sy, 0.9 * u);
  ctx.fillStyle = '#6b4a2b';
  ctx.beginPath();
  ctx.moveTo(sx - 0.30 * u, sy);
  ctx.lineTo(sx - 0.22 * u, sy - 0.78 * u);
  ctx.lineTo(sx + 0.22 * u, sy - 0.78 * u);
  ctx.lineTo(sx + 0.30 * u, sy);
  ctx.closePath();
  ctx.fill();
  ctx.fillStyle = '#d9b380'; // fresh-cut top
  ctx.beginPath();
  ctx.ellipse(sx, sy - 0.78 * u, 0.22 * u, 0.09 * u, 0, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

// Wood chips in flight (screen-projected from world each frame;
// gravity in world units). Drawn after the fog dim with the
// click markers — live action feedback stays readable.
function drawWoodChips(now) {
  for (let k = woodChips.length - 1; k >= 0; k--) {
    const c = woodChips[k];
    const age = now - c.t0;
    if (age > c.life) {
      woodChips.splice(k, 1);
      continue;
    }
    const t = age / 1000;
    const wx = c.x + c.vx * t;
    const wy = c.y + c.vy * t;
    const wz = Math.max(0, c.z + c.vz * t - 4.4 * t * t);
    const [sx, sy] = worldToScreen(wx, wy);
    const s = Math.max(1.6, camS * 0.16);
    ctx.save();
    ctx.globalAlpha = 1 - (age / c.life) * 0.5;
    ctx.translate(sx, sy - wz * camS);
    ctx.rotate(c.rot + age / 90);
    ctx.fillStyle = (k % 3 === 0) ? '#d9b380' : '#8a6238';
    ctx.fillRect(-s / 2, -s / 4, s, s / 2);
    ctx.restore();
  }
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
const TC_HALF = 8; // world units: TC footprint half-extent (4x4 cells x 4 wu, BLD-16)
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

// --- Building fire, smoke and rubble (milestone §10.3) -----------
// The documented damage stages, nothing more: smoke at HP <= 75%,
// flames at HP <= 50% (the flame clusters grow with damage inside
// the stage — presentation, not a third threshold). All original
// procedural canvas art, drawn in the building's own painter pass.
function flamePath(cx, baseY, w, h, sway, color, alpha) {
  ctx.fillStyle = color;
  ctx.globalAlpha = alpha;
  ctx.beginPath();
  ctx.moveTo(cx - w, baseY);
  ctx.quadraticCurveTo(cx - w * 0.9, baseY - h * 0.55, cx + sway, baseY - h);
  ctx.quadraticCurveTo(cx + w * 0.9, baseY - h * 0.55, cx + w, baseY);
  ctx.closePath();
  ctx.fill();
  ctx.globalAlpha = 1;
}

function drawBuildingFireAt(sx, baseY, sizeTiles, hpFrac, seed) {
  if (hpFrac > 0.75 || hpFrac <= 0) return;
  const now = performance.now();
  const u = camS;
  // Smoke: three puffs rising and thinning, phase-offset per site.
  for (let k = 0; k < 3; k++) {
    const cyc = ((now / 1600) + seed * 0.13 + k * 0.37) % 1;
    const px = sx + Math.sin(seed * 7 + k * 2.4) * sizeTiles * 0.45 * u
      + Math.sin(cyc * 5 + seed) * 0.4 * u;
    const py = baseY - cyc * 3.4 * u;
    const r = (0.55 + cyc * 1.25) * u * (0.7 + sizeTiles * 0.16);
    ctx.fillStyle = `rgba(46,44,48,${(0.32 * (1 - cyc)).toFixed(3)})`;
    ctx.beginPath();
    ctx.arc(px, py, r, 0, Math.PI * 2);
    ctx.fill();
  }
  if (hpFrac > 0.5) return; // smoke stage only
  const dmg = (0.5 - hpFrac) / 0.5; // 0..1 inside the flame stage
  const n = 2 + Math.round(dmg * (sizeTiles + 1));
  const spread = sizeTiles * 1.6 * u;
  for (let k = 0; k < n; k++) {
    const fx = sx + ((k / (n - 1)) - 0.5) * spread;
    const fh = (1.15 + dmg * 1.5) * u
      * (0.8 + 0.25 * Math.sin(now / 87 + seed * 9 + k * 1.7));
    const fw = fh * 0.42;
    const sway = Math.sin(now / 130 + seed * 5 + k) * 0.12 * u;
    flamePath(fx, baseY + 0.25 * u, fw, fh, sway, '#e8641c', 0.85);
    flamePath(fx, baseY + 0.25 * u, fw * 0.66, fh * 0.72, sway * 0.6, '#f59b23', 0.9);
    flamePath(fx, baseY + 0.25 * u, fw * 0.36, fh * 0.45, sway * 0.3, '#ffd94a', 0.95);
  }
}

// Rubble: one procedural heap image per footprint class, kept at
// the site fading out (adopted ~20 s, UNVERIFIED duration — the
// manual says only "eventually disappear"). The sim footprint
// frees at destruction; the decal is pure presentation over free
// ground, fog-gated like the building it replaces.
const RUBBLE_FADE_MS = 20000;
const rubbleImgCache = {};
let rubbleSim = null;
const rubbleSeen = new Map(); // building id -> {x, y, size, owner}
const rubbleTCSeen = new Map(); // entity index -> {x, y, owner}
const rubbleSites = []; // {x, y, size, owner, t0}

function rubbleImage(size) {
  if (rubbleImgCache[size]) return rubbleImgCache[size];
  const c = document.createElement('canvas');
  c.width = 168; c.height = 88;
  const g = c.getContext('2d');
  let rngState = 1234 + size * 777;
  const rnd = () => {
    rngState = (rngState * 1103515245 + 12345) & 0x7fffffff;
    return rngState / 0x7fffffff;
  };
  // The mound.
  const grad = g.createLinearGradient(0, 20, 0, 88);
  grad.addColorStop(0, '#6e5c49');
  grad.addColorStop(1, '#453a2f');
  g.fillStyle = grad;
  g.beginPath();
  g.ellipse(84, 66, 80, 24, 0, Math.PI, 0);
  g.fill();
  // Scattered chunks.
  for (let k = 0; k < 16; k++) {
    const rx = 14 + rnd() * 140;
    const ry = 52 + rnd() * 28;
    const rw = 5 + rnd() * 11;
    const rh = 3 + rnd() * 6;
    g.fillStyle = ['#7d6a54', '#5d4f3e', '#8a7a62', '#4c4133'][k % 4];
    g.fillRect(rx, ry, rw, rh);
  }
  // Two broken beams.
  g.strokeStyle = '#3d2c1c';
  g.lineWidth = 5;
  g.beginPath();
  g.moveTo(38, 66);
  g.lineTo(74, 40);
  g.stroke();
  g.beginPath();
  g.moveTo(128, 68);
  g.lineTo(102, 44);
  g.stroke();
  rubbleImgCache[size] = c;
  return c;
}

function drawRubble(site, alpha) {
  const im = rubbleImage(site.size);
  const [sx, sy] = worldToScreen(site.x, site.y);
  const w = site.size * CS * 1.06 * camS;
  const h = w * (im.height / im.width);
  ctx.globalAlpha = alpha;
  ctx.drawImage(im, sx - w / 2, sy - h + h * 0.16, w, h);
  ctx.globalAlpha = 1;
}

// Per-draw bookkeeping: a complete building (or Town Center) that
// was renderable and is now gone leaves a rubble site. Enemy sites
// spawn only when the ground is currently visible — re-scouting a
// razed building reveals its rubble, never before.
function updateRubble(buildings, st) {
  if (rubbleSim !== sim) {
    rubbleSim = sim;
    rubbleSeen.clear();
    rubbleTCSeen.clear();
    rubbleSites.length = 0;
  }
  const now = performance.now();
  const live = new Set();
  for (const b of buildings) {
    live.add(b.id);
    if (b.kind === 1 || !b.complete) continue; // farms leave no rubble
    if (b.owner !== foeOwner() || fogAtWorld(b.x, b.y) === 2) {
      rubbleSeen.set(b.id, { x: b.x, y: b.y, size: b.size, owner: b.owner });
    }
  }
  for (const [id, site] of rubbleSeen) {
    if (live.has(id)) continue;
    if (site.owner !== foeOwner() || fogAtWorld(site.x, site.y) === 2) {
      rubbleSites.push({ ...site, t0: now });
      rubbleSeen.delete(id);
    }
  }
  for (let i = 0; i < st.count; i++) {
    if (st.kinds[i] !== KIND_TC) continue;
    const x = st.pos[i * 2], y = st.pos[i * 2 + 1];
    if (st.hp[i * 2] >= 1) {
      rubbleTCSeen.set(i, { x, y, owner: st.owners[i] });
    } else if (rubbleTCSeen.has(i)) {
      const site = rubbleTCSeen.get(i);
      rubbleTCSeen.delete(i);
      if (site.owner !== foeOwner() || fogAtWorld(x, y) >= 1) {
        rubbleSites.push({ ...site, size: 4, t0: now });
      }
    }
  }
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
  // Fire/smoke by damage fraction (milestone §10.3): complete,
  // non-farm buildings only — a foundation has nothing to burn.
  if (b.complete && b.kind !== 1) {
    const [fx, fy] = worldToScreen(b.x, b.y);
    drawBuildingFireAt(fx, fy - (b.size === 3 ? 5.3 : 4.2) * camS,
      b.size, b.hpFrac, b.id * 1.71);
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
function drawSelRing(sx, sy, rWorld, color) {
  ctx.strokeStyle = color || '#ffcf4d';
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
  // Directional sheet first (the milestone standard); below it the
  // generated side-view frames, then the CC0 placeholder -> vector
  // chain, unchanged.
  const orig = drawOriginalVillager(anim, sx, sy);
  const ua = orig ? null : unitArtImage('villager', i, wx, wy);
  const art = ua ? drawUnitPose(ua.im, sx, sy, UNIT_ART_H.villager, ua.flip) : false;
  const spr = art || orig || drawSpriteH(SPRITES.villager, sx, sy, 2.4);
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
    // With a carry drawing the load is on the unit itself, so the
    // kind-colored dot rides above the bundle; otherwise above the head.
    const dotH = art ? (ua.action === 'carry' ? 3.05 : 2.8)
      : orig ? (anim.state === 'carry' ? 3.28 : 2.6) : spr ? 2.8 : 2.35;
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
  const act = unitActionFor('scout', i, wx, wy);
  if (drawDirUnit('scout', act, sx, sy)) return;
  const ua = unitArtImage('scout', i, wx, wy, act);
  if (ua && drawUnitPose(ua.im, sx, sy, UNIT_ART_H.scout, ua.flip)) return;
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
  drawFootman(i, wx, wy, 'militia');
}
function drawManAtArms(i, wx, wy) {
  drawFootman(i, wx, wy, 'maa');
}
// Shared draw for the two sword footmen; `unit` picks the art set
// (the Man-at-Arms wears the heavier armor in his own frames; the
// vector fallback below is the same body for both, as before).
function drawFootman(i, wx, wy, unit) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 0.95 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.25);
  const act = unitActionFor(unit, i, wx, wy);
  if (drawDirUnit(unit, act, sx, sy)) return;
  const ua = unitArtImage(unit, i, wx, wy, act);
  if (ua && drawUnitPose(ua.im, sx, sy, UNIT_ART_H[unit], ua.flip)) return;
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
  const act = unitActionFor('archer', i, wx, wy);
  if (drawDirUnit('archer', act, sx, sy)) return;
  const ua = unitArtImage('archer', i, wx, wy, act);
  if (ua && drawUnitPose(ua.im, sx, sy, UNIT_ART_H.archer, ua.flip)) return;
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
    (ANIMAL_POSE_STATE[i] = { x: wx, y: wy, face: -1, dir: 2, stamp: -1, pose: null });
  if (st.stamp === poseStamp && st.pose) return st.pose;
  const dx = wx - st.x;
  const dy = wy - st.y;
  const moving = Math.abs(dx) + Math.abs(dy) > 0.0001;
  if (moving) {
    const sdx = dx - dy;
    if (sdx > 0.0001) st.face = 1;
    else if (sdx < -0.0001) st.face = -1;
    if (Math.hypot(dx, dy) > 0.004) {
      const d = Math.round(Math.atan2(dy, dx) / (Math.PI / 4));
      st.dir = ((d % 8) + 8) % 8;
    }
  }
  st.x = wx;
  st.y = wy;
  st.pose = { moving, face: st.face, dir: st.dir };
  st.stamp = poseStamp;
  return st.pose;
}
// The pose image for one animal this frame, plus its flip flag.
// kind is the ANIMAL_ART key prefix: 'sheep' | 'boar' | 'deer'.
// The directional-sheet action for an animal this draw: walk
// while it moves, the graze cycle on the same slow phase the
// side-view poses use, idle otherwise — with the world facing.
function dirAnimalAct(i, wx, wy) {
  const pose = animalPoseFor(i, wx, wy);
  const tick = sim.tick_count();
  if (pose.moving) return { action: 'walk', dir: pose.dir };
  if ((Math.floor(tick / 240) + i) % 5 === 0) return { action: 'graze', dir: pose.dir };
  return { action: 'idle', dir: pose.dir };
}

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

// The owner band on a sheep: an ellipse stroke around the torso
// in the owner's colour — the "lenço na cor do dono" (Claudio,
// 2026-10-20) — drawn over whichever sprite path rendered the
// animal, so it reads at game zoom from every facing. A null
// colour (a wild Gaia sheep) draws nothing: unowned, unmarked.
function drawSheepBand(sx, sy, u, color) {
  if (!color) return;
  ctx.strokeStyle = color;
  ctx.lineWidth = Math.max(1.5, 0.30 * u);
  ctx.beginPath();
  ctx.ellipse(sx, sy - 1.02 * u, 0.80 * u, 0.36 * u, 0, 0, Math.PI * 2);
  ctx.stroke();
}

function drawSheep(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  // The sheep's owner as a colour: blue for our herd, red for the
  // enemy's, none while it is wild. (Client owner tags: 0 = the
  // human seat, 1 = Gaia, 2 = the opponent.)
  const ownerColor = drawOwner === 1 ? null
    : (drawOwner === foeOwner() ? ENEMY_COLOR : OWN_COLOR);
  shadow(sx, sy, 0.9 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.2, ownerColor);
  else if (selectedAnimal === i) drawSelRing(sx, sy, 1.2);
  if (drawDirUnit('sheep', dirAnimalAct(i, wx, wy), sx, sy)) {
    // The owner band applies on this path too (see drawSheepBand).
    drawSheepBand(sx, sy, u, ownerColor);
    return;
  }
  const sheepPose = animalPoseImage('sheep', i, wx, wy);
  if (drawAnimalPose(sheepPose.im, sx, sy, 2.0, sheepPose.flip)) {
    // The owner band applies on this path too (see drawSheepBand).
    drawSheepBand(sx, sy, u, ownerColor);
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
  drawSheepBand(sx, sy, u, ownerColor);
}

function drawBoar(i, wx, wy) {
  const [sx, sy] = worldToScreen(wx, wy);
  const u = camS;
  shadow(sx, sy, 1.25 * u);
  if (selection.has(i)) drawSelRing(sx, sy, 1.55);
  if (drawDirUnit('boar', dirAnimalAct(i, wx, wy), sx, sy)) return;
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
  if (drawDirUnit('deer', dirAnimalAct(i, wx, wy), sx, sy)) return;
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

// --- Unit animation art (UNIT_ART sets) -----------------------------
// Same inference model as the animals: the renderer gets positions
// every frame and no velocity or order state, so movement/facing
// come from the per-entity position delta, and work states come
// from the heuristics the client already maintains — carryKind (a
// villager hauling a load) and atNodeNow (a villager at a resource
// node) — plus two snapshots refreshed once per draw in
// updateUnitAnimState: atBuildNow (a villager beside its owner's
// unfinished building) and archerShoot (an archer whose arrow was
// just seen spawning). Melee attack is inferred by adjacency: a
// footman standing beside a living non-owned entity is mid-fight.
// Presentation only, never sim input.
const UNIT_ART_H = { villager: 2.55, archer: 2.55, militia: 2.6, maa: 2.65, scout: 3.1 };
const UNIT_POSE_STATE = {};
// Incremented once per draw() pass: the pose helpers memoize on it
// so the directional draw and the side-view fallback read ONE
// position delta per entity per pass (a second read would see a
// zero delta and report the unit as still).
let poseStamp = 0;
const atBuildNow = new Set();
const archerShoot = new Map(); // entity id -> {tick, face}
const seenProjKeys = new Set();
let frameSt = null; // the entityState() snapshot of the running draw

function unitPoseFor(i, wx, wy) {
  const st = UNIT_POSE_STATE[i] ||
    (UNIT_POSE_STATE[i] = { x: wx, y: wy, face: -1, dir: 2, stamp: -1, pose: null });
  if (st.stamp === poseStamp && st.pose) return st.pose;
  const dx = wx - st.x;
  const dy = wy - st.y;
  const moving = Math.abs(dx) + Math.abs(dy) > 0.0001;
  if (moving) {
    const sdx = dx - dy;
    if (sdx > 0.0001) st.face = 1;
    else if (sdx < -0.0001) st.face = -1;
    if (Math.hypot(dx, dy) > 0.004) {
      // Facing for the directional sheets: the world movement
      // angle quantized to the 8 rows (atan2 convention shared
      // with the villager sheets, updateVilAnim).
      const d = Math.round(Math.atan2(dy, dx) / (Math.PI / 4));
      st.dir = ((d % 8) + 8) % 8;
    }
  }
  st.x = wx;
  st.y = wy;
  st.pose = { moving, face: st.face, dir: st.dir };
  st.stamp = poseStamp;
  return st.pose;
}

// Per-draw refresh of the two inferred states the pose picker
// needs beyond carryKind/atNodeNow. Called from draw() with the
// frame's building list and entity snapshot.
function updateUnitAnimState(buildings, st) {
  atBuildNow.clear();
  const pos = st.pos;
  for (let i = 0; i < st.count; i++) {
    if (st.kinds[i] !== 0 || st.hp[i * 2] < 1) continue; // villagers only
    const wx = pos[i * 2];
    const wy = pos[i * 2 + 1];
    for (const b of buildings) {
      if (b.complete || b.owner !== st.owners[i]) continue;
      const rr = (b.size * CS) / 2 + 3.0;
      const dx = b.x - wx;
      const dy = b.y - wy;
      if (dx * dx + dy * dy <= rr * rr) {
        atBuildNow.add(i);
        break;
      }
    }
  }
  // Archer releases: a projectile whose spawn key was not in the
  // previous draw is new; its start point is the shooter's position
  // at release (the sim's only ranged unit is the Archer).
  const proj = sim.projectiles();
  const now = sim.tick_count();
  const live = new Set();
  for (let p = 0; p < proj.length; p += 6) {
    const key = proj[p + 4] + ':' + proj[p] + ':' + proj[p + 1];
    live.add(key);
    if (seenProjKeys.has(key)) continue;
    seenProjKeys.add(key);
    if (now - proj[p + 4] > 3) continue; // first sighting, but stale
    let best = -1;
    let bestD2 = 2.0 * 2.0;
    for (let i = 0; i < st.count; i++) {
      if (st.kinds[i] !== 4 || st.hp[i * 2] < 1) continue;
      const dx = pos[i * 2] - proj[p];
      const dy = pos[i * 2 + 1] - proj[p + 1];
      const d2 = dx * dx + dy * dy;
      if (d2 < bestD2) {
        bestD2 = d2;
        best = i;
      }
    }
    if (best >= 0) {
      const sdx = (proj[p + 2] - proj[p]) - (proj[p + 3] - proj[p + 1]);
      const wdir = Math.round(Math.atan2(proj[p + 3] - proj[p + 1],
        proj[p + 2] - proj[p]) / (Math.PI / 4));
      archerShoot.set(best, {
        tick: proj[p + 4], face: sdx > 0 ? 1 : -1,
        dir: ((wdir % 8) + 8) % 8,
      });
    }
  }
  if (seenProjKeys.size > 1024) {
    for (const k of seenProjKeys) if (!live.has(k)) seenProjKeys.delete(k);
  }
}

// A living entity that is not i's owner's, close enough to be the
// footman's melee opponent (an enemy unit, or a Gaia animal — a
// boar being fought reads exactly the same).
function foeNearUnit(i, wx, wy) {
  if (!frameSt) return false;
  const st = frameSt;
  const pos = st.pos;
  for (let j = 0; j < st.count; j++) {
    if (j === i || st.hp[j * 2] < 1) continue;
    if (st.owners[j] === st.owners[i] || st.kinds[j] === KIND_TC) continue;
    const dx = pos[j * 2] - wx;
    const dy = pos[j * 2 + 1] - wy;
    if (dx * dx + dy * dy <= 2.3 * 2.3) return true;
  }
  return false;
}

// A UNIT_ART image is drawable when loaded; the enemy variant is a
// palette-swap CANVAS, which spriteReady (an <img> test) would
// reject — accept either shape, like drawOriginalVillager does.
function unitArtReady(im) {
  return !!im && (im.naturalWidth > 0 || im.width > 0);
}
function unitArtPick(unit, action, n) {
  const key = unit + action[0].toUpperCase() + action.slice(1) + n;
  const red = drawOwner === foeOwner() ? UNIT_ART.imgRed[key] : null;
  const im = red || UNIT_ART.img[key];
  return unitArtReady(im) ? im : null;
}

// The frame for one unit this draw: {im, flip, action} or null when
// the chosen action has no art yet (the caller falls back to the
// previous drawing for that unit).
// The inferred action for one unit this draw: {action, n, face,
// dir, shotTick}. Shared by the directional-sheet draw (which adds
// the world facing `dir`) and the side-view picker below; `n` is
// the side-view frame number, the sheet frame is derived from the
// action + tick in dirSheetFrame.
function unitActionFor(unit, i, wx, wy) {
  const pose = unitPoseFor(i, wx, wy);
  const tick = sim.tick_count();
  let action = 'idle';
  let n = 1;
  let face = pose.face;
  let dir = pose.dir;
  let shotTick = -1;
  if (unit === 'villager') {
    if (pose.moving) {
      if (carryKind.has(i)) {
        action = 'carry';
        n = (Math.floor(tick / 7) + i) % 2 + 1;
      } else {
        action = 'walk';
        n = (Math.floor(tick / 7) + i) % 4 + 1;
      }
    } else if (atNodeNow.has(i)) {
      action = 'gather';
      n = Math.floor(tick / 8) % 3 + 1;
    } else if (atBuildNow.has(i)) {
      action = 'build';
      n = Math.floor(tick / 6) % 2 + 1;
    }
  } else if (unit === 'scout') {
    if (pose.moving) {
      action = 'gallop';
      n = (Math.floor(tick / 6) + i) % 4 + 1;
    }
  } else if (unit === 'archer') {
    if (pose.moving) {
      action = 'walk';
      n = (Math.floor(tick / 7) + i) % 4 + 1;
    } else {
      const shot = archerShoot.get(i);
      if (shot && tick - shot.tick <= 18) {
        action = 'shoot';
        n = Math.min(3, Math.floor((tick - shot.tick) / 6) + 1);
        face = shot.face; // the arrow's screen direction, not the last walk facing
        dir = shot.dir; // ...and its world direction, for the sheets
        shotTick = shot.tick;
      }
    }
  } else { // militia | maa
    if (pose.moving) {
      action = 'walk';
      n = (Math.floor(tick / 7) + i) % 4 + 1;
    } else if (foeNearUnit(i, wx, wy)) {
      action = 'attack';
      n = Math.floor(tick / 7) % 3 + 1;
    }
  }
  return { action, n, face, dir, shotTick };
}

function unitArtImage(unit, i, wx, wy, pre) {
  const a = pre || unitActionFor(unit, i, wx, wy);
  const im = unitArtPick(unit, a.action, a.n);
  return im ? { im, flip: a.face > 0, action: a.action } : null;
}

// Draw a unit-art frame bottom-anchored at (sx, sy), hWorld tall,
// flipped horizontally when flip is set. False when not drawable.
function drawUnitPose(im, sx, sy, hWorld, flip) {
  if (!unitArtReady(im)) return false;
  const h = hWorld * camS;
  const w = h * ((im.naturalWidth || im.width) / (im.naturalHeight || im.height));
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

// An arrow in flight, centered at (cx, cy) and rotated to `ang`
// (its screen-space direction of travel): wooden shaft, steel
// triangular head, and — when `detailed` — pale fletching barbs at
// the tail. Replaces the M3 tail-line/dot marks.
function drawArrowShape(cx, cy, ang, detailed) {
  const u = camS;
  const hx = Math.cos(ang);
  const hy = Math.sin(ang);
  const px = -hy;
  const py = hx;
  const half = 0.62 * u; // half the arrow's length
  const tipX = cx + hx * half;
  const tipY = cy + hy * half;
  const tailX = cx - hx * half;
  const tailY = cy - hy * half;
  const hbX = cx + hx * half * 0.45; // the head's base center
  const hbY = cy + hy * half * 0.45;
  ctx.strokeStyle = '#a97c46'; // shaft
  ctx.lineWidth = Math.max(1, u * 0.1);
  ctx.beginPath();
  ctx.moveTo(tailX, tailY);
  ctx.lineTo(hbX, hbY);
  ctx.stroke();
  ctx.fillStyle = '#d9d9d9'; // head
  const hw = 0.13 * u;
  ctx.beginPath();
  ctx.moveTo(tipX, tipY);
  ctx.lineTo(hbX + px * hw, hbY + py * hw);
  ctx.lineTo(hbX - px * hw, hbY - py * hw);
  ctx.closePath();
  ctx.fill();
  if (detailed) {
    ctx.strokeStyle = '#e8ddc8'; // fletching
    ctx.lineWidth = Math.max(1, u * 0.07);
    for (const s of [-1, 1]) {
      ctx.beginPath();
      ctx.moveTo(tailX, tailY);
      ctx.lineTo(tailX + (hx * 0.28 + px * s * 0.16) * u,
        tailY + (hy * 0.28 + py * s * 0.16) * u);
      ctx.stroke();
    }
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
// Generated-audio world state: the previous frame's own-seat HP
// map (entities + buildings) behind the under-attack cue, and
// the housed edge. Seeded silently on the first frame of a
// match (resetAudioWorldState, next to the feed's reset).
let audioPrevHp = null;
let audioHousedPrev = false;

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
    resAge.textContent = AGE_NAMES[age] || AGE_NAMES[0];
  }
  const pop = myPop();
  const cap = myPopCap();
  resPop.textContent = `${pop} / ${cap}`;
  // Warn tint when housed (reason 3) or simply at the cap.
  const housedNow = myTrainBlock() === 3 || pop >= cap;
  resPop.classList.toggle('warn', housedNow);
  // Idle indicators (INF-03): live counts from the sim's idle
  // queries; the buttons light up while anyone is standing free.
  const nVil = myIdleVillagers().length;
  if (nVil !== lastIdleVil) {
    // Generated audio: a villager BECOMING idle earns at most
    // one very soft tick (the engine throttles it to one per
    // 20 s, and the count only rising counts — never a nag).
    if (lastIdleVil >= 0 && nVil > lastIdleVil) playAudioCue('idleTick');
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
  updateAudioWorldState(housedNow);
}

/// Generated audio from own-seat state transitions: the housed
/// edge, and HP dropping anywhere in our own village (units,
/// the Town Center, buildings) — the under-attack warning. All
/// of it reads state the client already holds for its own seat
/// (fog-fair by construction), only while a match is being
/// played (never during replay playback), and the engine's
/// gate hard-throttles the attack cue to one per 9 s.
function updateAudioWorldState(housedNow) {
  if (!sim || playback) return;
  if (housedNow && !audioHousedPrev) playAudioCue('housed');
  audioHousedPrev = housedNow;
  const hp = new Map();
  const st = entityState();
  for (let i = 0; i < st.count; i++) {
    if (st.owners[i] === localOwner) hp.set('e' + i, st.hp[i * 2]);
  }
  for (const b of buildingList()) {
    if (b.owner === localOwner) hp.set('b' + b.id, b.hpFrac);
  }
  if (audioPrevHp) {
    for (const [key, value] of hp) {
      const before = audioPrevHp.get(key);
      if (before != null && value < before - 1e-9) {
        playAudioCue('underAttack');
        break;
      }
    }
  }
  audioPrevHp = hp;
}

/// One training attempt for a train button; true when the sim
/// (or the lockstep session) accepted the order. Extracted from
/// runCommandButton so the Shift repeat below runs the identical
/// order a plain click runs — never a parallel path.
function attemptTrain(id) {
  if (id === 'train') return issue({ t: 'train_vil' });
  const kindTag = { 'train-scout': 2, 'train-militia': 3, 'train-archer': 4 }[id];
  if (kindTag == null) return false;
  return selectedBuilding && selectedBuilding.type === 'building'
    ? issue({ t: 'train_at', b: selectedBuilding.id, k: kindTag })
    : false;
}

/// The refusal flash a single train click produces (the wording
/// is unchanged); shown when a train run queued nothing at all.
function flashTrainRefusal(id) {
  if (id === 'train') {
    const reason = myTrainBlock();
    flash(['', t('train.noFoodVillager'),
      t('block.queueFull'), t('train.housedShort')][reason] || t('block.cannotTrain'));
    return;
  }
  if (selectedBuilding) {
    const kindTag = { 'train-scout': 2, 'train-militia': 3, 'train-archer': 4 }[id];
    const reason = sim.building_train_block(selectedBuilding.id, kindTag);
    flash(['', t('train.noResourcesUnit'),
      t('block.queueFull'), t('train.housedShort')][reason] || t('block.cannotTrain'));
  }
}

// Shift+train x5 (2026-10-19 study fix, gap 4): DE queues five
// of a unit when Shift is held on its train button or hotkey.
// The repeat is deliberately dumb — the same issue() a plain
// click makes, up to five times, stopping at the first refusal;
// food, queue and housing rules stay the sim's. Research
// buttons are not train buttons and never repeat.
const SHIFT_TRAIN_COUNT = 5;

/// Run one command-panel button by id — the single implementation
/// behind the panel's delegated click listener AND the M13
/// hotkeys, so a key press and a click can never drift apart.
/// Callers gate on the button not being disabled (the listener
/// checks the element; the hotkey path checks it too). `times`
/// is the Shift+train repeat count (SHIFT_TRAIN_COUNT with Shift
/// held, 1 otherwise); only train buttons honor it.
function runCommandButton(id, times = 1) {
  if (id === 'train' || id === 'train-scout'
    || id === 'train-militia' || id === 'train-archer') {
    let done = 0;
    while (done < times && attemptTrain(id)) done++;
    if (done === 0) flashTrainRefusal(id);
    else playAudioCue(id === 'train' ? 'orderVillager' : 'orderMilitary');
    return;
  }
  if (id === 'page-eco' || id === 'page-mil') {
    // UI FIX 4 addendum: the root openers arm the very page the
    // Q / W hotkeys arm — one shared buildPage state drives the
    // rendered grid for mouse and keyboard alike.
    buildPage = id === 'page-eco' ? 'eco' : 'mil';
    repairArm = false;
    playAudioCue('click');
  } else if (id === 'page-back') {
    placement = null;
    repairArm = false;
    buildPage = null; // back to the root page
    playAudioCue('click');
  } else if (id === 'auto-explore') {
    // SEL-17: the selected scouts' AutoExplore toggle — the same
    // order the G key issues (toggleAutoExplore plays the cue).
    toggleAutoExplore();
  } else if (id === 'stop') {
    // SEL-14: halt every selected unit where it stands.
    stopSelectedUnits();
  } else if (id === 'delete') {
    // SEL-14: the panel Delete is the Shift+Delete form —
    // every selected unit.
    deleteSelectedUnits(true);
  } else if (id === 'delete-building') {
    // SEL-14 + BLD-02: destroy the selected own building.
    deleteSelectedBuilding();
  } else if (id === 'repair') {
    // BLD-17: arm (or disarm) Repair targeting — the click that
    // follows picks the building, exactly like build placement.
    repairArm = !repairArm;
    if (repairArm) {
      placement = null;
      flash(t('flash.repairArm'));
    }
    playAudioCue('click');
  } else if (id.startsWith('build-')) {
    const kind = Number(id.slice(6));
    repairArm = false;
    placement = placement && placement.kind === kind ? null : { kind };
    playAudioCue(placement ? 'orderBuilding' : 'click');
    // The page STAYS armed while placement runs (UI FIX 4
    // addendum cancel rule): cancelling placement — Esc, a
    // right-click, re-clicking the building — lands back on the
    // page the building came from; only a further Esc, a
    // right-click on the open page, or Back returns to the root.
  } else if (id === 'research-loom' || id === 'research-feudal') {
    // M6: Town Center research (shared queue with training).
    const tag = id === 'research-loom' ? 0 : 1;
    const ok = issue({ t: 'research_tc', tech: tag });
    if (!ok) {
      const rb = myResearchBlockTc(tag);
      flash(tag === 1 && rb === 4
        ? t('cmd.feudalNeeds')
        : RESEARCH_BLOCK_TITLES[rb] || t('block.cannotResearch'));
    } else {
      playAudioCue('orderBuilding');
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
          || t('block.cannotResearch'));
      } else {
        playAudioCue('orderBuilding');
      }
    }
  }
}

function cmdButton({ id, icon, img, label, costIcon, cost, cost2Icon, cost2, title, disabled, active, key }) {
  const btn = document.createElement('button');
  btn.type = 'button';
  btn.id = id;
  btn.className = 'cmd-btn' + (active ? ' active' : '');
  btn.disabled = !!disabled;
  // UI FIX 4 (Claudio, 2026-10-19): the DE command button is
  // JUST a square with its icon — no name and no price printed
  // on the face. Name and full cost (both resources, where two
  // apply) live in the hover tooltip: every call site's title
  // states them (e.g. 'Train Militia — 50 food · 20 gold').
  // The label/cost arguments stay in the signature because the
  // call sites pass them alongside the title; the face renders
  // only the glyph and the hotkey letter at its top-left corner.
  // NO indicators on the face (Claudio, 2026-10-19 addendum):
  // the production queue count is NOT a badge here — it lives
  // where DE puts it, in the selection panel's info line
  // (queue length + progress %), which updateSelPanel renders.
  btn.title = title;
  const glyph = img
    ? pngIcon(img, 'cmd-icon')
    : `<svg class="cmd-icon"><use href="#${icon}"/></svg>`;
  btn.innerHTML =
    `<span class="cmd-icon-wrap">${glyph}` +
    (key ? `<span class="cmd-key">${key}</span>` : '') +
    `</span>`;
  return btn;
}

let cmdSig = ''; // signature of the last-rendered command context

// Train-block explanations, indexed by the sim's TrainBlock tag
// (shared shape for the Town Center and production buildings).
// i18n: rebuilt by refreshNames().
let TRAIN_BLOCK_TITLES = [
  '',
  'Not enough resources.',
  'The training queue is full.',
  'Housed: population is at capacity — training cannot finish until a House stands.',
];

// Contextual commands, bottom-left: units selected (with a villager
// among them) -> build buttons (driving the existing placement
// flow); Town Center selected -> train Villager; Barracks selected
// -> train Militia/Archer; nothing -> an empty panel (DE's command
// panel carries no explanatory text). Button ids are stable across
// rebuilds; clicks are handled by the one delegated listener on
// #cmd-grid in main().
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
  // context renders nothing, never command buttons.
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
  // SEL-17: the AutoExplore button lives in the units context and
  // shows when any of the LOCAL player's Scout Cavalry is in the
  // selection — every scout is eligible, starting or trained
  // (current DE, update 111772); the sim answers eligibility per
  // entity and the client only mirrors it. The pressed face is on
  // when any selected scout is exploring (a mixed selection
  // toggles uniformly — see toggleAutoExplore).
  const selScouts = context === 'units'
    ? selUnits.filter((id) => sim.entity_can_auto_explore(id)) : [];
  const scoutSel = selScouts.length > 0;
  const scoutAuto = scoutSel
    && selScouts.some((id) => sim.entity_auto_explore()[id] === 1);
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
    tcQueueCodes, bkQueueCodes, buildPage || '', repairArm ? 1 : 0,
    scoutSel ? selScouts.join('.') : -1, scoutAuto ? 1 : 0,
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
      if (kind === 1 && !hasMill) gate = t('gate.mill');
      else if (kind >= 6 && age < 1) gate = t('gate.feudal');
      else if ((kind === 6 || kind === 7) && !hasBarracks) gate = t('gate.barracks');
      const lacksWood = wood < BUILD_COST[kind];
      return cmdButton({
        id: `build-${kind}`, icon: BUILD_ICON[kind], img: BUILD_PNG[kind], label: BUILD_NAME[kind],
        costIcon: 'i-wood', cost: BUILD_COST[kind],
        disabled: !!gate || lacksWood,
        active: !!placement && placement.kind === kind,
        key: badgeForButton({ context, page: buildPage, id: `build-${kind}` }),
        title: gate
          ? t('cmd.buildGate', { name: BUILD_NAME[kind], gate })
          : t(lacksWood ? 'cmd.buildNeeds' : 'cmd.buildOk',
            { name: BUILD_NAME[kind], cost: costText([[BUILD_COST[kind], 'wood']]) }),
      });
    };
    if (buildPage) {
      cmdGrid.append(...PAGE_KINDS[buildPage].map(buildBtn));
      // The page's last slot is Back (root) — a plain return
      // glyph, no PNG; its click runs the same state change an
      // Esc on the open page runs.
      cmdGrid.append(cmdButton({
        id: 'page-back', icon: 'i-back', label: t('cmd.back'),
        key: badgeForButton({ context, page: buildPage, id: 'page-back' }),
        title: t('cmd.backTitle'),
      }));
    } else {
      // DE page model (UI FIX 4 addendum): the root shows ONLY
      // the two category openers, badged with the very letters
      // (Q / W) that arm the same pages from the keyboard; the
      // remaining grid slots stay empty panel, exactly like the
      // reference game's villager panel.
      cmdGrid.append(
        cmdButton({
          id: 'page-eco', img: 'house.png', label: t('cmd.economic'),
          key: 'Q', title: t('cmd.ecoTitle'),
        }),
        cmdButton({
          id: 'page-mil', img: 'barracks.png', label: t('cmd.military'),
          key: 'W', title: t('cmd.milTitle'),
        }),
        // BLD-17: the root's third slot is Repair, as in the
        // reference panel — E arms targeting; the click that
        // follows picks one of our damaged buildings.
        cmdButton({
          id: 'repair', icon: 'i-hammer', label: t('cmd.repair'),
          key: 'E', active: repairArm,
          title: t('cmd.repairTitle'),
        }),
      );
    }
  } else if (context === 'tc') {
    // M6: the Town Center also researches — Loom and the Feudal
    // advancement share its queue with villager training (TEC-03).
    const loomDone = (techMask & 1) !== 0;
    const feudalDone = age >= 1;
    cmdGrid.append(
      cmdButton({
        id: 'train', icon: 'i-train', img: 'villager.png', label: 'Villager',
        costIcon: 'i-food', cost: 50,
        disabled: reason === 2, // queue full; other blocks flash on click
        key: badgeForButton({ context, id: 'train', index: 0 }),
        title: TRAIN_BLOCK_TITLES[reason]
          ? t('cmd.trainVilBlocked', { block: TRAIN_BLOCK_TITLES[reason] })
          : t('cmd.trainVilTitle', { cost: costText([[50, 'food']]) }),
      }),
      cmdButton({
        id: 'research-loom', icon: 'i-loom', img: 'loom.png', label: 'Loom',
        costIcon: 'i-gold', cost: 50,
        disabled: loomDone || rbTc[0] === 2 || rbTc[0] === 3,
        key: badgeForButton({ context, id: 'research-loom', index: 1 }),
        title: loomDone
          ? t('cmd.loomDone', { name: TECH_NAME[0] })
          : t('cmd.loomTitle', { name: TECH_NAME[0], cost: costText([[50, 'gold']]), secs: 25 }) + (RESEARCH_BLOCK_TITLES[rbTc[0]] ? ' ' + RESEARCH_BLOCK_TITLES[rbTc[0]] : ''),
      }),
      cmdButton({
        id: 'research-feudal', icon: 'i-feudal', img: 'feudal.png', label: 'Feudal Age',
        costIcon: 'i-food', cost: 500,
        disabled: feudalDone || rbTc[1] === 2 || rbTc[1] === 3,
        key: badgeForButton({ context, id: 'research-feudal', index: 2 }),
        title: feudalDone
          ? t('cmd.feudalDone', { age: AGE_NAMES[1] })
          : rbTc[1] === 4
            ? t('cmd.feudalGate', { age: AGE_NAMES[1] })
            : t('cmd.feudalTitle', { age: AGE_NAMES[1], cost: costText([[500, 'food']]), secs: 130 }) + (RESEARCH_BLOCK_TITLES[rbTc[1]] ? ' ' + RESEARCH_BLOCK_TITLES[rbTc[1]] : ''),
      }),
    );
  } else if (context === 'barracks') {
    const underConstruction = !bk.complete;
    const maaDone = (techMask & 32) !== 0;
    cmdGrid.append(
      cmdButton({
        id: 'train-militia', icon: 'i-militia', img: 'militia.png', label: 'Militia',
        costIcon: 'i-food', cost: 60, cost2Icon: 'i-gold', cost2: 20,
        disabled: underConstruction || blockM === 2,
        key: badgeForButton({ context, id: 'train-militia', index: 0 }),
        title: underConstruction
          ? t('cmd.underConstruction', { name: KIND_NAME[3], building: BUILD_NAME[2] })
          : t('cmd.trainTitle', { name: KIND_NAME[3], cost: costText([[50, 'food'], [20, 'gold']]) }) + (TRAIN_BLOCK_TITLES[blockM] ? ' ' + TRAIN_BLOCK_TITLES[blockM] : ''),
      }),
      cmdButton({
        id: 'research-maa', icon: 'i-maa', img: 'man-at-arms.png', label: 'Man-at-Arms',
        costIcon: 'i-food', cost: 100, cost2Icon: 'i-gold', cost2: 40,
        disabled: underConstruction || maaDone || rbBk[3] === 2 || rbBk[3] === 3,
        key: badgeForButton({ context, id: 'research-maa', index: 1 }),
        title: underConstruction
          ? t('cmd.underConstruction', { name: TECH_NAME[5], building: BUILD_NAME[2] })
          : maaDone
            ? t('cmd.maaDone', { name: TECH_NAME[5], militia: KIND_NAME[3], name2: TECH_NAME[5] })
            : t('cmd.maaTitle', { name: TECH_NAME[5], cost: costText([[100, 'food'], [40, 'gold']]), secs: 40, militia: KIND_NAME[3] }) + (RESEARCH_BLOCK_TITLES[rbBk[3]] ? ' ' + RESEARCH_BLOCK_TITLES[rbBk[3]] : ''),
      }),
    );
  } else if (context === 'range') {
    const underConstruction = !bk.complete;
    cmdGrid.append(cmdButton({
      id: 'train-archer', icon: 'i-archer', img: 'archer.png', label: 'Archer',
      costIcon: 'i-wood', cost: 25, cost2Icon: 'i-gold', cost2: 45,
      disabled: underConstruction || blockA === 2,
      key: badgeForButton({ context, id: 'train-archer', index: 0 }),
      title: underConstruction
        ? t('cmd.underConstruction', { name: KIND_NAME[4], building: BUILD_NAME[6] })
        : t('cmd.trainTitle', { name: KIND_NAME[4], cost: costText([[25, 'wood'], [45, 'gold']]) }) + (TRAIN_BLOCK_TITLES[blockA] ? ' ' + TRAIN_BLOCK_TITLES[blockA] : ''),
    }));
  } else if (context === 'stable') {
    const underConstruction = !bk.complete;
    cmdGrid.append(cmdButton({
      id: 'train-scout', icon: 'i-villager', img: 'scout.png', label: 'Scout Cavalry',
      costIcon: 'i-food', cost: 80,
      disabled: underConstruction || blockS === 2,
      key: badgeForButton({ context, id: 'train-scout', index: 0 }),
      title: underConstruction
        ? t('cmd.underConstruction', { name: KIND_NAME[2], building: BUILD_NAME[7] })
        : t('cmd.trainTitleTime', { name: KIND_NAME[2], cost: costText([[80, 'food']]), secs: 30 }) + (TRAIN_BLOCK_TITLES[blockS] ? ' ' + TRAIN_BLOCK_TITLES[blockS] : ''),
    }));
  } else if (context === 'blacksmith') {
    // M6: the Blacksmith's three Feudal researches.
    const underConstruction = !bk.complete;
    const techBtn = (id, tag, icon, img, label, title, cost2) => {
      const done = (techMask & (1 << tag)) !== 0;
      const rb = rbBk[tag - 2];
      return cmdButton({
        id, icon, img, label, costIcon: 'i-food', cost: 100,
        cost2Icon: cost2 ? 'i-gold' : undefined, cost2,
        key: badgeForButton({ context, id, index: tag - 2 }),
        disabled: underConstruction || done || rb === 2 || rb === 3,
        title: underConstruction
          ? t('cmd.underConstruction', { name: TECH_NAME[tag], building: BUILD_NAME[8] })
          : done
            ? t('cmd.researchDone', { name: TECH_NAME[tag] })
            : `${title}${RESEARCH_BLOCK_TITLES[rb] ? ' ' + RESEARCH_BLOCK_TITLES[rb] : ''}`,
      });
    };
    cmdGrid.append(
      techBtn('research-fletching', 2, 'i-fletching', 'fletching.png', 'Fletching',
        t('cmd.fletchingTitle', { name: TECH_NAME[2], cost: costText([[100, 'food'], [50, 'gold']]), secs: 30 }), 50),
      techBtn('research-scalemail', 3, 'i-scalemail', 'scale-mail.png', 'Scale Mail',
        t('cmd.scalemailTitle', { name: TECH_NAME[3], cost: costText([[100, 'food']]), secs: 40 })),
      techBtn('research-padded', 4, 'i-padded', 'padded-armor.png', 'Padded Armor',
        t('cmd.paddedTitle', { name: TECH_NAME[4], cost: costText([[100, 'food']]), secs: 40 })),
    );
  }
  if (context === 'units' && scoutSel) {
    // SEL-17: the AutoExplore toggle (G) for the selected
    // scout(s). The pressed face mirrors the sim's flag; the
    // tooltip carries the name and the state, as every command
    // button here.
    cmdGrid.append(cmdButton({
      id: 'auto-explore', icon: 'i-explore', label: t('cmd.autoExplore'),
      active: !!scoutAuto, key: 'G',
      title: t(scoutAuto ? 'cmd.autoExploreOnTitle' : 'cmd.autoExploreTitle'),
    }));
  }
  // SEL-14 (wiki Hotkey page): Stop and Delete ride along
  // whenever UNITS are selected — Stop halts every selected
  // unit where it stands; the panel Delete is the Shift+Delete
  // form (all selected units). An own building's panel carries
  // its own Delete (a foundation refunds in full — BLD-02;
  // a finished building refunds nothing).
  if (selUnits.length > 0) {
    cmdGrid.append(
      cmdButton({
        id: 'stop', icon: 'i-stop', label: t('cmd.stop'),
        title: t('cmd.stopTitle'),
      }),
      cmdButton({
        id: 'delete', icon: 'i-delete', label: t('cmd.delete'),
        title: t('cmd.deleteTitle'),
      }),
    );
  }
  if (selectedBuilding && selectedBuilding.owner === localOwner) {
    cmdGrid.append(cmdButton({
      id: 'delete-building', icon: 'i-delete', label: t('cmd.delete'),
      title: t('cmd.deleteBuildingTitle'),
    }));
  }
  // 'foe' and no-selection contexts (and a units selection without
  // a scout) render nothing: DE's command panel carries
  // no explanatory text — with no command buttons to show, the
  // panel is simply empty wood.
}

let selSig = ''; // signature of the last-rendered selection panel

// Selection panel, bottom-center: single unit -> portrait + name +
// HP + hint; group -> count + one cell per unit (capped at 12 +
// "+k"); Town Center / Barracks -> portrait + queue state; nothing
// -> guidance.
/// TEC-05: one cancel chip per entry of the selected TC's or
/// production building's queue, rendered into the selection
/// grid. Clicking a chip cancels exactly that entry through the
/// order channel; the sim refunds the entry's full cost.
function queueCancelChips(kind, id, codes) {
  codes.forEach((code, i) => {
    const chip = document.createElement('button');
    chip.type = 'button';
    chip.className = 'sel-cell qchip';
    chip.textContent = queueEntryName(code);
    chip.title = t('sel.cancelEntry', { name: queueEntryName(code) });
    chip.addEventListener('click', () => {
      if (kind === 'tc') issue({ t: 'cancel_tc', tc: id, i });
      else issue({ t: 'cancel_b', b: id, i });
      selSig = ''; // the queue changed under the panel: re-render
    });
    selGrid.append(chip);
  });
}

function updateSelPanel() {
  // Info-selected animal bookkeeping, before anything reads the
  // selection: a dead animal drops the selection; one that
  // converts to us mid-selection becomes a real unit selection
  // (it can take orders now); one stolen by the enemy stays an
  // information selection under its new owner.
  if (selectedAnimal != null) {
    const st0 = entityState();
    const i = selectedAnimal;
    if (i >= st0.count || st0.hp[i * 2] < 1) {
      selectedAnimal = null;
    } else if (st0.owners[i] === localOwner) {
      selectUnits(new Set([i]));
    }
  }
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
      : selectedAnimal != null ? 'animal'
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
    selectedAnimal != null
      ? `${selectedAnimal}:${Math.floor(st.hp[selectedAnimal * 2])}:${st.owners[selectedAnimal]}`
      : '',
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
    const kindName = KIND_NAME[kind] || t('unit.fallback');
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = KIND_PNG[kind]
      ? pngIcon(KIND_PNG[kind], 'ui-icon')
      : `<svg class="ui-icon"><use href="#${KIND_ICON[kind] || 'i-villager'}"/></svg>`;
    if (selUnits.length === 1) {
      const id = selUnits[0];
      const hpTxt = t('sel.hp', { cur: Math.ceil(st.hp[id * 2]), max: st.hp[id * 2 + 1] });
      selName.textContent = kindName;
      if (kind === 0) {
        selInfo.textContent = t('sel.villagerInfo', { hp: hpTxt });
      } else if (kind === KIND_SHEEP) {
        selInfo.textContent = t('sel.sheepInfo', { hp: hpTxt });
      } else {
        selInfo.textContent = t('sel.militaryInfo', { hp: hpTxt });
      }
    } else {
      selName.textContent = `${selUnits.length} · ${kindName}`;
      selInfo.textContent = t('sel.groupInfo');
      const shown = selUnits.slice(0, 12);
      for (let i = 0; i < shown.length; i++) {
        const cell = document.createElement('span');
        cell.className = 'sel-cell';
        cell.innerHTML = KIND_PNG[kind]
          ? pngIcon(KIND_PNG[kind], 'ui-icon')
          : '<svg class="ui-icon"><use href="#i-villager"/></svg>';
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
    selPortrait.innerHTML = pngIcon('town-center.png', 'ui-icon');
    selName.textContent = t('unit.townCenter');
    const codes = myTcQueue();
    if (codes.length > 0) {
      // TEC-05: one cancel chip per queue entry.
      queueCancelChips('tc', myTcId, codes);
      // The queue line comes first (addendum, 2026-10-19): with
      // the button badge gone, this line is where the queue
      // count + progress live — including while housed, where
      // the stall note rides along instead of replacing it.
      const head = codes[0];
      selInfo.textContent = (head >= 100
        ? t('sel.researchingQ', { name: queueEntryName(head), pct: Math.round(myTcProgress() * 100), n: codes.length })
        : t('sel.trainingVilQ', { pct: Math.round(myTcProgress() * 100), n: codes.length }))
        + (reason === 3 ? t('sel.housedSuffix') : '');
      if (reason === 3) selInfo.classList.add('warn');
    } else if (reason === 3) {
      selInfo.textContent = t('sel.housedLine');
      selInfo.classList.add('warn');
    } else {
      selInfo.textContent = t('sel.tcIdle');
    }
  } else if (context === 'production') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = pngIcon(BUILD_PNG[bk.kind], 'ui-icon');
    // M13: a Ctrl+Shift go-to selects a whole type at once; the
    // name carries the count (commands address the first).
    selName.textContent = buildingGroup && buildingGroup.length > 1
      ? `${BUILD_NAME[bk.kind]} ×${buildingGroup.length}`
      : BUILD_NAME[bk.kind];
    const codes = sim.building_queue(bk.id);
    if (!bk.complete) {
      selInfo.textContent = t('sel.underConstruction');
    } else if (codes.length > 0) {
      // TEC-05: one cancel chip per queue entry.
      queueCancelChips('b', bk.id, codes);
      const head = codes[0];
      selInfo.textContent = (head >= 100
        ? t('sel.researchingB', { name: queueEntryName(head), n: codes.length })
        : t('sel.trainingB', { name: queueEntryName(head), n: codes.length }))
        + (bkBlock === 3 ? t('sel.housedSuffix') : '');
      if (bkBlock === 3) selInfo.classList.add('warn');
    } else if (bkBlock === 3) {
      selInfo.textContent = t('sel.housedLine');
      selInfo.classList.add('warn');
    } else {
      selInfo.textContent = bk.kind === 8
        ? t('sel.blacksmithIdle')
        : t('sel.productionIdle');
    }
  } else if (context === 'animal') {
    // An animal that is not ours: portrait, name, hit points, and
    // one honest line about what it is (wild / the enemy's). No
    // commands — the command panel stays in its empty state.
    const i = selectedAnimal;
    const kind = st.kinds[i];
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = KIND_PNG[kind]
      ? pngIcon(KIND_PNG[kind], 'ui-icon')
      : `<svg class="ui-icon"><use href="#${KIND_ICON[kind] || 'i-villager'}"/></svg>`;
    selName.textContent = KIND_NAME[kind] || t('unit.fallback');
    const hpTxt = t('sel.hp', { cur: Math.ceil(st.hp[i * 2]), max: st.hp[i * 2 + 1] });
    selInfo.textContent = kind === KIND_SHEEP
      ? (st.owners[i] === 1
        ? t('sel.gaiaSheepInfo', { hp: hpTxt })
        : t('sel.foeSheepInfo', { hp: hpTxt }))
      : t('sel.wildAnimalInfo', { hp: hpTxt });
  } else if (context === 'foe-tc') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = pngIcon('town-center.png', 'ui-icon');
    selName.textContent = t('sel.enemyTc');
    const id = selectedBuilding.id;
    selInfo.textContent = t('sel.enemyTcInfo', {
      hp: t('sel.hp', { cur: Math.max(0, Math.ceil(st.hp[id * 2])), max: st.hp[id * 2 + 1] }),
    });
  } else if (context === 'foe-building') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = pngIcon(bk ? BUILD_PNG[bk.kind] : 'house.png', 'ui-icon');
    selName.textContent = bk ? t('sel.enemyName', { name: BUILD_NAME[bk.kind] }) : t('sel.enemyName', { name: t('building.fallback') });
    selInfo.textContent = bk
      ? t('sel.enemyInfo', { cur: Math.round(bk.hpFrac * BUILD_MAXHP[bk.kind]), max: BUILD_MAXHP[bk.kind], under: bk.complete ? '' : t('sel.underSuffix') })
      : '';
  } else if (context === 'building') {
    selPortrait.classList.remove('empty');
    selPortrait.innerHTML = pngIcon(bk ? BUILD_PNG[bk.kind] : 'house.png', 'ui-icon');
    selName.textContent = bk
      ? (buildingGroup && buildingGroup.length > 1
        ? `${BUILD_NAME[bk.kind]} ×${buildingGroup.length}`
        : BUILD_NAME[bk.kind])
      : t('building.fallback');
    selInfo.textContent = bk && !bk.complete ? t('sel.underConstruction') : '';
  } else {
    selPortrait.classList.add('empty');
    selPortrait.innerHTML = pngIcon('villager.png', 'ui-icon');
    selName.textContent = t('sel.none');
    selInfo.textContent = t('sel.noneInfo');
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

  poseStamp++; // one pose computation per entity per draw pass

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
  frameSt = st; // snapshot for the unit-animation inference (foeNearUnit)
  updateCarry(nodes, tcPos, pos, kinds, st.hp);
  updateUnitAnimState(buildings, st);

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
  // Tree depletion tracking (falling trees + stumps) — the same
  // per-frame bookkeeping pattern as rubble, over the node list.
  updateTreeTracking(nodes, performance.now());
  for (const f of fallingTrees) {
    draws.push({ key: f.x + f.y + 0.4, fn: () => drawFallingTree(f, performance.now()) });
  }
  for (const site of stumpSites) {
    const age = performance.now() - site.t0;
    const fadeStart = STUMP_FADE_MS - 5000;
    const alpha = age > fadeStart ? 1 - (age - fadeStart) / 5000 : 1;
    if (fogAtWorld(site.x, site.y) === 0) continue;
    draws.push({ key: site.x + site.y + 0.3, fn: () => drawStump(site, alpha) });
  }
  // Rubble decals (milestone §10.3): tracked from the live lists,
  // drawn at ground level under whatever now walks the site.
  updateRubble(buildings, st);
  for (let k = rubbleSites.length - 1; k >= 0; k--) {
    const site = rubbleSites[k];
    const age = performance.now() - site.t0;
    if (age > RUBBLE_FADE_MS) { rubbleSites.splice(k, 1); continue; }
    if (site.owner === foeOwner() && fogAtWorld(site.x, site.y) === 0) continue;
    const fadeStart = RUBBLE_FADE_MS - 5000;
    const alpha = age > fadeStart ? 1 - (age - fadeStart) / 5000 : 1;
    draws.push({ key: site.x + site.y + 0.5, fn: () => drawRubble(site, alpha) });
  }
  // Unit draws dispatch by kind (M3); the dead are skipped (they
  // are tombstones in the sim, invisible here). A damaged unit gets
  // an HP bar above it.
  const UNIT_DRAW = [drawVillager, null, drawScout, drawMilitia, drawArcher,
    drawSheep, drawBoar, drawDeer, drawManAtArms]; // 8: Man-at-Arms (M6)
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
            drawBuildingFireAt(sx, sy - 4.4 * camS, 4, hp / maxHp, i * 2.39);
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
  // trajectory and draw an oriented arrow (drawArrowShape) rotated
  // to the screen-space direction of travel — the delta between
  // the interpolated positions at t and t + dt, arc lift included,
  // so the arrow pitches with its arc. LOW drops only the
  // fletching; the arrow and its heading are information.
  {
    const proj = sim.projectiles();
    const now = sim.tick_count();
    for (let p = 0; p < proj.length; p += 6) {
      const span = Math.max(1, proj[p + 5] - proj[p + 4]);
      const t = Math.max(0, Math.min(1, (now - proj[p + 4]) / span));
      const wx = proj[p] + (proj[p + 2] - proj[p]) * t;
      const wy = proj[p + 1] + (proj[p + 3] - proj[p + 1]) * t;
      // M7 (VIS-01): an arrow is a unit for knowledge — drawn only
      // while its cell is currently visible.
      if (!cellVisibleW(wx, wy)) continue;
      const lift = Math.sin(Math.PI * t) * 1.2 * camS;
      const [sx, sy] = worldToScreen(wx, wy);
      const t2 = Math.min(1, t + 0.06);
      const wx2 = proj[p] + (proj[p + 2] - proj[p]) * t2;
      const wy2 = proj[p + 1] + (proj[p + 3] - proj[p + 1]) * t2;
      const [sx2, sy2] = worldToScreen(wx2, wy2);
      const lift2 = Math.sin(Math.PI * t2) * 1.2 * camS;
      const ang = Math.atan2((sy2 - lift2) - (sy - lift), sx2 - sx);
      drawArrowShape(sx, sy - lift, ang, Q.projTrail);
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

  // Click markers + wood chips (INF-09 / tree hit feedback):
  // drawn AFTER the fog dim on purpose — DE shows the move marker
  // even in the black (Study 1, f_011: the red X sits in the
  // fog), because it is the player's own order echo, not world
  // information.
  drawWoodChips(performance.now());
  {
    const lowTier = governor.level === 'low';
    for (const m of clickMarkers.active(performance.now(), lowTier)) {
      const { heads, alpha } = markerHeads(m.t);
      const [cx, cy] = worldToScreen(m.x, m.y);
      ctx.save();
      ctx.globalAlpha = alpha;
      ctx.fillStyle = '#e23b2e';
      for (const h of heads) {
        // Project the head's world offset; the triangle points
        // from the head toward the centre.
        const ox = (h.dx - h.dy) * camS;
        const oy = ((h.dx + h.dy) / 2) * camS;
        const ang = Math.atan2(-oy, -ox);
        const L = 0.42 * camS;
        const W = 0.30 * camS;
        ctx.save();
        ctx.translate(cx + ox, cy + oy);
        ctx.rotate(ang);
        ctx.beginPath();
        ctx.moveTo(L / 2, 0);
        ctx.lineTo(-L / 2, -W / 2);
        ctx.lineTo(-L / 2, W / 2);
        ctx.closePath();
        ctx.fill();
        ctx.restore();
      }
      ctx.restore();
    }
  }

  // Waypoint pennants (INF-09): the persistent flags of the
  // selected units' queued order chains, read from the sim's
  // order routes every frame — a flag exists exactly while its
  // order does (completion, replacement and death remove it by
  // construction). Drawn after the fog dim like the click
  // marker: DE plants the flags IN the black (study 2, f_006) —
  // they are the player's own order echo, not world
  // information. Only the leader's chain draws for a group
  // selection (the "Waypoint Markers" article, via
  // play/pennants.js). Own units only, in the player's colour.
  if (selection.size) {
    const st = entityState();
    const routes = [];
    for (const id of selection) {
      if (isOwnUnit(st, id)) routes.push(sim.entity_order_route(id));
    }
    const shape = pennantShape();
    for (const p of pennantsForSelection(routes)) {
      const [sx, sy] = worldToScreen(p.x, p.y);
      ctx.save();
      ctx.strokeStyle = 'rgba(216, 220, 224, 0.9)';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.moveTo(sx, sy);
      ctx.lineTo(sx + shape.poleTop[0] * camS, sy + shape.poleTop[1] * camS);
      ctx.stroke();
      ctx.fillStyle = OWN_COLOR;
      ctx.beginPath();
      ctx.moveTo(sx + shape.poleTop[0] * camS, sy + shape.poleTop[1] * camS);
      ctx.lineTo(sx + shape.tip[0] * camS, sy + shape.tip[1] * camS);
      ctx.lineTo(sx + shape.low[0] * camS, sy + shape.low[1] * camS);
      ctx.closePath();
      ctx.fill();
      ctx.restore();
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
  toast.textContent = t('flash.wasmFail', { err: e });
  toast.style.display = 'block';
  console.error(e);
});
