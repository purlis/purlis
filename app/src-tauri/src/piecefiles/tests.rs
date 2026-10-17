// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::PathBuf;

fn git(dir: &Path, args: &[&str]) {
    let ran = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.email=t@e.invalid", "-c", "user.name=t"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args),
    )
    .unwrap();
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

/// #1189: a reader whose child answers every ask with `answer` (the child's JSON), so a test
/// sees that a command found its branch's folder through the reader and nowhere else.
#[cfg(unix)]
fn reader_answering(answer: &str) -> (tempfile::TempDir, Reader) {
    let dir = tempfile::tempdir().unwrap();
    let said = dir.path().join("answer");
    std::fs::write(&said, format!("\u{1e}charter-read\u{1e}{answer}\n")).unwrap();
    let script = format!("cat >/dev/null; cat '{}'", said.display());
    let reader = Reader::new(
        "/bin/sh".into(),
        ["-c", &script, files::READ_ARG].map(std::ffi::OsString::from),
    );
    (dir, reader)
}

/// #1189: every file command finds its branch's folder through the bounded reader: a reader
/// that refuses the branch refuses each of them in its own sentence, word for word, in a
/// project where nothing could have found a folder any other way.
#[cfg(unix)]
#[test]
fn every_file_command_is_refused_in_the_readers_sentence() {
    let nowhere = tempfile::tempdir().unwrap();
    let plane = nowhere.path();
    let said = "thing in workspace 'alpha' has no branch folder called 'piece'";
    let (_answer, reader) = reader_answering(&serde_json::json!({ "Err": said }).to_string());
    let none = |_: &str| None;
    let refused = |answered: Result<(), String>| answered.unwrap_err();

    assert_eq!(
        refused(file_of(&reader, plane, PIECE, "a.txt").map(drop)),
        said
    );
    assert_eq!(refused(tree_of(&reader, plane, PIECE, "").map(drop)), said);
    assert_eq!(
        refused(launch_of(&reader, plane, PIECE, "a.txt", 1, YourEditor::Zed, &none).map(drop)),
        said
    );
    for absolute in [false, true] {
        assert_eq!(
            refused(path_text(&reader, plane, PIECE, "a.txt", absolute).map(drop)),
            said
        );
    }
    // A path refused by its spelling alone is refused before the branch is asked about.
    assert_eq!(
        refused(path_text(&reader, plane, PIECE, "../a.txt", false).map(drop)),
        "'../a.txt' is not a path inside the branch's folder"
    );
    for path in ["", "a.txt"] {
        assert_eq!(
            refused(placed_of(&reader, plane, PIECE, path).map(drop)),
            said
        );
    }
    for folder in ["", "src"] {
        assert_eq!(
            refused(shell_folder_by(&reader, plane, PIECE, folder).map(drop)),
            said
        );
    }
}

/// #1189 (FM-11): a reader whose child finds the branch at `folder` and answers every other
/// ask — the offered list, the ignore check — with `answer` (the child's JSON).
#[cfg(unix)]
fn reader_finding(folder: &Path, answer: &str) -> (tempfile::TempDir, Reader) {
    let dir = tempfile::tempdir().unwrap();
    let found = serde_json::json!({ "Ok": { "Root": { "base": folder, "refs": [] } } });
    for (name, said) in [("root", found.to_string()), ("other", answer.to_string())] {
        std::fs::write(
            dir.path().join(name),
            format!("\u{1e}charter-read\u{1e}{said}\n"),
        )
        .unwrap();
    }
    let at = dir.path().display();
    let script = format!(
        "q=$(cat); case \"$q\" in *'\"ask\":\"Root\"'*) cat '{at}/root';; \
             *) cat '{at}/other';; esac"
    );
    let reader = Reader::new(
        "/bin/sh".into(),
        ["-c", &script, files::READ_ARG].map(std::ffi::OsString::from),
    );
    (dir, reader)
}

