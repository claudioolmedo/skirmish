// Stub-DOM smoke for fullscreen + pointer lock on match start
// (Claudio, 2026-10-23 — the GeForce NOW model): entering a match
// asks for fullscreen and captures the pointer; under lock a
// VIRTUAL cursor (accumulated movementX/Y, clamped to the
// viewport) drives every coordinate read, the game draws its own
// cursor, and HUD events are re-dispatched to the element under
// the virtual cursor. Every failure of either API must be silent:
// the match starts windowed, exactly as before.
//
// Drives the REAL ../app.js in Node against a stub DOM whose
// document/window listener registries are REAL (unlike the
// prematch smoke's no-op ones), so the capture-phase router and
// the fullscreenchange refit can be exercised. The fetch stub
// serves the REAL .wasm bytes, so starts genuinely run.
// Run from web-preview/play/:
//   node fullscreen.smoke.mjs
// A smoke, not a unit test: not picked up by `node --test`.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { edgePan } from './hotkeys.js';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const WASM_BYTES = readFileSync(new URL('../pkg/skirmish_sim_bg.wasm', import.meta.url));

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
    return e;
  }
  dispatchEvent(ev) {
    const e = {
      preventDefault() {}, stopPropagation() {},
      target: this, currentTarget: this, ...ev,
    };
    for (const fn of this._listeners[ev.type] || []) fn(e);
    return true;
  }
  click() { this.dispatch('click'); }
  append(...kids) { this.children.push(...kids); }
  appendChild(k) { this.children.push(k); return k; }
  querySelectorAll() { return []; }
  querySelector() { return null; }
  closest() { return null; }
  focus() {}
  setPointerCapture() {}
  releasePointerCapture() {}
  getBoundingClientRect() {
    return {
      left: 0, top: 0, width: this.width, height: this.height,
      right: this.width, bottom: this.height,
    };
  }
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
    hitEl: null, // what document.elementFromPoint reports
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
  // Unlike the prematch stub, document/window listener registries
  // are real: the capture router and the change listeners must be
  // reachable by the cases below.
  const docListeners = [];
  const document = {
    getElementById: (id) => tab.el(id),
    createElement: (tag) => new El(tag),
    querySelector: () => null,
    querySelectorAll: () => [],
    addEventListener(type, fn, opts) {
      docListeners.push({ type, fn, capture: opts === true || !!(opts && opts.capture) });
    },
    removeEventListener() {},
    body: tab.body,
    documentElement: new El('html'),
    readyState: 'complete',
    hidden: false,
    fullscreenElement: null,
    pointerLockElement: null,
    elementFromPoint: () => tab.hitEl,
  };
  tab.fireDocument = (type, ev = {}, capture = true) => {
    const e = {
      preventDefault() {}, stopPropagation() {},
      target: document, currentTarget: document, ...ev,
    };
    if (ev.stopPropagation) e.stopPropagation = ev.stopPropagation;
    for (const l of docListeners.filter((l) => l.type === type && l.capture === capture)) l.fn(e);
    return e;
  };
  const winListeners = [];
  tab.winListeners = winListeners;
  tab.fireWindow = (type, ev = {}) => {
    const e = {
      preventDefault() {}, stopPropagation() {},
      target: null, currentTarget: null, ...ev,
    };
    for (const l of winListeners.filter((l) => l.type === type)) l.fn(e);
    return e;
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
  tab.win = {
    innerWidth: 1280, innerHeight: 800, devicePixelRatio: 1,
    addEventListener(type, fn, opts) {
      winListeners.push({ type, fn, capture: opts === true });
    },
    removeEventListener() {},
  };
  tab.location = location;
  tab.document = document;
  tab.history = history;
  tab.fetch = fetch;
  return tab;
}

class FakeImage {
  static plan = { mode: 'load', delayMs: 0 };
  constructor() {
    this.onload = null; this.onerror = null;
    this._listeners = {};
    this.complete = false;
    this.naturalWidth = 0;
    this.naturalHeight = 0;
  }
  addEventListener(ev, fn) { (this._listeners[ev] ||= []).push(fn); }
  removeEventListener(ev, fn) {
    const l = this._listeners[ev] || [];
    const i = l.indexOf(fn);
    if (i >= 0) l.splice(i, 1);
  }
  set src(v) {
    this._src = v;
    const plan = FakeImage.plan;
    if (plan.mode === 'hang') return;
    setTimeout(() => {
      this.complete = true;
      this.naturalWidth = 64;
      this.naturalHeight = 64;
      for (const fn of this._listeners.load || []) fn({});
      if (this.onload) this.onload({});
    }, plan.delayMs || 0);
  }
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
      userAgent: 'fullscreen-smoke',
      clipboard: { writeText: () => Promise.resolve() },
    },
    window: tab.win,
    requestAnimationFrame: () => 0,
    cancelAnimationFrame: () => {},
    addEventListener: tab.win.addEventListener.bind(tab.win),
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
  await import(`../app.js?fullscreen=${bootCount}`);
  await sleep(400);
}

