// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round five of #7131's review: the `case` word read with its quotes, a command in a case arm,
//! and the vacuous round-four tests made real.
//!
//! Reading the shell's text is the early warning, not the lock (Q331); the lock is the forge
//! credential (#7133).

use purlis_core::floorguard::release_floor_reason;
use purlis_core::mergehold;

fn assert_held(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

fn assert_passes(cmds: &[&str]) {
    for cmd in cmds {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "floored: {cmd}");
    }
}

/// Must-fix 1: a quote or an escaped backtick in a `case` word never puts back text that the
/// reading blanked.
#[test]
fn a_quoted_case_word_does_not_restore_what_was_blanked() {
    purlis_core::unsteered!();
    assert_held(&[
        "case \\`x in *) {gh,pr,merge,12};; y) \\` in;; esac",
        "case '$(' in *) {gh,pr,merge,12};; esac",
        "case '$(\")' in *) {gh,pr,merge,12};; esac",
        // A live substitution in the word is still read.
        "case $({gh,pr,merge,12}) in *) echo x;; esac",
        "case `{gh,pr,merge,12}` in *) echo x;; esac",
    ]);
}

/// Must-fix 2: a literal command in a case arm is a command, wherever the arm's pattern ends.
#[test]
fn a_literal_command_in_a_case_arm_is_read() {
    purlis_core::unsteered!();
    assert_held(&[
        "case x in x) gh pr merge 12;; esac",
        "case x in x) gh pr merge 12\nesac",
        "case $x in a|b) gh pr merge 12;; *) echo o;; esac",
        "case x in (x) gh pr merge 12;; esac",
    ]);
    assert_passes(&["case x in x) echo hi;; esac"]);
}

/// Nice-to-have: `dirname --` and `dirname --help`.
#[test]
fn dirname_accepts_a_double_dash_and_holds_help() {
    purlis_core::unsteered!();
    assert_passes(&["$(dirname -- tools/x)/check.sh"]);
    assert_held(&["$(dirname --help)/y"]);
}

/// Nice-to-have: the tests of round four that held for another reason.
#[test]
fn the_vacuous_round_four_tests_hold_for_their_own_reason() {
    purlis_core::unsteered!();
    assert_held(&[
        "$(which -a gh)",
        "$(git rev-parse HEAD)/y",
        "coproc case $x in *) {gh,pr,merge,12};; esac",
    ]);
}

/// Nice-to-have: shapes a script writes that must pass.
#[test]
fn common_case_words_and_interpreters_pass() {
    purlis_core::unsteered!();
    assert_passes(&[
        "case \"$(uname)\" in Darwin) echo m;; *) echo o;; esac",
        "case $(basename $(pwd)) in src) echo s;; *) echo o;; esac",
        "case ${x:-$(cmd)} in a) echo a;; *) echo o;; esac",
        "$PYTHON -c 'print(1)'",
    ]);
}
