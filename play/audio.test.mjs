// Headless tests for audio.js (generated-audio layer): the cue
// mapping, the snapshot-diff mapping, the throttle gate, the
// persisted settings, and the engine's synthesis against a fake
// Web Audio context. Run: node --test play/audio.test.mjs
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  AUDIO_STORE_KEY, CUE_INTERVALS_MS, createAudioEngine, createCueGate,
  cueForEvent, cuesForSnapshotDiff, distanceGain, loadAudioSettings,
} from './audio.js';

function makeStorage(seed = {}) {
  const m = new Map(Object.entries(seed));
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    _map: m,
  };
}

function makeParam() {
  return {
    value: 0,
    setValueAtTime() {},
    exponentialRampToValueAtTime() {},
  };
}

function makeFakeContext() {
  const ctx = {
    currentTime: 0,
    sampleRate: 8000,
    state: 'running',
    destination: {},
    oscillators: [],
    gains: [],
    buffers: [],
    sources: [],
    filters: [],
    createGain() {
      const g = { gain: makeParam(), connect() {} };
      ctx.gains.push(g);
      return g;
    },
    createOscillator() {
      const o = {
        type: '', frequency: makeParam(), started: 0, stopped: 0,
        connect() {}, start() { o.started += 1; }, stop() { o.stopped += 1; },
      };
      ctx.oscillators.push(o);
      return o;
    },
    createBuffer(channels, len, rate) {
      const b = { data: new Float32Array(len), getChannelData: () => b.data };
      ctx.buffers.push(b);
      return b;
    },
    createBufferSource() {
      const s = { buffer: null, connect() {}, start() {}, stop() {} };
      ctx.sources.push(s);
      return s;
    },
    createBiquadFilter() {
      const f = { type: '', frequency: makeParam(), Q: makeParam(), connect() {} };
      ctx.filters.push(f);
      return f;
    },
    resume() { ctx.state = 'running'; return Promise.resolve(); },
  };
  return ctx;
}

test('cueForEvent maps the semantic events and silences the rest', () => {
  assert.equal(cueForEvent('click'), 'click');
  assert.equal(cueForEvent({ type: 'ui', action: 'open' }), 'menuOpen');
  assert.equal(cueForEvent({ type: 'ui', action: 'close' }), 'menuClose');
  assert.equal(cueForEvent({ type: 'ui' }), 'click');
  assert.equal(cueForEvent({ type: 'selection' }), 'select');
  assert.equal(cueForEvent({ type: 'order', actor: 'villager' }), 'orderVillager');
  assert.equal(cueForEvent({ type: 'order', actor: 'military' }), 'orderMilitary');
  assert.equal(cueForEvent({ type: 'order', actor: 'building' }), 'orderBuilding');
  assert.equal(cueForEvent({ type: 'buildingPlaced' }), 'buildingPlaced');
  assert.equal(cueForEvent({ type: 'feed', kind: 'built' }), 'constructionComplete');
  assert.equal(cueForEvent({ type: 'feed', kind: 'created', unitKind: 0 }), 'villagerTrained');
  assert.equal(cueForEvent({ type: 'feed', kind: 'created', unitKind: 3 }), 'trained');
  assert.equal(cueForEvent({ type: 'feed', kind: 'research' }), 'researchComplete');
  assert.equal(cueForEvent({ type: 'feed', kind: 'age' }), 'ageAdvance');
  assert.equal(cueForEvent({ type: 'housed' }), 'housed');
  assert.equal(cueForEvent({ type: 'attack' }), 'underAttack');
  assert.equal(cueForEvent({ type: 'idle' }), 'idleTick');
  assert.equal(cueForEvent({ type: 'end', result: 'victory' }), 'victory');
  assert.equal(cueForEvent({ type: 'end', result: 'defeat' }), 'defeat');
  assert.equal(cueForEvent({ type: 'mystery' }), null);
  assert.equal(cueForEvent('not-a-cue'), null);
  assert.equal(cueForEvent(null), null);
});

