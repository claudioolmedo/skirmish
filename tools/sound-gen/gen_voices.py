#!/usr/bin/env python3
"""Skirmish II — voice generation (Phase 2 of the sound pipeline).

Renders the unit acknowledgment lines with the project's local
Kokoro TTS into work/voice/*.wav (24 kHz float). The phrases are
ORIGINAL lines written for this game (see
~/workspace/skirmish-study/sound-de/SOUND-CATALOG.md) — generic
functional RTS acknowledgments, recorded in our own voice slots.
No third-party game's script, audio, or performance is used.

Run with the TTS venv:  ~/.tts-venv/bin/python gen_voices.py
Deterministic: Kokoro CPU inference is reproducible for a fixed
(text, voice, speed) triple, and the triples below never change
without a catalog change.
"""
import json
import os
import sys

import soundfile as sf
from kokoro_onnx import Kokoro

MODEL = os.path.expanduser("~/.cache/kokoro/kokoro-v1.0.onnx")
VOICES = os.path.expanduser("~/.cache/kokoro/voices-v1.0.bin")
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "work", "voice")

# key -> (kokoro voice, speed, [phrase, phrase]) — two variants
# per slot so orders do not repeat verbatim back to back.
SLOTS = {
    "ack.villager.select": ("am_michael", 1.05, ["Yes?", "Ready."]),
    "ack.villager.order": ("am_michael", 1.08, ["Right away.", "On my way."]),
    "ack.villager.attack": ("am_michael", 1.1, ["I'll fight!", "Take that!"]),
    "ack.infantry.select": ("am_adam", 1.02, ["Yes, sir?", "At your command."]),
    "ack.infantry.order": ("am_adam", 1.06, ["Forward!", "As ordered."]),
    "ack.infantry.attack": ("am_adam", 1.08, ["Attack!", "Charge!"]),
    "ack.archer.select": ("af_sarah", 1.08, ["Archer ready.", "Yes?"]),
    "ack.archer.order": ("af_sarah", 1.1, ["Moving.", "To position."]),
    "ack.archer.attack": ("af_sarah", 1.1, ["Loose!", "Fire!"]),
    "ack.cavalry.select": ("am_eric", 1.04, ["Mounted and ready.", "Yes, sir?"]),
    "ack.cavalry.order": ("am_eric", 1.08, ["Riding out!", "At the gallop!"]),
    "ack.cavalry.attack": ("am_eric", 1.1, ["Charge!", "Ride them down!"]),
}


def main() -> int:
    os.makedirs(OUT, exist_ok=True)
    kokoro = Kokoro(MODEL, VOICES)
    manifest = {}
    for key, (voice, speed, phrases) in SLOTS.items():
        files = []
        for n, text in enumerate(phrases, start=1):
            samples, sr = kokoro.create(text, voice=voice, speed=speed, lang="en-us")
            name = f"{key}-{n}.wav"
            sf.write(os.path.join(OUT, name), samples, sr)
            files.append(name)
            print(f"{name}: voice={voice} speed={speed} text={text!r} "
                  f"dur={len(samples) / sr:.2f}s")
        manifest[key] = {"voice": voice, "phrases": phrases, "files": files}
    with open(os.path.join(HERE, "work", "voices.json"), "w") as f:
        json.dump(manifest, f, indent=2)
    print(f"voices: {sum(len(v['files']) for v in manifest.values())} clips")
    return 0


if __name__ == "__main__":
    sys.exit(main())
