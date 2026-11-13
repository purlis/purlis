// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// The key's parts are checked as they are made and as they are read off the wire: a command,
/// a URL or a short commit is never a merge's key.
#[test]
fn a_merges_key_is_checked_as_it_is_made_and_read() {
    for repo in ["o/r", "group/sub/project", "my.org/re_po-1"] {
        assert!(Repo::try_from(repo).is_ok(), "{repo}");
    }
    for repo in [
        "r",
        "o/r; rm -rf ~",
        "o/../r",
        "https://github.com/o/r",
        "o//r",
        "o/r/",
    ] {
        assert!(Repo::try_from(repo).is_err(), "{repo}");
    }
    assert!(HeadSha::try_from("0123456789abcdef0123456789abcdef01234567").is_ok());
    for head in [
        "0123456",
        "0123456789ABCDEF0123456789ABCDEF01234567",
        "HEAD",
    ] {
        assert!(HeadSha::try_from(head).is_err(), "{head}");
    }
    let wire =
        r#"{"repo":"o/r; id","number":12,"head":"0123456789abcdef0123456789abcdef01234567"}"#;
    assert!(serde_json::from_str::<MergeKey>(wire).is_err());
}

/// A value flag's value is never read as the request's number, and a repo flag that names no
/// repo leaves the request unread.
#[test]
fn a_merges_request_is_read_past_its_flags() {
    let read = |cmd: &str| held(cmd);
    assert_eq!(
        read("gh pr merge --subject 7 12 --repo=o/r"),
        Some(Held::Request(Request {
            repo: Some(Repo::try_from("o/r").unwrap()),
            number: 12
        }))
    );
    assert_eq!(read("gh pr merge -R 'o/r; id' 12"), Some(Held::Unreadable));
    assert_eq!(read("gh pr merge feature-branch"), Some(Held::Unreadable));
}

/// Q239 (S3): the hook's refusal is a hold only where the app confirmed it holds the ask, and
/// the chat is told the person is asked only then; every other answer is a block, and says
/// nobody was asked.
#[test]
fn a_held_merge_is_a_hold_only_where_the_app_confirmed_its_ask() {
    let request = Held::Request(Request {
        repo: None,
        number: 12,
    });
    let held = AppSaid::Held("The person is asked in Asks to merge request 12.");
    assert_eq!(reason(&request, &held), REASON);
    assert!(refusal(&request, &held).contains("The person is asked"));
    for app in [
        AppSaid::Refused("chat 4 is not one this app has open."),
        AppSaid::Unheard,
    ] {
        assert_eq!(reason(&request, &app), REASON_NOT_ASKED, "{app:?}");
        let said = refusal(&request, &app);
        assert!(
            said.contains("nobody was asked") || said.contains("Nobody was asked"),
            "{said}"
        );
        assert!(!said.contains("The person is asked"), "{said}");
    }
    assert_eq!(reason(&Held::Unreadable, &held), REASON_UNREADABLE);
}

/// Q239 (S8): a repo is the project's only where its inventory, read as `inventory::load` reads
/// it, lists it on the same forge, host and path; a link that takes the inventory out of the
/// project lists nothing.
#[test]
fn a_repo_is_the_projects_only_on_its_listed_host_and_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("inventory")).unwrap();
    let row = |web: &str| serde_json::json!({"path_with_namespace": "o/r", "forge": "github", "web_url": web});
    let write = |rows: Vec<serde_json::Value>| {
        std::fs::write(
            crate::inventory::path(root),
            serde_json::json!({"repos": rows}).to_string(),
        )
        .unwrap();
    };
    let github = crate::forge::Forge::default_of(crate::forge::Kind::GitHub);
    write(vec![row("https://github.com/o/r")]);
    assert!(listed_in(root, &github, "o/r"));
    assert!(!listed_in(root, &github, "o/other"));
    let elsewhere = crate::forge::Forge {
        kind: crate::forge::Kind::GitHub,
        host: "git.example.com".to_owned(),
    };
    assert!(
        !listed_in(root, &elsewhere, "o/r"),
        "same path, another host"
    );
    write(vec![row("")]);
    assert!(
        !listed_in(root, &github, "o/r"),
        "a row with no address names no host"
    );

    let outside = tempfile::tempdir().unwrap();
    std::fs::write(
        outside.path().join("repos.json"),
        serde_json::json!({"repos": [row("https://github.com/o/r")]}).to_string(),
    )
    .unwrap();
    std::fs::remove_dir_all(root.join("inventory")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.join("inventory")).unwrap();
    assert!(
        !listed_in(root, &github, "o/r"),
        "a linked inventory lists nothing"
    );
}

/// Q239: `purlis change land` is held in a chat the app started, and its refusal says what it
/// is, a landing the person does now, not a `gh pr merge` it never ran.
#[test]
fn a_change_land_is_held_and_told_it_is_the_persons_now() {
    for cmd in [
        "purlis change land api-2",
        "charter change land api-2 --repo api",
    ] {
        assert_eq!(held(cmd), Some(Held::Land), "{cmd}");
        let said = refusal(&Held::Land, &AppSaid::Unheard);
        assert!(said.contains("`purlis change land`"), "{said}");
        assert!(said.contains("nobody was asked"), "{said}");
        assert!(!said.contains("cannot tell which request"), "{said}");
        assert_eq!(reason(&Held::Land, &AppSaid::Unheard), REASON_UNREADABLE);
    }
}
