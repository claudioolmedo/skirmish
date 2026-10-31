// Siege Workshop queue-line regression pins (bot-loop, 2026-10-30).
//
// Defect (live session b57befa2 on the combo build 4965105):
// Castle P4a made the Siege Workshop (building kind 17) a producer
// (Train Battering Ram), but updateSelPanel's PROD_KINDS never
// learned kind 17, so its selection panel kept the plain 'building'
// shape: during the Ram's training the panel showed NO
// "Training… · Queue n" line and no cancel chips — the only
// feedback was a sweep on the button, so the player could not tell
// the click had worked (the same family as Claudio's Priority
// No. 1 complaint, and the same root cause as the Market fix in
// 5f64b27). TRAINS_TAG also had no 17 entry, so the housed block
// never read for the Workshop either.
//
// These pins keep the Siege Workshop in the production shape, with
// its housed block reading the Battering Ram train tag (entity
// tag 17).
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

test('the Siege Workshop (kind 17) is a production-shape selection', () => {
  const m = appJs.match(/const PROD_KINDS = \[([^\]]+)\];/);
  assert.ok(m, 'PROD_KINDS exists in app.js');
  const kinds = m[1].split(',').map((s) => Number(s.trim()));
  assert.ok(kinds.includes(17), 'PROD_KINDS includes the Siege Workshop kind 17');
});

test('the Siege Workshop housed block reads its Battering Ram train tag', () => {
  const m = appJs.match(/const TRAINS_TAG = \{([^}]+)\};/);
  assert.ok(m, 'TRAINS_TAG exists in app.js');
  assert.ok(/17:\s*17/.test(m[1]), 'TRAINS_TAG maps Siege Workshop 17 -> Battering Ram kind 17');
});
