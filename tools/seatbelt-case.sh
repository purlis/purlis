#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0
#
# **Does macOS's sandbox match a name as typed, or as it is on disk?** (#2071, #2107)
#
# Claude Code holds a name by a glob, and turns each glob into a Seatbelt regex the way its
# 2.1.300 build does: `.` and the other regex characters escaped, `**/` as `(.*/)?`, a `[...]`
# class kept, and `(/.*)?$` after a held folder. A disk that compares names case-folded opens
# `.PURLIS/VAULTS` as `.purlis/vaults`, so whether a case-sensitive rule holds depends on which
# path the sandbox matches.
#
# This measures it on the runner, applied for real, and says the answer as a notice:
#   - the rule as a plain glob makes it: one spelling only;
#   - the rule purlis now writes: each letter a class of both cases.
# Only the second is asserted (the run fails if a spelling of a held name gets through it). The
# first is the measurement, reported either way, with controls that tell a refusal by the rule
# from a refusal for any other reason.
set -eu

if [ "$(uname)" != Darwin ]; then
  echo "macOS only"
  exit 0
fi

dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
mkdir -p "$dir/.purlis/vaults" "$dir/Library/LaunchAgents" "$dir/Library/LaunchAgentz"
echo '{}' > "$dir/.purlis/vaults/ops.json"
echo '{}' > "$dir/notes.json"

# The rules as Claude Code 2.1.300 builds them from `/**/.purlis/vaults` and
# `/**/Library/LaunchAgents`, and from the same globs with each letter a class.
plain_vault='^/(.*/)?\\.purlis/vaults(/.*)?$'
plain_agents='^/(.*/)?Library/LaunchAgents(/.*)?$'
cased_vault='^/(.*/)?\\.[pP][uU][rR][lL][iI][sS]/[vV][aA][uU][lL][tT][sS](/.*)?$'
cased_agents='^/(.*/)?[Ll][iI][bB][rR][aA][rR][yY]/[Ll][aA][uU][nN][cC][hH][Aa][gG][eE][nN][tT][sS](/.*)?$'

profile() {
  printf '(version 1)\n(allow default)\n'
  printf '(deny file-read* file-write* (regex "%s"))\n' "$1"
  printf '(deny file-write* (regex "%s"))\n' "$2"
}

# Whether `script` succeeds under the profile for `vault` and `agents`.
tried() {
  if sandbox-exec -p "$(profile "$1" "$2")" /bin/sh -c "$3" >/dev/null 2>&1; then
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

for kind in plain cased; do
  if [ "$kind" = plain ]; then
    vault=$plain_vault
    agents=$plain_agents
  else
    vault=$cased_vault
    agents=$cased_agents
  fi
  # Controls: the same reads and writes of what no rule holds go through, and the held name
  # as written is refused.
  expect "$(tried "$vault" "$agents" "head -c 1 '$dir/notes.json'")" yes "$kind: a file nothing holds is read"
  expect "$(tried "$vault" "$agents" "touch '$dir/Library/LaunchAgentz/x.plist'")" yes "$kind: a name beside the held one is written"
  expect "$(tried "$vault" "$agents" "head -c 1 '$dir/.purlis/vaults/ops.json'")" no "$kind: the vault as written is read"
  expect "$(tried "$vault" "$agents" "touch '$dir/Library/LaunchAgents/x.plist'")" no "$kind: the held folder as written is written"
  # The measurement: the same names, spelled in another case.
  read_other=$(tried "$vault" "$agents" "head -c 1 '$dir/.PURLIS/VAULTS/ops.json'")
  write_other=$(tried "$vault" "$agents" "touch '$dir/library/launchagents/y.plist'")
  echo "::notice::seatbelt-case ($kind rule): read through .PURLIS/VAULTS let through: $read_other; write through library/launchagents let through: $write_other"
  if [ "$kind" = cased ]; then
    expect "$read_other" no "cased: the vault read through another case"
    expect "$write_other" no "cased: the held folder written through another case"
  fi
done

exit "$failed"
