// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a host keeps of each chat it composed in a sandbox (#2047).

use super::*;

fn composed(persona: &str) -> Composed {
    Composed {
        persona: Some(persona.into()),
        folder: PathBuf::from("/work/chat"),
        harness: None,
        confines: Confines::default(),
    }
}

#[test]
fn a_chat_composed_here_asks_as_this_host_recorded_it() {
    let root = tempfile::tempdir().unwrap();
    record(root.path(), 3, composed("devops"));

    let recorded = asker(root.path(), 3).expect("recorded");

    assert_eq!(recorded.chat, 3);
    assert_eq!(recorded.persona.as_deref(), Some("devops"));
    assert_eq!(recorded.folder.as_deref(), Some(Path::new("/work/chat")));
    assert!(recorded.confines.is_some(), "held to its sandbox here");
    assert!(
        asker(root.path(), 4).is_none(),
        "another number is no chat of its"
    );
}

#[test]
fn a_chat_forgotten_or_of_another_project_has_no_record_here() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    record(root.path(), 3, composed("devops"));

    assert!(asker(other.path(), 3).is_none(), "another project's chat 3");
    forget(root.path(), 3);
    assert!(asker(root.path(), 3).is_none(), "forgotten");
}

#[test]
fn a_chat_composed_again_under_the_same_number_is_the_new_one() {
    let root = tempfile::tempdir().unwrap();
    record(root.path(), 3, composed("devops"));
    record(root.path(), 3, composed("writer"));

    assert_eq!(
        asker(root.path(), 3)
            .and_then(|recorded| recorded.persona)
            .as_deref(),
        Some("writer")
    );
}
