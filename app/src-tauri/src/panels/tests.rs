// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::path::PathBuf;

/// A plane with one workspace holding one clone, and nothing fetched for it.
fn plane_with_a_clone() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a plane");
    let root = std::fs::canonicalize(dir.path()).expect("a resolved plane");
    std::fs::write(root.join("charter.toml"), "").expect("a manifest");
    std::fs::create_dir_all(root.join("workspaces/alpha/svc/.git")).expect("a clone");
    std::fs::write(
        root.join("workspaces/alpha/svc/.git/HEAD"),
        "ref: refs/heads/main\n",
    )
    .expect("a HEAD");
    (dir, root)
}

// ---- served from the plane's model (FD-10c) ---------------------------------------------

/// A held plane, watched, whose `alpha` has the todo `Alpha one` and the memory
/// `Memory one`.
fn held_plane() -> (
    tempfile::TempDir,
    crate::planes::Planes,
    std::sync::Arc<crate::planes::Held>,
) {
    held_plane_at(tempfile::tempdir().expect("a plane"))
}

/// [`held_plane`] in `dir`, whatever is already in it.
fn held_plane_at(
    dir: tempfile::TempDir,
) -> (
    tempfile::TempDir,
    crate::planes::Planes,
    std::sync::Arc<crate::planes::Held>,
) {
    let root = std::fs::canonicalize(dir.path()).expect("a resolved plane");
    std::fs::write(root.join("charter.toml"), "").expect("a manifest");
    for (store, slug, title) in [("todos", "a1", "Alpha one"), ("memory", "m1", "Memory one")] {
        let store = root.join("workspaces/alpha").join(store);
        std::fs::create_dir_all(&store).expect("a store");
        std::fs::write(store.join(format!("{slug}.md")), format!("# {title}\n")).expect("it");
    }
    let planes = crate::planes::Planes::telling(
        std::sync::Arc::new(|_: crate::hooks::Moved| {}),
        crate::Shipped::default(),
        None,
    );
    let id = planes.open(&root);
    let held = planes.held(&id).expect("held");
    (dir, planes, held)
}

/// The titles of the panel `id` that `panels` contributes.
fn rows_of(panels: &Panels, id: &str) -> Vec<String> {
    let read = serde_json::to_value(panels).expect("serialisable");
    read["contributed"]
        .as_array()
        .expect("contributed")
        .iter()
        .find(|panel| panel["key"].as_str().is_some_and(|key| key.ends_with(id)))
        .and_then(|panel| panel["blocks"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|block| block["rows"].as_array())
        .flatten()
        .map(|row| row["text"].as_str().expect("a row's text").to_owned())
        .collect()
}

#[test]
fn the_panels_a_watched_plane_serves_are_the_panels_read_from_the_disk() {
    let (_dir, _planes, held) = held_plane();
    let served = served(&held, "alpha").expect("served");
    let read = of(held.root(), "alpha").expect("read");
    assert_eq!(
        serde_json::to_value(&served).expect("serialisable"),
        serde_json::to_value(&read).expect("serialisable")
    );
    assert_eq!(rows_of(&served, "memory"), ["Memory one"]);
}

#[test]
fn a_memory_the_window_saves_is_in_the_panels_it_reads_straight_after() {
    // The window writes and reads its panels at once; the watch's batch arrives a quarter
    // of a second later, or on a busy Mac seconds later. What the window wrote is told to
    // the model with the write, so the read straight after already has it.
    let (_dir, _planes, held) = held_plane();
    served(&held, "alpha").expect("served once, so the model holds alpha");
    let store = held.root().join("workspaces/alpha/memory");
    std::fs::write(store.join("m2.md"), "# Memory two\n").expect("a memory");

    held.wrote(&["workspaces/alpha/memory".to_owned()]);

    assert_eq!(
        rows_of(&served(&held, "alpha").expect("served"), "memory"),
        ["Memory two", "Memory one"]
    );
}

#[test]
fn a_first_ask_reads_the_workspace_with_the_models_lock_let_go() {
    // #1160: making or removing a workspace reads the model again on the thread that
    // draws the window, and must not wait there for another workspace's first read.
    let (_dir, _planes, held) = held_plane();
    let moving = std::sync::Arc::clone(&held);

    let panels = served_reading(&held, "alpha", |root, workspace| {
        let (done, moved) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            moving.workspaces_moved();
            let _ = done.send(());
        });
        moved
            .recv_timeout(std::time::Duration::from_secs(60))
            .expect("the model's lock is free while the workspace is read");
        Sections::read(root, workspace)
    })
    .expect("served");

    assert_eq!(rows_of(&panels, "memory"), ["Memory one"]);
    // The model was read again whole under the read, which is therefore not held: a
    // memory written since is in the next answer, with nothing having told the model.
    let store = held.root().join("workspaces/alpha/memory");
    std::fs::write(store.join("m2.md"), "# Memory two\n").expect("a memory");
    assert_eq!(
        rows_of(&served(&held, "alpha").expect("served"), "memory"),
        ["Memory two", "Memory one"]
    );
}

