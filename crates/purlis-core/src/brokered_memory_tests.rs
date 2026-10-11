//! The memory verbs a chat asks the app to make (#1408): forget, edit, archive, unarchive,
//! move, optimize and a todo's close, each in the chat's own stores only.

use super::*;
use crate::repocmd::Say;

fn now() -> chrono::NaiveDateTime {
    "2026-10-06T12:00:00".parse().expect("a stamp")
}

/// A project with workspaces `alpha` and `beta` and personas `steward` and `devops`, each with
/// one memory, all four under the same title.
fn a_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("project");
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(root.join("workspaces").join(ws)).unwrap();
    }
    for persona in ["steward", "devops"] {
        std::fs::create_dir_all(root.join("personas").join(persona)).unwrap();
        std::fs::write(
            root.join("personas").join(persona).join("persona.md"),
            format!("---\nname: {persona}\n---\n# {persona}\n"),
        )
        .unwrap();
    }
    (dir, root)
}

fn chat_in_alpha_as(persona: Option<&str>) -> Asker {
    Asker {
        chat: 7,
        place: Place::Workspace("alpha".to_owned()),
        persona: persona.map(str::to_owned),
    }
}

/// One memory of persona `owner` (or `_shared`): its slug.
fn a_persona_memory(root: &Path, owner: &str, text: &str) -> String {
    let path = remember_persona(root, owner, text, None, now()).expect("remembered");
    path.file_stem().unwrap().to_string_lossy().into_owned()
}

/// One memory of workspace `ws`: its slug.
fn a_workspace_memory(root: &Path, ws: &str, text: &str) -> String {
    let path = crate::workspaces::Plane::open(root)
        .workspace(ws)
        .unwrap()
        .remember(text, now())
        .expect("remembered");
    path.file_stem().unwrap().to_string_lossy().into_owned()
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.path().is_file())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|name| name != crate::memstore::INDEX)
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// What the command said, and its exit code.
fn said(root: &Path, asker: &Asker, write: &Write) -> (Vec<Say>, u8) {
    match answer(root, asker, write, now()).expect("answered") {
        Done::Said { lines, code } => (lines, code),
        Done::Written(written) => panic!("{write:?} answered as one file: {written:?}"),
    }
}

fn joined(lines: &[Say]) -> String {
    lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_forget_takes_the_memory_from_the_chats_own_persona_and_no_other() {
    let (_dir, root) = a_project();
    let mine = a_persona_memory(&root, "steward", "The forge is self-hosted");
    let theirs = a_persona_memory(&root, "devops", "The forge is self-hosted");
    assert_eq!(mine, theirs, "the same slug in both stores");

    let (lines, code) = said(
        &root,
        &chat_in_alpha_as(Some("steward")),
        &Write::Forget {
            of: Store::Persona,
            slug: mine.clone(),
        },
    );

    assert_eq!(code, 0, "{lines:?}");
    assert_eq!(
        lines,
        vec![Say::Done(format!(
            "Forgot '{mine}' from steward's persistent memory."
        ))]
    );
    assert!(files_in(&root.join("personas/steward/memory")).is_empty());
    assert_eq!(
        files_in(&root.join("personas/devops/memory")),
        vec![format!("{theirs}.md")],
        "another persona's memory was touched"
    );
}

#[test]
fn a_chat_on_no_persona_has_no_persona_store_to_change() {
    let (_dir, root) = a_project();
    let slug = a_persona_memory(&root, "steward", "Kept");
    for of in [Store::Persona, Store::Shared] {
        let why = answer(
            &root,
            &chat_in_alpha_as(None),
            &Write::Forget {
                of,
                slug: slug.clone(),
            },
            now(),
        )
        .expect_err("refused");
        assert!(why.contains("runs as no persona"), "{why}");
    }
    assert_eq!(files_in(&root.join("personas/steward/memory")).len(), 1);
}

#[test]
fn a_workspace_memory_is_edited_archived_and_restored_in_the_chats_own_workspace() {
    let (_dir, root) = a_project();
    let slug = a_workspace_memory(&root, "alpha", "Ship on Fridays");
    let beta = a_workspace_memory(&root, "beta", "Ship on Fridays");
    let asker = chat_in_alpha_as(None);

    let (lines, code) = said(
        &root,
        &asker,
        &Write::EditMemory {
            of: Store::Workspace,
            slug: slug.clone(),
            title: None,
            text: Some("Ship on Thursdays".to_owned()),
        },
    );
    assert_eq!(code, 0, "{lines:?}");
    assert!(
        joined(&lines).starts_with(&format!("✓ Edited '{slug}' in workspace 'alpha' → ")),
        "{lines:?}"
    );
    let file = root.join(format!("workspaces/alpha/memory/{slug}.md"));
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("Thursdays")
    );

    let (lines, code) = said(
        &root,
        &asker,
        &Write::ArchiveMemory {
            of: Store::Workspace,
            slug: slug.clone(),
        },
    );
    assert_eq!(code, 0, "{lines:?}");
    assert!(joined(&lines).contains("Undo: `purlis workspace unarchive"));
    assert!(!file.exists());

    let (lines, code) = said(
        &root,
        &asker,
        &Write::UnarchiveMemory {
            of: Store::Workspace,
            slug: slug.clone(),
            restore_as: None,
        },
    );
    assert_eq!(code, 0, "{lines:?}");
    assert!(file.exists());
    assert_eq!(
        files_in(&root.join("workspaces/beta/memory")),
        vec![format!("{beta}.md")]
    );
}

