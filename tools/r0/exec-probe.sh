#!/usr/bin/env bash
# What it costs this machine to run an executable it has never seen, which is what a build script
# is: compile 60 distinct tiny programs, then run each once. Appends "label compile_s run_s" to
# $R0_DIR/exec-probe.tsv. Usage: exec-probe.sh LABEL
set -euo pipefail
label=$1
dir=$(mktemp -d)
now() { python3 -c 'import time;print("%.3f"%time.time())'; }
t0=$(now)
for i in $(seq 1 60); do
  printf 'int main(void){return %d %% 2 - %d %% 2;}\n' "$i" "$i" > "$dir/p$i.c"
  cc -o "$dir/p$i" "$dir/p$i.c"
done
t1=$(now)
for i in $(seq 1 60); do "$dir/p$i"; done
t2=$(now)
python3 - "$label" "$t0" "$t1" "$t2" <<'PY' | tee -a "$R0_DIR/exec-probe.tsv"
import sys
l, a, b, c = sys.argv[1], *map(float, sys.argv[2:])
print("%s\t%.2f\t%.2f" % (l, b - a, c - b))
PY
