#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **Rootless Podman on another machine, behind an sshd, for the remote parity job** (#6269, ADR
# 0123 Testing): this machine's own rootless Podman as the other machine's engine, its API
# service on its usual socket, and #6266's dockerised sshd (`tests/remote-docker/`) on
# 127.0.0.1:2223 with simulated latency and loss. The sshd's `pool` has this account's uid and
# sees the socket and the Podman store at the very paths Podman names them, so a probe there finds
# rootless Podman's socket, and the runtime config the check reads is the one Podman wrote. Run
# from the repository's root after `tests/remote-docker/up.sh`, whose key and agent it signs in
# with: `up.sh <image> <variable>` pulls <image> (by digest, from this machine's registry) into
# Podman and names it in `$GITHUB_ENV` as <variable>, beside what the tests read. It adds the
# machine as a Podman connection too, as a person's `podman system connection add` would, for
# Pool setup to offer. Nothing is published but the sshd's port, on loopback.
#
# The other machine's Podman binds its init (`--init`'s `catatonit`) from a folder of that
# machine's own, `/opt/remote-podman-init`, which the sshd sees and a test run through
# `tests/remote-podman/as-another-machine.sh` does not, and that test's `/tmp` is its own: so
# what is judged of the other machine's files is judged there, and what its engine reads is kept
# there through the sshd, beside the socket it shares (#6269).
set -euo pipefail
image=${1:?the image, by digest, in the registry here}
image_variable=${2:?the variable to name it in}
here=$(cd "$(dirname "$0")" && pwd)
ssh_dir="$RUNNER_TEMP/remote-ssh"
[ -f "$ssh_dir/id" ] || { echo "run tests/remote-docker/up.sh first: its key signs in here" >&2; exit 1; }
uid=$(id -u)
# A runtime folder of this account's, as a person's login gives one, for Podman's socket.
sudo loginctl enable-linger "$USER"
runtime=/run/user/$uid
for _ in $(seq 30); do [ -d "$runtime" ] && break; sleep 1; done
[ -d "$runtime" ] || { echo "no $runtime" >&2; exit 1; }
export XDG_RUNTIME_DIR=$runtime
# This machine's registry serves plain HTTP on loopback, as the parity job's does.
mkdir -p "$HOME/.config/containers"
printf '%s\n' '[[registry]]' 'location = "localhost:5000"' 'insecure = true' \
  > "$HOME/.config/containers/registries.conf"
# The other machine's init, where only it and its sshd see it: `init_path` for this account's
# Podman, a copy of the catatonit Podman ships with.
init_dir=/opt/remote-podman-init
catatonit=
for found in /usr/libexec/podman/catatonit /usr/bin/catatonit /usr/local/libexec/podman/catatonit; do
  [ -e "$found" ] && { catatonit=$(readlink -f "$found"); break; }
done
[ -n "$catatonit" ] || { echo "no catatonit for Podman's --init" >&2; exit 1; }
sudo install -d -m 0755 "$init_dir"
sudo install -m 0755 "$catatonit" "$init_dir/catatonit"
printf '%s\n' '[containers]' "init_path = \"$init_dir/catatonit\"" \
  > "$HOME/.config/containers/containers.conf"
podman pull --tls-verify=false "$image"
podman info --format '{{.Host.Security.Rootless}} {{.Store.GraphRoot}} {{.Version.Version}}'
graph_root=$(podman info --format '{{.Store.GraphRoot}}')
socket=$runtime/podman/podman.sock
# Podman's API service on its rootless socket, as `systemctl --user enable --now podman.socket`
# serves it; its own process, which the job's end takes down.
mkdir -p "$runtime/podman"
nohup podman system service --time=0 "unix://$socket" > "$RUNNER_TEMP/podman-service.log" 2>&1 &
for _ in $(seq 30); do [ -S "$socket" ] && break; sleep 1; done
[ -S "$socket" ] || { cat "$RUNNER_TEMP/podman-service.log" >&2; exit 1; }
docker build --file "$here/../remote-docker/sshd.Dockerfile" --build-arg "POOL_UID=$uid" \
  --tag remote-podman-sshd "$here/../remote-docker"
docker run --detach --name sshd-podman --cap-add NET_ADMIN --publish 127.0.0.1:2223:22 \
  --env "ENGINE_SOCKET=$socket" \
  --volume "$runtime/podman:$runtime/podman" \
  --volume "$graph_root:$graph_root:ro" \
  --volume "$init_dir:$init_dir:ro" \
  --volume "$ssh_dir/authorized:/etc/ssh/authorized-src:ro" \
  remote-podman-sshd
for _ in $(seq 60); do
  ssh-keyscan -p 2223 127.0.0.1 >/dev/null 2>&1 && break
  sleep 1
done
# Latency and loss on the other machine's side of every connection to it (QA-15, #598).
docker exec sshd-podman tc qdisc add dev eth0 root netem delay 40ms 10ms loss 1%
route="ssh://pool@127.0.0.1:2223$socket"
# The machine as a person's Podman keeps it, with the key file it signs in with.
podman system connection add --identity "$ssh_dir/id" parity-podman "$route"
podman system connection list --format json
{
  echo "$image_variable=$image"
  echo "REMOTE_PODMAN_TEST_ROUTE=ssh://pool@127.0.0.1:2223"
  echo "REMOTE_PODMAN_TEST_SSHD=sshd-podman"
  echo "REMOTE_PODMAN_TEST_INIT=$init_dir/catatonit"
} >> "$GITHUB_ENV"