#[test]
fn a_memory_moves_between_the_chats_own_stores() {
    let (_dir, root) = a_project();
    let slug = a_workspace_memory(&root, "alpha", "A fact the persona should keep");

    let (lines, code) = said(
        &root,
        &chat_in_alpha_as(Some("steward")),
        &Write::MoveMemory {
            from: Store::Workspace,
            slug,
            to: Store::Persona,
        },
    );

    assert_eq!(code, 0, "{lines:?}");
    assert!(
        joined(&lines).contains("from workspace 'alpha' to persona 'steward'"),
        "{lines:?}"
    );
    assert!(files_in(&root.join("workspaces/alpha/memory")).is_empty());
    assert_eq!(files_in(&root.join("personas/steward/memory")).len(), 1);
    assert!(files_in(&root.join("personas/devops/memory")).is_empty());
}

#[test]
fn a_verb_that_finds_nothing_says_so_and_exits_one() {
    let (_dir, root) = a_project();
    let (lines, code) = said(
        &root,
        &chat_in_alpha_as(None),
        &Write::Forget {
            of: Store::Workspace,
            slug: "nothing-like-it".to_owned(),
        },
    );
    assert_eq!(code, 1);
    assert_eq!(
        lines,
        vec![Say::Fail(
            "no memory 'nothing-like-it' in workspace 'alpha' (list them: purlis workspace \
             recall)."
                .to_owned()
        )]
    );
}

#[test]
fn a_todo_is_closed_in_the_chats_own_workspace_with_its_journal_entry() {
    let (_dir, root) = a_project();
    let ws = crate::workspaces::Plane::open(&root)
        .workspace("alpha")
        .unwrap();
    let todo = ws.record_todo("Port the picker", now()).expect("a todo");
    let slug = todo.file_stem().unwrap().to_string_lossy().into_owned();

    let (lines, code) = said(
        &root,
        &chat_in_alpha_as(None),
        &Write::TodoClose {
            how: crate::wscmd::todo::Close::Done,
            slug,
        },
    );

    assert_eq!(code, 0, "{lines:?}");
    let text = joined(&lines);
    assert!(text.contains("Remembered in 'alpha'"), "{text}");
    assert!(
        text.contains("Closed 'Port the picker' in 'alpha' — the journal has the trace."),
        "{text}"
    );
    assert!(files_in(&root.join("workspaces/alpha/todos")).is_empty());
    assert_eq!(files_in(&root.join("workspaces/alpha/memory")).len(), 1);
}

#[test]
fn a_dropped_todo_journals_nothing() {
    let (_dir, root) = a_project();
    let ws = crate::workspaces::Plane::open(&root)
        .workspace("alpha")
        .unwrap();
    let todo = ws.record_todo("Port the picker", now()).expect("a todo");
    let slug = todo.file_stem().unwrap().to_string_lossy().into_owned();

    let (lines, code) = said(
        &root,
        &chat_in_alpha_as(None),
        &Write::TodoClose {
            how: crate::wscmd::todo::Close::Forget,
            slug,
        },
    );

    assert_eq!(code, 0, "{lines:?}");
    assert!(joined(&lines).contains("Dropped 'Port the picker' from 'alpha'"));
    assert!(files_in(&root.join("workspaces/alpha/todos")).is_empty());
    assert!(files_in(&root.join("workspaces/alpha/memory")).is_empty());
}

#[test]
fn a_chat_at_the_project_root_has_no_workspace_store_or_todo_to_change() {
    let (_dir, root) = a_project();
    let asker = Asker {
        chat: 7,
        place: Place::PlaneRoot,
        persona: Some("steward".to_owned()),
    };
    for write in [
        Write::Forget {
            of: Store::Workspace,
            slug: "x".to_owned(),
        },
        Write::WorkspaceOptimize { stale_days: 30 },
        Write::TodoClose {
            how: crate::wscmd::todo::Close::Forget,
            slug: "x".to_owned(),
        },
    ] {
        let why = answer(&root, &asker, &write, now()).expect_err("refused");
        assert!(why.contains("not in a workspace"), "{write:?}: {why}");
    }
}