/// #1189 (FM-11): the light editor, the editor hand-off and the tree read what git would
/// answer — the offered list, what is ignored — in the reader's child, not in this process:
/// a child that refuses those reads refuses each command in its sentence, in a folder no git
/// here could read.
#[cfg(unix)]
#[test]
fn the_file_commands_read_git_only_in_the_reader() {
    let dir = tempfile::tempdir().unwrap();
    let plane = std::fs::canonicalize(dir.path()).unwrap();
    let folder = plane.join("workspaces/alpha/.worktrees/thing/piece");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("a.txt"), "a\n").unwrap();
    let said = "purlis could not read 'the files of piece': the index is not one";
    let refusing = serde_json::json!({ "Err": said }).to_string();
    let (_answers, reader) = reader_finding(&folder, &refusing);
    let none = |_: &str| None;
    let refused = |answered: Result<(), String>| answered.unwrap_err();

    assert_eq!(
        refused(file_of(&reader, &plane, PIECE, "a.txt").map(drop)),
        said
    );
    assert_eq!(
        refused(launch_of(&reader, &plane, PIECE, "a.txt", 1, YourEditor::Zed, &none).map(drop)),
        said
    );
    assert_eq!(refused(tree_of(&reader, &plane, PIECE, "").map(drop)), said);

    // Answered, the offered list is what opens.
    let offered = serde_json::json!({
        "Ok": { "Offered": { "files": ["a.txt"], "total": 1 } }
    });
    let (_answers, reader) = reader_finding(&folder, &offered.to_string());
    assert!(matches!(
        file_of(&reader, &plane, PIECE, "a.txt"),
        Ok(PieceFile::Text { .. })
    ));
}

/// #1189: Copy path, Reveal and the shell's folder act on the folder the reader found.
#[cfg(unix)]
#[test]
fn copy_path_reveal_and_the_shell_use_the_folder_the_reader_found() {
    let dir = tempfile::tempdir().unwrap();
    let plane = std::fs::canonicalize(dir.path()).unwrap();
    let folder = plane.join("workspaces/alpha/.worktrees/thing/piece");
    std::fs::create_dir_all(folder.join("src")).unwrap();
    std::fs::write(folder.join("src/lib.rs"), "").unwrap();
    let found = serde_json::json!({ "Ok": { "Root": { "base": folder, "refs": [] } } });
    let (_answer, reader) = reader_answering(&found.to_string());

    assert_eq!(
        path_text(&reader, &plane, PIECE, "src/lib.rs", false),
        Ok("src/lib.rs".to_string())
    );
    assert_eq!(
        path_text(&reader, &plane, PIECE, "src/lib.rs", true),
        Ok(folder.join("src/lib.rs").display().to_string())
    );
    assert_eq!(
        placed_of(&reader, &plane, PIECE, "src").map(|placed| placed.absolute),
        Ok(folder.join("src"))
    );
    assert_eq!(
        shell_folder_by(&reader, &plane, PIECE, "src"),
        Ok(folder.join("src"))
    );
    assert_eq!(shell_folder_by(&reader, &plane, PIECE, ""), Ok(folder));
}

const PIECE: Branch<'static> = Branch::piece("alpha", "thing", "piece");

/// Issue 1916: the plane root's two commands are the window's alone, so no link is served
/// the project's own files; and the commands a link is served name a branch through
/// [`branch`], which no names make the plane root.
#[test]
fn the_plane_roots_files_are_the_windows_alone_and_no_names_reach_them() {
    for command in ["plane_tree", "plane_file"] {
        assert!(
            purlis_session_protocol::ui::WINDOW_ONLY.contains(&command),
            "{command}"
        );
    }
    assert!(!branch("", "", &None).is_plane_root());
    assert!(!branch("", "", &Some(String::new())).is_plane_root());
    // Only the folder watch, which hands back nothing a folder holds, names it so.
    assert!(watched_place("", "", &None).is_plane_root());
    assert!(!watched_place("alpha", "", &None).is_plane_root());
    assert!(!watched_place("", "thing", &None).is_plane_root());
    assert!(!watched_place("", "", &Some("piece".into())).is_plane_root());
    assert!(!watched_place("alpha", "thing", &None).is_plane_root());
}

