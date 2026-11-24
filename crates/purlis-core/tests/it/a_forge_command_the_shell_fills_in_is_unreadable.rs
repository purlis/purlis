// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A forge command whose program, noun or verb the shell fills in is one purlis cannot read,
//! and the release floor and the merge hold treat it as the act it could be (#7124).
//!
//! `$(echo gh) pr merge 12` and `gh pr $(echo merge) 12` merge request 12, but neither reader
//! saw a `gh pr merge`: the program was `$(…)`, the verb `$(…)`. A word the shell fills in, in
//! a substitution, backticks, a variable, `${…}` or glued to letters, is now read as what it
//! could be. A program is read as each CLI the floor knows, and refused where one of them would
//! be. A forge CLI's noun, or its verb after a noun that has a held verb, could be any of them,
//! so it is unreadable outright. A word the shell fills in after a verb that is read and holds
//! nothing is an argument, and stays allowed.

use purlis_core::floorguard::release_floor_reason;
use purlis_core::handoffguard::Caller;
use purlis_core::mergehold::{self, Held};
use purlis_core::toolgate::{Arm, Call, Launched, Plane, verdict};

/// Merges whose program the shell fills in.
const PROGRAM_FILLED_IN: &[&str] = &[
    "$(echo gh) pr merge 12",
    "`echo gh` pr merge 12",
    "$GH pr merge 12",
    "${GH} pr merge 12",
    "g$(echo h) pr merge 12",
    "g${H} pr merge 12",
    "\"$GH\" pr merge 12",
    "env $GH pr merge 12",
    "cd /tmp && $(echo gh) pr merge 12",
    "$(echo glab) mr merge 3",
    "$GLAB mr accept 3",
    "$(echo gh) api -X PUT repos/o/r/pulls/12/merge",
    "$GH api -X PUT repos/o/r/pulls/12/merge",
];

/// Merges whose noun or verb the shell fills in.
const NOUN_OR_VERB_FILLED_IN: &[&str] = &[
    "gh pr $(echo merge) 12",
    "gh pr `echo merge` 12",
    "gh pr $V 12",
    "gh pr ${V} 12",
    "gh pr mer$(echo ge) 12",
    "gh pr mer${GE} 12",
    "gh $(echo pr) merge 12",
    "gh $N merge 12",
    "gh ${N} merge 12",
    "gh $CMD",
    "gh -R o/r pr $V 12",
    "glab mr $(echo merge) 3",
    "glab $N merge 3",
    "glab mr ac${CEPT} 3",
    "gh $(echo api) -X PUT repos/o/r/pulls/12/merge",
    "glab $API -X PUT projects/1/merge_requests/3/merge",
];

/// Publishes whose program, verb or flag the shell fills in: each is refused unattended.
const PUBLISHES: &[&str] = &[
    "$(echo gh) release create v1",
    "`echo gh` release create v1",
    "$GH release create v1",
    "gh release $(echo create) v1",
    "gh release cre${ATE} v1",
    "$(echo glab) release create v1",
    "glab release $V v1",
    "$(echo git) push --tags",
    "`echo git` push --tags",
    "$GIT push --tags",
    "${GIT} push --tags",
    "gi$(echo t) push --tags",
    "git $(echo push) --tags",
    "git `echo push` --tags",
    "git $SUB --tags",
    "git ${SUB} --tags",
    "git pu$(echo sh) --tags",
    "git push --$(echo tags)",
    "git push \"--$WHICH\"",
    "$(echo gh) api -X POST repos/o/r/releases -f tag_name=v1",
];

#[test]
fn a_merge_whose_program_the_shell_fills_in_is_held_as_unreadable() {
    purlis_core::unsteered!();
    for cmd in PROGRAM_FILLED_IN {
        assert_eq!(mergehold::held(cmd), Some(Held::Unreadable), "{cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
        assert_eq!(release_floor_reason(cmd, false), None, "attended: {cmd}");
    }
}

#[test]
fn a_merge_whose_noun_or_verb_the_shell_fills_in_is_held_as_unreadable() {
    purlis_core::unsteered!();
    for cmd in NOUN_OR_VERB_FILLED_IN {
        assert_eq!(mergehold::held(cmd), Some(Held::Unreadable), "{cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
}

#[test]
fn a_publish_whose_program_verb_or_flag_the_shell_fills_in_is_refused_unattended() {
    purlis_core::unsteered!();
    for cmd in PUBLISHES {
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
        assert_eq!(release_floor_reason(cmd, false), None, "attended: {cmd}");
    }
}

/// The whole gate, for a chat nobody watches that purlis did not start: the floor refuses each.
/// A git line whose subcommand the shell fills in is refused first by the plane-root guard,
/// which cannot tell it is no branch move, and that refusal stands as it did before.
#[test]
fn the_gate_refuses_each_unattended() {
    purlis_core::unsteered!();
    let state = std::env::temp_dir();
    let forges = Vec::new();
    let plane = Plane {
        root: "/nowhere/plane",
        forges: &forges,
        session_dir: "",
        launched: Launched {
            sandboxed: false,
            chat_dir: None,
            attendance: None,
        },
    };
    for cmd in PROGRAM_FILLED_IN
        .iter()
        .chain(NOUN_OR_VERB_FILLED_IN)
        .chain(PUBLISHES)
    {
        let call = Call {
            command: cmd,
            cwd: "/nowhere/plane",
            state_dir: &state,
            caller: Caller {
                agent_id: None,
                harness: Some("claude"),
                permission_mode: Some("bypassPermissions"),
            },
        };
        let arm = verdict(&call, Some(&plane)).and_then(|said| said.arm);
        let git_sub_filled_in = cmd.starts_with("git ") && !cmd.starts_with("git push ");
        if git_sub_filled_in && arm == Some(Arm::PlaneRootBranch) {
            continue;
        }
        assert_eq!(arm, Some(Arm::ReleaseFloor), "{cmd}");
    }
}

/// A word the shell fills in where it cannot make a merge or a publish is read as before: an
/// argument after a verb that is read, a program whose words make no held command as any CLI,
/// and a read through the raw API.
#[test]
fn a_word_the_shell_fills_in_where_it_cannot_merge_or_publish_stays_allowed() {
    purlis_core::unsteered!();
    for cmd in [
        "gh pr view $(cat n)",
        "gh pr view `cat n`",
        "gh pr view $N",
        "gh pr checks ${N} --watch",
        "gh pr view 12 --json $(echo state)",
        "gh -R \"$REPO\" pr view 12",
        "gh issue $(echo view) 3",
        "gh run watch $(gh run list -L1 --json databaseId -q '.[0].databaseId')",
        "gh api \"repos/o/r/pulls/$N\"",
        "glab mr view $(cat n)",
        "$EDITOR notes.md",
        "\"$HOME/.cargo/bin/cargo\" test -p purlis-core",
        "$(git rev-parse --show-toplevel)/tools/check.sh",
        "$PYTHON -m pytest",
        "${SHELL} -c 'echo hi'",
        "git push origin $BRANCH",
        "git push -u origin \"$(git branch --show-current)\"",
        "git -C $DIR status",
        "git log $(git merge-base HEAD main)..HEAD",
    ] {
        assert_eq!(mergehold::held(cmd), None, "held: {cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "floored: {cmd}");
    }
}
