// The technology tree (INF-06): the whole research graph as
// data — every technology the sim knows, with its host building,
// the Age it requires, its cost, its research time and its one
// prerequisite — plus the pure state helper the view paints
// from. This module is a MIRROR of sim/src/tech.rs (the tags,
// hosts, ages, costs, ticks and chain links are the sim's own
// table; techtree.test.mjs pins this file against the Rust
// source so the two can never drift silently). Display names do
// NOT live here: app.js resolves them through its TECH_NAME
// table (i18n), keyed by tag, exactly like the panels do.
//
// Hosts are BuildingKind tags (sim/src/build.rs): TownCenter 13,
// Mill 4, Barracks 2, Lumber Camp 3, Mining Camp 5, Stable 7,
// Blacksmith 8, Market 12, University 16. Costs are
// [food, wood, gold, stone]; secs is the research time in
// seconds (the sim's ticks / 60).

export const HOST_TC = 13;

/** One technology: { tag, host, age, cost, secs, requires }. */
export const TECH_TREE = [
  // --- Town Center -------------------------------------------------
  { tag: 0, host: 13, age: 0, cost: [0, 0, 50, 0], secs: 25, requires: null }, // Loom
  { tag: 1, host: 13, age: 0, cost: [500, 0, 0, 0], secs: 130, requires: null }, // Feudal Age
  { tag: 6, host: 13, age: 1, cost: [175, 50, 0, 0], secs: 75, requires: null }, // Wheelbarrow
  { tag: 14, host: 13, age: 1, cost: [800, 0, 200, 0], secs: 160, requires: null }, // Castle Age
  { tag: 15, host: 13, age: 2, cost: [300, 200, 0, 0], secs: 55, requires: 6 }, // Hand Cart
  { tag: 18, host: 13, age: 2, cost: [1000, 0, 800, 0], secs: 190, requires: null }, // Imperial Age
  { tag: 27, host: 13, age: 1, cost: [75, 0, 0, 0], secs: 25, requires: null }, // Town Watch
  { tag: 28, host: 13, age: 2, cost: [300, 0, 100, 0], secs: 40, requires: 27 }, // Town Patrol
  // --- Blacksmith --------------------------------------------------
  { tag: 2, host: 8, age: 1, cost: [100, 0, 50, 0], secs: 30, requires: null }, // Fletching
  { tag: 3, host: 8, age: 1, cost: [100, 0, 0, 0], secs: 40, requires: null }, // Scale Mail Armor
  { tag: 4, host: 8, age: 1, cost: [100, 0, 0, 0], secs: 40, requires: null }, // Padded Archer Armor
  { tag: 7, host: 8, age: 1, cost: [150, 0, 0, 0], secs: 50, requires: null }, // Forging
  { tag: 8, host: 8, age: 1, cost: [150, 0, 0, 0], secs: 45, requires: null }, // Scale Barding Armor
  // --- Barracks ----------------------------------------------------
  { tag: 5, host: 2, age: 1, cost: [100, 0, 40, 0], secs: 40, requires: null }, // Man-at-Arms
  // --- Stable ------------------------------------------------------
  { tag: 9, host: 7, age: 1, cost: [150, 0, 100, 0], secs: 50, requires: null }, // Bloodlines
  { tag: 16, host: 7, age: 2, cost: [150, 0, 50, 0], secs: 45, requires: null }, // Light Cavalry
  { tag: 17, host: 7, age: 2, cost: [150, 0, 0, 0], secs: 40, requires: null }, // Husbandry
  // --- Lumber Camp -------------------------------------------------
  { tag: 10, host: 3, age: 1, cost: [100, 50, 0, 0], secs: 25, requires: null }, // Double-Bit Axe
  // --- Mining Camp -------------------------------------------------
  { tag: 11, host: 5, age: 1, cost: [100, 75, 0, 0], secs: 30, requires: null }, // Gold Mining
  { tag: 12, host: 5, age: 1, cost: [100, 75, 0, 0], secs: 30, requires: null }, // Stone Mining
  // --- Mill (the farm chain, ECO-14) -------------------------------
  { tag: 13, host: 4, age: 1, cost: [75, 75, 0, 0], secs: 20, requires: null }, // Horse Collar
  { tag: 30, host: 4, age: 2, cost: [125, 125, 0, 0], secs: 40, requires: 13 }, // Heavy Plow
  { tag: 31, host: 4, age: 3, cost: [250, 250, 0, 0], secs: 70, requires: 30 }, // Crop Rotation
  // --- Market ------------------------------------------------------
  { tag: 19, host: 12, age: 2, cost: [200, 0, 200, 0], secs: 40, requires: null }, // Caravan
  { tag: 25, host: 12, age: 2, cost: [200, 0, 100, 0], secs: 70, requires: null }, // Coinage
  { tag: 26, host: 12, age: 3, cost: [300, 0, 200, 0], secs: 70, requires: 25 }, // Banking
  // --- University --------------------------------------------------
  { tag: 20, host: 16, age: 2, cost: [0, 300, 175, 0], secs: 60, requires: null }, // Ballistics
  { tag: 21, host: 16, age: 2, cost: [200, 0, 0, 100], secs: 35, requires: null }, // Murder Holes
  { tag: 22, host: 16, age: 2, cost: [0, 200, 0, 50], secs: 20, requires: null }, // Treadmill Crane
  { tag: 23, host: 16, age: 2, cost: [150, 175, 0, 0], secs: 50, requires: null }, // Masonry
  { tag: 24, host: 16, age: 2, cost: [200, 100, 0, 0], secs: 50, requires: null }, // Fortified Wall
  // --- Castle --------------------------------------------------------
  { tag: 29, host: 15, age: 3, cost: [150, 0, 150, 0], secs: 60, requires: null }, // Conscription
  // --- Monastery -----------------------------------------------------
  { tag: 32, host: 20, age: 2, cost: [0, 0, 175, 0], secs: 60, requires: null }, // Sanctity
  { tag: 33, host: 20, age: 2, cost: [0, 0, 140, 0], secs: 50, requires: null }, // Fervor
  { tag: 34, host: 20, age: 2, cost: [0, 0, 200, 0], secs: 35, requires: null }, // Herbal Medicine
  { tag: 35, host: 20, age: 2, cost: [0, 0, 325, 0], secs: 40, requires: null }, // Atonement (Castle P5b)
  { tag: 36, host: 20, age: 2, cost: [100, 0, 200, 0], secs: 40, requires: null }, // Devotion (Castle P5b)
  { tag: 37, host: 20, age: 2, cost: [0, 0, 475, 0], secs: 50, requires: null }, // Redemption (Castle P5c)
  { tag: 38, host: 20, age: 2, cost: [0, 0, 1000, 0], secs: 60, requires: null }, // Heresy (Castle P5c)
];

