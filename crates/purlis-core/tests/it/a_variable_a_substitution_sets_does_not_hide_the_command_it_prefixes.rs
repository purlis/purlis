// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A command prefixed with a variable that a substitution sets is still that command, to the
//! release floor, to the merge hold and to the whole gate (#7092).
//!
//! `A=$(true) gh pr merge 12` runs `true`, keeps its output in `A`, and runs `gh pr merge 12`.
//! The floor read every segment with each substitution's words handed out to it, so this one
//! was `A=$ true gh pr merge 12`: an assignment `A=$`, then a program `true`, which merges
//! nothing. An unattended chat could merge, tag, push tags or cut a release behind any such
//! prefix. The floor now reads each command's own words too, with each substitution one word in
//! its place, `A=$(…) gh pr merge 12`, whose program is `gh`.

use purlis_core::floorguard::release_floor_reason;
use purlis_core::handoffguard::Caller;
use purlis_core::mergehold::{self, Held};
use purlis_core::toolgate::{Arm, Call, Launched, Plane, verdict};

/// Merges behind a variable a substitution sets: in `$(…)`, in backticks, nested, quoted
/// around, several, behind `env`, and through each forge's CLI and the raw API.
const MERGES: &[&str] = &[
    "A=$(true) gh pr merge 12",
    "A=`true` gh pr merge 12",
    "A=$(echo $(true)) gh pr merge 12",
    "A=$(true)x gh pr merge 12",
    "A=x$(true) gh pr merge 12 --squash",
    "A=$(true) B=$(true) gh pr merge 12",
    "A=1 B=$(true) gh pr merge 12",
    "env A=$(true) gh pr merge 12",
    "A=$(true) glab mr merge 3",
    "A=$(true) gh api -X PUT repos/o/r/pulls/12/merge",
    "cd /tmp && A=$(true) gh pr merge 12",
    "A=$(true) bash -c 'gh pr merge 12'",
];

/// Publishes behind a variable a substitution sets.
const PUBLISHES: &[&str] = &[
    "A=$(true) gh release create v1",
    "A=`true` gh release create v1",
    "A=$(true) git push --tags",
    "A=`true` git push --tags",
    "A=$(true) git tag v1",
    "A=$(echo $(true)) git push origin refs/tags/v1",
    "A=$(true) gh api -X POST repos/o/r/releases -f tag_name=v1",
];

#[test]
fn a_merge_behind_a_variable_a_substitution_sets_is_held() {
    purlis_core::unsteered!();
    for cmd in MERGES {
        assert!(mergehold::held(cmd).is_some(), "not held: {cmd}");
        assert!(
            release_floor_reason(cmd, true).is_some(),
            "not floored: {cmd}"
        );
    }
    // A request the hold can name stays one it names, past a variable written out.
    assert!(matches!(
        mergehold::held("A=1 gh pr merge 12"),
        Some(Held::Request(_))
    ));
}

#[test]
fn a_publish_behind_a_variable_a_substitution_sets_is_refused_unattended() {
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
    for cmd in MERGES.iter().chain(PUBLISHES) {
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
        assert_eq!(
            verdict(&call, Some(&plane)).and_then(|said| said.arm),
            Some(Arm::ReleaseFloor),
            "{cmd}"
        );
    }
}

/// What a substitution's output is put into still reads as before: a word it fills into the
/// command is a word the shell fills in, and nothing that merges nothing is refused.
#[test]
fn a_variable_a_substitution_sets_is_still_no_merge_by_itself() {
    purlis_core::unsteered!();
    for cmd in [
        "A=$(git rev-parse HEAD)",
        "A=$(git rev-parse HEAD) gh pr view 12",
        "A=`date` git push origin feat",
        "A=$(true); gh pr view 12",
    ] {
        assert_eq!(mergehold::held(cmd), None, "{cmd}");
        assert_eq!(release_floor_reason(cmd, true), None, "{cmd}");
    }
    // What a substitution could put on the line is still read where it is put.
    for cmd in [
        "git push $(echo --tags)",
        "gh api repos/o/r/pulls/12/merge $(echo -X PUT)",
        "gh api -X PUT repos/o/r/pulls/$(echo 12)/merge",
    ] {
        assert!(release_floor_reason(cmd, true).is_some(), "{cmd}");
    }
}
