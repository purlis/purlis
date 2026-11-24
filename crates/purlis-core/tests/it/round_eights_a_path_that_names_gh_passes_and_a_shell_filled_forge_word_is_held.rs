// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round 8 of the merge guard (#7423): the general wrapper rule of round 7 held what only names
//! gh (a path, a glob next to it) and held more behind an unknown program than the call holds
//! without it. A forge word that is a path, or is followed by a word the shell fills in, is a
//! call only where a held verb stands after it. Behind an unknown program the call is read as the
//! direct call is. zsh's append-both operators are operators. A word the shell fills in, followed
//! by a held noun and verb, may be the forge CLI.

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
fn a_path_that_names_gh_with_no_held_verb_after_it_passes() {
    purlis_core::unsteered!();
    passes(&[
        "cp /opt/homebrew/bin/gh ~/bin/",
        "ls -l /opt/homebrew/bin/gh ~/bin/gh",
        "ln -s /opt/homebrew/bin/gh ~/bin/gh",
        "ls ~/.config/gh ~/.config/glab",
        "du -sh ~/.config/gh ~/.config/glab",
        "cp -r ~/.config/gh $TMPDIR/x",
        "ls -la .purlis/app/forge-wire/chat-1/bin/gh .purlis/app/tools/*/bin",
        "git add crates/gh src/*.rs",
        "git log -- cmd/gh pkg/*",
    ]);
}

#[test]
fn a_path_or_an_opaque_word_after_gh_with_a_held_verb_after_it_is_still_held() {
    purlis_core::unsteered!();
    held(&[
        "frobnicate /usr/bin/gh pr merge 5",
        "frobnicate ~/bin/gh pr merge 5",
        "frobnicate gh $V merge 5",
        "frobnicate gh pr $V 5",
        "ssh host gh pr merge 5",
        "chronic glab mr accept 5",
    ]);
}

#[test]
fn zsh_append_both_operators_are_one_operator_and_their_target_is_the_next_word() {
    purlis_core::unsteered!();
    held(&[
        "gh >>& f pr merge 5",
        "gh >>&| f pr merge 5",
        "gh >>&! f pr merge 5",
        "gh &>>| f pr merge 5",
        "gh &>>! f pr merge 5",
        "glab >>& f mr merge 5",
    ]);
    passes(&["gh >>& f pr view 5", "gh &>>| f pr view 5"]);
}

#[test]
fn a_word_the_shell_fills_in_before_a_held_noun_and_verb_is_held_as_a_forge_cli() {
    purlis_core::unsteered!();
    held(&[
        "chronic $(which gh) pr merge 5",
        "GH=gh; chronic $GH pr merge 5",
        "chronic {gh,pr} merge 5",
        "chronic /opt/homebrew/bin/g[h] pr merge 5",
        "chronic purlis change land",
        "chronic charter change land",
        "chronic $(which glab) mr merge 5",
    ]);
    passes(&[
        "chronic $(which gh) pr view 5",
        "chronic $GH pr list",
        "ls $dir pr",
    ]);
}

#[test]
fn behind_an_unknown_program_only_what_the_direct_call_holds_is_held() {
    purlis_core::unsteered!();
    passes(&[
        "op run -- glab mr create",
        "op run -- gh repo sync",
        "op run -- gh release create v1",
        "ssh host gh repo sync",
    ]);
    held(&[
        "op run -- gh api -X PUT repos/o/r/pulls/5/merge",
        "op run -- gh api repos/o/r/pulls/5/merge $X",
        "op run -- gh api --input f repos/o/r",
        "op run -- glab mr create --auto-merge",
        "op run -- gh repo sync --force",
    ]);
    // The release floor holds a release where it is run directly, so it does behind a wrapper.
    assert!(release_floor_reason("op run -- gh release create v1", true).is_some());
    assert!(release_floor_reason("gh release create v1", true).is_some());
}

#[test]
fn bare_words_that_are_arguments_pass_where_the_call_after_them_is_a_read() {
    purlis_core::unsteered!();
    passes(&["git grep gh api", "man gh api"]);
}

/// An accepted over-hold: a note whose words are a merge call is the same words as a wrapper's.
#[test]
fn a_note_whose_words_are_a_merge_call_is_held_as_a_wrapper_would_be() {
    purlis_core::unsteered!();
    held(&["purlis memory add never run gh pr merge yourself"]);
}

/// A plain read whose endpoint ends in the variable of the `for` that runs it (main's
/// `a_plain_api_read_inside_a_substitution_is_no_merge`).
#[test]
fn a_plain_api_read_in_a_for_loop_is_no_merge() {
    purlis_core::unsteered!();
    // Round 9: the variable is quoted; an unquoted one is split and globbed.
    passes(&[
        r#"for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api "repos/o/r/pulls/$id"; done"#,
    ]);
}

/// What the loop's list makes is one word, but a flag, a write, a quoted list or an unplain
/// substitution in it still holds.
#[test]
fn a_for_loop_over_a_substitution_holds_what_it_could_write() {
    purlis_core::unsteered!();
    held(&[
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api -X PUT repos/o/r/pulls/$id/merge; done",
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api repos/o/r/pulls/$id --input f; done",
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api repos/o/r/pulls/$id -f a=b; done",
        "for id in \"$(cat f)\"; do gh api repos/o/r/pulls/$id; done",
        "for id in $(echo \"5 -XPUT\"); do gh api repos/o/r/pulls/$id; done",
        "for id in $(gh api repos/o/r/pulls); do gh api $id; done",
    ]);
}
