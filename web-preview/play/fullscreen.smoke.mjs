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
// the fullscreenchange refit can be exercised. Element dispatch
// is FAITHFUL since the 2026-10-23 fix: an event delivered to an
// element travels the document capture listeners first, exactly
// as in a browser — the journey the pre-fix router's own
// re-dispatched events died on (it intercepted them en route,
// stopped them, and re-dispatched them again, so under a real
// browser every locked click did nothing). The fetch stub
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

// Set by makeTab: runs the tab's document capture listeners
// for an event on its way to an element. El.dispatch consults
// it BEFORE the element's own listeners — the real event path.
let docCaptureHook = null;

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
    let stopped = false;
    const e = {
      preventDefault() {},
      target: this, currentTarget: this, ...extra,
      stopPropagation() { stopped = true; },
    };
    if (docCaptureHook) docCaptureHook(ev, e);
    if (!stopped) {
      for (const fn of this._listeners[ev] || []) fn(e);
    }
    return e;
  }
  dispatchEvent(ev) {
    let stopped = false;
    const e = {
      preventDefault() {},
      target: this, currentTarget: this, ...ev,
      stopPropagation() { stopped = true; },
    };
    if (docCaptureHook) docCaptureHook(ev.type, e);
    if (!stopped) {
      for (const fn of this._listeners[ev.type] || []) fn(e);
    }
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
  // Element dispatches in this tab travel the document capture
  // phase first (see El.dispatch) — the real event path.
  docCaptureHook = (type, e) => {
    for (const l of docListeners.filter((l) => l.type === type && l.capture)) l.fn(e);
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
  assert.equal(tab.el('capture-overlay').hidden, false,
    'a refused capture is VISIBLE now: the click-to-capture pill is up');
  await sleep(200);
  assert.equal(unhandled.length, 0, 'no unhandled rejection escaped');
  console.log('PASS (2) refused APIs: match starts windowed, the pill offers the retry');
}

// (3) No capture APIs at all (iOS Safari on iPhone): the start is
//     exactly the old windowed start.
{
  const tab = makeTab('https://skirmish.online/play/');
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'match starts with no capture APIs present');
  assert.equal(tab.el('capture-overlay').hidden, true,
    'no capture pill where pointer lock does not exist (no nagging on iOS)');
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

// (5) Locked routing + the menu UNDER lock: a click over a HUD
//     button is re-dispatched to it and DELIVERED (this is the
//     regression case for "ao clicar não acontece nada": with
//     faithful dispatch the synthetic event travels back through
//     the document capture listeners, where the pre-fix router
//     intercepted and killed it). A press over the canvas passes
//     through untouched. The Menu button opens the pause menu
//     WITHOUT unlocking (Claudio's design call, 2026-10-23: Esc
//     exits capture, the menu does not); menu items respond at
//     the virtual cursor; Resume closes the menu still captured.
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
  // Click over the top bar's Menu button: intercepted + routed
  // + DELIVERED — the pause menu opens, still captured.
  tab.hitEl = tab.el('pause-btn');
  stopped = false;
  tab.fireDocument('click', { button: 0, stopPropagation: () => { stopped = true; } });
  assert.equal(stopped, true, 'HUD click is intercepted under lock');
  assert.equal(tab.el('pause-menu').style.display, 'flex', 'the routed Menu click opened the pause menu');
  assert.equal(calls.exitLock, 0, 'opening the menu does NOT hand the pointer back');
  assert.equal(tab.document.pointerLockElement, tab.el('view'), 'still locked with the menu open');
  assert.equal(tab.el('lock-cursor').hidden, false, 'the game cursor stays up over the menu');
  // A second menu item under lock: the menu's Fullscreen toggle.
  const fsBefore = calls.fs;
  tab.hitEl = tab.el('pause-fullscreen-btn');
  tab.fireDocument('click', { button: 0 });
  assert.ok(calls.fs > fsBefore, 'the menu Fullscreen toggle responds under lock');
  // Resume, clicked at the virtual cursor while still locked:
  // the menu closes and capture is untouched.
  tab.hitEl = tab.el('resume-btn');
  tab.fireDocument('click', { button: 0 });
  assert.equal(tab.el('pause-menu').style.display, 'none', 'the routed Resume click closed the menu');
  assert.equal(tab.document.pointerLockElement, tab.el('view'), 'still locked after Resume');
  assert.equal(calls.exitLock, 0, 'the lock was never exited for the menu');
  console.log('PASS (5) routing DELIVERS: menu opens/works/closes under lock, canvas passes');
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

// (9) The pill is a working recovery, not a label: after a
//     refused capture it is up; clicking it re-runs the whole
//     chain (fullscreen + lock) from its fresh gesture; when the
//     browser then grants the lock, it hides.
{
  const tab = makeTab('https://skirmish.online/play/');
  const calls = installCaptureApis(tab, 'reject');
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'match running');
  assert.equal(tab.el('capture-overlay').hidden, false, 'pill up after the refused capture');
  const fsBefore = calls.fs; const lockBefore = calls.lock;
  tab.el('capture-overlay').click();
  await sleep(120);
  assert.ok(calls.fs > fsBefore, 'pill click re-requests fullscreen');
  assert.ok(calls.lock > lockBefore, 'pill click re-requests the lock');
  // The browser grants this time:
  tab.document.pointerLockElement = tab.el('view');
  tab.fireDocument('pointerlockchange', {}, false);
  assert.equal(tab.el('capture-overlay').hidden, true, 'pill hides once the lock is held');
  console.log('PASS (9) pill: visible after refusal, one click re-runs the chain, hides on lock');
}

