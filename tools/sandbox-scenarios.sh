#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Run one set of the sandbox scenarios (M74.1, T1.1-09), as `.github/actions/sandbox-scenarios`
# does on a pull request and `.github/actions/nightly` does at night.
#
#   tools/sandbox-scenarios.sh suite|production-timings
#
# - `suite`: every scenario but the production-timing entries.
# - `production-timings`: the entries run at `Timings::production()` (ticket 54).
#
# **No retries** (`--retries 0`, over the `ci` profile's one): a scenario that passes only on a
# second try is red. Retry-to-green is forbidden.
#
# `STRESS=true` runs the set 20 times under `stress-ng` on every CPU (the `stress-x20` label).
# `QUIET=true` keeps the run's output in `$RUNNER_TEMP` and prints only `ENTRY`'s id and its verdict:
# a red escape entry on the public repository says nothing of how (Q287, Q298).
#
# The scenarios are a `#[cfg(test)]` module of purlis-app, `sandbox_scenarios` (ticket 06). Each run
# writes what a red run's reader needs (event logs, transcripts, the scenario report) under
# `target/sandbox-scenarios/`, which the workflow uploads when a loud run fails.
set -euo pipefail

case "${1-}" in
  suite) filter='package(purlis-app) & test(/^sandbox_scenarios::/) & not test(/^sandbox_scenarios::production_timings::/)' ;;
  production-timings) filter='package(purlis-app) & test(/^sandbox_scenarios::production_timings::/)' ;;
  *) echo "usage: $0 suite|production-timings" >&2; exit 2 ;;
esac

mkdir -p target/sandbox-scenarios
once() { cargo nextest run --locked -p purlis-app --profile ci --retries 0 -E "$filter"; }

# Each status is returned by hand: `if runs` below turns `set -e` off inside the function.
runs() {
  [ "${STRESS-}" = true ] || { once; return; }
  stress-ng --cpu 0 --quiet &
  local stress=$! status=0 i
  for i in $(seq 1 20); do
    echo "::group::run $i of 20 under load"
    once || status=$?
    echo "::endgroup::"
    [ "$status" -eq 0 ] || break
  done
  kill "$stress" 2>/dev/null || true
  return "$status"
}

if [ "${QUIET-}" = true ]; then
  log="${RUNNER_TEMP:?}/sandbox-scenarios-$1.log"
  if runs >"$log" 2>&1; then
    echo "${ENTRY:-$1}: passed"
  else
    echo "::error::${ENTRY:-$1} failed; its output is kept off the public log"
    exit 1
  fi
else
  runs
fi
