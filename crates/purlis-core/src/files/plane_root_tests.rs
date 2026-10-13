// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Issue 1916 (rulings G3, F1, F2): **the plane root's files are the whole project's, less what
//! purlis keeps to itself and less each workspace's clone**, asked from the hostile side.
//!
//! Each test names the plane root the way the window's commands do — a [`super::Root`] the
//! bounded reader found — with a reader whose child answers what a test says git would, so no
//! git runs and the offered list can be made to offer what it never should: the refusals here
//! must hold whatever git says.

use std::path::{Path, PathBuf};

use super::tests::{found_at, ignored, offered, reader_answering_each};
use super::{Branch, Entry, Kind, Opened, Reader, Root};

const ROOT: Branch<'static> = Branch::plane_root();

const KEPT: &str = "is one of the files purlis keeps to itself (vaults, keys and its own \
                    records), so it is not listed or opened here";

fn state() -> &'static str {
    crate::names::STATE_DIR.spellings().next().unwrap()
}

/// A project: purlis's state folder with a vault, its key, a hook spool, a dispatch's record,
/// a waiting report and a session record; a workspace `alpha` with its clone `svc`; a
/// repository of nobody's in `vendor/lib`; and files of its own.
fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let plane = std::fs::canonicalize(dir.path()).unwrap();
    let at = |path: &str| plane.join(path);
    for folder in [
        ".git",
        "docs",
        "vendor/lib/.git",
        "workspaces/alpha/svc/.git",
        "workspaces/alpha/.worktrees",
    ] {
        std::fs::create_dir_all(at(folder)).unwrap();
    }
    for folder in [
        "vaults",
        "app/spool/1",
        "app/dispatches",
        "handbacks/chat-1",
        "sessions",
    ] {
        std::fs::create_dir_all(plane.join(state()).join(folder)).unwrap();
    }
    for (file, text) in [
        ("vaults/forge.json", "the vault\n"),
        ("fingerprint.key", "the key\n"),
        ("app/spool/1/turn", "a turn\n"),
        ("app/dispatches/7.json", "a brief\n"),
        ("handbacks/chat-1/report.md", "a report\n"),
        ("sessions/one.md", "a record\n"),
    ] {
        std::fs::write(plane.join(state()).join(file), text).unwrap();
    }
    for (file, text) in [
        ("README.md", "read me\n"),
        ("docs/guide.md", "a guide\n"),
        ("vendor/lib/x.rs", "\n"),
        ("workspaces/alpha/workspace.md", "alpha\n"),
        ("workspaces/alpha/svc/main.rs", "fn main() {}\n"),
    ] {
        std::fs::write(at(file), text).unwrap();
    }
    (dir, plane)
}

/// The plane root, found by a reader whose child offers `offers` and answers `asked` paths as
/// not ignored.
fn root(plane: &Path, offers: &[&str], asked: usize) -> (tempfile::TempDir, Reader, Root) {
    let (answers, reader) = reader_answering_each(
        &found_at(plane),
        &offered(offers),
        &ignored(&vec![false; asked], &[]),
    );
    let root = super::root(&reader, plane, ROOT).unwrap();
    (answers, reader, root)
}

/// One folder of the plane root, `asked` being how many of its entries git is asked about.
fn level(plane: &Path, folder: &str, asked: usize) -> Vec<Entry> {
    let (_answers, reader, root) = root(plane, &[], asked);
    root.tree(&reader, folder)
        .unwrap_or_else(|refused| panic!("{folder}: {refused}"))
        .entries
}

fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|one| one.name.as_str()).collect()
}

fn refusal<T: std::fmt::Debug>(read: Result<T, super::Refused>) -> String {
    read.expect_err("it is refused").to_string()
}

#[test]
fn the_plane_root_lists_the_projects_own_top_folder() {
    let (_dir, plane) = project();

    let top = level(&plane, "", 5);

    assert_eq!(
        names(&top),
        [".git", state(), "docs", "vendor", "workspaces", "README.md"]
    );
    // git's own folder is drawn as in every branch: hidden with what git ignores, never open.
    assert!(top[0].ignored && top[0].refused.is_some(), "{:?}", top[0]);
    assert!(
        top[1..]
            .iter()
            .all(|one| one.refused.is_none() && one.workspace.is_none()),
        "{top:?}"
    );
}

