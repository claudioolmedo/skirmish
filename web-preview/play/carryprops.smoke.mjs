// Visible-carry smoke (stub DOM; NOT part of `node --test` —
// named *.smoke.mjs on purpose). Boots a real sandbox match
// against the real WASM sim and drives it the way a player
// would: the idle-villager key selects, a synthetic right-click
// orders the gather, hand-pumped animation frames advance the
// game — while every villager drawImage on the main canvas is
// RECORDED, so the proof is about what was actually drawn.
//
// Claudio's rule (2026-10-25): a villager hauling wood/gold/
// stone/food is drawn carrying THAT resource; walking empty —
// to the resource, and back right after the drop-off — it is
// drawn empty-handed. Phases: (1) nobody carries anything at
// the start; (2) order wood: walk (empty) → gather → the logs
// sheet appears; (3) the return leg after the deposit is drawn
// with the plain walk again; (4) a second villager repeats it
// for food (berries). Gold is proven end-to-end at the ABI
// level (entity_carry transitions) by the study script and
// pinned in carryprops.test.mjs; the strip shows all four.
//
// The scene geometry (TC, villagers, nodes) is read from a
// mirror sim built from the same seed — the client's sim is
// the same deterministic world, and idle villagers have not
// moved when each order is given.
//
// Run:  node web-preview/play/carryprops.smoke.mjs
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
          return (img, sx, sy, sw, sh) => {
            const src = img && img._src ? String(img._src) : '';
            const base = src.split('/').pop().split('?')[0];
            if (base.startsWith('villager-') && base.endsWith('.png')) {
              drawLog.push({ base, sx, sy, rows: Math.round((img.naturalHeight || 1024) / 128) });
            }
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
// The deterministic start state, read off a mirror sim (same
// seed, never ticked): TC, the villagers, the nearest nodes.
const pkg = await import('../pkg/skirmish_sim.js');
await pkg.default(new Response(new Uint8Array(WASM), {
  headers: { 'Content-Type': 'application/wasm' },
}));
const mirror = new pkg.WasmSim(7);
const mKinds = mirror.entity_kinds();
const mPos = mirror.positions();
const mNodes = mirror.nodes(); // flat [id, x, y, kind, amount]
const tcIdx = mKinds.indexOf(1);
const tc = { x: mPos[tcIdx * 2], y: mPos[tcIdx * 2 + 1] };
const villagers = [];
for (let i = 0; i < mKinds.length; i++) {
  if (mKinds[i] === 0) villagers.push({ i, x: mPos[i * 2], y: mPos[i * 2 + 1] });
}
const nodeByKind = (kind) => {
  let best = null;
  for (let j = 0; j + 4 < mNodes.length; j += 5) {
    const n = { id: mNodes[j], x: mNodes[j + 1], y: mNodes[j + 2], kind: mNodes[j + 3], amount: mNodes[j + 4] };
    if (n.kind !== kind || n.amount <= 0) continue;
    const d = Math.hypot(n.x - tc.x, n.y - tc.y);
    if (!best || d < best.d) best = { ...n, d };
  }
  return best;
};
const tree = nodeByKind(0);
const berry = nodeByKind(1);
const gold = nodeByKind(2);
console.log(`scene: TC (${tc.x}, ${tc.y}); villagers ${villagers.map((v) => v.i).join(',')}; ` +
  `tree #${tree.id} @${tree.d.toFixed(1)}u; berry #${berry.id} @${berry.d.toFixed(1)}u; gold #${gold.id} @${gold.d.toFixed(1)}u`);
assert.ok(villagers.length >= 3 && tree && berry, 'the sandbox start has villagers, a tree and berries');

// ------------------------------------------------------------- driving
const tab = makeTab('https://skirmish.online/play/?mode=sandbox&seed=7');
installGlobals(tab);
for (const id of ['pause-menu', 'help-overlay', 'end-overlay',
  'online-overlay', 'desync-overlay', 'peer-overlay', 'loading-overlay']) {
  tab.el(id).style.display = 'none';
}
await import('../app.js?carryprops=1');
await sleep(400);

tab.el('btn-solo').click();
let began = false;
for (let i = 0; i < 40 && !began; i += 1) {
  await sleep(250);
  began = tab.document.body.classList.contains('in-match');
}
assert.ok(began, 'sandbox match begins after the real load');

let nowMs = 1000;
let frameNo = 0;
const frameSheets = []; // per pumped frame: Map base -> first draw record
function pumpRec(n = 1, step = 50) {
  for (let i = 0; i < n; i += 1) {
    nowMs += step;
    const cb = rafCb;
    rafCb = null;
    assert.ok(cb, 'the frame loop stays registered');
    drawLog.length = 0;
    cb(nowMs);
    frameNo += 1;
    const m = new Map();
    for (const d of drawLog) if (!m.has(d.base)) m.set(d.base, d);
    frameSheets.push(m);
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
// centerCamera(wx, wy, s, w, h) = [w/2 - (wx-wy)*s, h/2 - ((wx+wy)/2)*s];
// the camera sits on the selected villager after the idle key.
// world->screen with that origin: sx = ox + (wx-wy)*s, sy = oy + ((wx+wy)/2)*s.
const S = 5; // INITIAL_S
function clickWorldAt(center, wx, wy) {
  const ox = 640 - (center.x - center.y) * S;
  const oy = 400 - ((center.x + center.y) / 2) * S;
  const cx = Math.round(ox + (wx - wy) * S);
  const cy = Math.round(oy + ((wx + wy) / 2) * S);
  const view = tab.el('view');
  const ev = { button: 2, clientX: cx, clientY: cy, shiftKey: false, preventDefault() {}, stopPropagation() {} };
  view.dispatch('mousedown', ev);
  for (const fn of winListeners.mousedown || []) fn({ target: view, ...ev });
  view.dispatch('mouseup', ev);
  for (const fn of winListeners.mouseup || []) fn({ target: view, ...ev });
}
function pumpUntil(pred, cap, what) {
  for (let i = 0; i < cap; i += 1) {
    pumpRec(10);
    if (pred()) return true;
  }
  const seen = new Set();
  for (const m of frameSheets) for (const k of m.keys()) seen.add(k);
  throw new Error(`timeout waiting for ${what}; sheets seen: ${[...seen].sort().join(', ')}`);
}
const firstDraw = (base) => {
  for (let f = 0; f < frameSheets.length; f++) {
    if (frameSheets[f].has(base)) return { frame: f, ...frameSheets[f].get(base) };
  }
  return null;
};
const firstDrawAfter = (base, afterFrame) => {
  for (let f = afterFrame + 1; f < frameSheets.length; f++) {
    if (frameSheets[f].has(base)) return { frame: f, ...frameSheets[f].get(base) };
  }
  return null;
};

// ------------------------------------------------------------- (1)
// At the start nobody carries anything: idle villagers, no
// cargo sheet anywhere.
pumpRec(20);
assert.ok(frameSheets.some((m) => m.has('villager-idle.png')), 'villagers render (originals path)');
assert.ok(!frameSheets.some((m) => [...m.keys()].some((b) => b.startsWith('villager-carry'))),
  'no carry prop before anyone gathers');
console.log('PASS (1) start: villagers empty-handed, no cargo sheet drawn');

// ------------------------------------------------------------- (2)
// Villager A to the nearest tree: plain walk on the way out,
// gather at the trunk, then the LOGS overhead.
key('Period'); // selects the first idle villager, camera centers on her
pumpRec(2);
clickWorldAt(villagers[0], tree.x, tree.y);
pumpUntil(() => firstDraw('villager-carry-wood.png'), 6000, 'the wood cargo sheet');
const woodFirst = firstDraw('villager-carry-wood.png');
const walkFirst = firstDraw('villager-walk.png');
const gatherFirst = firstDraw('villager-gather.png');
{ // first-appearance timeline (the drawn truth, for the log)
  const seen = new Map();
  frameSheets.forEach((m, f) => { for (const b of m.keys()) if (!seen.has(b)) seen.set(b, f); });
  console.log('sheet timeline:', [...seen.entries()].sort((a, b) => a[1] - b[1]).map(([b, f]) => `${b}@f${f}`).join(' '));
  if (process.env.CARRY_DEBUG) {
    const w = woodFirst.frame;
    for (let f = Math.max(0, w - 20); f < Math.min(frameSheets.length, w + 4); f++) {
      console.log(` f${f}:`, [...frameSheets[f].keys()].join(', '));
    }
  }
}
assert.ok(walkFirst && walkFirst.frame < woodFirst.frame, 'he walked (empty) before carrying');
// The sim's first chop fills the carry while the client still
// draws the arrival slide, so the first cargo frame can lead
// the first stationary gather frame by a few frames — what
// matters is that gathering is what produced the load.
assert.ok(gatherFirst && gatherFirst.frame <= woodFirst.frame + 6,
  'gathering at the tree is what filled the load');
assert.ok(!firstDraw('villager-carry.png'),
  'the generic bundle never stands in once the cargo sheet is live');
console.log(`PASS (2) wood: walk f${walkFirst.frame} -> gather f${gatherFirst.frame} -> carry-wood f${woodFirst.frame} (cell frame ${woodFirst.sx / 128}, dir ${woodFirst.sy / 128})`);

// ------------------------------------------------------------- (3)
// The deposit empties the hands: after the loaded leg, a frame
// shows the plain walk with neither the cargo nor gather sheet
// — the villager walking back empty (only A is moving).
pumpUntil(() => {
  for (let f = woodFirst.frame + 1; f < frameSheets.length; f++) {
    const m = frameSheets[f];
    if (m.has('villager-walk.png') && !m.has('villager-carry-wood.png') && !m.has('villager-gather.png')) return true;
  }
  return false;
}, 9000, 'the empty-handed return after the deposit');
const carryFrames = frameSheets.filter((m) => m.has('villager-carry-wood.png')).length;
assert.ok(carryFrames >= 5, `the loaded leg is a real walk, not a flicker (${carryFrames} frames)`);
console.log(`PASS (3) drop-off: the loaded leg home lasted ${carryFrames} drawn frames, then the same villager walks empty-handed again`);

// ------------------------------------------------------------- (4)
// Villager B to the berries: same story, the FOOD cargo.
key('Period'); // next idle villager (A is busy), camera centers on her
pumpRec(2);
const gatherBeforeFood = frameSheets.length;
clickWorldAt(villagers[1], berry.x, berry.y);
pumpUntil(() => firstDraw('villager-carry-food.png'), 6000, 'the food cargo sheet');
const foodFirst = firstDraw('villager-carry-food.png');
assert.ok(firstDrawAfter('villager-gather.png', gatherBeforeFood), 'the forager gathered at the bush first');
console.log(`PASS (4) food: carry-food first drawn at f${foodFirst.frame} (cell frame ${foodFirst.sx / 128}, dir ${foodFirst.sy / 128})`);

const summary = {
  seed: 7,
  tree: { id: tree.id, x: tree.x, y: tree.y },
  berry: { id: berry.id, x: berry.x, y: berry.y },
  gold: { id: gold.id, x: gold.x, y: gold.y },
  woodFirst,
  foodFirst,
};
writeFileSync(process.env.CARRY_SMOKE_OUT || '/tmp/carry-smoke-summary.json', JSON.stringify(summary, null, 2));
console.log('CARRY-PROPS SMOKE: ALL PASS');