#[test]
fn a_store_the_watch_could_not_watch_is_read_from_the_disk_on_every_ask() {
    // A failed registration must never leave the panels serving what the model held.
    let dir = tempfile::tempdir().expect("a plane");
    let root = std::fs::canonicalize(dir.path()).expect("a resolved plane");
    let memory = root.join("workspaces/alpha/memory");
    std::fs::create_dir_all(&memory).expect("a memory store");
    std::fs::write(memory.join("m1.md"), "# Memory one\n").expect("a memory");
    crate::watchset::refuse::refuse(&memory);
    let (_dir, _planes, held) = held_plane_at(dir);
    served(&held, "alpha").expect("served once");
    std::fs::write(memory.join("m2.md"), "# Memory two\n").expect("a memory");

    assert_eq!(
        rows_of(&served(&held, "alpha").expect("served"), "memory"),
        ["Memory two", "Memory one"]
    );
}

#[test]
fn a_todo_store_the_watch_could_not_watch_reaches_the_sidebar_on_the_next_ask() {
    let dir = tempfile::tempdir().expect("a plane");
    let root = std::fs::canonicalize(dir.path()).expect("a resolved plane");
    let todos = root.join("workspaces/alpha/todos");
    std::fs::create_dir_all(&todos).expect("a todo store");
    crate::watchset::refuse::refuse(&todos);
    let (_dir, _planes, held) = held_plane_at(dir);
    held.sidebar_model();
    std::fs::write(todos.join("a2.md"), "# Alpha two\n").expect("a todo");

    let model = held.sidebar_model();
    let todos: Vec<String> = model
        .rows()
        .expect("listed")
        .flat_map(|row| row.todos.clone())
        .collect();
    assert!(todos.contains(&"Alpha two".to_owned()), "{todos:?}");
}

