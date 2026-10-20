// End-of-match feedback smoke (stub DOM; NOT part of `node
// --test` — named *.smoke.mjs on purpose). Boots a real AI Normal
// match (default seed 7) against the real WASM sim and plays it
// the slow honest way — the player does nothing, the AI wins —
// until the end overlay rises, then drives the feedback block
// the way a player would.
//
//   Act A (dismiss): the block renders on the end card; the ✕
//   hides it; after Restart and a second defeat it is STILL
//   hidden (session-only hide), and nothing was ever posted.
//   Act B (send): a failed send keeps the typed text and shows
//   the error; a good send posts the exact payload to the
//   stubbed /feedback, sets localStorage skirmish2.feedbackDone,
//   and shows the thanks state; after Restart and a second
//   defeat the form is gone and only the GitHub star line shows.
//
// The acts run as separate processes (this file re-execs itself)
// so each gets a fresh app instance and fresh storage.
//
// Run:  node web-preview/play/feedback.smoke.mjs
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { spawnSync } from 'node:child_process';

const HERE = dirname(fileURLToPath(import.meta.url));
const ACT = process.argv[2];

if (!ACT) {
  for (const act of ['a', 'b']) {
    const r = spawnSync(process.execPath, [fileURLToPath(import.meta.url), act], {
      stdio: 'inherit',
    });
    if (r.status !== 0) process.exit(r.status ?? 1);
  }
  console.log('FEEDBACK SMOKE: all acts passed');
  process.exit(0);
}

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
  'online-overlay', 'desync-overlay', 'peer-overlay', 'loading-overlay']) {
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

const feedbackPosts = () => tab.fetchCalls.filter((c) => c.url.endsWith('/feedback'));

if (ACT === 'a') {
  // ------------------------------------------------------ dismiss
  playToEnd('act A match 1');
  const block = tab.el('end-feedback');
  assert.notEqual(block.style.display, 'none', 'the feedback block renders on the end card');
  assert.notEqual(tab.el('feedback-form').style.display, 'none', 'the form shows');
  assert.equal(tab.el('end-title').textContent, 'Defeat', 'an idle player loses');

  tab.el('feedback-close').click();
  assert.equal(block.style.display, 'none', 'the ✕ hides the block');

  tab.el('end-restart-btn').click();
  pump(3);
  assert.equal(tab.el('end-overlay').style.display, 'none', 'restart hides the end card');
  playToEnd('act A match 2');
  assert.equal(tab.el('end-overlay').style.display, 'flex', 'the second match ends too');
  assert.equal(block.style.display, 'none', 'the block stays hidden for the session');
  assert.equal(feedbackPosts().length, 0, 'nothing was ever posted');
  assert.equal(tab.local.getItem('skirmish2.feedbackDone'), null, 'dismiss does not set feedbackDone');
  console.log('PASS (A) dismiss: block renders, ✕ hides it for the whole session');
} else {
  // --------------------------------------------------------- send
  playToEnd('act B match 1');
  assert.notEqual(tab.el('feedback-form').style.display, 'none', 'the form shows on the end card');

  const text = tab.el('feedback-text');
  text.value = 'The boar ate my scout. 10/10 would lose again.';
  tab.el('feedback-star-4').click();
  assert.equal(tab.el('feedback-star-1').textContent, '★');
  assert.equal(tab.el('feedback-star-4').textContent, '★', 'stars paint up to the click');
  assert.equal(tab.el('feedback-star-5').textContent, '☆', 'the fifth star stays empty');

  // A failed send: quiet error, text kept, nothing stored.
  tab.feedbackFail = true;
  tab.el('feedback-send').click();
  await sleep(50);
  assert.equal(tab.el('feedback-error').hidden, false, 'the failure shows the error line');
  assert.equal(text.value, 'The boar ate my scout. 10/10 would lose again.', 'the text survives a failed send');
  assert.equal(text.hidden, false, 'the form stays put after a failure');
  assert.equal(tab.local.getItem('skirmish2.feedbackDone'), null, 'a failed send sets no flag');

  // A good send: exact payload, thanks state, flag set.
  tab.feedbackFail = false;
  tab.el('feedback-send').click();
  await sleep(50);
  const posts = feedbackPosts();
  assert.equal(posts.length, 2, 'one failed attempt + one good send');
  const good = posts[posts.length - 1];
  assert.equal(good.method, 'POST');
  const payload = JSON.parse(good.body);
  assert.equal(payload.stars, 4);
  assert.equal(payload.text, 'The boar ate my scout. 10/10 would lose again.');
  assert.equal(payload.result, 'defeat');
  assert.ok(Number.isInteger(payload.durationTicks) && payload.durationTicks > 100000,
    `durationTicks is the real match length (${payload.durationTicks})`);
  assert.equal(payload.buildVersion, 'm8');
  assert.equal(tab.el('feedback-thanks').hidden, false, 'the thanks state shows');
  assert.equal(text.hidden, true, 'the form inputs retire after sending');
  assert.equal(tab.local.getItem('skirmish2.feedbackDone'), '1', 'feedbackDone is set');

  // Next end card: no form, only the GitHub star line.
  tab.el('end-restart-btn').click();
  pump(3);
  playToEnd('act B match 2');
  assert.equal(tab.el('feedback-form').style.display, 'none', 'the form never shows again');
  assert.notEqual(tab.el('end-feedback').style.display, 'none', 'the block itself still renders');
  assert.notEqual(tab.el('feedback-gh-line').style.display, 'none', 'the GitHub star line remains');
  console.log('PASS (B) send: failure keeps text, success posts the exact payload and retires the form');
}
