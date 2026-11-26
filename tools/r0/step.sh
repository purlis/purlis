#!/usr/bin/env bash
# Run a command as a named, timed step: step.sh NAME cmd args…
set -uo pipefail
name=$1; shift
: "${R0_DIR:?R0_DIR is not set}"
mkdir -p "$R0_DIR"
now() { python3 -c 'import time;print("%.3f"%time.time())'; }
iso() { python3 -c 'import sys,datetime;print(datetime.datetime.fromtimestamp(float(sys.argv[1]),datetime.timezone.utc).strftime("%H:%M:%S"))' "$1"; }
echo "$name" > "$R0_DIR/current-step"
s=$(now)
echo "[r0] START $name at $(iso "$s")Z"
"$@"; rc=$?
e=$(now)
echo none > "$R0_DIR/current-step"
secs=$(python3 -c "import sys;print('%.1f'%(float(sys.argv[2])-float(sys.argv[1])))" "$s" "$e")
printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "$(iso "$s")" "$(iso "$e")" "$secs" "$rc" "$s" "$e" >> "$R0_DIR/steps.tsv"
echo "[r0] END   $name at $(iso "$e")Z  took ${secs}s  exit $rc"
exit $rc
