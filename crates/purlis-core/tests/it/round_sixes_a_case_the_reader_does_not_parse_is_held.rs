// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round six of #7131's review (the last parser round): any `case` the reader does not fully
//! parse is held, and what it does parse is read as the shell reads it.
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

#[test]
fn a_case_header_that_does_not_parse_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "case ${x// /_} in *) gh pr merge 12;; esac",
        "case ${x:-a b} in *) gh pr merge 12;; esac",
        "case ${arr[1 + 1]} in *) gh pr merge 12;; esac",
        "case ${x:-)} in *) gh pr merge 12;; esac",
        "case ${x:-(} in *) gh pr merge 12;; esac",
        "case $[1 + 2] in 3) gh pr merge 12;; esac",
        "case <(true) in *) gh pr merge 12;; esac",
        "coproc c case x in x) gh pr merge 12;; esac",
    ]);
}

#[test]
fn a_case_pattern_that_does_not_end_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "case x in ${y:-;}|x) gh pr merge 12;; esac",
        "case x in ${y:-&}|x) gh pr merge 12;; esac",
        "case x in ${y:-<}|x) gh pr merge 12;; esac",
        "case x in ${y:->}|x) gh pr merge 12;; esac",
        "case x in $[ 1 > 0 ]|x) gh pr merge 12;; esac",
        "case x in <(true)|x) gh pr merge 12;; esac",
    ]);
}

#[test]
fn a_case_inside_any_substitution_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "echo \"$(case x in x) gh pr merge 12;; esac)\"",
        "x=\"$(case x in x) gh pr merge 12;; esac)\"",
        "echo \"${x:-$(case x in x) gh pr merge 12;; esac)}\"",
        "cat <(case x in x) gh pr merge 12;; esac)",
        "case \"$(case y in y) gh pr merge 12;; esac)\" in *) echo;; esac",
    ]);
}

#[test]
fn a_case_keeps_its_command_position_through_time_coproc_and_for() {
    purlis_core::unsteered!();
    assert_held(&[
        "time -p case x in x) gh pr merge 12;; esac",
        "for ((i=0;i<1;i++)) do case x in x) gh pr merge 12;; esac; done",
        "for ((i=0;i<1;i++)) { case x in x) gh pr merge 12;; esac; }",
    ]);
}

#[test]
fn a_function_body_on_the_same_line_is_read() {
    purlis_core::unsteered!();
    assert_held(&[
        "f() { gh pr merge 12; }; f",
        "f() { case x in x) gh pr merge 12;; esac; }; f",
        "f() case x in x) gh pr merge 12;; esac; f",
    ]);
}

#[test]
fn a_substitution_is_read_with_its_quotes() {
    purlis_core::unsteered!();
    assert_held(&[
        "echo \"$(echo ')'; gh pr merge 12)\"",
        "echo \"$(echo \\); gh pr merge 12)\"",
        "case $(echo ')'; {gh,pr,merge,12}) in *) echo x;; esac",
    ]);
}

/// In bash an assignment, a redirection or a substitution before `case` makes it no reserved
/// word, so what follows the "header" is a real subshell.
#[test]
fn a_case_after_an_assignment_or_a_redirection_is_no_case() {
    purlis_core::unsteered!();
    assert_held(&[
        "A=1 case x in\n( {gh,pr,merge,12} )",
        ">/dev/null case x in\n( {gh,pr,merge,12} )",
        "x=(a b) case x in\n( {gh,pr,merge,12} )",
        "cat <(true) case x in\n( {gh,pr,merge,12} )",
    ]);
}

#[test]
fn a_case_word_with_a_quoted_dollar_is_still_a_case() {
    purlis_core::unsteered!();
    assert_passes(&[
        "case \"$a\"-$(uname) in Linux*) echo hi;; *) echo o;; esac",
        "case \"$prefix\"$(uname) in *) echo hi;; esac",
        "case 'v'$(uname) in *) echo hi;; esac",
        "case ${1:-default value} in a) echo a;; *) echo o;; esac",
    ]);
}
