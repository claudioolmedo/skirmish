#!/bin/bash
# Resumable serial cargo runner (bot-loop publish gate, 2026-10-29 18:22 run)
cd "$(dirname "$0")"
mkdir -p cargoGate1822
source "$HOME/.cargo/env" 2>/dev/null
export CARGO_TARGET_DIR=/home/hatch/workspace/.skirmish-target
# Rust-content hash of the tree (hash-reuse rule): record it in the summary
( find sim -name '*.rs' | sort; echo sim/Cargo.toml; echo Cargo.toml 2>/dev/null; echo Cargo.lock 2>/dev/null ) | while read f; do [ -f "$f" ] && sha256sum "$f"; done | sha256sum > cargoGate1822/rusthash.txt
cargo test --manifest-path sim/Cargo.toml --no-run > cargoGate1822/build.log 2>&1
echo BUILD_EXIT=$? >> cargoGate1822/build.log
for t in $(ls sim/tests/*.rs | xargs -n1 basename | sed 's/\.rs$//'); do
  if [ -f "cargoGate1822/$t.exit" ]; then continue; fi
  cargo test --manifest-path sim/Cargo.toml --test "$t" > "cargoGate1822/$t.log" 2>&1
  echo $? > "cargoGate1822/$t.exit"
done
if [ ! -f cargoGate1822/lib.exit ]; then
  cargo test --manifest-path sim/Cargo.toml --lib > cargoGate1822/lib.log 2>&1
  echo $? > cargoGate1822/lib.exit
fi
echo ALLDONE > cargoGate1822/SUMMARY.txt
cat cargoGate1822/rusthash.txt >> cargoGate1822/SUMMARY.txt
for f in cargoGate1822/*.exit; do echo "$f=$(cat $f)" >> cargoGate1822/SUMMARY.txt; done
