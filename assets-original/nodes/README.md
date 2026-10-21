# Node Art — Base44 Set

These resource-node sprites are **Skirmish II's own original art**,
generated with AI image generation on **Claudio Olmedo's Base44
account on 2026-10-18** (app "Skirmish II Art"), from this project's
standardized prompts (archived in the transfer repo
`github.com/claudioolmedo/base44-transfer`, `PROMPTS.md`). Clean-room:
original designs after real nature and medieval resource sites, never
copies of any other game's sprites or art.

| File | Node |
|---|---|
| `tree.png` | Tree (pine) — widened composite, see below |
| `tree-source.png` | The original pine generation (pipeline source) |
| `tree-dmg1.png` | Tree, chop-damage stage 1 (notch, ~30% foliage gone) |
| `tree-dmg2.png` | Tree, chop-damage stage 2 (deep notch, ~60% gone) |
| `berry-bush.png` | Berry bush |
| `gold.png` | Gold mine rocks |
| `stone.png` | Stone rocks |

Chroma-keyed (flat magenta background in the raw generations, keyed
out in processing), trimmed to content, RGBA, natural sizes.

The tree set was reworked on 2026-10-20 (forest readability pass):
`tree.png` and the two damage stages are deterministic pixel-level
derivatives of the original pine (`tree-source.png`), produced by
`tools/art/tree_widen.py` — three layered copies widen the canopy so
neighbouring tiles overlap into a forest mass, and the stages key
to the node's remaining-wood fraction in the renderer, like farm
growth states. Re-run the script after replacing the source. The
renderer prefers these over the CC0 placeholder sprites, on the same
size/anchor draw contract the placeholders used; placeholders and the
vector draws remain the fallback chain.
