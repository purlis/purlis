#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
#
# **The theme schema only grows since the last release** (#2524, spec #2313).
#
#     tools/theme-schema-since-release.sh <the test command...>
#
# Within a format the theme schema only grows. `the_published_schema_is_the_one_the_engine_generates`
# holds the engine's schema to the one committed in the working tree; this holds it to the one
# the last release published, as that release's tag holds it, so an edit of the committed file
# cannot carry a removal or a narrowing past the check.
#
# The last release is the highest `vX.Y.Z` tag on `origin` (`dev` and pre-release tags are not
# releases); PURLIS_RELEASE_TAG names another tag or ref to compare with. The tag's
# `docs/schema/theme.v2.schema.json` is fetched with git and written to a file, and the test
# command runs with PURLIS_THEME_SCHEMA_RELEASED naming it.
#
# It passes without running the command only when no release has published format 2's schema
# yet: the last release holds neither the file nor `crates/purlis-core/src/theme/published.rs`,
# the module that publishes it. A release that holds the module and not the file fails, and so
# does purlis/purlis listing no release tag at all; any other git failure fails too. In another
# repository (a fork with no tags) no release tag passes, and the notice says so.
#
# CI's `rust` job runs it with
#     cargo nextest run ... --run-ignored only -E 'test(/the_schema_only_grows_since_the_last_release$/)'
# and on a machine without nextest
#     tools/theme-schema-since-release.sh cargo test -p purlis-core --lib \
#       the_schema_only_grows_since_the_last_release -- --ignored
set -euo pipefail

file=docs/schema/theme.v2.schema.json
module=crates/purlis-core/src/theme/published.rs

if [ "$#" -eq 0 ]; then
  echo "usage: tools/theme-schema-since-release.sh <the test command...>" >&2
  exit 2
fi

say() {
  echo "::notice title=Theme schema since the last release::$1"
}

fail() {
  echo "::error title=Theme schema since the last release::$1"
  exit 1
}

tag="${PURLIS_RELEASE_TAG:-}"
if [ -z "$tag" ]; then
  tag=$(git ls-remote --tags --refs origin 'v*' \
    | sed 's|.*refs/tags/||' \
    | { grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' || true; } \
    | sort -V \
    | tail -n 1)
  if [ -z "$tag" ]; then
    if [ "${GITHUB_REPOSITORY:-}" = "purlis/purlis" ]; then
      fail "origin lists no vX.Y.Z tag, and purlis/purlis has releases: nothing to compare $file with."
    fi
    say "origin (${GITHUB_REPOSITORY:-this clone}) lists no vX.Y.Z tag, so there is no release to compare $file with. Passes."
    exit 0
  fi
fi

if ! git rev-parse --quiet --verify "$tag^{commit}" > /dev/null; then
  git fetch --quiet --no-tags --depth=1 origin "refs/tags/$tag:refs/tags/$tag"
fi

# `ls-tree` prints a path the tag holds and nothing for one it does not; a bad tag or any other
# failure exits non-zero, which ends the script.
held=$(git ls-tree --name-only "$tag" -- "$file")
if [ -z "$held" ]; then
  publishes=$(git ls-tree --name-only "$tag" -- "$module")
  if [ -n "$publishes" ]; then
    fail "$tag publishes format 2 ($module) but holds no $file."
  fi
  say "$tag has no $file: no release has published format 2's schema yet. Passes."
  exit 0
fi

released="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/theme.v2.schema.$(echo "$tag" | tr -c 'A-Za-z0-9.\n-' '_').json"
git show "$tag:$file" > "$released"
echo "Comparing the engine's theme schema with $tag's $file"
PURLIS_THEME_SCHEMA_RELEASED="$released" "$@"
