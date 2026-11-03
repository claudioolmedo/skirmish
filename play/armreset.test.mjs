// Armed-targeting reset pin (bot-loop, 2026-11-02) — regression for
// the live-observed leak (15:22 run, Pass B formal, session 46c52fe1):
// the Convert targeting state armed in one match SURVIVED "Restart
// Match" into the next match — the first clicks of the fresh match
// produced "Convert: …" toasts before any Monk existed, and only a
// full page reload cleared it. Root cause: resetSim() (and adoptSim(),
// the online/rejoin twin) cleared `placement` and the selection but
// never the armed targeting states — repairArm, orderArm (attack-move
// / patrol / attack-ground), healArm, convertArm — nor the wall-drag
// anchor. Every per-match reset path must disarm them all, exactly as
// Esc / right-click disarm them mid-match.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function fnBody(sig) {
  const start = appJs.indexOf(sig);
  assert.ok(start >= 0, `${sig} exists`);
  const end = appJs.indexOf('\n}\n', start);
  assert.ok(end > start, `${sig} is delimited`);
  return appJs.slice(start, end);
}

const DISARMS = [
  'placement = null',
  'repairArm = false',
  'orderArm = null',
  'healArm = false',
  'convertArm = false',
  'wallAnchor = null',
];

for (const sig of ['function resetSim(', 'function adoptSim(']) {
  test(`${sig} disarms every armed targeting state for the fresh match`, () => {
    const body = fnBody(sig);
    for (const line of DISARMS) {
      assert.ok(body.includes(line), `${sig} must contain "${line}"`);
    }
  });
}
