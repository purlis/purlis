// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round 10 of the merge guard (#7423): nine rounds closed spellings one at a time. The rule they
//! were instances of is one: **a word the shell may fill in, standing at or before a forge call's
//! verb (as the noun, the verb, a flag, or a flag's value), makes the call unreadable, and it is
//! held**, whatever way the word is filled and wherever the call stands (directly, behind a
//! program no table knows, or in a `for` over a forge answer). A word is literal only when the
//! lexer says it has no expansion outside single quotes.

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
fn a_quoted_method_flag_in_a_for_over_a_forge_answer_makes_the_call_a_write() {
    purlis_core::unsteered!();
    held(&[
        "for id in $(gh api repos/o/r --jq '\"merge\"'); do gh api '-XPUT' \"repos/o/r/pulls/5/$id\"; done",
        "for id in $(gh api repos/o/r --jq '\"merge\"'); do gh api -X\"PUT\" \"repos/o/r/pulls/5/$id\"; done",
        "for id in $(gh api repos/o/r --jq '\"merge\"'); do gh api --method=PUT \"repos/o/r/pulls/5/$id\"; done",
        "for id in $(gh api repos/o/r --jq '\"merge\"'); do gh api \"--method\" PUT \"repos/o/r/pulls/5/$id\"; done",
    ]);
}

#[test]
fn a_noun_or_verb_that_is_quoted_around_an_expansion_is_still_filled_in() {
    purlis_core::unsteered!();
    held(&[
        "V=merge; chronic /opt/homebrew/bin/gh pr ''$V'' 5",
        "A=pr B=merge; chronic gh ''$A'' ''$B'' 5",
        "V=merge; chronic gh pr ''$V'' 5",
        "V=merge; gh pr ''$V'' 5",
    ]);
}

#[test]
fn a_slash_inside_an_expansion_does_not_make_a_word_a_path() {
    purlis_core::unsteered!();
    held(&[
        "V=merge; chronic /opt/homebrew/bin/gh pr ${V%/} 5",
        "V=merge; chronic /opt/homebrew/bin/gh pr ${V/x/y} 5",
        "V=merge; chronic /opt/homebrew/bin/gh pr ${V:-a/b} 5",
        "V=merge; gh pr ${V%/} 5",
    ]);
}

#[test]
fn a_value_flag_before_the_verb_shifts_the_noun_and_the_verb() {
    purlis_core::unsteered!();
    held(&[
        "V=merge; chronic /opt/homebrew/bin/gh pr -b x $V 5",
        "V=merge; gh pr -b view $V 5",
        "A=pr B=merge; gh -b x $A $B 5",
        "A=pr B=merge; chronic gh -b x $A $B 5",
    ]);
}

#[test]
fn a_bare_brace_pair_beside_an_endpoint_behind_a_program_is_a_filled_in_flag() {
    purlis_core::unsteered!();
    held(&[
        "frob gh api repos/o/r/pulls/5/merge {}",
        "frob glab api projects/1/merge_requests/5/merge {}",
        "fd . repos -x gh api repos/o/r/pulls/5/merge {}",
    ]);
    passes(&[]);
}

#[test]
fn a_repository_flag_the_shell_fills_in_is_held_and_one_in_double_quotes_is_a_value() {
    purlis_core::unsteered!();
    held(&[
        "gh -R $(cat f)",
        "gh -R $X",
        "gh -R $REPO pr view 12",
        "gh pr --repo=$(echo o/r merge 5)",
        "gh pr -$X",
        "chronic gh pr -R $X",
        "chronic /opt/homebrew/bin/gh pr -R $X 5",
        "gh pr view -R $X 5 --json $Y; gh -R $X",
    ]);
    passes(&[
        "gh -R \"$REPO\" pr view 12",
        "gh pr view 12 -R $REPO",
        "gh pr --repo=\"$REPO\" view 12",
        "chronic gh -R \"$REPO\" pr view 12",
    ]);
}