#[test]
fn a_memory_saved_outside_the_window_reaches_the_panels_through_the_watch() {
    // No timing is asserted. The held plane's watch runs on notify's poller in these tests
    // on macOS (`planewatch::Platform`), since FSEvents has no delivery bound on a busy
    // Mac (#577, #756). The panels are asked again until they have it, for as long as a
    // loaded runner can take, and a passing run returns the moment they do.
    let (_dir, _planes, held) = held_plane();
    served(&held, "alpha").expect("served once, so the model holds alpha");
    let store = held.root().join("workspaces/alpha/memory");
    std::fs::write(store.join("m2.md"), "# Memory two\n").expect("a memory");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let rows = rows_of(&served(&held, "alpha").expect("served"), "memory");
        if rows == ["Memory two", "Memory one"] {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the memory never reached the panels: {rows:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// A stand-in `charter` that records how it was called and then ends at once.
fn stand_in(at: &Path) -> PathBuf {
    let binary = at.join("charter-stand-in");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf '%s %s\\n' \"$1\" \"$3\" >> \"$(dirname \"$0\")/ran\"\n",
    )
    .expect("the stand-in is written");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
        .expect("it is runnable");
    binary
}

/// Everything the stand-in has recorded so far, once it has recorded anything.
///
/// Waited for up to thirty seconds, which a passing test never spends: it returns the moment
/// the stand-in has written. The stand-in is a program written fresh for each test, and
/// macOS assesses a program file before its first run — on a loaded machine for seconds at a
/// time (#422) — so a shorter bound failed tests on how busy the machine was.
fn ran(at: &Path) -> String {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Ok(text) = std::fs::read_to_string(at.join("ran"))
            && !text.is_empty()
        {
            // Give a second line the chance to arrive, so "exactly one" is not merely
            // "the first one got there first".
            std::thread::sleep(std::time::Duration::from_millis(200));
            return std::fs::read_to_string(at.join("ran")).unwrap_or(text);
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the refresh never ran"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// A plane with one workspace holding one todo, and two personas — one of them the
/// plane's default, one of them with a memory.
fn plane_with_a_todo_and_two_personas() -> (tempfile::TempDir, PathBuf) {
    let (dir, root) = plane_with_a_clone();
    std::fs::write(
        root.join("charter.toml"),
        "[persona]\ndefault = \"steward\"\n",
    )
    .expect("a manifest naming a default");
    std::fs::create_dir_all(root.join("workspaces/alpha/todos")).expect("a todo store");
    std::fs::write(
        root.join("workspaces/alpha/todos/20260302-091400-review.md"),
        "# Review the rollout plan\n\n_2026-03-02 09:14 · todo_\n\nEvery step of it.\n",
    )
    .expect("a todo");
    for who in ["devops", "steward"] {
        std::fs::create_dir_all(root.join("personas").join(who).join("memory")).expect("a persona");
        std::fs::write(
            root.join("personas").join(who).join("persona.md"),
            format!("---\nrole: {who}\n---\n\n# {who}\n"),
        )
        .expect("a definition");
    }
    std::fs::write(
        root.join("personas/steward/memory/charter-defects-go-upstream.md"),
        "# Charter defects go upstream\n\n_2026-09-20 10:00 · durable_\n\nFile the issue.\n",
    )
    .expect("a memory");
    (dir, root)
}

#[test]
fn charters_own_panels_come_through_the_same_seam_an_extension_would_use() {
    // **The claim the whole change rests on.** Todos and personas used to be named fields
    // that hardcoded React read. They are `purlis_core::panel` values now, produced here
    // and drawn by the loop in `Panels.tsx` that draws a stranger's declared panel — so
    // what the window knows about a todo it learned from the vocabulary.
    let (_plane, root) = plane_with_a_todo_and_two_personas();

    let drawn = of(&root, "alpha").expect("the panels draw");

    let keys: Vec<&str> = drawn
        .contributed
        .iter()
        .map(|panel| panel.key.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "charter/todos",
            "charter/memory",
            "charter/personas",
            "charter/sessions"
        ]
    );
    assert!(
        drawn.contributed.iter().all(|panel| panel.from.is_none()),
        "purlis's own panels named an extension as their contributor"
    );
}

#[test]
fn a_todo_row_carries_its_whole_body_and_opens_the_todos_tab() {
    // The operator's own request — *"on clicking we should show full body"* — answered by
    // the todo's view tab now (#1214): the row is one line, runs the catalogue row that
    // opens the tab, and carries the body for the search.
    let (_plane, root) = plane_with_a_todo_and_two_personas();

    let drawn = of(&root, "alpha").expect("the panels draw");

    let rows = list_of(&drawn.contributed[0]);
    assert_eq!(rows[0].text, "Review the rollout plan");
    // The stamp as the file records it, which is the store's own reading and not this
    // panel's — a row that reformatted a date would be a second answer to what a todo says.
    assert_eq!(rows[0].note.as_deref(), Some("2026-03-02 09:14"));
    assert_eq!(
        rows[0].detail,
        Some(PanelDetail::Text {
            text: "Every step of it.".into()
        })
    );
    assert_eq!(
        rows[0].runs,
        Some(format!("todo.open:{}", rows[0].key)),
        "a todo row does not open the todo's tab"
    );
}

#[test]
fn a_persona_row_says_how_much_it_remembers_without_reading_a_memory() {
    // The count is a `read_dir` on the path that has 100 ms to draw; the memories
    // themselves are the persona's view, when a reader opens it. `0 memories` is said rather
    // than left off — a row that omits the count reads as one charter did not look at,
    // which is a different fact from a persona with none.
    let (_plane, root) = plane_with_a_todo_and_two_personas();

    let drawn = of(&root, "alpha").expect("the panels draw");

    let rows = list_of(panel_called(&drawn, "charter/personas"));
    let notes: Vec<Option<&str>> = rows.iter().map(|row| row.note.as_deref()).collect();
    assert_eq!(
        notes,
        [
            Some("0 memories"),
            Some("default · 1 memory"),
            Some("0 memories")
        ]
    );
    assert_eq!(rows[1].tone, "default");
    assert_eq!(rows[1].runs.as_deref(), Some("persona.show:steward"));
}

#[test]
fn a_contributed_row_could_never_carry_the_verb_a_persona_row_does() {
    // The asymmetry, pinned where it is created rather than only where it is refused.
    // `purlis_core::panel::declared` refuses `runs` from a manifest; this is the other
    // half — charter's own producer is the only thing that sets it, and it sets it to a
    // catalogue id the window looks up rather than to anything it made up.
    let (_plane, root) = plane_with_a_todo_and_two_personas();

    let drawn = of(&root, "alpha").expect("the panels draw");

    for row in list_of(&drawn.contributed[0]) {
        assert_eq!(
            row.runs,
            Some(format!("todo.open:{}", row.key)),
            "a todo row's verb is not the catalogue row that opens its view"
        );
    }
    for row in list_of(panel_called(&drawn, "charter/personas")) {
        if row.key == SHARED_ROW {
            continue;
        }
        assert!(
            row.runs
                .as_deref()
                .is_some_and(|id| id.starts_with("persona.show:")),
            "a persona row's verb is not the catalogue row that opens its view"
        );
    }
}

#[test]
fn a_memory_arrives_as_a_row_of_the_same_vocabulary_a_panel_is_drawn_from() {
    // **The second consumer of the list primitive**, and what makes it a primitive rather
    // than a panel with a general-sounding name: the window's search, bound and card work
    // on these without anything in it knowing what a memory is.
    let (_plane, root) = plane_with_a_todo_and_two_personas();

    let rows: Vec<PanelRow> = memory_rows(&root, "steward")
        .expect("the memories read")
        .iter()
        .map(PanelRow::from)
        .collect();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "Charter defects go upstream");
    assert_eq!(rows[0].mark, "note");
    // **It opens the memory's own tab** (SI-9b, ADR 0065), by the catalogue row named for
    // the memory's store and slug — and the body still travels, for the list's search.
    assert_eq!(
        rows[0].runs.as_deref(),
        Some("memory.open:persona/steward/charter-defects-go-upstream")
    );
    assert_eq!(
        rows[0].detail,
        Some(PanelDetail::Text {
            text: "File the issue.".into()
        })
    );
}

/// The panel `key` names, among what `of` drew.
fn panel_called<'a>(drawn: &'a Panels, key: &str) -> &'a PanelView {
    drawn
        .contributed
        .iter()
        .find(|panel| panel.key == key)
        .unwrap_or_else(|| panic!("no {key} panel"))
}

