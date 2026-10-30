// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[test]
fn the_inbox_gets_charters_words_for_each_alert() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[persona]\ndefault = \"ghost\"\n",
    )
    .unwrap();
    let (alerts, stopped) = rows(&root);
    assert_eq!(stopped, None);
    assert_eq!(alerts.len(), 1, "{alerts:?}");
    assert_eq!(alerts[0].severity, "warn");
    assert_eq!(alerts[0].subject, "front door");
    assert_eq!(alerts[0].detail, "ghost — no such persona");
    assert_eq!(
        alerts[0].way,
        AlertWay::Settings {
            group: "project.general".to_owned(),
            setting: Some("project.general.persona.default".to_owned()),
        }
    );
}

fn merge(state: &str) -> crate::saving::SaveMerge {
    crate::saving::SaveMerge {
        state: state.to_owned(),
        why: "1 check failed: lint".to_owned(),
        checks: vec![crate::saving::FailedCheck {
            name: "lint".to_owned(),
            url: None,
        }],
        needs: None,
        number: Some("#12".to_owned()),
        shown: "red - lint".to_owned(),
    }
}

#[test]
fn a_red_request_is_an_inbox_alert_with_its_two_answers_until_it_merges_or_goes_green() {
    const PR: &str = "https://github.com/acme/api/pull/12";
    let red = merge("red");
    let row = request_red(
        (None, None),
        "pull request",
        Some(PR),
        Some("main"),
        true,
        Some(&red),
    )
    .expect("a red request is said");
    assert_eq!(
        (row.severity.as_str(), row.subject.as_str()),
        ("bad", "save-request-red")
    );
    assert_eq!(
        row.detail,
        "the pull request #12 of this project is red, so purlis did not merge it: 1 check \
         failed: lint"
    );
    assert_eq!(
        row.way,
        AlertWay::SaveRequest {
            workspace: None,
            repo: None,
            name: "this project".to_owned(),
            request: "pull request".to_owned(),
            pr: PR.to_owned(),
            target: Some("main".to_owned()),
            autosave: true,
            merge: Box::new(red.clone()),
        }
    );
    // A repo's names the repo, and where its answers go.
    let repos = request_red(
        (Some("alpha"), Some("api")),
        "merge request",
        Some(PR),
        None,
        false,
        Some(&red),
    )
    .expect("said");
    assert!(
        repos
            .detail
            .starts_with("the merge request #12 of api is red"),
        "{}",
        repos.detail
    );
    assert!(matches!(
        &repos.way,
        AlertWay::SaveRequest { workspace: Some(ws), repo: Some(repo), .. }
            if ws == "alpha" && repo == "api"
    ));
    // It goes when the request merges or its checks pass, and waiting is no trouble.
    for state in ["waiting", "queued", "cannot-merge", "merged"] {
        let not_red = merge(state);
        assert!(
            request_red(
                (None, None),
                "pull request",
                Some(PR),
                None,
                true,
                Some(&not_red)
            )
            .is_none(),
            "{state}"
        );
    }
    assert!(request_red((None, None), "pull request", Some(PR), None, true, None).is_none());
    assert!(request_red((None, None), "pull request", None, None, true, Some(&red)).is_none());
}

#[test]
fn a_project_whose_saves_merge_nothing_is_asked_about_no_request() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("charter.toml"),
        "[plane]\nmode = \"commit\"\n",
    )
    .unwrap();
    assert!(requests_red(dir.path()).is_empty());
}

