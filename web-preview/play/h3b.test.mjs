// H3b cached-sprite recordability pins (bot-loop, 2026-11-18;
// occorder2-build H3, the slice h3spear-build left open after
// kinds 9/10).
//
// The defect: the Militia / Man-at-Arms vector fallback
// (drawFootman, shared by kinds 3 and 8), the Archer vector
// fallback (kind 4) and the Villager's final arc-fill fallback
// (kind 0) paint pure ctx fills/strokes — no drawImage, no
// cached sprite — so recordFrameSprites records nothing for
// them and no INF-12 occlusion record can exist while a unit
// is on its fallback. UNIT_ART covers the footmen's attack
// states (militiaAttack* / maaAttack*) and the Archer's shoot
// state (archerShoot*) only when those images are loaded, and
// the Villager's fallback sits behind three sprite layers (the
// original sheet, the UNIT_ART poses, the CC0 placeholder
// sprite); with the DIR sheet also unavailable, the fallback is
// what an attacking/shooting footman or archer — or any
// villager — degrades to. This is the monk/cavalry/Scout
// call-less defect class (5413df1), the Scout shape.
//
// The fix follows the Scout pattern in 5413df1 and the
// Spearman/Skirmisher pattern in 3d40245 exactly: each figure
// is painted ONCE into a cached sprite per cloth/colour
// (paintFootmanFigure / paintArcherFigure /
// paintVillagerFigure, shapes and colours verbatim) and
// blitted with drawImage; the direct vector paint remains only
// for a canvas-less stub. Presentation only; sim and golden
// hash untouched.
//
// The wiring pins fail on pristine; the behavioural pins run
// the REAL sliced draw chains against a recording stub canvas
// and count the drawImage calls the INF-12 recorder would
// capture.
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
// Reachability fact (passes both ways; it is why these fallbacks
// are exposed exactly when their art is not loaded/drawn).

