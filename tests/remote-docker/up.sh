#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **Docker on another machine, behind an sshd, for the remote parity job** (#6266, ADR 0123
# Testing): docker:dind as the other machine's engine, an sshd sharing its /var/run on
# 127.0.0.1:2222 with simulated latency and loss, a key in an agent this job's ssh signs with,
# and the parity image of this build pushed to a registry inside dind, so the engine there pulls
# it by digest. Run from the repository's root once the image is built and pushed to this
# machine's own registry: `up.sh <image> <variable>` copies <image> (a tag in this machine's
# Docker) into dind and names its digest there in `$GITHUB_ENV` as <variable>, beside what the
# tests read. Nothing is published but the sshd's port, on loopback.
set -euo pipefail
source_image=${1:?the image to copy into the other machine}
image_variable=${2:?the variable to name its digest there in}
here=$(cd "$(dirname "$0")" && pwd)
docker volume create dind-run >/dev/null
# No TLS port: the engine is reached only through its socket, as a machine's own Docker is.
docker run --detach --privileged --name dind --env DOCKER_TLS_CERTDIR= \
  --volume dind-run:/var/run docker:28-dind@sha256:2a232a42256f70d78e3cc5d2b5d6b3276710a0de0596c145f627ecfae90282ac --host unix:///var/run/docker.sock
docker build --file "$here/sshd.Dockerfile" --tag remote-docker-sshd "$here"
mkdir -p "$RUNNER_TEMP/remote-ssh/authorized"
ssh-keygen -q -t ed25519 -N '' -C remote-parity -f "$RUNNER_TEMP/remote-ssh/id"
cp "$RUNNER_TEMP/remote-ssh/id.pub" "$RUNNER_TEMP/remote-ssh/authorized/pool"
cp "$RUNNER_TEMP/remote-ssh/id.pub" "$RUNNER_TEMP/remote-ssh/authorized/outsider"
docker run --detach --name sshd --cap-add NET_ADMIN --publish 127.0.0.1:2222:22 \
  --volume dind-run:/var/run --volume "$RUNNER_TEMP/remote-ssh/authorized:/etc/ssh/authorized-src:ro" \
  remote-docker-sshd
for _ in $(seq 60); do
  if docker exec dind docker info >/dev/null 2>&1 && ssh-keyscan -p 2222 127.0.0.1 >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
docker exec dind docker info --format '{{.ID}} {{.ServerVersion}}'
# Latency and loss on the other machine's side of every connection to it (QA-15, #598).
docker exec sshd tc qdisc add dev eth0 root netem delay 40ms 10ms loss 1%
# The parity image, by digest, in a registry inside dind.
docker exec dind docker run --detach --publish 127.0.0.1:5000:5000 --name registry \
  registry:3@sha256:01cd312e67026f06f5d89c3e133a4a9adc7cb9eeb9b1194e0b2954dfd770efd2
docker save "$source_image" | docker exec -i dind docker load
docker exec dind docker push "$source_image"
image=$(docker exec dind docker image inspect --format '{{index .RepoDigests 0}}' "$source_image")
echo "image there: $image"
# ssh signs with an agent, as a person's does.
eval "$(ssh-agent -s -a "$RUNNER_TEMP/remote-ssh/agent.sock")"
ssh-add "$RUNNER_TEMP/remote-ssh/id"
{
  echo "SSH_AUTH_SOCK=$SSH_AUTH_SOCK"
  echo "$image_variable=$image"
  echo "REMOTE_DOCKER_TEST_ROUTE=ssh://pool@127.0.0.1:2222"
  echo "REMOTE_DOCKER_TEST_OUTSIDER_ROUTE=ssh://outsider@127.0.0.1:2222"
  echo "REMOTE_DOCKER_TEST_SSHD=sshd"
  echo "REMOTE_DOCKER_TEST_DIND=dind"
  echo "REMOTE_DOCKER_TEST_MAY_WRITE_KNOWN_HOSTS=1"
} >> "$GITHUB_ENV"
