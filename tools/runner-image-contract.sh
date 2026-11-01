# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The half of the runner image contract (docs/runner-image.md, #2043) that is read inside an
# image: `tools/runner-image-contract.mjs` runs it there as the image's own user, with
# `sh -s`, so it is plain POSIX sh and needs nothing the contract does not already ask for.
#
# It prints one line per problem and exits 1 when there is any. CONTRACT_ROOT, empty in an
# image, is where its tests put a folder standing for one (tools/runner-image-contract.test.mjs).

root=${CONTRACT_ROOT:-}
found=0
problem() {
  echo "$*"
  found=1
}

if [ "$(id -u)" = 0 ]; then
  problem "the image runs as root; its user has to be another"
fi

purlis="$root/usr/local/bin/purlis"
if [ ! -x "$purlis" ]; then
  problem "there is no executable purlis at /usr/local/bin/purlis"
else
  version=$("$purlis" --version 2>/dev/null)
  case "$version" in
    "purlis "*) ;;
    *) problem "/usr/local/bin/purlis --version did not answer with its version" ;;
  esac
fi

# purlis's plugin, where `purlis serve` finds it beside /usr/local/bin/purlis: a sandboxed
# Claude Code chat is armed with it, and refused without it (#2447).
if [ ! -r "$root/usr/local/lib/purlis/plugin/.claude-plugin/plugin.json" ]; then
  problem "there is no purlis plugin at /usr/local/lib/purlis/plugin, beside /usr/local/bin/purlis"
fi

for tool in git bash; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    problem "there is no $tool on PATH"
  fi
done

if [ ! -f "$root/etc/ssl/certs/ca-certificates.crt" ]; then
  problem "there is no certificate bundle at /etc/ssl/certs/ca-certificates.crt"
fi

if [ -z "${HOME:-}" ] || [ ! -d "$HOME" ] || [ ! -w "$HOME" ]; then
  problem "HOME (${HOME:-unset}) is not a folder the image's user can write"
fi

if [ ! -d "$root/tmp" ] || [ ! -w "$root/tmp" ]; then
  problem "/tmp is not writable; the session host's socket is made there"
fi

# What a build of this image promises beyond the contract, named by its caller (`--carries`):
# purlis's own image carries `gh` and `glab` (#4354), and a team's need not.
for tool in ${CONTRACT_CARRIES:-}; do
  if ! "$tool" --version >/dev/null 2>&1; then
    problem "this image is to carry $tool, and $tool --version did not answer"
  fi
done

for harness in claude codex opencode; do
  if command -v "$harness" >/dev/null 2>&1; then
    problem "the image carries a harness ($harness); a runner gets the harness its profile pins"
  fi
done

exit "$found"
