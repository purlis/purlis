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
            Family::GuardsPlane,
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
        // A hook skip runs at the stricter of save and credential (the re-review of #6992), so
        // its row moves both.
        if asked == family || (family == Family::GuardsSave && asked == Family::GuardsCredential) {
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
fn every_arm_but_the_persons_own_rules_has_a_family() {
    for arm in Arm::ALL {
        assert_eq!(
            arm.family().is_none(),
            *arm == Arm::RuleSpelling,
            "{}",
            arm.id()
        );
    }
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

#[test]
fn a_hook_skip_is_refused_while_the_credential_scan_is_enforced_whatever_save_says() {
    // The re-review of #6992, blocker: `--no-verify` skips the token scan `guards.credential`
    // keeps at enforce (Q308a), so the hook skip runs at the stricter of save and credential.
    let lines = [
        "git commit --no-verify -m x",
        "git push --no-verify origin main",
        "git merge --no-verify feature",
        "git send-pack origin main",
    ];
    for (policy, expected) in [
        ("template = \"watch\"\n", Some(Arm::HookSkip)),
        ("template = \"hold\"\n", Some(Arm::HookSkip)),
        ("template = \"team\"\n", Some(Arm::HookSkip)),
        (
            "template = \"team\"\nguards.save = \"off\"\n",
            Some(Arm::HookSkip),
        ),
        ("template = \"open\"\n", None),
    ] {
        let resolved = crate::enforcement::Resolved::of(
            &crate::enforcement::Settings::from_text(Some(&format!("[policy]\n{policy}"))),
            &crate::sandbox::policy::Locks::none(),
        );
        for cmd in lines {
            let dir = tempfile::tempdir().expect("a directory");
            let root = dir.path().display().to_string();
            let state = dir.path().join(".charter");
            let forges = vec![Forge::default_of(Kind::GitHub)];
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
                    permission_mode: Some("default"),
                },
            };
            let judged = judged(&call, Some(&plane), &|family| resolved.mode(family));
            assert_eq!(
                judged.refused.and_then(|v| v.arm),
                expected,
                "{policy:?} {cmd}"
            );
        }
    }
}

/// `[policy]` as `text` resolves it, with no lock.
fn resolved_of(text: &str) -> crate::enforcement::Resolved {
    crate::enforcement::Resolved::of(
        &crate::enforcement::Settings::from_text(Some(&format!("[policy]\n{text}"))),
        &crate::sandbox::policy::Locks::none(),
    )
}

/// What the gate makes of `cmd`, from a caller whose sub-agent id is `agent_id`, in a plane at
/// a fresh folder that `plant` has filled, under `[policy]` `text`.
fn judged_under(
    text: &str,
    cmd: &str,
    agent_id: Option<&str>,
    plant: &dyn Fn(&std::path::Path),
) -> Judged {
    let resolved = resolved_of(text);
    let dir = tempfile::tempdir().expect("a directory");
    plant(dir.path());
    let root = dir.path().display().to_string();
    let state = dir.path().join(".charter");
    let forges = vec![Forge::default_of(Kind::GitHub)];
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
            agent_id,
            harness: Some("claude-code"),
            permission_mode: Some("default"),
        },
    };
    judged(&call, Some(&plane), &|family| resolved.mode(family))
}

fn nothing_planted(_: &std::path::Path) {}

/// One line each shape arm refuses (Q308b: `guards.shape`), and the sub-agent it comes from.
fn shape_lines() -> Vec<(String, Arm, Option<&'static str>)> {
    let task = "purlis dispatch --name x <<'B'\nlook\nB";
    vec![
        (
            "x".repeat(crate::guardcaps::MAX_COMMAND_BYTES + 1),
            Arm::Caps,
            None,
        ),
        (task.to_owned(), Arm::SubAgentDispatch, Some("sub-1")),
        (
            "purlis dispatch report --outcome done \"$(cat notes.md)\"".to_owned(),
            Arm::DispatchText,
            None,
        ),
        (format!("{task}\ntouch /tmp/zz"), Arm::Rider, None),
    ]
}

#[test]
fn the_shape_arms_run_as_guards_shape_says_where_nothing_else_reads_the_command() {
    // Every other family off, so the caps answer to `guards.shape` alone.
    for (cmd, arm, agent) in shape_lines() {
        for (mode, refused, logged) in [
            ("enforce", Some(arm), vec![]),
            ("log", None, vec![Some(arm)]),
            ("off", None, vec![]),
        ] {
            let judged = judged_under(
                &format!("template = \"open\"\nguards.shape = \"{mode}\"\n"),
                &cmd,
                agent,
                &nothing_planted,
            );
            assert_eq!(
                judged.refused.and_then(|v| v.arm),
                refused,
                "{mode} {}",
                arm.id()
            );
            // Under `log` every shape arm the line trips is kept, the one it is about first: a
            // dispatch's text beside a substitution is also a rider (A10).
            let kept = judged.logged.iter().map(|v| v.arm).collect::<Vec<_>>();
            assert_eq!(kept.first(), logged.first(), "{mode} {}", arm.id());
            assert_eq!(kept.is_empty(), logged.is_empty(), "{mode} {}", arm.id());
        }
    }
    // And `strict` with only the shape off lifts every shape arm but the caps (below).
    for (cmd, arm, agent) in shape_lines().into_iter().skip(1) {
        let judged = judged_under(
            "template = \"strict\"\nguards.shape = \"off\"\n",
            &cmd,
            agent,
            &nothing_planted,
        );
        assert_eq!(judged.refused, None, "{}", arm.id());
    }
}

