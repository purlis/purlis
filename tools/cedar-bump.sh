#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
#
# Whether a change moves a cedar crate (M75 slice 42, Q317 point 12): `tools/cedar-bump.sh <base>
# <head>` compares the `cedar-policy*` packages of Cargo.lock at the two commits and prints
# `bumped=true` or `bumped=false`, the line a workflow appends to $GITHUB_OUTPUT.
#
# It fails closed: a base or head that cannot be read, or a lockfile with no cedar package, says
# `bumped=true`, so the nightly set runs rather than a bump going unproven.
set -u

cedar() {
  git show "$1:Cargo.lock" 2>/dev/null | awk '
    /^name = "cedar-policy/ { name = $3; next }
    name != "" && /^version = / { print name, $3; name = "" }
  ' | sort
}

base=$(cedar "${1:-}")
head=$(cedar "${2:-}")

if [ -z "$base" ] || [ -z "$head" ] || [ "$base" != "$head" ]; then
  echo "bumped=true"
else
  echo "bumped=false"
fi
