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
#   - plain: one spelling, as Claude Code builds it from a plain glob. Asserted for a name already
#     on disk (it is matched as it is on disk); for a name not yet on disk, reported as a notice;
#   - cased: each letter a class of both cases, exactly as purlis's own Seatbelt rules write it
#     (`vaults_everywhere_rules`, `floor_planted_rules`, `every_project_rules`; a test of the core
#     holds these lines to what those functions generate). Asserted: the run fails if a spelling
#     of a held name gets through one.
# Controls: with no rule at all, each operation goes through (so a refusal is not an absent file
# on a disk that compares names as written); with each rule, what it does not hold goes through
# and the held name as written is refused.
#
# **The profiles purlis emits** (#2128). Given a folder, the profiles a wrapped chat (Codex,
# opencode) starts under, at the full sandbox and at the floor, are applied whole: built by the
# core as a start builds them and written there by
# `crates/purlis-core/tests/it/the_profiles_a_wrapped_chat_starts_under_are_written_out.rs`, with
# the sample project each was built for. Asserted: in each, a held name, as written and in
# another case, is refused, and what nothing holds goes through. So a change in how a profile is
# assembled, a rule's order or a later allow, is applied where it ships.
#
# **Claude Code's own sandbox** (#2211). `tools/seatbelt-case.sh --here <project> <what>` runs
# the same probes of a sample project, and nothing else, in the sandbox this script already runs
# in, with no `sandbox-exec` of its own. Claude Code builds its sandbox itself from the settings
# purlis writes, so
# `crates/purlis-core/tests/it/claude_codes_own_sandbox_holds_the_settings_purlis_writes.rs` has the
# real Claude Code run this line through its Bash tool, at the full sandbox and at the floor. The
# last line says how many answers were checked and how many were wrong. There, it also probes
# the sample project's repositories: each git folder held as a whole, and git still at work.
set -eu

failed=0
checks=0
wrong=0
expect() {
  checks=$((checks + 1))
  if [ "$1" != "$2" ]; then
    echo "::error::seatbelt-case: $3: got $1, expected $2"
    failed=1
    wrong=$((wrong + 1))
  fi
}

# The operations, each given a folder: two on names that are on disk, three that make a held
# leaf that is not there yet.
op_read_vault() { echo "head -c 1 '$1/.PURLIS/VAULTS/ops.json'"; }
op_write_agents() { echo "touch '$1/library/launchagents/y.plist'"; }
op_new_zshrc() { echo "touch '$1/.ZSHRC'"; }
op_new_agents() { echo "mkdir '$1/Library/LAUNCHAGENTS'"; }
op_new_manifest() { echo "touch '$1/PURLIS.TOML'"; }

# yes or no: whether `script` succeeds here, in the sandbox this script runs in.
here() {
  if /bin/sh -c "$1" >/dev/null 2>&1; then
    echo yes
  else
    echo no
  fi
}

# The probes of the sample project at `p`, as the core's tests lay it out (a vault, a held
# folder and one beside it, a file nothing holds, the manifest), each script run by the
# function named `try` and reported as `what`.
held_in() {
  p=$1
  what=$2
  try=$3
  # Controls: what nothing holds is written and read, and a name beside a held one written.
  expect "$("$try" "echo y >> '$p/notes.json' && head -c 1 '$p/notes.json'")" yes "$what: a file nothing holds is written and read"
  expect "$("$try" "touch '$p/Library/LaunchAgentz/x.plist'")" yes "$what: a name beside a held one is written"
  # What the sandbox holds, as written and in another case.
  expect "$("$try" "head -c 1 '$p/.purlis/vaults/ops.json'")" no "$what: the vault as written is read"
  expect "$("$try" "touch '$p/Library/LaunchAgents/x.plist'")" no "$what: a held folder as written is written"
  expect "$("$try" "$(op_read_vault "$p")")" no "$what: a read of the vault through another case"
  expect "$("$try" "$(op_write_agents "$p")")" no "$what: a write through another case"
  expect "$("$try" "$(op_new_zshrc "$p")")" no "$what: a new held leaf in another case"
  expect "$("$try" "echo y >> '$p/purlis.toml'")" no "$what: the manifest is written"
  # Whatever got through is taken away, so the next sandbox is tried on the same project. Inside
  # a sandbox the removal may itself be refused, which leaves nothing to take away.
  rm -f "$p/Library/LaunchAgents/x.plist" "$p/library/launchagents/y.plist" "$p/.ZSHRC" 2>/dev/null || :
}

