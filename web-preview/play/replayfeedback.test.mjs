// Replay download feedback regression pins (bot-loop, 2026-10-25).
//
// Defect: "Download replay" triggered the .json save but gave NO
// in-page confirmation — the only flash in downloadReplay() was the
// no-replay FAILURE case. Three independent live sessions (bot-loop
// runs 2026-10-25 02:22, 04:22, 05:22) clicked the button and saw
// nothing happen at all, reading it as a dead button — the same
// confusion family as Claudio's "menus didn't work" report. A
// successful action must confirm itself like every other one does.
//
// These pins keep the success flash wired: downloadReplay must
// flash flash.replaySaved on the success path (after the click),
// keep flashing flash.noReplayYet on the empty path, and the new
// string must exist in every shipped locale.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');
const i18nJs = readFileSync(join(here, 'i18n.js'), 'utf8');

function downloadReplayBody() {
  const start = appJs.indexOf('function downloadReplay(replayObj)');
  assert.ok(start >= 0, 'downloadReplay exists in app.js');
  const end = appJs.indexOf('\n}\n', start);
  assert.ok(end > start, 'downloadReplay body is delimited');
  return appJs.slice(start, end);
}

test('download replay confirms a successful save', () => {
  const body = downloadReplayBody();
  const click = body.indexOf('a.click()');
  const saved = body.indexOf("flash(t('flash.replaySaved'))");
  assert.ok(click >= 0, 'the download is triggered');
  assert.ok(saved > click, 'a success flash fires after the download click');
});

test('download replay still explains the empty case', () => {
  const body = downloadReplayBody();
  assert.ok(
    body.includes("flash(t('flash.noReplayYet'))"),
    'the no-replay branch keeps its explanatory flash');
});

// "Watch replay" rename (bot-loop front 4, Claudio 2026-10-27): the
// pause-menu button is a file-picker label — clicking it opens the OS
// dialog to load a downloaded replay .json. Two live sessions in a row
// read the old bare "Watch replay" label as a dead button because
// nothing in-page happens and the label never said it loads a file.
// The label must say so, in every shipped locale, and the static
// fallback text in index.html must match the EN string.
test('the watch-replay label says it loads a file, in every locale', () => {
  const values = [...i18nJs.matchAll(/'pause\.watchReplay':\s*'([^']+)'/g)]
    .map((m) => m[1]);
  assert.equal(values.length, 3, 'pause.watchReplay is defined for EN, PT and ES');
  assert.equal(values[0], 'Watch replay from file…');
  assert.match(values[1], /arquivo/, 'PT label names the file it loads');
  assert.match(values[2], /archivo/, 'ES label names the file it loads');
  const html = readFileSync(join(here, 'index.html'), 'utf8');
  assert.ok(
    html.includes('id="btn-watch-replay"') &&
    html.includes('>Watch replay from file…</label>'),
    'the static index.html label matches the EN string');
});

test('the replay-saved string exists in every locale', () => {
  const hits = i18nJs.match(/'flash\.replaySaved':/g) || [];
  assert.equal(hits.length, 3, 'flash.replaySaved is defined for EN, PT and ES');
});

// Second half of the same defect (bot-loop live verification,
// 2026-10-25 12:22 run): the success flash existed and was live, yet
// a real-client session clicked Download replay FROM the pause menu
// and saw nothing in 8 s — because flash() renders into #toast
// (z-index 25) while the pause menu is an .overlay at z-index 40.
// The confirmation was drawn behind the very menu the click came
// from. Pin the layer order: the toast must sit above the overlays.
test('the confirmation toast renders above the menu overlays', () => {
  const css = readFileSync(join(here, 'play.css'), 'utf8');
  const zOf = (selectorRe) => {
    const m = css.match(selectorRe);
    assert.ok(m, 'selector block found in play.css');
    const z = m[0].match(/z-index:\s*(\d+)/);
    assert.ok(z, 'z-index declared');
    return Number(z[1]);
  };
  const toastZ = zOf(/\.toast\s*\{[^}]*\}/);
  const overlayZ = zOf(/\.overlay\s*\{[^}]*\}/);
  assert.ok(
    toastZ > overlayZ,
    `toast z-index ${toastZ} must exceed overlay z-index ${overlayZ}, ` +
    'or menu-triggered confirmations render hidden behind the menu');
});
