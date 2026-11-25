#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Cargo's runner for macOS targets (`.cargo/config.toml`): `cargo run`, `cargo test`, `cargo
# nextest run` and `cargo tauri dev` call it as `macos-dev-sign-run.sh <binary> [args…]`.
#
# Why: the linker signs every local build ad hoc, so its Keychain identity is its cdhash, which
# changes on every build, and an item's access list then treats each rebuild as a new app and asks
# for the login password again. This signs the binary with ONE stable identity and a stable
# identifier before it runs, so the Keychain sees the same app across rebuilds.
#
# The identity, first that exists:
#   1. $PURLIS_DEV_SIGN_IDENTITY (a certificate's name or SHA-1)
#   2. the first valid "Apple Development:" identity in `security find-identity -v -p codesigning`
#   3. a certificate named "purlis dev"
#   4. none: the binary runs unsigned, as before, and ONE hint a day says how to get an identity.
#
# The identifier is `dev.purlis.local.<binary name>` (a test binary's `-<hash>` dropped), never
# `dev.purlis.app`, the release app's. It does nothing at all, and starts no other process, on CI
# ($CI set) or where there is no `codesign`. A binary already signed with this identity and
# identifier is left alone, so the usual run costs one `codesign -dv`. Nothing here ever stops the
# binary from running: a failed signature runs it unsigned.
set -u

if [ "$#" -lt 1 ]; then
  echo "usage: macos-dev-sign-run.sh <binary> [args…]" >&2
  exit 2
fi
bin=$1

case "${CI:-}" in "" | false | 0) ;; *) exec "$@" ;; esac
command -v codesign >/dev/null 2>&1 || exec "$@"
[ -f "$bin" ] || exec "$@"

# Once a day at most, and only where that can be remembered (a run that cannot write its stamp
# would say it on every run).
hint() {
  local dir today stamp
  dir=${PURLIS_DEV_SIGN_STATE:-${XDG_STATE_HOME:-${HOME:-/nonexistent}/.local/state}/purlis}
  today=$(date +%F)
  stamp=$dir/dev-sign-hint-$1
  [ "$(cat "$stamp" 2>/dev/null)" = "$today" ] && return 0
  mkdir -p "$dir" 2>/dev/null && printf '%s\n' "$today" >"$stamp" 2>/dev/null || return 0
  printf 'purlis: %s\n' "$2" >&2
}

identity="" # what codesign is given
name=""     # the certificate's name, which a signature's Authority line carries
if [ -n "${PURLIS_DEV_SIGN_IDENTITY:-}" ]; then
  identity=$PURLIS_DEV_SIGN_IDENTITY
  name=$identity
  case $identity in
    [0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f][0-9A-Fa-f]*)
      # A SHA-1 is not what a signature's Authority line carries: look its name up.
      line=$(security find-identity -v -p codesigning 2>/dev/null | grep -i -m1 "$identity")
      name=$(printf '%s\n' "$line" | sed -nE 's/^[^"]*"(.*)"[^"]*$/\1/p')
      ;;
  esac
else
  list=$(security find-identity -v -p codesigning 2>/dev/null) || list=""
  line=$(printf '%s\n' "$list" | grep -m1 '"Apple Development: ')
  [ -n "$line" ] || line=$(printf '%s\n' "$list" | grep -m1 '"purlis dev"')
  if [ -n "$line" ]; then
    identity=$(printf '%s\n' "$line" | sed -E 's/^ *[0-9]+\) ([0-9A-Fa-f]+) .*/\1/')
    name=$(printf '%s\n' "$line" | sed -E 's/^[^"]*"(.*)"[^"]*$/\1/')
  fi
fi

if [ -z "$identity" ]; then
  hint none "this build is ad-hoc signed, so the Keychain asks for your password after every rebuild. Sign with a stable identity: see \"Local macOS signing\" in docs/development.md."
  exec "$@"
fi

base=$(basename "$bin")
base=$(printf '%s\n' "$base" | sed -E 's/-[0-9a-f]{16}$//')
id=dev.purlis.local.$base

signed_already() {
  local info
  info=$(codesign -dv --verbose=2 "$bin" 2>&1) || return 1
  case $info in *"Signature=adhoc"*) return 1 ;; esac
  case $info in *"Identifier=$id"$'\n'*) ;; *) return 1 ;; esac
  [ -z "$name" ] && return 0
  case $info in *"Authority=$name"$'\n'*) return 0 ;; esac
  return 1
}

if ! signed_already; then
  # nextest runs one binary once a test, in parallel: the first signs, the rest wait for it. A
  # lock is stale only when the pid in it is gone; a live owner (waiting on a Keychain dialog, say)
  # is waited for, and a waiter that runs out of patience runs the binary unsigned, never steals.
  lock=$bin.signlock
  tries=0
  until mkdir "$lock" 2>/dev/null; do
    owner=$(cat "$lock/pid" 2>/dev/null)
    if [ -n "$owner" ] && ! kill -0 "$owner" 2>/dev/null; then
      mv "$lock" "$lock.stale.$$" 2>/dev/null && rm -rf "$lock.stale.$$"
      continue
    fi
    tries=$((tries + 1))
    if [ "$tries" -gt 600 ]; then
      # No pid ever written: the owner died between mkdir and echo. Anything else: give up waiting.
      [ -z "$owner" ] || exec "$@"
      mv "$lock" "$lock.stale.$$" 2>/dev/null && rm -rf "$lock.stale.$$"
      tries=0
    fi
    sleep 0.1
  done
  trap 'rm -rf "$lock"' EXIT
  trap 'exit 130' INT TERM HUP
  echo $$ >"$lock/pid"
  if ! signed_already; then
    codesign --force --sign "$identity" --identifier "$id" "$bin" >/dev/null 2>&1 ||
      hint failed "codesign with \"$name\" failed for $base, so it runs unsigned and the Keychain may ask for your password. Check PURLIS_DEV_SIGN_IDENTITY, or run: codesign --force --sign \"$name\" $bin"
  fi
  trap - EXIT INT TERM HUP
  rm -rf "$lock"
fi

exec "$@"
