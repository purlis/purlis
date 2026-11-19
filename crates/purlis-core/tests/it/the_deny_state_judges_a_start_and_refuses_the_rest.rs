// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The deny state, through the decision function** (M75-07b, #7016; spec §4.6, §6.1).
//!
//! In the deny state the chat still starts and every action but `start` is refused, naming the deny
//! state. A `start` is still judged by the rules the project committed, so a forbid on it refuses
//! the chat: the deny state only adds refusals. The standing is built by hand here; the store that
//! gives it is slice 07a's, and is tested with its seal seam.
//!
//! Also `context.machine` (spec §3.2; Q320d point 1): the device id the person confirmed, unset
//! otherwise, and a rule that reads it unset errs, which is a deny (spec §6.3).

use std::path::Path;

use purlis_core::policy::decide::{self, Asker, Outcome, Sealed};
use purlis_core::policy::deny_state::RULE_ID;
use purlis_core::policy::levels::Chat;
use purlis_core::secrets::answers::{Deny, Standing, State};

fn git(dir: &Path, args: &[&str]) {
    let ran = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=nobody"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

/// A plane whose root commits `policy.cedar` with `rules`.
fn plane(rules: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("README.md"), "a plane\n").expect("a file");
    std::fs::write(dir.path().join("policy.cedar"), rules).expect("the rules");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "c"]);
    dir
}

fn asker<'a>(start: Option<decide::StartFacts<'a>>) -> Asker<'a> {
    Asker {
        chat: 1,
        of: Chat {
            workspace: None,
            persona: None,
        },
        sandbox: "full",
        attended: true,
        harness_mode: "default",
        depth: 0,
        via: if start.is_some() { "start" } else { "hook" },
        start,
    }
}

fn in_the_deny_state() -> Sealed {
    Sealed {
        standing: Standing {
            state: State::Deny(vec![Deny::CloneKeySwapped]),
            ..Standing::fresh()
        },
        machine: None,
    }
}

const FORBID_START: &str = r#"// purlis-policy 1
@id("trust-no-start")
forbid (principal, action == Action::"start", resource);
"#;

fn start(plane: &Path, sealed: &Sealed) -> decide::Decision {
    let gaps: Vec<String> = Vec::new();
    decide::in_plane(
        plane,
        &asker(Some(decide::StartFacts {
            profile: "default",
            gaps: &gaps,
        })),
        "start",
        ("Harness", "claude-code"),
        sealed,
    )
}

#[test]
fn in_the_deny_state_a_forbid_on_start_still_refuses_the_chat() {
    purlis_core::unsteered!();
    let forbidding = plane(FORBID_START);
    let said = start(forbidding.path(), &in_the_deny_state());
    assert_eq!(said.after_mode, Outcome::Deny, "{said:?}");
    assert_eq!(said.ids(), ["trust-no-start"], "{said:?}");
    assert!(
        said.deny_state.is_none(),
        "the rule refused it, not the deny state: {said:?}"
    );
    // It is the very answer the rules give outside the deny state: the deny state adds nothing here.
    assert_eq!(said, start(forbidding.path(), &Sealed::unsealed()));
}

#[test]
fn in_the_deny_state_a_start_no_rule_forbids_goes_ahead_and_the_rest_is_refused() {
    purlis_core::unsteered!();
    let open = plane("");
    let started = start(open.path(), &in_the_deny_state());
    assert_ne!(
        started.after_mode,
        Outcome::Deny,
        "the chat starts: {started:?}"
    );
    let sealed = in_the_deny_state();
    for (action, resource) in [
        ("exec", ("Command", "ls")),
        ("merge", ("Branch", "acme/web#main")),
        ("connect", ("Host", "example.org")),
    ] {
        let said = decide::in_plane(open.path(), &asker(None), action, resource, &sealed);
        assert_eq!(said.outcome, Outcome::Deny, "{action}: {said:?}");
        assert_eq!(said.ids(), [RULE_ID], "{action}: {said:?}");
        assert!(said.deny_state.is_some(), "{action}: {said:?}");
    }
}

/// A rule of the project's that names the device id (the schema makes `machine` optional, so the rule
/// must say `context has machine`) forbids where the id is unset, because what it expects is never
/// there; with the confirmed id it reads it and decides. A read the schema would allow to err is a
/// deny too (spec §6.3).
#[test]
fn a_rule_reading_an_unset_context_machine_denies() {
    purlis_core::unsteered!();
    let reads = plane(
        r#"// purlis-policy 1
@id("processes-this-machine-only") @level("project")
forbid (principal, action == Action::"exec", resource)
  unless { context has machine && context.machine == "01JABCDEFGHJKMNPQRSTVWXYZ0" };
"#,
    );
    let ask = |sealed: &Sealed| {
        decide::in_plane(
            reads.path(),
            &asker(None),
            "exec",
            ("Command", "ls"),
            sealed,
        )
    };
    let unset = ask(&Sealed::unsealed());
    assert_eq!(unset.outcome, Outcome::Deny, "{unset:?}");
    assert!(
        unset.ids().contains(&"processes-this-machine-only"),
        "{unset:?}"
    );

    let confirmed = ask(&Sealed {
        machine: Some("01JABCDEFGHJKMNPQRSTVWXYZ0".to_owned()),
        ..Sealed::unsealed()
    });
    assert_eq!(confirmed.outcome, Outcome::Allow, "{confirmed:?}");

    // Another machine's id decides the other way.
    let other = ask(&Sealed {
        machine: Some("01JZZZZZZZZZZZZZZZZZZZZZZZ".to_owned()),
        ..Sealed::unsealed()
    });
    assert_eq!(other.outcome, Outcome::Deny, "{other:?}");
}
