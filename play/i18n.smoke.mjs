// i18n smoke: boots the REAL ../app.js in Node against a stub DOM
// with the locale pre-seeded to pt-BR, asserts the lobby renders
// in Portuguese, then drives the language select to English and
// Spanish and asserts the CURRENT screen re-renders in place —
// no reload — and that the choice persists to localStorage.
// A smoke, not a unit test: not picked up by `node --test`.
// Run from web-preview/play/: node i18n.smoke.mjs

import assert from 'node:assert/strict';

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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

const DIRECTORY = {
  open: [
    { room: 'westopen01', region: 'wnam', players: 1, createdAt: 1000 },
  ],
  inProgress: 0,
};

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
    addEventListener() {},
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
    if (s.endsWith('/regions')) {
      return { ok: true, json: async () => ({ regions: [{ id: 'wnam', label: 'North America West' }] }) };
    }
    if (s.endsWith('/lobbies')) {
      return { ok: true, json: async () => DIRECTORY };
    }
    return { ok: true, json: async () => ({}) };
  };
  tab.location = location;
  tab.document = document;
  tab.history = history;
  tab.fetch = fetch;
  return tab;
}

const IMG_LOADS = [];
class FakeImage {
  constructor() { this.onload = null; this.onerror = null; }
  set src(v) { this._src = v; IMG_LOADS.push(v); }
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
      userAgent: 'i18n-smoke', language: 'en-US',
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

// ------------------------------------------------------------------ cases

const tab = makeTab('https://skirmish.online/play/');
tab.el('games-show-locked').checked = true; // the markup default
tab.local.setItem('skirmish2.locale', 'pt-BR'); // stored choice wins
installGlobals(tab);
await import('../app.js?i18nsmoke=1');
await sleep(400);

// (1) Boot in pt-BR: the lobby's chrome speaks Portuguese.
{
  assert.equal(tab.el('lobby-title-h').textContent, 'Navegador de Salas', 'lobby title in PT');
  assert.equal(tab.el('btn-solo').textContent, 'Jogar sozinho', 'solo button in PT');
  assert.equal(tab.el('btn-create-open').textContent, 'Criar Sala', 'create button in PT');
  assert.equal(tab.el('btn-join-lobby').textContent, 'Entrar na Sala', 'join button in PT');
  assert.equal(tab.el('lobby-status').textContent, 'Escolha um servidor e crie o jogo.', 'status line in PT');
  const head = tab.el('games-list').children[0];
  assert.ok(head.innerHTML.includes('Nome do Jogo'), 'table head in PT');
  assert.ok(head.innerHTML.includes('Jogadores'), 'players column in PT');
  const row = tab.el('games-list').children.find((c) => c.dataset && c.dataset.room === 'westopen01');
  assert.ok(row.innerHTML.includes('1v1 Arábia — North America West'), 'row title localized, region name kept');
  assert.ok(row.innerHTML.includes('Arábia'), 'map column in PT');
  assert.equal(tab.el('pause-title').textContent, 'Pausado', 'pause title in PT');
  assert.equal(tab.el('feedback-q').textContent, 'O que quebrou para você?', 'end card feedback question in PT');
  assert.equal(tab.el('lang-select').value, 'pt-BR', 'the select shows the active locale');
  console.log('PASS (1) boot with stored pt-BR renders the lobby in Portuguese');
}

// (2) Switch to English on the select: the same screen re-renders.
{
  const sel = tab.el('lang-select');
  sel.value = 'en';
  sel.dispatch('change');
  assert.equal(tab.el('lobby-title-h').textContent, 'Lobby Browser', 'title back to EN');
  assert.equal(tab.el('btn-solo').textContent, 'Play solo', 'solo button back to EN');
  assert.equal(tab.el('lobby-status').textContent, 'Pick a server, then create the game.', 'status back to EN');
  const head = tab.el('games-list').children[0];
  assert.ok(head.innerHTML.includes('Game Name'), 'table head back to EN');
  const row = tab.el('games-list').children.find((c) => c.dataset && c.dataset.room === 'westopen01');
  assert.ok(row.innerHTML.includes('1v1 Arabia — North America West'), 'row title back to EN');
  assert.equal(tab.local.getItem('skirmish2.locale'), 'en', 'choice persisted');
  console.log('PASS (2) switching to EN re-renders the current screen and persists');
}

// (3) Switch to Spanish from the PAUSE select: both selects agree.
{
  const sel = tab.el('lang-select-pause');
  sel.value = 'es';
  sel.dispatch('change');
  assert.equal(tab.el('lobby-title-h').textContent, 'Navegador de Salas', 'lobby title in ES');
  assert.equal(tab.el('btn-solo').textContent, 'Jugar en solitario', 'solo button in ES');
  assert.equal(tab.el('btn-create-open').textContent, 'Crear Sala', 'create button in ES');
  const head = tab.el('games-list').children[0];
  assert.ok(head.innerHTML.includes('Nombre de la Partida'), 'table head in ES');
  assert.equal(tab.el('pause-title').textContent, 'Pausa', 'pause title in ES');
  assert.equal(tab.el('lang-select').value, 'es', 'lobby select follows the pause select');
  assert.equal(tab.local.getItem('skirmish2.locale'), 'es', 'ES choice persisted');
  console.log('PASS (3) switching to ES from the pause select re-renders and syncs both selects');
}

console.log('I18N SMOKE: all cases passed');
