#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **A runner binary is the platform's it is named for, and says this release's version** (#723):
# what `release.yml` holds each `purlis` to before it hands it on as a runner asset, run on the
# platform that built it, so the binary can run.
#
#   tools/runner-binary-check.sh <binary> <target> <version>
#
# <target> is `linux-x86_64`, `linux-aarch64` or `macos-arm64`, as `runnerinstall::Target` names
# them. Exits 1 with a GitHub error line when either is not so.
set -euo pipefail
bin=${1:?the binary}
target=${2:?the runner target}
version=${3:?the release version}
described=$(file -b "$bin")
echo "$described"
case "$target:$described" in
  linux-x86_64:*ELF*x86-64*|linux-aarch64:*ELF*aarch64*|macos-arm64:*Mach-O*arm64*) ;;
  *) echo "::error::the $target runner binary is: $described"; exit 1 ;;
esac
line=$("$bin" --version)
echo "$line"
case "$line" in
  "purlis $version"|"purlis $version "*) ;;
  *) echo "::error::the release is $version and the $target runner binary says: $line"; exit 1 ;;
esac
