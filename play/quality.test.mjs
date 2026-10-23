// Headless tests for quality.js — run with:  node --test quality.test.mjs
// (from web-preview/play/). Everything under test is pure: probe
// signals are plain objects and the governor is fed scripted frame
// timings on a fake clock, so no browser is needed.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  LEVELS,
  settingsFor,
  scoreProbe,
  resolveMode,
  levelForMode,
  createGovernor,
  GOVERNOR,
} from './quality.js';

// --- Probe -> level -----------------------------------------------------

test('probe: 32 GB MacBook-class machine scores high', () => {
  // Chrome caps the deviceMemory report at 8; 10 cores, retina DPR.
  assert.equal(scoreProbe({
    deviceMemory: 8, hardwareConcurrency: 10, devicePixelRatio: 2,
    screenW: 1512, screenH: 982, touch: false,
  }), 'high');
});

test('probe: gaming desktop scores high without DPR help', () => {
  assert.equal(scoreProbe({
    deviceMemory: 8, hardwareConcurrency: 16, devicePixelRatio: 1,
    screenW: 2560, screenH: 1440, touch: false,
  }), 'high');
});

test('probe: 4 GB Chromebook-class machine lands on medium', () => {
  assert.equal(scoreProbe({
    deviceMemory: 4, hardwareConcurrency: 4, devicePixelRatio: 1,
    screenW: 1366, screenH: 768, touch: false,
  }), 'medium');
});

test('probe: old 2 GB / dual-core Chromebook lands on low', () => {
  assert.equal(scoreProbe({
    deviceMemory: 2, hardwareConcurrency: 2, devicePixelRatio: 1,
    screenW: 1366, screenH: 768, touch: false,
  }), 'low');
});

test('probe: hard caps beat a generous score', () => {
  // 1 GB RAM with many cores: memory wins, level is low.
  assert.equal(scoreProbe({
    deviceMemory: 1, hardwareConcurrency: 16, devicePixelRatio: 2,
    screenW: 1920, screenH: 1080, touch: false,
  }), 'low');
  // A phone-class touch screen never starts above medium.
  assert.equal(scoreProbe({
    deviceMemory: 8, hardwareConcurrency: 8, devicePixelRatio: 3,
    screenW: 390, screenH: 844, touch: true,
  }), 'medium');
});

test('probe: signal-poor browser (Safari/Firefox) defaults sensibly', () => {
  // No deviceMemory anywhere; a strong desktop still reaches high
  // via cores + DPR, an unknown everything lands mid, and a weak
  // signal set stays low (the runtime governor corrects the rest).
  assert.equal(scoreProbe({
    deviceMemory: undefined, hardwareConcurrency: 12, devicePixelRatio: 2,
    screenW: 1728, screenH: 1117, touch: false,
  }), 'high');
  assert.equal(scoreProbe({}), 'medium');
  assert.equal(scoreProbe({
    deviceMemory: undefined, hardwareConcurrency: 2, devicePixelRatio: 1,
    screenW: 1280, screenH: 800, touch: false,
  }), 'low');
});

// --- Mode resolution ------------------------------------------------------

test('mode: URL parameter wins, then stored, then auto', () => {
  assert.equal(resolveMode({ param: 'low', stored: 'high' }), 'low');
  assert.equal(resolveMode({ param: null, stored: 'medium' }), 'medium');
  assert.equal(resolveMode({ param: 'bogus', stored: 'high' }), 'high');
  assert.equal(resolveMode({ param: null, stored: null }), 'auto');
  assert.equal(resolveMode({}), 'auto');
  assert.equal(resolveMode({ param: 'auto', stored: 'low' }), 'auto');
});

test('mode: fixed modes pin the level, auto follows the probe', () => {
  assert.equal(levelForMode('auto', 'medium'), 'medium');
  assert.equal(levelForMode('high', 'low'), 'high');
  assert.equal(levelForMode('low', 'high'), 'low');
});

test('settings: levels map to tiers, DPR caps and cosmetic flags', () => {
  assert.deepEqual(LEVELS, ['low', 'medium', 'high']);
  const low = settingsFor('low');
  assert.equal(low.tier, 8);
  assert.equal(low.tierDir, '');
  assert.equal(low.dprCap, 1);
  assert.equal(low.shadows, false);
  assert.equal(low.groundStrokes, false);
  assert.equal(low.pathLines, false);
  assert.equal(low.projTrail, false);
  // Corpses are the FIRST cosmetic shed: off at low while every
  // other level keeps them (animation pass 2026-10-22).
  assert.equal(low.corpses, false);
  const med = settingsFor('medium');
  assert.equal(med.tier, 16);
  assert.equal(med.tierDir, 'tier16');
  assert.equal(med.dprCap, 1.5);
  assert.equal(med.shadows, true);
  assert.equal(med.corpses, true);
  const high = settingsFor('high');
  assert.equal(high.tier, 32);
  assert.equal(high.tierDir, 'tier32');
  assert.equal(high.dprCap, 2);
  assert.equal(high.corpses, true);
  // Unknown input degrades to the safest level.
  assert.equal(settingsFor('ultra'), low);
});

