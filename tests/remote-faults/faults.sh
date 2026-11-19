#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **Faults put under a live remote chat, on the remote parity job's rigs** (#7188): the link
# dropped, silenced or slowed, the sshd restarted or given another host key, a runner killed, the
# registry taken down. Each verb works on a container of a rig (`tests/remote-docker/up.sh`'s
# `sshd` and `dind`, `tests/ssh-runner/up-systemd.sh`'s `ssh-runner-systemd`) through
# `docker exec`, with the rig's own `tc` and, for a refusal, `iptables`.
#
#     faults.sh [--machine NAME] VERB [ARGUMENT]
#
# **Every verb can be said twice**: a second `drop` finds its rule there and adds none, and a
# second `blackhole` keeps the link as it was first, not as the first fault left it. **`restore`
# undoes every verb**: the rule is taken out, the link goes back to what it was before the first
# fault (the rig's own latency and loss, or none), the host key to the one the machine started
# with, the registry is started and the runner containers a kill left are removed. What it cannot
# undo is a runner's death, which is the point of it: the next chat makes a new one.
#
# `FAULTS_DRY_RUN=1` prints each command it would send instead of sending it, which is how
# `tools/remote-faults.test.mjs` reads it.
set -euo pipefail

VERBS="drop restore restart-sshd rotate-hostkey kill-runner slow break-registry blackhole"
DEV=${FAULTS_DEV:-eth0}
# The machine inside which `dind` runs its registry and the runners' containers.
DIND=${FAULTS_DIND:-dind}
# Where a machine keeps what it was before its first fault. On disk, not in /run: the systemd
# machine's /run is a tmpfs, which a restart empties.
KEEP=/root/.faults

usage() {
  cat <<USAGE
usage: faults.sh [--machine NAME] VERB [ARGUMENT]

machine: the container to work on (default \$FAULTS_MACHINE, else ssh-runner-systemd)

verbs:
  drop            refuse every new and old connection to the sshd (TCP reset), at once
  blackhole       lose every packet with no reset: a half-open link (netem loss 100%)
  slow [SECONDS]  delay every packet by SECONDS (default 15; netem delay)
  restart-sshd    end the sshd's sessions and restart it (a hangup, where the sshd is PID 1)
  rotate-hostkey  give the sshd a new host key and restart it
  kill-runner     kill -9 the runner: the host of a systemd machine, or a pool's runner containers
  break-registry  stop the registry inside dind, so no image can be pulled
  restore         undo every verb above
USAGE
}

die() {
  echo "faults.sh: $*" >&2
  usage >&2
  exit 2
}

machine=${FAULTS_MACHINE:-ssh-runner-systemd}
verb=
argument=
while [ "$#" -gt 0 ]; do
  case $1 in
    -h | --help)
      usage
      exit 0
      ;;
    --machine)
      [ "$#" -ge 2 ] || die "--machine needs a container's name"
      machine=$2
      shift 2
      ;;
    -*) die "unknown option $1" ;;
    *)
      if [ -z "$verb" ]; then
        verb=$1
      elif [ -z "$argument" ]; then
        argument=$1
      else
        die "too many arguments: $1"
      fi
      shift
      ;;
  esac
done
[ -n "$verb" ] || die "no verb"
case " $VERBS " in
  *" $verb "*) ;;
  *) die "no verb called $verb" ;;
esac

# Sent to `container`'s shell, or printed.
inside() {
  local container=$1 script=$2
  if [ -n "${FAULTS_DRY_RUN:-}" ]; then
    printf 'docker exec %s %s\n' "$container" "$script"
  else
    docker exec "$container" sh -c "$script"
  fi
}

# Whether the rig has a container called $1; always, when only saying what would run.
has() {
  [ -n "${FAULTS_DRY_RUN:-}" ] || docker inspect "$1" >/dev/null 2>&1
}

case $machine in
  *systemd*) init=systemd ;;
  *) init=plain ;;
esac

# The rule that refuses the sshd's port, written once so a check and a removal match it.
RULE="INPUT -p tcp --dport 22 -j REJECT --reject-with tcp-reset"