#[test]
fn what_purlis_keeps_to_itself_is_not_listed() {
    let (_dir, plane) = project();

    // The state folder is the project's to show; the vaults, the key and the waiting
    // reports in it are not, and nor are the spool and the dispatches under `app`.
    assert_eq!(names(&level(&plane, state(), 2)), ["app", "sessions"]);
    assert_eq!(
        names(&level(&plane, &format!("{}/app", state()), 0)),
        Vec::<&str>::new()
    );
    assert_eq!(
        names(&level(&plane, &format!("{}/sessions", state()), 1)),
        ["one.md"]
    );
}

#[test]
fn a_kept_folder_does_not_open_however_it_is_named() {
    let (_dir, plane) = project();
    let (_answers, reader, root) = root(&plane, &[], 0);
    for folder in [
        format!("{}/vaults", state()),
        format!("{}/VAULTS", state().to_uppercase()),
        format!("{}/app/spool", state()),
        format!("{}/app/spool/1", state()),
        format!("{}/app/dispatches", state()),
        format!("{}/handbacks", state()),
        format!("{}/handbacks/chat-1", state()),
        // Said the same of one that is not there: the refusal does not say what exists.
        format!("{}/vaults/not-there", state()),
    ] {
        assert_eq!(
            refusal(root.tree(&reader, &folder)),
            format!("'{folder}' {KEPT}"),
        );
        assert_eq!(refusal(root.folder(&folder)), format!("'{folder}' {KEPT}"));
    }
}

#[test]
fn a_kept_file_does_not_open_even_when_git_offers_it() {
    let (_dir, plane) = project();
    let vault = format!("{}/vaults/forge.json", state());
    let key = format!("{}/fingerprint.key", state());
    let report = format!("{}/handbacks/chat-1/report.md", state());
    let brief = format!("{}/app/dispatches/7.json", state());
    let turn = format!("{}/app/spool/1/turn", state());
    let all = [vault, key, report, brief, turn];
    let offers: Vec<&str> = all.iter().map(String::as_str).collect();
    // The reader's child offers every one of them, as a project that tracks them would.
    let (_answers, reader, root) = root(&plane, &offers, 0);

    for path in &all {
        assert_eq!(
            refusal(root.open(&reader, path)),
            format!("'{path}' {KEPT}")
        );
        assert_eq!(refusal(root.place(path)), format!("'{path}' {KEPT}"));
        assert_eq!(refusal(root.named(path)), format!("'{path}' {KEPT}"));
        let editor =
            root.in_your_editor(&reader, path, 1, crate::youreditor::Editor::VsCode, &|_| {
                None
            });
        assert_eq!(refusal(editor), format!("'{path}' {KEPT}"));
    }
    let shouted = format!("{}/Vaults/FORGE.json", state().to_uppercase());
    assert_eq!(
        refusal(root.open(&reader, &shouted)),
        format!("'{shouted}' {KEPT}")
    );
}

#[test]
fn a_path_cannot_walk_out_of_the_project_or_name_one_from_the_root() {
    let (_dir, plane) = project();
    let outside = plane.parent().unwrap().join("outside.txt");
    let absolute = plane.join("README.md");
    let (_answers, reader, root) = root(&plane, &["README.md"], 0);

    for path in [
        "../outside.txt".to_string(),
        "docs/../../outside.txt".to_string(),
        format!("docs/../{}/vaults/forge.json", state()),
        outside.display().to_string(),
        absolute.display().to_string(),
        String::new(),
    ] {
        let said = format!("'{path}' is not a path inside the branch's folder");
        assert_eq!(refusal(root.open(&reader, &path)), said, "{path}");
        assert_eq!(refusal(root.place(&path)), said, "{path}");
        if !path.is_empty() {
            assert_eq!(refusal(root.tree(&reader, &path)), said, "{path}");
        }
    }
    // What it does offer opens.
    assert_eq!(
        root.open(&reader, "README.md").unwrap(),
        Opened::Text {
            text: "read me\n".into()
        }
    );
}

