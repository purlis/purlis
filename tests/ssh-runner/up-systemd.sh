#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **A machine that boots systemd, behind an sshd, for RR-15's test** (#724): the image of
# `systemd.Dockerfile` on 127.0.0.1:2225, signed in to with the key `up.sh` made and put in this
# job's agent. Run after `up.sh`, from the repository's root.
#
# systemd as a container's init needs the container privileged, with a cgroup namespace of its own
# and `/run` and `/run/lock` on tmpfs, so a restart empties them as a boot does.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
dir="$RUNNER_TEMP/ssh-runner"
test -f "$dir/id.pub" || { echo "run tests/ssh-runner/up.sh first: it makes the key" >&2; exit 1; }
mkdir -p "$dir/authorized-systemd"
cp "$dir/id.pub" "$dir/authorized-systemd/keeper"
docker build --file "$here/systemd.Dockerfile" --tag ssh-runner-systemd "$here"
docker run --detach --name ssh-runner-systemd --privileged --cgroupns=private \
  --tmpfs /run --tmpfs /run/lock --publish 127.0.0.1:2225:22 \
  --volume "$dir/authorized-systemd:/etc/ssh/authorized-src:ro" ssh-runner-systemd
for _ in $(seq 90); do
  if ssh-keyscan -p 2225 127.0.0.1 >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
docker exec ssh-runner-systemd systemctl is-system-running --wait || {
  docker exec ssh-runner-systemd systemctl --failed --no-pager || true
}
# What the image lacks of what purlis links would fail the test far from the reason.
docker cp "$dir/purlis" ssh-runner-systemd:/tmp/purlis-check
if docker exec ssh-runner-systemd ldd /tmp/purlis-check | grep 'not found'; then
  echo "the systemd runner's image lacks a library purlis links" >&2
  exit 1
fi
docker exec ssh-runner-systemd rm /tmp/purlis-check
{
  echo "SSH_RUNNER_SYSTEMD_TEST_ROUTE=ssh://keeper@127.0.0.1:2225"
  echo "SSH_RUNNER_SYSTEMD_TEST_MACHINE=ssh-runner-systemd"
} >> "$GITHUB_ENV"