#[test]
fn words_a_program_in_front_of_gh_hands_it_could_complete_a_merge() {
    purlis_core::unsteered!();
    held(&[
        "echo pr merge 5 | gxargs gh",
        "echo pr merge 5 | xe gh",
        "echo merge 5 | xe gh pr",
        "echo pr merge 5 | xargs gh",
        "gxargs gh <<< 'pr merge 5'",
        "echo -XPUT | gxargs gh api repos/o/r/pulls/5/merge",
    ]);
    // Nothing is handed to it, or its noun and verb are already a read.
    passes(&[
        "echo 12 | gxargs gh pr view",
        "which gh | cat",
        "brew install gh | tee log",
        "echo hi | cat",
    ]);
}

#[test]
fn a_path_or_a_package_that_names_gh_is_no_call() {
    purlis_core::unsteered!();
    passes(&[
        "cp /opt/homebrew/bin/gh ~",
        "cp target/release/gh \"$DEST\"",
        "git add crates/gh *.md",
        "ls -la ~/.config/gh *",
        "brew install gh $PKGS",
        "diff /usr/bin/gh $(which gh)",
    ]);
}

#[test]
fn what_was_read_before_is_read_still() {
    purlis_core::unsteered!();
    passes(&[
        "gh pr view 12",
        "gh api repos/o/r/pulls/1",
        "for f in a b; do gh pr view \"$f\"; done",
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api \"repos/o/r/pulls/$id\"; done",
        "gh pr view $N",
        "gh pr view 12 --json $(echo state)",
        "chronic gh pr view 12",
        "chronic /opt/homebrew/bin/gh pr view \"$N\"",
    ]);
}

/// How a word may be filled in, each a spelling of one expansion that is open.
const FILLS: [&str; 7] = [
    "$V",
    "''$V''",
    "${V%/}",
    "$(cat v)",
    "m*",
    "{merge,view}",
    "\"$V\"''$V",
];

/// Where a call may stand.
const PREFIXES: [&str; 3] = ["", "chronic ", "chronic /opt/homebrew/bin/"];

/// Each shape of a read, with `@` at a word at or before its verb: the noun, the verb, a
/// repository flag's value, and the word after a value flag, which may be the verb.
const SHAPES_AT_THE_VERB: [&str; 5] = [
    "gh @ view 12",
    "gh pr @ 12",
    "gh -R @ pr view 12",
    "gh pr -b x @ 12",
    "gh pr -b view @ 12",
];

#[test]
fn every_read_shape_with_a_fill_at_or_before_its_verb_is_held() {
    purlis_core::unsteered!();
    for shape in SHAPES_AT_THE_VERB {
        for fill in FILLS {
            for prefix in PREFIXES {
                // A path to gh that a filled-in word follows is read as a file name unless a
                // literal noun or a held verb stands after it (`cp /usr/bin/gh "$DEST"`), so the
                // shape that fills the noun is held behind a path only with a held verb later.
                if prefix.ends_with('/') && shape.starts_with("gh @") {
                    continue;
                }
                let cmd = format!("V=merge; {prefix}{shape}").replace('@', fill);
                held(&[&cmd]);
            }
        }
    }
}

#[test]
fn every_read_shape_with_a_fill_after_its_verb_passes() {
    purlis_core::unsteered!();
    for shape in [
        "gh pr view @",
        "gh issue view @",
        "gh run list --json @",
        "gh pr view 12 --json @",
    ] {
        for fill in ["$V", "\"$V\"", "$(cat v)", "m*"] {
            let cmd = shape.replace('@', fill);
            passes(&[&cmd]);
        }
    }
}

#[test]
fn a_method_flag_in_any_spelling_makes_an_api_call_with_a_filled_in_path_a_write() {
    purlis_core::unsteered!();
    for flag in [
        "-XPUT",
        "'-XPUT'",
        "-X\"PUT\"",
        "--method=PUT",
        "\"--method=PUT\"",
        "-X PUT",
        "--method PUT",
    ] {
        let cmd = format!(
            "for id in $(gh api repos/o/r --jq '\"merge\"'); do gh api {flag} \"repos/o/r/pulls/5/$id\"; done"
        );
        held(&[&cmd]);
    }
}