// (10) Esc: the browser drops the lock (the accepted way out).
//      The game stays playable windowed, the pill is the way
//      back, and one click re-captures — the player is never
//      trapped and never loses the mouse by accident.
{
  const tab = makeTab('https://skirmish.online/play/');
  const calls = installCaptureApis(tab);
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  tab.document.pointerLockElement = tab.el('view');
  tab.fireDocument('pointerlockchange', {}, false);
  assert.equal(tab.el('capture-overlay').hidden, true, 'no pill while captured');
  // Esc: pointerlockchange with no element.
  tab.document.pointerLockElement = null;
  tab.fireDocument('pointerlockchange', {}, false);
  assert.equal(tab.el('capture-overlay').hidden, false, 'Esc leaves the pill as the way back');
  const lockBefore = calls.lock;
  tab.el('capture-overlay').click();
  await sleep(120);
  assert.ok(calls.lock > lockBefore, 'one click re-captures after Esc');
  console.log('PASS (10) Esc: clean exit, pill up, one click re-captures');
}

// (11) The top edge, held hard (the headline acceptance): with
//      the lock granted, repeated maximal upward movement keeps
//      the virtual cursor ON the top edge — the position every
//      coordinate read uses — so edge scroll pans up and the
//      real cursor (frozen by the lock) never exists to reach
//      the browser chrome. Asserted over several seconds of
//      frames: the cursor never leaves the edge, and edgePan
//      reads "up" on every frame.
{
  const tab = makeTab('https://skirmish.online/play/');
  installCaptureApis(tab);
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  tab.document.pointerLockElement = tab.el('view');
  tab.fireDocument('pointerlockchange', {}, false);
  const cursor = tab.el('lock-cursor');
  for (let frame = 0; frame < 180; frame++) { // ~3 s at 60 fps
    tab.fireDocument('mousemove', { movementX: 0, movementY: -500 });
    const m = /translate\((-?\d+)px, (-?\d+)px\)/.exec(cursor.style.transform);
    assert.ok(m, 'cursor transform readable');
    assert.equal(Number(m[2]), 0, `frame ${frame}: virtual cursor stays ON the top edge`);
    assert.equal(edgePan(640, 0, 1280, 800, 6).y, 1, `frame ${frame}: the view pans up`);
  }
  console.log('PASS (11) top edge held 180 frames: cursor pinned to the edge, view pans up');
}

