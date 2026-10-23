// Skirmish II — adaptive quality governor (client-only, pure logic).
//
// Founding doctrine: ONE build, an identical simulation for every
// player; ONLY the visuals scale to the hardware. A low-memory
// Chromebook and a 32 GB MacBook play the same game — the MacBook
// just gets a richer picture. The governor therefore never touches
// the sim, its tick rate, or any command: it picks sprite tiers,
// caps the canvas backing resolution, and sheds COSMETIC draw work
// (shadows, strokes, trails) under load. Information is never shed:
// unit visibility, HP bars on damaged units, selection rings,
// resource bars and the minimap render at every level.
//
// This module is deliberately free of DOM access at import time so
// the scoring and the adaptation state machine can be unit-tested
// headlessly in Node (quality.test.mjs). app.js feeds it plain
// signal objects and per-frame timings.

export const LEVELS = ['low', 'medium', 'high'];

export function levelIndex(level) {
  return LEVELS.indexOf(level);
}

export function isLevel(v) {
  return LEVELS.includes(v);
}

// What a level actually controls. `tier` is the sprite direction
// count (SPRITE-PIPELINE.md §3: a tier is a re-render flag, never
// new art) and `tierDir` the assets-original subfolder for it (LOW
// lives at the folder root, for back-compat). Sheets load lazily:
// only the ACTIVE tier's files are ever requested, so a LOW client
// never downloads a byte of tier32 art. `dprCap` caps the canvas
// backing-store resolution relative to CSS pixels — the single
// biggest fill-rate lever on weak GPUs. The remaining flags gate
// cosmetic passes in draw(); see app.js for each use site.
export const LEVEL_SETTINGS = Object.freeze({
  low: Object.freeze({
    tier: 8, tierDir: '', dprCap: 1,
    shadows: false, groundStrokes: false, pathLines: false,
    projTrail: false, idLabels: false,
    // Corpses (the death fall + fading body decals) are the
    // FIRST cosmetic shed as the level steps down — pure
    // battlefield dressing, gone before any other effect is
    // even considered. Information is never shed (doctrine
    // above); a corpse carries none.
    corpses: false,
  }),
  medium: Object.freeze({
    tier: 16, tierDir: 'tier16', dprCap: 1.5,
    shadows: true, groundStrokes: true, pathLines: true,
    projTrail: true, idLabels: true,
    corpses: true,
  }),
  high: Object.freeze({
    tier: 32, tierDir: 'tier32', dprCap: 2,
    shadows: true, groundStrokes: true, pathLines: true,
    projTrail: true, idLabels: true,
    corpses: true,
  }),
});

export function settingsFor(level) {
  return LEVEL_SETTINGS[isLevel(level) ? level : 'low'];
}

// --- Probe ------------------------------------------------------------
// Signals are a plain object so tests can script any device:
//   { deviceMemory, hardwareConcurrency, devicePixelRatio,
//     screenW, screenH, touch }
// Any field may be undefined (Safari exposes no deviceMemory,
// Firefox neither); unknowns score mid — if that guess is too
// generous for the real hardware, the runtime governor steps the
// level down within seconds of sustained low FPS.

// Collect the signals from the live browser. Kept separate from
// scoring; safe to call anywhere (returns neutrals off-browser).
export function collectProbeSignals() {
  const nav = typeof navigator !== 'undefined' ? navigator : {};
  const scr = typeof screen !== 'undefined' ? screen : {};
  const touch = typeof window !== 'undefined' &&
    ('ontouchstart' in window || (nav.maxTouchPoints || 0) > 0);
  return {
    deviceMemory: nav.deviceMemory,
    hardwareConcurrency: nav.hardwareConcurrency,
    devicePixelRatio: typeof window !== 'undefined'
      ? window.devicePixelRatio : undefined,
    screenW: scr.width,
    screenH: scr.height,
    touch,
  };
}

export function scoreProbe(s) {
  let mem;
  if (s.deviceMemory === undefined) mem = 2;
  else if (s.deviceMemory >= 8) mem = 3;
  else if (s.deviceMemory >= 4) mem = 2;
  else if (s.deviceMemory >= 2) mem = 1;
  else mem = 0;

  let cores;
  if (s.hardwareConcurrency === undefined) cores = 1;
  else if (s.hardwareConcurrency >= 8) cores = 2;
  else if (s.hardwareConcurrency >= 4) cores = 1;
  else cores = 0;

  const dpr = s.devicePixelRatio !== undefined && s.devicePixelRatio >= 2 ? 1 : 0;
  const smallScreen = s.screenW !== undefined && s.screenH !== undefined &&
    Math.min(s.screenW, s.screenH) <= 820;

  let score = mem + cores + dpr;
  if (s.touch && smallScreen) score -= 1;

  let level = score >= 5 ? 'high' : score >= 3 ? 'medium' : 'low';

  // Hard caps: no score outruns a hard hardware fact.
  const cap = (lvl) => {
    if (levelIndex(level) > levelIndex(lvl)) level = lvl;
  };
  if (s.deviceMemory !== undefined && s.deviceMemory <= 1) cap('low');
  else if (s.deviceMemory !== undefined && s.deviceMemory <= 2) cap('medium');
  if (s.hardwareConcurrency !== undefined && s.hardwareConcurrency <= 2) cap('medium');
  if (s.touch && smallScreen) cap('medium'); // phone-class screens
  return level;
}

