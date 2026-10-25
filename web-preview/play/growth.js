// Skirmish II — the player name (2026-10-24): a display string
// the player types once in the lobby, stored on this device in
// localStorage, riding the match handshake so both seats can
// show it. Pure module in the house pattern (no DOM; storage is
// injected so the tests can pin everything).
//
// (The challenge chain and the Instagram Login that shipped
// alongside this module were removed the same day at Claudio's
// direction — his challenge design lives on Instagram, not in
// the game, and the login is deferred. Both are preserved in
// git history at b7c7543 if they are ever wanted back.)

export const NAME_STORE_KEY = 'skirmish2.playerName';
export const GUEST_NAME = 'Guest';
export const NAME_MAX = 32;

/// The one name cleaner (strip control characters, trim, cap).
/// Returns null when nothing usable remains — callers fall
/// back instead of showing a blank name.
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

/// The display name for this player: the name they typed once,
/// else "Guest". Never empty — a guest is a first-class player.
export function displayName({ stored = null } = {}) {
  return cleanDisplayName(stored) || GUEST_NAME;
}
