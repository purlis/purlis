#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Run a `cargo test` that picks tests by name, then fail if a name picked no test (#3752).
#
#   tools/tests-ran.sh -- cargo test ... [-- NAME... and libtest's flags]
#
# `cargo test -p purlis-core --test it -- a_module::` passes green when no test's name holds
# `a_module::`: libtest runs zero tests and says ok. Before #3613 a step named a test binary
# (`--test a_module`), and a renamed file failed it loudly; now a step names a module inside
# the crate's one binary, and a renamed file would quietly stop running. So each NAME after
# the last `--` must pick at least one test that ran (`test <name containing NAME> ...`, not
# `... ignored`). With no NAME, the run as a whole must have run a test.
#
# libtest's flags are not names; those that take a value (`--skip X`, `--test-threads N`, ...)
# skip it too. The command's output is passed on as it comes. Its exit status is kept when it
# failed; otherwise it is 1 when a NAME picked nothing, else 0. Under GitHub Actions each such
# NAME is also an `::error` annotation.
set -u

[ "${1-}" = "--" ] || { echo "usage: $0 -- COMMAND [ARGS...]" >&2; exit 2; }
shift
[ $# -gt 0 ] || { echo "usage: $0 -- COMMAND [ARGS...]" >&2; exit 2; }

# The names: what follows the command's last `--`, less libtest's flags and their values.
last=0
i=0
for arg in "$@"; do
  i=$((i + 1))
  [ "$arg" = "--" ] && last=$i
done
names=()
if [ "$last" -gt 0 ]; then
  skip_value=0
  for arg in "${@:last+1}"; do
    if [ "$skip_value" = 1 ]; then skip_value=0; continue; fi
    case "$arg" in
      --skip | --test-threads | --format | --color | --logfile | --shuffle-seed | -Z) skip_value=1 ;;
      -*) ;;
      *) names+=("$arg") ;;
    esac
  done
fi

log=$(mktemp "${TMPDIR:-/tmp}/tests-ran.XXXXXX") || { echo "tests-ran: no temporary file" >&2; exit 2; }
trap 'rm -f "$log"' EXIT
"$@" | tee "$log"
status=${PIPESTATUS[0]}
[ "$status" = 0 ] || exit "$status"

say() {
  echo "tests-ran: $1" >&2
  if [ "${GITHUB_ACTIONS-}" = "true" ]; then
    echo "::error title=A test filter picked no test::$1"
  fi
}

# A test that ran: libtest's `test <name> ... ` line, unless its verdict there is `ignored`.
ran=$(grep -E '^test .+ \.\.\. ' "$log" | grep -vE ' \.\.\. ignored(, .*)?$' | sed -E 's/^test (.+) \.\.\. .*$/\1/')

if [ -z "${names[*]-}" ]; then
  if [ -z "$ran" ]; then
    say "the command ran no test: $*"
    exit 1
  fi
  exit 0
fi

failed=0
for name in ${names[@]+"${names[@]}"}; do
  if ! printf '%s\n' "$ran" | grep -qF -- "$name"; then
    say "\`$name\` picked no test that ran; was its file or test renamed?"
    failed=1
  fi
done
exit "$failed"