// --- Mode resolution ----------------------------------------------------
// Modes: 'auto' (probe + runtime adaptation) or a fixed level.
// Precedence: ?quality= URL parameter (support/debugging — wins for
// the session, never persisted) > the player's stored choice > auto.
export const MODES = ['auto', 'high', 'medium', 'low'];

export function isMode(v) {
  return MODES.includes(v);
}

export function resolveMode({ param = null, stored = null } = {}) {
  if (isMode(param)) return param;
  if (isMode(stored)) return stored;
  return 'auto';
}

export function levelForMode(mode, probeLevel) {
  return mode === 'auto' ? probeLevel : mode;
}

// --- Runtime adaptation -------------------------------------------------
// A small state machine fed one sample per rendered frame (only
// while the match is playing — a paused/static scene says nothing
// about headroom). FPS is judged in fixed windows; the thresholds
// are asymmetric on purpose (hysteresis): it steps DOWN quickly
// under sustained pain (<45 fps for ~6 s) but steps UP slowly
// (>=58 fps for ~12 s), never above the probe's initial level, one
// step at a time, with a cooldown after every change — so the level
// cannot oscillate. In manual mode the governor is frozen.
export const GOVERNOR = Object.freeze({
  WINDOW_MS: 1500,
  DOWN_FPS: 45,
  UP_FPS: 58,
  DOWN_WINDOWS: 4,
  UP_WINDOWS: 8,
  COOLDOWN_MS: 15000,
});

export function createGovernor({ initial = 'low', max = 'low' } = {}) {
  let level = isLevel(initial) ? initial : 'low';
  const maxLevel = isLevel(max) ? max : level;
  let manual = false;
  let accMs = 0;
  let frames = 0;
  let badStreak = 0;
  let goodStreak = 0;
  let cooldownUntil = -Infinity;

  function resetWindow() {
    accMs = 0;
    frames = 0;
  }

  return {
    get level() { return level; },
    get max() { return maxLevel; },
    get manual() { return manual; },

    // Pin the governor to a fixed level (manual override), or hand
    // control back with setManual(null): the level stays where it
    // is and adaptation resumes (still capped at max).
    setManual(lvl) {
      if (lvl === null) {
        manual = false;
        badStreak = 0;
        goodStreak = 0;
        resetWindow();
        return;
      }
      if (!isLevel(lvl)) return;
      manual = true;
      level = lvl;
      badStreak = 0;
      goodStreak = 0;
      resetWindow();
    },

    // Feed one rendered frame. dtMs = time since the previous frame,
    // nowMs = a monotonic timestamp (performance.now()). Returns
    // { level, changed: 'down' | 'up' | null }.
    sample(dtMs, nowMs) {
      if (manual || !(dtMs > 0)) return { level, changed: null };
      accMs += dtMs;
      frames += 1;
      if (accMs < GOVERNOR.WINDOW_MS) return { level, changed: null };
      const fps = (frames * 1000) / accMs;
      resetWindow();
      if (nowMs < cooldownUntil) {
        badStreak = 0;
        goodStreak = 0;
        return { level, changed: null };
      }
      if (fps < GOVERNOR.DOWN_FPS) {
        badStreak += 1;
        goodStreak = 0;
      } else if (fps >= GOVERNOR.UP_FPS) {
        goodStreak += 1;
        badStreak = 0;
      } else {
        badStreak = 0;
        goodStreak = 0;
      }
      if (badStreak >= GOVERNOR.DOWN_WINDOWS && levelIndex(level) > 0) {
        level = LEVELS[levelIndex(level) - 1];
        badStreak = 0;
        goodStreak = 0;
        cooldownUntil = nowMs + GOVERNOR.COOLDOWN_MS;
        return { level, changed: 'down' };
      }
      if (goodStreak >= GOVERNOR.UP_WINDOWS && levelIndex(level) < levelIndex(maxLevel)) {
        level = LEVELS[levelIndex(level) + 1];
        badStreak = 0;
        goodStreak = 0;
        cooldownUntil = nowMs + GOVERNOR.COOLDOWN_MS;
        return { level, changed: 'up' };
      }
      return { level, changed: null };
    },
  };
}
