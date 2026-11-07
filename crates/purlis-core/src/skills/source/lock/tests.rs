// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The lockfiles' shape, read back as written, and refused whole where an entry is not one purlis
//! writes.

use super::*;

fn entry(level: Level) -> Locked {
    Locked {
        source_name: None,
        name: "deploy".into(),
        level: AtLevel(level),
        source: Origin::Git(Remote {
            url: "https://example.com/skills".into(),
            branch: None,
        }),
        folder: "skills/deploy".into(),
        pin: Pin::Commit(Commit::parse(&"a".repeat(40)).unwrap()),
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
        skills: vec![entry(Level::workspace("ops")), entry(Level::project())],
    };
    write(root, Kept::Shared, None, &lock).unwrap();
    let text = std::fs::read_to_string(root.join(FILE)).unwrap();
    assert!(text.contains("\"level\": \"skills\""), "{text}");
    assert!(
        text.contains("\"level\": \"workspaces/ops/skills\""),
        "{text}"
    );
    let read_back = read(root, Kept::Shared).unwrap();
    assert_eq!(read_back.lock.skills.len(), 2);
    assert_eq!(read_back.lock.skills[0].level.0, Level::project());
    // A write over a lockfile that changed since it was read is refused.
    assert!(write(root, Kept::Shared, Some("{}"), &lock).is_err());
    assert!(write(root, Kept::Shared, read_back.was.as_deref(), &lock).is_ok());
    // A local level's entry belongs in the local lockfile alone.
    let local = Lockfile {
        version: VERSION,
        skills: vec![entry(Level::project().local(true))],
    };
    write(root, Kept::Local, None, &local).unwrap();
    assert!(read(root, Kept::Local).is_ok());
    std::fs::copy(root.join(LOCAL_FILE), root.join(FILE)).unwrap();
    assert!(read(root, Kept::Shared).is_err());
}

#[test]
fn a_missing_lockfile_is_empty_and_a_newer_one_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    assert!(read(root, Kept::Shared).unwrap().lock.skills.is_empty());
    std::fs::write(root.join(FILE), "{\"version\": 9, \"skills\": []}").unwrap();
    assert!(read(root, Kept::Shared).is_err());
    assert_eq!(Kept::of(&Level::project().local(true)), Kept::Local);
}

#[cfg(unix)]
#[test]
fn a_lockfile_that_is_a_link_is_never_read_or_written_through() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("elsewhere"), "{\"version\":1}").unwrap();
    std::os::unix::fs::symlink(root.join("elsewhere"), root.join(FILE)).unwrap();
    assert!(read(root, Kept::Shared).is_err());
    assert!(write(root, Kept::Shared, None, &Lockfile::default()).is_err());
    assert_eq!(
        std::fs::read_to_string(root.join("elsewhere")).unwrap(),
        "{\"version\":1}"
    );
}

#[test]
fn a_lockfile_naming_a_way_out_is_refused_whole() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    for (name, path, folder) in [
        ("../../evil", "SKILL.md", "skills/a"),
        ("deploy", "../../.zshrc", "skills/a"),
        ("deploy", "SKILL.md", "../out"),
    ] {
        let mut one = entry(Level::project());
        one.name = name.into();
        one.files[0].path = path.into();
        one.folder = folder.into();
        let text = serde_json::to_string(&Lockfile {
            version: VERSION,
            skills: vec![one],
        })
        .unwrap();
        std::fs::write(root.join(FILE), text).unwrap();
        let why = read(root, Kept::Shared).err().unwrap();
        assert!(!why.contains(FILE), "{why}");
    }
}

fn from_extension(level: Level) -> Locked {
    Locked {
        source: Origin::Extension("acme-registry".into()),
        pin: Pin::Revision(Revision::parse("v1.2.3+build.7").unwrap()),
        folder: "deploy".into(),
        ..entry(level)
    }
}

#[test]
fn an_extension_sources_entry_is_pinned_by_its_revision_and_makes_the_file_version_two() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let lock = Lockfile {
        version: VERSION,
        skills: vec![from_extension(Level::project())],
    };
    write(root, Kept::Shared, None, &lock).unwrap();
    let text = std::fs::read_to_string(root.join(FILE)).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["version"], 2, "{text}");
    let one = &doc["skills"][0];
    assert_eq!(
        one["source"],
        serde_json::json!({ "extension": "acme-registry" })
    );
    assert_eq!(one["revision"], "v1.2.3+build.7");
    assert!(one.get("commit").is_none(), "{text}");
    let read_back = read(root, Kept::Shared).unwrap();
    assert_eq!(read_back.lock.skills, lock.skills);

    // A file of git entries alone stays version 1, so an older purlis still reads it.
    let git = Lockfile {
        version: VERSION,
        skills: vec![entry(Level::project())],
    };
    write(root, Kept::Shared, read_back.was.as_deref(), &git).unwrap();
    let text = std::fs::read_to_string(root.join(FILE)).unwrap();
    assert!(text.contains("\"version\": 1"), "{text}");
    assert!(read(root, Kept::Shared).is_ok());
}

#[test]
fn an_entry_that_mixes_a_git_source_with_a_revision_or_names_a_bad_extension_is_refused_whole() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let good = || {
        serde_json::json!({
            "name": "deploy", "level": "skills", "folder": "deploy",
            "source": { "extension": "acme-registry" }, "revision": "r1",
            "digest": "sha256:00", "files": [{ "path": "SKILL.md", "sha256": "sha256:11" }],
        })
    };
    let refused = |one: serde_json::Value| {
        let doc = serde_json::json!({ "version": 2, "skills": [one] });
        std::fs::write(root.join(FILE), doc.to_string()).unwrap();
        read(root, Kept::Shared).is_err()
    };
    assert!(!refused(good()), "the good entry reads");
    std::fs::write(
        root.join(FILE),
        serde_json::json!({ "version": 1, "skills": [good()] }).to_string(),
    )
    .unwrap();
    assert!(
        read(root, Kept::Shared).is_err(),
        "an extension's entry in a version 1 file"
    );
    let mut both = good();
    both["source"]["url"] = "https://example.com/skills".into();
    assert!(refused(both), "a source that is both git and an extension");
    let mut commit = good();
    commit["commit"] = "a".repeat(40).into();
    assert!(
        refused(commit),
        "a pin that is both a commit and a revision"
    );
    let mut git_revision = good();
    git_revision["source"] = serde_json::json!({ "url": "https://example.com/skills" });
    assert!(refused(git_revision), "a git source pinned by a revision");
    let mut no_pin = good();
    no_pin.as_object_mut().unwrap().remove("revision");
    assert!(refused(no_pin), "an extension's entry with no revision");
    for bad in ["../up", "", "a/b", "-dash"] {
        let mut one = good();
        one["source"]["extension"] = bad.into();
        assert!(refused(one), "the extension id {bad:?}");
    }
    for bad in ["", "a b", "../x", &"r".repeat(129)] {
        let mut one = good();
        one["revision"] = bad.into();
        assert!(refused(one), "the revision {bad:?}");
    }
}
