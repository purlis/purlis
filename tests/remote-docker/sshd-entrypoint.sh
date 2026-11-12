#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Waits for the engine's socket (dind's, or `ENGINE_SOCKET`: a rootless Podman's, which `pool`
# owns by its uid, #6269), lets `pool` use dind's by its group, puts the keys in place, and runs
# sshd in the foreground as PID 1, so `kill -HUP 1` makes it read its host key again.
set -eu
socket=${ENGINE_SOCKET:-/var/run/docker.sock}
i=0
until [ -S "$socket" ]; do
  i=$((i + 1))
  [ "$i" -lt 120 ] || { echo "no $socket" >&2; exit 1; }
  sleep 1
done
if [ "$socket" = /var/run/docker.sock ]; then
  gid=$(stat -c %g /var/run/docker.sock)
  group=$(getent group "$gid" | cut -d: -f1 || true)
  if [ -z "$group" ]; then
    addgroup -g "$gid" dockersock
    group=dockersock
  fi
  addgroup pool "$group"
  chmod 0660 /var/run/docker.sock
fi
# The keys each user signs in with, owned by root as sshd's StrictModes wants them.
mkdir -p /etc/ssh/authorized
cp /etc/ssh/authorized-src/* /etc/ssh/authorized/
chown -R root:root /etc/ssh/authorized
chmod 0755 /etc/ssh/authorized
chmod 0644 /etc/ssh/authorized/*
exec /usr/sbin/sshd -D -e
