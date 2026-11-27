// Share + fullscreen-exit pins (sharefs package).
//
// Feature 1 — Chess.com-style friend sharing. The invite link
// already existed, but as one discreet text line with a small
// "copy" link (play/index.html's old .invite-row) that a player
// could easily miss. The room screen now carries a real invite
// panel: the full link in a read-only field, a labelled Copy
// button, and a native Share button where the browser offers
// navigator.share. The Lobby browser gained a prominent "Play
// with a friend" entry into the create view, with the one-line
// explanation. The link itself is unchanged — built on demand
// from in-memory room state, address bar room-free, guest
// auto-join consume-once untouched.
//
// Feature 2 — the obvious way out of fullscreen. Leaving used to
// mean finding the toggle in the top bar or the pause menu (or
// knowing the hold-Esc trick). Now a labelled "✕ Exit
// fullscreen" pill floats under the top bar whenever fullscreen
// is active (driven by syncFullscreenUi, the one place that
// already tracks the state), and the first match start in a tab
// session shows a ~6 s hint naming both exits.
//
// Presentation/client only: the sim, the golden hash and the
// room-link semantics are untouched.
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

test('room screen carries the invite panel: read-only link field, Copy and Share buttons', () => {
  assert.ok(playIndex.includes('class="invite-panel"'),
    'the invite panel replaces the discreet invite row');
  assert.ok(!playIndex.includes('class="invite-row"'),
    'the old discreet invite row is gone');
  assert.ok(/<input id="invite-link-input" type="text" readonly/.test(playIndex),
    'the full invite link lives in a read-only text input');
  assert.ok(/<label class="invite-label" id="invite-txt" for="invite-link-input">/.test(playIndex),
    'the invite label names the field');
  assert.ok(/<button id="btn-copy-link" class="btn btn-gold btn-sm" type="button">Copy link<\/button>/.test(playIndex),
    'copy is a real labelled button, not a small text link');
  assert.ok(!playIndex.includes('id="btn-copy-link" class="linklike"'),
    'the copy action no longer wears the linklike styling');
  assert.ok(/<button id="btn-share-link" class="btn btn-ghost btn-sm" type="button" hidden>Share…<\/button>/.test(playIndex),
    'the Share button exists and starts hidden (app.js unhides it only with navigator.share)');
});

test('the invite link still comes from refreshInviteLink over in-memory room state', () => {
  const body = functionBody(appJs, 'refreshInviteLink');
  assert.ok(body.includes('buildPlayUrl('), 'the link is built by buildPlayUrl');
  assert.ok(body.includes('net.room'), 'from the in-memory room state');
  assert.ok(body.includes('inviteLinkInput'), 'and lands in the read-only field');
  assert.ok(!body.includes('location.search'), 'never parsed back out of the address bar');
  // Guest auto-join from a link: the consume-once block stands.
  assert.ok(appJs.includes('room-link consume-once'),
    'the consume-once guest auto-join block is untouched');
});

test('share button logic hides without navigator.share and shares silently', () => {
  const supported = functionBody(appJs, 'shareSupported');
  assert.ok(supported.includes("typeof navigator.share === 'function'"),
    'share support is the navigator.share feature test');
  assert.ok(appJs.includes('netUi.shareLinkBtn.hidden = !shareSupported()'),
    'renderRoom hides the Share button where sharing is unavailable');
  assert.ok(appJs.includes('shareLinkBtn.hidden = !shareSupported()'),
    'the wiring applies the same visibility rule');
  assert.ok(appJs.includes('navigator.share({'), 'the button calls navigator.share');
  assert.ok(appJs.includes("text: t('room.shareText')"), 'with the localized share text');
  assert.ok(appJs.includes('url: link'), 'sharing the freshly built invite link');
  // Cancel/failure is silent: the rejection is swallowed, and no
  // flash() rides the share path.
  const shareAt = appJs.indexOf('navigator.share({');
  const shareBlock = appJs.slice(shareAt, shareAt + 700);
  assert.ok(shareBlock.includes('.catch(() => {})'),
    'a dismissed share sheet rejects silently');
  assert.ok(!shareBlock.includes('flash('),
    'no toast on the share path — cancel is not an error');
});

test('lobby browser offers "Play with a friend" into the create view, with the note', () => {
  assert.ok(/<button id="btn-play-friend" class="btn btn-gold" type="button">Play with a friend<\/button>/.test(playIndex),
    'the prominent friend button sits in the Lobby browser actions');
  assert.ok(playIndex.includes('id="play-friend-note"'),
    'the one-line explanation sits under it');
  assert.ok(appJs.includes("playFriendBtn: document.getElementById('btn-play-friend')"),
    'app.js collects the button');
  const wireAt = appJs.indexOf('netUi.playFriendBtn.addEventListener');
  assert.ok(wireAt >= 0, 'the button is wired');
  const wire = appJs.slice(wireAt, wireAt + 300);
  assert.ok(wire.includes('netUi.gamesSection.hidden = true'), 'it leaves the browser view');
  assert.ok(wire.includes('netUi.createSection.hidden = false'),
    'and lands on the create view — the same navigation as Create Lobby');
});

