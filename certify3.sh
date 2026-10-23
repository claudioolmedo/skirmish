#!/bin/bash
export PATH="$HOME/.cargo/bin:$PATH"
cd ~/workspace/skirmish-castle1-rebase
LOG=~/workspace/skirmish-castle1-rebase/cert_final2.log
run() {
  local name=$1; shift
  if grep -q "^DONE $name " $LOG 2>/dev/null; then return; fi
  echo "=== $name ===" >> $LOG
  "$@" >> $LOG 2>&1
  echo "DONE $name exit $?" >> $LOG
}
run ai_repair cargo test -p skirmish-sim --test ai_repair
run cancel_delete cargo test -p skirmish-sim --test cancel_delete
run combat cargo test -p skirmish-sim --test combat
run deer_push cargo test -p skirmish-sim --test deer_push
run tc_footprint cargo test -p skirmish-sim --test tc_footprint
run walls cargo test -p skirmish-sim --test walls
run input cargo test -p skirmish-sim --test input
run melee_building cargo test -p skirmish-sim --test melee_building
run movement cargo test -p skirmish-sim --test movement
run pace cargo test -p skirmish-sim --test pace
run repair cargo test -p skirmish-sim --test repair
run feudal_techs cargo test -p skirmish-sim --test feudal_techs
run economy cargo test -p skirmish-sim --test economy
echo ALL_CERT2_DONE >> $LOG
