#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Run one set of the sandbox scenarios (M74.1, T1.1-09 and T1.1-54), as
# `.github/actions/sandbox-scenarios` does on a pull request and `.github/actions/nightly` does at
# night.
#
#   tools/sandbox-scenarios.sh suite|production-timings
#
# - `suite`: every scenario but the production-timing entries.
# - `production-timings`: the entries run at `Timings::production()`: the derived set, less
#   M-NET-25 (`production_nightly_only`) unless this is the nightly (`SCENARIOS_TRIGGER=nightly`).
#
# **No retries** (`--retries 0`, over the `ci` profile's one): a scenario that passes only on a
# second try is red. Retry-to-green is forbidden.
#
# **Repeated runs, compared** (T1.1-54). `REPEAT=3` runs the set three times, each time with its
# batches (a test's module path) in an order `tools/scenario-legs.mjs shuffle` seeds from
# `SHUFFLE_SEED` and the run's number, never its cells (they depend on earlier cells' state). A
# cell whose verdict differs between runs is flaky: it opens a public `flaky-scenario` issue, and
# the run is red (`tools/scenario-legs.mjs flaky`). `STRESS=true` runs the set 20 times under
# `stress-ng` on every CPU (the `stress-x20` label), over the batches this change touched when it
# touched any (`BASE`, the commit it is measured against) and never M-NET-25 (Q300), and compares
# them the same way. Every run's own report is kept under `target/sandbox-scenarios-runs/`; the
# reports a leg uploads are the first run's.
#
# `CARGO_PACKAGES` is the packages the test build was built with (`-p purlis-app -p purlis-scenarios`
# unset): the run names the same ones, so it reuses that build. Every run prints all it says:
# nothing runs out of sight (Q301).
#
# The scenarios are a `#[cfg(test)]` module of purlis-app, `scenarios` (ticket 06); the suite also
# runs the resolver seam's and local origins' own tests (`purlis-scenarios`'s `resolver_seam`,
# T1.1-02), which bind local ports, so they run on each OS's leg. Each run writes
# what a red run's reader needs (event logs, transcripts, the scenario report) under
# `target/sandbox-scenarios/`, which the workflow uploads when a run fails.
set -euo pipefail

case "${1-}" in
  suite) filter='(package(purlis-app) & test(/^scenarios::/) & not test(/^scenarios::production_timings::/)) | binary_id(purlis-scenarios::resolver_seam)' ;;
  production-timings) filter='package(purlis-app) & test(/^scenarios::production_timings::/)' ;;
  *) echo "usage: $0 suite|production-timings" >&2; exit 2 ;;
esac

# The tools are found beside this script; the reports are under the folder it runs in.
here=$(cd "$(dirname "$0")" && pwd)
report=target/sandbox-scenarios
runs=target/sandbox-scenarios-runs
mkdir -p "$report"
rm -rf "$runs"

repeat=${REPEAT:-1}
source=nightly
if [ "${STRESS-}" = true ]; then
  repeat=20
  source=stress-x20
  # M-NET-25 never runs under the multiplier (Q300): its 15 minutes would be twenty times that.
  filter="($filter) & not test(/m_net_25/)"
  # The batches this change touched, when it touched any.
  # Only when they pick a test: a batch's tests are found by its name, and a change to cells no
  # test is named for stresses the whole suite rather than nothing.
  if [ -n "${BASE-}" ]; then
    changed=$(node "$here/scenario-legs.mjs" changed-batches "$BASE")
    if [ -n "$changed" ]; then
      # shellcheck disable=SC2086
      if cargo nextest list ${CARGO_PACKAGES:--p purlis-app -p purlis-scenarios} --locked --profile ci -E "($filter) & $changed" --message-format oneline 2>/dev/null | grep -q .; then
        filter="($filter) & $changed"
        echo "the batches this change touched: $changed"
      else
        echo "::notice title=stress-x20::this change touches cells no test is named for: the whole suite runs"
      fi
    fi
  fi
fi

# Unquoted on purpose: `--workspace`, or `-p` and a package.
# shellcheck disable=SC2086
once() {
  local status=0 junit=target/nextest/ci/junit.xml
  # nextest rewrites its JUnit file on every call: each call's own is kept under the run's folder
  # (`$2` is the run's number, `$3` names the call), and `flaky` reads them all, so a run's verdicts
  # are every batch's and never the last one's.
  rm -f "$junit"
  cargo nextest run ${CARGO_PACKAGES:--p purlis-app -p purlis-scenarios} --locked --profile ci --retries 0 -E "$1" || status=$?
  if [ "$repeat" -gt 1 ] && [ -f "$junit" ]; then
    mkdir -p "$runs/run-$2"
    cp "$junit" "$runs/run-$2/junit-$3.xml"
  fi
  return "$status"
}

# One pass over the set: in the order of `$1`'s run number when `SHUFFLE_SEED` is set, otherwise as
# nextest finds the tests. Returns the worst status of its batches.
# shellcheck disable=SC2086
pass() {
  local n=$1 status=0 batch k=0
  if [ -z "${SHUFFLE_SEED-}" ]; then
    once "$filter" "$n" all || status=$?
    return "$status"
  fi
  local names
  names=$(cargo nextest list ${CARGO_PACKAGES:--p purlis-app -p purlis-scenarios} --locked --profile ci -E "$filter" --message-format oneline | awk '{print $NF}')
  if [ -z "$names" ]; then
    once "$filter" "$n" all || status=$?
    return "$status"
  fi
  while IFS= read -r batch; do
    [ -n "$batch" ] || continue
    echo "::group::run $n, batch $batch"
    k=$((k + 1))
    once "($filter) & test(/^${batch}(::|\$)/)" "$n" "$k" || status=$?
    echo "::endgroup::"
  done < <(printf '%s\n' "$names" | node "$here/scenario-legs.mjs" shuffle "$((SHUFFLE_SEED + n))")
  return "$status"
}

# Each status is returned by hand: `if runs` below turns `set -e` off inside the function.
# Every run happens, red or not: a verdict that differs between runs is what the comparison below
# is for, and a red one is what makes the run red. Each run's report is moved to `$runs/run-<n>`
# as it ends, and what the folder held before is put back with the first run's beside it.
runs() {
  local stress=0 status=0 i aside f
  aside=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/scenarios-aside.XXXXXX")
  mv "$report"/*.jsonl "$aside"/ 2>/dev/null || true
  if [ "${STRESS-}" = true ]; then
    stress-ng --cpu 0 --quiet &
    stress=$!
  fi
  for i in $(seq 1 "$repeat"); do
    [ "$repeat" -eq 1 ] || echo "::group::run $i of $repeat${STRESS:+ under load}"
    pass "$i" || status=$?
    [ "$repeat" -eq 1 ] || echo "::endgroup::"
    mkdir -p "$runs/run-$i"
    mv "$report"/*.jsonl "$runs/run-$i"/ 2>/dev/null || true
  done
  [ "$stress" -eq 0 ] || kill "$stress" 2>/dev/null || true
  mv "$aside"/* "$report"/ 2>/dev/null || true
  for f in "$runs"/run-1/*.jsonl; do
    [ -e "$f" ] && cat "$f" >> "$report/$(basename "$f")"
  done
  if [ "$repeat" -gt 1 ]; then
    node "$here/scenario-legs.mjs" flaky "$runs" --source "$source" || status=$?
  fi
  return "$status"
}

runs