test('UNIT_ART covers the attack/shoot states these fallbacks sit behind', () => {
  const files = sliceFn(appJs, 'const UNIT_ART_FILES = {', '\n};');
  assert.match(files, /militiaAttack1/, 'militia attack frames exist — when loaded');
  assert.match(files, /maaAttack1/, 'maa attack frames exist — when loaded');
  assert.match(files, /archerShoot1/, 'archer shoot frames exist — when loaded');
  assert.match(files, /villagerIdle1/, 'villager frames exist — behind two more layers besides');
  const spec = sliceFn(appJs, 'const DIR_ART_SPEC = {', '\n};');
  assert.match(spec, /militia: \{ states: \['idle', 'walk', 'attack'\]/,
    'the DIR spec carries a militia sheet — when it is drawable');
  assert.match(spec, /archer: \{ states: \['idle', 'walk', 'shoot'\]/,
    'the DIR spec carries an archer sheet — when it is drawable');
});

// ------------------------------------------------------------------
// Footman wiring (fails on pristine).

test('drawFootman blits a cached sprite through drawImage', () => {
  const body = fnBody('drawFootman');
  assert.match(body, /footmanSprite\(cloth\)/,
    'drawFootman obtains the cached footman sprite for its fallback');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'the fallback records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintFootmanFigure\(ctx, sx, sy, u, cloth\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per footman draw (its own blit)');
  assert.match(body, /drawOwner === foeOwner\(\) \? ENEMY_CLOTH : '#31579e'/,
    'the cloth resolution is the pre-fix tunic colour expression');
});

test('the footman sprite is cached per cloth colour', () => {
  const body = fnBody('footmanSprite');
  assert.match(body, /footmanSpriteCache\.has\(cloth\)/,
    'cache is consulted before painting');
  assert.match(body, /footmanSpriteCache\.set\(cloth, cv\)/,
    'painted sprite is stored per cloth colour');
  assert.match(body, /paintFootmanFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintFootmanFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintFootmanFigure');
  for (const colour of ['#e8c39a', '#7a4a21', '#e8e8e8']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (head, shield, sword)`);
  }
  assert.match(body, /g\.ellipse\(sx, sy - 0\.85 \* u, 0\.62 \* u, 0\.8 \* u/,
    'the tunic keeps its geometry');
  assert.match(body, /g\.arc\(sx - 0\.75 \* u, sy - 1\.0 \* u, 0\.42 \* u/,
    'the shield keeps its geometry');
  assert.match(body, /g\.lineTo\(sx \+ 1\.15 \* u, sy - 2\.2 \* u\)/,
    'the sword still reaches its documented tip');
});

// ------------------------------------------------------------------
// Archer wiring (fails on pristine).

test('drawArcher blits a cached sprite through drawImage', () => {
  const body = fnBody('drawArcher');
  assert.match(body, /archerSprite\(cloth\)/,
    'drawArcher obtains the cached archer sprite for its fallback');
  assert.match(body, /ctx\.drawImage\(spr,/,
    'the fallback records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintArcherFigure\(ctx, sx, sy, u, cloth\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage per archer draw (its own blit)');
  assert.match(body, /drawOwner === foeOwner\(\) \? ENEMY_ARCHER_CLOTH : '#4a6e46'/,
    'the cloth resolution is the pre-fix tunic colour expression');
});

test('the archer sprite is cached per cloth colour', () => {
  const body = fnBody('archerSprite');
  assert.match(body, /archerSpriteCache\.has\(cloth\)/,
    'cache is consulted before painting');
  assert.match(body, /archerSpriteCache\.set\(cloth, cv\)/,
    'painted sprite is stored per cloth colour');
  assert.match(body, /paintArcherFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintArcherFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintArcherFigure');
  for (const colour of ['#e8c39a', '#8a6844', '#e8e8e8']) {
    assert.ok(body.includes(colour),
      `figure colour ${colour} preserved (head, bow arc, bowstring)`);
  }
  assert.match(body, /g\.ellipse\(sx, sy - 0\.85 \* u, 0\.58 \* u, 0\.8 \* u/,
    'the tunic keeps its geometry');
  assert.match(body, /g\.arc\(sx \+ 0\.75 \* u, sy - 1\.35 \* u, 0\.62 \* u, -1\.1, 1\.1\)/,
    'the bow arc keeps its geometry');
  assert.match(body, /g\.lineTo\(sx \+ 1\.12 \* u, sy - 0\.78 \* u\)/,
    'the bowstring keeps its geometry');
});

// ------------------------------------------------------------------
// Villager wiring (fails on pristine).

test('drawVillager blits a cached sprite through drawImage on its final fallback', () => {
  const body = fnBody('drawVillager');
  assert.match(body, /villagerSprite\(color\)/,
    'drawVillager obtains the cached villager sprite for its fallback');
  assert.match(body, /ctx\.drawImage\(vspr,/,
    'the fallback records a drawImage call the INF-12 recorder can capture');
  assert.match(body, /paintVillagerFigure\(ctx, sx, sy, u, color\)/,
    'vector fallback retained for environments without a canvas');
  assert.equal((body.match(/ctx\.drawImage\(/g) || []).length, 1,
    'exactly one drawImage in drawVillager (its own fallback blit)');
  assert.match(body, /drawOwner === foeOwner\(\) \? ENEMY_COLOR : COLORS\[i % COLORS\.length\]/,
    'the fill-colour resolution is the pre-fix palette expression');
});

test('the villager sprite is cached per fill colour', () => {
  const body = fnBody('villagerSprite');
  assert.match(body, /villagerSpriteCache\.has\(color\)/,
    'cache is consulted before painting');
  assert.match(body, /villagerSpriteCache\.set\(color, cv\)/,
    'painted sprite is stored per fill colour');
  assert.match(body, /paintVillagerFigure\(g,/,
    'the sprite is painted by the shared figure painter');
});

test('paintVillagerFigure keeps the original figure (colours unchanged)', () => {
  const body = fnBody('paintVillagerFigure');
  assert.ok(body.includes('rgba(0,0,0,0.4)'),
    'the outline colour is preserved');
  assert.match(body, /g\.arc\(sx, by, 0\.85 \* u, 0, Math\.PI \* 2\)/,
    'the circle keeps its radius');
  assert.match(body, /const by = sy - 1\.15 \* u/,
    'the circle still rides 1.15 u above the ground point');
});

// ------------------------------------------------------------------
// Sprite boxes vs the figures' content bounds (fails on
// pristine: the *_BOX consts do not exist). Each blit box must
// contain its figure's content bounds — read off the painters —
// with only a small margin, like every other cached-sprite
// kind. The FIX #3 UNIT_BODY_BOX entries for these kinds are
// derived from the ART heights (UNIT_ART_H), not from these
// vector figures, so unlike kinds 9/10 they are not the content
// bounds and are deliberately untouched — the last pin holds
// them at their baseline values.

test('sprite boxes contain their figure content bounds with a small margin', () => {
  const boxes = {};
  for (const name of ['FOOTMAN_BOX', 'ARCHER_BOX', 'VILLAGER_BOX']) {
    const m = appJs.match(new RegExp(
      `const ${name} = \\{ x0: (-?[\\d.]+), y0: (-?[\\d.]+), w: ([\\d.]+), h: ([\\d.]+) \\};`));
    assert.ok(m, `${name} declared`);
    boxes[name] = { x0: +m[1], y0: +m[2], w: +m[3], h: +m[4] };
  }
  // [content dxMin, content dxMax, content top, content bottom]
  const content = {
    FOOTMAN_BOX: [-1.17, 1.21, 2.29, 0],
    ARCHER_BOX: [-0.58, 1.45, 2.29, 0],
    VILLAGER_BOX: [-0.85, 0.85, 2.0, -0.3],
  };
  for (const [name, [dxMin, dxMax, top, bottom]] of Object.entries(content)) {
    const box = boxes[name];
    assert.ok(box.x0 <= dxMin && box.x0 >= dxMin - 0.5,
      `${name}: left margin is small and covers the figure`);
    assert.ok(box.x0 + box.w >= dxMax && box.x0 + box.w <= dxMax + 0.5,
      `${name}: right margin is small and covers the figure`);
    assert.ok(-box.y0 >= top && -box.y0 <= top + 0.5,
      `${name}: top margin is small and covers the figure`);
    assert.ok(box.y0 + box.h >= bottom && box.y0 + box.h <= bottom + 0.5,
      `${name}: bottom margin is small and covers the figure`);
  }
  const body = sliceFn(appJs, 'const UNIT_BODY_BOX = {', '\n  };');
  for (const [kind, entry] of [[0, '-1.1, 1.1, 3.1'], [3, '-1.17, 1.15, 2.6'],
    [4, '-0.9, 1.15, 2.55'], [8, '-1.17, 1.15, 2.65']]) {
    assert.ok(body.includes(`${kind}: [${entry}]`),
      `UNIT_BODY_BOX[${kind}] untouched at its art-derived baseline`);
  }
});

// ------------------------------------------------------------------
// Behavioural: the REAL sliced draw chains against a recording
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
    UNIT_ART_H: { militia: 2.6, maa: 2.65, archer: 2.55, villager: 2.55 },
    drawOwner: owner,
    foeOwner: () => 2,
    ENEMY_CLOTH: '#a02c22',
    ENEMY_ARCHER_CLOTH: '#8e3b2f',
    // Villager chain extras: every sprite layer above the
    // fallback answers "not drawable", and the villager carries
    // nothing, so only the fallback paints.
    performance: { now: () => 0 },
    updateVilAnim: () => ({ carryRes: null, sheetUsed: null }),
    drawOriginalVillager: () => false,
    drawSpriteH: () => false,
    SPRITES: { villager: {} },
    ENEMY_COLOR: '#e0483e',
    COLORS: ['#e74c3c', '#3498db', '#2ecc71', '#f1c40f', '#9b59b6'],
    CARRY_COLORS: {},
    CARRY_DOT_KIND: {},
    Q: { idLabels: false },
  };
  const footSrc =
    sliceFn(appJs, 'function paintFootmanFigure(g, sx, sy, u, cloth) {',
      '\n// --- Animal pose art') +
    fnBody('drawMilitia') + '\n' + fnBody('drawManAtArms');
  const vilSrc =
    sliceFn(appJs, 'function paintVillagerFigure(g, sx, sy, u, color) {',
      '\nfunction drawVillager') + '\n' + fnBody('drawVillager');
  const factory = new Function(...Object.keys(env),
    `${footSrc}\n${vilSrc}\nreturn { drawMilitia, drawManAtArms, drawArcher, drawVillager };`);
  const fns = factory(...Object.values(env));
  return { ...fns, created, blits, frameOps };
}

test('behavioural: a fallback Militia draw records exactly one drawImage (the INF-12 record)', () => {
  const w = makeWorld({ owner: 0 });
  w.drawMilitia(5, 10, 20);
  assert.equal(w.blits.length, 1,
    'recordFrameSprites would capture one call for this draw item');
  const { im, rest } = w.blits[0];
  assert.equal(im.width, Math.ceil(2.88 * 16), 'sprite canvas width from FOOTMAN_BOX');
  assert.equal(im.height, Math.ceil(2.69 * 16), 'sprite canvas height from FOOTMAN_BOX');
  assert.deepEqual(rest, [110 - 1.42 * 8, 220 - 2.54 * 8, 2.88 * 8, 2.69 * 8],
    'the blit dest is the sprite box at the unit anchor');
  assert.ok(im.ops.some((op) => op[0] === 'fill' && op[1] === '#31579e'),
    'the sprite was painted with the friendly cloth');
  w.drawManAtArms(6, 10, 20);
  assert.equal(w.blits.length, 2, 'every frame blits (every frame records)');
  assert.equal(w.created.length, 1,
    'the Man-at-Arms shares the one cached footman sprite for the cloth (one shared figure, as before)');
});

test('behavioural: an enemy footman gets its own cached sprite in the enemy cloth', () => {
  const w = makeWorld({ owner: 2 });
  w.drawMilitia(5, 10, 20);
  assert.equal(w.blits.length, 1);
  assert.ok(w.blits[0].im.ops.some((op) => op[0] === 'fill' && op[1] === '#a02c22'),
    'the sprite was painted with ENEMY_CLOTH');
});

test('behavioural: a fallback Archer draw records exactly one drawImage (the INF-12 record)', () => {
  const w = makeWorld({ owner: 0 });
  w.drawArcher(6, 10, 20);
  assert.equal(w.blits.length, 1,
    'recordFrameSprites would capture one call for this draw item');
  const { im, rest } = w.blits[0];
  assert.equal(im.width, Math.ceil(2.53 * 16), 'sprite canvas width from ARCHER_BOX');
  assert.equal(im.height, Math.ceil(2.69 * 16), 'sprite canvas height from ARCHER_BOX');
  assert.deepEqual(rest, [110 - 0.83 * 8, 220 - 2.54 * 8, 2.53 * 8, 2.69 * 8],
    'the blit dest is the sprite box at the unit anchor');
  assert.ok(im.ops.some((op) => op[0] === 'fill' && op[1] === '#4a6e46'),
    'the sprite was painted with the friendly archer cloth');
  w.drawArcher(6, 10, 20);
  assert.equal(w.blits.length, 2, 'every frame blits (every frame records)');
  assert.equal(w.created.length, 1, 'but the sprite itself is painted only once (cached)');
});

test('behavioural: an enemy Archer gets its own cached sprite in the enemy archer cloth', () => {
  const w = makeWorld({ owner: 2 });
  w.drawArcher(6, 10, 20);
  assert.equal(w.blits.length, 1);
  assert.ok(w.blits[0].im.ops.some((op) => op[0] === 'fill' && op[1] === '#8e3b2f'),
    'the sprite was painted with ENEMY_ARCHER_CLOTH');
});

test('behavioural: a fallback Villager draw records exactly one drawImage (the INF-12 record)', () => {
  const w = makeWorld({ owner: 0 });
  w.drawVillager(3, 10, 20);
  assert.equal(w.blits.length, 1,
    'recordFrameSprites would capture one call for this draw item');
  const { im, rest } = w.blits[0];
  assert.equal(im.width, Math.ceil(2.2 * 16), 'sprite canvas width from VILLAGER_BOX');
  assert.equal(im.height, Math.ceil(2.4 * 16), 'sprite canvas height from VILLAGER_BOX');
  assert.deepEqual(rest, [110 - 1.1 * 8, 220 - 2.25 * 8, 2.2 * 8, 2.4 * 8],
    'the blit dest is the sprite box at the unit anchor');
  assert.ok(im.ops.some((op) => op[0] === 'fill' && op[1] === '#f1c40f'),
    'the sprite was painted with COLORS[3], the unit palette colour');
  w.drawVillager(4, 10, 20);
  assert.equal(w.blits.length, 2, 'every frame blits (every frame records)');
  assert.equal(w.created.length, 2,
    'a different palette colour paints its own sprite (cache is per fill colour)');
  assert.ok(w.blits[1].im.ops.some((op) => op[0] === 'fill' && op[1] === '#9b59b6'),
    'the second sprite was painted with COLORS[4]');
  w.drawVillager(3, 10, 20);
  assert.equal(w.created.length, 2, 'a repeated colour reuses its cached sprite');
});

test('behavioural: an enemy Villager gets its own cached sprite in the enemy colour', () => {
  const w = makeWorld({ owner: 2 });
  w.drawVillager(3, 10, 20);
  assert.equal(w.blits.length, 1);
  assert.ok(w.blits[0].im.ops.some((op) => op[0] === 'fill' && op[1] === '#e0483e'),
    'the sprite was painted with ENEMY_COLOR');
});

test('behavioural: without a canvas, all three draws keep the direct vector paint (no drawImage)', () => {
  const w = makeWorld({ owner: 0, canvasless: true });
  w.drawMilitia(5, 10, 20);
  w.drawArcher(6, 10, 20);
  w.drawVillager(3, 10, 20);
  assert.equal(w.blits.length, 0, 'nothing to record — the documented stub degradation');
  assert.ok(w.frameOps.filter((op) => op[0] === 'ellipse').length >= 2,
    'both figures still paint directly onto the frame canvas');
  assert.ok(w.frameOps.some((op) => op[0] === 'fill' && op[1] === '#f1c40f'),
    'the villager circle still paints directly onto the frame canvas');
});
