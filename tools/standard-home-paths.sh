#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
#
# **What a chat can make in its home, tried from inside its sandbox** (#4357).
#
# `tools/standard-home-paths.sh [--installers] <home> <what> <folder>...` tries what tools make
# under `~/.local` and `~/.config` at the floor, in a home that started without those folders, and
# prints a line for each: `standard-paths: <what>: <what was tried>: yes|no`. Each `<folder>`, as
# named under the home, is another harness's default folder: never made, and nothing moved there.
# The last line says how many answers were checked and how many were wrong. `--installers` also
# reports real pip and pipx installs of a wheel made here, with no index, unchecked, since what
# they do depends on the machine's Python.
set -u

installers=
if [ "$1" = --installers ]; then
  installers=1
  shift
fi
home=$1
what=$2
shift 2
HOME=$home
export HOME
PATH=${PATH:-/usr/bin:/bin}
export PATH
scratch=${TMPDIR:-/tmp}/standard-home-paths.$$
mkdir -p "$scratch"

checks=0
wrong=0

# `try <what was tried> <script>`: says what happened, and sets `answer` to yes or no.
try() {
  said=$(/bin/sh -c "$2" 2>&1)
  if [ $? -eq 0 ]; then
    answer=yes
    echo "standard-paths: $what: $1: yes"
  else
    answer=no
    echo "standard-paths: $what: $1: no ($(printf '%s' "$said" | tail -n 1 | cut -c 1-160))"
  fi
}

# `expect yes|no <what was tried> <script>`.
expect() {
  want=$1
  try "$2" "$3"
  checks=$((checks + 1))
  if [ "$answer" != "$want" ]; then
    echo "::error::standard-paths: $what: $2: got $answer, expected $want"
    wrong=$((wrong + 1))
  fi
}

# What tools make for themselves.
expect yes 'mkdir -p ~/.local/share/sometool' 'mkdir -p "$HOME/.local/share/sometool"'
expect yes 'mkdir -p ~/.local/share/uv/tools' 'mkdir -p "$HOME/.local/share/uv/tools"'
expect yes 'mkdir -p ~/.local/state/sometool' 'mkdir -p "$HOME/.local/state/sometool"'
expect yes 'mkdir -p ~/.local/pipx/venvs' 'mkdir -p "$HOME/.local/pipx/venvs"'
expect yes 'mkdir -p ~/.config/sometool' 'mkdir -p "$HOME/.config/sometool"'
expect yes 'a file in ~/.config/sometool' 'touch "$HOME/.config/sometool/settings"'
expect yes 'mkdir -p ~/.cache/sometool' 'mkdir -p "$HOME/.cache/sometool"'

# What the floor asks about first: a program folder, and Python's own site folder.
expect no 'mkdir -p ~/.local/bin' 'mkdir -p "$HOME/.local/bin"'
expect no 'mkdir -p ~/.local/lib/python3/site-packages' 'mkdir -p "$HOME/.local/lib/python3/site-packages"'

if [ -n "$installers" ]; then
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
      pipx install "$wheel" 2>&1 | tail -n 6 | sed "s/^/standard-paths: $what: pipx said: /"
      try 'pipx ran the tool' "$HOME/.local/pipx/venvs/probe-tool/bin/probe-tool"
    else
      echo "standard-paths: $what: pipx install: not tried, no pipx here"
    fi
  else
    echo "standard-paths: $what: pip and pipx: not tried, no python3 here"
  fi
fi

# What stays: the shared folders are not moved, and no other harness's folder is made or planted.
mkdir -p "$scratch/planted"
echo '{}' > "$scratch/planted/auth.json"
expect no 'mv ~/.local aside' 'mv "$HOME/.local" "$HOME/.local.moved"'
expect no 'mv ~/.local/share aside' 'mv "$HOME/.local/share" "$HOME/.local/share.moved"'
expect no 'mv ~/.config aside' 'mv "$HOME/.config" "$HOME/.config.moved"'
for folder in "$@"; do
  expect no "mkdir ~/$folder" "mkdir \"\$HOME/$folder\""
  expect no "a folder moved to ~/$folder" "mv '$scratch/planted' \"\$HOME/$folder\""
done

# What the tries left, with each entry's kind: a sandbox may leave a file where a folder belongs.
find "$HOME/.local" "$HOME/.config" -maxdepth 2 2>/dev/null | while read -r at; do
  if [ -d "$at" ]; then kind=folder; elif [ -c "$at" ]; then kind=device; else kind=file; fi
  echo "standard-paths: $what: left: ${at#"$HOME"/} $kind"
done
rm -rf "$scratch" 2>/dev/null
echo "standard-paths: $what: $checks checks, $wrong wrong"
[ "$wrong" -eq 0 ]
