#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
#
# **Does macOS's sandbox match a name as typed, or as it is on disk?** (#2071, #2107)
#
# Claude Code holds a name by a glob, and turns each glob into a Seatbelt regex the way its
# 2.1.300 build does: `.` and the other regex characters escaped, `**/` as `(.*/)?`, a `[...]`
# class kept, and `(/.*)?$` after a held path. A disk that compares names case-folded opens
# `.PURLIS/VAULTS` as `.purlis/vaults`, so whether a case-sensitive rule holds depends on which
# path the sandbox matches, and may differ for a name that is on disk and one that is not yet.
#
# Measured here, applied for real, under two kinds of rule:
#   - plain: one spelling, as Claude Code builds it from a plain glob (reported as a notice);
#   - cased: each letter a class of both cases, exactly as purlis's own Seatbelt rules write it
#     (`vaults_everywhere_rules`, `floor_planted_rules`, `every_project_rules`; a test of the core
#     holds these lines to what those functions generate). Asserted: the run fails if a spelling
#     of a held name gets through one.
# Controls: with no rule at all, each operation goes through (so a refusal is not an absent file
# on a disk that compares names as written); with each rule, what it does not hold goes through
# and the held name as written is refused.
set -eu

if [ "$(uname)" != Darwin ]; then
  echo "macOS only"
  exit 0
fi

# Each rule's regex, in the sandbox profile's own string form (a backslash written twice).
plain_vault='^/(.*/)?\\.purlis/vaults(/.*)?$'
plain_agents='^/(.*/)?Library/LaunchAgents(/.*)?$'
plain_zshrc='^/(.*/)?\\.zshrc(/.*)?$'
plain_hooks='^/(.*/)?\\.claude/hooks(/.*)?$'
plain_manifest='^/(.*/)?purlis\\.toml(/.*)?$'
cased_vault='^/(.*/)?\\.[pP][uU][rR][lL][iI][sS]/[vV][aA][uU][lL][tT][sS](/.*)?$'
cased_agents='^/(.*/)?[lL][iI][bB][rR][aA][rR][yY]/[lL][aA][uU][nN][cC][hH][aA][gG][eE][nN][tT][sS](/.*)?$'
cased_zshrc='^/(.*/)?\\.[zZ][sS][hH][rR][cC](/.*)?$'
cased_hooks='^/(.*/)?\\.[cC][lL][aA][uU][dD][eE]/[hH][oO][oO][kK][sS](/.*)?$'
cased_manifest='^/(.*/)?[pP][uU][rR][lL][iI][sS]\\.[tT][oO][mM][lL]$'

top=$(mktemp -d)
trap 'rm -rf "$top"' EXIT

# A fresh folder for each try, so what one try made is not on disk for the next.
fresh() {
  d=$(mktemp -d "$top/try.XXXXXX")
  mkdir -p "$d/.purlis/vaults" "$d/Library/LaunchAgents" "$d/Library/LaunchAgentz" "$d/.claude"
  echo '{}' > "$d/.purlis/vaults/ops.json"
  echo '{}' > "$d/notes.json"
  echo "$d"
}

# A fresh folder holding no held leaf yet: `Library` and `.claude` only.
bare() {
  d=$(mktemp -d "$top/bare.XXXXXX")
  mkdir -p "$d/Library" "$d/.claude"
  echo "$d"
}

profile() {
  printf '(version 1)\n(allow default)\n'
  for rule in "$@"; do
    printf '(deny file-read* file-write* (regex "%s"))\n' "$rule"
  done
}

# yes or no: whether `script` succeeds under a profile denying each of the rules after it.
tried() {
  script=$1
  shift
  if sandbox-exec -p "$(profile "$@")" /bin/sh -c "$script" >/dev/null 2>&1; then
    echo yes
  else
    echo no
  fi
}

failed=0
expect() {
  if [ "$1" != "$2" ]; then
    echo "::error::seatbelt-case: $3: got $1, expected $2"
    failed=1
  fi
}

