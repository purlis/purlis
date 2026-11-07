#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# TEMPORARY (#5486): trace2 of the git standing look on the scale run's project, in each state
# its index can be in. Removed before review.
#   tools/scale-diag-5486.sh <project> <out dir>
set -uo pipefail
project=$1
out=$2
mkdir -p "$out"
sp=$PWD/target/release/scale-project
now() { perl -MTime::HiRes=time -e 'printf "%.0f", time*1000'; }
note() { echo "diag | $*" | tee -a "$out/summary.txt"; }

# Every git the look starts reads the global config from here (its environment keeps it), and
# writes one perf trace a process into the folder named.
traced() {
  local name=$1
  mkdir -p "$out/$name" "$out/xdg-$name/git"
  printf '[trace2]\n\tperfTarget = %s\n' "$out/$name" >"$out/xdg-$name/git/config"
}
look() {
  local name=$1
  traced "$name"
  note "$name: $(XDG_CONFIG_HOME="$out/xdg-$name" "$sp" look --project "$project" --times 3)"
}
raw() {
  local name=$1
  shift
  traced "$name"
  local s e
  s=$(now)
  GIT_TRACE2_PERF="$out/$name" git -C "$project" "$@" >/dev/null 2>>"$out/$name.stderr"
  e=$(now)
  note "$name: $((e - s)) ms ($*)"
}

note "git: $(command -v git) $(git version)"
note "fsmonitor before: $(git -C "$project" fsmonitor--daemon status 2>&1)"
note "index: $("$sp" index --project "$project")"
note "untracked after index: $(git -C "$project" status --porcelain -uall | wc -l)"
look a-look-unprimed
note "fsmonitor after first look: $(git -C "$project" fsmonitor--daemon status 2>&1)"
spec=(-- . ':(exclude,literal)skills.local.lock' ':(exclude,glob)workspaces/.throwaway/**')
for n in 1 2; do
  raw "b$n-fsm-nopathspec" -c core.fsmonitor=true --no-optional-locks status --porcelain=v1 -z --untracked-files=all
  raw "b$n-fsm-pathspec" -c core.fsmonitor=true --no-optional-locks status --porcelain=v1 -z --untracked-files=all "${spec[@]}"
  raw "b$n-nofsm-nopathspec" -c core.fsmonitor=false --no-optional-locks status --porcelain=v1 -z --untracked-files=all
  raw "b$n-nofsm-pathspec" -c core.fsmonitor=false --no-optional-locks status --porcelain=v1 -z --untracked-files=all "${spec[@]}"
done
# What planegit::prime runs.
raw c1-prime -c core.fsmonitor=true -c core.untrackedCache=true -c status.showUntrackedFiles=all status --porcelain=v1 -z --untracked-files=all
raw c2-prime-again -c core.fsmonitor=true -c core.untrackedCache=true -c status.showUntrackedFiles=all status --porcelain=v1 -z --untracked-files=all
look d-look-primed
sleep 30
look e-look-primed-30s-later
raw f-fsm-pathspec-primed -c core.fsmonitor=true --no-optional-locks status --porcelain=v1 -z --untracked-files=all "${spec[@]}"
raw g-nofsm-nopathspec-primed -c core.fsmonitor=false --no-optional-locks status --porcelain=v1 -z --untracked-files=all
note "fsmonitor at the end: $(git -C "$project" fsmonitor--daemon status 2>&1)"
git -C "$project" fsmonitor--daemon stop 2>/dev/null || true
exit 0