const inMatch = (tab) => tab.body.classList.contains('in-match');
const roomSocket = () =>
  FakeWS.instances.find((w) => w.url.includes('/room/') && !w.url.includes('probe-'));

// Install capture APIs on a tab with call counters. mode:
// 'resolve' | 'reject' for the promises they return.
function installCaptureApis(tab, mode = 'resolve') {
  const calls = { fs: 0, lock: 0, exitFs: 0, exitLock: 0 };
  const settle = () => (mode === 'reject'
    ? Promise.reject(new Error('api refused (smoke)'))
    : Promise.resolve());
  tab.document.documentElement.requestFullscreen = () => { calls.fs += 1; return settle(); };
  tab.el('view').requestPointerLock = () => { calls.lock += 1; return settle(); };
  tab.document.exitFullscreen = () => { calls.exitFs += 1; tab.document.fullscreenElement = null; };
  tab.document.exitPointerLock = () => {
    calls.exitLock += 1;
    tab.document.pointerLockElement = null;
    tab.fireDocument('pointerlockchange', {}, false);
  };
  return calls;
}

const unhandled = [];
process.on('unhandledRejection', (e) => unhandled.push(e));

// ------------------------------------------------------------------ cases

// (1) Solo start: fullscreen AND pointer lock are requested from
//     the Play gesture; the match starts. The manual toggle still
//     works off the real fullscreen state.
{
  const tab = makeTab('https://skirmish.online/play/');
  const calls = installCaptureApis(tab);
  await boot(tab);
  assert.equal(calls.fs, 0, 'no fullscreen before Play');
  assert.equal(calls.lock, 0, 'no lock before Play');
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'solo match started');
  assert.equal(calls.fs, 1, 'fullscreen requested once at Play');
  assert.equal(calls.lock, 1, 'pointer lock requested once at Play (chained)');
  // Toggle: state-driven via fullscreenchange.
  tab.document.fullscreenElement = tab.document.documentElement;
  tab.fireDocument('fullscreenchange', {}, false);
  assert.equal(tab.el('fullscreen-btn').textContent, '⤢', 'glyph tracks fullscreen state');
  tab.el('fullscreen-btn').click();
  assert.equal(calls.exitFs, 1, 'toggle exits fullscreen');
  console.log('PASS (1) solo start: fullscreen + pointer lock requested; toggle intact');
}

// (2) Both APIs refuse (rejected promises — iframe without
//     allow="fullscreen", the re-lock cooldown): the match still
//     starts, and no rejection escapes.
{
  const tab = makeTab('https://skirmish.online/play/');
  const calls = installCaptureApis(tab, 'reject');
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'match starts despite refused capture');
  assert.equal(calls.fs, 1, 'fullscreen was attempted');
  assert.equal(calls.lock, 1, 'lock was attempted after the refusal');
  await sleep(200);
  assert.equal(unhandled.length, 0, 'no unhandled rejection escaped');
  console.log('PASS (2) refused APIs: silent, the match starts windowed');
}

// (3) No capture APIs at all (iOS Safari on iPhone): the start is
//     exactly the old windowed start.
{
  const tab = makeTab('https://skirmish.online/play/');
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'match starts with no capture APIs present');
  console.log('PASS (3) no APIs: windowed start, unchanged');
}

// (4) The virtual cursor: seeded where the real cursor was
//     captured, accumulated from movementX/Y, clamped to the
//     viewport — pushed hard up it sits ON the top edge, where
//     edgePan keeps panning up.
{
  const tab = makeTab('https://skirmish.online/play/');
  const calls = installCaptureApis(tab);
  await boot(tab);
  tab.fireWindow('mousemove', { clientX: 640, clientY: 400, movementX: 0, movementY: 0 });
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'match running');
  // The browser grants the lock:
  tab.document.pointerLockElement = tab.el('view');
  tab.fireDocument('pointerlockchange', {}, false);
  const cursor = tab.el('lock-cursor');
  assert.equal(cursor.hidden, false, 'the game cursor shows under lock');
  assert.equal(cursor.style.transform, 'translate(640px, 400px)', 'virtual cursor seeded at the capture point');
  // Shove the mouse up-left far past the corner:
  tab.fireDocument('mousemove', { movementX: -100000, movementY: -100000 });
  assert.equal(cursor.style.transform, 'translate(0px, 0px)', 'virtual cursor clamps to the viewport corner');
  // And hard down-right:
  tab.fireDocument('mousemove', { movementX: 100000, movementY: 100000 });
  assert.equal(cursor.style.transform, 'translate(1280px, 800px)', 'virtual cursor clamps to the far corner');
  // The top edge specifically: cursor resting ON it pans up.
  tab.fireDocument('mousemove', { movementX: -640, movementY: -100000 });
  assert.equal(cursor.style.transform, 'translate(640px, 0px)', 'cursor rests on the top edge');
  assert.equal(edgePan(640, 0, 1280, 800, 6).y, 1, 'edgePan pans up at the top edge');
  assert.equal(edgePan(640, 400, 1280, 800, 6).y, 0, 'no pan mid-screen');
  console.log('PASS (4) virtual cursor: seeded, accumulated, clamped; top edge pans');
}

