# tools/sound-gen — the Skirmish II sound generator

Every sound in the game is **generated** — nothing is recorded from
or sampled out of any other game. The design study (which DE sound
categories exist, what triggers them, what they feel like, and our
recipe for each) lives in
`~/workspace/skirmish-study/sound-de/SOUND-CATALOG.md`.

## Pipeline

```
./build.sh
  ├─ gen_voices.py   (~/.tts-venv python; Kokoro TTS, local model)
  │    renders the 24 acknowledgment clips from ORIGINAL phrases
  │    (one voice per unit class) into work/voice/
  └─ gen_sfx.py      (system python3; numpy + scipy)
       renders all DSP effects (swords, picks, animals, horns,
       stingers, ambience), post-processes the voices, and writes
       web-preview/play/audio/*.wav + manifest.json
```

- Output format: **22050 Hz, mono, 16-bit WAV**, tightly trimmed.
- **Deterministic:** every recipe is seeded; Kokoro CPU inference is
  reproducible for fixed (text, voice, speed). Rerunning `build.sh`
  on the same inputs rewrites identical assets.
- `manifest.json` maps sound keys (`ack.infantry.attack`,
  `work.chop`, …) to their variant files, and legacy cue names to
  sound keys (`cueSounds`). The engine (`web-preview/play/audio.js`)
  loads it lazily after the first user gesture; a missing asset is
  graceful silence and the runtime synth recipes remain as fallback.

## Adding a sound

1. Add the row to the catalog (trigger + character + recipe).
2. Add a recipe function in `gen_sfx.py` (or a phrase slot in
   `gen_voices.py`), register it in `RECIPES`.
3. Run `build.sh`, wire the key in `play/audio.js` / `app.js`,
   extend `play/audio.test.mjs`.
