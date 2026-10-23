#!/bin/bash
export PATH="$HOME/.cargo/bin:$PATH"
cd ~/workspace/skirmish-castle1-rebase
LANE=$1; shift
for t in "$@"; do
  echo "=== $t ===" >> ~/workspace/skirmish-castle1-rebase/cert_$LANE.log
  cargo test -p skirmish-sim --test $t 2>&1 | grep -E "test result|error" >> ~/workspace/skirmish-castle1-rebase/cert_$LANE.log
  echo "--- $t exit ${PIPESTATUS[0]}" >> ~/workspace/skirmish-castle1-rebase/cert_$LANE.log
done
echo LANE_DONE >> ~/workspace/skirmish-castle1-rebase/cert_$LANE.log