/// Issue 1916 (ruling F1): what the plane root's commands read is refused by the core for
/// what purlis keeps to itself, in its sentence, before anything is asked of git — the
/// reader here answers every question with the project's folder and nothing else.
#[cfg(unix)]
#[test]
fn the_plane_roots_commands_refuse_what_purlis_keeps_in_the_cores_sentence() {
    let dir = tempfile::tempdir().unwrap();
    let plane = std::fs::canonicalize(dir.path()).unwrap();
    let state = purlis_core::names::STATE_DIR.spellings().next().unwrap();
    std::fs::create_dir_all(plane.join(state).join("vaults")).unwrap();
    std::fs::write(plane.join(state).join("vaults/forge.json"), "a vault").unwrap();
    std::fs::create_dir_all(plane.join("workspaces/alpha/thing/.git")).unwrap();
    let found = serde_json::json!({ "Ok": { "Root": { "base": plane, "refs": [] } } });
    let (_answer, reader) = reader_answering(&found.to_string());
    let kept = "is one of the files purlis keeps to itself (vaults, keys and its own \
                    records), so it is not listed or opened here";

    let vault = format!("{state}/vaults/forge.json");
    assert_eq!(
        file_of(&reader, &plane, Branch::plane_root(), &vault),
        Err(format!("'{vault}' {kept}"))
    );
    let vaults = format!("{state}/vaults");
    assert_eq!(
        tree_of(&reader, &plane, Branch::plane_root(), &vaults),
        Err(format!("'{vaults}' {kept}"))
    );
    assert_eq!(
        tree_of(
            &reader,
            &plane,
            Branch::plane_root(),
            "workspaces/alpha/thing"
        ),
        Err(
            "'workspaces/alpha/thing' is in a repo of workspace 'alpha', whose files are on \
                 that workspace's tab"
                .to_string()
        )
    );
    assert_eq!(
        file_of(&reader, &plane, Branch::plane_root(), "../outside"),
        Err("'../outside' is not a path inside the branch's folder".to_string())
    );
}

/// A plane with a clone and one piece cut from it.
fn plane() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
    let clone = root.join("workspaces/alpha/thing");
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q", "-b", "main", "."]);
    std::fs::write(clone.join("README.md"), "one\n").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "one"]);
    let piece = purlis_core::worktree::add(&root, "alpha", "thing", "piece", None)
        .unwrap()
        .path;
    (dir, root, piece)
}

#[test]
fn a_pieces_file_crosses_to_the_window() {
    let (_dir, root, _piece) = plane();

    let file = file_of(&crate::reader(), &root, PIECE, "README.md").unwrap();

    assert_eq!(
        file,
        PieceFile::Text {
            text: "one\n".to_string()
        }
    );
}

#[test]
fn an_image_crosses_to_the_window_as_its_type_and_base64() {
    let (_dir, root, piece) = plane();
    std::fs::write(piece.join("dot.gif"), b"GIF89a\x01\x00\x01\x00\x00\x00\x00").unwrap();

    let file = file_of(&crate::reader(), &root, PIECE, "dot.gif").unwrap();

    assert_eq!(
        file,
        PieceFile::Image {
            mime: "image/gif".to_string(),
            base64: "R0lGODlhAQABAAAAAA==".to_string(),
        }
    );
}

#[test]
fn the_editor_the_window_names_reaches_the_core_as_a_url_for_that_editor() {
    let (_dir, root, piece) = plane();
    let none = |_: &str| None;
    let file = std::fs::canonicalize(piece).unwrap().join("README.md");

    let vscode = launch_of(
        &crate::reader(),
        &root,
        PIECE,
        "README.md",
        4,
        YourEditor::Vscode,
        &none,
    );
    let refused = launch_of(
        &crate::reader(),
        &root,
        PIECE,
        "../x",
        4,
        YourEditor::Zed,
        &none,
    );

    assert_eq!(
        vscode,
        Ok(Launch::Url(format!("vscode://file{}:4:1", file.display())))
    );
    assert_eq!(
        refused,
        Err("'../x' is not a path inside the branch's folder".to_string())
    );
}

#[test]
fn a_folder_of_a_branch_crosses_one_level_deep_with_each_entrys_kind() {
    let (_dir, root, piece) = plane();
    std::fs::create_dir_all(piece.join("src")).unwrap();
    std::fs::write(piece.join("src/lib.rs"), "\n").unwrap();

    // The worktree's own `.git` is there too, and hidden with what git ignores.
    let top: Vec<FolderEntry> = tree_of(&crate::reader(), &root, PIECE, "")
        .unwrap()
        .entries
        .into_iter()
        .filter(|one| !one.ignored)
        .collect();
    let repo = tree_of(&crate::reader(), &root, branch("alpha", "thing", &None), "").unwrap();

    assert_eq!(
        top,
        [
            FolderEntry {
                name: "src".to_string(),
                kind: EntryKind::Folder,
                ignored: false,
                refused: None,
                workspace: None,
            },
            FolderEntry {
                name: "README.md".to_string(),
                kind: EntryKind::File,
                ignored: false,
                refused: None,
                workspace: None,
            },
        ]
    );
    assert!(
        repo.entries.iter().any(|one| one.name == "README.md"),
        "{repo:?}"
    );
    assert_eq!(
        tree_of(&crate::reader(), &root, PIECE, ".."),
        Err("'..' is not a path inside the branch's folder".to_string())
    );
}

