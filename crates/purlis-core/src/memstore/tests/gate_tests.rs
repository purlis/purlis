// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn stamp() -> chrono::NaiveDateTime {
    "2026-03-02T09:14:00".parse().unwrap()
}

fn plane() -> (tempfile::TempDir, std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), "schema = 1\n").unwrap();
    // A store still reached by path: a persona's ephemeral memory. A persona's own memory
    // is held by descriptor (D-90c), and its writer is tested with the held layer.
    let store = crate::recall::ephemeral_dir(dir.path(), "gate", "devops");
    std::fs::create_dir_all(&store).unwrap();
    (dir, store, tempfile::tempdir().unwrap())
}

#[test]
fn a_refused_index_leaves_no_memory_file_behind() {
    // Both targets are gated before a byte is written, as charter's `write` gates them:
    // a memory file on disk that nothing indexes is the half-write this prevents.
    let (dir, store, outside) = plane();
    std::fs::write(outside.path().join("idx"), "PRECIOUS\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("idx"), store.join(INDEX)).unwrap();

    let refused = write(
        dir.path(),
        &store,
        "A fact",
        None,
        false,
        "persistent",
        true,
        stamp(),
    );

    assert_eq!(
        refused.unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert!(!store.join("a-fact.md").exists(), "no memory file was left");
    assert_eq!(
        std::fs::read_to_string(outside.path().join("idx")).unwrap(),
        "PRECIOUS\n"
    );
}

/// `write` into `store` with its index, after `plant` made the index refuse the line.
#[cfg(unix)]
fn refused_at_the_append(plant: impl FnOnce(&std::path::Path)) {
    let (dir, store, _outside) = plane();
    std::fs::write(store.join("fact.md"), "PRECIOUS\n").unwrap();
    plant(&store);

    let refused = write(
        dir.path(),
        &store,
        "A fact",
        None,
        false,
        "persistent",
        true,
        stamp(),
    );

    assert!(refused.is_err(), "{refused:?}");
    assert!(
        std::fs::symlink_metadata(store.join("a-fact.md")).is_err(),
        "a memory file nothing indexes was left"
    );
    assert_eq!(
        std::fs::read_to_string(store.join("fact.md")).unwrap(),
        "PRECIOUS\n"
    );
}

#[cfg(unix)]
#[test]
fn an_index_linked_inside_the_store_refuses_the_line_and_leaves_no_memory_file() {
    // #1058: the link passes containment, so the refusal comes at the append itself.
    refused_at_the_append(|store| {
        std::os::unix::fs::symlink(store.join("fact.md"), store.join(INDEX)).unwrap();
    });
}

#[cfg(unix)]
#[test]
fn a_read_only_index_refuses_the_line_and_leaves_no_memory_file() {
    use std::os::unix::fs::PermissionsExt;
    refused_at_the_append(|store| {
        std::fs::write(store.join(INDEX), "# Memory\n").unwrap();
        std::fs::set_permissions(store.join(INDEX), std::fs::Permissions::from_mode(0o444))
            .unwrap();
    });
}

#[cfg(unix)]
#[test]
fn a_fifo_at_the_index_refuses_the_line_at_once_and_leaves_no_memory_file() {
    // Opened without waiting for a reader: a FIFO must never hold the write for ever.
    refused_at_the_append(|store| {
        let made =
            crate::forklock::status(std::process::Command::new("mkfifo").arg(store.join(INDEX)))
                .expect("mkfifo runs");
        assert!(made.success());
    });
}

#[cfg(unix)]
#[test]
fn an_index_that_is_a_link_is_refused_and_what_it_points_at_is_left_alone() {
    // A link to another file in the same store passes containment; the open refuses it.
    let (dir, store, _outside) = plane();
    std::fs::write(store.join("fact.md"), "PRECIOUS\n").unwrap();
    std::os::unix::fs::symlink(store.join("fact.md"), store.join(INDEX)).unwrap();

    assert!(index_append(dir.path(), &store.join(INDEX), "a.md", "A").is_err());
    assert_eq!(
        std::fs::read_to_string(store.join("fact.md")).unwrap(),
        "PRECIOUS\n"
    );

    // A dangling one is refused too: the header is not written through it.
    let gone = store.join("gone.md");
    std::fs::remove_file(store.join(INDEX)).unwrap();
    std::os::unix::fs::symlink(&gone, store.join(INDEX)).unwrap();
    assert!(index_append(dir.path(), &store.join(INDEX), "a.md", "A").is_err());
    assert!(!gone.exists(), "nothing was created where the link points");
}

/// Plant a link to `at_target` at the path being replaced, in the window between the
/// gate and the rename — where an in-place write would have followed it. Returns whether
/// the window was ever reached, so a writer that bypasses `rewrite` cannot pass vacuously.
#[cfg(unix)]
fn plant_in_the_window(
    at_target: std::path::PathBuf,
) -> (
    std::rc::Rc<std::cell::Cell<bool>>,
    crate::rewrite::hook::Unset,
) {
    let fired = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = std::rc::Rc::clone(&fired);
    let unset = crate::rewrite::hook::set(move |target, _temp| {
        seen.set(true);
        let _ = std::fs::remove_file(target);
        std::os::unix::fs::symlink(&at_target, target)
    });
    (fired, unset)
}

#[cfg(unix)]
#[test]
fn a_link_planted_at_a_new_memorys_name_is_never_written_through() {
    // #434: the name is gated, then written. A link that lands in between used to take
    // the whole memory to wherever it pointed.
    let (dir, store, outside) = plane();
    let theirs = outside.path().join("theirs");
    std::fs::write(&theirs, "THEIRS\n").unwrap();
    let (fired, _unset) = plant_in_the_window(theirs.clone());

    let path = write(
        dir.path(),
        &store,
        "A fact",
        None,
        false,
        "persistent",
        false,
        stamp(),
    )
    .unwrap();

    assert!(fired.get(), "the memory went through rewrite::replace");
    assert_eq!(std::fs::read_to_string(&theirs).unwrap(), "THEIRS\n");
    assert!(!path.is_symlink(), "the link was replaced, not followed");
}

#[test]
fn a_memory_write_that_dies_before_its_rename_leaves_no_memory_behind() {
    let (dir, store, _outside) = plane();
    let _killed = crate::rewrite::hook::set(|_, _| Err(std::io::Error::other("killed")));

    let refused = write(
        dir.path(),
        &store,
        "A fact",
        None,
        false,
        "persistent",
        false,
        stamp(),
    );

    assert!(refused.is_err());
    assert_eq!(
        std::fs::read_dir(&store).unwrap().count(),
        0,
        "no temp, no memory"
    );
}

#[test]
fn forgetting_a_memory_that_dies_while_dropping_its_line_leaves_the_index_whole() {
    // #434: the index is replaced whole, so a crash mid-rewrite leaves the old index,
    // stale line and all, rather than an empty or cut-short one.
    let (dir, store, _outside) = plane();
    std::fs::write(store.join("a.md"), "# A\n").unwrap();
    let index = "# Memory Index\n\n- [A](a.md)\n- [B](b.md)\n";
    std::fs::write(store.join(INDEX), index).unwrap();
    let _killed = crate::rewrite::hook::set(|_, _| Err(std::io::Error::other("killed")));

    forget(dir.path(), &store, "a").unwrap();

    assert_eq!(std::fs::read_to_string(store.join(INDEX)).unwrap(), index);
}

#[cfg(unix)]
#[test]
fn a_link_planted_at_the_index_while_a_line_is_dropped_is_never_truncated_through() {
    let (dir, store, outside) = plane();
    std::fs::write(store.join("a.md"), "# A\n").unwrap();
    std::fs::write(store.join(INDEX), "# Memory Index\n\n- [A](a.md)\n").unwrap();
    let theirs = outside.path().join("theirs");
    std::fs::write(&theirs, "THEIRS\n").unwrap();
    let (fired, _unset) = plant_in_the_window(theirs.clone());

    forget(dir.path(), &store, "a").unwrap();

    assert!(fired.get(), "the index went through rewrite::replace");
    assert_eq!(std::fs::read_to_string(&theirs).unwrap(), "THEIRS\n");
    assert_eq!(
        std::fs::read_to_string(store.join(INDEX)).unwrap(),
        "# Memory Index\n\n"
    );
}

#[test]
fn a_new_index_gets_its_header_and_then_its_lines_in_order() {
    let (dir, store, _outside) = plane();
    index_append(dir.path(), &store.join(INDEX), "a.md", "A").unwrap();
    index_append(dir.path(), &store.join(INDEX), "b.md", "B").unwrap();
    assert_eq!(
        std::fs::read_to_string(store.join(INDEX)).unwrap(),
        "# Memory Index\n\n- [A](a.md)\n- [B](b.md)\n"
    );
}

#[cfg(unix)]
#[test]
fn a_memory_under_the_state_directory_is_private() {
    // The ephemeral quadrant is charter's own state: 0700 directories and a 0600 file,
    // whatever the umask. A committed store is left to the operator's modes.
    use std::os::unix::fs::PermissionsExt;
    let (dir, _store, _outside) = plane();
    let scratch = dir
        .path()
        .join(".charter/persona-state/ephemeral/s1/devops");

    let path = write(
        dir.path(),
        &scratch,
        "Scratch",
        None,
        false,
        "ephemeral",
        false,
        stamp(),
    )
    .unwrap();

    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&path), 0o600);
    assert_eq!(mode(&scratch), 0o700);
    assert_eq!(mode(scratch.parent().unwrap()), 0o700);
}

