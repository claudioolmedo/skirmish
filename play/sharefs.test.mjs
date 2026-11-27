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
//
// Follow-up (same package): sharing moves to the very start of
// the site. The homepage hero gains a Play / Play a friend CTA
// row and a share box (the game's link, ready to copy or send),
// the play page boots ?amigo=1 straight into the create-room
// view (consume-once, like a room link), and the homepage gains
// a Quick tips section covering the friend flow and the
// fullscreen exits.
//
// Extension (glink package): the homepage share box becomes an
// INVITE box — it mints a fresh room code on load (region probed
// nearest the visitor), the field/copy/share carry the room's
// short link, and an "Enter this room and play" button claims
// the code on the play page (?dono=CODE; the claim wiring is
// pinned in glink.test.mjs, the pure parse in netplay.test.mjs).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { STRINGS, LOCALES } from './i18n.js';

const playIndex = readFileSync(new URL('./index.html', import.meta.url), 'utf8');
const appJs = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const playCss = readFileSync(new URL('./play.css', import.meta.url), 'utf8');
const homeIndex = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
const siteCss = readFileSync(new URL('../site.css', import.meta.url), 'utf8');

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
  // glink (2026-11-26) superseded the link FORM, not the
  // mechanism: still built on demand from in-memory state, now
  // as the short root link via buildShortUrl (see glink.test.mjs).
  assert.ok(body.includes('buildShortUrl('), 'the link is built by buildShortUrl');
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

test('homepage hero: few options — title, tagline, short line, invite box; no CTA button row', () => {
  // Claudio's dedup corrections (2026-11-26, after the glink
  // publish): the friend action appeared twice in the hero —
  // the invite box AND a "Play a friend" button — "ainda está
  // tudo duplicado, muito confuso, tem que ter poucas opções".
  // Final hero: title → tagline → one short line → invite box
  // (link + Copy + Share + "Enter this room and play") and
  // NOTHING else: the whole hero CTA row (both "Play a friend"
  // and "Play the test build") is gone. The top nav keeps its
  // "Play" link as the general door to the game; the ?amigo=1
  // boot handling and the in-Lobby friend button stay.
  const heroSlice = homeIndex.slice(0, homeIndex.indexOf('</section>'));
  assert.ok(!heroSlice.includes('hero-cta'),
    'no CTA button row in the hero');
  assert.ok(!heroSlice.includes('play/?amigo=1'),
    'no ?amigo=1 anchor in the hero');
  assert.ok(!heroSlice.includes('>Play the test build<'),
    'no "Play the test build" button in the hero');
  assert.ok(!heroSlice.includes('>Play a friend<'),
    'no "Play a friend" button in the hero');
  assert.ok(heroSlice.includes('Enter this room and play'),
    'the invite box keeps its own door into the room');
  assert.ok(homeIndex.includes('<a class="btn btn-gold nav-play" href="play/">Play</a>'),
    'the top nav keeps its Play link — the general door to the game');
  const boxAt = homeIndex.indexOf('class="share-box invite-box"');
  // One box only, directly under the tagline text — the first
  // (and only) interactive element a visitor reaches in the
  // hero. The hero was simplified to title → tagline → one
  // short line → invite box (the long pitch and the chess
  // line moved down to the Why section).
  assert.ok(boxAt > homeIndex.indexOf('class="tagline"'),
    'the invite box sits under the tagline');
  assert.ok(boxAt > homeIndex.indexOf('class="hero-short"'),
    'the one short line stands between the tagline and the box');
  assert.ok(homeIndex.includes('Play a friend right in the browser &mdash; no install, no account.'),
    'the short line: play a friend, in the browser, no install, no account');
  const heroEnd = homeIndex.indexOf('</section>');
  assert.ok(!homeIndex.slice(0, heroEnd).includes('class="pitch"'),
    'the long pitch paragraph is out of the hero');
  assert.ok(!homeIndex.slice(0, heroEnd).includes('class="chess-line"'),
    'the chess line is out of the hero');
  const aboutAt = homeIndex.indexOf('id="about"');
  assert.ok(homeIndex.indexOf('class="pitch"') > aboutAt,
    'the pitch moved into the Why section, not deleted');
  assert.ok(homeIndex.indexOf('kept alive by the community') > aboutAt,
    'with its community text intact');
  assert.ok(homeIndex.indexOf('class="chess-line"') > aboutAt,
    'the chess line moved into the Why section too');
  assert.equal(homeIndex.split('class="share-box').length - 1, 1,
    'exactly one share/invite box on the homepage — the invite box replaced the generic share box');
  assert.ok(!homeIndex.includes('Or send the game to a friend'),
    'the old generic-link share box copy is gone — no duplicate friend flow');
});

