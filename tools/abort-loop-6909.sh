#!/bin/bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
# TEMPORARY (#6909): abort-loop-6909.sh <test binary> <runs> <parallel>. Runs the abort test many
# times, each under a 60 s watchdog that samples a stuck probe and its companion, and lists the
# runs that failed or hung.
T=$1; N=$2; J=$3
OUT=$(mktemp -d "${TMPDIR:-/tmp}/abortloop.XXXX"); export T OUT
one() {
  "$T" an_aborts_stack_starts_where_it_aborted_not_in_the_crash_handler > "$OUT/$1.txt" 2>&1 & p=$!
  for _ in $(seq 1 600); do kill -0 $p 2>/dev/null || break; sleep 0.1; done
  if kill -0 $p 2>/dev/null; then
    { echo "HUNG after 60 s"; ps -o pid,ppid,stat,etime,command -g "$(ps -o pgid= -p $p | tr -d ' ')" 2>/dev/null
      for q in $(pgrep -P $p) $(for c in $(pgrep -P $p); do pgrep -P "$c"; done); do
        echo "--- sample of $q"; sample "$q" 2 2>&1 | sed -n '1,120p'; done; } >> "$OUT/$1.txt"
    pkill -9 -P $p; for c in $(pgrep -P $p); do pkill -9 -P "$c"; done; kill -9 $p
    echo "$1 hung"; return
  fi
  wait $p || { if grep -q "left: 0" "$OUT/$1.txt"; then echo "$1 no-minidump"; else echo "$1 failed"; fi; }
}
export -f one
seq 1 "$N" | xargs -P "$J" -I{} bash -c 'one {}' > "$OUT/bad.txt"
echo "of $N runs: $(awk '{print $2}' "$OUT/bad.txt" | sort | uniq -c | tr '\n' ' ')"
for i in $(awk '{print $1}' "$OUT/bad.txt" | head -12); do echo "::group::run $i ($(grep "^$i " "$OUT/bad.txt" | cut -d' ' -f2))"; cat "$OUT/$i.txt"; echo "::endgroup::"; done
test ! -s "$OUT/bad.txt"