#[test]
fn a_name_is_resolved_only_to_a_file_the_listing_would_read() {
    // The direct hit asks the listing's whole question. `is_file()` alone let a link out
    // of the plane, named, reach `forget`.
    let (dir, store, outside) = plane();
    std::fs::write(outside.path().join("secret.md"), "# Secret\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("secret.md"), store.join("leak.md")).unwrap();
    std::fs::write(store.join("real.md"), "# Real\n").unwrap();

    assert!(resolve(dir.path(), &store, "leak").is_err());
    assert!(resolve(dir.path(), &store, "leak.md").is_err());
    assert_eq!(
        resolve(dir.path(), &store, "real").unwrap(),
        store.join("real.md")
    );
}

/// A persona's ephemeral store, reached by path, with no `charter.toml` beside it: a
/// refusal test needs nothing the sandbox would not let it write. (A persona's own memory
/// is held by descriptor, D-90c.)
#[cfg(target_os = "macos")]
fn persona_store() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::recall::ephemeral_dir(dir.path(), "gate", "devops");
    (dir, store)
}

#[cfg(target_os = "macos")]
#[test]
fn a_store_the_filesystem_refuses_to_make_is_named() {
    // #1359: `persona remember` into a store not there yet printed only the OS error.
    let (dir, store) = persona_store();
    let persona = store.parent().unwrap();
    std::fs::create_dir_all(persona).unwrap();
    let _frozen = crate::rewrite::frozen::Frozen::at(persona);

    let refused = write(
        dir.path(),
        &store,
        "a fact",
        None,
        false,
        "fact",
        true,
        stamp(),
    )
    .unwrap_err();

    crate::rewrite::frozen::names(&refused, &store);
}

#[cfg(target_os = "macos")]
#[test]
fn an_index_line_the_filesystem_refuses_names_the_index() {
    let (dir, store) = persona_store();
    std::fs::create_dir_all(&store).unwrap();
    let index = store.join(INDEX);
    std::fs::write(&index, INDEX_FALLBACK).unwrap();
    let _frozen = crate::rewrite::frozen::Frozen::at(&index);

    let refused = index_append(dir.path(), &index, "a-fact.md", "A fact").unwrap_err();

    crate::rewrite::frozen::names(&refused, &index);
}

#[cfg(target_os = "macos")]
#[test]
fn an_index_the_filesystem_refuses_to_create_is_named() {
    let (dir, store) = persona_store();
    std::fs::create_dir_all(&store).unwrap();
    let _frozen = crate::rewrite::frozen::Frozen::at(&store);

    let refused = ensure_index(dir.path(), &store, "# Memory").unwrap_err();

    crate::rewrite::frozen::names(&refused, &store.join(INDEX));
}
