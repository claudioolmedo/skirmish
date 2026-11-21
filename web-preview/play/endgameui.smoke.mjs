// Endgame-UI regression smoke (stub DOM; NOT part of `node
// --test` — named *.smoke.mjs on purpose). Two client defects,
// live-confirmed in bot-loop runs #449 and #451 on the real
// client:
//
// (a) SCORE GRAPH EMPTY: on the Defeat screen the Score graph
// box rendered as an empty cream rectangle while the score
// TABLE above it was filled. Mechanism (proven by the recorded
// canvas calls on pristine 43a16e5): the timeline's samples are
// per-minute (sampleScore) and the final totals at the decision
// tick were never sampled, so a match ending inside its last
// minute — the usual Resign — reached drawScoreTimeline with
// zero or one points; a one-point series strokes a lone moveTo
// (no segment, nothing visible) and a zero-point history fell
// back to [[0,0]]. Fix: renderEndStats appends the final totals
// once (sampleScoreFinal) and every sample draws a marker dot,
// so lone points render too. This smoke records the
// #end-timeline canvas calls and requires real segments AND
// markers at the final scores.
//
// (b) RESIGN LEAK: Resign -> end screen -> "Back to menu" ->
// Lobby Browser -> close (x) revealed the RESIGNED match still
// sitting paused underneath (frozen clock, pop 0/5). Mechanism:
// Back to menu called openLobby with matchEnded still standing,
// so openLobby derived lobbyReturnToEnded and the lobby offered
// the finished-match return that belongs ONLY to the
// Play-online-over-a-finished-match path (lobbytrap.smoke.mjs).
// Fix: the end card's Back-to-menu handler clears the
// match-standing flags (playing / matchEnded / paused) before
// openLobby, so this lobby is the front door: no x is offered
// and no close can resurrect the dead match. The lobbytrap
// smoke separately proves the Play-online return still works.
//
// Scenario: solo -> pump a few frames (minute-0 sample) ->
// Pause -> Resign -> end overlay -> timeline calls checked ->
// Back to menu -> lobby up, x HIDDEN -> click x anyway (the
// handler guard) -> lobby still up, no game view, no pause
// menu, no end card. Harness copied from lobbytrap.smoke.mjs
// (same stub DOM + real WASM sim), plus a recording 2d context
// for #end-timeline.
//
// Run:  node web-preview/play/endgameui.smoke.mjs
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';

const HERE = dirname(fileURLToPath(import.meta.url));

const WASM = readFileSync(join(HERE, '..', 'pkg', 'skirmish_sim_bg.wasm'));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ------------------------------------------------------------- DOM stub
let ctxProxy;
function makeCtx() {
  if (!ctxProxy) {
    ctxProxy = new Proxy({}, {
      get: (t, k) => {
        if (k === 'canvas') return { width: 0, height: 0 };
        if (k === 'measureText') return () => ({ width: 10 });
        if (k === 'getImageData') {
          return (x, y, w, h) => ({ data: new Uint8ClampedArray(Math.max(4, w * h * 4)) });
        }
        if (k === 'createLinearGradient' || k === 'createRadialGradient') {
          return () => ({ addColorStop() {} });
        }
        if (typeof k === 'string') return () => {};
        return undefined;
      },
      set: () => true,
    });
  }
  return ctxProxy;
}

class ClassList {
  constructor() { this.s = new Set(); }
  add(...c) { c.forEach((x) => this.s.add(x)); }
  remove(...c) { c.forEach((x) => this.s.delete(c)); }
  toggle(c, force) {
    if (force === undefined) { this.s.has(c) ? this.s.delete(c) : this.s.add(c); }
    else if (force) this.s.add(c); else this.s.delete(c);
  }
  contains(c) { return this.s.has(c); }
}

