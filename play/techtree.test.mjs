// INF-06 pins: the client's technology-tree table
// (play/techtree.js) is a mirror of the sim's own table
// (sim/src/tech.rs) — this test parses the Rust tables and
// fails if the mirror drifts: every variant, its tag, host,
// required Age, prerequisite, research ticks and cost must
// agree. The state helper's three states are pinned against
// the chain cases (Wheelbarrow → Hand Cart, Horse Collar →
// Heavy Plow → Crop Rotation, Coinage → Banking) and the
// age-advancement convention (advancements read off the age,
// never a mask bit).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { TECH_TREE, techDone, techState, techTreeColumns } from './techtree.js';

const rust = readFileSync(new URL('../../sim/src/tech.rs', import.meta.url), 'utf8');

function section(startMarker, endMarker) {
  const a = rust.indexOf(startMarker);
  assert.ok(a >= 0, `section start ${startMarker} found`);
  const b = endMarker ? rust.indexOf(endMarker, a) : rust.length;
  assert.ok(b > a, `section end ${endMarker} found`);
  return rust.slice(a, b);
}

/** Parse `TechId::A | TechId::B => value` arms into a
 * variant → value map (single-variant arms included). */
function arms(sec, valuePattern) {
  const out = new Map();
  // Normalize: strip comments, collapse whitespace, and unwrap
  // the block-form arms (`=> { TechHost::... }`) onto one line,
  // so every arm reads `TechId::A | TechId::B => value,`.
  const flat = sec
    .replace(/\/\/[^\n]*/g, '')
    .replace(/\s+/g, ' ')
    // Block arms carry no trailing comma in Rust — supply one,
    // or the value reader would swallow the next arm's header.
    .replace(/=> \{ ([^}]*) \}/g, '=> $1,');
  const re = /((?:TechId::\w+\s*(?:\|\s*)?)+)=>\s*([^,]+),/g;
  let m;
  while ((m = re.exec(flat)) !== null) {
    const value = m[2].trim();
    const vm = value.match(valuePattern);
    if (!vm) continue;
    const captured = vm[1] !== undefined ? vm[1] : (vm[2] !== undefined ? vm[2] : vm[0]);
    for (const name of m[1].matchAll(/TechId::(\w+)/g)) {
      out.set(name[1], captured);
    }
  }
  return out;
}

const variants = [...section('pub enum TechId {', '}').matchAll(/^\s{4}(\w+),/gm)]
  .map((m) => m[1]);
assert.ok(variants.length >= 31, `enum variants parsed (${variants.length})`);

const tags = arms(section('pub fn tag(', 'pub fn from_tag'), /^(\d+)$/);
const hosts = arms(
  section('pub fn host(', 'pub fn age_required'),
  /TechHost::(?:Building\(BuildingKind::(\w+)\)|(TownCenter))/,
);
const ages = arms(section('pub fn age_required(', 'pub fn requires_tech'), /Age::(\w+)/);
const requires = arms(section('pub fn requires_tech(', 'pub fn ticks'), /Some\(TechId::(\w+)\)/);
const ticks = arms(section('pub fn ticks(', 'pub fn cost'), /^(\d+)$/);
// Costs parse on their own regex: the tuple's inner commas
// defeat the generic arm reader.
const costs = new Map();
{
  const flat = section('pub fn cost(', 'pub fn tribute_fee_pct')
    .replace(/\/\/[^\n]*/g, '')
    .replace(/\s+/g, ' ');
  for (const m of flat.matchAll(/TechId::(\w+) => \((\d+), (\d+), (\d+), (\d+)\)/g)) {
    costs.set(m[1], `${m[2]}, ${m[3]}, ${m[4]}, ${m[5]}`);
  }
}

