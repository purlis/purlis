// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! PR #7195 round 5: a brace expansion in a writing call's path is held, a heredoc delimiter
//! bash does not end at a CR, FF, VT or NBSP is held, and `time` with an option says so.

use purlis_core::floorguard::release_floor_reason;
use purlis_core::mergehold;

fn assert_held(cmds: &[&str]) {
    for cmd in cmds {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
    }
}

#[test]
fn a_brace_expansion_in_a_writing_calls_path_is_held() {
    purlis_core::unsteered!();
    assert_held(&[
        "gh api -X PUT repos/o/r/{issues/1,pulls/5/merge}",
        "gh api -X PUT repos/o/r/pulls/{5/merge,6}",
        "glab api -X PUT projects/p/{issues/1,merge_requests/5/merge}",
        "gh api -X PATCH repos/o/r/git/refs/heads/{main,x} -f sha=abc",
        "gh api repos/o/r/pulls/{5/merge,6} -f title=x",
    ]);
}

#[test]
fn a_brace_expansion_in_a_reads_path_still_passes() {
    purlis_core::unsteered!();
    assert_eq!(mergehold::held("gh api repos/o/r/pulls/{1,2}"), None);
}

#[test]
fn a_heredoc_delimiter_bash_does_not_end_at_a_blank_like_character_is_held() {
    purlis_core::unsteered!();
    for ws in ["\r", "\u{c}", "\u{b}", "\u{a0}"] {
        let plain = format!("echo \"$(cat <<EOF{ws}\nEOF\n)\nEOF{ws}\ngh pr merge 12)\"");
        let quoted = format!("echo \"$(cat <<'EOF'{ws}\nEOF\n)\nEOF{ws}\ngh pr merge 12)\"");
        assert_held(&[&plain, &quoted]);
    }
    assert_eq!(
        release_floor_reason("echo \"$(cat <<EOF\nhello\nEOF\n)\"", true),
        None,
        "a normal heredoc is read past"
    );
}

#[test]
fn time_with_an_option_says_it_has_an_option() {
    purlis_core::unsteered!();
    let why = release_floor_reason("time -- cargo build", true).expect("held");
    assert!(why.contains("`time` has an option"), "{why}");
    assert!(!why.contains("`case`"), "{why}");
}