test('homepage invite box: a fresh room code at the front door, copy/share/enter wired', () => {
  // The box (glink extension): the hero share box upgraded — the
  // field starts on the game link (the no-JS / pre-probe
  // fallback) and the script swaps in the room's short link.
  assert.ok(/<input id="site-share-link" type="text" readonly value="https:\/\/skirmish\.online\/play\/">/.test(homeIndex),
    'the field starts on the game link, read-only');
  assert.ok(homeIndex.includes('Copy this link and share it with your friend to play — you\'ll both land in the same room:'),
    'the label is the one-line explanation: copy, share, both land in the same room');
  assert.ok(/<button id="site-share-copy" class="btn btn-gold" type="button">Copy link<\/button>/.test(homeIndex),
    'copy is a real labelled button');
  assert.ok(/<button id="site-share-share" class="btn btn-ghost" type="button" hidden>Share&hellip;<\/button>/.test(homeIndex),
    'the Share button exists and starts hidden (the script unhides it only with navigator.share)');
  assert.ok(/<a id="site-invite-enter" class="btn btn-gold" href="play\/">Enter this room and play<\/a>/.test(homeIndex),
    'the host entry button exists, starting on the plain play link until the code lands');
  // The code scheme replicates the game's (netplay.js): the
  // no-lookalikes alphabet and the region-prefix map, all seven.
  assert.ok(homeIndex.includes("var ALPHABET = 'abcdefghjkmnpqrstuvwxyz23456789';"),
    'the homepage mints from the same alphabet as makeRoomId');
  for (const [id, prefix] of [['wnam', 'w'], ['enam', 'e'], ['sam', 's'], ['weur', 'u'], ['eeur', 'r'], ['apac', 'a'], ['oc', 'c']]) {
    assert.ok(homeIndex.includes(`{ id: '${id}', prefix: '${prefix}' }`),
      `the prefix map carries ${id} → ${prefix}, matching REGION_PREFIX`);
  }
  // The region is measured, not guessed: a probe per region
  // against the relay's probe rooms, timing the second echo,
  // inside a ~1.5 s budget, with wnam as the silence fallback.
  assert.ok(homeIndex.includes("'/room/probe-' + region.id + '-' + suffix"),
    'the probe opens the region probe room, like probeRegionPing');
  assert.ok(homeIndex.includes("'?region=' + encodeURIComponent(region.id)"),
    'with the region as the placement hint');
  assert.ok(homeIndex.includes('if (echoes === 1)'),
    'the first echo only re-arms — the second is the timed one');
  assert.ok(homeIndex.includes('var PROBE_BUDGET_MS = 1500;'),
    'the probe budget is ~1.5 s — the box never blocks the page');
  assert.ok(homeIndex.includes('adoptRegion(best || REGIONS[0]);'),
    'silence everywhere falls back to the first region (wnam)');
  // Once minted, the code never changes (the decided guard), and
  // adopting it rewires the whole box to the one room: the field
  // shows the short link and Enter claims the same code.
  assert.ok(homeIndex.includes('if (decided) return;'),
    'the region decision lands once — a copied link can never be swapped under the visitor');
  assert.ok(homeIndex.includes("url = 'https://skirmish.online/g=' + code;"),
    'the shared link is the short root link');
  assert.ok(homeIndex.includes('input.value = url;'), 'the field shows it');
  assert.ok(homeIndex.includes("enterBtn.href = 'play/?dono=' + code;"),
    'Enter claims exactly that code through the dono boot');
  // Copy/Share ride the same final url variable.
  assert.ok(homeIndex.includes("typeof navigator.share === 'function'"),
    'the homepage script feature-tests navigator.share like the play page does');
  assert.ok(homeIndex.includes('navigator.clipboard.writeText(url)'),
    'copy writes the link to the clipboard');
  assert.ok(homeIndex.includes('navigator.share({'),
    'the Share button calls navigator.share');
  assert.ok(homeIndex.includes("input.addEventListener('click', function () { input.select(); })"),
    'the field selects on click for a manual copy');
  // Cancel is silent on the homepage too: the rejection is
  // swallowed and no alert rides the share path.
  const shareAt = homeIndex.indexOf('navigator.share({');
  const shareBlock = homeIndex.slice(shareAt, shareAt + 500);
  assert.ok(shareBlock.includes('p.catch(function () {})'),
    'a dismissed share sheet rejects silently');
  assert.ok(!shareBlock.includes('alert('), 'no alert on the share path');
  const boxCss = siteCss.slice(siteCss.indexOf('.share-box {'), siteCss.indexOf('.share-label {'));
  assert.ok(boxCss.includes('border-radius: 10px'), 'the share box wears the site panel styling');
  assert.ok(siteCss.includes('.invite-enter-row {'), 'the enter row is styled in site.css');
});

