// H3 cached-sprite recordability pins (bot-loop, 2026-11-18;
// occorder2-build H3, the follow-up occorder2 deliberately did
// not bundle).
//
// The defect: Spearman (kind 9) and Skirmisher (kind 10) have NO
// UNIT_ART_FILES entries at all. Their only sprite path is the
// DIR sheet; when it is not drawable, drawSpearman /
// drawSkirmisher fell back to pure ctx fills/strokes — no
// drawImage, no cached sprite — so recordFrameSprites recorded
// nothing and no INF-12 occlusion record could exist for these
// kinds in ANY state, attacking included. This is the
// monk/cavalry/Scout call-less defect class (5413df1), still open
// for these two kinds.
//
// The fix follows the Scout pattern in 5413df1 exactly: each
// figure is painted ONCE into a cached sprite per cloth
// (paintSpearmanFigure / paintSkirmisherFigure, shapes and
// colours verbatim) and blitted with drawImage; the direct
// vector paint remains only for a canvas-less stub.
// Presentation only; sim and golden hash untouched.
//
// The wiring pins fail on pristine; the behavioural pins run the
// REAL sliced draw chain against a recording stub canvas and
// count the drawImage calls the INF-12 recorder would capture.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

function fnBody(name) {
  const m = appJs.match(new RegExp(`function ${name}\\([\\s\\S]*?\\n\\}`));
  assert.ok(m, `${name} found in app.js`);
  return m[0];
}

function sliceFn(src, startMarker, endMarker) {
  const a = src.indexOf(startMarker);
  assert.ok(a >= 0, `${startMarker} found in app.js`);
  const b = src.indexOf(endMarker, a + startMarker.length);
  assert.ok(b > a, `${endMarker} found after ${startMarker}`);
  return src.slice(a, b);
}

// ------------------------------------------------------------------
// Reachability fact (passes both ways; it is why the fallback is
// the ONLY non-DIR path for these kinds, in every state).