class El {
  constructor(tag = 'div', id = '') {
    this.tagName = String(tag).toUpperCase();
    this.id = id;
    this.children = [];
    this.style = {};
    this.dataset = {};
    this.classList = new ClassList();
    this.className = '';
    this.textContent = '';
    this._innerHTML = '';
    this.value = '';
    this.checked = false;
    this.disabled = false;
    this.hidden = false;
    this.title = '';
    this.listeners = {};
    this.width = 300;
    this.height = 150;
    this.lastViewport = null;
  }
  get innerHTML() { return this._innerHTML; }
  set innerHTML(v) { this._innerHTML = v; if (v === '') this.children = []; }
  addEventListener(t, fn) { (this.listeners[t] ||= []).push(fn); }
  removeEventListener() {}
  dispatch(t, ev = {}) {
    const e = { preventDefault() {}, stopPropagation() {}, target: this, ...ev };
    for (const fn of this.listeners[t] || []) fn(e);
    return e;
  }
  click() { this.dispatch('click'); }
  append(...kids) { this.children.push(...kids); }
  appendChild(c) { this.children.push(c); c.parentElement = this; return c; }
  prepend(c) { this.children.unshift(c); return c; }
  removeChild(c) { this.children = this.children.filter((x) => x !== c); return c; }
  remove() {}
  querySelector() { return null; }
  querySelectorAll() { return []; }
  closest() { return null; }
  getBoundingClientRect() { return { left: 0, top: 0, width: 300, height: 150 }; }
  getContext() {
    if (this.id === 'end-timeline') {
      if (!this._tlCtx) {
        const el = this; el.tlCalls = [];
        this._tlCtx = new Proxy({}, {
          get: (t, k) => {
            if (k === 'canvas') return el;
            if (typeof k === 'string') return (...a) => { el.tlCalls.push([k, ...a]); };
            return undefined;
          },
          set: (t, k, v) => { el.tlCalls.push(['set:'+String(k), v]); return true; },
        });
      }
      return this._tlCtx;
    }
    if (this.id !== 'minimap') return makeCtx();
    if (!this._mmCtx) {
      const el = this;
      this._mmCtx = new Proxy({}, {
        get: (t, k) => {
          if (k === 'canvas') return el;
          if (k === 'strokeRect') {
            return (x, y, w, h) => { el.lastViewport = { x, y, w, h }; };
          }
          if (k === 'createLinearGradient' || k === 'createRadialGradient') {
            return () => ({ addColorStop() {} });
          }
          if (typeof k === 'string') return () => {};
          return undefined;
        },
        set: () => true,
      });
    }
    return this._mmCtx;
  }
  focus() {}
  select() {}
  setPointerCapture() {}
  releasePointerCapture() {}
}

function makeStorage() {
  const m = new Map();
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    clear: () => m.clear(),
  };
}

class FakeWS {
  static instances = [];
  constructor(url) {
    this.url = String(url);
    this.readyState = 0;
    this.sent = [];
    this.handlers = {};
    FakeWS.instances.push(this);
    queueMicrotask(() => {
      this.readyState = 1;
      this._emit('open', {});
      if (this.url.includes('probe-')) {
        for (let i = 0; i < 2; i += 1) {
          setTimeout(() => { try { this._emit('message', { data: 'probe' }); } catch { /* closed */ } }, 5 + i * 5);
        }
      }
    });
  }
  addEventListener(t, fn) { (this.handlers[t] ||= []).push(fn); }
  removeEventListener() {}
  _emit(t, ev) { for (const fn of this.handlers[t] || []) fn(ev); }
  send(data) { this.sent.push(String(data)); }
  close() {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this._emit('close', {});
  }
}

class FakeImage {
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
    setTimeout(() => {
      this.complete = true;
      this.naturalWidth = 64;
      this.naturalHeight = 64;
      for (const fn of this._listeners.load || []) fn({});
      if (this.onload) this.onload({});
    }, 0);
  }
  get src() { return this._src; }
}

const winListeners = {};
const docListeners = {};
let rafCb = null;

