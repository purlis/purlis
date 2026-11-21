// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`[policy.limits]`** (M75 slice 12, #7021; spec §5.1): typed against the limits key table, the
//! value in force the stricter of the committed file and the accepted copy, and a limit under
//! `limits = log` counted, not applied.

use std::path::Path;

use purlis_core::policy::levels::{self, Accepted, Chat, Loaded, Organisation, Sources};
use purlis_core::policy::limits::Verdict;

const ANYONE: Chat<'static> = Chat {
    workspace: None,
    persona: None,
};

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

/// A plane whose root commits `charter.toml` with `settings`.
fn plane(settings: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    git(dir.path(), &["init", "-q"]);
    std::fs::write(dir.path().join("charter.toml"), settings).expect("the project file");
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "c"]);
    dir
}

fn sources<'a>(root: &'a Path, accepted: Option<&'a str>) -> Sources<'a> {
    Sources {
        root,
        workspaces: &[],
        personas: &[],
        organisation: None,
        accepted: accepted.map(|settings| Accepted {
            settings: Some(settings),
            rules: None,
        }),
    }
}

fn load(committed: &str, accepted: Option<&str>) -> Loaded {
    let root = plane(committed);
    levels::load(&sources(root.path(), accepted))
}

#[test]
fn an_unknown_limits_key_makes_the_file_broken() {
    purlis_core::unsteered!();
    let loaded = load(
        "[policy.limits]\n\"dispatch.running-per-chatt\" = 3\n",
        None,
    );
    let said = loaded
        .notices
        .iter()
        .find(|n| n.what.contains("dispatch.running-per-chatt"))
        .unwrap_or_else(|| panic!("a notice names the key: {:#?}", loaded.notices));
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));
    assert!(said.what.contains("broken"), "{}", said.what);
    assert_eq!(
        loaded.sandbox_at_least,
        Some(purlis_core::enforcement::SandboxLevel::Full),
        "a broken file runs the full sandbox"
    );
}

#[test]
fn a_limit_that_is_no_whole_number_makes_the_file_broken() {
    purlis_core::unsteered!();
    for text in [
        "[policy.limits]\n\"trust.unread-max\" = \"many\"\n",
        "[policy.limits]\n\"trust.unread-max\" = -1\n",
    ] {
        let loaded = load(text, None);
        assert!(
            loaded
                .notices
                .iter()
                .any(|n| n.what.contains("trust.unread-max")),
            "{text}: {:#?}",
            loaded.notices
        );
    }
}

#[test]
fn a_dotted_key_reads_as_the_same_limit() {
    purlis_core::unsteered!();
    let loaded = load("[policy.limits.dispatch]\nrunning-per-chat = 3\n", None);
    assert_eq!(loaded.notices, []);
    assert_eq!(loaded.limit("dispatch.running-per-chat"), Some(3));
}

#[test]
fn a_raised_cap_waits_for_the_press() {
    purlis_core::unsteered!();
    let loaded = load(
        "[policy.limits]\n\"dispatch.running-per-chat\" = 12\n",
        Some("[policy]\ntemplate = \"watch\"\n"),
    );
    assert_eq!(loaded.notices, []);
    assert_eq!(loaded.limit("dispatch.running-per-chat"), Some(6));
    // Accepted at 10, committed at 12: the accepted number still holds.
    let loaded = load(
        "[policy.limits]\n\"dispatch.running-per-chat\" = 12\n",
        Some("[policy.limits]\n\"dispatch.running-per-chat\" = 10\n"),
    );
    assert_eq!(loaded.limit("dispatch.running-per-chat"), Some(10));
}

#[test]
fn a_lowered_cap_applies_at_once() {
    purlis_core::unsteered!();
    let loaded = load("[policy.limits]\n\"dispatch.running-per-chat\" = 2\n", None);
    assert_eq!(loaded.limit("dispatch.running-per-chat"), Some(2));
}

#[test]
fn a_lowered_retention_waits_for_the_press() {
    purlis_core::unsteered!();
    let loaded = load(
        "[policy.limits]\n\"audit.record-days\" = 30\n",
        Some("[policy]\ntemplate = \"watch\"\n"),
    );
    assert_eq!(loaded.limit("audit.record-days"), Some(365));
    let loaded = load("[policy.limits]\n\"audit.record-days\" = 730\n", None);
    assert_eq!(
        loaded.limit("audit.record-days"),
        Some(730),
        "keeping longer applies at once"
    );
}

#[test]
fn a_broken_committed_file_leaves_the_accepted_limit_in_force() {
    purlis_core::unsteered!();
    let loaded = load(
        "[policy.limits]\n\"no.such-key\" = 1\n",
        Some("[policy.limits]\n\"dispatch.running-per-chat\" = 4\n"),
    );
    assert_eq!(loaded.limit("dispatch.running-per-chat"), Some(4));
}

#[test]
fn the_organisations_limit_binds_every_level_below() {
    purlis_core::unsteered!();
    let root = plane("[policy.limits]\n\"dispatch.running-per-chat\" = 5\n");
    let org = Organisation {
        folder: "/etc/purlis",
        owner: "Platform team",
        rules: None,
        settings: Some("[policy.limits]\n\"dispatch.running-per-chat\" = 3\n"),
        json: None,
        failed: None,
    };
    let loaded = levels::load(&Sources {
        organisation: Some(org),
        ..sources(root.path(), None)
    });
    assert_eq!(loaded.notices, []);
    assert_eq!(loaded.limit("dispatch.running-per-chat"), Some(3));
}

#[test]
fn under_limits_log_an_exceeded_cap_is_counted_not_applied() {
    purlis_core::unsteered!();
    let enforced = load("[policy]\ntemplate = \"strict\"\n", None);
    assert_eq!(
        enforced.limit_verdict(ANYONE, "dispatch.running-per-chat", 7),
        Verdict::Refuse
    );
    let logged = load("[policy]\nlimits = \"log\"\n", None);
    assert_eq!(
        logged.limit_verdict(ANYONE, "dispatch.running-per-chat", 7),
        Verdict::Counted
    );
    assert_eq!(
        logged.limit_verdict(ANYONE, "dispatch.running-per-chat", 6),
        Verdict::Within
    );
}
