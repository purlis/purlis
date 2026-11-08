// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// GitHub's words for a push refused by a protected branch (`GH006`) and by a ruleset (`GH013`),
/// as git prints them, and for a merge refused by the same rule.
const PROTECTED: &str = "remote: error: GH006: Protected branch update failed for refs/heads/main.\n\
remote: error: Commits must have verified signatures.\n\
To https://github.com/octo/hello.git\n ! [remote rejected] main -> main (protected branch hook declined)";
const RULESET: &str = "remote: error: GH013: Repository rule violations found for refs/heads/p1.\n\
remote: Review all repository rules at https://github.com/octo/hello/rules?ref=refs%2Fheads%2Fp1\n\
remote: \n\
remote: - Commits must have verified signatures.\n\
remote:   Found 1 violation:\n\
remote: \n\
remote:   a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0\n\
remote: \n\
To https://github.com/octo/hello.git\n ! [remote rejected] p1 -> p1 (push declined due to repository rule violations)";
const MERGE: &str = "Commits must have verified signatures.";

#[test]
fn githubs_refusals_of_unsigned_commits_are_told_apart_from_its_other_refusals() {
    for said in [PROTECTED, RULESET, MERGE] {
        assert!(refused_unsigned(Kind::GitHub, said), "{said}");
    }
    for other in [
        "remote: error: GH006: Protected branch update failed for refs/heads/main.\n\
         remote: error: At least 1 approving review is required by reviewers with write access.",
        " ! [rejected]        p1 -> p1 (fetch first)",
        "remote: Permission to octo/hello.git denied to someone.",
        "Pull request is not mergeable",
        "",
    ] {
        assert!(!refused_unsigned(Kind::GitHub, other), "{other}");
    }
}

#[test]
fn gitlabs_refusal_keeps_its_plain_wording_until_it_is_read_from_a_real_server() {
    assert!(!refused_unsigned(
        Kind::GitLab,
        "remote: GitLab: Commit must be signed with a GPG key"
    ));
}

#[test]
fn the_chat_is_told_it_cant_set_signing_up_and_the_person_does_it_in_the_window() {
    let said = refused_said(Kind::GitHub, "p1");
    assert!(
        said.starts_with("GitHub only accepts signed commits here"),
        "{said}"
    );
    assert!(said.contains("A chat can't set signing up"), "{said}");
    assert!(said.contains("Sign and retry"), "{said}");
}

fn refused(piece: &str, at: i64) -> Waiting {
    Waiting {
        ws: "alpha".to_owned(),
        repo: "widget".to_owned(),
        piece: piece.to_owned(),
        branch: piece.to_owned(),
        kind: Kind::GitHub,
        host: "github.com".to_owned(),
        path: "octo/widget".to_owned(),
        commit: "0123abc".to_owned(),
        chat_id: Some("01JCHATALPHA".to_owned()),
        at,
        sent: false,
        why: None,
    }
}

#[test]
fn waiting_work_is_kept_once_per_branch_folder_and_goes_when_pushed() {
    let root = tempfile::tempdir().unwrap();
    assert!(waiting(root.path()).is_empty());
    note_refused(root.path(), refused("p1", 1)).unwrap();
    note_refused(root.path(), refused("p2", 2)).unwrap();
    // Refused again: one entry, the newer.
    note_refused(root.path(), refused("p1", 3)).unwrap();
    let all = waiting(root.path());
    let pieces: Vec<_> = all.iter().map(|one| (one.piece.as_str(), one.at)).collect();
    assert_eq!(pieces, [("p2", 2), ("p1", 3)]);

    // Signed and sent: listed as done until the window has shown it.
    note_pushed(root.path(), "alpha", "widget", "p1", true);
    assert!(
        waiting(root.path())
            .iter()
            .any(|one| one.piece == "p1" && one.sent)
    );
    forget_sent(root.path()).unwrap();
    // Pushed some other way: it waits no more.
    note_pushed(root.path(), "alpha", "widget", "p2", false);
    assert!(waiting(root.path()).is_empty());
}

#[test]
fn githubs_words_inside_another_line_such_as_a_files_path_are_not_its_refusal() {
    let push_protection = "remote: error: GH013: Repository rule violations found for refs/heads/p1.\n\
remote: - GITHUB PUSH PROTECTION\n\
remote:     - Push cannot contain secrets\n\
remote:       path: docs/commits must have verified signatures.txt:3";
    assert!(!refused_unsigned(Kind::GitHub, push_protection));
}

/// #5789: a waiting lane names its chat by the chat's lasting id. A record kept before that,
/// which named it by the app's number, reads with no chat: a number reused since a restart would
/// put its prompt on another chat.
#[test]
fn a_lane_names_its_chat_by_its_lasting_id_and_an_old_number_names_none() {
    let root = tempfile::tempdir().unwrap();
    note_refused(root.path(), refused("p1", 1)).unwrap();
    assert_eq!(
        waiting(root.path())[0].chat_id.as_deref(),
        Some("01JCHATALPHA")
    );

    let old = r#"[{"ws":"alpha","repo":"widget","piece":"p2","branch":"p2","kind":"github",
        "host":"github.com","path":"octo/widget","commit":"0123abc","chat":4,"at":2}]"#;
    std::fs::write(waiting_file(root.path()), old).unwrap();
    let read = waiting(root.path());
    assert_eq!(read.len(), 1, "an old record still waits");
    assert_eq!(read[0].piece, "p2");
    assert_eq!(read[0].chat_id, None);
}
