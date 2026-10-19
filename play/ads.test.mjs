// Unit tests for ../ads.js rotation (M10) — run from
// web-preview/play/:
//   node --test ads.test.mjs
// ads.js is a classic script (shared by the landing and play
// pages), so it is loaded into a vm context with a stub DOM. The
// rotation contract under test: the two waiting slots start on
// the designed promo; a tick alternates promo <-> house; a tick
// while the tab is hidden changes nothing; a slot in a hidden
// overlay (a match has started) is never touched.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const ADS_SRC = readFileSync(new URL('../ads.js', import.meta.url), 'utf8');

function makeWorld({ hidden = false } = {}) {
  const els = new Map();
  const mkEl = (id) => {
    const el = {
      id,
      classList: { add() {}, remove() {}, toggle() {} },
      style: {},
      _attrs: new Map(),
      _innerHTML: '',
      // Layout stand-in: a "visible" slot has an offsetParent.
      offsetParent: {},
      offsetWidth: 300,
      set innerHTML(v) { this._innerHTML = v; },
      get innerHTML() { return this._innerHTML; },
      setAttribute(k, v) { this._attrs.set(k, String(v)); },
      getAttribute(k) { return this._attrs.has(k) ? this._attrs.get(k) : null; },
      appendChild() {},
      addEventListener() {},
    };
    els.set(id, el);
    return el;
  };
  for (const id of ['ad-slot-lobby', 'ad-slot-loading', 'ad-slot-leaderboard', 'ad-notice']) {
    mkEl(id);
  }
  const document = {
    readyState: 'complete',
    hidden,
    currentScript: null,
    getElementById: (id) => els.get(id) || null,
    createElement: () => mkEl('dyn' + Math.random()),
    addEventListener() {},
    head: mkEl('head'),
  };
  const window = {
    localStorage: {
      getItem: () => null,
      setItem() {},
    },
  };
  const sandbox = {
    window, document,
    setInterval: () => 1, // the timer is never the thing under test
    clearInterval: () => {},
    console,
  };
  sandbox.window.window = sandbox.window;
  vm.createContext(sandbox);
  vm.runInContext(ADS_SRC, sandbox);
  return { els, document, window };
}

test('waiting slots open on the designed promo; landing slots stay house', () => {
  const { els, window } = makeWorld();
  assert.equal(window.SkirmishAds.creativeOf('ad-slot-lobby'), 'promo');
  assert.equal(window.SkirmishAds.creativeOf('ad-slot-loading'), 'promo');
  assert.match(els.get('ad-slot-lobby').innerHTML, /promo-ad/);
  assert.match(els.get('ad-slot-leaderboard').innerHTML, /house-ad/);
  assert.equal(window.SkirmishAds.ROTATE_MS, 60_000);
});

test('rotation alternates promo <-> house on the waiting slots only', () => {
  const { els, window } = makeWorld();
  window.SkirmishAds.rotateNow();
  assert.equal(window.SkirmishAds.creativeOf('ad-slot-lobby'), 'house');
  assert.equal(window.SkirmishAds.creativeOf('ad-slot-loading'), 'house');
  assert.match(els.get('ad-slot-lobby').innerHTML, /house-ad/);
  window.SkirmishAds.rotateNow();
  assert.equal(window.SkirmishAds.creativeOf('ad-slot-lobby'), 'promo');
  assert.match(els.get('ad-slot-lobby').innerHTML, /promo-ad/);
  // The static landing slot never rotates.
  assert.equal(window.SkirmishAds.creativeOf('ad-slot-leaderboard'), null);
  assert.match(els.get('ad-slot-leaderboard').innerHTML, /house-ad/);
});

test('rotation pauses while the tab is hidden', () => {
  const { window, document } = makeWorld();
  document.hidden = true;
  window.SkirmishAds.rotateNow();
  assert.equal(window.SkirmishAds.creativeOf('ad-slot-lobby'), 'promo');
});

test('rotation never touches a slot inside a hidden overlay (match started)', () => {
  const { els, window } = makeWorld();
  const lobby = els.get('ad-slot-lobby');
  lobby.offsetParent = null;
  lobby.offsetWidth = 0;
  window.SkirmishAds.rotateNow();
  assert.equal(window.SkirmishAds.creativeOf('ad-slot-lobby'), 'promo');
  // The still-visible loading slot rotated normally.
  assert.equal(window.SkirmishAds.creativeOf('ad-slot-loading'), 'house');
});
