// Unit tests for room.js (M10) — run from web-preview/play/:
//   node --test room.test.mjs
// The room model's gates: password check (host side), admission
// (both views), the ready toggle, and the both-ready start rule.

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  checkPassword,
  peerAdmitted,
  guestAdmitted,
  canToggleReady,
  startGate,
} from './room.js';

test('checkPassword accepts only the exact password', () => {
  assert.equal(checkPassword('swordfish', 'swordfish'), true);
  assert.equal(checkPassword('swordfish', 'Swordfish'), false);
  assert.equal(checkPassword('swordfish', 'swordfish '), false);
  assert.equal(checkPassword('swordfish', ''), false);
  assert.equal(checkPassword('swordfish', null), false);
  assert.equal(checkPassword('swordfish', 42), false);
});

test('checkPassword never matches an unset password', () => {
  // A public room has no password; the check is simply never the
  // gate there. An empty/missing expectation must not open a
  // private room by accident.
  assert.equal(checkPassword('', ''), false);
  assert.equal(checkPassword(null, ''), false);
  assert.equal(checkPassword(undefined, 'x'), false);
});

test('peerAdmitted: presence is admission in public, pw-ok in private', () => {
  assert.equal(peerAdmitted({ privacy: 'public', peerPwOk: false }), true);
  assert.equal(peerAdmitted({ privacy: 'private', peerPwOk: false }), false);
  assert.equal(peerAdmitted({ privacy: 'private', peerPwOk: true }), true);
});

test('guestAdmitted: only pwState ok opens a private room', () => {
  assert.equal(guestAdmitted({ isPrivate: false, pwState: 'na' }), true);
  assert.equal(guestAdmitted({ isPrivate: true, pwState: 'needed' }), false);
  assert.equal(guestAdmitted({ isPrivate: true, pwState: 'sent' }), false);
  assert.equal(guestAdmitted({ isPrivate: true, pwState: 'wrong' }), false);
  assert.equal(guestAdmitted({ isPrivate: true, pwState: 'ok' }), true);
});

test('canToggleReady needs the lobby phase, a present peer, admission', () => {
  assert.equal(canToggleReady({ phase: 'lobby', peerPresent: true, admitted: true }), true);
  assert.equal(canToggleReady({ phase: 'lobby', peerPresent: false, admitted: true }), false);
  assert.equal(canToggleReady({ phase: 'lobby', peerPresent: true, admitted: false }), false);
  assert.equal(canToggleReady({ phase: 'starting', peerPresent: true, admitted: true }), false);
  assert.equal(canToggleReady({ phase: 'playing', peerPresent: true, admitted: true }), false);
});

test('startGate: the host starts automatically when BOTH are ready', () => {
  const base = {
    role: 'host',
    phase: 'lobby',
    peerPresent: true,
    admitted: true,
    selfReady: true,
    peerReady: true,
  };
  assert.equal(startGate(base), true);
  assert.equal(startGate({ ...base, peerReady: false }), false);
  assert.equal(startGate({ ...base, selfReady: false }), false);
  // The guest never initiates, even seeing both flags.
  assert.equal(startGate({ ...base, role: 'guest' }), false);
  // An unadmitted peer (private room, no pw-ok) blocks the start
  // even with both ready flags somehow up.
  assert.equal(startGate({ ...base, admitted: false }), false);
  assert.equal(startGate({ ...base, peerPresent: false }), false);
  assert.equal(startGate({ ...base, phase: 'starting' }), false);
});
