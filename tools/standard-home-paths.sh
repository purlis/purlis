#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
#
# **What a chat can make in its home, tried from inside its sandbox** (#4357).
#
# `tools/standard-home-paths.sh <home> <what>` tries, in order, what installers and tools make
# under `~/.local` and `~/.config` where those folders may be missing, and prints one line for
# each: `standard-paths: <what>: <what was tried>: yes|no`. Run under a sandbox by the tests that
# build one. Real pip and pipx installs of a wheel made here are tried too, with no index, where
# the machine has them.
set -u

home=$1
what=$2
HOME=$home
export HOME
PATH=${PATH:-/usr/bin:/bin}
export PATH
scratch=${TMPDIR:-/tmp}/standard-home-paths.$$
mkdir -p "$scratch"

try() {
  said=$(/bin/sh -c "$2" 2>&1)
  if [ $? -eq 0 ]; then
    echo "standard-paths: $what: $1: yes"
  else
    echo "standard-paths: $what: $1: no ($(printf '%s' "$said" | tail -n 1 | cut -c 1-160))"
  fi
}

try 'mkdir -p ~/.local/bin' 'mkdir -p "$HOME/.local/bin"'
try 'a file in ~/.local/bin' 'touch "$HOME/.local/bin/tool"'
try 'mkdir -p ~/.local/share/sometool' 'mkdir -p "$HOME/.local/share/sometool"'
try 'mkdir -p ~/.local/state/sometool' 'mkdir -p "$HOME/.local/state/sometool"'
try 'mkdir -p ~/.local/pipx/venvs' 'mkdir -p "$HOME/.local/pipx/venvs"'
try 'mkdir -p ~/.local/share/uv/tools' 'mkdir -p "$HOME/.local/share/uv/tools"'
try 'mkdir -p ~/.local/lib/python3/site-packages' 'mkdir -p "$HOME/.local/lib/python3/site-packages"'
try 'mkdir -p ~/.config/sometool' 'mkdir -p "$HOME/.config/sometool"'
try 'a file in ~/.config/sometool' 'touch "$HOME/.config/sometool/settings"'
try 'mkdir -p ~/.cache/sometool' 'mkdir -p "$HOME/.cache/sometool"'

if command -v python3 >/dev/null 2>&1; then
  wheel=$scratch/probe_tool-1.0-py3-none-any.whl
  python3 - "$wheel" <<'PY'
import sys, zipfile
info = "probe_tool-1.0.dist-info/"
with zipfile.ZipFile(sys.argv[1], "w") as z:
    z.writestr("probe_tool.py", "def main():\n    print('ok')\n")
    z.writestr(info + "METADATA", "Metadata-Version: 2.1\nName: probe-tool\nVersion: 1.0\n")
    z.writestr(info + "WHEEL", "Wheel-Version: 1.0\nGenerator: probe\nRoot-Is-Purelib: true\nTag: py3-none-any\n")
    z.writestr(info + "entry_points.txt", "[console_scripts]\nprobe-tool = probe_tool:main\n")
    z.writestr(info + "RECORD", "")
PY
  if python3 -m pip --version >/dev/null 2>&1; then
    echo "standard-paths: $what: user base is $(python3 -m site --user-base 2>/dev/null)"
    try 'pip install --user' "PIP_BREAK_SYSTEM_PACKAGES=1 python3 -m pip install --user --no-index --no-warn-script-location --disable-pip-version-check '$wheel'"
  else
    echo "standard-paths: $what: pip install --user: not tried, no pip here"
  fi
  if command -v pipx >/dev/null 2>&1; then
    try 'pipx install' "PIP_NO_INDEX=1 pipx install '$wheel'"
  else
    echo "standard-paths: $what: pipx install: not tried, no pipx here"
  fi
else
  echo "standard-paths: $what: pip and pipx: not tried, no python3 here"
fi

# What the tries left, with each entry's kind: a sandbox may leave a file where a folder belongs.
find "$HOME/.local" "$HOME/.config" -maxdepth 2 2>/dev/null | while read -r at; do
  if [ -d "$at" ]; then kind=folder; elif [ -c "$at" ]; then kind=device; else kind=file; fi
  echo "standard-paths: $what: left: ${at#"$HOME"/} $kind"
done
rm -rf "$scratch" 2>/dev/null
exit 0
