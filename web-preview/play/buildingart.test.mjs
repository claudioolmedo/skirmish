// Building-art integrity (2026-10-23) — regression for the Mining
// Camp that rendered SPLIT IN THE MIDDLE in live play: the art
// pipeline's strip slicer (process_strip.py, 0.98 full-column
// magenta rule) mis-segmented the mining strip, so the shipped
// mining-camp*.png files were composites of two adjacent strip
// panels with a keyed-out gutter between them — two half-buildings
// with a transparent seam. The raw strip was always coherent; the
// slicing was not.
//
// Contract asserted here for every file the renderer loads via
// BUILDING_ART_FILES (app.js):
//  1. It exists and is a decodable 8-bit RGBA PNG.
//  2. NO SPLIT SEAM: no fully-transparent interior column band
//     >= 6px wide with substantial art (>= 1500 opaque px) on
//     BOTH sides — the signature of two panels spliced into one
//     sprite.
//  3. NO FOREIGN FRAGMENT: no component >= 100px that sits
//     detached (> 10px away) from the main component AND outside
//     the main component's bounding box (expanded 12px) — the
//     signature of a sliver of a neighbouring strip panel bleeding
//     into the crop (found and removed from lumber-camp.png).
//  4. STAGE-SET CANVAS: a building kind's completed art and its
//     Stage1..3 frames share one canvas size, so the bottom-center
//     anchored draw (drawArtImage) never pops between stages.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { inflateSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const buildingsDir = join(here, '..', 'assets-original', 'buildings');
const appJs = readFileSync(join(here, '..', 'app.js'), 'utf8');

// The exact file set the renderer loads (BUILDING_ART_FILES values).
const artBlock = appJs.match(/const BUILDING_ART_FILES = \{([\s\S]*?)\};/);
assert.ok(artBlock, 'BUILDING_ART_FILES block found in app.js');
const ART_FILES = {}; // key -> file
for (const m of artBlock[1].matchAll(/(\w+):\s*'([^']+\.png)'/g)) {
  ART_FILES[m[1]] = m[2];
}
assert.ok(Object.keys(ART_FILES).length >= 40, 'building art file list parsed');

// Minimal PNG decoder for the pipeline's output format: 8-bit
// RGBA (color type 6), non-interlaced. Returns { w, h, alpha }.
function decodePngAlpha(path) {
  const buf = readFileSync(path);
  assert.equal(buf.toString('ascii', 1, 4), 'PNG', `${path} is a PNG`);
  let w = 0, h = 0, bitDepth = 0, colorType = 0, interlace = 0;
  const idat = [];
  for (let off = 8; off < buf.length;) {
    const len = buf.readUInt32BE(off);
    const type = buf.toString('ascii', off + 4, off + 8);
    const data = buf.subarray(off + 8, off + 8 + len);
    if (type === 'IHDR') {
      w = data.readUInt32BE(0); h = data.readUInt32BE(4);
      bitDepth = data[8]; colorType = data[9]; interlace = data[12];
    } else if (type === 'IDAT') idat.push(data);
    off += 12 + len;
  }
  assert.equal(bitDepth, 8, `${path}: 8-bit PNG`);
  assert.equal(colorType, 6, `${path}: RGBA PNG (color type 6)`);
  assert.equal(interlace, 0, `${path}: non-interlaced PNG`);
  const raw = inflateSync(Buffer.concat(idat));
  const stride = w * 4;
  const px = new Uint8Array(w * h * 4);
  let prev = new Uint8Array(stride);
  for (let y = 0; y < h; y++) {
    const rowOff = y * (stride + 1);
    const filter = raw[rowOff];
    const row = raw.subarray(rowOff + 1, rowOff + 1 + stride);
    const out = px.subarray(y * stride, (y + 1) * stride);
    for (let x = 0; x < stride; x++) {
      const a = x >= 4 ? out[x - 4] : 0;
      const b = prev[x];
      const c = x >= 4 ? prev[x - 4] : 0;
      let v = row[x];
      if (filter === 1) v = (v + a) & 255;
      else if (filter === 2) v = (v + b) & 255;
      else if (filter === 3) v = (v + ((a + b) >> 1)) & 255;
      else if (filter === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        v = (v + (pa <= pb && pa <= pc ? a : pb <= pc ? b : c)) & 255;
      } else assert.equal(filter, 0, `${path}: PNG filter ${filter}`);
      out[x] = v;
    }
    prev = out;
  }
  const alpha = new Uint8Array(w * h);
  for (let i = 0; i < w * h; i++) alpha[i] = px[i * 4 + 3];
  return { w, h, alpha };
}