test('UNIT_ART_FILES has no spearman/skirmisher entries (DIR is their only sprite path)', () => {
  const files = sliceFn(appJs, 'const UNIT_ART_FILES = {', '\n};');
  assert.ok(!/spearman/.test(files), 'no spearman entries in UNIT_ART_FILES');
  assert.ok(!/skirmisher/.test(files), 'no skirmisher entries in UNIT_ART_FILES');
  const spec = sliceFn(appJs, 'const DIR_ART_SPEC = {', '\n};');
  assert.match(spec, /spearman: \{ states: \['idle', 'walk', 'attack'\]/,
    'the DIR spec does carry a spearman sheet — when it is drawable');
  assert.match(spec, /skirmisher: \{ states: \['idle', 'walk', 'shoot'\]/,
    'the DIR spec does carry a skirmisher sheet — when it is drawable');
});

// ------------------------------------------------------------------
// Spearman wiring (fails on pristine).

test('drawSpearman blits a cached sprite through drawImage', () => {
  const body = fnBody('drawSpearman');
  assert.match(body, /spearmanSprite\(cloth\)/,
    'drawSpearman obtains the cached spearman sprite for its fallback');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'the fallback records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintSpearmanFigure\(ctx, sx, sy, u, cloth\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per spearman draw (its own blit)');
  assert.match(body, /drawOwner === foeOwner\(\) \? ENEMY_CLOTH : '#31579e'/,
    'the cloth resolution is the pre-fix tunic/shield colour expression');
});

test('the spearman sprite is cached per cloth colour', () => {
  const body = fnBody('spearmanSprite');
  assert.match(body, /spearmanSpriteCache\.has\(cloth\)/,
    'cache is consulted before painting');
  assert.match(body, /spearmanSpriteCache\.set\(cloth, cv\)/,
    'painted sprite is stored per cloth colour');
  assert.match(body, /paintSpearmanFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintSpearmanFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintSpearmanFigure');
  for (const colour of ['#e8c39a', '#7a4a21', '#8a6844', '#d8d8d8']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (head, shield rim, spear shaft, spear tip)`);
  }
  assert.match(body, /g\.ellipse\(sx, sy - 0\.85 \* u, 0\.62 \* u, 0\.8 \* u/,
    'the tunic keeps its geometry');
  assert.match(body, /g\.arc\(sx - 0\.8 \* u, sy - 1\.05 \* u, 0\.5 \* u/,
    'the big round shield keeps its geometry');
  assert.match(body, /g\.lineTo\(sx \+ 0\.55 \* u, sy - 3\.05 \* u\)/,
    'the spear tip still reaches the documented 3.05 top');
});

// ------------------------------------------------------------------
// Skirmisher wiring (fails on pristine).

test('drawSkirmisher blits a cached sprite through drawImage', () => {
  const body = fnBody('drawSkirmisher');
  assert.match(body, /skirmisherSprite\(cloth\)/,
    'drawSkirmisher obtains the cached skirmisher sprite for its fallback');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'the fallback records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintSkirmisherFigure\(ctx, sx, sy, u, cloth\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per skirmisher draw (its own blit)');
  assert.match(body, /drawOwner === foeOwner\(\) \? ENEMY_ARCHER_CLOTH : '#4a6e46'/,
    'the cloth resolution is the pre-fix tunic/shield colour expression');
});

test('the skirmisher sprite is cached per cloth colour', () => {
  const body = fnBody('skirmisherSprite');
  assert.match(body, /skirmisherSpriteCache\.has\(cloth\)/,
    'cache is consulted before painting');
  assert.match(body, /skirmisherSpriteCache\.set\(cloth, cv\)/,
    'painted sprite is stored per cloth colour');
  assert.match(body, /paintSkirmisherFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintSkirmisherFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintSkirmisherFigure');
  for (const colour of ['#e8c39a', '#7a4a21', '#8a6844', '#d8d8d8']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (head, shield rim, javelin shaft, javelin tip)`);
  }
  assert.match(body, /g\.ellipse\(sx, sy - 0\.85 \* u, 0\.58 \* u, 0\.8 \* u/,
    'the tunic keeps its geometry');
  assert.match(body, /g\.arc\(sx - 0\.85 \* u, sy - 1\.05 \* u, 0\.58 \* u/,
    'the large shield keeps its geometry');
  assert.match(body, /g\.lineTo\(sx \+ 0\.55 \* u, sy - 2\.7 \* u\)/,
    'the javelin tip still reaches the documented 2.7 top');
});

// ------------------------------------------------------------------
// Sprite boxes vs the FIX #3 body boxes (fails on pristine: the
// *_BOX consts do not exist). The blit box must contain the
// figure's content bounds — which are exactly the UNIT_BODY_BOX
// entries — with only a small margin, like every other
// cached-sprite kind.

test('sprite boxes contain the UNIT_BODY_BOX content bounds with a small margin', () => {
  const boxes = {};
  for (const name of ['SPEARMAN_BOX', 'SKIRMISHER_BOX']) {
    const m = appJs.match(new RegExp(
      `const ${name} = \\{ x0: (-?[\\d.]+), y0: (-?[\\d.]+), w: ([\\d.]+), h: ([\\d.]+) \\};`));
    assert.ok(m, `${name} declared`);
    boxes[name] = { x0: +m[1], y0: +m[2], w: +m[3], h: +m[4] };
  }
  const body = sliceFn(appJs, 'const UNIT_BODY_BOX = {', '\n  };');
  const entry = (kind) => {
    const m = body.match(new RegExp(`\\n    ${kind}: \\[(-?[\\d.]+), (-?[\\d.]+), ([\\d.]+)\\]`));
    assert.ok(m, `UNIT_BODY_BOX[${kind}] found`);
    return { dxMin: +m[1], dxMax: +m[2], h: +m[3] };
  };
  for (const [box, kind] of [[boxes.SPEARMAN_BOX, 9], [boxes.SKIRMISHER_BOX, 10]]) {
    const b = entry(kind);
    assert.ok(box.x0 <= b.dxMin && box.x0 >= b.dxMin - 0.5,
      `kind ${kind}: left margin is small and covers the shield edge`);
    assert.ok(box.x0 + box.w >= b.dxMax && box.x0 + box.w <= b.dxMax + 0.5,
      `kind ${kind}: right margin is small and covers the weapon tip`);
    assert.ok(-box.y0 >= b.h && -box.y0 <= b.h + 0.5,
      `kind ${kind}: top margin is small and covers the weapon top`);
    assert.ok(box.y0 + box.h >= 0 && box.y0 + box.h <= 0.5,
      `kind ${kind}: the box reaches the feet with a small margin`);
  }
});

// ------------------------------------------------------------------
// Behavioural: the REAL sliced draw chain against a recording
// stub canvas. On pristine the slice markers do not exist, and a
// pristine draw records zero drawImage calls — both fail there.

function makeWorld({ owner = 0, canvasless = false } = {}) {
  const created = [];
  const recordingCtx = (ops) => ({
    ops,
    fillStyle: null, strokeStyle: null, lineWidth: null,
    beginPath() { ops.push(['beginPath']); },
    ellipse(...a) { ops.push(['ellipse', ...a]); },
    arc(...a) { ops.push(['arc', ...a]); },
    fill() { ops.push(['fill', this.fillStyle]); },
    stroke() { ops.push(['stroke', this.strokeStyle]); },
    moveTo(...a) { ops.push(['moveTo', ...a]); },
    lineTo(...a) { ops.push(['lineTo', ...a]); },
    closePath() { ops.push(['closePath']); },
  });
  const documentStub = {
    createElement() {
      if (canvasless) throw new Error('no canvas in this stub');
      const cv = { width: 0, height: 0, ops: [], getContext: null };
      cv.getContext = () => recordingCtx(cv.ops);
      created.push(cv);
      return cv;
    },
  };
  const frameOps = [];
  const blits = [];
  const ctxStub = recordingCtx(frameOps);
  ctxStub.drawImage = (im, ...rest) => { blits.push({ im, rest }); };
  const env = {
    document: documentStub,
    ctx: ctxStub,
    worldToScreen: (wx, wy) => [100 + wx, 200 + wy],
    camS: 8,
    shadow: () => {},
    drawSelRing: () => {},
    selection: new Set(),
    unitActionFor: () => 'attack',
    drawDirUnit: () => false,
    unitArtImage: () => null,
    drawUnitPose: () => true,
    UNIT_ART_H: { spearman: 2.6, skirmisher: 2.55 },
    drawOwner: owner,
    foeOwner: () => 2,
    ENEMY_CLOTH: '#a02c22',
    ENEMY_ARCHER_CLOTH: '#8e3b2f',
  };
  const src =
    sliceFn(appJs, 'function paintSpearmanFigure(g, sx, sy, u, cloth) {',
      '\nfunction paintSkirmisherFigure') + '\n' +
    sliceFn(appJs, 'function paintSkirmisherFigure(g, sx, sy, u, cloth) {',
      '\n// Shared draw for the two sword footmen');
  const factory = new Function(...Object.keys(env),
    `${src}\nreturn { drawSpearman, drawSkirmisher };`);
  const fns = factory(...Object.values(env));
  return { ...fns, created, blits, frameOps };
}

test('behavioural: a fallback Spearman draw records exactly one drawImage (the INF-12 record)', () => {
  const w = makeWorld({ owner: 0 });
  w.drawSpearman(5, 10, 20);
  assert.equal(w.blits.length, 1,
    'recordFrameSprites would capture one call for this draw item');
  const { im, rest } = w.blits[0];
  assert.equal(im.width, Math.ceil(2.45 * 16), 'sprite canvas width from SPEARMAN_BOX');
  assert.equal(im.height, Math.ceil(3.45 * 16), 'sprite canvas height from SPEARMAN_BOX');
  assert.deepEqual(rest, [110 - 1.55 * 8, 220 - 3.3 * 8, 2.45 * 8, 3.45 * 8],
    'the blit dest is the sprite box at the unit anchor');
  assert.ok(im.ops.some((op) => op[0] === 'fill' && op[1] === '#31579e'),
    'the sprite was painted with the friendly cloth');
  w.drawSpearman(5, 10, 20);
  assert.equal(w.blits.length, 2, 'every frame blits (every frame records)');
  assert.equal(w.created.length, 1, 'but the sprite itself is painted only once (cached)');
});

test('behavioural: an enemy Spearman gets its own cached sprite in the enemy cloth', () => {
  const w = makeWorld({ owner: 2 });
  w.drawSpearman(5, 10, 20);
  assert.equal(w.blits.length, 1);
  assert.ok(w.blits[0].im.ops.some((op) => op[0] === 'fill' && op[1] === '#a02c22'),
    'the sprite was painted with ENEMY_CLOTH');
});

test('behavioural: a fallback Skirmisher draw records exactly one drawImage (the INF-12 record)', () => {
  const w = makeWorld({ owner: 0 });
  w.drawSkirmisher(6, 10, 20);
  assert.equal(w.blits.length, 1,
    'recordFrameSprites would capture one call for this draw item');
  const { im, rest } = w.blits[0];
  assert.equal(im.width, Math.ceil(2.58 * 16), 'sprite canvas width from SKIRMISHER_BOX');
  assert.equal(im.height, Math.ceil(3.1 * 16), 'sprite canvas height from SKIRMISHER_BOX');
  assert.deepEqual(rest, [110 - 1.68 * 8, 220 - 2.95 * 8, 2.58 * 8, 3.1 * 8],
    'the blit dest is the sprite box at the unit anchor');
  assert.ok(im.ops.some((op) => op[0] === 'fill' && op[1] === '#4a6e46'),
    'the sprite was painted with the friendly archer cloth');
  w.drawSkirmisher(6, 10, 20);
  assert.equal(w.blits.length, 2, 'every frame blits (every frame records)');
  assert.equal(w.created.length, 1, 'but the sprite itself is painted only once (cached)');
});

test('behavioural: an enemy Skirmisher gets its own cached sprite in the enemy archer cloth', () => {
  const w = makeWorld({ owner: 2 });
  w.drawSkirmisher(6, 10, 20);
  assert.equal(w.blits.length, 1);
  assert.ok(w.blits[0].im.ops.some((op) => op[0] === 'fill' && op[1] === '#8e3b2f'),
    'the sprite was painted with ENEMY_ARCHER_CLOTH');
});

test('behavioural: without a canvas, both draws keep the direct vector paint (no drawImage)', () => {
  const w = makeWorld({ owner: 0, canvasless: true });
  w.drawSpearman(5, 10, 20);
  w.drawSkirmisher(6, 10, 20);
  assert.equal(w.blits.length, 0, 'nothing to record — the documented stub degradation');
  assert.ok(w.frameOps.filter((op) => op[0] === 'ellipse').length >= 2,
    'both figures still paint directly onto the frame canvas');
});
