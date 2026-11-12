#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Waits for dind's socket, lets `pool` use it by its group, puts the keys in place, and runs sshd in the foreground as
# PID 1, so `kill -HUP 1` makes it read its host key again.
set -eu
i=0
until [ -S /var/run/docker.sock ]; do
  i=$((i + 1))
  [ "$i" -lt 120 ] || { echo "no docker.sock" >&2; exit 1; }
  sleep 1
done
gid=$(stat -c %g /var/run/docker.sock)
group=$(getent group "$gid" | cut -d: -f1 || true)
if [ -z "$group" ]; then
  addgroup -g "$gid" dockersock
  group=dockersock
fi
addgroup pool "$group"
chmod 0660 /var/run/docker.sock
# The keys each user signs in with, owned by root as sshd's StrictModes wants them.
mkdir -p /etc/ssh/authorized
cp /etc/ssh/authorized-src/* /etc/ssh/authorized/
chown -R root:root /etc/ssh/authorized
chmod 0755 /etc/ssh/authorized
chmod 0644 /etc/ssh/authorized/*
exec /usr/sbin/sshd -D -e
