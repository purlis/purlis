# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **A machine without Docker that boots systemd, reached over SSH, for RR-15** (#724, ADR 0068
# §3 as amended for #724): Ubuntu, as `sshd.Dockerfile` is, but with systemd as its init, so a
# sign-in over SSH is a logind session, `keeper` has a systemd user manager, and `loginctl
# enable-linger` means what it means on a real machine. polkitd is there because that is what
# lets an account turn its own linger on (logind asks it). `docker restart` is the machine's
# restart: systemd shuts down on SIGRTMIN+3 and boots again, with `/run` empty and the disk kept.
# `UsePAM yes` is what makes each SSH sign-in a logind session (pam_systemd).
# `tc` and `iptables` are the fault suite's (`tests/remote-faults/faults.sh`, #7188).
# `/var/log/journal` keeps the journal on disk, so the boot a restart ended can still be read
# after it (`journalctl -m`, #6700).
FROM ubuntu:26.04@sha256:6a06111c3b0a8600feadb270aec771dc239bf407b25625f93f44ab00e11f4d3c
ENV container=docker
RUN apt-get --option Acquire::Retries=5 update \
 && apt-get --option Acquire::Retries=5 install --yes --no-install-recommends \
      systemd systemd-sysv dbus dbus-user-session libpam-systemd polkitd \
      openssh-server procps ca-certificates libdbus-1-3 iproute2 iptables \
 && rm -rf /var/lib/apt/lists/* \
 && userdel --remove ubuntu \
 && mkdir -p /var/log/journal \
 && useradd --create-home --shell /bin/bash keeper \
 && sed -i -e 's/^keeper:!/keeper:*/' /etc/shadow \
 && rm -f /etc/ssh/ssh_host_* \
 && ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_ed25519_key \
 && printf '%s\n' 'HostKey /etc/ssh/ssh_host_ed25519_key' 'UsePAM yes' \
      'PasswordAuthentication no' 'KbdInteractiveAuthentication no' 'PermitRootLogin no' \
      'X11Forwarding no' 'AuthorizedKeysFile /etc/ssh/authorized/%u' \
      'MaxStartups 200:30:400' 'MaxSessions 100' 'PerSourcePenalties no' \
      > /etc/ssh/sshd_config \
 && printf '%s\n' '[Unit]' 'Description=The keys each user signs in with' 'Before=ssh.service' \
      '[Service]' 'Type=oneshot' \
      'ExecStart=/bin/sh -c "mkdir -p /etc/ssh/authorized && cp /etc/ssh/authorized-src/* /etc/ssh/authorized/ && chown -R root:root /etc/ssh/authorized && chmod 0755 /etc/ssh/authorized && chmod 0644 /etc/ssh/authorized/*"' \
      '[Install]' 'WantedBy=multi-user.target' \
      > /etc/systemd/system/authorized-keys.service \
 && systemctl enable authorized-keys.service ssh.service \
 && (systemctl disable ssh.socket || true) \
 && systemctl mask getty.target console-getty.service systemd-firstboot.service \
      systemd-udevd.service systemd-udevd-control.socket systemd-udevd-kernel.socket \
      systemd-modules-load.service sys-kernel-config.mount sys-kernel-debug.mount \
      sys-kernel-tracing.mount systemd-remount-fs.service
STOPSIGNAL SIGRTMIN+3
CMD ["/sbin/init"]