/// **Each row carries its way out** (NO-6, #1238): where the window mends it, by the core's
/// own kind of alert rather than by its words.
#[test]
fn each_kind_of_alert_has_its_way_out() {
    // On the version lock's own control (#1289).
    let general = AlertWay::Settings {
        group: "project.general".to_owned(),
        setting: Some("project.general.charter.version".to_owned()),
    };
    let way = |alert: alerts::Alert| way_out(&alert);
    assert_eq!(
        way(alerts::Alert::PinBesideDev {
            pinned: "1.0.0".into()
        }),
        general
    );
    assert_eq!(
        way(alerts::Alert::PinDrift {
            running: "2.0.0".into(),
            pinned: "1.0.0".into()
        }),
        general
    );
    // The same fix id `purlis doctor --fix` takes, so the Inbox and the CLI do one thing.
    assert_eq!(
        way(alerts::Alert::Reinit {
            stale: vec!["ide".into()]
        }),
        AlertWay::Fix {
            id: "workspace-reinit".to_owned()
        }
    );
    assert_eq!(
        way(alerts::Alert::NestedPlane {
            inner: "/home/dev/outer/workspaces/inner".into(),
            outer: "/home/dev/outer".into(),
        }),
        AlertWay::OpenProject {
            path: "/home/dev/outer".to_owned()
        }
    );
    assert_eq!(
        way(alerts::Alert::PlaneRoot {
            name: "plane".into(),
            dirty: false,
            detached: true,
            off: None,
            memory: None,
        }),
        AlertWay::Saving
    );
}

#[test]
fn a_reading_that_stopped_says_why_and_keeps_what_it_found() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n[[[ not toml\n").unwrap();
    let (alerts, stopped) = rows(&root);
    assert!(stopped.is_some(), "{alerts:?}");
    assert!(alerts.is_empty());
}

#[test]
fn the_inbox_leaves_to_the_save_indicator_what_it_already_says_about_the_plane_root() {
    let dirty = alerts::Alert::PlaneRoot {
        name: "plane".into(),
        dirty: true,
        detached: false,
        off: None,
        memory: Some(alerts::Memory::NotPushed),
    };
    assert_eq!(beside_the_indicator(&dirty), None);

    let off = alerts::Alert::PlaneRoot {
        name: "plane".into(),
        dirty: true,
        detached: false,
        off: Some(("work".into(), "main".into())),
        memory: Some(alerts::Memory::NotPushed),
    };
    assert_eq!(
        beside_the_indicator(&off),
        Some(alerts::Alert::PlaneRoot {
            name: "plane".into(),
            dirty: false,
            detached: false,
            off: Some(("work".into(), "main".into())),
            memory: None,
        })
    );
}

fn blocked_plane(detail: &str, at: f64) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("charter.toml"), "[plane]\nmode = \"push\"\n").unwrap();
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "one"],
    ] {
        let mut command = std::process::Command::new("git");
        command
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid");
        purlis_core::forklock::output(&mut command).expect("git runs");
    }
    let journal = purlis_core::planegit::journal_path(root);
    std::fs::create_dir_all(journal.parent().unwrap()).unwrap();
    std::fs::write(
        journal,
        format!(
            "{}\n",
            serde_json::json!({"at": at, "outcome": "blocked", "detail": detail})
        ),
    )
    .unwrap();
    dir
}

#[test]
fn a_save_blocked_by_a_secret_is_said_at_once() {
    let dir = blocked_plane(purlis_core::planegit::SECRET_REFUSED, now());
    let memory = dir.path().join("personas/steward/memory");
    std::fs::create_dir_all(&memory).unwrap();
    std::fs::write(
        memory.join("m.md"),
        // Split, so the commit scan reads no whole token in this file; the value is the same.
        concat!("token: ghp", "_0123456789abcdefghijklmnopqrstuvwxyz\n"),
    )
    .unwrap();
    let row = save_blocked(dir.path(), now()).expect("said at once");
    assert_eq!(
        (row.severity.as_str(), row.subject.as_str()),
        ("bad", "save")
    );
    assert_eq!(row.way, AlertWay::Saving);
    assert!(row.detail.contains("secret-shaped"), "{row:?}");
    // And it names the file and the line, so the person knows what to mend.
    assert!(
        row.detail.ends_with("in personas/…/m.md, line 1"),
        "{row:?}"
    );
}

#[test]
fn any_other_block_is_said_after_ten_minutes_and_not_before() {
    let at = now();
    // The plane on another branch than it saves into, recorded by the save that stopped.
    let dir = blocked_plane("this plane is on main, and [plane] branch is trunk", at);
    std::fs::write(
        dir.path().join("charter.toml"),
        "[plane]\nmode = \"push\"\nbranch = \"trunk\"\n",
    )
    .unwrap();
    assert!(save_blocked(dir.path(), at + 60.0).is_none());
    assert!(save_blocked(dir.path(), at + 601.0).is_some());
}