# The probes of the sample project's repositories at `p`: `repo`, a clone git works in; `scratch`,
# a clone each refused change is tried on, so one that got through spoils no other probe of
# `repo`; and `wt`, a linked worktree of `repo`. Each git folder there as the chat started stays
# where it is, as do the folders above it, while git still commits, branches and checks out.
git_held_in() {
  p=$1
  what=$2
  try=$3
  s=$p/scratch
  g="git -C '$p/repo' -c user.name=probe -c user.email=probe@example.invalid -c commit.gpgsign=false"
  expect "$("$try" "echo y > '$p/repo/probe.txt' && $g add probe.txt && $g commit -q -m probe")" yes "$what: git commits in a clone"
  expect "$("$try" "$g branch probe && $g checkout -q probe")" yes "$what: git branches and checks out in a clone"
  expect "$("$try" "mkdir '$p/plain' && mv '$p/plain' '$p/plain2'")" yes "$what: a folder that holds no repository is moved"
  rm -rf "$p/plain" "$p/plain2" 2>/dev/null || :
  expect "$("$try" "mv '$s/.git' '$s/g'")" no "$what: a clone's git folder is moved"
  [ -e "$s/g" ] && mv "$s/g" "$s/.git" 2>/dev/null || :
  expect "$("$try" "mv '$s/.GIT' '$s/g'")" no "$what: a clone's git folder is moved by another spelling"
  [ -e "$s/g" ] && mv "$s/g" "$s/.git" 2>/dev/null || :
  expect "$("$try" "mv '$s/.Git' '$s/g'")" no "$what: a clone's git folder is moved by a third spelling"
  [ -e "$s/g" ] && mv "$s/g" "$s/.git" 2>/dev/null || :
  expect "$("$try" "mv '$s' '$p/scratch-moved'")" no "$what: a clone is moved"
  [ -e "$p/scratch-moved" ] && mv "$p/scratch-moved" "$s" 2>/dev/null || :
  # A folder swapped into the git folder's place in one step: `renamex_np` with `RENAME_SWAP` on
  # macOS, `renameat2` with `RENAME_EXCHANGE` on Linux, both flag 2. Where there is no python3 to
  # make the call, the probe is refused all the same, and said so.
  if [ "$(uname)" = Darwin ]; then
    exchange='c.renamex_np(a.encode(), b.encode(), 2)'
  else
    exchange='c.renameat2(-100, a.encode(), -100, b.encode(), 2)'
  fi
  if command -v python3 >/dev/null 2>&1; then
    swap="python3 -c 'import ctypes, os, sys
c = ctypes.CDLL(None, use_errno=True)
a, b = sys.argv[1], sys.argv[2]
os.makedirs(a + \"/x\")
sys.exit(0 if $exchange == 0 else 1)' '$s/swap' '$s/.git'"
  else
    echo "::notice::seatbelt-case: $what: no python3, so the swap probe could not be tried"
    swap=false
  fi
  expect "$("$try" "$swap")" no "$what: a folder is swapped into a clone's git folder's place"
  if [ -e "$s/swap/HEAD" ]; then
    python3 -c "import ctypes, sys
c = ctypes.CDLL(None)
a, b = sys.argv[1], sys.argv[2]
$exchange" "$s/swap" "$s/.git" || :
  fi
  expect "$("$try" "rm -rf '$s/.git'")" no "$what: a clone's git folder is removed"
  expect "$("$try" "rm -rf '$s/.git' ; mkdir '$s/.git'")" no "$what: a clone's git folder is replaced"
  expect "$("$try" "mv '$p/wt/.git' '$p/wt/g'")" no "$what: a worktree's .git file is moved"
  [ -e "$p/wt/g" ] && mv "$p/wt/g" "$p/wt/.git" 2>/dev/null || :
  expect "$("$try" "rm -f '$p/wt/.git'")" no "$what: a worktree's .git file is removed"
  # A held config reached by another name: a hard link made in a folder the chat writes, and a
  # write through it, which would change the same file.
  expect "$("$try" "ln '$s/.git/config' '$p/config-link' && echo '[x]' >> '$p/config-link'")" no "$what: a held config is written through a hard link"
  rm -f "$p/config-link" 2>/dev/null || :
}

# The default sign-ins of the other harnesses, for a Claude Code chat: neither read nor written,
# and their folders not moved. The test puts them in `$HOME`.
others_held() {
  expect "$(here "cat '$HOME/.codex/auth.json'")" no "$1: Codex's sign-in read"
  expect "$(here "echo x >> '$HOME/.codex/auth.json'")" no "$1: Codex's sign-in written"
  expect "$(here "mv '$HOME/.codex' '$HOME/.codex.moved'")" no "$1: Codex's folder moved"
  expect "$(here "cat '$HOME/.local/share/opencode/auth.json'")" no "$1: opencode's sign-in read"
  expect "$(here "echo x >> '$HOME/.local/share/opencode/auth.json'")" no "$1: opencode's sign-in written"
  expect "$(here "mv '$HOME/.local/share/opencode' '$HOME/.local/share/opencode.moved'")" no "$1: opencode's folder moved"
}

# `--repositories-here <project> <what>`: the repositories' probes alone, for a sandbox whose
# other holds are not the case-folding ones `held_in` asks about (Claude Code's on Linux).
if [ "${1-}" = --repositories-here ]; then
  if [ $# -ne 3 ]; then
    echo "usage: tools/seatbelt-case.sh --repositories-here <project> <what>" >&2
    exit 2
  fi
  git_held_in "$2" "$3" here
  others_held "$3"
  echo "seatbelt-case: $3: $checks checks, $wrong wrong"
  exit "$failed"
fi

if [ "${1-}" = --here ]; then
  if [ $# -ne 3 ]; then
    echo "usage: tools/seatbelt-case.sh --here <project> <what>" >&2
    exit 2
  fi
  held_in "$2" "$3" here
  git_held_in "$2" "$3" here
  others_held "$3"
  echo "seatbelt-case: $3: $checks checks, $wrong wrong"
  exit "$failed"
fi

emitted=${1-}

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
  # A name already on disk is matched as it is on disk: the one-spelling rule holds it too.
  if [ "$kind" = plain ]; then
    expect "$read_vault" no "plain: a read through another case of a name on disk"
    expect "$write_agents" no "plain: a write through another case of a name on disk"
  fi
  if [ "$kind" = cased ]; then
    expect "$read_vault" no "cased: a read through another case"
    expect "$write_agents" no "cased: a write through another case"
    expect "$new_zshrc" no "cased: a new leaf in another case"
    expect "$new_agents" no "cased: a new folder in another case"
    expect "$new_manifest" no "cased: a new manifest in another case"
  fi
done

# The profiles purlis emits, each applied whole, against the same probes.
if [ -n "$emitted" ]; then
  # yes or no: whether `script` succeeds under the profile in the file `$sb`.
  under() {
    if sandbox-exec -f "$sb" /bin/sh -c "$1" >/dev/null 2>&1; then
      echo yes
    else
      echo no
    fi
  }
  applied=0
  for level in on floor; do
    for harness in codex opencode; do
      sb="$emitted/$harness-$level.sb"
      if [ ! -f "$sb" ]; then
        echo "::error::seatbelt-case: $harness at $level: no profile at $sb"
        failed=1
        continue
      fi
      held_in "$emitted/$level/project" "$harness at $level" under
      applied=$((applied + 1))
    done
  done
  expect "$applied" 4 "each emitted profile applied"
fi

exit "$failed"
