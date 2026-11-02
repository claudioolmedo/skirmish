// Wave-1 leftovers wiring pins (INF-11 + INF-06 + ECO-14):
// the client halves of the three rules are mostly DOM and
// dispatch code that the node suite cannot execute, so — in
// the researchlock.test.mjs idiom — this file pins the wiring
// as source facts: if a future edit drops a case, a button or
// an export call, the pin fails instead of the feature silently
// dying. The sim-side behavior is pinned by the Rust suites
// (lib tests + feudal_techs/save_restore), and the tree data
// by techtree.test.mjs against the Rust table itself.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const app = readFileSync(new URL('../app.js', import.meta.url), 'utf8');
const html = readFileSync(new URL('./index.html', import.meta.url), 'utf8');
const netplay = readFileSync(new URL('./netplay.js', import.meta.url), 'utf8');
const i18n = readFileSync(new URL('./i18n.js', import.meta.url), 'utf8');

test('ECO-14: the reseed commands ride every client path', () => {
  // Local dispatch.
  assert.ok(app.includes("case 'queue_reseed': return sim.push_queue_reseed(cmd.b);"));
  assert.ok(app.includes("case 'dequeue_reseed': return sim.push_dequeue_reseed(cmd.b);"));
  assert.ok(app.includes("case 'set_auto_reseed': return sim.push_set_auto_reseed(cmd.b, cmd.on);"));
  // Netplay: validation + application, same vocabulary.
  assert.ok(netplay.includes("case 'queue_reseed': return isInt(c.b);"));
  assert.ok(netplay.includes("case 'dequeue_reseed': return isInt(c.b);"));
  assert.ok(netplay.includes("case 'set_auto_reseed': return isInt(c.b) && typeof c.on === 'boolean';"));
  assert.ok(netplay.includes("case 'queue_reseed': return sim.push_queue_reseed(cmd.b);"));
  assert.ok(netplay.includes("case 'dequeue_reseed': return sim.push_dequeue_reseed(cmd.b);"));
  assert.ok(netplay.includes("case 'set_auto_reseed': return sim.push_set_auto_reseed(cmd.b, cmd.on);"));
  // The Mill panel issues them, and its signature watches the
  // reseed state (title count + toggle face).
  assert.ok(app.includes("issue({ t: 'queue_reseed', b: selectedBuilding.id })"));
  assert.ok(app.includes("issue({ t: 'dequeue_reseed', b: selectedBuilding.id })"));
  assert.ok(app.includes("on: auto !== 1"));
  assert.ok(app.includes("sim.reseed_for(localOwner).join('.')"));
  assert.ok(app.includes("id: 'reseed-queue'"));
  assert.ok(app.includes("id: 'reseed-auto'"));
});

test('ECO-14: the farm chain is on the Mill panel and the research map', () => {
  assert.ok(app.includes("'research-heavyplow': 30, 'research-croprotation': 31,"));
  assert.ok(app.includes("campBtn('research-heavyplow', 30, 'i-heavyplow', 'Heavy Plow',"));
  assert.ok(app.includes("campBtn('research-croprotation', 31, 'i-croprotation', 'Crop Rotation',"));
  assert.ok(app.includes('mill: [13, 30, 31]'), 'RB_TAGS mill carries the chain');
  // Netplay lets the two new tags through research_at.
  assert.ok(netplay.includes('(c.tech >= 29 && c.tech <= 36)'));
  // Icons exist for both buttons and the reseed desk.
  for (const icon of ['i-heavyplow', 'i-croprotation', 'i-reseed']) {
    assert.ok(html.includes(`<symbol id="${icon}"`), `${icon} symbol`);
  }
});

test('INF-11: scoreboard, sampling, end statistics and view-map are wired', () => {
  // Top-bar door + overlay exist and toggle without pausing.
  assert.ok(html.includes('id="score-btn"'));
  assert.ok(html.includes('id="score-overlay"'));
  assert.ok(app.includes('function refreshScoreboard()'));
  assert.ok(app.includes('function setScore(open)'));
  // The timeline samples both public totals per game minute
  // and resets per match.
  assert.ok(app.includes('function sampleScore()'));
  assert.ok(app.includes('Math.floor(sim.tick_count() / 3600)'));
  assert.ok(app.includes('function resetScoreHistory()'));
  assert.ok(app.includes('resetScoreHistory();'));
  // The end card renders the sim counters + researched lists
  // and draws the sampled timeline.
  assert.ok(app.includes('function renderEndStats()'));
  assert.ok(app.includes('renderEndStats();'));
  assert.ok(app.includes('sim.match_stats_for(localOwner)'));
  assert.ok(app.includes('sim.researched_techs_for(owner)'));
  assert.ok(html.includes('id="end-timeline"'));
  assert.ok(html.includes('id="end-stats"'));
  // View map: the reveal flag feeds myFog, the button and the
  // Results chip are wired.
  assert.ok(app.includes('if (revealMap) return revealedFog();'));
  assert.ok(app.includes('function setRevealMap(on)'));
  assert.ok(html.includes('id="end-viewmap-btn"'));
  assert.ok(html.includes('id="end-results-chip"'));
});

test('INF-06: the technology tree view is wired to the pinned data module', () => {
  assert.ok(html.includes('id="techtree-btn"'));
  assert.ok(html.includes('id="techtree-overlay"'));
  assert.ok(html.includes('id="techtree-body"'));
  assert.ok(app.includes("from './play/techtree.js'"));
  assert.ok(app.includes('function buildTechTree()'));
  assert.ok(app.includes('function refreshTechTree()'));
  assert.ok(app.includes('techState(e, mask, age)'));
  assert.ok(app.includes('function setTechTree(open)'));
});

test('the new strings exist in all three locales', () => {
  for (const key of [
    'tech.heavyplow', 'tech.croprotation', 'cmd.heavyplowTitle',
    'cmd.croprotationTitle', 'mill.reseedQueue', 'mill.reseedAuto',
    'top.score', 'top.techtree', 'score.title', 'tree.title',
    'end.statsUnitsKilled', 'end.statsFood', 'end.techsTitle',
    'end.viewMap', 'end.results',
  ]) {
    const hits = i18n.split(`'${key}':`).length - 1;
    assert.equal(hits, 3, `${key} present in EN + PT-BR + ES`);
  }
});