#[test]
fn a_link_into_what_purlis_keeps_is_not_followed() {
    let (_dir, plane) = project();
    let vaults = plane.join(state()).join("vaults");
    std::os::unix::fs::symlink(vaults.join("forge.json"), plane.join("docs/peek.json")).unwrap();
    std::os::unix::fs::symlink(&vaults, plane.join("docs/shelf")).unwrap();
    std::os::unix::fs::symlink(plane.join("README.md"), plane.join("docs/readme")).unwrap();
    let vault = format!("{}/vaults/forge.json", state());
    let offers = [
        "README.md",
        "docs/guide.md",
        "docs/peek.json",
        "docs/readme",
        "docs/shelf",
        vault.as_str(),
    ];
    // Four questions for git: three of `docs`'s entries, and where the one link that is
    // followed leads. A link that IS a kept folder is not listed at all, as the folder is not.
    let (_answers, reader, root) = root(&plane, &offers, 4);

    let docs = root.tree(&reader, "docs").unwrap().entries;
    assert_eq!(names(&docs), ["guide.md", "peek.json", "readme"]);
    let said = |name: &str| {
        docs.iter()
            .find(|one| one.name == name)
            .unwrap_or_else(|| panic!("no {name} in {docs:?}"))
            .refused
            .clone()
    };
    let not_followed =
        Some("a link purlis does not follow from the project's own files".to_string());
    assert_eq!(said("peek.json"), not_followed);
    // A link that stays among the project's own files is followed, as in any branch.
    assert_eq!(said("readme"), None);

    assert_eq!(
        refusal(root.open(&reader, "docs/peek.json")),
        format!("'docs/peek.json' {KEPT}")
    );
    assert_eq!(
        refusal(root.open(&reader, "docs/shelf/forge.json")),
        format!("'docs/shelf/forge.json' {KEPT}")
    );
    assert_eq!(
        refusal(root.tree(&reader, "docs/shelf")),
        format!("'docs/shelf' {KEPT}")
    );
    assert_eq!(
        root.open(&reader, "docs/readme").unwrap(),
        Opened::Text {
            text: "read me\n".into()
        }
    );
}

#[test]
fn a_second_name_of_a_kept_file_does_not_open() {
    let (_dir, plane) = project();
    let state = plane.join(state());
    std::fs::hard_link(
        state.join("vaults/forge.json"),
        plane.join("docs/copy.json"),
    )
    .unwrap();
    std::fs::hard_link(state.join("fingerprint.key"), plane.join("docs/copy.key")).unwrap();
    std::fs::hard_link(plane.join("README.md"), plane.join("docs/readme.md")).unwrap();
    let (_answers, reader, root) = root(
        &plane,
        &["docs/copy.json", "docs/copy.key", "docs/readme.md"],
        0,
    );

    for path in ["docs/copy.json", "docs/copy.key"] {
        assert_eq!(
            refusal(root.open(&reader, path)),
            format!("'{path}' {KEPT}")
        );
        let editor =
            root.in_your_editor(&reader, path, 1, crate::youreditor::Editor::VsCode, &|_| {
                None
            });
        assert_eq!(refusal(editor), format!("'{path}' {KEPT}"));
    }
    // A file with two names, neither of them kept, opens.
    assert!(root.open(&reader, "docs/readme.md").is_ok());
}

#[test]
fn a_workspaces_clone_is_one_row_that_names_its_workspace() {
    let (_dir, plane) = project();

    // Only `.worktrees` and `workspace.md` are asked of git: the clone is drawn whatever the
    // project's own git says of it.
    let alpha = level(&plane, "workspaces/alpha", 2);

    assert_eq!(names(&alpha), [".worktrees", "svc", "workspace.md"]);
    let svc = &alpha[1];
    assert_eq!(svc.kind, Kind::Folder);
    assert_eq!(svc.workspace.as_deref(), Some("alpha"));
    assert!(!svc.ignored);
    assert_eq!(
        svc.refused.as_deref(),
        Some("a repo of workspace 'alpha', whose files are on that workspace's tab")
    );
    assert!(alpha[0].workspace.is_none() && alpha[2].workspace.is_none());
}

#[test]
fn nothing_of_a_workspaces_clone_is_listed_or_opened_at_the_plane_root() {
    let (_dir, plane) = project();
    let (_answers, reader, root) = root(&plane, &["workspaces/alpha/svc/main.rs"], 0);
    let said = |path: &str| {
        format!(
            "'{path}' is in a repo of workspace 'alpha', whose files are on that workspace's tab"
        )
    };

    for folder in ["workspaces/alpha/svc", "workspaces/alpha/svc/.git"] {
        // git's own folder is refused as git's, as everywhere.
        if folder.ends_with(".git") {
            assert!(root.tree(&reader, folder).is_err());
            continue;
        }
        assert_eq!(refusal(root.tree(&reader, folder)), said(folder));
        assert_eq!(refusal(root.folder(folder)), said(folder));
    }
    let file = "workspaces/alpha/svc/main.rs";
    assert_eq!(refusal(root.open(&reader, file)), said(file));
    assert_eq!(refusal(root.place(file)), said(file));
    assert_eq!(refusal(root.named(file)), said(file));
}