#[test]
fn a_command_too_big_to_read_is_refused_while_any_family_that_reads_it_is_enforced() {
    // A command past the caps is one no arm can read, so it cannot be shown clear of an enforced
    // one: the caps run at the strictest family the gate asks, whatever `guards.shape` says.
    let huge = "x".repeat(crate::guardcaps::MAX_COMMAND_BYTES + 1);
    for text in [
        "template = \"watch\"\nguards.shape = \"off\"\n",
        "template = \"strict\"\nguards.shape = \"off\"\n",
        "template = \"open\"\nsecrets = \"enforce\"\n",
    ] {
        let judged = judged_under(text, &huge, None, &nothing_planted);
        assert_eq!(
            judged.refused.and_then(|v| v.arm),
            Some(Arm::Caps),
            "{text}"
        );
    }
    // Where the strictest of them only logs, the caps log.
    let judged = judged_under(
        "template = \"open\"\nguards.save = \"log\"\n",
        &huge,
        None,
        &nothing_planted,
    );
    assert_eq!(judged.refused, None);
    assert_eq!(
        judged.logged.iter().map(|v| v.arm).collect::<Vec<_>>(),
        [Some(Arm::Caps)]
    );
}

#[test]
fn the_plane_roots_branch_and_reset_arms_run_as_guards_plane_says() {
    // Q308b: the plane root's git is `guards.plane`'s, not `guards.git`'s.
    assert_eq!(Arm::PlaneRootBranch.family(), Some(Family::GuardsPlane));
    assert_eq!(Arm::PlaneRootReset.family(), Some(Family::GuardsPlane));
    let cmd = "git checkout feature";
    for (text, refused, logged) in [
        (
            "template = \"strict\"\n",
            Some(Arm::PlaneRootBranch),
            vec![],
        ),
        (
            "template = \"strict\"\nguards.git = \"off\"\n",
            Some(Arm::PlaneRootBranch),
            vec![],
        ),
        (
            "template = \"strict\"\nguards.plane = \"log\"\n",
            None,
            vec![Some(Arm::PlaneRootBranch)],
        ),
        (
            "template = \"strict\"\nguards.plane = \"off\"\n",
            None,
            vec![],
        ),
        ("template = \"team\"\n", Some(Arm::PlaneRootBranch), vec![]),
    ] {
        let judged = judged_under(text, cmd, None, &nothing_planted);
        assert_eq!(judged.refused.and_then(|v| v.arm), refused, "{text}");
        assert_eq!(
            judged.logged.iter().map(|v| v.arm).collect::<Vec<_>>(),
            logged,
            "{text}"
        );
    }
}

#[test]
fn the_persons_own_ask_and_deny_rules_hold_under_every_spelling_in_every_mode() {
    // Q308b: the person's own denies are theirs, not a shipped block, so A7c holds under `open`.
    assert_eq!(Arm::RuleSpelling.family(), None);
    let plant = |dir: &std::path::Path| {
        std::fs::create_dir_all(dir.join(".claude")).expect(".claude");
        std::fs::write(
            dir.join(".claude/settings.json"),
            r#"{"permissions": {"deny": ["Bash(terraform apply *)"]}}"#,
        )
        .expect("settings");
    };
    for template in crate::enforcement::Template::ALL {
        let judged = judged_under(
            &format!(
                "template = \"{}\"\nguards.shape = \"off\"\n",
                template.word()
            ),
            "/usr/local/bin/terraform apply",
            None,
            &plant,
        );
        assert_eq!(
            judged.refused.and_then(|v| v.arm),
            Some(Arm::RuleSpelling),
            "{}",
            template.word()
        );
    }
}

#[test]
fn the_persons_own_rules_hold_under_open_for_a_command_too_big_or_too_deep_to_read() {
    // #7104: under `open` the caps let through a command no arm can read whole, and A7c, which
    // holds in every mode, would never be asked. Where the person's rules name a program, such
    // a command cannot be shown clear of them, so it is refused.
    let plant = |dir: &std::path::Path| {
        std::fs::create_dir_all(dir.join(".claude")).expect(".claude");
        std::fs::write(
            dir.join(".claude/settings.json"),
            r#"{"permissions": {"deny": ["Bash(terraform apply *)"]}}"#,
        )
        .expect("settings");
    };
    let rule = "/usr/local/bin/terraform apply";
    // Past the size the caps read.
    let huge = format!(
        "{rule}; echo {}",
        "x".repeat(crate::guardcaps::MAX_COMMAND_BYTES)
    );
    // Within the caps' size and nesting, but more text, read inward, than the readers may read:
    // seven substitutions deep round a long word.
    let deep = format!(
        "{rule}; {}{}{}",
        "echo $(".repeat(7),
        "y".repeat(20 * 1024),
        ")".repeat(7)
    );
    assert_eq!(crate::guardcaps::refusal(&deep), None, "the caps read it");
    let refused = |plant: &dyn Fn(&std::path::Path)| {
        [("too big", &huge), ("too deep", &deep)].map(|(shape, cmd)| {
            let judged = judged_under("template = \"open\"\n", cmd, None, plant);
            (shape, judged.refused.is_some())
        })
    };
    assert_eq!(refused(&plant), [("too big", true), ("too deep", true)]);
    // Where the person has no rule, `open` lets either through as before.
    assert_eq!(
        refused(&nothing_planted),
        [("too big", false), ("too deep", false)]
    );
}
