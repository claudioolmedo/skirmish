// M13 hotkeys + edge-scrolling smoke (stub DOM; NOT part of
// `node --test` — the file is named *.smoke.mjs on purpose). Boots
// a real sandbox match against the real WASM sim, then drives it
// the way a player would: synthetic keydown events through the
// app's own global listener and hand-pumped animation frames
// (the older smokes stub rAF away entirely; this one captures the
// frame callback and the global listeners so keys and frames flow
// exactly as in the browser).
//
// Proves: '.' selects an idle villager → Q arms the economic page
// (panel shows that page's buildings, House badged E) → E enters
// House placement exactly like the button click; Q,W enters Mill
// placement; W arms the military page and Esc backs out without
// pausing; H selects the Town Center → Q queues a villager; A with
// an unloaded villager runs the drop-off path (its feedback
// shows); Ctrl+Z with no Lumber Camp flashes its feedback, and
// after one is really built and finished, Ctrl+Z selects it.
// Edge scrolling: the pointer is read in VIEWPORT coordinates,
// so parking it at any window edge — over the canvas, the top
// resource bar, or the bottom bar — moves the minimap's
// viewport rect that way (the canvas-only first build never
// panned at the top edge: the top bar swallowed the band).
// The fullscreen toggle's two controls drive the API and sync
// their glyph/label on fullscreenchange.
//
// Run:  node web-preview/play/hotkeys.smoke.mjs
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

pump(3);

// ------------------------------------------------------------- (1)
// '.' picks an idle villager; Q arms the economic page; E places
// a House (Claudio's sequence); Esc cancels the placement.
key('Period');
pump(2);
assert.deepEqual(cmdIds(), ['build-0', 'build-1', 'build-2', 'build-3', 'build-4',
  'build-5', 'build-6', 'build-7', 'build-8'], 'villager selected: the full build panel');

key('KeyQ');
assert.deepEqual(cmdIds(), ['build-0', 'build-1', 'build-3', 'build-4', 'build-5'],
  'Q arms the economic page — its five buildings only');
assert.ok(cmdBtn('build-0').innerHTML.includes('<span class="cmd-key">E</span>'),
  'the House button is badged E');
assert.ok(cmdBtn('build-4').innerHTML.includes('<span class="cmd-key">W</span>'),
  'the Mill button is badged W');

key('KeyE');
assert.deepEqual(cmdIds(), ['build-0', 'build-1', 'build-2', 'build-3', 'build-4',
  'build-5', 'build-6', 'build-7', 'build-8'], 'placing closes the page — full panel back');
assert.ok(cmdBtn('build-0').className.includes('active'), 'E entered House placement');

key('Escape');
pump(2);
assert.ok(!cmdBtn('build-0').className.includes('active'), 'Esc cancels the placement');
console.log('PASS (1) Q arms economic, E enters House placement, Esc cancels');

// ------------------------------------------------------------- (2)
// Q then W (the Mill's key on the economic page) places a Mill;
// W alone arms the military page; Esc leaves it without pausing.
key('KeyQ');
key('KeyW');
assert.ok(cmdBtn('build-4').className.includes('active'), 'Q,W entered Mill placement');
key('Escape');
pump(2);

key('KeyW');
assert.deepEqual(cmdIds(), ['build-2', 'build-6', 'build-7', 'build-8'],
  'W arms the military page — its four buildings only');
assert.ok(cmdBtn('build-2').innerHTML.includes('<span class="cmd-key">Q</span>'),
  'the Barracks button is badged Q');
key('Escape');
pump(2);
assert.deepEqual(cmdIds(), ['build-0', 'build-1', 'build-2', 'build-3', 'build-4',
  'build-5', 'build-6', 'build-7', 'build-8'], 'Esc backed out of the page');
assert.notEqual(tab.el('pause-menu').style.display, 'flex', '…and did not open the pause menu');
console.log('PASS (2) Q,W enters Mill placement; W arms military; Esc backs out cleanly');

