// Regression tests for the both-ready deadlock (Claudio, live
// report 2026-11-26, two tabs on the g= short link): BOTH room
// screens showed both players Ready ✓ — yet the match never
// started. A slower manual run of the same flow DID start, which
// pointed at an ordering race, not a broken gate.
//
// Root cause, traced in app.js: the both-ready gate
// (maybeStartMatch) was evaluated at exactly TWO moments — the
// player's own ready toggle, and the arrival of the peer's
// 'ready' frame — while its inputs settle at several more sites,
// and one input could be silently LOST:
//
//   1. The 'ready' case recorded the peer's flag ONLY while
//      net.peerPresent was already true. A ready that arrived
//      while presence had not settled yet (the peer's hello
//      still in flight / its first hello lost before we joined —
//      the guest-first room the short link creates) was dropped
//      on the floor — and a peer sends 'ready' only when it
//      toggles, so the flag never came again.
//   2. When presence then settled, the hello case RESET
//      net.peerReady to false ("a fresh arrival starts
//      unready") — erasing even a recorded early ready — and
//      never re-evaluated the gate.
//
// Departures are not the hole: onPeerGone (the relay's
// peer-left) already clears both ready flags when a peer
// actually leaves, so an early-recorded ready cannot be a
// previous seat-holder's leftover — it can only be the
// arriving peer's own flag, sent a moment before its hello
// finished settling presence here.
//
// The fix, in app.js:
//   - the 'ready' case records the flag in the lobby phase
//     regardless of presence (presence stays a GATE input via
//     startGate/canToggleReady — a ready from an absent peer
//     still starts nothing, it is just no longer lost);
//   - the hello case no longer resets peerReady on new
//     presence, and — like the 'pw' / 'pw-ok' admission sites —
//     re-evaluates maybeStartMatch() once presence/admission
//     has settled;
//   - a client that is itself ready re-announces its 'ready'
//     when a peer hello settles presence in the lobby, so the
//     peer's record of MY flag reconverges after any flap on
//     ITS side (the mirror of hole 1);
//   - toggleReady refreshes the status line, so a player who
//     has readied no longer reads "Press Start when you are
//     ready" while waiting (the screenshot's stale line).
//
// These tests drive the REAL ../app.js in Node against a stub
// DOM and a stub WebSocket (the prematch.smoke harness pattern):
// one real client per case, the peer played by injecting frames
// into its room socket in a chosen order. Run from
// web-preview/play/: node --test readylock.test.mjs

import { test, after } from 'node:test';

// The booted app.js keeps lobby/heartbeat intervals alive, so
// the test process would never drain its event loop on its own
// (the reason the *.smoke.mjs harnesses end in process.exit).
// Count failures ourselves and exit with the honest code once
// the reporter has had a moment to flush.
let failures = 0;
const check = (name, fn) => test(name, async () => {
  try { await fn(); } catch (e) { failures += 1; throw e; }
});
after(() => { setTimeout(() => process.exit(failures ? 1 : 0), 300); });
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const WASM_BYTES = readFileSync(new URL('../pkg/skirmish_sim_bg.wasm', import.meta.url));

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
    createElementNS: (_ns, tag) => new El(tag),
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
      userAgent: 'readylock-test',
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
  await import(`../app.js?readylock=${bootCount}`);
  await sleep(400);
}

const roomSocket = () =>
  FakeWS.instances.find((w) => w.url.includes('/room/') && !w.url.includes('probe-'));

const sentFrames = (sock) => sock.sent
  .map((d) => { try { return JSON.parse(d); } catch { return null; } })
  .filter(Boolean);

async function waitForStart(sock, ms = 5000) {
  const t0 = Date.now();
  while (Date.now() - t0 < ms) {
    if (sentFrames(sock).some((f) => f.type === 'start')) return true;
    await sleep(100);
  }
  return false;
}

// The room code the homepage minted in Claudio's report (w =
// wnam in char 1, the rest from the link alphabet).
const CODE = 'wbdtwx2';

// ------------------------------------------------------------------ cases

check('control: hello, peer ready, own ready — the host starts', async () => {
  const tab = makeTab(`https://skirmish.online/play/?dono=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the claim boot created the room and connected');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'hello', v: 1, role: 'guest', name: 'Guest' });
  await sleep(60);
  peerSay({ type: 'ready', ready: true });
  await sleep(60);
  tab.el('btn-ready').click();
  assert.ok(await waitForStart(sock),
    'with presence settled before the ready, the both-ready gate fires and the host sends start');
});

check('ready BEFORE presence settles is not lost: the host still starts', async () => {
  const tab = makeTab(`https://skirmish.online/play/?dono=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the claim boot created the room and connected');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  // The bad order from the live report: the guest's ready beats
  // its hello here (its first hello was lost before the host
  // joined — the guest-first room — and its reply hello settles
  // presence only afterwards). The ready must be RECORDED, not
  // dropped for arriving while peerPresent was still false, and
  // the hello that settles presence must not erase it.
  peerSay({ type: 'ready', ready: true });
  await sleep(60);
  peerSay({ type: 'hello', v: 1, role: 'guest', name: 'Guest' });
  await sleep(60);
  tab.el('btn-ready').click();
  assert.ok(await waitForStart(sock),
    'both players are ready — the host must send start even though the peer ready arrived before presence settled');
});

check('a readied guest re-announces its ready on a later hello, and its status stops saying "Press Start"', async () => {
  const tab = makeTab(`https://skirmish.online/play/?room=${CODE}`);
  await boot(tab);
  const sock = roomSocket();
  assert.ok(sock, 'the link boot joined the room and connected');
  const peerSay = (obj) => sock._emit('message', { data: JSON.stringify(obj) });
  peerSay({ type: 'hello', v: 1, role: 'host', name: 'Host' });
  await sleep(60);
  tab.el('btn-ready').click();
  await sleep(60);
  const readies = () => sentFrames(sock).filter((f) => f.type === 'ready' && f.ready === true).length;
  assert.equal(readies(), 1, 'the toggle sent the ready once');
  // The screenshot's stale line: having readied, the guest must
  // no longer be told to press Start.
  const status = tab.el('lobby-status').textContent;
  assert.ok(!status.includes('Press Start when you are ready'),
    `a readied guest's status must not say "Press Start…", got: ${status}`);
  assert.ok(status.includes('You are ready'),
    `a readied guest's status says it is ready and waiting, got: ${status}`);
  // A later hello from the host (its client re-settled — the
  // mirror flap) must draw a re-announcement: the host's record
  // of this guest's ready is exactly what such a flap can erase.
  peerSay({ type: 'hello', v: 1, role: 'host', name: 'Host' });
  await sleep(60);
  assert.equal(readies(), 2,
    'a hello settling while this client is ready re-announces its ready');
});
