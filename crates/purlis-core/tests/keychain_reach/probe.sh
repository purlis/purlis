#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# R-772's probe (T1a-72): what a chat's command, hook or MCP server reaches of the system's
# keychain, run from inside the chat's sandbox. Checked in so no shell line ever names a
# keychain read command in plain text; `the_keychain_service_reach_table_matches_the_pinned_table`
# runs the CI half, and a person runs the item half by hand on a signed build
# (`pinned-table.md` says how).
#
#   probe.sh [--as command|hook|mcp] reach              the keychain service: reachable or denied
#   probe.sh [--as command|hook|mcp] stub <file>...     each file: read and write, reachable or denied
#   probe.sh [--as command|hook|mcp] fetch <service> <account>
#                                                       the item: readable, prompted or denied,
#                                                       with the error code it was mapped from
#
# One `key=value` line per answer. As an MCP server (`--as mcp`) it first reads the client's one
# request line from its input, as a stdio server does, and answers with the same lines. It never
# prints a value it read: a fetch says only whether one came back.
#
# The service, per system:
# - macOS: whether the security server answers at all is the Seatbelt profile's rule for its
#   Mach service, which the test reads from the profile, so this says `service=by-profile`.
# - Linux: the Secret Service, `org.freedesktop.secrets` on the session bus. It answers a ping
#   only where the bus is reachable and a keyring daemon holds the name.
set -u

caller=command
if [ "${1-}" = "--as" ]; then
  caller=$2
  shift 2
fi
if [ "$caller" = mcp ]; then
  # A stdio MCP server is started with its client's request on its input.
  IFS= read -r _request || true
fi
echo "caller=$caller"

mode=${1-}
[ $# -gt 0 ] && shift

reach() {
  case "$(uname -s)" in
    Darwin)
      echo "service=by-profile"
      ;;
    Linux)
      if [ -z "${DBUS_SESSION_BUS_ADDRESS-}" ]; then
        echo "service=denied"
        echo "why=no session bus is named"
      elif dbus-send --session --print-reply --reply-timeout=5000 \
          --dest=org.freedesktop.secrets /org/freedesktop/secrets \
          org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1; then
        echo "service=reachable"
      else
        echo "service=denied"
      fi
      ;;
    *)
      echo "service=unknown"
      ;;
  esac
}

# Each file read, then opened to append nothing (its contents are left as they are).
stub() {
  read_cell=denied
  write_cell=denied
  for file in "$@"; do
    if cat "$file" >/dev/null 2>&1; then read_cell=reachable; fi
    if (: >>"$file") 2>/dev/null; then write_cell=reachable; fi
  done
  echo "stub-read=$read_cell"
  echo "stub-write=$write_cell"
}

# The hand run's fetch. The outcome and the code it was mapped from, never the value.
fetch() {
  service=$1
  account=$2
  case "$(uname -s)" in
    Darwin)
      said=$(/usr/bin/security find-generic-password -s "$service" -a "$account" -w 2>&1 >/dev/null)
      code=$?
      if [ "$code" -eq 0 ]; then
        echo "item=readable"
        echo "code=0"
      else
        case "$said" in
          *"could not be found"*) echo "item=denied"; echo "code=-25300 errSecItemNotFound" ;;
          *"User interaction is not allowed"*) echo "item=prompted"; echo "code=-25308 errSecInteractionNotAllowed" ;;
          *"user canceled"* | *"User canceled"*) echo "item=prompted"; echo "code=-128 errSecUserCanceled" ;;
          *) echo "item=denied"; echo "code=exit $code"; echo "said=$said" ;;
        esac
      fi
      ;;
    Linux)
      # The keyring crate's Secret Service items carry `service` and `username` attributes.
      if secret-tool lookup service "$service" username "$account" >/dev/null 2>&1; then
        echo "item=readable"
        echo "code=0"
      else
        code=$?
        echo "item=denied"
        echo "code=exit $code (no item, or no Secret Service reachable)"
      fi
      ;;
    *)
      echo "item=unknown"
      ;;
  esac
}

case "$mode" in
  reach) reach ;;
  stub) stub "$@" ;;
  fetch) fetch "$@" ;;
  *)
    echo "usage: probe.sh [--as command|hook|mcp] reach | stub <file>... | fetch <service> <account>" >&2
    exit 2
    ;;
esac