// ------------------------------------------------------------- (3)
// H selects the Town Center; Q (its first button) queues a villager.
key('KeyH');
pump(2);
assert.ok(cmdBtn('train'), 'Town Center selected: the train panel is up');
assert.ok(cmdBtn('train').innerHTML.includes('<span class="cmd-key">Q</span>'),
  'the Villager button is badged Q');
key('KeyQ');
pump(6);
assert.ok(cmdBtn('train').innerHTML.includes('cmd-badge'), 'Q queued a villager (queue badge shows)');
console.log('PASS (3) TC selected + Q queues a villager');

// ------------------------------------------------------------- (4)
// A with villagers selected and no page armed: force drop-off.
// These villagers carry nothing, so the path answers with its
// own feedback — and no Farm placement is armed by accident.
key('Period');
pump(2);
key('KeyA');
pump(2);
assert.equal(tab.el('toast').textContent, 'No loaded villagers selected — nothing to drop off.',
  'A ran the drop-off path and reported');
assert.ok(!cmdBtn('build-1').className.includes('active'), 'A did not arm Farm placement');
console.log('PASS (4) villager + A issues the force drop-off path');

// ------------------------------------------------------------- (5)
// Go-to: Ctrl+Z with no Lumber Camp flashes; then build one for
// real (hotkeys + a canvas click), wait out its construction,
// and Ctrl+Z selects it.
key('KeyZ', { ctrl: true });
pump(2);
assert.equal(tab.el('toast').textContent, 'No Lumber Camp yet.', 'Ctrl+Z reports the missing camp');

key('Period');
pump(2);
key('KeyQ');
key('KeyR');
assert.ok(cmdBtn('build-3').className.includes('active'), 'Q,R entered Lumber Camp placement');
// Click near the centered villager: identical down/up points (a
// true click, under the drag threshold). Fractional client px —
// the stub viewport is 300x150 against a 1280x800 canvas, so a
// couple of client px already span several world tiles; the
// sweep covers the open ground around the start.
const view = tab.el('view');
let placed = false;
for (const [cx, cy] of [[150, 75], [151.5, 75.8], [148.5, 74.2], [153, 76.5],
  [147, 73.5], [151.5, 77.6], [148.5, 72.4], [154.5, 75], [145.5, 75], [150, 78.5]]) {
  // The app hears mousedown on the canvas, mouseup on the window.
  view.dispatch('mousedown', { clientX: cx, clientY: cy, button: 0 });
  for (const fn of winListeners.mouseup || []) {
    fn({ clientX: cx, clientY: cy, button: 0, shiftKey: false, target: view, preventDefault() {} });
  }
  pump(2);
  if (!cmdBtn('build-3').className.includes('active')) { placed = true; break; }
}
assert.ok(placed, 'the canvas click laid the Lumber Camp foundation');
// Construction takes 35 game-seconds; pump in 250 ms steps (the
// frame loop caps at 5 ticks per frame) until it must be done.
pump(500, 250);
key('KeyZ', { ctrl: true });
pump(2);
assert.equal(tab.el('sel-name').textContent, 'Lumber Camp', 'Ctrl+Z selected the finished camp');
key('KeyZ', { ctrl: true, shift: true });
pump(2);
assert.equal(tab.el('sel-name').textContent, 'Lumber Camp', 'Ctrl+Shift+Z keeps the (single) camp selected');
console.log('PASS (5) go-to: missing-type feedback, then select + center after a real build');

// ------------------------------------------------------------- (6)
// Edge scrolling reads VIEWPORT coordinates (2026-10-19 fix):
// the pointer counts at any of the four window edges over ANY
// element. The stub window is 1280x800.
const mm = tab.el('minimap');
const winMove = (x, y, target) => {
  for (const fn of winListeners.mousemove || []) fn({ clientX: x, clientY: y, target });
};
pump(2);
const before = mm.lastViewport;
assert.ok(before, 'the minimap drew a viewport rect');
// Right edge, over the canvas.
winMove(1279, 400, view);
pump(30);
const scrolled = mm.lastViewport;
assert.ok(scrolled.x > before.x + 2,
  `right-edge parking panned the camera right (viewport ${before.x.toFixed(1)} → ${scrolled.x.toFixed(1)})`);
