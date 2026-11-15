#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **A machine without Docker, behind an sshd, for RR-14's install test** (#723): the image of
# `sshd.Dockerfile` on 127.0.0.1:2224 with simulated latency and loss, a key in an agent this
# job's ssh signs with (the one `tests/remote-docker/up.sh` started, where it did), and this
# build's purlis, stripped of its debug information, named for the test. Run from the
# repository's root once `purlis` is built: `up.sh <purlis>`.
set -euo pipefail
purlis=${1:?the purlis binary of this build}
here=$(cd "$(dirname "$0")" && pwd)
dir="$RUNNER_TEMP/ssh-runner"
mkdir -p "$dir/authorized"
docker build --file "$here/sshd.Dockerfile" --tag ssh-runner-sshd "$here"
ssh-keygen -q -t ed25519 -N '' -C ssh-runner -f "$dir/id"
cp "$dir/id.pub" "$dir/authorized/runner"
cp "$dir/id.pub" "$dir/authorized/liar"
docker run --detach --name ssh-runner --cap-add NET_ADMIN --publish 127.0.0.1:2224:22 \
  --volume "$dir/authorized:/etc/ssh/authorized-src:ro" ssh-runner-sshd
for _ in $(seq 60); do
  if ssh-keyscan -p 2224 127.0.0.1 >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
# Latency and loss on the machine's side of every connection to it (QA-15, #598).
docker exec ssh-runner tc qdisc add dev eth0 root netem delay 40ms 10ms loss 1%
# A smaller copy to send twice: the same code, without its debug information.
strip --strip-debug -o "$dir/purlis" "$purlis"
ls -l "$dir/purlis"
# Everything it links is on that machine, or the test would fail far from the reason.
docker cp "$dir/purlis" ssh-runner:/tmp/purlis-check
if docker exec ssh-runner ldd /tmp/purlis-check | grep 'not found'; then
  echo "the SSH runner's image lacks a library purlis links" >&2
  exit 1
fi
docker exec ssh-runner rm /tmp/purlis-check
# ssh signs with an agent, as a person's does: the job's own, where one runs.
if [ -z "${SSH_AUTH_SOCK:-}" ]; then
  eval "$(ssh-agent -s -a "$dir/agent.sock")"
  echo "SSH_AUTH_SOCK=$SSH_AUTH_SOCK" >> "$GITHUB_ENV"
fi
ssh-add "$dir/id"
{
  echo "SSH_RUNNER_TEST_ROUTE=ssh://runner@127.0.0.1:2224"
  echo "SSH_RUNNER_TEST_LIAR_ROUTE=ssh://liar@127.0.0.1:2224"
  echo "SSH_RUNNER_TEST_PURLIS=$dir/purlis"
} >> "$GITHUB_ENV"
