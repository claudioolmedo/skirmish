// Skirmish II — generated audio (2026-10-20, Claudio's request).
//
// A client-only sound layer: EVERY cue is synthesized at runtime
// with the Web Audio API — oscillators and filtered noise shaped
// by envelopes. There are no samples, no audio files, and nothing
// from any other game. The layer reads only local UI actions and
// client-side state transitions (the same own-seat snapshots the
// event feed uses); it never touches the sim, never issues a
// command, and never enters a network payload — lockstep
// determinism and the golden hash are untouched by construction.
//
// The context is created lazily by unlock(), which app.js calls
// from the first pointer/key gesture (browser autoplay policy).
// Cues requested before that gesture, while sound is off, or
// inside their throttle window are simply dropped.

export const AUDIO_STORE_KEY = 'skirmish2.audio';
export const DEFAULT_AUDIO_SETTINGS = { enabled: true, volume: 0.8 };
export const GLOBAL_MIN_INTERVAL_MS = 35;

// Per-cue minimum spacing. Warnings are deliberately hard to
// repeat: a battle can rage for a minute and the attack cue can
// sound at most once every 9 s; the idle tick is softer still
// and at most once every 20 s — a nudge, never a nag.
export const CUE_INTERVALS_MS = {
  click: 50,
  menuOpen: 200,
  menuClose: 200,
  select: 80,
  orderVillager: 140,
  orderMilitary: 140,
  orderBuilding: 200,
  buildingPlaced: 300,
  constructionComplete: 600,
  villagerTrained: 500,
  trained: 500,
  researchComplete: 1000,
  ageAdvance: 1500,
  housed: 12000,
  underAttack: 9000,
  idleTick: 20000,
  victory: 2000,
  defeat: 2000,
};

// Priority cues bypass the small global spacing (a match can end
// a few ms after a click; the stinger must not be eaten by it) —
// their own per-cue intervals still apply.
const PRIORITY_CUES = new Set(['victory', 'defeat', 'ageAdvance', 'housed', 'underAttack']);

/**
 * Map a semantic event to a cue name (pure). Accepts a cue name
 * string (returned when known) or an event object:
 *   { type: 'ui', action: 'open'|'close'|'click' }
 *   { type: 'order', actor: 'villager'|'military'|'building' }
 *   { type: 'buildingPlaced' } / { type: 'selection' }
 *   { type: 'feed', kind: 'built'|'created'|'research'|'age', unitKind }
 *   { type: 'housed' } / { type: 'attack' } / { type: 'idle' }
 *   { type: 'end', result: 'victory'|'defeat' }
 * Unknown events map to null (silence).
 */
export function cueForEvent(event) {
  if (!event) return null;
  if (typeof event === 'string') {
    return Object.hasOwn(CUE_INTERVALS_MS, event) ? event : null;
  }
  switch (event.type) {
    case 'ui':
      if (event.action === 'open') return 'menuOpen';
      if (event.action === 'close') return 'menuClose';
      return 'click';
    case 'selection': return 'select';
    case 'order':
      if (event.actor === 'military') return 'orderMilitary';
      if (event.actor === 'building') return 'orderBuilding';
      return 'orderVillager';
    case 'buildingPlaced': return 'buildingPlaced';
    case 'feed':
      if (event.kind === 'built') return 'constructionComplete';
      if (event.kind === 'created') return event.unitKind === 0 ? 'villagerTrained' : 'trained';
      if (event.kind === 'age') return 'ageAdvance';
      if (event.kind === 'research') return 'researchComplete';
      return null;
    case 'housed': return 'housed';
    case 'attack': return 'underAttack';
    case 'idle': return 'idleTick';
    case 'end': return event.result === 'defeat' ? 'defeat' : 'victory';
    default: return null;
  }
}

/**
 * Snapshot-diff cue mapping (pure): the same own-player snapshot
 * shape the event feed diffs (play/feed.js) — completed own
 * buildings, living own units by kind, age, researched techs.
 * Returns the cue names the transition earns, in play order.
 */
