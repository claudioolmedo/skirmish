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
  GUEST_SLOTS,
  isGuestSlot,
  guestSlotOf,
  slotIsAi,
  slotSealed,
  slotAiMode,
  slotLabel,
  roomConfigText,
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

// --- M12: the guest slot ------------------------------------------

test('guest slots: the four values, junk falls back to open', () => {
  assert.deepEqual(GUEST_SLOTS, ['open', 'ai-easy', 'ai-normal', 'closed']);
  for (const s of GUEST_SLOTS) assert.equal(isGuestSlot(s), true, s);
  assert.equal(isGuestSlot('ai-hard'), false);
  assert.equal(isGuestSlot(''), false);
  assert.equal(isGuestSlot(null), false);
  assert.equal(guestSlotOf('ai-easy'), 'ai-easy');
  assert.equal(guestSlotOf('closed'), 'closed');
  assert.equal(guestSlotOf('nonsense'), 'open');
  assert.equal(guestSlotOf(undefined), 'open');
});

test('slot predicates: AI seals the seat, closed seals it too', () => {
  assert.equal(slotIsAi('ai-easy'), true);
  assert.equal(slotIsAi('ai-normal'), true);
  assert.equal(slotIsAi('open'), false);
  assert.equal(slotIsAi('closed'), false);
  assert.equal(slotSealed('ai-easy'), true);
  assert.equal(slotSealed('ai-normal'), true);
  assert.equal(slotSealed('closed'), true);
  assert.equal(slotSealed('open'), false);
  assert.equal(slotAiMode('ai-easy'), 'easy');
  assert.equal(slotAiMode('ai-normal'), 'normal');
  assert.equal(slotAiMode('open'), null);
  assert.equal(slotAiMode('closed'), null);
  assert.equal(slotLabel('open'), 'Open');
  assert.equal(slotLabel('ai-easy'), 'AI · Easy');
  assert.equal(slotLabel('ai-normal'), 'AI · Normal');
  assert.equal(slotLabel('closed'), 'Closed');
});

test('canToggleReady: AI slot readies alone, closed never readies', () => {
  // No peer exists with an AI slot — the host may still ready.
  assert.equal(canToggleReady({
    phase: 'lobby', peerPresent: false, admitted: true, guestSlot: 'ai-normal',
  }), true);
  assert.equal(canToggleReady({
    phase: 'lobby', peerPresent: false, admitted: true, guestSlot: 'ai-easy',
  }), true);
  // A closed seat means no match at all.
  assert.equal(canToggleReady({
    phase: 'lobby', peerPresent: false, admitted: true, guestSlot: 'closed',
  }), false);
  // The phase gate still applies to AI slots.
  assert.equal(canToggleReady({
    phase: 'starting', peerPresent: false, admitted: true, guestSlot: 'ai-normal',
  }), false);
  // Open slot: unchanged (a peer is still required).
  assert.equal(canToggleReady({
    phase: 'lobby', peerPresent: false, admitted: true, guestSlot: 'open',
  }), false);
});

test('startGate: AI slot starts on the host ready alone', () => {
  const ai = {
    role: 'host',
    phase: 'lobby',
    peerPresent: false,
    admitted: true,
    selfReady: true,
    peerReady: false,
    guestSlot: 'ai-normal',
  };
  assert.equal(startGate(ai), true);
  assert.equal(startGate({ ...ai, selfReady: false }), false);
  assert.equal(startGate({ ...ai, phase: 'starting' }), false);
  assert.equal(startGate({ ...ai, role: 'guest' }), false);
  // Closed never starts, whatever the flags say.
  assert.equal(startGate({ ...ai, guestSlot: 'closed', peerReady: true }), false);
});

test('roomConfigText: the configuration summary line', () => {
  assert.equal(roomConfigText({ guestSlot: 'open' }),
    '1v1 · Arabia · Conquest · Guest: Open');
  assert.equal(roomConfigText({ guestSlot: 'ai-normal' }),
    '1v1 · Arabia · Conquest · Guest: AI · Normal');
  assert.equal(roomConfigText({ guestSlot: 'closed' }),
    '1v1 · Arabia · Conquest · Guest: Closed');
  // From the guest's own seat the last part reads "You".
  assert.equal(roomConfigText({ guestSlot: 'open', selfIsGuest: true }),
    '1v1 · Arabia · Conquest · Guest: You');
  assert.equal(roomConfigText({}), '1v1 · Arabia · Conquest · Guest: Open');
});
