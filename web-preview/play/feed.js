// Skirmish II — the event notification feed (2026-10-19 study
// fix, gap 3). Pure diff logic only: no DOM, no sim access.
// app.js snapshots the LOCAL player's observable state each
// frame (completed own buildings, living own units by kind,
// researched techs, age, owned sheep) and this module turns a
// previous/next snapshot pair into the feed's message lines.
//
// The feed is DE's top-left event log: it tells the player what
// happened while they were looking somewhere else. Own-player
// events only — everything diffed here is state the client
// legitimately holds for its own seat, so the feed is fog-fair
// by construction. Strings are DE's: "<Building> Built",
// "<Unit> Created", "<Tech> Research Complete", "Sheep Found"
// (all observed in the 2026-10-19 player study; the Feudal line
// is the study's "Feudal Age Research Complete").

// Research lines by snapshot key, in display order. Feudal is
// not here: it arrives as the age transition, not a tech flag.
const TECH_LINES = [
  ['loom', 'Loom'],
  ['fletching', 'Fletching'],
  ['scalemail', 'Scale Mail Armor'],
  ['padded', 'Padded Archer Armor'],
  ['maa', 'Man-at-Arms'],
  ['wheelbarrow', 'Wheelbarrow'],
  ['forging', 'Forging'],
  ['barding', 'Scale Barding Armor'],
  ['bloodlines', 'Bloodlines'],
  ['doublebit', 'Double-Bit Axe'],
  ['goldmining', 'Gold Mining'],
  ['stonemining', 'Stone Mining'],
  ['horsecollar', 'Horse Collar'],
];

// A single frame can legitimately complete several of a kind
// (parallel production buildings); cap the repeats per kind so
// one diff can never flood the feed.
const MAX_PER_KIND = 3;

/**
 * Diff two own-player snapshots into feed lines (newest batch
 * last within the returned order; app.js prepends each line, so
 * the LAST line of a batch ends up on top).
 *
 * Snapshot shape (built by app.js):
 *   {
 *     built: { [buildingId]: kind },   // completed, own
 *     units: { [kind]: count },        // living, own, TC excluded
 *     techs: { loom, fletching, scalemail, padded, maa, wheelbarrow,
 *              forging, barding, bloodlines, doublebit, goldmining,
 *              stonemining, horsecollar }: bool,
 *     age: 0 | 1,
 *     sheep: count,                    // living, own
 *   }
 * `names` supplies the display tables app.js mirrors from the
 * sim: { buildingName(kind), unitName(kind) }.
 * A null prev (match start) yields no lines — the opening state
 * is not news.
 */
/// The EN formatters — the module's default voice (i18n: app.js
/// passes localized formatters built from play/i18n.js; the tests
/// call with three arguments and keep this exact English).
const EN_FEED_FMT = {
  built: (name) => `${name} Built`,
  created: (name) => `${name} Created`,
  research: (name) => `${name} Research Complete`,
  feudal: 'Feudal Age Research Complete',
  sheep: 'Sheep Found',
};

export function diffFeedEvents(prev, next, names, fmt = EN_FEED_FMT) {
  if (!prev || !next) return [];
  const out = [];
  const prevBuilt = prev.built || {};
  for (const [id, kind] of Object.entries(next.built || {})) {
    if (!(id in prevBuilt)) out.push(fmt.built(names.buildingName(kind), kind));
  }
  const prevUnits = prev.units || {};
  for (const [kind, count] of Object.entries(next.units || {})) {
    const delta = count - (prevUnits[kind] || 0);
    for (let i = 0; i < Math.min(delta, MAX_PER_KIND); i++) {
      out.push(fmt.created(names.unitName(Number(kind)), Number(kind)));
    }
  }
  if ((next.age ?? 0) > (prev.age ?? 0)) out.push(fmt.feudal);
  for (const [key, label] of TECH_LINES) {
    if (next.techs && next.techs[key] && !(prev.techs && prev.techs[key])) {
      out.push(fmt.research(names.techName ? names.techName(key) : label));
    }
  }
  const sheepDelta = (next.sheep || 0) - (prev.sheep || 0);
  for (let i = 0; i < Math.min(sheepDelta, MAX_PER_KIND); i++) {
    out.push(fmt.sheep);
  }
  return out;
}