export function cuesForSnapshotDiff(prev, next) {
  if (!prev || !next) return [];
  const cues = [];
  const prevBuilt = prev.built || {};
  if (Object.keys(next.built || {}).some((id) => !(id in prevBuilt))) {
    cues.push('constructionComplete');
  }
  const prevUnits = prev.units || {};
  let villager = false;
  let other = false;
  for (const [kind, count] of Object.entries(next.units || {})) {
    if (count > (prevUnits[kind] || 0)) {
      if (Number(kind) === 0) villager = true;
      else other = true;
    }
  }
  if (villager) cues.push('villagerTrained');
  else if (other) cues.push('trained');
  if ((next.age ?? 0) > (prev.age ?? 0)) {
    cues.push('ageAdvance');
  } else {
    const prevTechs = prev.techs || {};
    const techAdvanced = Object.entries(next.techs || {})
      .some(([k, v]) => v && !prevTechs[k]);
    if (techAdvanced) cues.push('researchComplete');
  }
  return cues;
}

/**
 * The throttle gate (pure logic, injectable clock): a cue plays
 * only when its own interval has elapsed since it last played
 * AND the global spacing has elapsed since any cue played
 * (priority cues skip only the global spacing).
 */
export function createCueGate(now = () => Date.now()) {
  const lastByCue = new Map();
  let lastAny = -Infinity;
  return {
    allow(cue, at = now()) {
      const interval = CUE_INTERVALS_MS[cue];
      if (interval == null) return false;
      const last = lastByCue.get(cue);
      if (last != null && at - last < interval) return false;
      if (!PRIORITY_CUES.has(cue) && at - lastAny < GLOBAL_MIN_INTERVAL_MS) return false;
      lastByCue.set(cue, at);
      lastAny = at;
      return true;
    },
    reset() {
      lastByCue.clear();
      lastAny = -Infinity;
    },
  };
}

export function loadAudioSettings(storage) {
  try {
    const raw = storage ? storage.getItem(AUDIO_STORE_KEY) : null;
    if (!raw) return { ...DEFAULT_AUDIO_SETTINGS };
    const parsed = JSON.parse(raw);
    return {
      enabled: typeof parsed.enabled === 'boolean'
        ? parsed.enabled : DEFAULT_AUDIO_SETTINGS.enabled,
      volume: Number.isFinite(parsed.volume)
        ? Math.min(1, Math.max(0, parsed.volume)) : DEFAULT_AUDIO_SETTINGS.volume,
    };
  } catch {
    return { ...DEFAULT_AUDIO_SETTINGS };
  }
}

export function saveAudioSettings(storage, settings) {
  try {
    if (storage) storage.setItem(AUDIO_STORE_KEY, JSON.stringify(settings));
  } catch { /* storage unavailable — the setting lives for the session */ }
}

function defaultStorage() {
  try { return globalThis.localStorage || null; } catch { return null; }
}

function defaultContextFactory() {
  const AC = globalThis.AudioContext || globalThis.webkitAudioContext;
  return AC ? new AC() : null;
}

/**
 * Create the audio engine. `contextFactory` and `now` are
 * injectable for headless tests; in the browser both default to
 * the real Web Audio API and clock.
 */