test('homepage Quick tips: the five one-liners, friend flow and fullscreen exits', () => {
  assert.ok(homeIndex.includes('<section class="section" id="tips">'),
    'the tips section exists on the homepage');
  assert.ok(homeIndex.includes('<h2>Quick tips</h2>'), 'titled Quick tips');
  assert.ok(homeIndex.includes('No account, no install'),
    'tip 1: no account, no install');
  assert.ok(homeIndex.includes('create a room and send them the invite link'),
    'tip 2: the friend flow — create a room, send the invite link');
  assert.ok(homeIndex.includes('they land straight in your room'),
    'tip 2: the friend lands straight in the room');
  assert.ok(homeIndex.includes('A match opens in fullscreen'),
    'tip 3: a match opens in fullscreen');
  assert.ok(homeIndex.includes('&times; Exit fullscreen button (top-right)'),
    'tip 3: the ✕ Exit fullscreen button is the way out');
  assert.ok(homeIndex.includes('or by holding Esc'),
    'tip 3: holding Esc is the other way out');
  assert.ok(homeIndex.includes('A short Esc press cancels and closes panels'),
    'tip 4: short Esc works the game, not the fullscreen');
  assert.ok(homeIndex.includes('on phone and computer'),
    'tip 5: phone and computer');
  assert.ok(siteCss.includes('.tips-list {'), 'the tips list is styled in site.css');
});

test('play boot ?amigo=1: consume-once jump to the create view, param cleaned', () => {
  assert.ok(appJs.includes("const AMIGO_MARK_KEY = 'skirmish2.amigoConsumed'"),
    'the once-per-tab-session flag lives in sessionStorage');
  const read = functionBody(appJs, 'readAmigoMark');
  assert.ok(read.includes('sessionStorage.getItem(AMIGO_MARK_KEY)'), 'the mark is read at boot');
  const write = functionBody(appJs, 'writeAmigoMark');
  assert.ok(write.includes('sessionStorage.setItem(AMIGO_MARK_KEY'), 'and written on consumption');
  const intent = functionBody(appJs, 'amigoIntent');
  assert.ok(intent.includes("get('amigo') === '1'"), 'the intent is the amigo=1 param');
  assert.ok(intent.includes('if (consumed) return false'),
    'a consumed amigo link never jumps twice');
  const clean = functionBody(appJs, 'cleanPlayUrl');
  assert.ok(clean.includes("p.delete('amigo')"),
    'the amigo param leaves the address bar with the room params');
  // The boot block: intent read BEFORE the URL is cleaned, the
  // room link wins, and only the plain-lobby landing jumps —
  // mark first, then the same section flip the friend button
  // makes.
  const bootAt = appJs.indexOf('linkJoinRoom(location.search, readRoomLinkMark())');
  assert.ok(bootAt >= 0, 'the boot block stands');
  // The slice reaches past the dono claim branch (glink
  // extension) into the plain-landing amigo jump at the end.
  const boot = appJs.slice(bootAt, bootAt + 3400);
  const intentAt = boot.indexOf('amigoIntent(location.search, readAmigoMark())');
  assert.ok(intentAt >= 0, 'the boot reads the amigo intent');
  assert.ok(intentAt < boot.indexOf('cleanPlayUrl()'),
    'the intent is read before cleanPlayUrl strips the param');
  const jumpAt = boot.indexOf('if (amigo) {');
  assert.ok(jumpAt > boot.indexOf('openLobby(null)'),
    'the jump rides the plain Lobby Browser landing (a room link still wins)');
  const jump = boot.slice(jumpAt, jumpAt + 500);
  assert.ok(jump.includes('writeAmigoMark()'), 'the mark is written on the jump');
  assert.ok(jump.indexOf('writeAmigoMark()') < jump.indexOf('netUi.gamesSection.hidden = true'),
    'mark first — a reload lands on the front door');
  assert.ok(jump.includes('netUi.gamesSection.hidden = true'), 'it leaves the browser view');
  assert.ok(jump.includes('netUi.createSection.hidden = false'),
    'and lands on the create view — the same navigation as the friend button');
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
