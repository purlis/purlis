// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The lockfiles' shape, read back as written.

use super::*;

fn entry(level: Level) -> Locked {
    Locked {
        name: "deploy".into(),
        level: AtLevel(level),
        source: From {
            url: "https://example.com/skills".into(),
            branch: None,
            folder: "skills/deploy".into(),
        },
        commit: "a".repeat(40),
        digest: "sha256:00".into(),
        files: vec![Wrote {
            path: "SKILL.md".into(),
            executable: false,
            sha256: "sha256:11".into(),
        }],
        brought_by: None,
    }
}

#[test]
fn a_lockfile_names_each_level_by_its_folder_and_reads_back_as_written() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let lock = Lockfile {
        version: VERSION,
        skills: vec![entry(Level::workspace("ops").local(true)), entry(Level::project())],
    };
    write(root, FILE, None, &lock).unwrap();
    let text = std::fs::read_to_string(root.join(FILE)).unwrap();
    assert!(text.contains("\"level\": \"skills\""), "{text}");
    assert!(text.contains("\"level\": \"workspaces/ops/skills.local\""), "{text}");
    let read_back = read(root, FILE).unwrap();
    assert_eq!(read_back.lock.skills.len(), 2);
    assert_eq!(read_back.lock.skills[0].level.0, Level::project());
    // A write over a lockfile that changed since it was read is refused.
    assert!(write(root, FILE, Some("{}"), &lock).is_err());
    assert!(write(root, FILE, read_back.was.as_deref(), &lock).is_ok());
}

#[test]
fn a_missing_lockfile_is_empty_and_a_newer_one_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    assert!(read(root, FILE).unwrap().lock.skills.is_empty());
    std::fs::write(root.join(FILE), "{\"version\": 9, \"skills\": []}").unwrap();
    assert!(read(root, FILE).is_err());
    assert_eq!(file_for(&Level::project().local(true)), LOCAL_FILE);
}

#[cfg(unix)]
#[test]
fn a_lockfile_that_is_a_link_is_never_read_or_written_through() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("elsewhere"), "{\"version\":1}").unwrap();
    std::os::unix::fs::symlink(root.join("elsewhere"), root.join(FILE)).unwrap();
    assert!(read(root, FILE).is_err());
    assert!(write(root, FILE, None, &Lockfile::default()).is_err());
    assert_eq!(std::fs::read_to_string(root.join("elsewhere")).unwrap(), "{\"version\":1}");
}
