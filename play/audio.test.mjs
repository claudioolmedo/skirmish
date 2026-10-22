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