# What the link was before the first fault, kept once: netem's options, or `none`.
keep_link() {
  inside "$machine" "mkdir -p $KEEP && test -e $KEEP/netem || { tc qdisc show dev $DEV root | sed -n 's/^qdisc netem [^ ]* root.* limit [0-9]* *//p' | grep . > $KEEP/netem || echo none > $KEEP/netem; }"
}

# What the sshd's host key was before the first rotation, kept once.
keep_hostkey() {
  inside "$machine" "mkdir -p $KEEP && test -e $KEEP/ssh_host_ed25519_key || cp -p /etc/ssh/ssh_host_ed25519_key /etc/ssh/ssh_host_ed25519_key.pub $KEEP/"
}

# The sshd's sessions end, and the daemon reads its configuration and key again.
restart_sshd() {
  # A session is `sshd: keeper@notty` or `sshd-session: keeper [priv]`; the listener's is `sshd: /usr/sbin/sshd`.
  inside "$machine" "pkill -KILL -f '^sshd(-session)?: [a-z]' || true"
  if [ "$init" = systemd ]; then
    inside "$machine" "systemctl restart ssh"
  else
    inside "$machine" "kill -HUP 1"
  fi
}

case $verb in
  drop)
    inside "$machine" "iptables -C $RULE 2>/dev/null || iptables -I $RULE"
    ;;
  blackhole)
    keep_link
    inside "$machine" "tc qdisc replace dev $DEV root netem loss 100%"
    ;;
  slow)
    seconds=${argument:-15}
    case $seconds in
      '' | *[!0-9]* | 0) die "slow takes a whole number of seconds, not '$seconds'" ;;
    esac
    keep_link
    inside "$machine" "tc qdisc replace dev $DEV root netem delay ${seconds}s"
    ;;
  restart-sshd)
    restart_sshd
    ;;
  rotate-hostkey)
    keep_hostkey
    inside "$machine" "rm -f /etc/ssh/ssh_host_ed25519_key /etc/ssh/ssh_host_ed25519_key.pub && ssh-keygen -q -t ed25519 -N '' -f /etc/ssh/ssh_host_ed25519_key"
    restart_sshd
    ;;
  kill-runner)
    if [ "$init" = systemd ]; then
      # The host of the account's chats: `purlis serve --as-a-service`, as the unit runs it.
      inside "$machine" "pkill -KILL -u keeper -f ' serve --as-a-service\$' || true"
    else
      # A pool's runners are containers of the engine behind the sshd.
      inside "$DIND" "docker ps -q --filter label=purlis.runner | xargs -r docker kill"
    fi
    ;;
  break-registry)
    inside "$DIND" "docker stop registry"
    ;;
  restore)
    # Each undo is its own command, and one that has nothing to undo says nothing and succeeds.
    inside "$machine" "while iptables -C $RULE 2>/dev/null; do iptables -D $RULE; done"
    inside "$machine" "if test -e $KEEP/netem; then if test \"\$(cat $KEEP/netem)\" = none; then tc qdisc del dev $DEV root 2>/dev/null || true; else tc qdisc replace dev $DEV root netem \$(cat $KEEP/netem); fi; fi"
    inside "$machine" "rm -f $KEEP/netem"
    # The key the machine started with goes back, and only then is the sshd restarted: a restore
    # with no rotation to undo leaves the sshd alone.
    if [ "$init" = systemd ]; then reload="systemctl restart ssh"; else reload="kill -HUP 1"; fi
    inside "$machine" "if test -e $KEEP/ssh_host_ed25519_key; then cp -p $KEEP/ssh_host_ed25519_key $KEEP/ssh_host_ed25519_key.pub /etc/ssh/ && rm -f $KEEP/ssh_host_ed25519_key $KEEP/ssh_host_ed25519_key.pub && $reload; fi"
    # Only the plain machines (`sshd`, `dind`) sit in front of the engine; the systemd machine has
    # no registry or runners, and restoring it leaves dind alone.
    if [ "$init" = plain ] && has "$DIND"; then
      inside "$DIND" "docker start registry || true"
      inside "$DIND" "docker ps -aq --filter label=purlis.runner --filter status=exited | xargs -r docker rm"
    fi
    ;;
esac