/// Writes a journal entry into `alpha`'s memory, as `charter ws remember` would.
fn journal(root: &Path, file: &str, title: &str, body: &str) {
    let dir = root.join("workspaces/alpha/memory");
    std::fs::create_dir_all(&dir).expect("a journal");
    std::fs::write(
        dir.join(file),
        format!("# {title}\n\n_2026-03-02 09:12 · note_\n\n{body}\n"),
    )
    .expect("a memory");
}

#[test]
fn the_focused_workspaces_memory_is_a_panel_between_todos_and_personas_newest_first() {
    // ADR 0065 Q5: directly under Todos, the focused workspace's, newest first — the
    // journal's filename stamp orders it oldest first, so the panel turns it round.
    let (_plane, root) = plane_with_a_todo_and_two_personas();
    journal(
        &root,
        "20260302-091200-the-api-returns-418.md",
        "The API returns 418",
        "On Mondays.",
    );
    journal(
        &root,
        "20260302-091500-closed-todo-write-it.md",
        "Closed todo: write it",
        "Done.",
    );

    let drawn = of(&root, "alpha").expect("the panels draw");

    let memory = panel_called(&drawn, "charter/memory");
    assert_eq!(memory.title, "Memory");
    let orders = |key: &str| panel_called(&drawn, key).order;
    assert!(orders("charter/todos") < memory.order && memory.order < orders("charter/personas"));
    let rows = list_of(memory);
    let texts: Vec<&str> = rows.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(texts, ["Closed todo: write it", "The API returns 418"]);
    // The rows a persona's tab lists, in the same style: opening the memory's own tab by
    // its store and slug, with the body along for the search.
    assert_eq!(
        rows[1].runs.as_deref(),
        Some("memory.open:workspace/alpha/20260302-091200-the-api-returns-418")
    );
    assert_eq!(rows[1].mark, "note");
    assert_eq!(
        rows[1].detail,
        Some(PanelDetail::Text {
            text: "On Mondays.".into()
        })
    );
}

#[test]
fn a_workspace_with_no_journal_says_nothing_is_remembered_rather_than_drawing_nothing() {
    let (_plane, root) = plane_with_a_todo_and_two_personas();

    let drawn = of(&root, "alpha").expect("the panels draw");

    let memory = panel_called(&drawn, "charter/memory");
    assert!(list_of(memory).is_empty());
    let empty = memory
        .blocks
        .iter()
        .find_map(|block| match block {
            PanelBlock::List { empty, .. } => Some(empty.headline.clone()),
            _ => None,
        })
        .expect("a list");
    assert_eq!(empty, "Nothing remembered yet");
}