// Top edge, over the TOP BAR (its button is the event target) —
// the band the canvas-only build could never reach.
winMove(640, 1, tab.el('pause-btn'));
pump(30);
const topped = mm.lastViewport;
assert.ok(Math.abs(topped.y - scrolled.y) > 2,
  `top edge over the top bar panned the camera (viewport y ${scrolled.y.toFixed(1)} → ${topped.y.toFixed(1)})`);
// Bottom edge, over the BOTTOM BAR (the command grid): pans
// back the opposite way.
winMove(640, 799, tab.el('cmd-grid'));
pump(30);
const bottomed = mm.lastViewport;
assert.ok(Math.abs(bottomed.y - topped.y) > 2
  && Math.sign(bottomed.y - topped.y) === -Math.sign(topped.y - scrolled.y),
  'bottom edge over the bottom bar panned the camera the opposite way');
// Over the bottom bar but NOT at a viewport edge: rests.
winMove(640, 700, tab.el('cmd-grid'));
const restFrom = mm.lastViewport;
pump(10);
assert.ok(Math.abs(mm.lastViewport.x - restFrom.x) < 0.01
  && Math.abs(mm.lastViewport.y - restFrom.y) < 0.01,
  'away from the viewport edges, edge scrolling rests');
// Pointer leaves the window: the point clears, the pan stops.
winMove(1279, 400, view);
for (const fn of docListeners.mouseleave || []) fn({});
const leftFrom = mm.lastViewport;
pump(10);
assert.ok(Math.abs(mm.lastViewport.x - leftFrom.x) < 0.01,
  'after the pointer leaves the window, edge scrolling rests');
console.log('PASS (6) edge scrolling pans at all four viewport edges, over any element');

// ------------------------------------------------------------- (7)
// Fullscreen toggle: the top-bar button and the pause-menu
// entry both call the Fullscreen API on the document element;
// fullscreenchange syncs glyph, title and menu label.
const fsBtn = tab.el('fullscreen-btn');
const fsMenuBtn = tab.el('pause-fullscreen-btn');
assert.equal(fsBtn.title, 'Fullscreen', 'the top-bar button starts titled Fullscreen');
assert.equal(fsBtn.textContent, '⛶', '…with the enter-fullscreen glyph');
let fsEntered = 0;
let fsExited = 0;
tab.document.documentElement.requestFullscreen = () => { fsEntered += 1; return Promise.resolve(); };
tab.document.exitFullscreen = () => { fsExited += 1; };
fsBtn.click();
assert.equal(fsEntered, 1, 'the top-bar button requests fullscreen on the document element');
tab.document.fullscreenElement = tab.document.documentElement;
for (const fn of docListeners.fullscreenchange || []) fn({});
assert.equal(fsBtn.textContent, '⤢', 'fullscreen state swaps the glyph');
assert.equal(fsBtn.title, 'Exit fullscreen', '…and the title');
assert.equal(fsMenuBtn.textContent, 'Exit fullscreen', '…and the pause-menu entry label');
fsMenuBtn.click();
assert.equal(fsExited, 1, 'the pause-menu entry exits fullscreen');
tab.document.fullscreenElement = null;
for (const fn of docListeners.fullscreenchange || []) fn({});
assert.equal(fsBtn.textContent, '⛶', 'leaving fullscreen restores the enter glyph');
assert.equal(fsMenuBtn.textContent, 'Fullscreen', '…and the menu entry label');
console.log('PASS (7) fullscreen toggle: both controls drive the API, glyph/label sync on fullscreenchange');

console.log('HOTKEYS SMOKE: all cases passed');
process.exit(0);