// (12) Escape NEVER shows the pause menu (Claudio, 2026-10-23:
//      "a única forma de mostrar o menu é apertar menu, e ESC é
//      para sair do modo tela cheia"). The Menu button is the
//      ONLY way in — and it toggles back out, windowed and under
//      lock alike. Esc is the browser's key (leave fullscreen /
//      lock); in-game it only backs out of transient states.
//      The help overlay is pre-dismissed so each Escape falls
//      through every transient branch to where the pause toggle
//      used to live.
{
  const tab = makeTab('https://skirmish.online/play/');
  const calls = installCaptureApis(tab);
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'match running');
  const menuShown = () => tab.el('pause-menu').style.display === 'flex';
  assert.equal(menuShown(), false, 'menu starts hidden');
  tab.el('help-overlay').style.display = 'none';
  // Windowed: Escape does not open the menu…
  tab.fireWindow('keydown', { code: 'Escape' });
  assert.equal(menuShown(), false, 'Escape does NOT show the pause menu (windowed)');
  // …the Menu button does…
  tab.el('pause-btn').click();
  assert.equal(menuShown(), true, 'the Menu button shows the pause menu');
  // …Escape does not act on the open menu either…
  tab.fireWindow('keydown', { code: 'Escape' });
  assert.equal(menuShown(), true, 'Escape leaves the open menu alone');
  // …and the Menu button toggles it back closed.
  tab.el('pause-btn').click();
  assert.equal(menuShown(), false, 'the Menu button hides the pause menu again');
  // Captured: the same rule under pointer lock.
  tab.document.pointerLockElement = tab.el('view');
  tab.fireDocument('pointerlockchange', {}, false);
  tab.fireWindow('keydown', { code: 'Escape' });
  assert.equal(menuShown(), false, 'Escape does NOT show the pause menu (captured)');
  tab.hitEl = tab.el('pause-btn');
  tab.fireDocument('click', { button: 0 });
  assert.equal(menuShown(), true, 'the routed Menu click opens the menu under lock');
  tab.fireDocument('click', { button: 0 });
  assert.equal(menuShown(), false, 'a second routed Menu click closes it under lock');
  assert.equal(tab.document.pointerLockElement, tab.el('view'), 'still locked throughout');
  assert.equal(calls.exitLock, 0, 'the lock was never exited for the menu');
  console.log('PASS (12) Escape never shows the menu; the Menu button toggles it, windowed + locked');
}

// (13) Keyboard Lock for Escape (Claudio, 2026-10-26 design;
//      bot-loop front (3), 2026-10-28): entering fullscreen locks
//      Escape via navigator.keyboard.lock(["Escape"]) so the
//      game's Esc stack receives the key; leaving fullscreen
//      unlocks it. Without the API (Safari/Firefox) the
//      fullscreenchange path must run cleanly with no error.
{
  const tab = makeTab('https://skirmish.online/play/');
  const calls = installCaptureApis(tab);
  await boot(tab);
  tab.el('btn-solo').click();
  await sleep(1500);
  assert.equal(inMatch(tab), true, 'match running');
  // No keyboard API in this tab's navigator: entering and
  // leaving fullscreen must not throw (the old behaviour).
  tab.document.fullscreenElement = tab.document.documentElement;
  tab.fireDocument('fullscreenchange', {}, false);
  tab.document.fullscreenElement = null;
  tab.fireDocument('fullscreenchange', {}, false);
  // With the API present: lock on entry, unlock on exit.
  const kbCalls = { lock: [], unlock: 0 };
  globalThis.navigator.keyboard = {
    lock: (keys) => { kbCalls.lock.push(keys); return Promise.resolve(); },
    unlock: () => { kbCalls.unlock += 1; },
  };
  tab.document.fullscreenElement = tab.document.documentElement;
  tab.fireDocument('fullscreenchange', {}, false);
  assert.deepEqual(kbCalls.lock, [['Escape']], 'entering fullscreen locks Escape');
  tab.document.fullscreenElement = null;
  tab.fireDocument('fullscreenchange', {}, false);
  assert.equal(kbCalls.unlock, 1, 'leaving fullscreen unlocks Escape');
  delete globalThis.navigator.keyboard;
  // Portrait click with a selection-less panel: a plain no-op
  // that must not throw (the deselect wiring is pinned in
  // esclock.test.mjs).
  tab.el('sel-portrait').click();
  console.log('PASS (13) Keyboard Lock: Escape locked on fullscreen entry, unlocked on exit; no-API path clean');
}

assert.equal(unhandled.length, 0, 'no unhandled rejection in any case');
console.log('FULLSCREEN SMOKE: all cases passed');
process.exit(0);
