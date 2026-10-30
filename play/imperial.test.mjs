// Imperial Age rung pins (bot-loop TEC-01 package, 2026-10-29).
// The sim pins the numbers (tech.rs table tests + the
// imperial_age_gates_cost_timing_and_steps integration test in
// sim/tests/castle_age.rs); these pins hold the CLIENT half of
// the rung: the age/tech name tables carry tag 3 / tag 18, the
// TC panel offers the Imperial advance from Castle Age with the
// exact cost/time in its description, the click dispatches tech
// tag 18, the Locked refusal names the TEC-02 gate, and the
// event feed gains the Imperial lines in all three locales.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const i18n = readFileSync(join(here, 'i18n.js'), 'utf8');
const feed = readFileSync(join(here, 'feed.js'), 'utf8');

test('age and tech name tables carry the Imperial entries', () => {
  assert.match(appJs, /'Castle Age', 'Imperial Age'\]/);
  assert.match(appJs, /18: 'Imperial Age'/);
  assert.match(appJs, /t\('age\.imperial'\)/);
});

test('the TC panel offers Imperial from Castle Age at 1000 F / 800 G / 190 s', () => {
  assert.match(appJs, /myResearchBlockTc\(18\)/);
  assert.match(appJs, /cost: 1000, cost2Icon: 'i-gold', cost2: 800/);
  assert.match(appJs, /cmd\.imperialTitle.*1000, 'food'.*800, 'gold'.*secs: 190/s);
  assert.match(appJs, /cmd\.imperialGate/);
});

test('the age click dispatches tag 18 from Castle Age and names the gate on refusal', () => {
  assert.match(appJs, /myAge\(\) >= 2 \? 18/);
  assert.match(appJs, /tag === 18 && rb === 4[\s\S]{0,80}cmd\.imperialNeeds/);
});

test('the feed carries the Imperial lines', () => {
  assert.match(feed, /imperial: 'Imperial Age Research Complete'/);
  assert.match(feed, /next\.age >= 3/);
  assert.match(appJs, /feed\.foeImperial/);
});

test('i18n parity: the Imperial keys exist in EN, PT and ES', () => {
  for (const key of ['age.imperial', 'feed.imperial', 'feed.foeImperial',
    'gate.imperial', 'cmd.imperialGate', 'cmd.imperialTitle', 'cmd.imperialNeeds']) {
    const n = i18n.split(`'${key}'`).length - 1;
    assert.equal(n, 3, `${key} should appear once per locale, found ${n}`);
  }
});
