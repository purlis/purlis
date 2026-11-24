// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round 12 of the merge guard (#7423). Two holes of round 11's allowlist:
//!
//! - `never_executes` took the first non-flag word as the subcommand, so a flag's value that is a
//!   listed subcommand (`git --namespace log bisect run gh …`, `npm --loglevel info exec -- gh …`)
//!   hid the real one. Names-only now holds only when the subcommand is the first word, or follows
//!   git's `-C <v>` / `-c <v>` pairs and nothing else.
//! - `find`'s `-exec` run `gh -R {}` read `{}` as a plain `-R` value, and the run ended with its
//!   verb still to come. In a `find` run `{}` is a word the shell fills in, wherever it stands,
//!   and a run that could still be completed is held.
//!
//! The related `echo o/r pr merge 5 | xargs gh -R` ends the same way and is held by the fed-call
//! rule (`placed::may_be_completed` on a call behind a program, `floorguard.rs`'s `behind` loop).

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
fn a_flag_value_that_is_a_subcommand_does_not_hide_the_one_that_runs() {
    purlis_core::unsteered!();
    held(&[
        "git --namespace log bisect run gh pr merge 5",
        "git --namespace log submodule foreach gh pr merge 5",
        "git --git-dir X log bisect run gh pr merge 5",
        "git --work-tree X log bisect run gh pr merge 5",
        "git --config-env X log bisect run gh pr merge 5",
        "git --attr-source X log bisect run gh pr merge 5",
        "git --git-dir X log submodule foreach gh pr merge 5",
        "git --work-tree X log submodule foreach gh pr merge 5",
        "git --config-env X log submodule foreach gh pr merge 5",
        "git --attr-source X log submodule foreach gh pr merge 5",
        "pnpm --loglevel info exec gh pr merge 5",
        "npm --loglevel info exec -- gh pr merge 5",
        "mise -E list exec -- gh pr merge 5",
    ]);
}

#[test]
fn a_find_placeholder_is_a_word_the_shell_fills_in() {
    purlis_core::unsteered!();
    held(&[
        "mkdir -p o/r && touch pr merge 5 && find o/r pr merge 5 -maxdepth 0 -exec gh -R {} +",
        "find o/r pr merge 5 -maxdepth 0 -execdir gh -R {} +",
        "find pr merge 5 -maxdepth 0 -exec gh -R o/r +",
        "find pr merge 5 -maxdepth 0 -execdir gh -R o/r +",
        "find o/r pr merge 5 -maxdepth 0 -ok gh -R {} \\;",
    ]);
}

#[test]
fn a_call_that_a_program_in_front_may_complete_is_held() {
    purlis_core::unsteered!();
    held(&["echo o/r pr merge 5 | xargs gh -R"]);
}

#[test]
fn names_and_plain_reads_still_pass() {
    purlis_core::unsteered!();
    passes(&[
        "git -C repo log --oneline",
        "git -c core.pager=cat log",
        "npm install",
        "find . -name '*.rs' -exec grep -l gh {} +",
        "find . -exec gh pr view 12 \\;",
    ]);
}
