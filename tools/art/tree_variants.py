#!/usr/bin/env python3
"""
Tree variety generator (bot-loop front 6, Claudio's order
2026-10-28: "fazer vários tipos diferentes de árvores, para
ficar mais parecido ao DE — todas as árvores são iguais").

Derives three additional tree TYPES from our own pine art
(web-preview/assets-original/nodes/tree-source.png, the same
source tree_widen.py uses) so a forest mixes silhouettes the
way DE forests do. Every variant keeps the closed front-1
contract: same 300x393 canvas, canopy spanning the full canvas
width, single centre trunk/anchor — so the renderer draw math
(TREE_H / TREE_WIDE, trunk anchor) is unchanged and every type
FILLS its tile.

Types (renderer assigns one per tree deterministically from a
position hash — see treeVariant in app.js):
  classic  tree.png (existing, from tree_widen.py — untouched)
  spire    one dominant tall pine + two small saplings low at
           the sides; cooler, deeper green
  broad    three near-equal pines in a row, flat-topped wall
           of foliage (flank trunks masked, centre trunk owns
           the anchor, exactly like widen()); warmer green
  round    a dense, squat, rounded crown with a short trunk;
           lighter yellow-green

Each type also gets the two chop-damage stages, derived with
the same thinning + notch recipe as tree_widen.py (same fixed
seeds), so the tremble -> damage -> fall -> stump sequence is
identical for every type.

All transforms are deterministic and purely pixel-level
processing of our own art — no new imagery. Re-run after
replacing the source pine (and re-run tree_widen.py first for
the classic set):

    python3 tools/art/tree_variants.py
"""
import os
import sys

import numpy as np
from PIL import Image

HERE = os.path.dirname(os.path.abspath(__file__))
NODES = os.path.join(HERE, '..', '..', 'web-preview',
                     'assets-original', 'nodes')
sys.path.insert(0, HERE)
from tree_widen import thin, notch  # noqa: E402  (same recipe)

W, H = 300, 393
FOLIAGE_BOTTOM = 305  # rows above this are foliage (tree_widen)


def foliage_only(im, bottom=FOLIAGE_BOTTOM):
    """Mask everything below the foliage line (scaled coords)."""
    a = np.asarray(im).copy()
    cut = int(bottom * im.height / H)
    a[cut:, :, 3] = 0
    return Image.fromarray(a)


def tint(im, mult, amt):
    """Deterministic per-type colour cast baked into the art."""
    a = np.asarray(im).astype(np.float32)
    m = np.array(mult, dtype=np.float32)
    a[:, :, :3] = np.clip(a[:, :, :3] * ((1 - amt) + amt * m), 0, 255)
    return Image.fromarray(a.astype(np.uint8))


def spire(src):
    out = Image.new('RGBA', (W, H))
    tuft = foliage_only(src.resize((120, 273), Image.LANCZOS))
    out.alpha_composite(tuft, (0, H - 200))
    out.alpha_composite(tuft, (W - 120, H - 200))
    centre = src.resize((190, H), Image.LANCZOS)
    out.alpha_composite(centre, ((W - 190) // 2, 0))
    return tint(out, (0.82, 1.04, 1.18), 0.55)  # cooler/deeper


def broad(src):
    out = Image.new('RGBA', (W, H))
    flank_full = src.resize((168, 362), Image.LANCZOS)
    flank = foliage_only(flank_full)  # flank trunks masked away
    out.alpha_composite(flank, (0, H - 362))
    out.alpha_composite(flank, (W - 168, H - 362))
    centre = src.resize((180, 378), Image.LANCZOS)
    out.alpha_composite(centre, ((W - 180) // 2, 15))
    return tint(out, (1.14, 1.08, 0.80), 0.50)  # warmer


def round_tree(src):
    out = Image.new('RGBA', (W, H))
    main = foliage_only(src).resize((240, 300), Image.LANCZOS)
    out.alpha_composite(main, (30, 0))
    blob = foliage_only(src).resize((170, 212), Image.LANCZOS)
    out.alpha_composite(blob, (-15, 105))
    out.alpha_composite(blob, (W - 155, 105))
    blob2 = foliage_only(src).resize((190, 238), Image.LANCZOS)
    out.alpha_composite(blob2, (55, 120))
    # Short trunk drawn last, only its lower stretch visible
    # below the crown: the classic tree.png's own trunk strip
    # (centre x 137-162, rows 300-393 — a known-good trunk).
    classic = Image.open(os.path.join(NODES, 'tree.png')).convert('RGBA')
    trunk = classic.crop((134, 300, 166, H)).resize((26, 93), Image.LANCZOS)
    out.alpha_composite(trunk, (137, H - 93))
    return tint(out, (1.06, 1.16, 0.88), 0.45)  # lighter


# Trunk x-ranges per type for the damage notch (centre trunk).
TRUNKS = {'spire': (136, 164), 'broad': (136, 164), 'round': (138, 162)}


def main():
    src = Image.open(os.path.join(NODES, 'tree-source.png')).convert('RGBA')
    makers = {'spire': spire, 'broad': broad, 'round': round_tree}
    for name, make in makers.items():
        base = make(src)
        base.save(os.path.join(NODES, f'tree-{name}.png'))
        print(f'tree-{name}.png: {base.width}x{base.height}')
        x0, x1 = TRUNKS[name]
        rng = np.random.default_rng(20261020)
        d1 = thin(base, 0.30, rng)
        d1 = notch(d1, x0, x0 + 13, 338, 376)
        d1.save(os.path.join(NODES, f'tree-{name}-dmg1.png'))
        print(f'tree-{name}-dmg1.png')
        rng = np.random.default_rng(20261021)
        d2 = thin(base, 0.60, rng)
        d2 = notch(d2, x0, x0 + 21, 336, 380)
        d2 = notch(d2, x1 - 9, x1, 318, 340, pale=False)
        d2.save(os.path.join(NODES, f'tree-{name}-dmg2.png'))
        print(f'tree-{name}-dmg2.png')


if __name__ == '__main__':
    main()