#[test]
fn the_personas_panel_ends_with_one_shared_row_that_is_not_a_persona() {
    // ADR 0065 Q6: "shared · N memories", opening the shared store's own list. `_shared`
    // is not a persona, so the plane's persona list — what the palette's persona rows
    // are made from — still does not name it.
    let (_plane, root) = plane_with_a_todo_and_two_personas();
    let shared = root.join("personas/_shared/memory");
    std::fs::create_dir_all(&shared).expect("a shared store");
    std::fs::write(
        shared.join("the-plane-is-the-unit-of-work.md"),
        "# The plane is the unit of work\n\n_2026-09-20 10:00 · durable_\n\nYes.\n",
    )
    .expect("a shared memory");

    let drawn = of(&root, "alpha").expect("the panels draw");

    let rows = list_of(panel_called(&drawn, "charter/personas"));
    let last = rows.last().expect("rows");
    assert_eq!(last.key, SHARED_ROW);
    assert_eq!(last.text, "shared");
    assert_eq!(last.note.as_deref(), Some("1 memory"));
    assert_eq!(last.runs.as_deref(), Some("memory.shared"));
    assert_eq!(
        rows.iter().filter(|row| row.key == SHARED_ROW).count(),
        1,
        "{rows:?}"
    );
    assert!(!drawn.personas.iter().any(|name| name.starts_with('_')));
}

#[test]
fn a_plane_with_no_personas_and_nothing_shared_keeps_its_empty_personas_panel() {
    // The row would otherwise hide "No personas in this project" behind a store that holds
    // nothing either.
    let (_plane, root) = plane_with_a_clone();

    let drawn = of(&root, "alpha").expect("the panels draw");

    assert!(list_of(panel_called(&drawn, "charter/personas")).is_empty());
}

#[test]
fn the_shared_store_lists_as_memory_rows_keyed_by_the_shared_scope() {
    let (_plane, root) = plane_with_a_todo_and_two_personas();
    let shared = root.join("personas/_shared/memory");
    std::fs::create_dir_all(&shared).expect("a shared store");
    std::fs::write(
        shared.join("the-plane-is-the-unit-of-work.md"),
        "# The plane is the unit of work\n\n_2026-09-20 10:00 · durable_\n\nYes.\n",
    )
    .expect("a shared memory");

    let blocks = shared_memory_view(&root).expect("the shared store reads");

    let rows: Vec<PanelRow> = blocks
        .iter()
        .find_map(|block| match block {
            panel::Block::List { rows, .. } => Some(rows.iter().map(PanelRow::from).collect()),
            _ => None,
        })
        .expect("a list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "The plane is the unit of work");
    assert_eq!(
        rows[0].runs.as_deref(),
        Some("memory.open:shared/the-plane-is-the-unit-of-work")
    );
}

/// The rows the shared store's list draws, as the window receives them.
fn shared_rows(root: &Path) -> Vec<PanelRow> {
    shared_memory_view(root)
        .expect("the shared store reads")
        .iter()
        .find_map(|block| match block {
            panel::Block::List { rows, .. } => Some(rows.iter().map(PanelRow::from).collect()),
            _ => None,
        })
        .expect("a list")
}

#[test]
fn a_memory_file_made_by_hand_never_opens_as_a_new_memorys_tab() {
    let (_plane, root) = plane_with_a_todo_and_two_personas();
    let shared = root.join("personas/_shared/memory");
    std::fs::create_dir_all(&shared).expect("a shared store");
    std::fs::write(shared.join("+.md"), "# Plus\n\nBy hand.\n").expect("a memory by hand");

    let rows = shared_rows(&root);

    let draft = format!(
        "memory.open:{}",
        crate::memories::view_key(
            &crate::memories::MemoryScope::Shared,
            crate::memories::DRAFT
        )
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].runs.as_deref(), Some("memory.open:shared/+"));
    assert_ne!(rows[0].runs.as_deref(), Some(draft.as_str()));
}

#[cfg(unix)]
#[test]
fn a_memory_file_whose_name_no_operation_takes_is_listed_and_opens_nothing() {
    // `\` is a legal filename on a Unix plane and a slug the core refuses everywhere: its
    // row is drawn, so the file is not hidden, and runs nothing, so no row can ever carry
    // the key a new memory's tab is (`memories::DRAFT`).
    let (_plane, root) = plane_with_a_todo_and_two_personas();
    let shared = root.join("personas/_shared/memory");
    std::fs::create_dir_all(&shared).expect("a shared store");
    std::fs::write(
        shared.join(format!("{}.md", crate::memories::DRAFT)),
        "# Backslash\n\nBy hand.\n",
    )
    .expect("a file named for the draft slug");

    let rows = shared_rows(&root);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].text, "Backslash");
    assert_eq!(rows[0].runs, None);
}

