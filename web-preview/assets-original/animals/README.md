# Animal Art — Base44 Pose Sets

These animal sprites are **Skirmish II's own original art**, generated
with AI image generation on **Claudio Olmedo's Base44 account on
2026-10-18** (app "Skirmish II Art"), from this project's standardized
prompts (archived in the transfer repo
`github.com/claudioolmedo/base44-transfer`, `PROMPTS.md`). Clean-room:
original drawings after real animals, never copies of any other
game's sprites or art.

Each animal has **one generated strip of four poses**, sliced into
frames that share one bottom-aligned canvas:

| Pose | Meaning |
|---|---|
| `*-pose1.png` | Idle, standing |
| `*-pose2.png` | Walk stride A |
| `*-pose3.png` | Walk stride B |
| `*-pose4.png` | Head down: graze (sheep/deer), root (boar) |

Animals: `sheep-pose1..4.png`, `boar-pose1..4.png`,
`deer-pose1..4.png`. The generated poses face **left**; the renderer
draws them unflipped when the animal moves left and flipped
horizontally when it moves right. Chroma-keyed (flat magenta
background in the raw generations, keyed out in processing), RGBA.
The vector animal draws remain the fallback when the poses are not
loaded.
