// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The index's writer plans its sources in batches on every thread and writes them in order
//! (#5486). On a project with far more sources than one batch holds, a sweep after files were
//! changed, added, deleted and only touched, early and late in the order, leaves the index
//! answering exactly what an index built afresh answers.

use std::path::{Path, PathBuf};

use purlis_core::projectindex::{self, Kind, Reach, Reader, Writer};

/// Enough workspaces that their sources (three each) span several batches.
const WORKSPACES: usize = 40;

fn write(root: &Path, at: &str, title: &str, body: &str) {
    let path = root.join(at);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        path,
        format!("# {title}\n\n_2026-03-02 09:14 · persistent_\n\n{body}\n"),
    )
    .unwrap();
}

fn at(n: usize, file: usize) -> String {
    format!("workspaces/ws-{n:02}/memory/20260302-0914{file:02}-note.md")
}

fn project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    for n in 0..WORKSPACES {
        for file in 0..3 {
            write(
                &root,
                &at(n, file),
                &format!("Note {file} of {n}"),
                "The deploy pipeline runs on mondays.",
            );
        }
    }
    (dir, root)
}

/// Every workspace row the index lists, and every one it finds for `deploy` and `fridays`.
fn answers(root: &Path) -> Vec<Vec<(PathBuf, String)>> {
    let reader = Reader::open(root).expect("an index");
    let reach = Reach::none().with(Kind::Workspace, None);
    let rows = |rows: Vec<projectindex::Row>| -> Vec<(PathBuf, String)> {
        let mut rows: Vec<_> = rows.into_iter().map(|r| (r.path, r.title)).collect();
        rows.sort();
        rows
    };
    vec![
        rows(reader.newest(&reach, 10_000).unwrap()),
        rows(reader.search(&reach, &["deploy".into()], 10_000).unwrap()),
        rows(reader.search(&reach, &["fridays".into()], 10_000).unwrap()),
    ]
}

#[test]
fn a_sweep_across_many_sources_holds_what_a_fresh_build_holds() {
    purlis_core::unsteered!();
    let (_dir, root) = project();
    let mut writer = Writer::open(&root).unwrap();
    // The first workspace's sources come in the first batch, the last's in the last.
    for n in [0, WORKSPACES / 2, WORKSPACES - 1] {
        write(
            &root,
            &at(n, 0),
            "Changed",
            "Deploys freeze on fridays now.",
        );
        std::fs::remove_file(root.join(at(n, 1))).unwrap();
        write(&root, &at(n, 7), "Added", "A new note about fridays.");
        // Same text, newer stat: kept, only its stat recorded.
        let same = std::fs::read(root.join(at(n, 2))).unwrap();
        std::fs::write(root.join(at(n, 2)), same).unwrap();
    }

    let swept = writer.sweep(None).unwrap();
    let after_sweep = answers(&root);
    drop(writer);
    std::fs::remove_dir_all(projectindex::home(&root)).unwrap();
    let _fresh = Writer::open(&root).unwrap();
    let fresh = answers(&root);

    assert_eq!(swept.written, 6, "three changed and three added");
    assert_eq!(swept.deleted, 3, "three deleted");
    assert_eq!(after_sweep[0].len(), WORKSPACES * 3);
    assert_eq!(
        after_sweep[2].len(),
        6,
        "the changed and the added say fridays"
    );
    assert_eq!(after_sweep, fresh);
}
