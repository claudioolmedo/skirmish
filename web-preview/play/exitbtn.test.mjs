// Close game pins (exitbtn package).
//
// Claudio's report (2026-11-26): for someone who has never
// entered, closing the game is very complicated — fullscreen +
// pointer lock hide the browser's own exits, and every in-game
// exit landed on another game screen (the Lobby Browser, a fresh
// local game under the pause menu). The only door to the website
// homepage was a note-sized text link at the bottom of the pause
// menu. Now a persistent, text-labelled "✕ Close game" button
// floats at the top right on the lobby screens and in-match:
// lobby/room exits are immediate; over a live match one inline
// confirm (Leave / Stay) stands in front, and Leave runs the
// existing resign/leave machinery before navigating to '/'
// (the homepage — '../' from /play/, the back-to-site target).
// The fullscreen pill is untouched and keeps its own meaning:
// it leaves fullscreen, this button closes the game.
//
// Presentation/client only: no sim, no netplay protocol change —
// the peer hears the same 'resign' / 'cancel' messages the
// existing flows already send.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { STRINGS, LOCALES } from './i18n.js';

const playIndex = readFileSync(new URL('./index.html', import.meta.url), 'utf8');
const appJs = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const playCss = readFileSync(new URL('./play.css', import.meta.url), 'utf8');

function functionBody(src, name) {
  const start = src.indexOf(`function ${name}(`);
  assert.ok(start >= 0, `${name} must exist in app.js`);
  const open = src.indexOf('{', start);
  let depth = 0;
  for (let i = open; i < src.length; i++) {
    if (src[i] === '{') depth++;
    else if (src[i] === '}') { depth--; if (depth === 0) return src.slice(open, i + 1); }
  }
  throw new Error(`unbalanced ${name}`);
}

test('the button exists: real text label, starts hidden, beside the fullscreen pill', () => {
  assert.ok(/<button id="close-game-btn" type="button" hidden>✕ Close game<\/button>/.test(playIndex),
    'a labelled button (not a bare glyph), hidden until syncCloseGameUi shows it');
  const pillAt = playIndex.indexOf('id="fs-exit-pill"');
  const btnAt = playIndex.indexOf('id="close-game-btn"');
  assert.ok(pillAt >= 0 && btnAt > pillAt,
    'the button follows the pill in the DOM — the stacking sibling rules key off that order');
  assert.ok(/<div id="close-game-confirm" hidden role="alertdialog"/.test(playIndex),
    'the inline confirm exists and starts hidden');
  assert.ok(/<p id="close-game-confirm-txt">Leave this match\? The game will close\.<\/p>/.test(playIndex),
    'the confirm question is real text in the HTML (the no-JS paint)');
  assert.ok(/<button id="close-game-leave" type="button">Leave<\/button>/.test(playIndex),
    'Leave is a real button');
  assert.ok(/<button id="close-game-stay" type="button">Stay<\/button>/.test(playIndex),
    'Stay is a real button');
});

test('every close-game i18n key exists in EN, PT and ES with the pinned labels', () => {
  const KEYS = ['top.closeGame', 'closeGame.confirm', 'closeGame.leave', 'closeGame.stay'];
  for (const loc of LOCALES) {
    for (const key of KEYS) {
      assert.equal(typeof STRINGS[loc][key], 'string', `${loc}:${key} must exist`);
      assert.ok(STRINGS[loc][key].length > 0, `${loc}:${key} must not be empty`);
    }
  }
  assert.equal(STRINGS.en['top.closeGame'], 'Close game');
  assert.equal(STRINGS['pt-BR']['top.closeGame'], 'Fechar o jogo');
  assert.equal(STRINGS.es['top.closeGame'], 'Cerrar el juego');
  assert.equal(STRINGS.en['closeGame.confirm'], 'Leave this match? The game will close.');
  assert.equal(STRINGS['pt-BR']['closeGame.confirm'], 'Sair desta partida? O jogo vai fechar.');
  assert.equal(STRINGS.es['closeGame.confirm'], '¿Salir de esta partida? El juego se cerrará.');
  assert.equal(STRINGS.en['closeGame.leave'], 'Leave');
  assert.equal(STRINGS['pt-BR']['closeGame.leave'], 'Sair');
  assert.equal(STRINGS.es['closeGame.leave'], 'Salir');
  assert.equal(STRINGS.en['closeGame.stay'], 'Stay');
  assert.equal(STRINGS['pt-BR']['closeGame.stay'], 'Ficar');
  assert.equal(STRINGS.es['closeGame.stay'], 'Quedarme');
  // Distinct from the fullscreen pill's key in every locale:
  // the two ✕ actions must never share a label.
  for (const loc of LOCALES) {
    assert.notEqual(STRINGS[loc]['top.closeGame'], STRINGS[loc]['top.exitFullscreen'],
      `${loc}: Close game and Exit fullscreen read as different actions`);
  }
});

