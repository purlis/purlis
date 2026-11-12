#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **A command run as this account, as from a machine other than the engine's** (#6269):
# `tests/remote-podman/up.sh`'s Podman, the "other machine", runs on this runner's disk. The
# command runs in a mount namespace of its own with an empty folder over the other machine's init
# (`$REMOTE_PODMAN_TEST_INIT`'s folder), as a client whose catatonit is elsewhere or nowhere (a
# Mac) sees it: a check that judges that machine's file here refuses every runner there.
# This side's own files cannot be hidden from the engine so: this machine's rootless `podman`
# client joins its pause process's namespaces and would lose the forward's socket with them. So
# where the run's seccomp profile was kept is read from the container instead (the route's test).
# `as-another-machine.sh <command> [args]...`, from the repository's root.
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
