//! #1083: the change store's `exists`, and the landing and pending readers, asked while a
//! workspace's `changes/` is swapped for a link to another workspace's, never answer from it.

use crate::held::racer::{project, while_swapped};

const RECORD: &str = r#"{"change": "theirs", "why": "theirs", "created": "2026-10-01", "by": "them", "members": [], "excluded": []}"#;
const LANDING: &str = r#"{"change": "theirs", "head": "h", "merge": "m", "number": 7, "repo": "web", "ts": "2026-10-02T09:00:00+00:00"}"#;
const PENDING: &str = r#"{"change": "theirs", "head": "h", "number": 7, "repo": "web", "stage": "asked", "ts": "2026-10-02T09:00:00+00:00", "via": "direct"}"#;

fn changes(root: &std::path::Path, ws: &str) {
    let dir = root.join("workspaces").join(ws).join("changes");
    std::fs::create_dir_all(dir.join("log/pending")).unwrap();
    std::fs::write(dir.join("theirs.json"), RECORD).unwrap();
    std::fs::write(dir.join("log/dev.jsonl"), format!("{LANDING}\n")).unwrap();
    std::fs::write(dir.join("log/pending/dev.jsonl"), format!("{PENDING}\n")).unwrap();
}

#[test]
fn a_changes_store_swapped_for_a_link_never_answers_from_another_workspace() {
    let (_tmp, root) = project(&[]);
    std::fs::create_dir_all(root.join("workspaces/alpha/changes")).unwrap();
    changes(&root, "beta");
    while_swapped(
        &root,
        "workspaces/alpha/changes",
        "../beta/changes",
        || {
            (
                super::exists(&root, "alpha", "theirs")
                    && super::read(&root, "alpha", "theirs").is_ok(),
                super::super::landing::landings(&root, "alpha", "theirs").len(),
                super::super::pending::pendings(&root, "alpha", "theirs").len(),
            )
        },
        |(read, landed, pending)| {
            assert!(!read, "beta's record was read as alpha's");
            assert_eq!(landed, 0, "beta's landing was read as alpha's");
            assert_eq!(pending, 0, "beta's pending line was read as alpha's");
        },
    );
}

#[test]
fn the_landing_and_pending_logs_are_read_from_the_held_store() {
    let (_tmp, root) = project(&[]);
    changes(&root, "alpha");
    assert!(super::exists(&root, "alpha", "theirs"));
    assert!(!super::exists(&root, "alpha", "other"));
    assert!(!super::exists(&root, "gamma", "theirs"));
    assert_eq!(
        super::super::landing::landings(&root, "alpha", "theirs").len(),
        1
    );
    assert!(super::super::pending::pendings(&root, "alpha", "theirs").contains_key("web"));
}

#[test]
fn a_log_folder_that_is_a_link_inside_the_project_is_not_read() {
    let (_tmp, root) = project(&[]);
    changes(&root, "beta");
    std::fs::create_dir_all(root.join("workspaces/alpha/changes")).unwrap();
    std::os::unix::fs::symlink(
        "../../beta/changes/log",
        root.join("workspaces/alpha/changes/log"),
    )
    .unwrap();
    assert!(super::super::landing::landings(&root, "alpha", "theirs").is_empty());
    assert!(super::super::pending::pendings(&root, "alpha", "theirs").is_empty());
}