test('cuesForSnapshotDiff reads the feed snapshot shape', () => {
  const snap = (over = {}) => ({ built: {}, units: {}, techs: {}, age: 0, sheep: 0, ...over });
  assert.deepEqual(cuesForSnapshotDiff(null, snap()), []);
  assert.deepEqual(cuesForSnapshotDiff(snap(), snap()), []);
  assert.deepEqual(
    cuesForSnapshotDiff(snap(), snap({ built: { 7: 0 } })),
    ['constructionComplete']);
  assert.deepEqual(
    cuesForSnapshotDiff(snap({ units: { 0: 3 } }), snap({ units: { 0: 4 } })),
    ['villagerTrained']);
  assert.deepEqual(
    cuesForSnapshotDiff(snap({ units: { 3: 0 } }), snap({ units: { 3: 1 } })),
    ['trained']);
  assert.deepEqual(
    cuesForSnapshotDiff(snap(), snap({ techs: { loom: true } })),
    ['researchComplete']);
  assert.deepEqual(
    cuesForSnapshotDiff(snap(), snap({ age: 1 })),
    ['ageAdvance'], 'an age advance is the fanfare, not a research blip');
  // Deaths and losses are not cues.
  assert.deepEqual(
    cuesForSnapshotDiff(snap({ units: { 0: 4 }, sheep: 2 }), snap({ units: { 0: 3 }, sheep: 1 })),
    []);
});

test('the gate enforces per-cue intervals and the global spacing', () => {
  let t = 1000;
  const gate = createCueGate(() => t);
  assert.equal(gate.allow('click'), true);
  t += 10;
  assert.equal(gate.allow('select'), false, 'global spacing blocks a second cue at +10ms');
  t += 30;
  assert.equal(gate.allow('click'), false, 'the click interval (50ms) still blocks at +40ms');
  t += 20;
  assert.equal(gate.allow('click'), true, 'the click interval has elapsed at +60ms');
  // Warnings are hard-throttled.
  t += 100;
  assert.equal(gate.allow('underAttack'), true);
  t += 1000;
  assert.equal(gate.allow('underAttack'), false, 'attack cue cannot repeat inside 9s');
  t += CUE_INTERVALS_MS.underAttack;
  assert.equal(gate.allow('underAttack'), true);
  // A priority cue is not eaten by a just-played click.
  t += 100;
  assert.equal(gate.allow('click'), true);
  t += 5;
  assert.equal(gate.allow('victory'), true, 'the stinger bypasses the global spacing');
  assert.equal(gate.allow('bogus-cue'), false, 'unknown cues never pass');
  gate.reset();
  assert.equal(gate.allow('victory'), true, 'reset clears the memory');
});

test('settings load, clamp and persist', () => {
  const storage = makeStorage();
  assert.deepEqual(loadAudioSettings(storage), { enabled: true, volume: 0.8 });
  const engine = createAudioEngine({ storage, contextFactory: () => makeFakeContext(), now: () => 0 });
  engine.setVolume(1.7);
  assert.equal(engine.settings().volume, 1, 'volume clamps to 1');
  engine.setVolume(-2);
  assert.equal(engine.settings().volume, 0, 'volume clamps to 0');
  engine.setEnabled(false);
  assert.deepEqual(JSON.parse(storage._map.get(AUDIO_STORE_KEY)), { enabled: false, volume: 0 });
  const reloaded = createAudioEngine({ storage, contextFactory: () => makeFakeContext(), now: () => 0 });
  assert.deepEqual(reloaded.settings(), { enabled: false, volume: 0 }, 'settings survive a reload');
});

test('the engine stays silent until unlocked, then synthesizes', () => {
  let t = 0;
  const ctx = makeFakeContext();
  let contextsMade = 0;
  const engine = createAudioEngine({
    storage: makeStorage(),
    contextFactory: () => { contextsMade += 1; return ctx; },
    now: () => t,
  });
  assert.equal(engine.play('select'), false, 'no gesture yet — autoplay policy');
  assert.equal(contextsMade, 0, 'the context is lazy: play() before unlock creates nothing');
  assert.equal(engine.unlock(), true);
  assert.equal(contextsMade, 1, 'unlock creates the one context');
  assert.equal(engine.play('select'), true);
  assert.ok(ctx.oscillators.length >= 1, 'a tone was scheduled');
  assert.ok(ctx.oscillators.every((o) => o.started === 1 && o.stopped === 1));
  t += 500;
  assert.equal(engine.play('buildingPlaced'), true);
  assert.ok(ctx.sources.length >= 1, 'the wood knock uses filtered noise');
  t += 50;
  assert.equal(engine.play('buildingPlaced'), false, 'the throttle drops the repeat');
  engine.setEnabled(false);
  t += 5000;
  assert.equal(engine.play('select'), false, 'off is off');
  assert.equal(engine.unlock(), false, 'a disabled engine does not unlock');
});

