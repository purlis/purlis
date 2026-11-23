#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Install one harness on this Linux or macOS runner, for its recording (T1.1-03) and the real-harness
# scenario legs (T1.1-53): at the version its file
# in `tests/fixtures/harness-versions` pins, from the committed lockfile so npm checks every
# tarball's integrity, or at its latest release. Only this runner's build is installed, with no
# install scripts run, under `$RUNNER_TEMP` (or `$TMPDIR`), never globally. The program is linked
# as `claude`, `codex` or `opencode` into a folder of its own, whose path is the one line printed.
#
#   tools/harness-install.sh claude|codex|opencode pinned|latest
set -euo pipefail

harness=${1-}
at=${2-}
case "$at" in pinned | latest) ;; *) echo "usage: $0 claude|codex|opencode pinned|latest" >&2; exit 2 ;; esac
# The platform package each harness ships is named for the OS and CPU: `linux` or `darwin`, `x64`
# or `arm64`. Codex's program is under the target triple of its own build.
case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=darwin ;;
  *) echo "no harness build for $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  aarch64 | arm64) arch=arm64; cpu=aarch64 ;;
  x86_64) arch=x64; cpu=x86_64 ;;
  *) echo "no harness build for $(uname -m)" >&2; exit 1 ;;
esac
case "$os" in
  linux) triple=$cpu-unknown-linux-musl ;;
  darwin) triple=$cpu-apple-darwin ;;
esac
repo=$(cd "$(dirname "$0")/.." && pwd)
pins="$repo/tests/fixtures/harness-versions"
base="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/harness-install/$harness-$at"
bin="$base/bin"
rm -rf "$base"
mkdir -p "$base/npm" "$bin"

case "$harness" in
  claude) package="@anthropic-ai/claude-code-$os-$arch"; folder=claude-code; pinned=$(sed 's/ .*//' "$pins/claude.txt") ;;
  codex) package="@openai/codex-$os-$arch"; folder=codex; pinned=$(sed 's/^codex-cli //' "$pins/codex.txt") ;;
  opencode) package="opencode-$os-$arch"; folder=opencode; pinned=$(cat "$pins/opencode.txt") ;;
  *) echo "usage: $0 claude|codex|opencode pinned|latest" >&2; exit 2 ;;
esac

if [ "$at" = pinned ]; then
  cp "$pins/$folder/package.json" "$pins/$folder/package-lock.json" "$base/npm/"
  locked=$(jq -r --arg p "node_modules/$package" '.packages[$p].version' "$base/npm/package-lock.json")
  want=$pinned
  [ "$harness" = codex ] && want="$pinned-$os-$arch"
  if [ "$locked" != "$want" ]; then
    echo "::error file=tests/fixtures/harness-versions/$folder/package-lock.json::the lockfile holds $package $locked and $harness.txt says $pinned: set the version in its package.json and run npm install --package-lock-only there" >&2
    exit 1
  fi
  (cd "$base/npm" && npm ci --ignore-scripts --no-audit --no-fund >&2)
else
  # The latest release, as npm's `latest` tag names it.
  case "$harness" in
    claude) version=$(npm view @anthropic-ai/claude-code version); spec="$package@$version" ;;
    codex) version=$(npm view @openai/codex version); spec="$package@npm:@openai/codex@$version-$os-$arch" ;;
    opencode) version=$(npm view opencode-ai version); spec="$package@$version" ;;
  esac
  (cd "$base/npm" && echo '{"private":true}' > package.json && npm install --ignore-scripts --no-audit --no-fund --no-save "$spec" >&2)
fi

modules="$base/npm/node_modules/$package"
case "$harness" in
  claude) program="$modules/claude" ;;
  codex) program="$modules/vendor/$triple/bin/codex" ;;
  opencode) program="$modules/bin/opencode" ;;
esac
[ -x "$program" ] || { echo "$program is not there" >&2; exit 1; }
ln -s "$program" "$bin/$harness"
echo "$harness: $("$bin/$harness" --version | head -1)" >&2
echo "$bin"