export function createAudioEngine({
  storage = defaultStorage(),
  contextFactory = defaultContextFactory,
  now = () => Date.now(),
} = {}) {
  let settings = loadAudioSettings(storage);
  const gate = createCueGate(now);
  let ctx = null;
  let master = null;
  let armed = false; // a user gesture has unlocked audio at least once
  let noiseBuffer = null;

  function ensureContext() {
    if (ctx) return ctx;
    let created = null;
    try { created = contextFactory(); } catch { created = null; }
    if (!created) return null;
    ctx = created;
    master = ctx.createGain();
    master.gain.value = settings.volume;
    master.connect(ctx.destination);
    return ctx;
  }

  function tone({
    freq, freqEnd = null, type = 'sine', at = 0, dur = 0.08, gain = 0.12,
  }) {
    const t0 = ctx.currentTime + at;
    const osc = ctx.createOscillator();
    const env = ctx.createGain();
    osc.type = type;
    osc.frequency.setValueAtTime(Math.max(1, freq), t0);
    if (freqEnd != null) {
      osc.frequency.exponentialRampToValueAtTime(Math.max(1, freqEnd), t0 + dur);
    }
    env.gain.setValueAtTime(0.0001, t0);
    env.gain.exponentialRampToValueAtTime(Math.max(0.0002, gain), t0 + 0.012);
    env.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
    osc.connect(env);
    env.connect(master);
    osc.start(t0);
    osc.stop(t0 + dur + 0.05);
  }

  function noise({ at = 0, dur = 0.08, gain = 0.15, freq = 700, q = 0.8 }) {
    if (!ctx.createBuffer || !ctx.createBiquadFilter) return;
    if (!noiseBuffer) {
      const len = Math.max(1, Math.floor(ctx.sampleRate * 0.25));
      noiseBuffer = ctx.createBuffer(1, len, ctx.sampleRate);
      const data = noiseBuffer.getChannelData(0);
      for (let i = 0; i < len; i++) data[i] = Math.random() * 2 - 1;
    }
    const t0 = ctx.currentTime + at;
    const src = ctx.createBufferSource();
    src.buffer = noiseBuffer;
    const filter = ctx.createBiquadFilter();
    filter.type = 'lowpass';
    filter.frequency.setValueAtTime(freq, t0);
    filter.Q.value = q;
    const env = ctx.createGain();
    env.gain.setValueAtTime(0.0001, t0);
    env.gain.exponentialRampToValueAtTime(Math.max(0.0002, gain), t0 + 0.008);
    env.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
    src.connect(filter);
    filter.connect(env);
    env.connect(master);
    src.start(t0);
    src.stop(t0 + dur + 0.05);
  }

  // One small synthesized recipe per cue — pitch, rhythm and
  // timbre carry the meaning; volume stays tasteful throughout.
  function schedule(cue) {
    switch (cue) {
      case 'click':
        tone({ freq: 840, type: 'square', dur: 0.035, gain: 0.07 });
        break;
      case 'menuOpen':
        tone({ freq: 392, type: 'sine', dur: 0.07, gain: 0.10 });
        tone({ freq: 523, type: 'sine', at: 0.06, dur: 0.09, gain: 0.10 });
        break;
      case 'menuClose':
        tone({ freq: 523, type: 'sine', dur: 0.07, gain: 0.10 });
        tone({ freq: 392, type: 'sine', at: 0.06, dur: 0.09, gain: 0.09 });
        break;
      case 'select':
        tone({ freq: 660, type: 'triangle', dur: 0.05, gain: 0.10 });
        break;
      case 'orderVillager': // soft spoken-like fall
        tone({ freq: 520, freqEnd: 392, type: 'sine', dur: 0.08, gain: 0.13 });
        break;
      case 'orderMilitary': // firmer rising answer
        tone({ freq: 330, freqEnd: 495, type: 'triangle', dur: 0.07, gain: 0.13 });
        break;
      case 'orderBuilding': // low thunk of a plan being set
        tone({ freq: 196, freqEnd: 147, type: 'sine', dur: 0.09, gain: 0.14 });
        noise({ dur: 0.05, gain: 0.06, freq: 500 });
        break;
      case 'buildingPlaced': // the wood knock: two mallet hits
        noise({ dur: 0.07, gain: 0.20, freq: 800 });
        tone({ freq: 150, freqEnd: 110, type: 'triangle', dur: 0.10, gain: 0.16 });
        noise({ at: 0.09, dur: 0.06, gain: 0.13, freq: 700 });
        tone({ freq: 130, freqEnd: 98, type: 'triangle', at: 0.09, dur: 0.09, gain: 0.12 });
        break;
      case 'constructionComplete':
        tone({ freq: 523, type: 'triangle', dur: 0.09, gain: 0.11 });
        tone({ freq: 659, type: 'triangle', at: 0.08, dur: 0.12, gain: 0.11 });
        break;
      case 'villagerTrained':
      case 'trained':
        tone({ freq: 587, type: 'sine', dur: 0.06, gain: 0.10 });
        tone({ freq: 880, type: 'sine', at: 0.05, dur: 0.07, gain: 0.09 });
        break;
      case 'researchComplete': // a short horn answers
        tone({ freq: 392, type: 'sawtooth', dur: 0.13, gain: 0.055 });
        tone({ freq: 494, type: 'sawtooth', at: 0.11, dur: 0.17, gain: 0.055 });
        break;
      case 'ageAdvance': // the fanfare: four rising horn notes
        [262, 392, 523, 659].forEach((f, i) => {
          tone({ freq: f, type: 'sawtooth', at: i * 0.09, dur: 0.16, gain: 0.05 });
          tone({ freq: f, type: 'triangle', at: i * 0.09, dur: 0.16, gain: 0.09 });
        });
        break;
      case 'housed': // two low warnings, never shrill
        tone({ freq: 220, type: 'square', dur: 0.11, gain: 0.07 });
        tone({ freq: 196, type: 'square', at: 0.14, dur: 0.14, gain: 0.07 });
        break;
      case 'underAttack': // two deep pulses
        tone({ freq: 180, freqEnd: 160, type: 'square', dur: 0.10, gain: 0.10 });
        tone({ freq: 180, freqEnd: 160, type: 'square', at: 0.15, dur: 0.10, gain: 0.10 });
        break;
      case 'idleTick': // barely there
        tone({ freq: 980, type: 'sine', dur: 0.03, gain: 0.022 });
        break;
      case 'victory':
        [392, 523, 659, 784].forEach((f, i) => {
          tone({ freq: f, type: 'triangle', at: i * 0.09, dur: 0.18, gain: 0.12 });
        });
        break;
      case 'defeat':
        [392, 330, 262, 196].forEach((f, i) => {
          tone({ freq: f, type: 'triangle', at: i * 0.11, dur: 0.20, gain: 0.11 });
        });
        break;
      default:
        break;
    }
  }

  return {
    /** The current persisted settings (a copy). */
    settings() { return { ...settings }; },
    /**
     * Unlock on a user gesture: create the context (once) and
     * resume it. Returns whether audio is ready to sound.
     */
    unlock() {
      if (!settings.enabled) return false;
      armed = true;
      const c = ensureContext();
      if (!c) return false;
      if (c.state === 'suspended' && c.resume) {
        const p = c.resume();
        if (p && p.catch) p.catch(() => {});
      }
      return true;
    },
    /** Play a cue name or semantic event; true when it sounded. */
    play(event) {
      const cue = cueForEvent(event);
      if (!cue || !settings.enabled || !armed) return false;
      if (!gate.allow(cue)) return false;
      const c = ensureContext();
      if (!c) return false;
      if (c.state === 'suspended' && c.resume) {
        const p = c.resume();
        if (p && p.catch) p.catch(() => {});
      }
      schedule(cue);
      return true;
    },
    setEnabled(enabled) {
      settings = { ...settings, enabled: !!enabled };
      saveAudioSettings(storage, settings);
      if (!settings.enabled) gate.reset();
    },
    setVolume(volume) {
      const v = Number.isFinite(volume) ? Math.min(1, Math.max(0, volume)) : settings.volume;
      settings = { ...settings, volume: v };
      saveAudioSettings(storage, settings);
      if (master && master.gain) master.gain.value = v;
    },
    /** Test/debug view of the throttle state. */
    gate,
  };
}
