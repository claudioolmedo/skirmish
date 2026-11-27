// Short invite links (glink package, Claudio's request
// 2026-11-26): the invite link is the site root plus a short,
// unique code — https://skirmish.online/g=wlehsls — and whoever
// opens it lands in that room automatically.
//
// The mechanism, end to end:
//   - makeRoomId mints a 7-char code whose FIRST char encodes the
//     region (netplay.js), so the link needs no region param;
//   - the room screen's invite (field, Copy, Share) is the short
//     link, for public AND private rooms alike — a flag-less
//     guest of a private room is still asked for the password by
//     the host's 'pw-req' handshake before presence counts, so
//     dropping the private=1 flag loses no protection;
//   - GitHub Pages serves the root 404.html for the unknown
//     /g=CODE path, and it bounces to /play/?room=CODE;
//   - parsePlayParams derives the region from the code when the
//     link names none, and accepts `g` as the room param's alias
//     (the netplay-side pins live in netplay.test.mjs).
//
// Client/presentation only: the sim and the golden hash are
// untouched, and legacy 10-char long links keep working.
//
// Extension (homepage invite box): the homepage now mints the
// code itself and offers the host a claim link —
// /play/?dono=CODE. The claim boot runs the ordinary create
// flow with the code as an override (one claim, one room, the
// room screen at the end), consume-once like every link
// doctrine here. The pure claim parse lives in netplay.js
// (claimRoomFromParams, pinned in netplay.test.mjs); the pins
// below cover the app.js wiring and the guest-first pairing
// the flow relies on.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';

const appJs = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const notFoundUrl = new URL('../404.html', import.meta.url);

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

test('the invite link shown, copied and shared is the short root link', () => {
  const body = functionBody(appJs, 'refreshInviteLink');
  assert.ok(body.includes('buildShortUrl(location.origin, net.room)'),
    'refreshInviteLink builds the short link from the site root + code');
  assert.ok(!body.includes('buildPlayUrl'), 'the long link form is gone from the invite');
  assert.ok(!body.includes('net.privacy'), 'one link form for every room — no privacy branch');
});

test('room creation mints the code from the picked region', () => {
  assert.ok(appJs.includes('net.room = makeRoomId(net.region);'),
    'the create flow passes its region so the code can carry it');
});

test('private rooms: the flag-less guest is still asked for the password', () => {
  // The host demands the password in the handshake itself: on a
  // peer hello at a private room it sends 'pw-req' instead of
  // counting presence — no link flag involved.
  assert.ok(appJs.includes("net.client.send({ type: 'pw-req' });"),
    'the host sends pw-req to an unadmitted guest');
  // And the guest side of the same door: receiving 'pw-req'
  // flips a guest who arrived WITHOUT the private flag (pwState
  // 'na') into the password prompt.
  const at = appJs.indexOf("case 'pw-req':");
  assert.ok(at >= 0, 'the pw-req case exists');
  const body = appJs.slice(at, at + 600);
  assert.ok(body.includes("net.privacy = 'private';"),
    'the guest adopts the private posture from the handshake');
  assert.ok(body.includes("if (net.pwState === 'na') net.pwState = 'needed';"),
    'a flag-less guest is moved to the password prompt');
});

test('the address bar is cleaned of the g alias too', () => {
  const body = functionBody(appJs, 'cleanPlayUrl');
  assert.ok(body.includes("p.delete('g');"),
    'a consumed ?g= join leaves the bar room-free like ?room=');
});

test('the dono claim: consume-once mark, cleaned param, one armed override', () => {
  assert.ok(appJs.includes("const DONO_MARK_KEY = 'skirmish2.donoConsumed'"),
    'the once-per-tab-session claim mark lives in sessionStorage');
  const read = functionBody(appJs, 'readDonoMark');
  assert.ok(read.includes('sessionStorage.getItem(DONO_MARK_KEY)'), 'the mark is read at boot');
  const write = functionBody(appJs, 'writeDonoMark');
  assert.ok(write.includes('sessionStorage.setItem(DONO_MARK_KEY, code)'),
    'and written with the claimed code on consumption');
  const clean = functionBody(appJs, 'cleanPlayUrl');
  assert.ok(clean.includes("p.delete('dono')"),
    'a consumed claim leaves the address bar param-free like a join');
  assert.ok(appJs.includes('let claimedRoomCode = null;'),
    'the create override is a single armed slot, null at rest');
});