test('every cue in the table synthesizes without throwing', () => {
  let t = 0;
  const ctx = makeFakeContext();
  const engine = createAudioEngine({
    storage: makeStorage(),
    contextFactory: () => ctx,
    now: () => t,
  });
  engine.unlock();
  for (const cue of Object.keys(CUE_INTERVALS_MS)) {
    t += 30000; // outrun every interval, incl. the idle tick's 20s
    assert.equal(engine.play(cue), true, `cue ${cue} should sound`);
  }
});

test('INF-10 ambient cues: combat and work map, throttle and attenuate', () => {
  assert.equal(cueForEvent({ type: 'combat' }), 'combat');
  assert.equal(cueForEvent({ type: 'work' }), 'work');
  assert.equal(cueForEvent('combat'), 'combat');
  assert.equal(cueForEvent('work'), 'work');
  // Distance curve: full at the near radius, silent at the far
  // one, halfway between them half.
  assert.equal(distanceGain(0), 1);
  assert.equal(distanceGain(4), 1);
  assert.equal(distanceGain(38), 0);
  assert.equal(distanceGain(100), 0);
  assert.ok(Math.abs(distanceGain(21) - 0.5) < 1e-9);
  assert.equal(distanceGain(NaN), 0);
  // The gate spaces combat strikes like a real skirmish, not a
  // buzz: a second hit inside 450 ms is dropped, later ones play.
  let t = 0;
  const ctx = makeFakeContext();
  const engine = createAudioEngine({
    storage: makeStorage(), contextFactory: () => ctx, now: () => t,
  });
  engine.unlock();
  t = 1000;
  assert.equal(engine.play({ type: 'combat' }, { gain: 0.5 }), true);
  assert.ok(ctx.oscillators.length > 0 && ctx.sources.length > 0);
  t = 1200;
  assert.equal(engine.play({ type: 'combat' }, { gain: 0.5 }), false);
  t = 1500;
  assert.equal(engine.play({ type: 'combat' }, { gain: 0.5 }), true);
  // Work strikes ride the same plumbing on their own clock
  // (past the 35 ms global spacing from the combat hit).
  t = 1600;
  assert.equal(engine.play({ type: 'work' }, { gain: 1 }), true);
});

// --- The generated sound bank (tools/sound-gen) -------------------
// The manifest on disk is the contract: every file it names
// exists, decodes as our WAV format, and the priority spine of
// the catalog (acks for every class, horses, animals, combat,
// economy, building, UI, alerts, stingers) is covered.
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const AUDIO_DIR = join(dirname(fileURLToPath(import.meta.url)), 'audio');
const MANIFEST = JSON.parse(readFileSync(join(AUDIO_DIR, 'manifest.json'), 'utf8'));
const MANIFEST_FILES = [...new Set(Object.values(MANIFEST.sounds).flat())];

function wavInfo(buf) {
  assert.equal(buf.toString('ascii', 0, 4), 'RIFF');
  assert.equal(buf.toString('ascii', 8, 12), 'WAVE');
  // fmt chunk: audioFormat, channels, sampleRate, ..., bitsPerSample
  const fmtAt = buf.indexOf('fmt ');
  assert.ok(fmtAt > 0, 'fmt chunk present');
  return {
    format: buf.readUInt16LE(fmtAt + 8),
    channels: buf.readUInt16LE(fmtAt + 10),
    rate: buf.readUInt32LE(fmtAt + 12),
    bits: buf.readUInt16LE(fmtAt + 22),
  };
}