test('fullscreen exit pill exists and syncFullscreenUi drives its visibility and label', () => {
  assert.ok(/<button id="fs-exit-pill" type="button" hidden>✕ Exit fullscreen<\/button>/.test(playIndex),
    'the pill exists, labelled with text (not just a glyph), starting hidden');
  const sync = functionBody(appJs, 'syncFullscreenUi');
  assert.ok(sync.includes('fsExitPill.hidden = !fs'),
    'visibility is the fullscreen state itself, synced in syncFullscreenUi');
  assert.ok(sync.includes("'✕ ' + t('top.exitFullscreen')"),
    'the label is the ✕ plus the localized Exit fullscreen string');
  assert.ok(appJs.includes("fsExitPill.addEventListener('click', toggleFullscreen)"),
    'the click runs the same toggleFullscreen exit path as the top-bar button');
  // Layering: the pill rides the capture pill layer (z 30) under
  // the top bar's bottom edge, semi-transparent at rest.
  const pillCss = playCss.slice(playCss.indexOf('#fs-exit-pill {'), playCss.indexOf('#fs-exit-pill[hidden]'));
  assert.ok(pillCss.includes('position: fixed'), 'the pill floats');
  assert.ok(pillCss.includes('z-index: 30'), 'on the capture-pill layer, above the HUD, below the menus');
  assert.ok(pillCss.includes('top: calc(var(--beta-h) + 50px)'), 'below the top bar');
  assert.ok(pillCss.includes('right: 14px'), 'at the top-right');
  assert.ok(pillCss.includes('opacity: 0.55'), 'semi-transparent at rest');
  assert.ok(playCss.includes('#fs-exit-pill:hover, #fs-exit-pill:focus-visible { opacity: 1; }'),
    'fully opaque on hover/focus');
});

test('fullscreen hint: once per tab session, ~6 s, localized, non-interactive', () => {
  assert.ok(/<div id="fs-hint" hidden role="status"><\/div>/.test(playIndex),
    'the hint element exists and starts hidden');
  assert.ok(appJs.includes("const FS_HINT_KEY = 'skirmish2.fsHintShown'"),
    'the once-per-tab-session flag lives in sessionStorage');
  assert.ok(appJs.includes('sessionStorage.getItem(FS_HINT_KEY)'),
    'and is read before arming the hint');
  const show = functionBody(appJs, 'showFsHint');
  assert.ok(show.includes("fsHintEl.textContent = t('fs.hint')"), 'the hint text is localized');
  assert.ok(show.includes('setTimeout(hideFsHint, 6000)'), 'and lives ~6 s');
  const change = functionBody(appJs, 'onFullscreenChange');
  assert.ok(change.includes('fsHintPending') && change.includes('showFsHint()'),
    'the hint shows when a match start\u2019s fullscreen transition settles');
  const hintCss = playCss.slice(playCss.indexOf('#fs-hint {'), playCss.indexOf('#fs-hint[hidden]'));
  assert.ok(hintCss.includes('pointer-events: none'), 'the hint never blocks play');
});

test('every new i18n key exists in EN, PT and ES with the pinned labels', () => {
  const NEW_KEYS = [
    'room.copyLink', 'room.share', 'room.shareText',
    'lobby.playFriend', 'lobby.playFriendNote', 'fs.hint',
  ];
  for (const loc of LOCALES) {
    for (const key of NEW_KEYS) {
      assert.equal(typeof STRINGS[loc][key], 'string', `${loc}:${key} must exist`);
      assert.ok(STRINGS[loc][key].length > 0, `${loc}:${key} must not be empty`);
    }
  }
  assert.equal(STRINGS.en['room.copyLink'], 'Copy link');
  assert.equal(STRINGS['pt-BR']['room.copyLink'], 'Copiar link');
  assert.equal(STRINGS.es['room.copyLink'], 'Copiar enlace');
  assert.equal(STRINGS.en['room.share'], 'Share…');
  assert.equal(STRINGS['pt-BR']['room.share'], 'Compartilhar…');
  assert.equal(STRINGS.es['room.share'], 'Compartir…');
  assert.equal(STRINGS.en['lobby.playFriend'], 'Play with a friend');
  assert.equal(STRINGS['pt-BR']['lobby.playFriend'], 'Jogar com um amigo');
  assert.equal(STRINGS.es['lobby.playFriend'], 'Jugar con un amigo');
  assert.equal(STRINGS['pt-BR']['top.exitFullscreen'], 'Sair da tela cheia',
    'the exit pill label key the pill is built from');
});