#[test]
fn no_memory_verb_changes_a_project_this_purlis_may_not_write() {
    let (_dir, root) = a_project();
    let slug = a_workspace_memory(&root, "alpha", "A fact");
    std::fs::write(
        root.join(crate::plane::MANIFEST),
        "schema = 2\nrequires = [{ feature = \"from-the-future\", since = \"9.0.0\" }]\n",
    )
    .unwrap();
    let file = root.join(format!("workspaces/alpha/memory/{slug}.md"));
    for write in [
        Write::Forget {
            of: Store::Workspace,
            slug: slug.clone(),
        },
        Write::ArchiveMemory {
            of: Store::Workspace,
            slug: slug.clone(),
        },
        Write::MoveMemory {
            from: Store::Workspace,
            slug: slug.clone(),
            to: Store::Shared,
        },
        Write::WorkspaceOptimize { stale_days: 0 },
    ] {
        let why =
            answer(&root, &chat_in_alpha_as(Some("steward")), &write, now()).expect_err("refused");
        assert!(why.contains("from-the-future"), "{write:?}: {why}");
        assert!(file.is_file(), "{write:?} changed it");
    }
}

#[cfg(unix)]
#[test]
fn a_store_that_leads_into_git_is_refused_and_nothing_there_is_touched() {
    let (_dir, root) = a_project();
    std::fs::create_dir_all(root.join(".git/info")).unwrap();
    std::fs::write(root.join(".git/info/exclude.md"), "kept").unwrap();
    std::os::unix::fs::symlink(root.join(".git/info"), root.join("workspaces/alpha/memory"))
        .unwrap();

    let why = answer(
        &root,
        &chat_in_alpha_as(None),
        &Write::Forget {
            of: Store::Workspace,
            slug: "exclude".to_owned(),
        },
        now(),
    )
    .expect_err("refused");

    assert!(why.contains("not purlis's to write for a chat"), "{why}");
    assert!(root.join(".git/info/exclude.md").is_file());
}

#[test]
fn a_memory_verb_reads_off_the_wire_naming_no_workspace_and_no_persona() {
    let write = Write::MoveMemory {
        from: Store::Persona,
        slug: "the-forge".to_owned(),
        to: Store::Shared,
    };
    let text = serde_json::to_string(&write).expect("json");
    assert_eq!(
        text,
        r#"{"op":"move_memory","from":"persona","slug":"the-forge","to":"shared"}"#
    );
    assert_eq!(
        serde_json::from_str::<Write>(&text).expect("a write"),
        write
    );
    // A name where a store goes is not a store: the line is not read.
    assert!(
        serde_json::from_str::<Write>(
            r#"{"op":"forget","of":{"persona":"devops"},"slug":"the-forge"}"#
        )
        .is_err()
    );
}

#[test]
fn a_slug_too_long_to_carry_is_refused_saying_so() {
    let write = Write::Forget {
        of: Store::Workspace,
        slug: "s".repeat(MOST_TEXT_BYTES + 1),
    };
    let why = write.check().expect_err("refused");
    assert!(why.contains("make it shorter"), "{why}");
}

#[test]
fn every_memory_verb_that_changed_a_store_is_credited_to_the_chat() {
    let (_dir, root) = a_project();
    let slug = a_persona_memory(&root, "steward", "Credited");
    said(
        &root,
        &chat_in_alpha_as(Some("steward")),
        &Write::ArchiveMemory {
            of: Store::Persona,
            slug,
        },
    );
    let trace = std::fs::read_to_string(crate::trace::file(&root, "7")).expect("chat 7's trace");
    assert!(trace.contains("archive_memory"), "{trace}");
    assert!(trace.contains("personas/steward/memory"), "{trace}");
}

#[test]
fn a_memory_verb_is_never_made_where_one_file_is_expected() {
    let (_dir, root) = a_project();
    let slug = a_workspace_memory(&root, "alpha", "Kept");
    let why = perform(
        &root,
        &chat_in_alpha_as(None),
        &Write::Forget {
            of: Store::Workspace,
            slug: slug.clone(),
        },
        now(),
    )
    .expect_err("refused");
    assert!(why.contains("nothing was written"), "{why}");
    assert_eq!(files_in(&root.join("workspaces/alpha/memory")).len(), 1);
}
