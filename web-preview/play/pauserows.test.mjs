// Pause-menu action-row clipping regression pins (bot-loop, 2026-11-05).
//
// Defect (live proof #155 on the real client): in the pause menu,
// the first .pause-actions row's sixth button, "Resign match"
// (#btn-resign), rendered clipped at the panel's right edge — the
// label read "Resig matc".
//
// Root cause: .pause-actions was `display:flex` WITHOUT flex-wrap,
// inside an .overlay-card whose content box is 460px (max-width
// 520px, border-box, 2x30px padding). The row's six .btn buttons
// (site.css: 15px font, 9px/18px padding, 1px border) have natural
// widths summing to ~725px in EN (measured with Liberation Sans
// Bold at 15px: 96+122+90+117+113+136 + 5x10px gaps = ~725px;
// PT sums to ~787px), and the second row's five .btn-sm buttons
// sum to ~811px. A non-wrapping flex row cannot fit either sum:
// the items overflow the card and the last label is cut.
//
// Fix: flex-wrap: wrap on .pause-actions (the .mode-row and
// .quality-btns rows in the same menu already wrap). Each row now
// breaks into lines at natural button widths — every single
// button's natural width (max ~247px, "Report a problem /
// Contribute" at .btn-sm) is far under the 460px line width, and
// under the ~288px content width of a 360px phone — so no label
// can be clipped by the panel edge in any locale. Rows that
// already fit on one line render exactly as before.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const indexHtml = readFileSync(join(here, 'index.html'), 'utf8');
const playCss = readFileSync(join(here, 'play.css'), 'utf8');

test('the pause action rows wrap instead of overflowing the card', () => {
  const m = playCss.match(/\.pause-actions\s*\{[^}]*\}/);
  assert.ok(m, '.pause-actions rule found in play.css');
  assert.ok(/display:\s*flex/.test(m[0]), 'the rows stay flex rows');
  assert.ok(/flex-wrap:\s*wrap/.test(m[0]),
    '.pause-actions must wrap: the 6-button first row (~725px EN) ' +
    'and the 5-button second row (~811px) both exceed the card\'s ' +
    '460px content box, so a nowrap row clips the last label ' +
    '(live proof #155: "Resign match" read as "Resig matc")');
});

test('the first pause row still carries all six buttons, Resign included', () => {
  const pauseMenu = indexHtml.indexOf('id="pause-menu"');
  assert.ok(pauseMenu >= 0, 'pause menu exists');
  const rowStart = indexHtml.indexOf('class="pause-actions"', pauseMenu);
  const rowEnd = indexHtml.indexOf('</div>', rowStart);
  const row = indexHtml.slice(rowStart, rowEnd);
  for (const id of ['resume-btn', 'help-btn', 'restart-btn', 'btn-online',
    'pause-fullscreen-btn', 'btn-resign']) {
    assert.ok(row.includes(`id="${id}"`),
      `first pause row still contains #${id} (the fix is layout, not removal)`);
  }
});
