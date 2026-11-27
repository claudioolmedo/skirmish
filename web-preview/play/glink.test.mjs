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
