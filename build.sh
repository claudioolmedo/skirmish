#!/usr/bin/env bash
# Builds the Skirmish sim crate to WebAssembly for the debug preview.
#
# Output: web-preview/pkg/ (JS glue + .wasm), loaded by web-preview/app.js.
# Serve the preview statically, e.g.:  cd web-preview && python3 -m http.server 8080
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
fi

if ! command -v wasm-pack >/dev/null 2>&1; then
    echo "wasm-pack not found — installing with cargo (one-time)..."
    cargo install wasm-pack
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
# NOTE: wasm-pack resolves --out-dir relative to the *crate* directory, so
# pass an absolute path.
wasm-pack build sim --target web --out-dir "$ROOT/web-preview/pkg" --out-name skirmish_sim

echo "OK: web-preview/pkg/ ready."
echo "Serve with: cd web-preview && python3 -m http.server 8080"
echo "Then open:  http://localhost:8080/"
