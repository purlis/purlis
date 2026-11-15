#!/bin/bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
# TEMPORARY (#6909): abort-loop-6909.sh <probe> <runs> <parallel> [how]. Runs the probe's abort
# many times and counts the runs that left no minidump, at once and 10 seconds later.
P=$1; N=$2; J=$3; HOW=${4:-abort}
OUT=$(mktemp -d "${TMPDIR:-/tmp}/abortloop.XXXX")
one() { d="$OUT/$1"; mkdir -p "$d"
  CRASH_PROBE_CRASHES="$d" "$P" "$HOW" 2>"$d.err" >/dev/null; st=$?
  now=$(ls "$d" 2>/dev/null | grep -c '\.dmp$'); echo "$1 $st $now"; }
export -f one; export P HOW OUT
seq 1 "$N" | xargs -P "$J" -I{} bash -c 'one {}' > "$OUT/results"
sleep 10
miss=0; late=0
while read -r i st now; do later=$(ls "$OUT/$i" 2>/dev/null | grep -c '\.dmp$')
  if [ "$now" != 1 ]; then
    if [ "$later" = 1 ]; then late=$((late+1)); echo "LATE $i st=$st"
    else miss=$((miss+1)); echo "::group::MISS $i st=$st files: $(ls "$OUT/$i" | tr '\n' ' ')"; cat "$OUT/$i.err"; echo "::endgroup::"; fi
  fi
done < "$OUT/results"
echo "::group::a run that left one"; cat "$OUT/1.err"; echo "::endgroup::"
echo "statuses: $(awk '{print $2}' "$OUT/results" | sort | uniq -c | tr '\n' ' ')"
echo "runs=$N late=$late miss=$miss"
test "$miss" -eq 0 && test "$late" -eq 0
