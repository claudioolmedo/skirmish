#!/bin/sh
# Skirmish II — sound asset build. Renders the whole generated
# sound set (voices + DSP effects) into web-preview/play/audio/
# with its manifest.json. Seeded and reproducible: rerunning on a
# clean checkout yields the same files.
#
#   tools/sound-gen/build.sh
set -e
cd "$(dirname "$0")"
TTS_PY="${TTS_PY:-$HOME/.tts-venv/bin/python}"
"$TTS_PY" gen_voices.py
python3 gen_sfx.py
