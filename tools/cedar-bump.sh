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

# Two commits, both named: an empty argument would read the index (`git show :Cargo.lock`).
if [ "$#" -ne 2 ] || [ -z "$1" ] || [ -z "$2" ]; then
  echo "bumped=true"
  exit 0
fi

# Each cedar package's name, version, source and checksum: a `[patch]` or a git source at the same
# version is a move too.
cedar() {
  git show "$1:Cargo.lock" 2>/dev/null | awk '
    /^\[\[package\]\]/ { if (name != "") print name, version, source, checksum; name = ""; version = ""; source = ""; checksum = ""; next }
    /^name = "cedar-policy/ { name = $3; next }
    name != "" && /^version = / { version = $3; next }
    name != "" && /^source = / { source = $3; next }
    name != "" && /^checksum = / { checksum = $3; next }
    END { if (name != "") print name, version, source, checksum }
  ' | sort
}

base=$(cedar "$1")
head=$(cedar "$2")

if [ -z "$base" ] || [ -z "$head" ] || [ "$base" != "$head" ]; then
  echo "bumped=true"
else
  echo "bumped=false"
fi
