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
