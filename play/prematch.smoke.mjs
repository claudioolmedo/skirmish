// Stub-DOM smoke for the pre-match state (M11 addendum, Claudio's
// rule): before a match is entered there is NO game on screen —
// body carries no .in-match, so play.css hides the canvas and
// every HUD element, and the lobby sits on the plain background.
// The class flips on exactly when a match starts and off on
// every return.
//
// Drives the REAL ../app.js in Node against a stub DOM and a stub
// WebSocket; the fetch stub serves the REAL .wasm bytes, so the
// solo start and the guest load genuinely run. Run from
// web-preview/play/:
//   node prematch.smoke.mjs
// A smoke, not a unit test: not picked up by `node --test`.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const WASM_BYTES = readFileSync(new URL('../pkg/skirmish_sim_bg.wasm', import.meta.url));

// The smoke's own sim instance, for the host's construction hash.
const simMod = await import('../pkg/skirmish_sim.js');
await simMod.default(new Uint8Array(WASM_BYTES));

// ------------------------------------------------------------------ stubs

class El {
  constructor(tag = 'div', id = '') {
    this.tagName = tag.toUpperCase();
    this.id = id;
    this.children = [];
    this.style = {};
    this.dataset = {};
    this.classList = {
      _set: new Set(),
      add(...cs) { cs.forEach((c) => this._set.add(c)); },
      remove(...cs) { cs.forEach((c) => this._set.delete(c)); },
      toggle(c, on) { if (on === undefined) on = !this._set.has(c); on ? this._set.add(c) : this._set.delete(c); },
      contains(c) { return this._set.has(c); },
    };
    this._listeners = {};
    this.hidden = false;
    this.disabled = false;
    this.checked = false;
    this.value = '';
    this.textContent = '';
    this._innerHTML = '';
    this.width = 300;
    this.height = 150;
  }
  get innerHTML() { return this._innerHTML; }
  set innerHTML(v) { this._innerHTML = String(v); this.children = []; }
  addEventListener(ev, fn) { (this._listeners[ev] ||= []).push(fn); }
  removeEventListener() {}
  dispatch(ev, extra = {}) {
    const e = {
      preventDefault() {}, stopPropagation() {},
      target: this, currentTarget: this, ...extra,
    };
    for (const fn of this._listeners[ev] || []) fn(e);
  }
  click() { this.dispatch('click'); }
  append(...kids) { this.children.push(...kids); }
  appendChild(k) { this.children.push(k); return k; }
  querySelectorAll() { return []; }
  querySelector() { return null; }
  closest() { return null; }
  focus() {}
  getContext() { return CTX; }
}

const CTX = new Proxy({}, {
  get(t, p) {
    if (p === 'measureText') return () => ({ width: 10 });
    if (p === 'createLinearGradient' || p === 'createRadialGradient') {
      return () => ({ addColorStop() {} });
    }
    if (p === 'getImageData') return () => ({ data: new Uint8ClampedArray(4) });
    if (typeof p === 'string' && !(p in t)) t[p] = () => {};
    return t[p];
  },
  set(t, p, v) { t[p] = v; return true; },
});

class FakeWS {
  static instances = [];
  constructor(url) {
    this.url = url;
    this.readyState = 0;
    this.listeners = {};
    this.sent = [];
    FakeWS.instances.push(this);
    queueMicrotask(() => {
      if (this.readyState !== 0) return;
      this.readyState = 1;
      this._emit('open', {});
    });
  }
  addEventListener(ev, fn) { (this.listeners[ev] ||= []).push(fn); }
  _emit(ev, data) { for (const fn of this.listeners[ev] || []) fn(data); }
  send(data) {
    this.sent.push(data);
    if (data === 'probe') {
      queueMicrotask(() => {
        this._emit('message', { data: 'probe' });
        queueMicrotask(() => this._emit('message', { data: 'probe' }));
      });
    }
  }
  close() {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this._emit('close', {});
  }
}

function makeStorage() {
  const m = new Map();
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => { m.set(k, String(v)); },
    removeItem: (k) => { m.delete(k); },
    clear: () => m.clear(),
    _map: m,
  };
}