const KIND_TAGS = {
  House: 0, Farm: 1, Barracks: 2, LumberCamp: 3, Mill: 4, MiningCamp: 5,
  ArcheryRange: 6, Stable: 7, Blacksmith: 8, PalisadeWall: 9, WatchTower: 10,
  Outpost: 11, Market: 12, TownCenter: 13, StoneWall: 14, Castle: 15,
  University: 16, SiegeWorkshop: 17, PalisadeGate: 18, StoneGate: 19,
  Monastery: 20,
};
const AGE_TAGS = { Dark: 0, Feudal: 1, Castle: 2, Imperial: 3 };

test('the tree mirrors sim/src/tech.rs exactly', () => {
  assert.equal(TECH_TREE.length, variants.length, 'one entry per TechId variant');
  const byTag = new Map(TECH_TREE.map((e) => [e.tag, e]));
  for (const name of variants) {
    const tag = Number(tags.get(name));
    const entry = byTag.get(tag);
    assert.ok(entry, `${name} (tag ${tag}) present in TECH_TREE`);
    // Host: BuildingKind names map through KIND_TAGS;
    // TechHost::TownCenter captures as the literal 'TownCenter'.
    const hostName = hosts.get(name);
    const hostTag = hostName === 'TownCenter' ? 13 : KIND_TAGS[hostName];
    assert.equal(entry.host, hostTag, `${name} host`);
    if (ages.has(name)) {
      assert.equal(entry.age, AGE_TAGS[ages.get(name)], `${name} age`);
    } else {
      assert.equal(entry.age, 1, `${name} age (the Feudal default arm)`);
    }
    const reqName = requires.get(name);
    assert.equal(
      entry.requires,
      reqName ? Number(tags.get(reqName)) : null,
      `${name} prerequisite`,
    );
    assert.equal(entry.secs * 60, Number(ticks.get(name)), `${name} ticks`);
    const cost = costs.get(name);
    assert.ok(cost, `${name} cost parsed`);
    assert.deepEqual(
      entry.cost,
      cost.split(', ').map(Number),
      `${name} cost`,
    );
  }
});

test('the columns cover every tag exactly once', () => {
  const seen = [];
  for (const col of techTreeColumns()) {
    for (const group of col) {
      for (const e of group.entries) seen.push(e.tag);
    }
  }
  assert.deepEqual(
    seen.slice().sort((a, b) => a - b),
    TECH_TREE.map((e) => e.tag).sort((a, b) => a - b),
  );
  assert.equal(new Set(seen).size, seen.length, 'no tag twice');
});

test('techState walks the chains and the age gates', () => {
  const byTag = new Map(TECH_TREE.map((e) => [e.tag, e]));
  const HC = 1 << 13;
  const HP = 1 << 30;
  // Horse Collar at Feudal with nothing: available; Heavy Plow
  // locked behind it (and behind Castle); Crop Rotation
  // locked behind both.
  assert.equal(techState(byTag.get(13), 0, 1), 'available');
  assert.equal(techState(byTag.get(30), 0, 1), 'locked');
  assert.equal(techState(byTag.get(30), HC, 1), 'locked', 'age still gates');
  assert.equal(techState(byTag.get(30), HC, 2), 'available');
  assert.equal(techState(byTag.get(31), HC | HP, 2), 'locked', 'Imperial gates');
  assert.equal(techState(byTag.get(31), HC | HP, 3), 'available');
  assert.equal(techState(byTag.get(31), HC | HP | (1 << 31), 3), 'done');
  // Advancements read off the age, never a mask bit.
  assert.equal(techDone(1, 0, 0), false);
  assert.equal(techDone(1, 0, 1), true);
  assert.equal(techDone(14, 0, 1), false);
  assert.equal(techDone(18, 0, 3), true);
  assert.equal(techState(byTag.get(14), 0, 1), 'available', 'Castle Age researchable in Feudal');
  assert.equal(techState(byTag.get(14), 0, 2), 'done');
  // Coinage → Banking across the Imperial gate.
  assert.equal(techState(byTag.get(26), 0, 3), 'locked');
  assert.equal(techState(byTag.get(26), 1 << 25, 3), 'available');
});
