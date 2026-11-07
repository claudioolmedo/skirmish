// Build-menu icon integrity regression pins (bot-loop, 2026-11-07).
//
// Defect (found live 2026-11-06, confirmed in a directed session):
// the Economic build page's fourth row rendered two broken <img>s —
// V = Wonder (completely blank, no tooltip image) and N = Dock (only
// a fragment at the top). Root cause: BUILD_PNG in app.js names
// 'wonder.png', 'dock.png' and 'fish-trap.png', but those files were
// never added to play/ui-art/icons/ when their buildings landed
// (Wonder VIC-02, Dock Water W1, Fish Trap Water W2b), so pngIcon()
// emitted <img> tags for files that 404. The buttons still worked —
// a player just could not see what they were, the same confusion
// family as Claudio's "menus didn't work" report.
//
// Fix: the three icons now exist as original programmatic
// pictograms in the Castle precedent's style (see
// tools/art/gen-building-icons.py and ui-art/README.md).
//
// These pins keep the whole class closed: EVERY file named by
// BUILD_PNG or KIND_PNG (the two tables pngIcon() serves) must
// exist in ui-art/icons/ and be a decodable 128x128 PNG.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const iconsDir = join(here, 'ui-art', 'icons');

function pngNames(re, label) {
  const m = appJs.match(re);
  assert.ok(m, `${label} table found in app.js`);
  const names = [...m[1].matchAll(/'([^']+\.png)'/g)].map((x) => x[1]);
  assert.ok(names.length > 0, `${label} names at least one PNG`);
  return names;
}

const BUILD_PNG = pngNames(/const BUILD_PNG = \[([\s\S]*?)\];/, 'BUILD_PNG');
const KIND_PNG = pngNames(/const KIND_PNG = \{([\s\S]*?)\};/, 'KIND_PNG');

test('every BUILD_PNG / KIND_PNG icon file exists', () => {
  for (const name of [...BUILD_PNG, ...KIND_PNG]) {
    assert.ok(existsSync(join(iconsDir, name)),
      `ui-art/icons/${name} exists (a missing file renders a broken button)`);
  }
});

test('the three late-building icons are present by name', () => {
  // The exact files whose absence this regression closes.
  for (const name of ['wonder.png', 'dock.png', 'fish-trap.png']) {
    assert.ok(BUILD_PNG.includes(name), `BUILD_PNG still names ${name}`);
    assert.ok(existsSync(join(iconsDir, name)), `ui-art/icons/${name} exists`);
  }
});

test('every named icon is a 128x128 PNG', () => {
  for (const name of [...BUILD_PNG, ...KIND_PNG]) {
    const buf = readFileSync(join(iconsDir, name));
    assert.equal(buf.toString('ascii', 1, 4), 'PNG', `${name} is a PNG`);
    assert.equal(buf.readUInt32BE(16), 128, `${name} is 128 wide`);
    assert.equal(buf.readUInt32BE(20), 128, `${name} is 128 high`);
  }
});
