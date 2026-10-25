// Player-name unit tests (2026-10-24) + the removal pins for
// the challenge chain and the Instagram Login (removed the
// same day at Claudio's direction: his challenge design lives
// on Instagram, not in the game, and the login is deferred —
// both are preserved in git history at b7c7543). Pure — storage
// is a Map-backed stub in the localStorage shape; the removal
// pins read the shipped sources.
// Run from web-preview/: node --test play/growth.test.mjs

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  NAME_STORE_KEY, GUEST_NAME,
  cleanDisplayName, readStoredName, writeStoredName, displayName,
} from './growth.js';
import * as growth from './growth.js';
import * as netplay from './netplay.js';

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

test('displayName: the typed name, else Guest', () => {
  assert.equal(displayName({}), GUEST_NAME);
  assert.equal(displayName({ stored: 'Ana' }), 'Ana');
  assert.equal(displayName({ stored: '  ' }), GUEST_NAME);
  assert.equal(displayName({ stored: null }), GUEST_NAME);
});

// --- removal pins: the challenge chain + Instagram Login are GONE ---

test('growth.js exports no challenge or Instagram surface anymore', () => {
  for (const gone of [
    'IG_LOGIN_ENABLED', 'IG_APP_ID', 'IG_REDIRECT_URI', 'IG_SCOPE',
    'IG_STORE_KEY', 'readIgIdentity', 'writeIgIdentity',
    'buildIgAuthorizeUrl', 'igLoginAvailable',
    'challengeJoinPayload', 'challengeResultPayload',
  ]) {
    assert.equal(gone in growth, false, `${gone} must stay removed`);
  }
  assert.equal('buildChallengeUrl' in netplay, false);
});

const read = (rel) => readFileSync(new URL(rel, import.meta.url), 'utf8');

test('no shipped source carries the challenge or Instagram surfaces', () => {
  const app = read('../app.js');
  assert.equal(/challenge/i.test(app), false, 'app.js still mentions the challenge');
  assert.equal(/instagram/i.test(app), false, 'app.js still mentions Instagram');
  const page = read('./index.html');
  for (const id of ['btn-challenge', 'end-challenge-btn', 'challenge-box',
    'btn-share-challenge', 'btn-ig-login', 'ig-line']) {
    assert.equal(page.includes(`id="${id}"`), false,
      `play/index.html still carries #${id}`);
  }
  const i18n = read('./i18n.js');
  assert.equal(/'challenge\.|'ig\.|'status\.challengeRoom'/.test(i18n), false,
    'i18n.js still carries challenge/ig strings');
  const css = read('./play.css');
  assert.equal(css.includes('challenge-share'), false,
    'play.css still carries the challenge share box style');
  const privacy = read('../privacy/index.html');
  assert.equal(/challenge/i.test(privacy), false,
    'the privacy page still carries a Challenges mention');
  assert.equal(/instagram/i.test(privacy), false,
    'the privacy page still carries an Instagram mention');
});
