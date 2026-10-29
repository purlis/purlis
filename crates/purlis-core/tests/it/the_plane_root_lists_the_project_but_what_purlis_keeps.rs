// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Issue 1916 (rulings G3, F1 and F2 of 2026-10-13): **at the plane root, the window lists and
//! opens the whole project's files** — less what purlis keeps to itself, and with each
//! workspace's clone as one row that leads to that workspace.
//!
//! The project here is a real repository, read by git in this process and by gitoxide in the
//! bounded reader's child, and it is a hostile one: it tracks its own vault file, so git offers
//! what purlis must still refuse. `files/plane_root_tests.rs` asks the same from a reader whose
//! answers a test writes; this holds the two real readers to it.
// Links are made with the Unix call; Windows reads the same code.
#![cfg(unix)]

use crate::support;

use std::path::Path;

use purlis_core::files::{self, Answer, Ask, Branch, Entry, Opened};

const ROOT: Branch<'static> = Branch::plane_root();

/// The bounded reader, as the app starts it, but this test binary run again.
fn reader() -> files::Reader {
    files::Reader::new(
        std::env::current_exe().expect("the test binary"),
        [
            "the_plane_root_lists_the_project_but_what_purlis_keeps::reader_child",
            "--exact",
            "--nocapture",
            "--test-threads=1",
            files::READ_ARG,
        ]
        .map(std::ffi::OsString::from),
    )
}

/// The reader's child: in a run of this binary that [`reader`] started, it answers the one
/// question it was asked and exits; in any other run it does nothing.
#[test]
fn reader_child() {
    purlis_core::unsteered!();
    if let Some(code) = files::serve_if_asked() {
        std::process::exit(code);
    }
}

fn write(at: &Path, path: &str, text: &str) {
    let file = at.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

/// The state folder a project made now has.
fn state() -> &'static str {
    purlis_core::names::STATE_DIR.spellings().next().unwrap()
}

/// A project that is a repository of its own, with a workspace `alpha` and its clone `thing`
/// (ignored by the project's git, as a project's clones are), a build folder git ignores, and
/// purlis's state folder — whose vault file and key the project tracks, though it never should.
fn project() -> support::Fixture {
    let f = support::plane_with_clone("thing");
    let plane = &f.plane;
    support::git(plane, &["init", "-q", "-b", "main", "."]);
    write(plane, ".gitignore", "/workspaces/*/thing/\ntarget/\n");
    write(plane, "README.md", "read me\n");
    write(plane, "docs/guide.md", "a guide\n");
    write(plane, "workspaces/alpha/workspace.md", "alpha\n");
    write(plane, "target/debug/out.bin", "built\n");
    let s = state();
    write(plane, &format!("{s}/vaults/forge.json"), "the vault\n");
    write(plane, &format!("{s}/fingerprint.key"), "the key\n");
    write(plane, &format!("{s}/app/spool/1/turn"), "a turn\n");
    write(plane, &format!("{s}/app/dispatches/7.json"), "a brief\n");
    write(
        plane,
        &format!("{s}/handbacks/chat-1/report.md"),
        "a report\n",
    );
    write(plane, &format!("{s}/sessions/one.md"), "a record\n");
    support::git(plane, &["add", "-A"]);
    support::git(plane, &["commit", "-q", "-m", "a project"]);
    f
}

fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|one| one.name.as_str()).collect()
}

fn entry<'a>(entries: &'a [Entry], name: &str) -> &'a Entry {
    entries
        .iter()
        .find(|one| one.name == name)
        .unwrap_or_else(|| panic!("no {name} in {entries:?}"))
}

fn kept(path: &str) -> String {
    format!(
        "'{path}' is one of the files purlis keeps to itself (vaults, keys and its own records), \
         so it is not listed or opened here"
    )
}

#[test]
fn the_plane_root_lists_the_projects_top_folder_as_a_branch_lists_its_own() {
    purlis_core::unsteered!();
    let f = project();

    let top = files::tree(&f.plane, ROOT, "").unwrap().entries;

    assert_eq!(
        names(&top),
        [
            ".git",
            state(),
            "docs",
            "target",
            "workspaces",
            ".gitignore",
            "charter.toml",
            "README.md"
        ],
        "{top:?}"
    );
    // What git ignores is marked, and expands lazily as in any branch; nothing below is read.
    assert!(entry(&top, "target").ignored);
    assert!(entry(&top, ".git").refused.is_some());
    assert!(entry(&top, "docs").refused.is_none());
    assert_eq!(
        files::open(&f.plane, ROOT, "README.md").unwrap(),
        Opened::Text {
            text: "read me\n".into()
        }
    );
}

#[test]
fn what_purlis_keeps_is_neither_listed_nor_opened_though_the_project_tracks_it() {
    purlis_core::unsteered!();
    let f = project();
    let s = state();

    let state_folder = files::tree(&f.plane, ROOT, s).unwrap().entries;
    assert_eq!(
        names(&state_folder),
        ["app", "sessions"],
        "{state_folder:?}"
    );
    let app = files::tree(&f.plane, ROOT, &format!("{s}/app")).unwrap();
    assert!(app.entries.is_empty(), "{app:?}");

    let offered = files::list(&f.plane, ROOT).unwrap();
    assert!(
        offered.contains(&format!("{s}/sessions/one.md")),
        "{offered:?}"
    );
    for path in [
        format!("{s}/vaults/forge.json"),
        format!("{s}/fingerprint.key"),
        format!("{s}/app/spool/1/turn"),
        format!("{s}/app/dispatches/7.json"),
        format!("{s}/handbacks/chat-1/report.md"),
    ] {
        assert!(!offered.contains(&path), "{path} is offered");
        assert_eq!(
            files::open(&f.plane, ROOT, &path).unwrap_err().to_string(),
            kept(&path)
        );
        assert_eq!(
            files::place(&f.plane, ROOT, &path).unwrap_err().to_string(),
            kept(&path)
        );
    }
    let vaults = format!("{s}/vaults");
    assert_eq!(
        files::tree(&f.plane, ROOT, &vaults)
            .unwrap_err()
            .to_string(),
        kept(&vaults)
    );
    assert_eq!(
        files::folder(&f.plane, ROOT, &vaults)
            .unwrap_err()
            .to_string(),
        kept(&vaults)
    );
}

