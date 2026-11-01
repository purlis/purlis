#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
#
# **What a chat can make in its home, tried from inside its sandbox** (#4357).
#
# `tools/standard-home-paths.sh <home> <what> <folder>...` tries what tools make under `~/.local`
# and `~/.config` at the floor, in a home that started without those folders, and prints a line for
# each: `standard-paths: <what>: <what was tried>: yes|no`. Each `<folder>`, as named under the
# home, is another harness's default folder: never made, and nothing moved there. The last line
# says how many answers were checked and how many were wrong.
set -u

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

# `expect yes|no <what was tried> <script>`.
expect() {
  said=$(/bin/sh -c "$3" 2>&1)
  if [ $? -eq 0 ]; then
    answer=yes
  else
    answer=no
  fi
  echo "standard-paths: $what: $2: $answer ($(printf '%s' "$said" | tail -n 1 | cut -c 1-160))"
  checks=$((checks + 1))
  if [ "$answer" != "$1" ]; then
    echo "::error::standard-paths: $what: $2: got $answer, expected $1"
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

rm -rf "$scratch" 2>/dev/null
echo "standard-paths: $what: $checks checks, $wrong wrong"
[ "$wrong" -eq 0 ]
