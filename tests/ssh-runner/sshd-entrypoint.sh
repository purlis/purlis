#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Puts the keys each user signs in with in place, owned by root as sshd's StrictModes wants
# them, and runs sshd in the foreground as PID 1.
set -eu
mkdir -p /etc/ssh/authorized
cp /etc/ssh/authorized-src/* /etc/ssh/authorized/
chown -R root:root /etc/ssh/authorized
chmod 0755 /etc/ssh/authorized
chmod 0644 /etc/ssh/authorized/*
exec /usr/sbin/sshd -D -e
