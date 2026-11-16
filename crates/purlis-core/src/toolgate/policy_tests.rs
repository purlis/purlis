// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Each guard family of the Bash guard runs as `[policy]` says** (Q308, Q308a): off, the arm is
//! not asked; log, it is asked and its refusal kept for the log while the call goes ahead;
//! enforce, it refuses as it always has.

use super::*;
use crate::forge::{Forge, Kind};

/// One refused line per family, in a plane whose root the line runs in: the family, the line,
/// the arm that refuses it, and the permission mode it is run in.
fn refused_lines() -> [(Family, &'static str, Arm, Option<&'static str>); 6] {
    [
        (
            Family::Secrets,
            "cat .charter/vaults/db.json",
            Arm::Leak,
            None,
        ),
        (
            Family::GuardsCredential,
            "git clone git@github.com:o/r.git",
            Arm::SingleCredential,
            None,
        ),
        (
            Family::GuardsGit,
            "git checkout feature",
            Arm::PlaneRootBranch,
            None,
        ),
        (
            Family::GuardsRelease,
            "gh release create v1.0.0",
            Arm::ReleaseFloor,
            Some(crate::attendance::PROMPTS_OFF),
        ),
        (
            Family::GuardsSave,
            "git commit --no-verify -m x",
            Arm::HookSkip,
            None,
        ),
        (
            Family::GuardsRelease,
            "gh issue create --body \"$(cat notes.md)\"",
            Arm::ForgeSubstitution,
            None,
        ),
    ]
}

fn judged_as(cmd: &str, mode_of_its_family: Mode, family: Family, prompts: Option<&str>) -> Judged {
    let dir = tempfile::tempdir().expect("a directory");
    // Only the leak's line needs a vault: a sandbox that runs these tests may deny making one.
    if cmd.contains(".charter/vaults") {
        let vaults = dir.path().join(".charter").join("vaults");
        std::fs::create_dir_all(&vaults).expect("the vault directory");
        std::fs::write(vaults.join("db.json"), "{}\n").expect("a vault");
    }
    let root = dir.path().display().to_string();
    let state = dir.path().join(".charter");
    let forges = vec![
        Forge::default_of(Kind::GitHub),
        Forge::default_of(Kind::GitLab),
    ];
    let plane = Plane {
        root: &root,
        forges: &forges,
        session_dir: &root,
        launched: Launched {
            sandboxed: false,
            chat_dir: Some(&root),
            attendance: None,
        },
    };
    let call = Call {
        command: cmd,
        cwd: &root,
        state_dir: &state,
        caller: Caller {
            agent_id: None,
            harness: Some("claude-code"),
            permission_mode: Some(prompts.unwrap_or("default")),
        },
    };
    judged(&call, Some(&plane), &|asked| {
        if asked == family {
            mode_of_its_family
        } else {
            Mode::Enforce
        }
    })
}

#[test]
fn an_enforced_guard_family_refuses_as_it_always_has() {
    for (family, cmd, arm, prompts) in refused_lines() {
        let judged = judged_as(cmd, Mode::Enforce, family, prompts);
        assert_eq!(
            judged.refused.as_ref().and_then(|v| v.arm),
            Some(arm),
            "{}: {cmd}",
            family.key()
        );
        assert!(judged.logged.is_empty(), "{cmd}");
    }
}

#[test]
fn a_logged_guard_family_lets_the_call_through_and_keeps_what_it_would_have_refused() {
    for (family, cmd, arm, prompts) in refused_lines() {
        let judged = judged_as(cmd, Mode::Log, family, prompts);
        assert_eq!(judged.refused, None, "{}: {cmd}", family.key());
        assert_eq!(
            judged.logged.iter().map(|v| v.arm).collect::<Vec<_>>(),
            [Some(arm)],
            "{cmd}"
        );
    }
}

#[test]
fn a_guard_family_that_is_off_is_not_asked() {
    for (family, cmd, _, prompts) in refused_lines() {
        let judged = judged_as(cmd, Mode::Off, family, prompts);
        assert_eq!(judged.refused, None, "{}: {cmd}", family.key());
        assert!(judged.logged.is_empty(), "{cmd}");
    }
}

#[test]
fn an_arm_that_only_logs_leaves_a_later_enforced_arm_to_refuse() {
    // One credential (guards.credential) is asked first; the hook skip (guards.save) after it.
    let cmd = "git clone git@github.com:o/r.git; git commit --no-verify -m x";
    let judged = judged_as(cmd, Mode::Log, Family::GuardsCredential, None);
    assert_eq!(judged.refused.and_then(|v| v.arm), Some(Arm::HookSkip));
    assert_eq!(
        judged.logged.iter().map(|v| v.arm).collect::<Vec<_>>(),
        [Some(Arm::SingleCredential)]
    );
}

#[test]
fn every_arm_has_a_family_and_the_shape_arms_hold_under_open_until_wired() {
    for arm in Arm::ALL {
        assert!(arm.family().is_some(), "{}", arm.id());
    }
    // `guards.shape` is a key now and not yet switchable: `open` leaves the caps refusing.
    let open = crate::enforcement::Resolved::of(
        &crate::enforcement::Settings::from_text(Some("[policy]\ntemplate = \"open\"\n")),
        &crate::sandbox::policy::Locks::none(),
    );
    let huge = "x".repeat(crate::guardcaps::MAX_COMMAND_BYTES + 1);
    let dir = tempfile::tempdir().expect("a directory");
    let state = dir.path().join(".charter");
    let call = Call {
        command: &huge,
        cwd: "",
        state_dir: &state,
        caller: Caller {
            agent_id: None,
            harness: Some("claude-code"),
            permission_mode: Some("default"),
        },
    };
    let judged = judged(&call, None, &|family| open.mode(family));
    assert_eq!(judged.refused.and_then(|v| v.arm), Some(Arm::Caps));
}

#[test]
fn the_arms_that_exist_because_of_the_wrap_hold_under_every_template() {
    // The review of #6992, S5: git the sandbox would stop partway, or that a sandboxed chat
    // cannot finish, is refused whenever that wrap is applied, so the root is never left
    // half-updated, whatever the project's `[policy]` says.
    for template in crate::enforcement::Template::ALL {
        let resolved = crate::enforcement::Resolved::of(
            &crate::enforcement::Settings::from_text(Some(&format!(
                "[policy]\ntemplate = \"{}\"\nguards.git = \"off\"\n",
                template.word()
            ))),
            &crate::sandbox::policy::Locks::none(),
        );
        for arm in [
            Arm::RootUpdate,
            Arm::CloneUpdate,
            Arm::ProjectGit,
            Arm::BranchGit,
        ] {
            let family = arm.family().expect("a family");
            assert_eq!(
                resolved.mode(family),
                Mode::Enforce,
                "{} {}",
                template.word(),
                arm.id()
            );
        }
    }
}
