#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **Push the local `benchmarks` branch, over a point another writer pushed meanwhile** (#8014).
# Three jobs record to `benchmarks` (ci.yml's bench-record, stress.yml's stress-record and
# budget-run.yml's budget-record), each through github-action-benchmark with `auto-push: false`,
# which leaves its point as a commit on the local `benchmarks` branch. Nothing orders the three
# across workflows, and one concurrency group for all of them would not: GitHub keeps one pending
# run per group, so a third would cancel the one waiting and lose its point. So each pushes here:
# a push that lost the race fetches the branch, rebases its commit onto it and tries again.
#
# Each writer keeps its own data set, its own `<dir>/data.js`, so the rebase over another
# writer's point is clean. Two points of the same data set at once (two runs of one record job)
# cannot both be kept by a rebase: that is one `::error::` naming the file, and the branch keeps
# the first.
#
#   tools/benchmarks-push.sh      from the checkout the action committed in
#
# `TOKEN`, when set, is handed to git for this push and fetch alone, for a checkout made with
# `persist-credentials: false`. `TRIES` (5) and `WAIT` (seconds, 5) pace the tries.
set -euo pipefail
tries=${TRIES:-5}
wait=${WAIT:-5}
run_git() {
  if [ -n "${TOKEN:-}" ]; then
    local auth
    auth=$(printf 'x-access-token:%s' "$TOKEN" | base64 | tr -d '\n')
    echo "::add-mask::$auth"
    git -c "http.https://github.com/.extraheader=AUTHORIZATION: basic $auth" "$@"
  else
    git "$@"
  fi
}
for attempt in $(seq "$tries"); do
  if run_git push origin refs/heads/benchmarks:refs/heads/benchmarks; then
    exit 0
  fi
  [ "$attempt" -lt "$tries" ] || break
  sleep $((attempt * wait))
  run_git fetch origin +refs/heads/benchmarks:refs/remotes/origin/benchmarks
  if ! git rebase origin/benchmarks benchmarks; then
    clashed=$(git diff --name-only --diff-filter=U | tr '\n' ' ')
    git rebase --abort
    echo "::error title=Benchmark point not recorded::another run pushed a point to ${clashed:-the same data set} first; this run's point was not recorded"
    exit 1
  fi
done
echo "::error title=Benchmark point not recorded::the push to benchmarks was refused $tries times"
exit 1