function makeTab(href) {
  const elements = new Map();
  const url = new URL(href);
  const tab = {
    session: makeStorage(),
    local: makeStorage(),
    fetchCalls: [],
    feedbackFail: false,
    el(id) {
      if (!elements.has(id)) elements.set(id, new El(id === 'view' || id === 'minimap' ? 'canvas' : 'div', id));
      return elements.get(id);
    },
  };
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
    addEventListener: (t, fn) => { (docListeners[t] ||= []).push(fn); },
    removeEventListener() {},
    body: new El('body'),
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
  const fetch = async (u, opts = {}) => {
    const s = String(u);
    tab.fetchCalls.push({ url: s, method: opts.method || 'GET', body: opts.body || null });
    if (s.endsWith('.wasm')) {
      return new Response(new Uint8Array(WASM), {
        headers: { 'Content-Type': 'application/wasm' },
      });
    }
    if (s.endsWith('/feedback')) {
      return tab.feedbackFail
        ? { ok: false, status: 500, json: async () => ({}) }
        : { ok: true, status: 204, json: async () => ({}) };
    }
    if (s.endsWith('/regions')) {
      return { ok: true, json: async () => ({ regions: [{ id: 'wnam', label: 'North America West' }] }) };
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

function installGlobals(tab) {
  const on = (store) => (t, fn) => { (store[t] ||= []).push(fn); };
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
      userAgent: 'feedback-smoke',
      clipboard: { writeText: () => Promise.resolve() },
    },
    window: {
      innerWidth: 1280, innerHeight: 800, devicePixelRatio: 1,
      addEventListener: on(winListeners), removeEventListener() {},
    },
    requestAnimationFrame: (cb) => { rafCb = cb; return 1; },
    cancelAnimationFrame: () => {},
    addEventListener: on(winListeners),
    removeEventListener: () => {},
  };
  for (const [k, v] of Object.entries(defs)) {
    Object.defineProperty(globalThis, k, { value: v, configurable: true, writable: true });
  }
}

// ------------------------------------------------------------- driving
const tab = makeTab('https://skirmish.online/play/?mode=normal');
installGlobals(tab);
for (const id of ['pause-menu', 'help-overlay', 'end-overlay',
  'online-overlay', 'desync-overlay', 'peer-overlay', 'loading-overlay',
  'score-overlay', 'techtree-overlay']) {
  tab.el(id).style.display = 'none';
}
await import('../app.js');
await sleep(400);
assert.equal(tab.el('online-overlay').style.display, 'flex', 'lobby up at boot');

tab.el('btn-solo').click();
let began = false;
for (let i = 0; i < 40 && !began; i += 1) {
  await sleep(250);
  began = tab.document.body.classList.contains('in-match');
}
assert.ok(began, 'AI Normal match begins after the real load');

let nowMs = 1000;
function pump(n = 1, step = 250) {
  for (let i = 0; i < n; i += 1) {
    nowMs += step;
    const cb = rafCb;
    rafCb = null;
    assert.ok(cb, 'the frame loop stays registered');
    cb(nowMs);
  }
}

// Play (passively) until the end overlay rises. The seed-7 AI
// Normal match defeats an idle player at ~133k ticks; the frame
// loop advances at most 9 ticks per pumped frame.
function playToEnd(label) {
  let frames = 0;
  while (tab.el('end-overlay').style.display !== 'flex') {
    pump(1);
    frames += 1;
    assert.ok(frames < 120000, `${label}: the match ends (frames=${frames})`);
  }
  return frames;
}

const inMatch = () => tab.document.body.classList.contains('in-match');



// ------------------------------------------------------------ scenario
// (The solo match was started by the driving section above.)
// A few live frames: the per-minute sampler records minute 0.
pump(10);

// Resign: Pause -> Resign, then pump until the end overlay rises.
tab.el('pause-btn').click();
assert.equal(tab.el('pause-menu').style.display, 'flex', 'pause menu open');
tab.el('btn-resign').click();
{
  let frames = 0;
  while (tab.el('end-overlay').style.display !== 'flex') {
    pump(1);
    frames += 1;
    assert.ok(frames < 5000, `the resigned match ends (frames=${frames})`);
  }
}

// (a) The timeline actually drew the score history: with the
// minute-0 sample plus the appended final sample each series is
// a real segment (lineTo beyond the baseline's single lineTo),
// and every sample carries a marker (arc).
const tlCalls = tab.el('end-timeline').tlCalls || [];
const lineTos = tlCalls.filter((c) => c[0] === 'lineTo').length;
const arcs = tlCalls.filter((c) => c[0] === 'arc').length;
assert.ok(lineTos >= 3,
  `(a) score series draw segments, not lone moveTos (lineTo=${lineTos})`);
assert.ok(arcs >= 4,
  `(a) every sample draws a marker (arc=${arcs})`);

// (b) Back to menu: the lobby is the front door — no close is
// offered, and even a dispatched close cannot reveal the dead
// resigned match paused underneath.
tab.el('end-menu-btn').click();
assert.equal(tab.el('online-overlay').style.display, 'flex', 'Back to menu lands on the Lobby Browser');
assert.equal(inMatch(), false, 'game view hidden while the lobby is up');
assert.equal(tab.el('online-close').style.display, 'none',
  '(b) the lobby offers NO close over a match left via Back to menu');
tab.el('view').width = 999;
tab.el('online-close').click();
assert.equal(tab.el('online-overlay').style.display, 'flex',
  '(b) a dispatched close leaves the front-door lobby up');
assert.equal(inMatch(), false, '(b) closing the lobby does NOT reveal the resigned match');
assert.equal(tab.el('pause-menu').style.display, 'none',
  '(b) no pause menu sits over a dead match after the close');
assert.equal(tab.el('end-overlay').style.display, 'none',
  '(b) the end card does not come back after the close');

console.log('PASS endgame-ui: score graph draws segments+markers; Back-to-menu lobby cannot reveal the resigned match');
console.log('ENDGAMEUI SMOKE: all cases passed');
process.exit(0);
