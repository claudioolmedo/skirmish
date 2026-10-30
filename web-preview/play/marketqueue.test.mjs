// Market queue-line regression pins (bot-loop, 2026-10-30).
//
// Defect (live session fc61130d on the Package D build 4f75091):
// Package D made the Market a producer/researcher (Train Trade
// Cart, research Caravan), but its selection panel kept the plain
// 'building' shape, whose info line is only the exchange rates.
// During both the cart's training and Caravan's research the panel
// showed no "Training… / Researching… · Queue n" line and no
// cancel chips — the only feedback was the buttons' disabled
// state, so the player could not tell the click had worked (the
// same family as Claudio's Priority No. 1 complaint).
//
// Root cause: updateSelPanel's PROD_KINDS did not include the
// Market (kind 12), and its TRAINS_TAG had no Market entry for
// the housed block. These pins keep the Market in the production
// shape, with its idle line still carrying the exchange rates.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

test('the Market (kind 12) is a production-shape selection', () => {
  const m = appJs.match(/const PROD_KINDS = \[([^\]]+)\];/);
  assert.ok(m, 'PROD_KINDS exists in app.js');
  const kinds = m[1].split(',').map((s) => Number(s.trim()));
  assert.ok(kinds.includes(12), 'PROD_KINDS includes the Market kind 12');
});

test('the Market housed block reads its Trade Cart train tag', () => {
  const m = appJs.match(/const TRAINS_TAG = \{([^}]+)\};/);
  assert.ok(m, 'TRAINS_TAG exists in app.js');
  assert.ok(/12:\s*16/.test(m[1]), 'TRAINS_TAG maps Market 12 -> Trade Cart kind 16');
});

function productionBlock() {
  const start = appJs.indexOf("} else if (context === 'production') {");
  assert.ok(start >= 0, 'the production branch exists in updateSelPanel');
  return appJs.slice(start, start + 4200);
}

test('the production shape renders the queue line and cancel chips', () => {
  const block = productionBlock();
  assert.ok(block.includes('queueCancelChips'), 'queue entries get cancel chips');
  assert.ok(block.includes("t('sel.researchingB'"), 'research entries show a Researching line');
  assert.ok(block.includes("t('sel.trainingB'"), 'train entries show a Training line');
});

test('the Market idle line keeps the exchange rates', () => {
  const block = productionBlock();
  assert.ok(/bk\.kind === 12/.test(block), 'the production idle branch special-cases the Market');
  assert.ok(block.includes("t('sel.marketRates'"), 'the Market idle line is the rates line');
});