#[test]
fn what_a_branch_changed_crosses_with_each_files_mark_and_its_folders() {
    let (_dir, root, piece) = plane();
    std::fs::create_dir_all(piece.join("src")).unwrap();
    std::fs::write(piece.join("src/lib.rs"), "\n").unwrap();

    let status = status_of(&root, PIECE).unwrap();

    assert_eq!(
        status.changes,
        [FileChange {
            path: "src/lib.rs".to_string(),
            mark: ChangeMark::Added,
            from: None,
            uncommitted: true,
        }]
    );
    assert!(
        status
            .folders
            .iter()
            .any(|one| one.folder == "src" && one.mark == ChangeMark::Added && one.count == 1),
        "{status:?}"
    );
    assert_eq!(status.base.as_deref(), Some("main"));
}

/// #704: a path the engine names and never reads crosses as a row of its own, with its mark
/// and the core's sentence for why, and never among the files that open.
#[cfg(unix)]
#[test]
fn a_refused_path_crosses_with_its_mark_and_why_and_never_among_the_files() {
    let nowhere = tempfile::tempdir().unwrap();
    let answer = serde_json::json!({ "Ok": { "Status": {
            "changes": [{ "path": "a.txt", "mark": "Changed", "from": null, "uncommitted": true }],
            "folders": [],
            "refused": [
                { "path": "vendor/.GIT/x", "mark": "Added", "why": "GitsOwnFolder",
                  "uncommitted": false },
                { "path": "a\\b.txt", "mark": "Deleted", "why": "NotAPlainPath",
                  "uncommitted": true },
            ],
            "more": 0,
            "base": "main",
        } } });
    let (_answer, reader) = reader_answering(&answer.to_string());

    let status = status_by(&reader, nowhere.path(), PIECE).unwrap();

    assert_eq!(
        status.refused,
        [
            RefusedFile {
                path: "vendor/.GIT/x".to_string(),
                mark: ChangeMark::Added,
                why: files::RefusedFor::GitsOwnFolder.said().to_string(),
                uncommitted: false,
            },
            RefusedFile {
                path: "a\\b.txt".to_string(),
                mark: ChangeMark::Deleted,
                why: files::RefusedFor::NotAPlainPath.said().to_string(),
                uncommitted: true,
            },
        ]
    );
    assert_eq!(
        status
            .changes
            .iter()
            .map(|one| one.path.as_str())
            .collect::<Vec<_>>(),
        ["a.txt"]
    );
}

#[test]
fn what_changed_in_a_file_crosses_with_both_sides_and_gits_hunks() {
    let (_dir, root, piece) = plane();
    std::fs::write(piece.join("README.md"), "one\ntwo\n").unwrap();

    let shown = what_changed_of(&root, PIECE, "README.md").unwrap();

    assert_eq!(
        shown,
        WhatChanged {
            mark: ChangeMark::Changed,
            from: None,
            uncommitted: true,
            base: Some("main".to_string()),
            diff: FileDiff::Text {
                base: "one\n".to_string(),
                head: "one\ntwo\n".to_string(),
                hunks: vec![GitHunk {
                    old_start: 1,
                    old_lines: 0,
                    new_start: 2,
                    new_lines: 1,
                }],
            },
        }
    );
}

#[test]
fn what_changed_refuses_in_the_cores_sentence() {
    let (_dir, root, piece) = plane();
    std::fs::write(piece.join("other.txt"), "\n").unwrap();

    assert_eq!(
        what_changed_of(&root, PIECE, "README.md"),
        Err("'README.md' is not a file this branch changed against its base".to_string())
    );
    assert_eq!(
        what_changed_of(&root, PIECE, ".git/config"),
        Err("'.git/config' is not a path inside the branch's folder".to_string())
    );
}

#[test]
fn how_far_a_branch_is_from_its_base_crosses_with_the_base_named() {
    let (_dir, root, piece) = plane();
    std::fs::write(piece.join("new.txt"), "\n").unwrap();
    git(&piece, &["add", "-A"]);
    git(&piece, &["commit", "-q", "-m", "on the branch"]);

    let apart = ahead_behind_of(&root, PIECE).unwrap();

    assert_eq!(
        apart,
        AheadBehind {
            ahead: 1,
            behind: 0,
            base: Some("main".to_string()),
        }
    );
}

