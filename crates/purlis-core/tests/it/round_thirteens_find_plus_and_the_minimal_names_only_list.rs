// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round 13 of the merge guard (#7423).
//!
//! - `find` ends a run at `+` only when it directly follows `{}`; any other `+` is a word of the
//!   command, so `find … -exec env -u + gh pr merge 5 \;` runs `env -u + gh pr merge 5`.
//! - The programs behind which a forge word is only a name are a MINIMAL list: the ones that
//!   provably cannot run a word operand. Package managers, `sed`, `awk`, `find`, `less`, `tee`,
//!   `jq`, `sort`, `install`, `codesign`, `xattr`, `locate`, `mdfind`, `apropos`, `tldr` and
//!   `git commit`/`checkout`/`switch`/`restore`/`reset`/`stash`/`pull`/`push` are not on it.

use purlis_core::floorguard::{merge_reason, release_floor_reason};
use purlis_core::mergehold;

/// Each is held by the hold, and refused by the floor's merge half and the release floor.
fn held(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            merge_reason(cmd).is_some(),
            "not a merge to the floor: {cmd}"
        );
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

/// Each passes the hold and the floor's merge half.
fn passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(merge_reason(cmd), None, "a merge to the floor: {cmd}");
    }
}

#[test]
fn a_plus_that_does_not_follow_a_placeholder_does_not_end_a_find_run() {
    purlis_core::unsteered!();
    held(&[
        "find . -maxdepth 0 -exec env -u + gh pr merge 5 \\;",
        "find . -maxdepth 0 -execdir env -u + gh pr merge 5 \\;",
        "find . -maxdepth 0 -ok env -u + gh pr merge 5 \\;",
        "find . -maxdepth 0 -okdir env -u + gh pr merge 5 \\;",
    ]);
}

#[test]
fn a_package_manager_plugin_test_runs_its_trailing_operands() {
    purlis_core::unsteered!();
    held(&[
        "asdf plugin test x https://example.com/p.git gh pr merge 5",
        "asdf plugin test x https://example.com/p.git --asdf-tool-version 1 gh pr merge 5",
    ]);
}

#[test]
fn a_program_off_the_minimal_list_gives_a_forge_call_no_cover() {
    purlis_core::unsteered!();
    held(&[
        "brew install gh pr merge 5",
        "brew upgrade gh pr merge 5",
        "apt install gh pr merge 5",
        "apt-get install gh pr merge 5",
        "asdf install gh pr merge 5",
        "mise install gh pr merge 5",
        "npm install gh pr merge 5",
        "pnpm install gh pr merge 5",
        "yarn install gh pr merge 5",
        "pip install gh pr merge 5",
        "cargo install gh pr merge 5",
        "sed -n p gh pr merge 5",
        "awk '{print}' gh pr merge 5",
        "find . -name x gh pr merge 5",
        "less gh pr merge 5",
        "tee gh pr merge 5",
        "jq . gh pr merge 5",
        "sort gh pr merge 5",
        "install gh pr merge 5",
        "codesign gh pr merge 5",
        "xattr gh pr merge 5",
        "locate gh pr merge 5",
        "mdfind gh pr merge 5",
        "apropos gh pr merge 5",
        "tldr gh pr merge 5",
        "git commit gh pr merge 5",
        "git checkout gh pr merge 5",
        "git switch gh pr merge 5",
        "git restore gh pr merge 5",
        "git reset gh pr merge 5",
        "git stash gh pr merge 5",
        "git pull gh pr merge 5",
        "git push gh pr merge 5",
        "rg --pre gh pr merge 5",
        "rg --pre-glob '*' gh pr merge 5",
        "git grep -O gh pr merge 5",
        "git grep --open-files-in-pager=x gh pr merge 5",
        "git -c core.editor=x log gh pr merge 5",
        "git -c core.pager=x log gh pr merge 5",
        "git -c core.fsmonitor=x log gh pr merge 5",
        "git -c core.hooksPath=x log gh pr merge 5",
        "git -c core.sshCommand=x log gh pr merge 5",
        "git -c sequence.editor=x log gh pr merge 5",
        "git -c diff.external=x log gh pr merge 5",
        "git -c diff.x.textconv=x log gh pr merge 5",
        "git -c x.command=x log gh pr merge 5",
        "git -c alias.x=y log gh pr merge 5",
        "git -c credential.helper=x log gh pr merge 5",
        "GIT_EDITOR=x git log gh pr merge 5",
        "GIT_PAGER=x git log gh pr merge 5",
    ]);
}

#[test]
fn the_kept_programs_still_take_gh_as_a_name() {
    purlis_core::unsteered!();
    passes(&[
        "cp /opt/homebrew/bin/gh ~",
        "git add crates/gh *.md",
        "mv gh gh.old",
        "ln -s /opt/homebrew/bin/gh gh",
        "rm gh",
        "ls gh",
        "stat gh",
        "file gh",
        "du gh",
        "cat gh",
        "head gh",
        "tail gh",
        "wc gh",
        "diff gh gh2",
        "cmp gh gh2",
        "shasum gh",
        "md5 gh",
        "md5sum gh",
        "sha256sum gh",
        "grep gh src/main.rs",
        "rg gh src",
        "mkdir gh",
        "touch gh",
        "chmod +x gh",
        "chown me gh",
        "which gh",
        "type gh",
        "man gh",
        "git status gh",
        "git log gh",
        "git diff gh",
        "git show gh",
        "git blame gh",
        "git shortlog gh",
        "git grep gh",
        "git mv gh gh2",
        "git rm gh",
        "git branch gh",
        "git tag gh",
        "git fetch gh",
        "git remote add gh url",
        "git -C repo log gh",
        "git -c core.abbrev=8 log gh",
    ]);
}

#[test]
fn a_quoted_message_is_no_call_and_a_held_name_says_how_to_rewrite() {
    purlis_core::unsteered!();
    // One quoted word is a message, not a call: it is never split, so a `gh pr merge` inside it
    // is not read (round 13 documents this; the forge route is the boundary for such a word).
    passes(&[
        "git commit -m \"mention gh pr merge in text\"",
        "git commit -m \"fix gh\"",
    ]);
    // A bare `gh` behind a program off the list is held, and the refusal says how to rewrite.
    held(&["git commit -m fix gh", "brew install gh"]);
    for cmd in ["git commit -m fix gh", "brew install gh"] {
        let said = merge_reason(cmd).expect(cmd);
        assert!(said.contains("run the `gh` command first"), "{said}");
        assert!(said.contains("say what you want done"), "{said}");
    }
}

/// Each of these passed in rounds 8 to 12, behind a program the old tables called names-only.
/// The program is off the minimal list now, so a bare `gh` or a path to it behind it is a call
/// that the allowlist reads, and `gh` alone is held. A false hold, accepted: run `gh` first.
#[test]
fn what_the_older_rounds_passed_behind_a_dropped_program_is_held_now() {
    purlis_core::unsteered!();
    held(&[
        "find tests/fixtures/gh -name '*.json'",
        "install -m 755 target/release/gh \"$out/bin/gh\"",
        "ack gh src/*",
        "find / -name gh -type f $X",
        "asdf install gh $(asdf latest gh)",
        "mise install gh $V",
        "snap install gh $OPTS",
        "brew install gh $PKGS",
        "brew install gh | tee log",
        "find . -name '*.rs' -exec grep -l gh {} +",
    ]);
}