#[test]
fn a_todo_store_charter_will_not_read_is_a_sentence_and_never_an_empty_list() {
    // "Nothing to do" is what an empty list would claim about a store charter refused, and
    // that claim is false in the direction that matters.
    let (_plane, root) = plane_with_a_clone();
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("the workspace");
    let outside = root.parent().expect("a parent").join("elsewhere");
    std::fs::create_dir_all(&outside).expect("somewhere outside");
    std::os::unix::fs::symlink(&outside, root.join("workspaces/alpha/todos"))
        .expect("a link out of the plane");

    let drawn = of(&root, "alpha").expect("the panels draw");

    let told = drawn.contributed[0]
        .blocks
        .iter()
        .find_map(|block| match block {
            PanelBlock::Note { text, tone } => Some((text.clone(), tone.clone())),
            PanelBlock::List { .. } | PanelBlock::Chart { .. } | PanelBlock::Facts { .. } => None,
        })
        .expect("the refusal is drawn");
    assert_eq!(told.1, "trouble");
    assert!(!told.0.is_empty());
}

#[test]
fn an_empty_todo_list_and_an_empty_personas_panel_each_say_how_to_add_one() {
    // The copy guide (`docs/ui-copy.md`): an empty state says what is true, then the way
    // out — never the storage it is read from.
    let (_plane, root) = plane_with_a_clone();
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("the workspace");

    let drawn = of(&root, "alpha").expect("the panels draw");

    let body = |id: &str| {
        panel_called(&drawn, id)
            .blocks
            .iter()
            .find_map(|block| match block {
                PanelBlock::List { empty, .. } => Some(empty.body.clone()),
                _ => None,
            })
            .expect("a list")
    };
    assert_eq!(
        body("charter/todos").as_deref(),
        Some("Add one in the box above.")
    );
    assert_eq!(
        body("charter/personas").as_deref(),
        Some("Make one with the + above, or New persona… in the palette.")
    );
}

/// The rows of a panel's one list block.
fn list_of(panel: &PanelView) -> &[PanelRow] {
    panel
        .blocks
        .iter()
        .find_map(|block| match block {
            PanelBlock::List { rows, .. } => Some(rows.as_slice()),
            PanelBlock::Note { .. } | PanelBlock::Chart { .. } | PanelBlock::Facts { .. } => None,
        })
        .expect("a list block")
}

#[test]
fn focusing_a_workspace_is_what_kicks_a_forge_refresh() {
    // **The wiring, and the whole of charter-app#69's visible half.** Until this, nothing
    // anywhere called the refresher: the CI column showed whatever the last
    // `charter gl-refresh` typed by hand had left. `glstate` holds the policy, and this is
    // the only thing that asks it — a user action, never a timer.
    let (_plane, root) = plane_with_a_clone();
    let beside = tempfile::tempdir().expect("somewhere for the stand-in");
    let binary = stand_in(beside.path());
    // The refresh is detached into a group of its own: killed with the test by the pid
    // the lock names, however the assertions below end (#923).
    let _ends = stand_in::Ends::named_in(purlis_core::glrefresh::lock(&root));

    let drawn = states_of(&root, "alpha", Some(&binary)).expect("the panel draws");

    assert_eq!(drawn.workspace, "alpha");
    assert_eq!(
        ran(beside.path()).trim(),
        "gl-refresh alpha",
        "the refresh was not asked for the workspace that was focused"
    );
}

#[test]
fn focusing_it_again_does_not_start_a_second_refresh() {
    // The cooldown reaching all the way out to the trigger: an operator clicking between
    // two workspaces, or a panel asked twice, is one forge process and not two.
    let (_plane, root) = plane_with_a_clone();
    let beside = tempfile::tempdir().expect("somewhere for the stand-in");
    let binary = stand_in(beside.path());
    // The refresh is detached into a group of its own: killed with the test by the pid
    // the lock names, however the assertions below end (#923).
    let _ends = stand_in::Ends::named_in(purlis_core::glrefresh::lock(&root));

    states_of(&root, "alpha", Some(&binary)).expect("the panel draws");
    let once = ran(beside.path());
    states_of(&root, "alpha", Some(&binary)).expect("the panel draws again");
    std::thread::sleep(std::time::Duration::from_millis(300));

    assert_eq!(once.lines().count(), 1, "the first focus ran {once:?}");
    assert_eq!(
        std::fs::read_to_string(beside.path().join("ran")).unwrap_or_default(),
        once,
        "the second focus started another refresh"
    );
}

