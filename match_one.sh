#!/bin/bash
B=./target/debug/deps/match-34c4202f0f844408
for t in opponent_start_is_mirrored_and_complete ai_economy_grows_without_human_commands passive_human_loses_the_match ai_raises_army_and_wave_engages_the_human ai_match_replays_bit_identically scripted_siege_wins_the_match; do
  if grep -q "^$t: " ~/workspace/skirmish-castle1-rebase/match_each.log 2>/dev/null; then continue; fi
  timeout 1500 $B --exact $t --test-threads=1 > /tmp/mt.txt 2>&1
  echo "$t: exit $? $(grep -oE 'test result: [a-z]+' /tmp/mt.txt | head -1)" >> ~/workspace/skirmish-castle1-rebase/match_each.log
done
echo MATCH_EACH_DONE >> ~/workspace/skirmish-castle1-rebase/match_each.log
