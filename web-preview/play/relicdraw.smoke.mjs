// Relic-draw smoke (stub DOM; NOT part of `node --test` —
// named *.smoke.mjs on purpose). Boots a real sandbox match
// against the real WASM sim and RECORDS every drawImage on the
// main canvas, so the proof is about what was actually drawn.
//
// Bot-loop fix (2026-11-03): two long live sessions swept
// explored ground and never saw a relic, because the draw loop
// routed the ground relic through the generic UNIT branch —
// VIS-06's visible-only gate — while resource nodes and
// buildings draw on explored ground. A ground relic is a
// static Gaia object: (1) it must draw while its cell is
// currently visible; (2) it must STAY drawn (fog-dimmed) once
// the scout leaves and the cell is merely explored. Phase (2)
// fails on the pre-fix client.
//
// Run:  node web-preview/play/relicdraw.smoke.mjs
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const HERE = dirname(fileURLToPath(import.meta.url));
const WASM = readFileSync(join(HERE, '..', 'pkg', 'skirmish_sim_bg.wasm'));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
// ------------------------------------------------------------- DOM stub
// (harness: auto-explore.smoke.mjs verbatim, plus drawImage
// recording on the view canvas and real PNG sizes on FakeImage)
const drawLog = []; // {base, sx, sy, rows} per villager drawImage
let viewCtxProxy;
function viewCtx() {
  if (!viewCtxProxy) {
    viewCtxProxy = new Proxy({}, {
      get: (t, k) => {
        if (k === 'canvas') return { width: 1280, height: 800 };
        if (k === 'measureText') return () => ({ width: 10 });
        if (k === 'getImageData') {
          return (x, y, w, h) => ({ data: new Uint8ClampedArray(Math.max(4, w * h * 4)) });
        }
        if (k === 'createLinearGradient' || k === 'createRadialGradient') {
          return () => ({ addColorStop() {} });
        }
        if (k === 'drawImage') {
          return (img, ...rest) => {
            const src = img && img._src ? String(img._src) : '';
            const base = src.split('/').pop().split('?')[0];
            drawLog.push({ base, w: img && img.width, h: img && img.height, nw: img && img.naturalWidth, nh: img && img.naturalHeight, rest: rest.slice(0,8) });
          };
        }
        if (typeof k === 'string') return () => {};
        return undefined;
      },
      set: () => true,
    });
  }
  return viewCtxProxy;
}
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
  getBoundingClientRect() {
    // The view canvas is fullscreen at dpr 1 in this harness:
    // client px == backing px, so click math is the identity.
    if (this.id === 'view') return { left: 0, top: 0, width: 1280, height: 800 };
    return { left: 0, top: 0, width: 300, height: 150 };
  }
  getContext() {
    if (this.id === 'view') return viewCtx();
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

function pngDims(absPath) {
  try {
    const b = readFileSync(absPath);
    if (b.toString('ascii', 1, 4) !== 'PNG') return null;
    return [b.readUInt32BE(16), b.readUInt32BE(20)];
  } catch { return null; }
}

class FakeImage {
  // Real sizes from the real files, so "sheet loaded" means what
  // it means in the browser (and the tier loader picks real art).
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
    const s = String(v);
    const idx = s.indexOf('assets-original');
    const dims = idx >= 0 ? pngDims(join(HERE, '..', s.slice(idx).split('?')[0])) : null;
    if (dims) { this.naturalWidth = dims[0]; this.naturalHeight = dims[1]; }
    setTimeout(() => {
      this.complete = true;
      if (!dims) { this.naturalWidth = 64; this.naturalHeight = 64; }
      for (const fn of this._listeners.load || []) fn({});
      if (this.onload) this.onload({});
    }, 0);
  }
  get src() { return this._src; }
}

// Global listener capture (verbatim pattern).
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
      userAgent: 'carryprops-smoke',
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

// ------------------------------------------------------------- mirror
const pkg = await import('../pkg/skirmish_sim.js');
await pkg.default(new Response(new Uint8Array(WASM), {
  headers: { 'Content-Type': 'application/wasm' },
}));
const mirror = new pkg.WasmSim(7);
const mKinds = mirror.entity_kinds();
const mPos = mirror.positions();
const mOwners = mirror.entity_owners();
let scout=null; const relics=[];
for (let i=0;i<mKinds.length;i++){
  if (mKinds[i]===2 && mOwners[i]===0) scout={i,x:mPos[i*2],y:mPos[i*2+1]};
  if (mKinds[i]===21) relics.push({i,x:mPos[i*2],y:mPos[i*2+1]});
}
const central = relics.reduce((a,b)=> (Math.abs(a.x)+Math.abs(a.y) < Math.abs(b.x)+Math.abs(b.y) ? a:b));
console.log('scout', JSON.stringify(scout), 'relics', JSON.stringify(relics), 'central', JSON.stringify(central));