#[test]
fn a_clone_comes_with_the_path_the_core_checked_so_the_window_never_joins_one() {
    // charter-app#174: a clone row can be picked as where the next chat starts only if the
    // window holds a path for it that the core spelled.
    let (_plane, root) = plane_with_a_clone();

    let drawn = of(&root, "alpha").expect("the panels draw");

    assert_eq!(drawn.repos, vec!["svc".to_string()]);
    assert_eq!(
        drawn.paths.get("svc").map(String::as_str),
        Some(
            root.join("workspaces/alpha/svc")
                .display()
                .to_string()
                .as_str()
        ),
    );
}

#[test]
fn a_panel_drawn_with_no_charter_beside_the_app_still_draws() {
    // Without a binary there is nothing to spawn, and a panel is not the place to say so:
    // the launch already said it once, and repeating it per focus is fifty lines an hour.
    let (_plane, root) = plane_with_a_clone();

    let drawn = states_of(&root, "alpha", None).expect("the panel draws");

    assert_eq!(drawn.repos.len(), 1);
}

// ---- the Sessions panel (SI-8d) ------------------------------------------------------

/// A record of `place`'s, written through the one writer, at 2026-09-28 `h`:00:00.
fn a_record(root: &Path, place: Place, title: &str, h: u32, conversation: Option<&str>) -> String {
    sessionrecord::record(
        root,
        &sessionrecord::New {
            title,
            body: "## Goal\n\ng\n\n## Done\n\nd\n\n## Decisions\n\nx\n\n## Open\n\no\n\n\
                   ## How to resume\n\nr\n",
            facts: &sessionrecord::Facts {
                place,
                at: chrono::NaiveDate::from_ymd_opt(2026, 9, 28)
                    .and_then(|d| d.and_hms_opt(h, 0, 0))
                    .expect("a time"),
                chat: Some(sessionrecord::ChatFacts {
                    number: 3,
                    name: None,
                    harness: Some("claude".to_owned()),
                    profile: None,
                    conversation: conversation.map(str::to_owned),
                    cwd: None,
                    unsandboxed: false,
                    identity: Default::default(),
                    handed_from: purlis_core::sessionrecord::Handed::No,
                }),
                persona: Some("steward".to_owned()),
                pieces: Vec::new(),
            },
        },
    )
    .expect("a record")
    .shown
}

#[test]
fn a_workspaces_sessions_panel_lists_its_records_newest_first_and_marks_the_resumable() {
    let (_plane, root) = plane_with_a_todo_and_two_personas();
    let alpha = || Place::Workspace("alpha".to_owned());
    a_record(&root, alpha(), "Older", 9, None);
    let newer = a_record(
        &root,
        alpha(),
        "Newer",
        10,
        Some("0f6c2a1e-aaaa-4bbb-8ccc-1234"),
    );

    let drawn = of(&root, "alpha").expect("the panels draw");

    let panel = drawn
        .contributed
        .iter()
        .find(|panel| panel.key == "charter/sessions")
        .expect("a Sessions panel");
    let rows = list_of(panel);
    let titles: Vec<&str> = rows.iter().map(|row| row.text.as_str()).collect();
    assert_eq!(titles, ["Newer", "Older"]);
    assert_eq!(
        rows[0].note.as_deref(),
        Some("2026-09-28 10:00 · steward · claude · ↻ resumable")
    );
    assert_eq!(
        rows[1].note.as_deref(),
        Some("2026-09-28 09:00 · steward · claude")
    );
    assert_eq!(rows[0].runs, Some(format!("session.open:{newer}")));
    let facts: Vec<(&str, bool)> = drawn
        .sessions
        .iter()
        .map(|row| (row.title.as_str(), row.resumable))
        .collect();
    assert_eq!(facts, [("Newer", true), ("Older", false)]);
}

#[test]
fn the_plane_root_draws_the_plane_roots_records_and_no_workspaces() {
    let (_plane, root) = plane_with_a_todo_and_two_personas();
    a_record(&root, Place::PlaneRoot, "Tidy personas", 8, None);
    a_record(
        &root,
        Place::Workspace("alpha".to_owned()),
        "Alpha work",
        9,
        None,
    );

    let drawn = plane_root(&root);

    let titles: Vec<&str> = drawn
        .sessions
        .iter()
        .map(|row| row.title.as_str())
        .collect();
    assert_eq!(titles, ["Tidy personas"]);
    assert_eq!(
        drawn.sessions[0].path,
        "sessions/20260928-080000-tidy-personas.md"
    );
    let keys: Vec<&str> = drawn
        .contributed
        .iter()
        .map(|panel| panel.key.as_str())
        .collect();
    assert_eq!(keys, ["charter/personas", "charter/sessions"]);
}