#[test]
fn a_repository_of_no_workspace_is_drawn_as_every_nested_one_is() {
    let (_dir, plane) = project();

    let vendor = level(&plane, "vendor", 1);
    assert_eq!(names(&vendor), ["lib"]);
    assert!(vendor[0].refused.is_none() && vendor[0].workspace.is_none());

    // The tree's own rule for a repository nested in a branch: drawn, and nothing in it opens.
    let lib = level(&plane, "vendor/lib", 0);
    assert_eq!(names(&lib), [".git", "x.rs"]);
    assert_eq!(
        lib[1].refused.as_deref(),
        Some("inside another git repository, whose files purlis does not open here")
    );
}

#[test]
fn the_folder_watch_is_put_on_no_kept_folder_and_no_clone() {
    let (_dir, plane) = project();
    let (_answers, _reader, root) = root(&plane, &[], 0);
    let vaults = format!("{}/vaults", state());
    let spool = format!("{}/app/spool", state());

    let resolved = root.resolve(&[
        "",
        "docs",
        vaults.as_str(),
        spool.as_str(),
        "workspaces/alpha/svc",
        ".git",
    ]);

    let watched: Vec<Option<&Path>> = resolved.iter().map(|one| one.as_deref().ok()).collect();
    assert_eq!(
        watched,
        [
            Some(plane.as_path()),
            Some(plane.join("docs").as_path()),
            None,
            None,
            None,
            None
        ]
    );
}

#[test]
fn the_tree_and_every_open_are_refused_where_the_vault_registry_does_not_read() {
    let (_dir, plane) = project();
    std::fs::write(plane.join("vaults.json"), "{ not json").unwrap();
    let (_answers, reader, root) = root(&plane, &["README.md"], 5);
    let said = "purlis could not read 'the project's own files': the vault registry could not \
                be read, so purlis cannot tell which files are a vault's. Doctor says what is \
                wrong with it";

    assert_eq!(refusal(root.tree(&reader, "")), said);
    assert_eq!(refusal(root.tree(&reader, "docs")), said);
    assert_eq!(refusal(root.open(&reader, "README.md")), said);
    assert_eq!(refusal(root.place("README.md")), said);
    assert!(root.resolve(&["", "docs"]).iter().all(Result::is_err));

    // Mended, it lists again.
    std::fs::remove_file(plane.join("vaults.json")).unwrap();
    assert!(root.tree(&reader, "").is_ok());
}

#[test]
fn the_plane_root_is_no_place_a_palette_or_a_search_looks_in() {
    let (_dir, plane) = project();
    let vault = format!("{}/vaults/forge.json", state());
    let (_answers, reader, _root) = root(&plane, &["README.md", vault.as_str()], 0);
    let scope = [super::Place {
        plane: &plane,
        branch: ROOT,
    }];

    let found = super::Finder::default()
        .reading_with(reader.clone())
        .find(&scope, "forge", 50);
    assert!(found.hits.is_empty(), "{:?}", found.hits);
    assert_eq!(
        found.refused,
        [(
            0,
            "purlis does not find files by name for the project's own files".to_string()
        )]
    );

    let mut search = super::search(&scope, "vault", super::SearchOptions::default())
        .unwrap()
        .reading_with(reader);
    let stop = std::sync::atomic::AtomicBool::new(false);
    let mut heard = Vec::new();
    search.more(50, &stop, &mut |one| heard.push(one));
    assert!(
        heard
            .iter()
            .all(|one| !matches!(one, super::Searched::File(_))),
        "{heard:?}"
    );
}

#[test]
fn no_workspace_repo_or_piece_name_spells_the_plane_root() {
    assert!(ROOT.is_plane_root());
    assert!(!Branch::repo("", "").is_plane_root());
    assert!(!Branch::piece("", "", "").is_plane_root());
    let named = super::Named {
        ws: String::new(),
        repo: String::new(),
        piece: None,
    };
    assert!(!named.branch().is_plane_root());
    assert_ne!(Branch::repo("", ""), ROOT);
}

#[test]
fn a_workspaces_own_branch_keeps_reading_as_it_did() {
    let (_dir, plane) = project();
    // The same folders, read as a repo's own: no guard, so nothing here is hidden by it.
    let folder = plane.clone();
    let (_answers, reader) = reader_answering_each(
        &found_at(&folder),
        &offered(&[]),
        &ignored(&[false; 5], &[]),
    );
    let branch = super::root(&reader, &plane, Branch::repo("alpha", "svc")).unwrap();

    let level = branch.tree(&reader, state()).unwrap().entries;

    assert_eq!(
        names(&level),
        ["app", "handbacks", "sessions", "vaults", "fingerprint.key"]
    );
}
