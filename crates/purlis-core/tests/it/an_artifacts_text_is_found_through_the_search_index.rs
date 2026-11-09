// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Artifacts in search** (ADR 0120, spec #5948, #5968): what a pinned artifact's page says is
//! in the project's search index, read through the pin's own gate (a folder inside its
//! workspace's `artifacts/`, no link out) and the artifact folder's walk that never follows a
//! link. The index finds what a scan of the same files finds, and nothing a person's own data
//! files say: they are each person's, never the project's.

use std::path::{Path, PathBuf};

use purlis_core::artifact::{self, search};
use purlis_core::projectindex::Writer;

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap().join("project");
    std::fs::create_dir_all(root.join("workspaces/alpha/artifacts")).unwrap();
    std::fs::create_dir_all(root.join("workspaces/beta/artifacts")).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    (dir, root)
}

fn page(root: &Path, workspace: &str, folder: &str, html: &str) -> PathBuf {
    let dir = root
        .join("workspaces")
        .join(workspace)
        .join("artifacts")
        .join(folder);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), html).unwrap();
    dir
}

fn pin(root: &Path, workspace: &str, folder: &str, name: &str) {
    artifact::pin(
        root,
        &root.join("workspaces").join(workspace),
        &artifact::Request {
            folder: folder.into(),
            name: Some(name.into()),
            ..Default::default()
        },
    )
    .unwrap();
}

/// What the index finds for `query` across every workspace, and what a scan finds: each as
/// `workspace/folder name`, sorted, after asserting both agree.
fn found(root: &Path, query: &str) -> Vec<String> {
    let said = |found: Vec<search::Found>| {
        let mut out: Vec<String> = found
            .into_iter()
            .map(|f| format!("{}/{} {}", f.workspace, f.folder, f.name))
            .collect();
        out.sort();
        out
    };
    let indexed = search::find(root, None, query);
    assert!(indexed.indexed, "the index answered '{query}'");
    let indexed = said(indexed.found);
    let scanned = said(search::scan(root, None, query));
    assert_eq!(
        indexed, scanned,
        "the index and the scan agree on '{query}'"
    );
    indexed
}

#[test]
fn a_pinned_artifacts_page_is_found_by_what_it_says_and_an_unpinned_one_is_not() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    page(
        &root,
        "alpha",
        "board",
        "<title>Team board</title><h1>Deploy freeze</h1><p>Milestones on track</p>",
    );
    pin(&root, "alpha", "board", "Team board");
    page(&root, "alpha", "draft", "<p>Deploy draft never pinned</p>");
    page(&root, "beta", "costs", "<p>Cloud costs by month</p>");
    pin(&root, "beta", "costs", "Costs");
    drop(Writer::open(&root).unwrap());

    assert_eq!(found(&root, "deploy"), ["alpha/board Team board"]);
    assert_eq!(
        found(&root, "milest"),
        ["alpha/board Team board"],
        "a prefix"
    );
    assert_eq!(found(&root, "costs"), ["beta/costs Costs"]);
    assert_eq!(found(&root, "team"), ["alpha/board Team board"], "the name");
    assert!(found(&root, "draft").is_empty(), "not pinned, not found");

    let only_beta = search::find(&root, Some("beta"), "deploy costs");
    assert_eq!(
        only_beta
            .found
            .iter()
            .map(|f| f.folder.as_str())
            .collect::<Vec<_>>(),
        ["costs"],
        "one workspace's artifacts alone"
    );
}