# The operations, each given a folder: two on names that are on disk, three that make a held
# leaf that is not there yet.
op_read_vault() { echo "head -c 1 '$1/.PURLIS/VAULTS/ops.json'"; }
op_write_agents() { echo "touch '$1/library/launchagents/y.plist'"; }
op_new_zshrc() { echo "touch '$1/.ZSHRC'"; }
op_new_agents() { echo "mkdir '$1/Library/LAUNCHAGENTS'"; }
op_new_manifest() { echo "touch '$1/PURLIS.TOML'"; }

# With no rule, every operation goes through on this disk.
expect "$(tried "$(op_read_vault "$(fresh)")")" yes "no rule: a read through another case"
expect "$(tried "$(op_write_agents "$(fresh)")")" yes "no rule: a write through another case"
expect "$(tried "$(op_new_zshrc "$(bare)")")" yes "no rule: a new leaf in another case"
expect "$(tried "$(op_new_agents "$(bare)")")" yes "no rule: a new folder in another case"
expect "$(tried "$(op_new_manifest "$(bare)")")" yes "no rule: a new manifest in another case"

for kind in plain cased; do
  if [ "$kind" = plain ]; then
    set -- "$plain_vault" "$plain_agents" "$plain_zshrc" "$plain_hooks" "$plain_manifest"
  else
    set -- "$cased_vault" "$cased_agents" "$cased_zshrc" "$cased_hooks" "$cased_manifest"
  fi
  d=$(fresh)
  # Controls under the rule.
  expect "$(tried "head -c 1 '$d/notes.json'" "$@")" yes "$kind: a file nothing holds is read"
  expect "$(tried "touch '$d/Library/LaunchAgentz/x.plist'" "$@")" yes "$kind: a name beside a held one is written"
  expect "$(tried "head -c 1 '$d/.purlis/vaults/ops.json'" "$@")" no "$kind: the vault as written is read"
  expect "$(tried "touch '$d/Library/LaunchAgents/x.plist'" "$@")" no "$kind: a held folder as written is written"
  # The measurement.
  read_vault=$(tried "$(op_read_vault "$(fresh)")" "$@")
  write_agents=$(tried "$(op_write_agents "$(fresh)")" "$@")
  new_zshrc=$(tried "$(op_new_zshrc "$(bare)")" "$@")
  new_agents=$(tried "$(op_new_agents "$(bare)")" "$@")
  new_manifest=$(tried "$(op_new_manifest "$(bare)")" "$@")
  echo "::notice::seatbelt-case ($kind rule): let through, on disk: read .PURLIS/VAULTS $read_vault, write library/launchagents $write_agents; not yet on disk: new .ZSHRC $new_zshrc, new Library/LAUNCHAGENTS $new_agents, new PURLIS.TOML $new_manifest"
  if [ "$kind" = cased ]; then
    expect "$read_vault" no "cased: a read through another case"
    expect "$write_agents" no "cased: a write through another case"
    expect "$new_zshrc" no "cased: a new leaf in another case"
    expect "$new_agents" no "cased: a new folder in another case"
    expect "$new_manifest" no "cased: a new manifest in another case"
  fi
done

# Letters outside ASCII that a disk comparing names case-folded may take for ASCII ones, under
# the cased rules: whether the name is the same file, and whether the rule holds it.
s_long=$(printf '\305\277')
k_sign=$(printf '\342\204\252')
set -- "$cased_zshrc" "$cased_hooks"
d=$(fresh)
echo x > "$d/.zshrc"
mkdir -p "$d/.claude/hooks"
same_s=$(tried "test -e '$d/.z${s_long}hrc'")
same_k=$(tried "test -e '$d/.claude/hoo${k_sign}s'")
held_s=$(tried "echo y >> '$d/.z${s_long}hrc'" "$@")
held_k=$(tried "touch '$d/.claude/hoo${k_sign}s/x'" "$@")
d=$(bare)
new_s=$(tried "touch '$d/.z${s_long}hrc' && test -e '$d/.zshrc'" "$@")
d=$(bare)
new_k=$(tried "mkdir '$d/.claude/hoo${k_sign}s' && test -e '$d/.claude/hooks'" "$@")
echo "::notice::seatbelt-case (other letters): $same_s $same_k $held_s $held_k $new_s $new_k"

exit "$failed"
