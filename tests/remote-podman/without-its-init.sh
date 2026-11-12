#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **A command run as this account, on this machine, without the other machine's init** (#6269):
# `tests/remote-podman/up.sh` gives the other machine's Podman its `catatonit` in
# `$REMOTE_PODMAN_TEST_INIT`'s folder, which is this runner's disk too. So the command runs in a
# mount namespace of its own with an empty folder over that one, as a client machine whose
# catatonit is elsewhere or nowhere (a Mac) would see it: a check that judged the other
# machine's file here refuses every runner there.
# `without-its-init.sh <command> [args]...`, from the repository's root.
set -euo pipefail
init=${REMOTE_PODMAN_TEST_INIT:?run tests/remote-podman/up.sh first}
[ "$#" -gt 0 ] || { echo "usage: $0 <command> [args]..." >&2; exit 2; }
exec sudo --preserve-env unshare --mount --propagation private -- /bin/sh -c '
  dir=$1 uid=$2 gid=$3
  shift 3
  mount -t tmpfs -o size=1m,mode=0755 tmpfs "$dir"
  [ ! -e "$dir/catatonit" ] || { echo "the other machine'\''s init is still seen here" >&2; exit 1; }
  exec setpriv --reuid="$uid" --regid="$gid" --init-groups -- "$@"
' sh "$(dirname "$init")" "$(id -u)" "$(id -g)" \
  env PATH="$PATH" HOME="$HOME" "$@"
