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
# `CARGO_PACKAGES` is the packages the test build was built with (`-p purlis-app` unset): the run
# names the same ones, so it reuses that build. Every run prints all it says: nothing runs out of
# sight (Q301).
#
# The scenarios are a `#[cfg(test)]` module of purlis-app, `scenarios` (ticket 06). Each run writes
# what a red run's reader needs (event logs, transcripts, the scenario report) under
# `target/sandbox-scenarios/`, which the workflow uploads when a run fails.
set -euo pipefail

case "${1-}" in
  suite) filter='package(purlis-app) & test(/^scenarios::/) & not test(/^scenarios::production_timings::/)' ;;
  production-timings) filter='package(purlis-app) & test(/^scenarios::production_timings::/)' ;;
  *) echo "usage: $0 suite|production-timings" >&2; exit 2 ;;
esac

mkdir -p target/sandbox-scenarios
# Unquoted on purpose: `--workspace`, or `-p` and a package.
# shellcheck disable=SC2086
once() { cargo nextest run ${CARGO_PACKAGES:--p purlis-app} --locked --profile ci --retries 0 -E "$filter"; }

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

runs