test('syncCloseGameUi: labels from i18n, visibility from lobby / loading / in-match', () => {
  assert.ok(appJs.includes("const closeGameBtn = document.getElementById('close-game-btn')"),
    'app.js collects the button');
  const sync = functionBody(appJs, 'syncCloseGameUi');
  assert.ok(sync.includes("'✕ ' + t('top.closeGame')"),
    'the label is the ✕ plus the localized Close game string — the pill\u2019s own recipe');
  assert.ok(sync.includes("t('closeGame.confirm')"), 'the confirm question is localized');
  assert.ok(sync.includes("t('closeGame.leave')"), 'Leave is localized');
  assert.ok(sync.includes("t('closeGame.stay')"), 'Stay is localized');
  assert.ok(sync.includes('netUi.onlineOverlay'),
    'the Lobby Browser / create / room screens are one ground: the lobby overlay');
  assert.ok(sync.includes('netUi.loadingOverlay'), 'the loading screen counts too');
  assert.ok(sync.includes("classList.contains('in-match')"), 'in-match is the third ground');
  assert.ok(sync.includes('closeGameBtn.hidden = !(lobbyUp || loadingUp || inMatch)'),
    'visible exactly on those three grounds');
  assert.ok(sync.includes('hideCloseGameConfirm()'),
    'hiding the button folds a standing confirm away with it');
});

test('visibility re-syncs at the choke points: showOverlay, setInMatch, applyLocale, wiring', () => {
  const show = functionBody(appJs, 'showOverlay');
  assert.ok(show.includes('el === netUi.onlineOverlay || el === netUi.loadingOverlay'),
    'showOverlay re-syncs on the lobby/loading flips — openLobby, enterLoading, teardownNet ride it');
  assert.ok(show.includes('syncCloseGameUi()'), 'the re-sync is the one sync function');
  const sim = functionBody(appJs, 'setInMatch');
  assert.ok(sim.includes('syncCloseGameUi()'), 'setInMatch re-syncs (.in-match flips there)');
  const loc = functionBody(appJs, 'applyLocale');
  assert.ok(loc.includes('syncCloseGameUi()'), 'applyLocale re-syncs — the labels follow the language');
  assert.ok(appJs.includes("closeGameBtn.addEventListener('click', onCloseGameClick)"),
    'the button is wired');
  assert.ok(appJs.includes("closeGameStayBtn.addEventListener('click', hideCloseGameConfirm)"),
    'Stay only folds the confirm away');
  assert.ok(appJs.includes("closeGameLeaveBtn.addEventListener('click', closeGameExit)"),
    'Leave runs the exit');
});

test('the confirm gate: only a live match asks first; the lobby makes exits immediate', () => {
  const gate = functionBody(appJs, 'closeGameLiveMatch');
  assert.ok(gate.includes('netUi.onlineOverlay') && gate.includes('return false'),
    'with the lobby up the exit is immediate — no confirm');
  assert.ok(gate.includes("net.phase === 'playing'"),
    'online, the contested playing phase is the live case');
  assert.ok(!gate.includes("'peer-left'"),
    'peer-left has nobody left to protect — no confirm');
  assert.ok(gate.includes('playing && !matchEnded && !playback'),
    'locally: a running, undecided match, and never a replay');
  const click = functionBody(appJs, 'onCloseGameClick');
  assert.ok(click.includes('closeGameLiveMatch()'), 'the click consults the gate');
  assert.ok(click.includes('closeGameConfirmEl.hidden = false'),
    'a live match raises the inline confirm instead of exiting');
  assert.ok(click.includes('closeGameExit()'), 'every other state exits at once');
});

