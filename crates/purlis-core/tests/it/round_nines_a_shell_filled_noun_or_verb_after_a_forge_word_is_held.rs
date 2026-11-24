// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round 9 of the merge guard (#7423): round 8's skip for a forge word that is a path, or is
//! followed by a word the shell fills in, opened merges. The skip applies only where no word the
//! shell fills in stands as the noun or the verb after the forge word; otherwise the call is read
//! as the direct call is. A bare `{}` behind a program that is not gh is a filled-in endpoint in
//! a write. A `for` variable from a forge answer is read only quoted.

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
fn a_path_to_gh_followed_by_a_shell_filled_verb_is_held() {
    purlis_core::unsteered!();
    held(&[
        "V=merge; chronic /opt/homebrew/bin/gh pr $V 5",
        "chronic ~/bin/gh pr $V 5",
        "chronic ./gh pr $V 5",
        "chronic /opt/homebrew/bin/gh pr $(cat v) 5",
        "chronic /opt/homebrew/bin/gh pr m* 5",
        "chronic /opt/homebrew/bin/glab mr $V 5",
        "chronic /opt/homebrew/bin/gh pr -R o/r $V 5",
    ]);
}

#[test]
fn a_bare_gh_followed_by_a_shell_filled_word_is_held() {
    purlis_core::unsteered!();
    held(&[
        "A=pr B=merge; chronic gh $A $B 5",
        "chronic gh $ARGS",
        "chronic gh \"$@\"",
        "chronic gh $(cat args)",
        "chronic gh *",
        "A=api; chronic gh $A -X PUT repos/o/r/pulls/5/$M",
    ]);
}

#[test]
fn a_path_or_a_glob_that_is_an_argument_next_to_a_forge_word_still_passes() {
    purlis_core::unsteered!();
    passes(&[
        "cp /opt/homebrew/bin/gh ~/bin/",
        "ls ~/.config/gh ~/.config/glab",
        "git add crates/gh src/*.rs",
        "git log -- cmd/gh pkg/*",
        "ls -l /opt/homebrew/bin/gh ~/bin/gh",
        "cp -r ~/.config/gh $TMPDIR/x",
        "ls -la .purlis/app/forge-wire/chat-1/bin/gh .purlis/app/tools/*/bin",
    ]);
}

#[test]
fn a_bare_brace_pair_behind_a_program_that_is_not_gh_is_a_filled_in_endpoint_in_a_write() {
    purlis_core::unsteered!();
    held(&[
        "fd . repos -x gh api -X PUT {}",
        "frob gh api -X PUT {}",
        "frob glab api -X PUT {}",
    ]);
    // A read behind the program, and gh's own `{owner}` placeholder, are no write of a filled name.
    passes(&[]);
}

#[test]
fn a_for_variable_from_a_forge_answer_is_read_only_when_it_is_quoted() {
    purlis_core::unsteered!();
    // With a planted file named `merge -XPUT`, an unquoted `mer*` is two words, one a flag.
    held(&[
        "for id in $(gh api repos/o/r --jq '\"mer*\"'); do gh api repos/o/r/pulls/5/$id; done",
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api repos/o/r/pulls/$id; done",
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api \"$id\"; done",
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api repos/o/r/'pulls/$id'/x $id; done",
    ]);
    passes(&[
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api \"repos/o/r/pulls/$id\"; done",
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api repos/o/r/pulls/\"$id\"; done",
        "for id in $(gh api repos/o/r/pulls --jq '.[].number'); do gh api \"repos/o/r/pulls/${id}/files\"; done",
    ]);
}
