// Regression tests for the "guest name invisible in the room"
// defect (bot-loop run #422, 2026-11-18, live two-client proof):
// a guest who set the display name "GuestPlayer" appeared to the
// host ONLY as "Guest — Red" in the room slot line, and the
// loading screen showed only "Red — Guest" / "Guest — Red" — the
// chosen name was never shown to the other player.
//
// Root cause pinned by these tests: the name DOES travel — the
// peer's hello carries `name: myName()` and app.js stores it in
// `net.peerName` (cleaned) — and chat already reads it. But the
// two seat renderers (renderRoom / renderLoading in app.js)
// built their labels from role + colour only
// (t('room.guestSeat') / t('load.redGuest') / the host twins) and
// never consulted net.peerName. The fix routes those labels
// through room.js's peerSeatLabel: a custom peer name replaces
// the role word (colour kept), and the classic label is the
// fallback when no custom name was set (an unset name rides the
// hello as the "Guest" sentinel — see growth.js displayName).
// Run from web-preview/play/: node --test guestname.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { customPeerName, peerSeatLabel } from './room.js';
import { tIn } from './i18n.js';

const read = (rel) => readFileSync(new URL(rel, import.meta.url), 'utf8');

test('customPeerName: a chosen name survives, the unset sentinel does not', () => {
  assert.equal(customPeerName('GuestPlayer'), 'GuestPlayer');
  assert.equal(customPeerName('  GuestPlayer  '), 'GuestPlayer');
  // displayName() sends "Guest" when nothing was typed — that is
  // the unset sentinel, not a chosen name.
  assert.equal(customPeerName('Guest'), null);
  assert.equal(customPeerName(null), null);
  assert.equal(customPeerName(undefined), null);
  assert.equal(customPeerName(''), null);
  assert.equal(customPeerName('   '), null);
  assert.equal(customPeerName(42), null);
});

test('room guest seat: the host sees the guest\'s chosen name', () => {
  const label = peerSeatLabel('GuestPlayer', {
    fallback: tIn('en', 'room.guestSeat'),
    named: tIn('en', 'room.guestSeatNamed'),
  });
  assert.equal(label, 'GuestPlayer — Red');
});

test('room guest seat: no chosen name keeps the classic label', () => {
  for (const unset of [null, undefined, '', 'Guest']) {
    assert.equal(peerSeatLabel(unset, {
      fallback: tIn('en', 'room.guestSeat'),
      named: tIn('en', 'room.guestSeatNamed'),
    }), 'Guest — Red', `peerName=${String(unset)}`);
  }
});

test('loading guest seat: the host sees the guest\'s chosen name', () => {
  assert.equal(peerSeatLabel('GuestPlayer', {
    fallback: tIn('en', 'load.redGuest'),
    named: tIn('en', 'load.redNamed'),
  }), 'Red — GuestPlayer');
  assert.equal(peerSeatLabel('Guest', {
    fallback: tIn('en', 'load.redGuest'),
    named: tIn('en', 'load.redNamed'),
  }), 'Red — Guest');
});

test('host seat is symmetric: the guest sees the host\'s chosen name', () => {
  assert.equal(peerSeatLabel('HostPlayer', {
    fallback: tIn('en', 'room.hostSeat'),
    named: tIn('en', 'room.hostSeatNamed'),
  }), 'HostPlayer — Blue');
  assert.equal(peerSeatLabel('HostPlayer', {
    fallback: tIn('en', 'load.blueHost'),
    named: tIn('en', 'load.blueNamed'),
  }), 'Blue — HostPlayer');
  assert.equal(peerSeatLabel(null, {
    fallback: tIn('en', 'room.hostSeat'),
    named: tIn('en', 'room.hostSeatNamed'),
  }), 'Host — Blue');
});

test('named seat labels localize (pt-BR / es), fallback stays localized', () => {
  assert.equal(peerSeatLabel('GuestPlayer', {
    fallback: tIn('pt-BR', 'room.guestSeat'),
    named: tIn('pt-BR', 'room.guestSeatNamed'),
  }), 'GuestPlayer — Vermelho');
  assert.equal(peerSeatLabel('GuestPlayer', {
    fallback: tIn('es', 'load.redGuest'),
    named: tIn('es', 'load.redNamed'),
  }), 'Rojo — GuestPlayer');
  // The unset sentinel must NOT leak the English word "Guest"
  // into a localized label.
  assert.equal(peerSeatLabel('Guest', {
    fallback: tIn('pt-BR', 'room.guestSeat'),
    named: tIn('pt-BR', 'room.guestSeatNamed'),
  }), 'Convidado — Vermelho');
});

test('a peer name is foreign input: it never reaches the label as HTML', () => {
  const label = peerSeatLabel('<b>X</b>', {
    fallback: tIn('en', 'room.guestSeat'),
    named: tIn('en', 'room.guestSeatNamed'),
  });
  assert.equal(label.includes('<b>'), false);
  assert.ok(label.includes('&lt;b&gt;'), label);
});

test('app.js renders both seat lines and both loading rows via peerSeatLabel', () => {
  const app = read('../app.js');
  const calls = app.split('peerSeatLabel(').length - 1;
  assert.ok(calls >= 4, `expected >= 4 peerSeatLabel call sites in app.js, got ${calls}`);
  for (const key of ['room.hostSeatNamed', 'room.guestSeatNamed', 'load.blueNamed', 'load.redNamed']) {
    assert.ok(app.includes(`t('${key}')`), `app.js never looks up ${key}`);
  }
});
