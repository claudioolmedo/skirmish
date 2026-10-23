#!/bin/bash
export PATH="$HOME/.cargo/bin:$PATH"
cd ~/workspace/skirmish-castle1-rebase
LOG=~/workspace/skirmish-castle1-rebase/cert_final.log
run() { # name, command...
  local name=$1; shift
  if grep -q "^DONE $name " $LOG 2>/dev/null; then return; fi
  echo "=== $name ===" >> $LOG
  "$@" >> $LOG 2>&1
  echo "DONE $name exit $?" >> $LOG
}
run lib cargo test -p skirmish-sim --lib
run determinism cargo test -p skirmish-sim --test determinism
run castle_age cargo test -p skirmish-sim --test castle_age
run construction cargo test -p skirmish-sim --test construction
run wolf cargo test -p skirmish-sim --test wolf
run orders_stances cargo test -p skirmish-sim --test orders_stances
run score cargo test -p skirmish-sim --test score
run sheep_boar cargo test -p skirmish-sim --test sheep_boar
run world cargo test -p skirmish-sim --test world
run two_player cargo test -p skirmish-sim --test two_player
run auto_explore cargo test -p skirmish-sim --test auto_explore
echo ALL_CERT_DONE >> $LOG