#[test]
fn a_link_or_a_second_name_reaches_nothing_purlis_keeps() {
    purlis_core::unsteered!();
    let f = project();
    let vault = f.plane.join(state()).join("vaults/forge.json");
    std::os::unix::fs::symlink(&vault, f.plane.join("docs/peek.json")).unwrap();
    support::git(&f.plane, &["add", "-A"]);
    support::git(&f.plane, &["commit", "-q", "-m", "a link in"]);

    let docs = files::tree(&f.plane, ROOT, "docs").unwrap().entries;
    assert_eq!(
        entry(&docs, "peek.json").refused.as_deref(),
        Some("a link purlis does not follow from the project's own files")
    );
    assert_eq!(
        files::open(&f.plane, ROOT, "docs/peek.json")
            .unwrap_err()
            .to_string(),
        kept("docs/peek.json")
    );

    // A second name of the vault's file is kept as the file is: it is not listed, and nor is
    // a link that leads to that file.
    std::fs::hard_link(&vault, f.plane.join("docs/copy.json")).unwrap();
    support::git(&f.plane, &["add", "-A"]);
    support::git(&f.plane, &["commit", "-q", "-m", "a second name"]);

    let docs = files::tree(&f.plane, ROOT, "docs").unwrap().entries;
    assert_eq!(names(&docs), ["guide.md"]);
    for path in ["docs/peek.json", "docs/copy.json"] {
        assert_eq!(
            files::open(&f.plane, ROOT, path).unwrap_err().to_string(),
            kept(path)
        );
    }
}

#[test]
fn a_workspaces_clone_is_one_row_and_nothing_in_it_is_the_plane_roots() {
    purlis_core::unsteered!();
    let f = project();

    let alpha = files::tree(&f.plane, ROOT, "workspaces/alpha")
        .unwrap()
        .entries;

    let thing = entry(&alpha, "thing");
    // Drawn though the project's git ignores it: it is how a person gets to that workspace.
    assert!(!thing.ignored, "{thing:?}");
    assert_eq!(thing.workspace.as_deref(), Some("alpha"));
    assert_eq!(
        thing.refused.as_deref(),
        Some("a repo of workspace 'alpha', whose files are on that workspace's tab")
    );
    assert!(entry(&alpha, "workspace.md").workspace.is_none());

    let inside = "'workspaces/alpha/thing' is in a repo of workspace 'alpha', whose files are on \
                  that workspace's tab";
    assert_eq!(
        files::tree(&f.plane, ROOT, "workspaces/alpha/thing")
            .unwrap_err()
            .to_string(),
        inside
    );
    assert!(files::open(&f.plane, ROOT, "workspaces/alpha/thing/README.md").is_err());
    // And the workspace's own tab still reads it, as before.
    assert!(files::open(&f.plane, Branch::repo("alpha", "thing"), "README.md").is_ok());
}

#[test]
fn no_names_reach_the_plane_root_and_a_project_in_someone_elses_repository_is_not_listed() {
    purlis_core::unsteered!();
    let f = project();

    // A branch named by empty names is no branch: only `Branch::plane_root` is the plane root.
    assert!(files::tree(&f.plane, Branch::repo("", ""), "").is_err());
    assert!(files::open(&f.plane, Branch::repo("", ""), "README.md").is_err());

    // A project folder that git answers for only because a folder above it is a repository.
    let inner = f.plane.join("docs");
    let refused = files::tree(&inner, ROOT, "").unwrap_err().to_string();
    assert!(
        refused.contains("not a git repository of its own"),
        "{refused}"
    );
}

#[test]
fn the_readers_child_reads_the_plane_root_the_same_and_compares_nothing_of_it() {
    purlis_core::unsteered!();
    let f = project();
    let reader = reader();
    let s = state();

    let root = files::root(&reader, &f.plane, ROOT).unwrap();
    assert_eq!(root.path(), f.plane.as_path());

    let here = files::tree(&f.plane, ROOT, "").unwrap();
    assert_eq!(root.tree(&reader, "").unwrap(), here);
    assert_eq!(
        names(&root.tree(&reader, s).unwrap().entries),
        ["app", "sessions"]
    );
    assert_eq!(
        root.open(&reader, "docs/guide.md").unwrap(),
        Opened::Text {
            text: "a guide\n".into()
        }
    );
    let vault = format!("{s}/vaults/forge.json");
    assert_eq!(
        root.open(&reader, &vault).unwrap_err().to_string(),
        kept(&vault)
    );
    // The child itself offers the tracked vault file: the refusal is not git's to give.
    match reader
        .ask(&f.plane, ROOT, Ask::Offered { most: None })
        .unwrap()
    {
        Answer::Offered(offered) => assert!(offered.files.contains(&vault), "{offered:?}"),
        other => panic!("{other:?}"),
    }

    for ask in [Ask::Status, Ask::AheadBehind] {
        let refused = reader.ask(&f.plane, ROOT, ask).unwrap_err().to_string();
        assert_eq!(
            refused,
            "purlis does not show what changed for the project's own files"
        );
    }
}