#[test]
fn the_editors_cross_from_the_window_by_their_wire_names() {
    let named: Vec<YourEditor> =
        serde_json::from_str(r#"["vscode", "zed", "idea", "variable"]"#).unwrap();

    assert_eq!(
        named,
        [
            YourEditor::Vscode,
            YourEditor::Zed,
            YourEditor::Idea,
            YourEditor::Variable
        ]
    );
}

#[test]
fn copy_path_is_the_path_in_the_branch_or_the_one_the_core_resolved() {
    let (_dir, root, piece) = plane();
    std::fs::create_dir_all(piece.join("src")).unwrap();
    std::fs::write(piece.join("src/lib.rs"), "\n").unwrap();
    let resolved = std::fs::canonicalize(&piece).unwrap();

    let relative = path_text(&crate::reader(), &root, PIECE, "./src/lib.rs", false);
    let absolute = path_text(&crate::reader(), &root, PIECE, "src", true);

    assert_eq!(relative, Ok("src/lib.rs".to_string()));
    assert_eq!(absolute, Ok(resolved.join("src").display().to_string()));
}

/// #1143: the branch's own folder, named by the empty path, is copied and revealed whole;
/// its relative path means nothing, so it is refused as the core refuses it.
#[test]
fn the_branchs_own_folder_is_copied_and_revealed_by_the_empty_path() {
    let (_dir, root, piece) = plane();
    let resolved = std::fs::canonicalize(&piece).unwrap();

    assert_eq!(
        path_text(&crate::reader(), &root, PIECE, "", true),
        Ok(resolved.display().to_string())
    );
    assert_eq!(
        placed_of(&crate::reader(), &root, PIECE, "").map(|placed| placed.absolute),
        Ok(resolved)
    );
    assert_eq!(
        path_text(&crate::reader(), &root, PIECE, "", false),
        Err("'' is not a path inside the branch's folder".to_string())
    );
}

#[cfg(unix)]
#[test]
fn a_links_relative_path_is_copied_and_its_absolute_path_and_reveal_are_refused() {
    let (_dir, root, piece) = plane();
    std::os::unix::fs::symlink(piece.join("README.md"), piece.join("CLAUDE.md")).unwrap();
    let refused = "'CLAUDE.md' is not a path inside the branch's folder".to_string();

    assert_eq!(
        path_text(&crate::reader(), &root, PIECE, "CLAUDE.md", false),
        Ok("CLAUDE.md".to_string())
    );
    assert_eq!(
        path_text(&crate::reader(), &root, PIECE, "CLAUDE.md", true),
        Err(refused.clone())
    );
    assert_eq!(
        placed_of(&crate::reader(), &root, PIECE, "CLAUDE.md").map(|placed| placed.absolute),
        Err(refused)
    );
}

#[test]
fn a_path_outside_the_branch_is_refused_before_it_is_copied_or_revealed() {
    let (_dir, root, _piece) = plane();

    assert_eq!(
        path_text(&crate::reader(), &root, PIECE, "../../charter.toml", true),
        Err("'../../charter.toml' is not a path inside the branch's folder".to_string())
    );
    assert_eq!(
        placed_of(&crate::reader(), &root, PIECE, ".git").map(|placed| placed.absolute),
        Err("'.git' is not a path inside the branch's folder".to_string())
    );
}

#[cfg(unix)]
#[test]
fn a_shell_starts_only_in_a_folder_of_the_branch_reached_through_no_link() {
    let (_dir, root, piece) = plane();
    std::fs::create_dir_all(piece.join("src")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), piece.join("out")).unwrap();
    let resolved = std::fs::canonicalize(&piece).unwrap();

    assert_eq!(shell_folder(&root, PIECE, "src"), Ok(resolved.join("src")));
    assert_eq!(shell_folder(&root, PIECE, ""), Ok(resolved));
    assert_eq!(
        shell_folder(&root, PIECE, "out"),
        Err("'out' is not a path inside the branch's folder".to_string())
    );
    assert_eq!(
        shell_folder(&root, PIECE, "README.md"),
        Err("'README.md' is not a folder".to_string())
    );
    assert_eq!(
        shell_folder(&root, PIECE, "../.."),
        Err("'../..' is not a path inside the branch's folder".to_string())
    );
}

#[test]
fn a_refusal_crosses_as_the_cores_sentence() {
    let (_dir, root, _piece) = plane();

    let refused = file_of(&crate::reader(), &root, PIECE, "../../README.md").unwrap_err();

    assert_eq!(
        refused,
        "'../../README.md' is not a path inside the branch's folder"
    );
}