#[test]
fn the_plane_root_draws_the_projects_personas_as_a_workspace_does() {
    // #1686: personas are the project's, so the Personas view has them with no workspace
    // focused too, the same rows a workspace's view draws, and no todo panel beside them.
    let (_plane, root) = plane_with_a_todo_and_two_personas();

    let at_root = plane_root(&root);
    let in_alpha = of(&root, "alpha").expect("the panels draw");

    let personas = at_root
        .contributed
        .iter()
        .find(|panel| panel.key == "charter/personas")
        .expect("the plane root has the personas panel");
    assert_eq!(personas, panel_called(&in_alpha, "charter/personas"));
    let texts: Vec<&str> = list_of(personas)
        .iter()
        .map(|row| row.text.as_str())
        .collect();
    assert_eq!(texts, ["devops", "steward", "shared"]);
    assert!(
        at_root
            .contributed
            .iter()
            .all(|panel| panel.key != "charter/todos"),
        "the plane root has no todos"
    );
}

#[test]
fn a_session_record_is_read_for_its_tab_by_its_path_and_a_path_out_is_refused() {
    let (_plane, root) = plane_with_a_todo_and_two_personas();
    let shown = a_record(
        &root,
        Place::Workspace("alpha".to_owned()),
        "Ship it",
        9,
        None,
    );
    std::fs::write(root.join("secret.md"), "x").expect("a file outside");

    let view = session_record(&root, &shown)
        .expect("it reads")
        .expect("it is there");
    assert_eq!(view.place, "alpha");
    assert_eq!(view.row.title, "Ship it");
    assert!(
        view.body.starts_with("# Ship it\n"),
        "the body, without the frontmatter"
    );

    for path in [
        "sessions/../secret.md",
        "workspaces/alpha/workspace.md",
        "/etc/passwd",
    ] {
        assert!(session_record(&root, path).is_err(), "a path out was read");
    }
    assert_eq!(
        session_record(&root, "workspaces/alpha/sessions/20260101-000000-gone.md"),
        Ok(None),
        "a record that has gone is gone, not a failure"
    );
}

#[test]
fn the_plane_root_names_what_the_last_send_of_the_project_left_out_and_nothing_else() {
    let dir = tempfile::tempdir().expect("a plane");
    let root = std::fs::canonicalize(dir.path()).expect("a resolved plane");
    std::fs::write(root.join("charter.toml"), "").expect("a manifest");
    let titles = |panels: &PlaneRootPanels| -> Vec<String> {
        serde_json::to_value(panels).expect("serialisable")["contributed"]
            .as_array()
            .expect("contributed")
            .iter()
            .map(|one| one["title"].as_str().unwrap_or_default().to_owned())
            .collect()
    };
    assert!(!titles(&plane_root(&root)).contains(&"Sent to runners".to_owned()));

    sent_leaving(&root, vec!["workspaces/app/memory/token.md".to_owned()]);
    let shown = serde_json::to_value(plane_root(&root)).expect("serialisable");
    let sent = shown["contributed"]
        .as_array()
        .expect("contributed")
        .iter()
        .find(|one| one["title"] == "Sent to runners")
        .expect("the panel is drawn")
        .to_string();
    assert!(sent.contains("left out 1 file"), "{sent}");
    assert!(
        sent.contains("refuse it: workspaces/app/memory/token.md"),
        "{sent}"
    );
    assert!(sent.contains("It stays on this machine"), "{sent}");

    sent_leaving(&root, Vec::new());
    assert!(!titles(&plane_root(&root)).contains(&"Sent to runners".to_owned()));
}

#[test]
fn what_a_send_left_out_is_said_in_words_that_agree_and_names_at_most_five() {
    let many: Vec<String> = (1..=7).map(|n| format!("f{n}.key")).collect();
    let said = left_out_said("the project", &many);
    assert!(said.contains("left out 7 files"), "{said}");
    assert!(
        said.contains("refuse them: f1.key, f2.key, f3.key, f4.key, f5.key, and 2 more."),
        "{said}"
    );
    assert!(said.ends_with("They stay on this machine."), "{said}");
}
