// Skirmish II battle formations — standalone module for the
// play page (MOV-07).
//
// Pure logic only: no DOM access, no sim access. app.js owns
// the current formation choice through this module; the SIM
// owns slot assignment (formation::assign_slots) — the
// client only names the formation kind on each member's
// move order, exactly as it names a stance tag on a stance
// order. Unit-tested headlessly — see formation.test.mjs.
//
// The model is DE's (wiki, "Battle Formations"): Line, Box,
// Staggered, Flank. The formation is a standing choice for
// the selection, not an order of its own: picking one issues
// nothing; the NEXT ground move (or attack-move's issued
// leg is NOT included — plain moves only, the sim's scope)
// of 2+ selected units travels in formation, with the sim
// assigning destination slots by class layer. With no
// formation chosen (the default), ground moves keep the
// long-standing keep-your-offset shape.

/** The four formations, in panel order: wire tag + i18n keys. */
export const FORMATIONS = [
  { id: 'line', tag: 0, labelKey: 'cmd.formationLine', titleKey: 'cmd.formationLineTitle' },
  { id: 'box', tag: 1, labelKey: 'cmd.formationBox', titleKey: 'cmd.formationBoxTitle' },
  { id: 'staggered', tag: 2, labelKey: 'cmd.formationStaggered', titleKey: 'cmd.formationStaggeredTitle' },
  { id: 'flank', tag: 3, labelKey: 'cmd.formationFlank', titleKey: 'cmd.formationFlankTitle' },
];

/** Formation id for a wire tag, or null for an unknown tag. */
export function formationIdForTag(tag) {
  const f = FORMATIONS.find((x) => x.tag === tag);
  return f ? f.id : null;
}

/** Wire tag for a formation id, or null for an unknown id. */
export function formationTagForId(id) {
  const f = FORMATIONS.find((x) => x.id === id);
  return f ? f.tag : null;
}

/**
 * Create the formation-choice store. Holds at most one
 * active formation id (or null — no formation, the default
 * keep-offset group move). Choosing the active formation
 * again clears it (the stance-row toggle discipline: a
 * setting face is pressed while it stands, and pressing it
 * again is how the player turns the setting off).
 */
export function createFormationChoice() {
  let current = null;
  return {
    /** The active formation id, or null. */
    current() {
      return current;
    },
    /** The active formation's wire tag, or null. */
    tag() {
      return current == null ? null : formationTagForId(current);
    },
    /**
     * Choose formation `id` (must be one of FORMATIONS);
     * choosing the active one again clears the choice.
     * Returns the new current id (or null).
     */
    choose(id) {
      if (formationTagForId(id) == null) {
        throw new Error(`unknown formation: ${id}`);
      }
      current = current === id ? null : id;
      return current;
    },
    /** Forget the choice (a new match starts with none). */
    clear() {
      current = null;
    },
  };
}

/**
 * Does a ground move of `count` selected units travel in
 * formation? Formations are group shapes: a lone unit's
 * slot is its destination in every formation (the sim's
 * own rule), so the client only names a formation on
 * orders for 2+ units — a single-unit order stays a plain
 * move, byte for byte what it was before MOV-07.
 */
export function formationApplies(choice, count) {
  return choice != null && choice.tag() != null && count >= 2;
}