// Connected components over alpha > 12 (the pipeline's own keying
// threshold). Returns [{ size, minX, maxX, minY, maxY, mask }].
function components({ w, h, alpha }) {
  const label = new Int32Array(w * h).fill(-1);
  const comps = [];
  const stack = [];
  for (let s = 0; s < w * h; s++) {
    if (alpha[s] <= 12 || label[s] !== -1) continue;
    const id = comps.length;
    let size = 0, minX = w, maxX = 0, minY = h, maxY = 0;
    label[s] = id; stack.push(s);
    while (stack.length) {
      const p = stack.pop();
      const x = p % w, y = (p / w) | 0;
      size++;
      if (x < minX) minX = x; if (x > maxX) maxX = x;
      if (y < minY) minY = y; if (y > maxY) maxY = y;
      if (x > 0 && alpha[p - 1] > 12 && label[p - 1] === -1) { label[p - 1] = id; stack.push(p - 1); }
      if (x < w - 1 && alpha[p + 1] > 12 && label[p + 1] === -1) { label[p + 1] = id; stack.push(p + 1); }
      if (y > 0 && alpha[p - w] > 12 && label[p - w] === -1) { label[p - w] = id; stack.push(p - w); }
      if (y < h - 1 && alpha[p + w] > 12 && label[p + w] === -1) { label[p + w] = id; stack.push(p + w); }
    }
    comps.push({ size, minX, maxX, minY, maxY });
  }
  return { comps, label };
}

const decoded = {};
for (const [key, file] of Object.entries(ART_FILES)) {
  const path = join(buildingsDir, file);
  test(`building art ${file}: exists, decodable, no split seam, no foreign fragment`, () => {
    assert.ok(existsSync(path), `${file} exists`);
    const img = decodePngAlpha(path);
    decoded[key] = img;
    const { w, h, alpha } = img;
    // Column occupancy + interior transparent bands.
    const colHas = new Array(w).fill(false);
    const colCount = new Array(w).fill(0);
    for (let x = 0; x < w; x++) {
      for (let y = 0; y < h; y++) {
        if (alpha[y * w + x] > 12) { colHas[x] = true; colCount[x]++; }
      }
    }
    let x = 0;
    while (x < w) {
      if (colHas[x]) { x++; continue; }
      let e = x;
      while (e < w && !colHas[e]) e++;
      const bandW = e - x;
      if (x > 0 && e < w && bandW >= 6) {
        let left = 0, right = 0;
        for (let i = 0; i < x; i++) left += colCount[i];
        for (let i = e; i < w; i++) right += colCount[i];
        assert.ok(left < 1500 || right < 1500,
          `${file}: split seam — ${bandW}px transparent band at x=${x} with ${left}px art left and ${right}px art right of it`);
      }
      x = e;
    }
    // Foreign fragments: detached from and outside the main body.
    const { comps, label } = components(img);
    assert.ok(comps.length >= 1, `${file}: has art`);
    const big = comps.reduce((a, b) => (b.size > a.size ? b : a));
    const near = (cx, cy) => {
      const x0 = Math.max(0, big.minX - 12), x1 = Math.min(w - 1, big.maxX + 12);
      const y0 = Math.max(0, big.minY - 12), y1 = Math.min(h - 1, big.maxY + 12);
      return cx >= x0 && cx <= x1 && cy >= y0 && cy <= y1;
    };
    for (const c of comps) {
      if (c === big || c.size < 100) continue;
      // Detached: no pixel of this component within 10px of the big one.
      let detached = true;
      outer: for (let yy = Math.max(0, c.minY - 10); yy <= Math.min(h - 1, c.maxY + 10); yy++) {
        for (let xx = Math.max(0, c.minX - 10); xx <= Math.min(w - 1, c.maxX + 10); xx++) {
          if (label[yy * w + xx] === comps.indexOf(big)) { detached = false; break outer; }
        }
      }
      const outsideBox = !near(c.minX, c.minY) && !near(c.maxX, c.maxY) &&
        !near(c.minX, c.maxY) && !near(c.maxX, c.minY);
      assert.ok(!(detached && outsideBox),
        `${file}: foreign fragment — component of ${c.size}px at x ${c.minX}-${c.maxX}, y ${c.minY}-${c.maxY}, detached from the main body (x ${big.minX}-${big.maxX}, y ${big.minY}-${big.maxY})`);
    }
  });
}

test('building art: each kind and its stage frames share one canvas', () => {
  const groups = {};
  for (const key of Object.keys(ART_FILES)) {
    const base = key.replace(/Stage[123]$/, '');
    (groups[base] ||= []).push(key);
  }
  for (const [base, keys] of Object.entries(groups)) {
    if (keys.length < 2) continue;
    const sizes = keys.map((k) => {
      const p = join(buildingsDir, ART_FILES[k]);
      const b = readFileSync(p);
      return `${b.readUInt32BE(16)}x${b.readUInt32BE(20)}`;
    });
    assert.ok(sizes.every((s) => s === sizes[0]),
      `${base}: stage set canvases differ (${keys.map((k, i) => `${k}=${sizes[i]}`).join(', ')})`);
  }
});