// ------------------------------------------------------------- driving
const tab = makeTab('https://skirmish.online/play/?mode=sandbox&seed=7');
installGlobals(tab);
for (const id of ['pause-menu', 'help-overlay', 'end-overlay',
  'online-overlay', 'desync-overlay', 'peer-overlay', 'loading-overlay',
  'score-overlay', 'techtree-overlay']) {
  tab.el(id).style.display = 'none';
}
await import('../app.js?relicdraw=1');
await sleep(400);
tab.el('btn-solo').click();
let began = false;
for (let i = 0; i < 40 && !began; i += 1) {
  await sleep(250);
  began = tab.document.body.classList.contains('in-match');
}
assert.ok(began, 'sandbox match begins');
let nowMs = 1000;
function pump(n=1, step=50){
  for(let i=0;i<n;i++){ nowMs+=step; const cb=rafCb; rafCb=null; assert.ok(cb,'frame loop'); cb(nowMs); }
}
function key(code, mods = {}) {
  const ev = { code, key: '', target: tab.document.body, repeat: false,
    ctrlKey: !!mods.ctrl, shiftKey: !!mods.shift, metaKey: false, altKey: false,
    defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
  for (const fn of winListeners.keydown || []) fn(ev);
  return ev;
}
const S = 5;
function clickWorldAt(center, wx, wy, button=2) {
  const ox = 640 - (center.x - center.y) * S;
  const oy = 400 - ((center.x + center.y) / 2) * S;
  const cx = Math.round(ox + (wx - wy) * S);
  const cy = Math.round(oy + ((wx + wy) / 2) * S);
  const view = tab.el('view');
  const ev = { button, clientX: cx, clientY: cy, shiftKey: false, preventDefault() {}, stopPropagation() {} };
  view.dispatch('mousedown', ev);
  for (const fn of winListeners.mousedown || []) fn({ target: view, ...ev });
  view.dispatch('mouseup', ev);
  for (const fn of winListeners.mouseup || []) fn({ target: view, ...ev });
  return {cx,cy};
}
const isRelicSprite = (d) => d.w===23 && d.h===36; // RELIC_BOX 1.4x2.2 @16px/u canvas
function census(label){
  const counts=new Map();
  for(const d of drawLog){ const k=d.base? d.base : `canvas:${d.w}x${d.h}`; counts.set(k,(counts.get(k)||0)+1); }
  const relicCalls=drawLog.filter(isRelicSprite);
  console.log(label, 'total drawImage', drawLog.length, 'relicSpriteCalls', relicCalls.length, JSON.stringify(relicCalls.slice(0,3)));
  return relicCalls.length;
}
pump(20);
drawLog.length=0; pump(5); census('at start (relic far, unexplored):');
// Select idle military (the scout) and right-click the central relic position
key('Comma'); pump(2);
console.log('clicked to relic at', JSON.stringify(clickWorldAt(scout, central.x, central.y)));
let found=0; let frames=0;
for(let chunk=0; chunk<400 && !found; chunk++){
  drawLog.length=0; pump(50); frames+=50;
  found = census(`after ${frames} frames:`);
  if(found) break;
}
assert.ok(found, 'the relic draws while its cell is currently visible');
console.log(`PASS (1) visible: relic sprite drawn after ${frames} frames`);
// (2) Send the scout back to its start. Once it is far away the
// relic's cell is EXPLORED but no longer visible — a static Gaia
// object must STILL draw there (dimmed by the fog pass), the way
// resource nodes and buildings do. On the pre-fix client the
// generic unit gate (visible-only) made it vanish here.
console.log('return click', JSON.stringify(clickWorldAt(scout, scout.x, scout.y)));
pump(900);
drawLog.length=0; pump(5);
const exploredCalls = census('after the scout left (explored, not visible):');
assert.ok(exploredCalls > 0, 'the ground relic still draws on explored ground after the scout leaves');
console.log('PASS (2) explored: the ground relic remains drawn from explored memory');
console.log('RELIC-DRAW SMOKE: ALL PASS');
