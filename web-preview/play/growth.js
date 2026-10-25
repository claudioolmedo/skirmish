// Skirmish II — growth package (2026-10-24, Claudio's design):
// the player name, the challenge chain's client half, and the
// Instagram Login client half. Pure module in the house pattern
// (no DOM; storage is injected so the tests can pin everything).
//
// The governing rule is Claudio's: SUPER EASY. Playing never
// requires an account, a login or a form. The name exists so a
// challenge can say who challenges whom; the Instagram sign-in
// is OPTIONAL — its one job is that a player who chooses it
// never types a name at all: their in-game name becomes their
// Instagram username, automatically.
//
// The name the game shows rides three sources, in this order:
//   1. the Instagram identity (when the player signed in),
//   2. the name the player typed once (localStorage),
//   3. "Guest".
// A guest is a first-class player: a challenge link opens
// straight into the match under the name "Guest" until the
// player gives themselves a better one.

// --- served config ---------------------------------------------------
// Instagram Login ships DISABLED: until a Meta app exists and
// its credentials sit in the relay's env, IG_LOGIN_ENABLED stays
// false and NO sign-in button ever renders — a dead button is
// worse than none. Flipping this flag (and setting IG_APP_ID)
// is the whole enable step on the client; the relay answers
// 501 while its own secrets are unset, so the two sides can
// never half-enable.
export const IG_LOGIN_ENABLED = false;
export const IG_APP_ID = '';
export const IG_REDIRECT_URI = 'https://skirmish.online/auth/instagram/callback';
export const IG_SCOPE = 'instagram_business_basic';

export const NAME_STORE_KEY = 'skirmish2.playerName';
export const IG_STORE_KEY = 'skirmish2.igIdentity';
export const GUEST_NAME = 'Guest';
export const NAME_MAX = 32;

/// The one name cleaner, mirroring the relay's (strip control
/// characters, trim, cap). Returns null when nothing usable
/// remains — callers fall back instead of showing a blank name.
export function cleanDisplayName(value) {
  if (typeof value !== 'string') return null;
  const name = value
    .replace(/[\u0000-\u001F\u007F]/g, '')
    .trim()
    .slice(0, NAME_MAX);
  return name || null;
}

// --- stored name ------------------------------------------------------

export function readStoredName(storage) {
  try {
    return cleanDisplayName(storage.getItem(NAME_STORE_KEY));
  } catch {
    return null;
  }
}

/// Persist the typed name (cleaned). An empty/garbage value
/// clears the stored name instead of saving a blank.
export function writeStoredName(storage, value) {
  const name = cleanDisplayName(value);
  try {
    if (name) storage.setItem(NAME_STORE_KEY, name);
    else storage.removeItem(NAME_STORE_KEY);
  } catch { /* private mode — the name just does not persist */ }
  return name;
}

// --- Instagram identity -------------------------------------------------

/// The stored identity is exactly what the relay's exchange
/// answered: { id, username }. Anything off-shape reads as
/// "not signed in" — a corrupt entry must never break the name.
export function readIgIdentity(storage) {
  try {
    const raw = storage.getItem(IG_STORE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object') return null;
    const id = typeof parsed.id === 'string' && parsed.id
      ? parsed.id.slice(0, 32) : null;
    const username = cleanDisplayName(parsed.username);
    if (!id || !username) return null;
    return { id, username };
  } catch {
    return null;
  }
}

export function writeIgIdentity(storage, profile) {
  const id = profile && typeof profile.id === 'string' && profile.id
    ? profile.id.slice(0, 32) : null;
  const username = profile ? cleanDisplayName(profile.username) : null;
  if (!id || !username) return null;
  const identity = { id, username };
  try {
    storage.setItem(IG_STORE_KEY, JSON.stringify(identity));
    // The identity IS the name now — keep the typed-name slot in
    // step, so every reader (the form's prefill, an older build)
    // shows the same player.
    storage.setItem(NAME_STORE_KEY, username);
  } catch { /* private mode — sign-in lasts the session only */ }
  return identity;
}

/// The display name for { ig, stored } sources, in the order
/// the header comment promises. Never empty: a guest is "Guest".
export function displayName({ ig = null, stored = null } = {}) {
  if (ig && cleanDisplayName(ig.username)) return cleanDisplayName(ig.username);
  return cleanDisplayName(stored) || GUEST_NAME;
}

// --- Instagram Login -----------------------------------------------------

/// The official Instagram Login authorize URL. The authorize
/// step happens on Instagram's own page — the password never
/// comes near this site; Meta returns a one-time code to
/// IG_REDIRECT_URI, which the relay exchanges server-side.
export function buildIgAuthorizeUrl({ appId = IG_APP_ID, redirectUri = IG_REDIRECT_URI } = {}) {
  if (!appId) return null;
  const q = new URLSearchParams({
    client_id: appId,
    redirect_uri: redirectUri,
    scope: IG_SCOPE,
    response_type: 'code',
  });
  return `https://www.instagram.com/oauth/authorize?${q.toString()}`;
}

/// Whether the "Continue with Instagram" button may render:
/// the flag AND a configured app id. (The relay independently
/// refuses with 501 while its secrets are unset.)
export function igLoginAvailable({ enabled = IG_LOGIN_ENABLED, appId = IG_APP_ID } = {}) {
  return !!(enabled && appId);
}

// --- the challenge chain ---------------------------------------------------

const ROOM_PATTERN = /^[A-Za-z0-9_-]{1,64}$/;

/// The join report both clients send when a challenge match
/// begins: { room, challenger, challenged } or null when the
/// pieces are not usable (the chain is a courtesy — a missing
/// record never blocks a match).
export function challengeJoinPayload({ room, challenger, challenged } = {}) {
  if (typeof room !== 'string' || !ROOM_PATTERN.test(room)) return null;
  const from = cleanDisplayName(challenger);
  const to = cleanDisplayName(challenged);
  if (!from || !to) return null;
  return { room, challenger: from, challenged: to };
}

/// The result report, from ONE client's seat: `selfRole` is this
/// player's place in the chain ('challenger' for the host of a
/// challenge room, 'challenged' for the guest who took the
/// link), `selfWon` whether this player won. A desync/abandoned
/// match reports nothing — callers pass selfWon === null and
/// get null back.
export function challengeResultPayload({ room, selfRole, selfName, peerName, selfWon } = {}) {
  if (typeof room !== 'string' || !ROOM_PATTERN.test(room)) return null;
  if (selfRole !== 'challenger' && selfRole !== 'challenged') return null;
  if (selfWon !== true && selfWon !== false) return null;
  const winner = selfWon
    ? selfRole
    : (selfRole === 'challenger' ? 'challenged' : 'challenger');
  const winnerName = cleanDisplayName(selfWon ? selfName : peerName);
  if (!winnerName) return null;
  return { room, winner, winnerName };
}