test('the claim boot runs the ordinary create flow with the code armed', () => {
  const bootAt = appJs.indexOf('linkJoinRoom(location.search, readRoomLinkMark())');
  assert.ok(bootAt >= 0, 'the boot block stands');
  const boot = appJs.slice(bootAt, bootAt + 2600);
  const claimAt = boot.indexOf('claimRoomFromParams(location.search, readDonoMark())');
  assert.ok(claimAt >= 0, 'the boot reads the claim');
  assert.ok(claimAt < boot.indexOf('cleanPlayUrl()'),
    'the claim is read before cleanPlayUrl strips the param');
  const branchAt = boot.indexOf('} else if (claim) {');
  assert.ok(branchAt > boot.indexOf('if (join) {'),
    'a room link still wins — the claim only runs when there is no join');
  const branch = boot.slice(branchAt, branchAt + 700);
  assert.ok(branch.includes('writeDonoMark(claim)'), 'the mark is written on the claim');
  assert.ok(branch.indexOf('writeDonoMark(claim)') < branch.indexOf('openLobby(null)'),
    'mark and clean first — a reload lands on the front door, never re-claims');
  assert.ok(branch.includes('claimedRoomCode = claim;'),
    'the code is armed as the create override');
  assert.ok(branch.indexOf('claimedRoomCode = claim;') < branch.indexOf('createGame();'),
    'armed before the ordinary create flow runs — no parallel create path');
});

test('createGame consumes the claim: exact code, region from the code', () => {
  const body = functionBody(appJs, 'createGame');
  assert.ok(body.includes('const claim = claimedRoomCode;'),
    'the override is taken into a local at the top');
  assert.ok(body.indexOf('const claim = claimedRoomCode;') < body.indexOf('claimedRoomCode = null;'),
    'and the slot is cleared immediately — an early return cannot leak it');
  assert.ok(body.includes('net.region = regionFromRoomId(claim) || net.region;'),
    'the room is born on the region the code carries');
  assert.ok(body.includes('net.room = claim;'),
    'with EXACTLY the homepage code — no re-mint');
  assert.ok(body.includes('net.room = makeRoomId(net.region);'),
    'the unclaimed create still mints from the picked region');
});

test('guest-first: a guest waiting in the room pairs when the host arrives', () => {
  // The relay is a dumb broadcast pipe and roles are client-side
  // state, so arrival order cannot matter — provided the hello
  // handshake is symmetric. It is: every client sends its hello
  // on connect (the host claiming late sends his then)...
  const connect = functionBody(appJs, 'connectToRoom');
  assert.ok(connect.includes("type: 'hello', v: 1, role: net.role"),
    'connect sends the hello with the client-side role, host or guest alike');
  // ...and the first hello a client RECEIVES is answered with
  // its own hello, so the early guest's presence reaches the
  // late host (and vice versa) without either re-sending.
  const helloAt = appJs.indexOf("case 'hello': {");
  assert.ok(helloAt >= 0, 'the hello case exists');
  const hello = appJs.slice(helloAt, helloAt + 3400);
  assert.ok(hello.includes('if (!net.helloReplied)'),
    'the reply is gated on not having replied yet');
  const replyAt = hello.indexOf('if (!net.helloReplied)');
  assert.ok(hello.slice(replyAt, replyAt + 500).includes("type: 'hello', v: 1, role: net.role"),
    'the unreplied hello is answered with a hello');
  assert.ok(hello.includes('net.peerPresent = true;'),
    'the (possibly reply) hello completes presence on the receiving side');
  // And presence is what the start gate consumes: the host's
  // client fires the handshake itself once both sides are
  // present and ready — a guest who readied while waiting is
  // paired the moment the host claims and readies.
  const gate = functionBody(appJs, 'maybeStartMatch');
  assert.ok(gate.includes('peerPresent: net.peerPresent'),
    'the both-ready gate reads the presence the hellos set');
  assert.ok(gate.includes('hostStartMatch()'),
    'and fires the host start handshake from it');
});

test('404.html exists at the site root and bounces /g=CODE to the play page', () => {
  assert.ok(existsSync(notFoundUrl), 'web-preview/404.html must exist (NEW root file for the gh-pages delta)');
  const html = readFileSync(notFoundUrl, 'utf8');
  assert.ok(html.includes('/^\\/g=([a-z2-9]{7})$/i'),
    'it recognises exactly the /g= + 7-char code shape');
  assert.ok(html.includes("location.replace('/play/?room=' + m[1].toLowerCase())"),
    'and replaces to the play page with the code as the room param');
  // Any other path is a plain 404 with a way home.
  assert.ok(html.includes('Page not found'), 'non-invite paths show the 404 text');
  assert.ok(html.includes('href="/"'), 'with a link back home');
  // Dependency-free: no external script or stylesheet rides along.
  assert.ok(!/<script[^>]+src=/.test(html), 'no external scripts');
  assert.ok(!/<link[^>]+stylesheet/.test(html), 'no external stylesheets');
});
