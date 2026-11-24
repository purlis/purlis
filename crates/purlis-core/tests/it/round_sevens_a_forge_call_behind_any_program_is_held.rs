// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Round 7 of the merge guard (#7423): general rules, not more lists. A forge CLI named behind a
//! program the tables do not know, with a call the floor holds after it, is held whatever the
//! program is. zsh's clobber operators, `+=` assignments, zsh's short forms, dash's `&>`,
//! `env -S`'s separators and a bare `"$@"` are each read as the shell reads them. The reads the
//! earlier rounds held by mistake pass.

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
fn a_merge_behind_a_program_no_table_lists_is_held_and_says_what_to_do() {
    purlis_core::unsteered!();
    held(&[
        "taskset 1 gh pr merge 5",
        "chronic gh pr merge 5",
        "ssh h gh pr merge 5",
        "fd -x gh pr merge 5",
        "frobnicate gh pr merge 5",
        "frobnicate glab mr merge 5",
        "frobnicate gh api -X PUT repos/o/r/pulls/5/merge",
        "frobnicate gh api repos/o/r/pulls/5/merge -f merge_method=squash",
        "sudo -u a ssh h gh pr merge 5",
        "frobnicate gh -b x pr merge 5",
        "frobnicate gh alias set m 'pr merge'",
        "frobnicate /usr/bin/gh pr merge 5",
        "frobnicate gh pr $V 5",
    ]);
    let said = release_floor_reason("ssh h gh pr merge 5", true).expect("a refusal");
    assert!(said.contains("put gh first in the command"), "{said}");
}

#[test]
fn timeout_takes_one_leading_operand_as_its_duration_whatever_it_looks_like() {
    purlis_core::unsteered!();
    held(&[
        "timeout .5 gh pr merge 5",
        "timeout 5. gh pr merge 5",
        "timeout 1e1 gh pr merge 5",
        "timeout 0x10 gh pr merge 5",
        "timeout +5 gh pr merge 5",
        "timeout -k 5 .5 gh pr merge 5",
        "env timeout .5 gh pr merge 5",
        "gtimeout inf gh pr merge 5",
        "timeout 5 gh pr merge 5",
        "timeout --foreground .5 gh pr merge 5",
        "find . -exec timeout .5 gh pr merge 5 \\;",
        "find . -exec timeout 1e1 glab mr merge 5 \\;",
    ]);
}

#[test]
fn zsh_clobber_operators_are_one_operator_and_their_target_is_the_next_word() {
    purlis_core::unsteered!();
    held(&[
        "gh >! f pr merge 5",
        "env -u A gh pr >! f merge 5",
        "gh >>! f pr merge 5",
        "gh >| f pr merge 5",
        "gh >>| f pr merge 5",
        "gh &>! f pr merge 5",
        "gh &>| f pr merge 5",
        "gh >&! f pr merge 5",
        "gh >&| f pr merge 5",
        "gh 2>! f pr merge 5",
        "nice -n 5 >! f gh pr merge 5",
        "glab >! f mr merge 5",
        // Read as bash reads them, the `!` is the target and the call is another.
        "gh >! pr merge 5",
    ]);
}

#[test]
fn a_plus_equals_assignment_is_an_assignment() {
    purlis_core::unsteered!();
    held(&[
        "A+=1 gh pr merge 5",
        "A=1 B+=2 gh pr merge 5",
        "A+=1 env B+=2 gh pr merge 5",
    ]);
}

#[test]
fn zsh_short_forms_that_run_a_command_are_held() {
    purlis_core::unsteered!();
    held(&[
        "repeat 3 gh pr merge 5",
        "for i (1 2) gh pr merge $i",
        "foreach i (1 2); gh pr merge $i; end",
        "if [[ -n x ]] gh pr merge 5",
        "repeat 2 glab mr merge 5",
    ]);
}

#[test]
fn dash_reads_an_ampersand_greater_than_as_two_operators() {
    purlis_core::unsteered!();
    held(&[
        "dash -c 'echo &>f gh pr merge 5'",
        "sh -c 'echo &>f gh pr merge 5'",
        "dash -c 'echo &>>f gh pr merge 5'",
        "dash -c \"echo &>f glab mr merge 5\"",
    ]);
}

#[test]
fn env_dash_s_is_split_as_env_splits_it() {
    purlis_core::unsteered!();
    held(&[
        "env -S 'gh pr merge 5'",
        "env -S 'gh\\_pr\\_merge\\_5'",
        "env --split-string='gh\\_pr\\_merge\\_5'",
        "env -S 'timeout\\_.5\\_gh\\_pr\\_merge\\_5'",
        "env -S '${X} pr merge 5' gh",
        // A string with a forge CLI in it is held whatever it does: it is unpacked, not read.
        "env -S 'gh pr view 5'",
        "env -S 'gh\\_pr\\_view\\_5'",
    ]);
}

#[test]
fn a_bare_dollar_at_as_the_program_is_held() {
    purlis_core::unsteered!();
    held(&[
        "bash -c '\"$@\"' _ gh pr merge 5",
        "bash -c '$0 \"$@\"' gh pr merge 5",
        "bash -c 'exec \"$@\"' _ gh pr merge 5",
    ]);
}

#[test]
fn the_reads_the_round_held_by_mistake_pass() {
    purlis_core::unsteered!();
    passes(&[
        "gh pr view 5",
        "gh api repos/o/r/pulls/5",
        "timeout 60 gh pr view 5",
        "timeout --foreground 5 gh pr view 5",
        "timeout --preserve-status 5 gh pr view 5",
        "timeout --verbose 5 gh pr view 5",
        "timeout .5 gh pr view 5",
        "sudo --preserve-env gh pr view 5",
        "sudo --preserve-env=PATH gh pr view 5",
        "sudo --login gh pr view 5",
        "sudo --non-interactive gh pr view 5",
        "env --ignore-environment gh pr view 5",
        "watch --no-title gh pr view 5",
        "watch --color gh pr view 5",
        "nice -10 gh pr view 5",
        "ssh h gh pr view 5",
        "taskset 1 gh pr list",
        "fd -x gh pr view 5",
        "A+=1 gh pr view 5",
        "gh >>| f pr view 5",
        "echo gh pr merge 5",
        "printf '%s\\n' gh pr merge 5",
        "for i in gh pr merge; do echo $i; done",
        "dash -c 'echo &>f gh pr view 5'",
        "bash -c 'echo \"$@\"' _ gh pr view 5",
        "find . -exec timeout .5 gh pr view 5 \\;",
    ]);
}
