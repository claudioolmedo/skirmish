// Growth package unit tests (2026-10-24): the player name
// sources, the Instagram authorize URL + availability flag, and
// the challenge-chain payloads. Pure — storage is a Map-backed
// stub in the localStorage shape.
// Run from web-preview/: node --test play/growth.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  IG_LOGIN_ENABLED, IG_APP_ID, IG_REDIRECT_URI,
  NAME_STORE_KEY, IG_STORE_KEY, GUEST_NAME,
  cleanDisplayName, readStoredName, writeStoredName,
  readIgIdentity, writeIgIdentity, displayName,
  buildIgAuthorizeUrl, igLoginAvailable,
  challengeJoinPayload, challengeResultPayload,
} from './growth.js';

function stubStorage(seed = {}) {
  const map = new Map(Object.entries(seed));
  return {
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => map.set(k, String(v)),
    removeItem: (k) => map.delete(k),
    _map: map,
  };
}

test('cleanDisplayName strips controls, trims, caps, rejects blanks', () => {
  assert.equal(cleanDisplayName('  Ana  '), 'Ana');
  assert.equal(cleanDisplayName('A\u0007na'), 'Ana');
  assert.equal(cleanDisplayName('x'.repeat(40)).length, 32);
  assert.equal(cleanDisplayName('   '), null);
  assert.equal(cleanDisplayName(''), null);
  assert.equal(cleanDisplayName(42), null);
  assert.equal(cleanDisplayName(null), null);
});

test('the stored name round-trips cleaned; garbage clears it', () => {
  const s = stubStorage();
  assert.equal(readStoredName(s), null);
  assert.equal(writeStoredName(s, '  Bob  '), 'Bob');
  assert.equal(s.getItem(NAME_STORE_KEY), 'Bob');
  assert.equal(readStoredName(s), 'Bob');
  assert.equal(writeStoredName(s, '   '), null);
  assert.equal(s.getItem(NAME_STORE_KEY), null);
  // A corrupt stored value reads as no name, never a crash.
  const bad = stubStorage({ [NAME_STORE_KEY]: 'x'.repeat(500) });
  assert.equal(readStoredName(bad).length, 32);
});

test('the Instagram identity round-trips and shadows the typed name', () => {
  const s = stubStorage();
  assert.equal(readIgIdentity(s), null);
  writeStoredName(s, 'TypedName');
  const ig = writeIgIdentity(s, { id: '178414', username: 'claudioolmedo_com' });
  assert.deepEqual(ig, { id: '178414', username: 'claudioolmedo_com' });
  assert.deepEqual(readIgIdentity(s), ig);
  // The typed-name slot is kept in step with the identity.
  assert.equal(readStoredName(s), 'claudioolmedo_com');
  // Off-shape identities never stick.
  assert.equal(writeIgIdentity(s, { id: '', username: 'x' }), null);
  assert.equal(writeIgIdentity(s, { username: 'x' }), null);
  const corrupt = stubStorage({ [IG_STORE_KEY]: '{not json' });
  assert.equal(readIgIdentity(corrupt), null);
  const weird = stubStorage({ [IG_STORE_KEY]: '{"id":42,"username":"x"}' });
  assert.equal(readIgIdentity(weird), null);
});

test('displayName: Instagram first, then the typed name, then Guest', () => {
  assert.equal(displayName({}), GUEST_NAME);
  assert.equal(displayName({ stored: 'Ana' }), 'Ana');
  assert.equal(
    displayName({ ig: { id: '1', username: 'ana.gg' }, stored: 'Ana' }),
    'ana.gg');
  assert.equal(displayName({ ig: { id: '1', username: ' ' }, stored: 'Ana' }), 'Ana');
});

test('Instagram Login ships disabled: no URL, no button', () => {
  assert.equal(IG_LOGIN_ENABLED, false);
  assert.equal(IG_APP_ID, '');
  assert.equal(igLoginAvailable(), false);
  assert.equal(igLoginAvailable({ enabled: true, appId: '' }), false);
  assert.equal(igLoginAvailable({ enabled: false, appId: 'x' }), false);
  assert.equal(igLoginAvailable({ enabled: true, appId: 'x' }), true);
  // With no app id there is no authorize URL to send anyone to.
  assert.equal(buildIgAuthorizeUrl(), null);
});

test('the authorize URL is the official Instagram Login shape', () => {
  const url = buildIgAuthorizeUrl({ appId: '123456' });
  const u = new URL(url);
  assert.equal(u.origin + u.pathname, 'https://www.instagram.com/oauth/authorize');
  assert.equal(u.searchParams.get('client_id'), '123456');
  assert.equal(u.searchParams.get('redirect_uri'), IG_REDIRECT_URI);
  assert.equal(IG_REDIRECT_URI, 'https://skirmish.online/auth/instagram/callback');
  assert.equal(u.searchParams.get('scope'), 'instagram_business_basic');
  assert.equal(u.searchParams.get('response_type'), 'code');
});

test('challengeJoinPayload validates and cleans both names', () => {
  assert.deepEqual(
    challengeJoinPayload({ room: 'abc123', challenger: ' Ana ', challenged: 'Bob' }),
    { room: 'abc123', challenger: 'Ana', challenged: 'Bob' });
  assert.equal(challengeJoinPayload({ room: 'bad room', challenger: 'A', challenged: 'B' }), null);
  assert.equal(challengeJoinPayload({ room: 'abc', challenger: '', challenged: 'B' }), null);
  assert.equal(challengeJoinPayload({ room: 'abc', challenger: 'A' }), null);
  assert.equal(challengeJoinPayload(), null);
});

test('challengeResultPayload maps a seat win/loss to the chain', () => {
  // The challenger (host) won, reported from the host's seat.
  assert.deepEqual(
    challengeResultPayload({
      room: 'r1', selfRole: 'challenger', selfName: 'Ana', peerName: 'Bob', selfWon: true,
    }),
    { room: 'r1', winner: 'challenger', winnerName: 'Ana' });
  // The same match from the challenged guest's seat: agrees.
  assert.deepEqual(
    challengeResultPayload({
      room: 'r1', selfRole: 'challenged', selfName: 'Bob', peerName: 'Ana', selfWon: false,
    }),
    { room: 'r1', winner: 'challenger', winnerName: 'Ana' });
  // A desync / abandoned match (selfWon null) reports nothing.
  assert.equal(challengeResultPayload({
    room: 'r1', selfRole: 'challenger', selfName: 'Ana', peerName: 'Bob', selfWon: null,
  }), null);
  assert.equal(challengeResultPayload({
    room: 'r1', selfRole: 'host', selfName: 'Ana', peerName: 'Bob', selfWon: true,
  }), null);
});
