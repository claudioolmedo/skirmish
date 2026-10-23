// Orders & stances smoke (CMB-09/10/11; stub DOM; NOT part of
// `node --test` — the file is named *.smoke.mjs on purpose).
// Boots a real sandbox match against the real WASM sim and
// drives it the way a player would: synthetic keydown events
// and panel clicks through the app's own listeners (the harness
// is auto-explore.smoke.mjs's, verbatim).
//
// Proves the client half of the orders package: selecting the
// starting Scout Cavalry shows the DE-form military panel —
// Attack Move (badged R, DE's dedicated key), Patrol (badged
// with its grid letter W), the four stance faces with the
// scout's Stand Ground pressed, then AutoExplore, Stop, Delete.
// Clicking a stance face applies it in the sim (the pressed
// face follows the sim's per-entity stance tags). R arms
// attack-move targeting; the panel button arms patrol; Esc and
// a re-click disarm. The sim-level behaviour (acquire / chase /
// leash matrix, patrol loops, queue interplay, determinism) is
// pinned in sim/tests/orders_stances.rs.
//
// Run:  node web-preview/play/orders.smoke.mjs
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

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
          // makeRedSheet (enemy palette swap) reads real pixels
          // once sheet images report loaded.
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
    this.lastViewport = null; // minimap strokeRect (the viewport) lands here
  }
  // Like the real DOM: assigning innerHTML replaces the children.
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
  // Models a fast successful load ('load' fires on a later tick):
  // the match-start gate awaits the building art settling, and a
  // never-settling stub would hold every start for the 8s cap.
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

// Global listener capture: app.js registers its keydown / pointer
// handlers on the window — here they land in maps the test fires.
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
  const fetch = async (u) => {
    const s = String(u);
    tab.fetchCalls.push(s);
    if (s.endsWith('.wasm')) {
      return new Response(new Uint8Array(WASM), {
        headers: { 'Content-Type': 'application/wasm' },
      });
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
      userAgent: 'hotkeys-smoke',
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
const tab = makeTab('https://skirmish.online/play/?mode=sandbox');
installGlobals(tab);
// The real page carries style="display:none" on its overlays;
// the stub starts with empty styles, so mirror the HTML once.
for (const id of ['pause-menu', 'help-overlay', 'end-overlay',
  'online-overlay', 'desync-overlay', 'peer-overlay', 'loading-overlay']) {
  tab.el(id).style.display = 'none';
}
await import('../app.js?hotkeys=1');
await sleep(400);
assert.equal(tab.el('online-overlay').style.display, 'flex', 'lobby up at boot');

tab.el('btn-solo').click();
let began = false;
for (let i = 0; i < 40 && !began; i += 1) {
  await sleep(250);
  began = tab.document.body.classList.contains('in-match');
}
assert.ok(began, 'sandbox match begins after the real load');

let nowMs = 1000;
function pump(n = 1, step = 50) {
  for (let i = 0; i < n; i += 1) {
    nowMs += step;
    const cb = rafCb;
    rafCb = null;
    assert.ok(cb, 'the frame loop stays registered');
    cb(nowMs);
  }
}
function key(code, mods = {}) {
  const ev = {
    code, key: '', target: tab.document.body, repeat: false,
    ctrlKey: !!mods.ctrl, shiftKey: !!mods.shift, metaKey: false, altKey: false,
    defaultPrevented: false,
    preventDefault() { this.defaultPrevented = true; },
  };
  for (const fn of winListeners.keydown || []) fn(ev);
  return ev;
}
const cmdButtons = () => tab.el('cmd-grid').children.filter((c) => c.tagName === 'BUTTON');
const cmdIds = () => cmdButtons().map((b) => b.id);
const cmdBtn = (id) => cmdButtons().find((b) => b.id === id);

const MIL_IDS = ['attack-move', 'patrol', 'stance-0', 'stance-1',
  'stance-2', 'stance-3', 'auto-explore', 'stop', 'delete'];

pump(3);

// ------------------------------------------------------------- (1)
// Nothing selected: no military panel at all.
assert.deepEqual(cmdIds(), [], 'no selection: the command panel is empty');
console.log('PASS (1) no selection: panel empty');

// ------------------------------------------------------------- (2)
// ',' selects the starting scout: the full military panel, in
// DE form — faces only, the key letter the only text on a face.
key('Comma');
pump(2);
assert.deepEqual(cmdIds(), MIL_IDS,
  'starting scout selected: Attack Move, Patrol, the stance row, AutoExplore, Stop, Delete');
const am = () => cmdBtn('attack-move');
const pat = () => cmdBtn('patrol');
const stance = (n) => cmdBtn(`stance-${n}`);
assert.ok(am().innerHTML.includes('<span class="cmd-key">R</span>'),
  'the Attack Move button is badged R (DE\'s dedicated key)');
assert.ok(pat().innerHTML.includes('<span class="cmd-key">W</span>'),
  'the Patrol button is badged with its grid letter W');
assert.ok(stance(0).innerHTML.includes('<span class="cmd-key">E</span>'),
  'the Aggressive face is badged with its grid letter E');
// The starting scout stands at Stand Ground (CMB-09): its face
// — and only its face — is pressed.
assert.ok(stance(2).className.includes('active'), 'Stand Ground starts pressed');
for (const n of [0, 1, 3]) {
  assert.ok(!stance(n).className.includes('active'), `stance-${n} starts unpressed`);
}
console.log('PASS (2) military panel in DE form; Stand Ground pressed for the starting scout');

// ------------------------------------------------------------- (3)
// Clicking a stance face applies it in the sim: the pressed
// face follows the sim's stance tags after the order lands.
const grid = tab.el('cmd-grid');
const clickBtn = (id) => {
  const b = cmdBtn(id);
  assert.ok(b, `${id} is rendered`);
  b.closest = () => b;
  grid.dispatch('click', { target: b });
  pump(4);
};
clickBtn('stance-0');
assert.ok(stance(0).className.includes('active'), 'Aggressive pressed after the click');
assert.ok(!stance(2).className.includes('active'), 'Stand Ground released after the click');
clickBtn('stance-1');
assert.ok(stance(1).className.includes('active'), 'Defensive pressed after the click');
console.log('PASS (3) stance faces apply in the sim and the panel follows');

// ------------------------------------------------------------- (4)
// R arms attack-move targeting (the button shows the armed
// state); Esc disarms. The panel button arms patrol the same
// way, and re-clicking it disarms.
key('KeyR');
pump(2);
assert.ok(am().className.includes('active'), 'R armed attack-move targeting');
key('Escape');
pump(2);
assert.ok(!am().className.includes('active'), 'Esc disarmed attack-move targeting');
clickBtn('patrol');
assert.ok(pat().className.includes('active'), 'the Patrol button armed patrol targeting');
clickBtn('patrol');
assert.ok(!pat().className.includes('active'), 're-clicking Patrol disarmed it');
console.log('PASS (4) R / button arming and disarming');