#[test]
fn neither_data_files_nor_script_nor_style_are_indexed() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let board = page(
        &root,
        "alpha",
        "board",
        "<h1>Board</h1><script>const zebra = 1;</script><style>.giraffe{}</style>",
    );
    std::fs::write(board.join("data.json"), r#"{"velocity": 12}"#).unwrap();
    std::fs::create_dir_all(board.join("data")).unwrap();
    std::fs::write(board.join("data/week.json"), r#"{"burndown": 3}"#).unwrap();
    // A page among the data is data too: written on each person's machine, never the template.
    std::fs::write(board.join("data/week.html"), "<p>Standup attendance</p>").unwrap();
    std::fs::write(board.join("notes.html"), "<p>Retrospective notes</p>").unwrap();
    pin(&root, "alpha", "board", "Board");
    drop(Writer::open(&root).unwrap());

    for unsaid in ["velocity", "burndown", "attendance", "zebra", "giraffe"] {
        assert!(found(&root, unsaid).is_empty(), "'{unsaid}' is not said");
    }
    assert_eq!(
        found(&root, "retrospective"),
        ["alpha/board Board"],
        "every page of the template"
    );
}

#[cfg(unix)]
#[test]
fn nothing_outside_the_artifact_folder_is_indexed_through_a_link() {
    purlis_core::unsteered!();
    let (dir, root) = project();
    let outside = dir.path().canonicalize().unwrap().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("index.html"), "<p>Exfiltrated payroll</p>").unwrap();
    std::fs::write(outside.join("leak.html"), "<p>Leaked salaries</p>").unwrap();

    // A page beside the pinned one that is a link out of the folder.
    let board = page(&root, "alpha", "board", "<p>Board</p>");
    std::os::unix::fs::symlink(outside.join("leak.html"), board.join("leak.html")).unwrap();
    pin(&root, "alpha", "board", "Board");
    // A pinned folder swapped for a link out of artifacts/ after it was pinned, as a pull
    // could bring.
    let gone = page(&root, "alpha", "gone", "<p>Gone</p>");
    pin(&root, "alpha", "gone", "Gone");
    std::fs::remove_dir_all(&gone).unwrap();
    std::os::unix::fs::symlink(&outside, &gone).unwrap();
    drop(Writer::open(&root).unwrap());

    assert!(
        found(&root, "salaries").is_empty(),
        "a linked page is not read"
    );
    assert!(
        found(&root, "payroll").is_empty(),
        "a linked folder is not read"
    );
    assert_eq!(found(&root, "board"), ["alpha/board Board"]);
}

#[test]
fn an_edit_a_rename_and_an_unpin_reach_the_index_at_its_next_sweep() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let board = page(&root, "alpha", "board", "<p>Sprint goals</p>");
    pin(&root, "alpha", "board", "Board");
    let mut writer = Writer::open(&root).unwrap();
    assert_eq!(found(&root, "sprint"), ["alpha/board Board"]);

    std::fs::write(board.join("index.html"), "<p>Quarter goals</p>").unwrap();
    pin(&root, "alpha", "board", "Roadmap");
    writer.sweep(Some(&[board.join("index.html")])).unwrap();
    assert!(found(&root, "sprint").is_empty(), "the old words are gone");
    assert_eq!(found(&root, "quarter"), ["alpha/board Roadmap"]);

    artifact::unpin(&root, &root.join("workspaces/alpha"), "board").unwrap();
    writer
        .sweep(Some(&[root.join("workspaces/alpha/artifacts/pins.toml")]))
        .unwrap();
    assert!(found(&root, "quarter").is_empty(), "unpinned, not found");
}

#[test]
fn a_secret_in_a_page_never_reaches_the_index() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    // Put together at run time, so no token is written in this file.
    let token = ["ghp", "_", "Zq7Xw3Vb9Nc5Mk1Lp8Rt2Ys6Ud4Fh0Gj3Ka5"].concat();
    // Pinning refuses a folder holding a secret, so the token arrives after the pin, as a pull
    // could bring it.
    let board = page(&root, "alpha", "board", "<p>Token here</p>");
    pin(&root, "alpha", "board", "Board");
    std::fs::write(
        board.join("index.html"),
        format!("<p>Token {token} here</p>"),
    )
    .unwrap();
    drop(Writer::open(&root).unwrap());

    assert!(
        search::find(&root, None, "zq7xw3vb9nc5mk1lp8rt2ys6ud4fh0gj3ka5")
            .found
            .is_empty()
    );
    assert_eq!(found(&root, "token"), ["alpha/board Board"]);
}