test('the generated manifest covers the catalog spine and every file decodes', () => {
  const keys = Object.keys(MANIFEST.sounds);
  assert.ok(keys.length >= 50, `sound keys: ${keys.length}`);
  for (const cls of ['villager', 'infantry', 'archer', 'cavalry']) {
    for (const action of ['select', 'order', 'attack']) {
      assert.ok(MANIFEST.sounds[`ack.${cls}.${action}`], `ack.${cls}.${action}`);
    }
  }
  for (const key of [
    'alert.bell', 'alert.horn', 'horse.hooves', 'horse.neigh',
    'sheep.baa', 'sheep.death', 'boar.grunt', 'boar.death',
    'deer.death', 'wolf.growl', 'wolf.death',
    'hit.sword', 'hit.spear', 'hit.knife', 'bow.release',
    'arrow.hit', 'hit.building', 'death.human', 'death.cavalry',
    'work.chop', 'work.treefall', 'work.minegold', 'work.minestone',
    'work.farm', 'work.hammer', 'work.dropoff',
    'build.place', 'build.complete', 'build.research', 'build.destroy',
    'ui.coin', 'ui.error', 'ui.ready', 'housed.warn',
    'sting.ageup', 'sting.victory', 'sting.defeat',
    'ambient.bird', 'ambient.wind',
  ]) {
    assert.ok(MANIFEST.sounds[key], `spine key ${key}`);
  }
  let totalBytes = 0;
  for (const file of MANIFEST_FILES) {
    const path = join(AUDIO_DIR, file);
    assert.ok(existsSync(path), `asset exists: ${file}`);
    const buf = readFileSync(path);
    totalBytes += buf.length;
    const info = wavInfo(buf);
    assert.equal(info.format, 1, `${file}: PCM`);
    assert.equal(info.channels, 1, `${file}: mono`);
    assert.equal(info.rate, 22050, `${file}: rate`);
    assert.equal(info.bits, 16, `${file}: 16-bit`);
    assert.ok(buf.length > 1000, `${file}: non-trivial payload`);
  }
  assert.ok(totalBytes < 3 * 1024 * 1024, `bank stays small: ${totalBytes}`);
  for (const cue of ['ageAdvance', 'victory', 'defeat', 'researchComplete',
    'constructionComplete', 'buildingPlaced', 'housed', 'combat', 'work']) {
    assert.ok(MANIFEST.cueSounds[cue], `cue mapping ${cue}`);
    assert.ok(MANIFEST.sounds[MANIFEST.cueSounds[cue]],
      `cue ${cue} maps to a real sound`);
  }
});

test('the taunt set is complete: every chat code has a sound, and voice taunts carry their phrase', async () => {
  const { TAUNTS, TAUNT_CODES } = await import('./chat.js');
  for (const code of TAUNT_CODES) {
    const key = `taunt.${code}`;
    assert.ok(MANIFEST.sounds[key], `${key} has a generated sound`);
    for (const file of MANIFEST.sounds[key]) {
      assert.ok(MANIFEST_FILES.includes(file), `${file} listed`);
    }
    // Code 30 is the wordless chant (a DSP render, no voice
    // slot); every other code's manifest phrase is the line
    // the chat log shows.
    if (code !== 30) {
      assert.deepEqual(MANIFEST.phrases[key], [TAUNTS[code]], `${key} phrase`);
      assert.ok(MANIFEST.voices[key], `${key} voice`);
    }
  }
});

test('playTaunt plays the local taunt asset, honors mute and the 2 s spacing', async () => {
  const { engine, ctx, tagOf, advance } = await bankedEngine();
  assert.equal(engine.playTaunt(7), true);
  const tauntTags = MANIFEST.sounds['taunt.7'].map(tagOf);
  assert.ok(tauntTags.includes(ctx.startedBuffers.at(-1).tag), 'the taunt.7 asset played');
  // Spacing: an instant repeat is handled but silent.
  const started = ctx.startedBuffers.length;
  assert.equal(engine.playTaunt(7), true);
  assert.equal(ctx.startedBuffers.length, started);
  advance(2100);
  assert.equal(engine.playTaunt(7), true);
  assert.equal(ctx.startedBuffers.length, started + 1);
  // A code with no asset is missing, not a synth beep.
  assert.equal(engine.playTaunt(99), false);
  // Mute governs taunts exactly like every other sound.
  engine.setEnabled(false);
  assert.equal(engine.playTaunt(1), false);
});

// A fake context that also decodes: each fetched file's first
// byte tags its decoded buffer, so tests can see WHICH asset a
// call started.
function makeBankContext() {
  const ctx = makeFakeContext();
  ctx.startedBuffers = [];
  ctx.decodeAudioData = (buf) => Promise.resolve({
    duration: 0.2, tag: new Uint8Array(buf)[0],
  });
  const baseCreate = ctx.createBufferSource.bind(ctx);
  ctx.createBufferSource = () => {
    const src = baseCreate();
    const start = src.start.bind(src);
    src.start = (when) => { ctx.startedBuffers.push(src.buffer); start(when); };
    return src;
  };
  return ctx;
}