/** The age-advancement tags and the age each one reaches. */
export const ADVANCEMENTS = { 1: 1, 14: 2, 18: 3 };

/**
 * Whether a technology counts as researched for a player whose
 * researched-mask is `mask` (TechId bits, the sim's u64 read as
 * a JS number) and whose current age tag is `age`. The three
 * advancements are recorded by the age itself, never by a mask
 * bit (the sim's own convention).
 *
 * The bit test goes through BigInt, NOT `mask & (1 << tag)`
 * (bot-loop, 2026-11-01): tags reached 32 (Sanctity) with
 * Castle P5a and 34 (Herbal Medicine) with P5e, the wasm mask
 * now crosses as a full f64, and a 32-bit shift wraps — tag 33
 * aliased onto the Feudal bit and read "researched" from the
 * Feudal Age on.
 */
export function techDone(tag, mask, age) {
  if (tag in ADVANCEMENTS) return age >= ADVANCEMENTS[tag];
  return ((BigInt(Math.round(mask)) >> BigInt(tag)) & 1n) === 1n;
}

/**
 * The tree state of one technology for that player:
 * 'done' (researched), 'available' (its Age is reached and its
 * prerequisite, if any, is done — research could start now,
 * funds aside), or 'locked' (an unmet prerequisite: the Age or
 * the chain link before it).
 */
export function techState(entry, mask, age) {
  if (techDone(entry.tag, mask, age)) return 'done';
  if (age < entry.age) return 'locked';
  if (entry.requires != null && !techDone(entry.requires, mask, age)) return 'locked';
  return 'available';
}

/** The tree grouped for display: age columns 0..3, each a list
 * of { host, entries } in host-tag order, entries in tag order. */
export function techTreeColumns() {
  const columns = [];
  for (let age = 0; age <= 3; age++) {
    const byHost = new Map();
    for (const entry of TECH_TREE) {
      if (entry.age !== age) continue;
      if (!byHost.has(entry.host)) byHost.set(entry.host, []);
      byHost.get(entry.host).push(entry);
    }
    columns.push(
      [...byHost.entries()]
        .sort((a, b) => a[0] - b[0])
        .map(([host, entries]) => ({ host, entries })),
    );
  }
  return columns;
}
