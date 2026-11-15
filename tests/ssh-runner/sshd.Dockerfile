# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **A machine without Docker, reached over SSH, for RR-14** (#723, ADR 0078 §5): Ubuntu, as the
# runner image is, with OpenSSH's server and no purlis. purlis installs itself there over the
# pinned route as `runner`, into `~/.config/<purlis's folder>/server/<version>/`. `liar` signs in
# with the same key and its sessions find a `sha256sum` that reports another digest first on their
# PATH, standing for a copy changed on the way, which the install must delete and refuse.
# `fisher` signs in with it too and its login shell is fish, which reads no POSIX `sh`: every
# command purlis sends must reach `sh` whatever the login shell is (ADR 0123 as amended
# 2026-11-12).
# `libdbus-1-3` is what purlis links beyond the base image. Its one host key is ed25519. tc gives
# the job its latency and loss (QA-15, #598).
FROM ubuntu:26.04@sha256:6a06111c3b0a8600feadb270aec771dc239bf407b25625f93f44ab00e11f4d3c
RUN apt-get --option Acquire::Retries=5 update \
 && apt-get --option Acquire::Retries=5 install --yes --no-install-recommends \
      openssh-server iproute2 procps ca-certificates libdbus-1-3 fish \
 && rm -rf /var/lib/apt/lists/* \
 && userdel --remove ubuntu \
 && useradd --create-home --shell /bin/bash runner \
 && useradd --create-home --shell /bin/bash liar \
 && useradd --create-home --shell /usr/bin/fish fisher \
 && sed -i -e 's/^runner:!/runner:*/' -e 's/^liar:!/liar:*/' -e 's/^fisher:!/fisher:*/' /etc/shadow \
 && mkdir -p /opt/liar /run/sshd \
 && printf '%s\n' '#!/bin/sh' 'echo "0000000000000000000000000000000000000000000000000000000000000000  $2"' \
      > /opt/liar/sha256sum && chmod 0755 /opt/liar/sha256sum \
 && rm -f /etc/ssh/ssh_host_* \
 && ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_ed25519_key \
 && printf '%s\n' 'HostKey /etc/ssh/ssh_host_ed25519_key' 'PasswordAuthentication no' \
      'KbdInteractiveAuthentication no' 'PermitRootLogin no' 'X11Forwarding no' \
      'AuthorizedKeysFile /etc/ssh/authorized/%u' 'MaxStartups 200:30:400' 'MaxSessions 100' \
      'PerSourcePenalties no' \
      'Match User liar' '    SetEnv PATH=/opt/liar:/usr/local/bin:/usr/bin:/bin' \
      > /etc/ssh/sshd_config
COPY --chmod=0755 sshd-entrypoint.sh /usr/local/bin/sshd-entrypoint.sh
ENTRYPOINT ["/usr/local/bin/sshd-entrypoint.sh"]
