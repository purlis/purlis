#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Run a command, then fail if it left a process of its own behind (#923).
#
#   tools/no-orphans.sh [--reap] [--grace SECONDS] -- COMMAND [ARGS...]
#
# A process counts as left behind when it started while COMMAND ran, belongs to this user,
# is still alive GRACE seconds (default 5) after COMMAND exited, and is not one this script
# started itself. Each one is named with its pid and command line, and the exit status is 1
# even when COMMAND passed. Otherwise the exit status is COMMAND's own.
#
# Under GitHub Actions (`GITHUB_ACTIONS=true`) the list is also written as one `::error`
# annotation titled `Failed: processes left running after the command`: at most twenty
# processes, and at most two hundred characters of each one's command line.
#
# --reap also kills each one, by its pid, never by name. CI passes it: the runner is the
# job's own machine, so every new process on it is the job's. On a machine shared with other
# work, leave it off — a process another session started while COMMAND ran is counted too,
# and killing it would kill someone else's.
#
# Started-while-it-ran is read as pid plus start time, so a pid reused during the run is
# still a new process. `ps -A -o pid=,ppid=,uid=,lstart=,command=` reads the same on Linux
# (procps) and macOS.
set -u

usage() {
  echo "usage: $0 [--reap] [--grace SECONDS] -- COMMAND [ARGS...]" >&2
  exit 2
}

reap=0
grace=5
while [ $# -gt 0 ]; do
  case "$1" in
    --reap) reap=1; shift ;;
    --grace)
      case "${2-}" in
        '' | *[!0-9.]*) usage ;;
      esac
      grace="$2"; shift 2 ;;
    --) shift; break ;;
    -*) usage ;;
    *) break ;;
  esac
done
[ $# -gt 0 ] || usage

me=$(id -u)
here=$$
# On macOS, launchd starts this user's own agents while COMMAND runs (nbagent, mdworker_shared,
# CoreSimulator's services and dozens more on a runner), each with launchd (pid 1) as its parent
# and its program in the system's own folders. They are the system's, not COMMAND's (#2458). A
# process COMMAND left behind is reparented to launchd too, so it is the program's folder that
# tells them apart, read as the kernel's path of its executable (`ps -o comm`), never its
# arguments, which a process writes itself: the sealed system volume (`/System`),
# `/usr/libexec` and CoreSimulator's framework, the ones seen on a runner. Never `/bin`,
# `/usr/bin` or `/usr/sbin`, whose `lsof` and `sysctl` purlis runs itself, nor anything a build
# put there. Elsewhere every process counts, as before.
os=$(uname -s 2>/dev/null || echo unknown)

# Fails closed: a snapshot that could not be taken, or came back empty, is not "nothing new",
# so the check stops with status 3 rather than pass a suite it could not see.
#
# pid ppid uid lstart(5 words) command... -> "pid|ppid|uid|lstart|command". LC_ALL=C, so
# lstart is spelled the same in both snapshots whatever the locale.
table() {
  local raw
  if ! raw=$(LC_ALL=C ps -A -o pid=,ppid=,uid=,lstart=,command=) || [ -z "$raw" ]; then
    echo "no-orphans: ps could not list the processes; nothing was checked" >&2
    exit 3
  fi
  awk '{
    pid=$1; ppid=$2; uid=$3; start=$4" "$5" "$6" "$7" "$8
    cmd=""; for (i=9;i<=NF;i++) cmd=cmd (i>9?" ":"") $i
    print pid "|" ppid "|" uid "|" start "|" cmd
  }' <<<"$raw"
}

before=$(mktemp) || exit 3
programs=$(mktemp) || exit 3
trap 'rm -f "$before" "$programs"' EXIT
first=$(table) || exit 3
awk -F'|' '{print $1 "|" $4}' <<<"$first" >"$before"

"$@"
status=$?

sleep "$grace"

now=$(table) || exit 3
# Each process's executable, as the kernel names it, on macOS only: "pid|path". A list that
# could not be read leaves every process counted.
if [ "$os" = Darwin ]; then
  LC_ALL=C ps -A -o pid=,comm= 2>/dev/null |
    awk '{ p=$1; sub(/^[ \t]*[0-9]+[ \t]+/, ""); print p "|" $0 }' >"$programs" || true
fi
left=$(
  awk -F'|' -v me="$me" -v here="$here" -v os="$os" -v programs="$programs" '
    FNR == NR { seen[$0] = 1; next }
    FILENAME == programs { exe[$1] = $2; next }
    { pid[++rows]=$1; ppid[$1]=$2; uid[$1]=$3; key[$1]=$1 "|" $4; cmd[$1]=$5 }
    END {
      q = here
      for (hops = 0; q != "" && q > 1 && hops < 64; hops++) { above[q] = 1; q = ppid[q] }
      for (r = 1; r <= rows; r++) {
        p = pid[r]
        if (uid[p] != me || (key[p] in seen)) continue
        if (os == "Darwin" && ppid[p] == 1 && (p in exe) &&
            exe[p] ~ /^\/(System|usr\/libexec|Library\/Developer\/PrivateFrameworks\/CoreSimulator\.framework)\//) continue
        # One of this script'"'"'s own (the ps and awk reading the table right now), or one
        # of the processes this script itself runs under.
        q = p; ours = 0
        for (hops = 0; q != "" && q > 1 && hops < 64; hops++) {
          if (q == here) { ours = 1; break }
          q = ppid[q]
        }
        if (!ours && !(p in above)) print p "|" cmd[p]
      }
    }' "$before" "$programs" - <<<"$now"
)

if [ -z "$left" ]; then
  exit "$status"
fi

echo "no-orphans: '$*' left these processes running:" >&2
while IFS='|' read -r pid cmd; do
  echo "  pid $pid: $cmd" >&2
done <<<"$left"
# On a GitHub runner, the same list as an error on the run: this exit of 1 is otherwise the
# one red of the `rust` job that names nothing through the checks API, where a failed test is
# named by `tools/slowest-tests.mjs` (`Failed: <binary> <test>`). With how long each process
# has run, which places the test that started it. Workflow command escaping: `%`, CR and LF.
#
# Bounded, since an annotation is read whole through the API and GitHub cuts a long one where
# it likes: the first $listed_most processes, and the first $line_most characters of each
# one's command line. How many more there were is said; the log above names every one in full.
listed_most=20
line_most=200
if [ "${GITHUB_ACTIONS-}" = true ]; then
  said=""
  listed=0
  more=0
  while IFS='|' read -r pid cmd; do
    if [ "$listed" -ge "$listed_most" ]; then
      more=$((more + 1))
      continue
    fi
    listed=$((listed + 1))
    ran=$(LC_ALL=C ps -o etime= -p "$pid" 2>/dev/null | tr -d ' ')
    if [ "${#cmd}" -gt "$line_most" ]; then
      cmd="${cmd:0:$line_most}..."
    fi
    said="${said}pid $pid, running for ${ran:-an unread time}: $cmd"$'\n'
  done <<<"$left"
  if [ "$more" -gt 0 ]; then
    said="${said}and $more more, each named in the job's log"$'\n'
  fi
  said=${said%$'\n'}
  said=${said//'%'/%25}
  said=${said//$'\r'/%0D}
  said=${said//$'\n'/%0A}
  echo "::error title=Failed%3A processes left running after the command::$said"
fi
if [ "$reap" = 1 ]; then
  while IFS='|' read -r pid _; do
    kill -KILL "$pid" 2>/dev/null || true
  done <<<"$left"
  echo "no-orphans: killed them, by pid" >&2
fi
exit 1