function makeTab(href) {
  const elements = new Map();
  const url = new URL(href);
  const tab = {
    session: makeStorage(),
    local: makeStorage(),
    fetchCalls: [],
    el(id) {
      if (!elements.has(id)) elements.set(id, new El(id === 'view' || id === 'minimap' ? 'canvas' : 'div', id));
      return elements.get(id);
    },
  };
  tab.body = tab.el('body');
  const location = {
    get href() { return url.href; },
    get origin() { return url.origin; },
    get pathname() { return url.pathname; },
    get search() { return url.search; },
    get hash() { return url.hash; },
  };
  const document = {
    getElementById: (id) => tab.el(id),
    createElement: (tag) => new El(tag),
    querySelector: () => null,
    querySelectorAll: () => [],
    addEventListener() {},
    removeEventListener() {},
    body: tab.body,
    documentElement: new El('html'),
    readyState: 'complete',
    hidden: false,
  };
  const history = {
    replaceState: (_a, _b, u) => {
      if (u != null) {
        const next = new URL(u, url.href);
        url.pathname = next.pathname;
        url.search = next.search;
        url.hash = next.hash;
      }
    },
    pushState() {},
  };
  const fetch = async (u) => {
    const s = String(u);
    tab.fetchCalls.push(s);
    if (s.endsWith('.wasm')) {
      // The REAL bytes: the load genuinely instantiates.
      return new Response(new Uint8Array(WASM_BYTES), {
        headers: { 'Content-Type': 'application/wasm' },
      });
    }
    if (s.endsWith('/regions')) {
      return { ok: true, json: async () => ({ regions: [{ id: 'wnam', label: 'North America West' }, { id: 'weur', label: 'Europe West' }] }) };
    }
    if (s.endsWith('/lobbies')) {
      return { ok: true, json: async () => ({ open: [], inProgress: 0 }) };
    }
    return { ok: true, json: async () => ({}) };
  };
  tab.location = location;
  tab.document = document;
  tab.history = history;
  tab.fetch = fetch;
  return tab;
}

class FakeImage {
  constructor() { this.onload = null; this.onerror = null; }
  addEventListener() {}
  removeEventListener() {}
  set src(v) { this._src = v; }
  get src() { return this._src; }
}

function installGlobals(tab) {
  const defs = {
    document: tab.document,
    location: tab.location,
    history: tab.history,
    sessionStorage: tab.session,
    localStorage: tab.local,
    WebSocket: FakeWS,
    fetch: tab.fetch,
    Image: FakeImage,
    navigator: {
      hardwareConcurrency: 4, deviceMemory: 8, maxTouchPoints: 0,
      userAgent: 'prematch-smoke',
      clipboard: { writeText: () => Promise.resolve() },
    },
    window: {
      innerWidth: 1280, innerHeight: 800, devicePixelRatio: 1,
      addEventListener() {}, removeEventListener() {},
    },
    requestAnimationFrame: () => 0,
    cancelAnimationFrame: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
  };
  for (const [k, v] of Object.entries(defs)) {
    Object.defineProperty(globalThis, k, { value: v, configurable: true, writable: true });
  }
}

let bootCount = 0;
async function boot(tab) {
  bootCount += 1;
  FakeWS.instances = [];
  tab.fetchCalls.length = 0;
  installGlobals(tab);
  await import(`../app.js?prematch=${bootCount}`);
  await sleep(400);
}

const inMatch = (tab) => tab.body.classList.contains('in-match');
const roomSocket = () =>
  FakeWS.instances.find((w) => w.url.includes('/room/') && !w.url.includes('probe-'));

// ------------------------------------------------------------------ cases

// (1) Boot: the Lobby Browser is the screen; the pre-match state
//     holds — no .in-match, so the canvas and HUD stay hidden.
{
  const tab = makeTab('https://skirmish.online/play/');
  await boot(tab);
  assert.equal(tab.el('online-overlay').style.display, 'flex', 'lobby overlay is up');
  assert.equal(inMatch(tab), false, 'chrome hidden at boot (no in-match)');
  console.log('PASS (1) boot: lobby visible, game chrome hidden');

  // (2) Play solo: the real load runs and the match starts —
  //     the chrome appears exactly now.
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'chrome visible once the solo match starts');
  assert.equal(tab.el('online-overlay').style.display, 'none', 'lobby closed');
  assert.equal(tab.el('loading-overlay').style.display, 'none', 'loading screen closed');
  console.log('PASS (2) solo start: chrome appears with the match');
}

// (3) A guest whose host cancels mid-loading lands back in the
//     Lobby Browser with the chrome still hidden throughout.
{
  const ROOM = 'guestroom1';
  const tab = makeTab(`https://skirmish.online/play/?room=${ROOM}&region=wnam`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'guest connected to the room');
  assert.equal(inMatch(tab), false, 'chrome hidden in the room screen');
  const hostSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  hostSay({ type: 'hello', v: 1, role: 'host' });
  await sleep(60);
  const seed = 424242;
  const probe = simMod.WasmSim.new_match(seed, 3);
  hostSay({ type: 'start', seed, hash0: probe.state_hash() });
  await sleep(1200); // the guest's real load + build + verify
  assert.equal(tab.el('loading-overlay').style.display, 'flex', 'loading screen is up');
  assert.equal(inMatch(tab), false, 'chrome STILL hidden while loading');
  hostSay({ type: 'cancel' });
  await sleep(150);
  assert.equal(tab.el('loading-overlay').style.display, 'none', 'loading screen closed');
  assert.equal(tab.el('online-overlay').style.display, 'flex', 'back in the Lobby Browser');
  assert.equal(inMatch(tab), false, 'chrome hidden again after the cancel');
  assert.ok(tab.el('lobby-status').textContent.includes('Match cancelled'),
    'the cancellation message shows');
  console.log('PASS (3) loading cancelled: lobby returns, chrome stays hidden');
}

console.log('PREMATCH SMOKE: all cases passed');
process.exit(0);