test('closeGameExit reuses the existing leave paths, releases capture, lands on the homepage', () => {
  const exit = functionBody(appJs, 'closeGameExit');
  assert.ok(exit.includes("cancelLoading(true)"),
    'a match still loading leaves through the one cancellation path');
  assert.ok(exit.includes("net.phase === 'starting'"),
    'the cancellation only fires in the loading phase');
  assert.ok(exit.includes("net.client.send({ type: 'resign' })"),
    'a live online match notifies the peer with the Resign button\u2019s own message');
  assert.ok(exit.includes("net.phase === 'playing'"),
    'the resign only fires over a live match');
  assert.ok(exit.includes('teardownNet()'),
    'the session teardown is the common leave path, not a parallel one');
  assert.ok(exit.includes('exitGamePointerLock()'), 'the pointer lock is released');
  assert.ok(exit.includes('document.exitFullscreen'), 'fullscreen is exited, exit-only');
  assert.ok(!exit.includes('toggleFullscreen'),
    'never the toggle: closing must not be able to ENTER fullscreen');
  assert.ok(exit.includes("location.href = '../'"),
    'the landing is the website homepage — the back-to-site target');
  assert.ok(!exit.includes('window.close'),
    'window.close is never attempted (dead on player-opened tabs)');
  // The Resign button's own send is untouched — the exit rides
  // the same message, it does not fork it.
  assert.ok(appJs.includes("net.client.send({ type: 'resign' });\n        showOverlay(netUi.peerOverlay, false);"),
    'the pause-menu Resign handler stands as it was');
});

test('CSS: top-right, opaque red slab above the menus, stacking clear of the pill', () => {
  const btnCss = playCss.slice(playCss.indexOf('#close-game-btn {'), playCss.indexOf('#close-game-btn[hidden]'));
  assert.ok(btnCss.includes('position: fixed'), 'the button floats');
  assert.ok(btnCss.includes('top: calc(var(--beta-h) + 50px)'),
    'on the pills\u2019 row, below the top bar');
  assert.ok(btnCss.includes('right: 14px'), 'at the top-right, the pill\u2019s edge');
  assert.ok(btnCss.includes('z-index: 48'),
    'above the menu overlays (40) and the beta strip (45), below the toast (50)');
  assert.ok(btnCss.includes('background: rgba(122, 32, 18, 0.95)'),
    'the opaque danger-red slab — visibly not the pill\u2019s translucent dark pill');
  assert.ok(btnCss.includes('border-radius: 10px'),
    'square corners, not the pill\u2019s 999px capsule');
  assert.ok(!btnCss.includes('opacity: 0.55'),
    'never semi-transparent at rest — a first-timer must see it');
  assert.ok(playCss.includes('#close-game-btn[hidden] { display: none; }'),
    'the hidden attribute always wins');
  assert.ok(playCss.includes('#fs-exit-pill:not([hidden]) ~ #close-game-btn { top: calc(var(--beta-h) + 94px); }'),
    'with the fullscreen pill up, the button stacks below it — no collision, both labels readable');
  const confirmCss = playCss.slice(playCss.indexOf('#close-game-confirm {'), playCss.indexOf('#close-game-confirm[hidden]'));
  assert.ok(confirmCss.includes('z-index: 48'), 'the confirm rides the same layer');
  assert.ok(confirmCss.includes('right: 14px'), 'anchored to the same top-right corner');
  assert.ok(playCss.includes('#fs-exit-pill:not([hidden]) ~ #close-game-confirm'),
    'the confirm stacks with the same pill discipline');
  assert.ok(playCss.includes('@media (max-width: 700px)'),
    'the small-screen offset block exists (the top bar wraps there)');
});

test('existing exits are untouched: pill, pause menu, lobby ✕, back-to-site link', () => {
  assert.ok(/<button id="fs-exit-pill" type="button" hidden>✕ Exit fullscreen<\/button>/.test(playIndex),
    'the fullscreen pill stands');
  assert.ok(appJs.includes("fsExitPill.addEventListener('click', toggleFullscreen)"),
    'and still exits fullscreen only');
  assert.ok(playIndex.includes('id="back-site-link" href="../"'),
    'the pause menu\u2019s back-to-site link stands — the button joins it, not replaces it');
  assert.ok(playIndex.includes('id="online-close"'), 'the lobby card ✕ stands');
  assert.ok(playIndex.includes('id="pause-btn"'), 'the in-game Menu button stands');
  assert.ok(playIndex.includes('id="btn-resign"'), 'the Resign button stands');
  assert.ok(playIndex.includes('id="end-menu-btn"'), 'the end card\u2019s Back to menu stands');
  assert.ok(playIndex.includes('id="pb-exit"'), 'the playback bar\u2019s Exit replay stands');
});
