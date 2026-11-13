# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **The other machine of the remote parity job** (#6266, ADR 0123 Testing): OpenSSH's server,
# sharing docker:dind's /var/run, so /var/run/docker.sock is that engine. `pool` may use Docker
# there and `outsider` may not. Its one host key is ed25519, so the key a client meets is that
# file's. Forwarding is sshd's default, as on a person's machine: `AllowStreamLocalForwarding no`
# or `DisableForwarding yes` would deny the socket's forward (`AllowTcpForwarding no` would not,
# being TCP's alone), and a test sets each for a while (#6349). Every behaviour of the parity job meets it at once, so it takes
# that many unauthenticated connections, each Connect's first contact signing in with nothing,
# which OpenSSH 9.8's per-source penalties would hold against the one address they all come
# from. tc gives the job its latency and loss (QA-15, #598).
#
# `POOL_UID` gives `pool` the uid of the account whose rootless Podman it reaches (#6269), so
# the socket and the store that account owns are `pool`'s on this machine as on that one.
FROM alpine:3.22@sha256:49340b346798875b5592e5d420241f2f9cfba5f47e62ebb1446a1b34734e898f
ARG POOL_UID=1000
RUN apk add --no-cache openssh-server iproute2 procps \
 && adduser -D -u "$POOL_UID" -s /bin/sh pool && adduser -D -s /bin/sh outsider \
 && sed -i -e 's/^pool:!/pool:*/' -e 's/^outsider:!/outsider:*/' /etc/shadow \
 && rm -f /etc/ssh/ssh_host_* \
 && ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_ed25519_key \
 && printf '%s\n' 'HostKey /etc/ssh/ssh_host_ed25519_key' 'PasswordAuthentication no' \
      'KbdInteractiveAuthentication no' 'PermitRootLogin no' 'AllowStreamLocalForwarding yes' \
      'X11Forwarding no' 'AuthorizedKeysFile /etc/ssh/authorized/%u' \
      'MaxStartups 200:30:400' 'MaxSessions 100' 'PerSourcePenalties no' \
      > /etc/ssh/sshd_config
COPY --chmod=0755 sshd-entrypoint.sh /usr/local/bin/sshd-entrypoint.sh
ENTRYPOINT ["/usr/local/bin/sshd-entrypoint.sh"]