function makeBankFetch() {
  const tags = new Map();
  MANIFEST_FILES.forEach((f, i) => tags.set(f, i + 1));
  const fetchImpl = (url) => {
    const s = String(url);
    const file = MANIFEST_FILES.find((f) => s.endsWith('/' + f))
      || (s.endsWith('manifest.json') ? 'manifest.json' : null);
    if (!file) return Promise.resolve({ ok: false, status: 404 });
    if (file === 'manifest.json') {
      return Promise.resolve({
        ok: true,
        json: () => Promise.resolve(MANIFEST),
        arrayBuffer: () => Promise.resolve(new ArrayBuffer(0)),
      });
    }
    const bytes = new Uint8Array(64);
    bytes[0] = tags.get(file);
    return Promise.resolve({
      ok: true,
      json: () => Promise.reject(new Error('not json')),
      arrayBuffer: () => Promise.resolve(bytes.buffer),
    });
  };
  return { fetchImpl, tagOf: (file) => tags.get(file) };
}

async function bankedEngine() {
  const ctx = makeBankContext();
  const { fetchImpl, tagOf } = makeBankFetch();
  let t = 0;
  const engine = createAudioEngine({
    storage: makeStorage(),
    contextFactory: () => ctx,
    now: () => t,
    fetchImpl,
    soundBaseUrl: 'https://example.test/audio/',
  });
  engine.unlock();
  assert.equal(await engine.loadSounds(), true);
  return { engine, ctx, tagOf, advance: (ms) => { t += ms; } };
}

test('the bank loads through the injected fetch and plays decoded buffers', async () => {
  const { engine, ctx, tagOf } = await bankedEngine();
  assert.equal(engine.bank.manifestSeen, true);
  assert.ok(engine.bank.sounds.get('work.chop').length >= 1);
  assert.equal(engine.playSound('work.chop'), true);
  const chopTags = MANIFEST.sounds['work.chop'].map(tagOf);
  assert.ok(chopTags.includes(ctx.startedBuffers.at(-1).tag), 'a chop variant played');
  // A genuinely unknown key reports missing, so callers fall back.
  assert.equal(engine.playSound('nope.key'), false);
});

test('playSound spacing: inside the window it is handled but silent', async () => {
  const { engine, ctx, advance } = await bankedEngine();
  assert.equal(engine.playSound('work.chop'), true);
  const started = ctx.startedBuffers.length;
  assert.equal(engine.playSound('work.chop'), true); // cooldown: handled
  assert.equal(ctx.startedBuffers.length, started); // ...but not replayed
  advance(500); // past the 380 ms chop spacing
  assert.equal(engine.playSound('work.chop'), true);
  assert.equal(ctx.startedBuffers.length, started + 1);
});

test('playAck routes by class and action, with the per-voice cooldown', async () => {
  const { engine, ctx, tagOf, advance } = await bankedEngine();
  assert.equal(engine.playAck({ cls: 'villager', action: 'select' }), true);
  assert.ok(MANIFEST.sounds['ack.villager.select'].map(tagOf)
    .includes(ctx.startedBuffers.at(-1).tag), 'the villager answered');
  // The same voice again at once: refused (no machine-gun acks).
  assert.equal(engine.playAck({ cls: 'villager', action: 'select' }), false);
  // A different class answers once the small global gap passes.
  advance(200);
  assert.equal(engine.playAck({ cls: 'infantry', action: 'order' }), true);
  assert.ok(MANIFEST.sounds['ack.infantry.order'].map(tagOf)
    .includes(ctx.startedBuffers.at(-1).tag), 'the footman answered');
});

test('mapped cues and the typed under-attack alert play the bank first', async () => {
  const { engine, ctx, tagOf, advance } = await bankedEngine();
  assert.equal(engine.play({ type: 'feed', kind: 'research' }), true);
  assert.ok(MANIFEST.sounds['build.research'].map(tagOf)
    .includes(ctx.startedBuffers.at(-1).tag), 'the research chime is generated');
  advance(10000); // past the 9 s underAttack cue gate
  assert.equal(engine.play({ type: 'attack', town: true }), true);
  assert.ok(MANIFEST.sounds['alert.bell'].map(tagOf)
    .includes(ctx.startedBuffers.at(-1).tag), 'the town gets bells');
  advance(10000); // past the alert gate again
  assert.equal(engine.play({ type: 'attack', town: false }), true);
  assert.ok(MANIFEST.sounds['alert.horn'].map(tagOf)
    .includes(ctx.startedBuffers.at(-1).tag), 'soldiers get the horn');
});

test('the ambient layer arms quietly, then speaks within its window', async () => {
  const { engine, ctx, advance } = await bankedEngine();
  assert.equal(engine.ambience(), false); // first call only arms
  advance(10000);
  engine.ambience();
  assert.ok(ctx.startedBuffers.length >= 1, 'a bird or the wind played');
});
