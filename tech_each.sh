#!/bin/bash
cd ~/workspace/skirmish-castle1-rebase
B=./target/debug/deps/tech-e61828dd7384dca0
LOG=~/workspace/skirmish-castle1-rebase/tech_each.log
TESTS=$($B --list 2>/dev/null | sed 's/: test$//' | grep -E '^[a-z_0-9]+$')
for t in $TESTS; do
  if grep -q "^$t: " $LOG 2>/dev/null; then continue; fi
  timeout 3000 $B --exact "$t" --test-threads=1 > ~/workspace/skirmish-castle1-rebase/tt_out.txt 2>&1
  echo "$t: exit $? $(grep -oE '[0-9]+ passed' ~/workspace/skirmish-castle1-rebase/tt_out.txt | head -1)" >> $LOG
done
echo TECH_EACH_DONE >> $LOG
