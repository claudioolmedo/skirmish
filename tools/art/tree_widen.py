#!/usr/bin/env python3
"""
Widen the pine node art into a canopy that can overlap its
neighbours, and derive the two progressive chop-damage stages
(forest readability pass, 2026-10-20).

Input : web-preview/assets-original/nodes/tree-source.png
        (the original Base44 pine, 172x393 — kept as the source)
Output: tree.png       widened composite (three layered copies of
                       the same pine: two foliage-only flank copies
                       behind one full centre copy, so the canopy
                       spans ~300px while the single trunk/anchor
                       contract is unchanged)
        tree-dmg1.png  stage 1 (1/3-2/3 wood left): foliage
                       thinned ~30%, a pale chop notch cut into
                       the trunk's left side at axe height
        tree-dmg2.png  stage 2 (<1/3 wood left): foliage thinned
                       ~60%, deep notch + a second bite, lowest
                       branch ring partly gone

All transforms are deterministic (fixed RNG seed) and purely
pixel-level processing of our own art — no new imagery. The
renderer picks the stage by the node's remaining-wood fraction,
exactly like farm growth stages. Re-run after replacing the
source pine:

    python3 tools/art/tree_widen.py
"""
import os

import numpy as np
from PIL import Image, ImageFilter

HERE = os.path.dirname(os.path.abspath(__file__))
NODES = os.path.join(HERE, '..', '..', 'web-preview',
                     'assets-original', 'nodes')
CANVAS_W = 300
TRUNK = (137, 162)      # centre copy's trunk x-range on the canvas
FOLIAGE_BOTTOM = 305    # side copies keep only rows above this


def widen(src):
    """Three-copy composite on a CANVAS_W-wide canvas."""
    out = Image.new('RGBA', (CANVAS_W, src.height))
    # Flank copies are SMALLER (78%) and planted lower, so their
    # tops sit well under the centre top and their canopies merge
    # into the centre tree's lower canopy — one wide tree, not
    # three tops in a row. Foliage only: their trunks are masked
    # away, the centre copy owns the one visible trunk.
    fw = int(src.width * 0.78)
    fh = int(src.height * 0.78)
    flank = src.resize((fw, fh), Image.LANCZOS)
    fa = np.asarray(flank).copy()
    fa[int(FOLIAGE_BOTTOM * 0.78):, :, 3] = 0
    flank = Image.fromarray(fa)
    fy = src.height - fh  # planted on the same ground line
    out.alpha_composite(flank, (2, fy))
    out.alpha_composite(flank, (CANVAS_W - fw - 2, fy))
    out.alpha_composite(src, ((CANVAS_W - src.width) // 2, 0))
    return out


def thin(img, frac, rng):
    """Erase a deterministic share of the foliage pixels."""
    a = np.asarray(img).copy()
    foliage = (a[:, :, 3] > 8) & (np.arange(a.shape[0])[:, None]
                                  < FOLIAGE_BOTTOM)
    # Patchy erosion: a coarse noise grid upsampled by repetition,
    # so whole tufts vanish instead of a uniform dither.
    gh, gw = a.shape[0] // 6 + 1, a.shape[1] // 6 + 1
    noise = rng.random((gh, gw))
    noise = np.kron(noise, np.ones((6, 6)))[:a.shape[0], :a.shape[1]]
    # Blur the block grid into soft blobs so tufts vanish as
    # patches, not as a checkerboard.
    nimg = Image.fromarray((noise * 255).astype(np.uint8))
    nimg = nimg.filter(ImageFilter.GaussianBlur(5))
    noise = np.asarray(nimg).astype(np.float64) / 255.0
    # Threshold by quantile over the foliage itself, so `frac`
    # is the share of foliage removed regardless of the blurred
    # noise's value distribution.
    thr = np.quantile(noise[foliage], frac)
    kill = foliage & (noise <= thr)
    a[kill, 3] = 0
    return Image.fromarray(a)


def notch(img, x0, x1, y0, y1, pale=True):
    """Cut a wedge out of the trunk and paint the fresh wood."""
    a = np.asarray(img).copy()
    a[y0:y1, x0:x1, 3] = 0
    if pale:
        # Fresh-cut wood: a pale vertical sliver at the cut's
        # inner edge, only where the trunk still stands.
        edge = a[y0:y1, x1, 3] > 8
        ys = np.arange(y0, y1)[edge]
        a[ys, x1, :3] = (217, 179, 128)
        a[ys, x1, 3] = 255
    return Image.fromarray(a)


def main():
    src = Image.open(os.path.join(NODES, 'tree-source.png')).convert('RGBA')
    base = widen(src)
    base.save(os.path.join(NODES, 'tree.png'))
    print(f"tree.png: {base.width}x{base.height}")

    rng = np.random.default_rng(20261020)
    d1 = thin(base, 0.30, rng)
    d1 = notch(d1, TRUNK[0], TRUNK[0] + 13, 338, 376)
    d1.save(os.path.join(NODES, 'tree-dmg1.png'))
    print(f"tree-dmg1.png: {d1.width}x{d1.height}")

    rng = np.random.default_rng(20261021)
    d2 = thin(base, 0.60, rng)
    d2 = notch(d2, TRUNK[0], TRUNK[0] + 21, 336, 380)
    d2 = notch(d2, TRUNK[1] - 9, TRUNK[1], 318, 340, pale=False)
    d2.save(os.path.join(NODES, 'tree-dmg2.png'))
    print(f"tree-dmg2.png: {d2.width}x{d2.height}")


if __name__ == '__main__':
    main()