// (5) Locked routing: a click over a HUD button is re-dispatched
//     to it (the pause menu opens, and hands the pointer back);
//     a press over the canvas passes through untouched; Resume
//     re-captures.
{
  const tab = makeTab('https://skirmish.online/play/');
  const calls = installCaptureApis(tab);
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  tab.document.pointerLockElement = tab.el('view');
  tab.fireDocument('pointerlockchange', {}, false);
  // Click over the canvas: NOT intercepted.
  tab.hitEl = tab.el('view');
  let stopped = false;
  tab.fireDocument('mousedown', { button: 0, stopPropagation: () => { stopped = true; } });
  assert.equal(stopped, false, 'canvas press passes through under lock');
  // Click over the top bar's Menu button: intercepted + routed.
  tab.hitEl = tab.el('pause-btn');
  stopped = false;
  tab.fireDocument('click', { button: 0, stopPropagation: () => { stopped = true; } });
  assert.equal(stopped, true, 'HUD click is intercepted under lock');
  assert.equal(tab.el('pause-menu').style.display, 'flex', 'the routed click opened the pause menu');
  assert.equal(calls.exitLock, 1, 'opening the menu handed the pointer back');
  assert.equal(tab.el('lock-cursor').hidden, true, 'game cursor hides with the lock');
  // Resume (a real click, unlocked): re-captures.
  const lockBefore = calls.lock;
  tab.el('resume-btn').click();
  await sleep(60);
  assert.equal(tab.el('pause-menu').style.display, 'none', 'menu closed');
  assert.ok(calls.lock > lockBefore, 'Resume re-requests the lock');
  console.log('PASS (5) routing: HUD clicks land, canvas passes, Resume re-captures');
}

// (6) fullscreenchange refits the canvas backing store.
{
  const tab = makeTab('https://skirmish.online/play/');
  installCaptureApis(tab);
  await boot(tab);
  const view = tab.el('view');
  const w0 = view.width; const h0 = view.height;
  tab.win.innerWidth = 1920;
  tab.win.innerHeight = 1080;
  tab.fireDocument('fullscreenchange', {}, false);
  assert.equal(view.width, Math.round(1920 * (w0 / 1280)), 'canvas refits on fullscreenchange (width)');
  assert.equal(view.height, Math.round(1080 * (h0 / 800)), 'canvas refits on fullscreenchange (height)');
  console.log('PASS (6) fullscreenchange: canvas backing store refit');
}

// (7) The room's Start button: readying up requests the capture
//     from the guest's own click — the later 'go' carries no
//     gesture, so this click is the only chance.
{
  const ROOM = 'guestroom7';
  const tab = makeTab(`https://skirmish.online/play/?room=${ROOM}&region=wnam`);
  const calls = installCaptureApis(tab);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'guest connected to the room');
  sock._emit('message', { data: JSON.stringify({ type: 'hello', v: 1, role: 'host' }) });
  await sleep(60);
  tab.el('btn-ready').click();
  await sleep(120);
  assert.equal(calls.fs, 1, 'fullscreen requested at the Start button');
  assert.equal(calls.lock, 1, 'pointer lock requested at the Start button');
  console.log('PASS (7) room Start: capture requested from the ready click');
}

// (8) Restart from the pause menu is a start gesture too.
{
  const tab = makeTab('https://skirmish.online/play/');
  const calls = installCaptureApis(tab);
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'match running');
  const fsBefore = calls.fs; const lockBefore = calls.lock;
  tab.el('restart-btn').click();
  await sleep(300);
  assert.ok(calls.fs > fsBefore, 'Restart re-requests fullscreen');
  assert.ok(calls.lock > lockBefore, 'Restart re-requests the lock');
  assert.equal(inMatch(tab), true, 'still in the fresh match');
  console.log('PASS (8) Restart: capture re-requested');
}

assert.equal(unhandled.length, 0, 'no unhandled rejection in any case');
console.log('FULLSCREEN SMOKE: all cases passed');
process.exit(0);