// --- Governor state machine ----------------------------------------------
// Feed `seconds` of frames at a steady `fps`, starting the fake
// clock at t; returns the new clock and the last sample() result.
function feed(gov, fps, seconds, t) {
  const dt = 1000 / fps;
  let last = { level: gov.level, changed: null };
  const n = Math.round(seconds * fps);
  for (let i = 0; i < n; i++) {
    t += dt;
    last = gov.sample(dt, t);
  }
  return { t, last };
}

test('governor: healthy fps never changes the level', () => {
  const g = createGovernor({ initial: 'medium', max: 'medium' });
  const { last } = feed(g, 60, 30, 0);
  assert.equal(last.changed, null);
  assert.equal(g.level, 'medium');
});

test('governor: sustained low fps steps down one level, not instantly', () => {
  const g = createGovernor({ initial: 'high', max: 'high' });
  let r = feed(g, 30, 4, 0); // ~2 bad windows: not enough
  assert.equal(g.level, 'high');
  r = feed(g, 30, 4, r.t); // crosses DOWN_WINDOWS bad windows
  assert.equal(g.level, 'medium');
});

test('governor: cooldown blocks immediate further step-down', () => {
  const g = createGovernor({ initial: 'high', max: 'high' });
  let r = feed(g, 30, 8, 0);
  assert.equal(g.level, 'medium');
  r = feed(g, 30, 8, r.t); // still inside the post-change cooldown
  assert.equal(g.level, 'medium');
  r = feed(g, 30, 30, r.t); // pain persists past cooldown -> next step
  assert.equal(g.level, 'low');
  r = feed(g, 30, 30, r.t); // floor: never below low
  assert.equal(g.level, 'low');
});

test('governor: alternating good/bad fps cannot oscillate the level', () => {
  const g = createGovernor({ initial: 'medium', max: 'high' });
  let t = 0;
  for (let i = 0; i < 10; i++) {
    t = feed(g, 30, GOVERNOR.WINDOW_MS / 1000, t).t; // one bad window
    t = feed(g, 52, GOVERNOR.WINDOW_MS / 1000, t).t; // one mid window
  }
  assert.equal(g.level, 'medium');
});

test('governor: sustained headroom steps up, capped at the probe level', () => {
  const g = createGovernor({ initial: 'low', max: 'medium' });
  let r = feed(g, 60, 20, 0); // UP_WINDOWS great windows -> one step
  assert.equal(g.level, 'medium');
  r = feed(g, 60, 60, r.t); // never above max, however long it runs
  assert.equal(g.level, 'medium');
});

test('governor: step-up is slower than step-down (hysteresis)', () => {
  const g = createGovernor({ initial: 'low', max: 'high' });
  // 6 s of perfect fps is enough to trigger a step DOWN from pain,
  // but not a step UP: up needs UP_WINDOWS (~12 s) of headroom.
  const r = feed(g, 60, 6, 0);
  assert.equal(g.level, 'low');
  const r2 = feed(g, 60, 10, r.t);
  assert.equal(g.level, 'medium');
});

test('governor: manual mode freezes adaptation until released', () => {
  const g = createGovernor({ initial: 'high', max: 'high' });
  g.setManual('low');
  let r = feed(g, 60, 40, 0);
  assert.equal(g.level, 'low'); // no step-up while pinned
  r = feed(g, 15, 40, r.t);
  assert.equal(g.level, 'low');
  g.setManual(null); // hand control back: level stays, adaptation resumes
  assert.equal(g.level, 'low');
  r = feed(g, 60, 20, r.t);
  assert.equal(g.level, 'medium');
});

test('governor: degenerate samples are ignored', () => {
  const g = createGovernor({ initial: 'medium', max: 'medium' });
  assert.deepEqual(g.sample(0, 1000), { level: 'medium', changed: null });
  assert.deepEqual(g.sample(-5, 1000), { level: 'medium', changed: null });
  assert.equal(g.level, 'medium');
});
