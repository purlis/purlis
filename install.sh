#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# Install purlis from its GitHub releases:
#
#   curl -fsSL https://raw.githubusercontent.com/purlis/purlis/main/install.sh | sh
#
# macOS (Apple silicon): the app goes to /Applications (or ~/Applications when /Applications is
# not writable). A file curl downloads carries no quarantine flag, so the ad-hoc signed,
# un-notarized build opens without Gatekeeper refusing it.
# Linux (x86_64): the .deb, through `sudo apt install`, which also puts `purlis` on PATH. Without
# apt, the AppImage goes to ~/Applications.
#
# PURLIS_CHANNEL=dev installs the dev build of `main` instead of the latest stable release.
# PURLIS_APPLICATIONS=<dir> installs the macOS app or the AppImage there instead.
# When `gh` is installed and signed in, each download's build provenance is verified first
# (SECURITY.md); a download that fails it is not installed.

set -eu

REPO="purlis/purlis"
channel="${PURLIS_CHANNEL:-stable}"

fail() {
  printf 'purlis install: %s\n' "$1" >&2
  exit 1
}
say() {
  printf '%s\n' "$1"
}

case "$channel" in
  stable) base="https://github.com/$REPO/releases/latest/download" ;;
  dev) base="https://github.com/$REPO/releases/download/dev" ;;
  *) fail "PURLIS_CHANNEL is \"$channel\". Use stable or dev." ;;
esac

os="$(uname -s)"
arch="$(uname -m)"
case "$os/$arch" in
  Darwin/arm64) target=macos ;;
  Linux/x86_64 | Linux/amd64) target=linux ;;
  Darwin/*) fail "purlis has no build for macOS on $arch. It runs on Apple silicon." ;;
  Linux/*) fail "purlis has no build for Linux on $arch. It runs on x86_64." ;;
  *) fail "purlis has no build for $os. It runs on macOS (Apple silicon) and Linux (x86_64)." ;;
esac

tmp="$(mktemp -d "${TMPDIR:-/tmp}/purlis-install.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT INT TERM

# Download one release file into $tmp, and verify its provenance when gh can.
fetch() {
  say "Downloading $1 ($channel)…"
  curl -fL --proto '=https' --tlsv1.2 -o "$tmp/$1" "$base/$1" ||
    fail "could not download $base/$1"
  if command -v gh >/dev/null 2>&1 && gh auth status >/dev/null 2>&1; then
    gh attestation verify "$tmp/$1" --repo "$REPO" \
      --signer-workflow "$REPO/.github/workflows/release.yml" \
      --deny-self-hosted-runners >/dev/null ||
      fail "$1 failed its build provenance check, so it was not installed. See SECURITY.md."
    say "Verified: $1 was built by $REPO's release workflow."
  fi
}

if [ "$target" = macos ]; then
  apps="${PURLIS_APPLICATIONS:-/Applications}"
  if [ -z "${PURLIS_APPLICATIONS:-}" ] && [ ! -w "$apps" ]; then
    apps="$HOME/Applications"
  fi
  mkdir -p "$apps"
  file="purlis-macos-arm64.app.zip"
  fetch "$file"
  ditto -x -k "$tmp/$file" "$tmp/app"
  [ -d "$tmp/app/purlis.app" ] || fail "$file did not contain purlis.app"
  rm -rf "$apps/purlis.app"
  mv "$tmp/app/purlis.app" "$apps/purlis.app"
  # Only a browser's download is quarantined; this clears a flag left by an earlier one.
  xattr -dr com.apple.quarantine "$apps/purlis.app" 2>/dev/null || true
  say "purlis is installed in $apps/purlis.app."
  say "Open it, then run Install \`purlis\` command in PATH from the command palette to use \`purlis\` in a terminal."
  exit 0
fi

if command -v apt >/dev/null 2>&1 && command -v dpkg >/dev/null 2>&1; then
  file="purlis-linux-x86_64.deb"
  fetch "$file"
  say "Installing $file with apt. sudo may ask for your password."
  if [ "$(id -u)" -eq 0 ]; then
    apt install -y "$tmp/$file"
  else
    sudo apt install -y "$tmp/$file"
  fi
  say "purlis is installed. Start it from your applications menu, or run \`purlis\`."
  exit 0
fi

apps="${PURLIS_APPLICATIONS:-$HOME/Applications}"
mkdir -p "$apps"
file="purlis-linux-x86_64-appimage.AppImage"
fetch "$file"
mv "$tmp/$file" "$apps/purlis.AppImage"
chmod +x "$apps/purlis.AppImage"
say "purlis is installed as $apps/purlis.AppImage. Run that file to start it."
