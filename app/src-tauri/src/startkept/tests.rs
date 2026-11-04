// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::cell::Cell;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use purlis_core::pickerstamp::Stamp;

use super::Kept;
use crate::planes::Held;

/// A stamp of nothing, which always holds.
fn holding() -> Stamp {
    Stamp::of_paths([])
}

/// `kept` asked once, `reads` counting each time it had to read.
fn ask(kept: &Kept<u32>, trusted: bool, reads: &Cell<u32>) -> u32 {
    kept.served::<()>(trusted, || {
        reads.set(reads.get() + 1);
        Ok((reads.get(), holding()))
    })
    .expect("answered")
}

#[test]
fn a_second_ask_with_nothing_changed_is_served_from_what_the_first_read() {
    let kept = Kept::default();
    let reads = Cell::new(0);

    assert_eq!(ask(&kept, true, &reads), 1);
    assert_eq!(ask(&kept, true, &reads), 1);
    assert_eq!(reads.get(), 1, "read once");
}

#[test]
fn an_answer_dropped_is_read_again_on_the_next_ask() {
    let kept = Kept::default();
    let reads = Cell::new(0);
    ask(&kept, true, &reads);

    kept.moved();

    assert_eq!(ask(&kept, true, &reads), 2);
}

#[test]
fn an_answer_whose_stamp_broke_is_read_again() {
    let dir = tempfile::tempdir().expect("a dir");
    let policy = dir.path().join("policy.json");
    let kept = Kept::default();
    let reads = Cell::new(0);
    let read = || {
        kept.served::<()>(true, || {
            reads.set(reads.get() + 1);
            Ok((reads.get(), Stamp::of_paths([policy.clone()])))
        })
        .expect("answered")
    };
    read();

    std::fs::write(&policy, "{}").expect("a policy lands");

    assert_eq!(read(), 2);
    assert_eq!(read(), 2, "kept again under the new stamp");
}

#[test]
fn a_read_that_began_before_a_drop_is_answered_and_not_kept() {
    let kept = Kept::default();
    let reads = Cell::new(0);

    let answer = kept
        .served::<()>(true, || {
            reads.set(reads.get() + 1);
            // The profile moves while the picker reads it.
            kept.moved();
            Ok((reads.get(), holding()))
        })
        .expect("answered");

    assert_eq!(answer, 1);
    assert_eq!(ask(&kept, true, &reads), 2);
}

#[test]
fn a_plane_not_wholly_watched_reads_on_every_ask() {
    let kept = Kept::default();
    let reads = Cell::new(0);
    ask(&kept, true, &reads);

    assert_eq!(ask(&kept, false, &reads), 2);
    assert_eq!(ask(&kept, false, &reads), 3);
}

#[test]
fn a_read_that_failed_is_not_kept() {
    let kept: Kept<u32> = Kept::default();
    let refused = kept.served(true, || Err::<(u32, Stamp), _>("unreadable"));
    assert_eq!(refused, Err("unreadable"));

    let reads = Cell::new(0);
    assert_eq!(ask(&kept, true, &reads), 1);
}

// ---- through a held plane's watch ----------------------------------------------------------

/// A held plane with a persona, a workspace with a todo, and a profile of its own.
fn held_plane() -> (tempfile::TempDir, crate::planes::Planes, Arc<Held>) {
    let dir = tempfile::tempdir().expect("a plane");
    let root = std::fs::canonicalize(dir.path()).expect("a resolved plane");
    std::fs::write(root.join("charter.toml"), "").expect("a manifest");
    std::fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        "[harness.mine]\nkind = \"claude\"\ncommand = [\"claude\"]\n",
    )
    .expect("a profile");
    std::fs::create_dir_all(root.join("personas/scribe")).expect("a persona");
    std::fs::write(root.join("personas/scribe/persona.md"), "# scribe\n").expect("its md");
    std::fs::create_dir_all(root.join(".claude")).expect("harness settings");
    std::fs::create_dir_all(root.join("workspaces/alpha/todos")).expect("a todo store");
    std::fs::write(root.join("workspaces/alpha/todos/a1.md"), "# Alpha one\n").expect("a todo");
    let planes = crate::planes::Planes::telling(
        Arc::new(|_: crate::hooks::Moved| {}),
        crate::Shipped::default(),
        None,
    );
    let id = planes.open(&root);
    let held = planes.held(&id).expect("held");
    (dir, planes, held)
}

/// The picker's details asked of `held`, `reads` counting each time they had to be read: what a
/// read costs is a sandbox compile per harness and a check of each profile's program.
fn details(held: &Held, reads: &Cell<u32>) {
    held.kept_start_details(|| {
        reads.set(reads.get() + 1);
        Ok((Vec::new(), Stamp::before_reading(held.root())))
    })
    .expect("answered");
}

/// Asks `held` for the details until they are read again, for as long as a loaded runner can
/// take to tell the change: no timing is asserted, and a passing run returns the moment they are.
fn read_again_once_told(held: &Held, reads: &Cell<u32>, what: &str) {
    let was = reads.get();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        details(held, reads);
        if reads.get() > was {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{what} never dropped the kept details"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// `held`'s plane changed by `change`, the details asked before and after.
fn after(change: impl FnOnce(&Path), what: &str) {
    let (_dir, _planes, held) = held_plane();
    let reads = Cell::new(0);
    details(&held, &reads);
    details(&held, &reads);
    assert_eq!(
        reads.get(),
        1,
        "a second open with nothing changed is served kept"
    );

    change(held.root());

    read_again_once_told(&held, &reads, what);
}

#[test]
fn a_profile_changed_drops_the_kept_answer() {
    after(
        |root| {
            std::fs::write(
                root.join(purlis_core::profiles::LOCAL_FILE),
                "[harness.mine]\nkind = \"claude\"\n\n[harness.other]\nkind = \"claude\"\n",
            )
            .expect("another profile");
        },
        "a profile",
    );
}

#[test]
fn a_persona_changed_drops_the_kept_answer() {
    after(
        |root| {
            std::fs::write(root.join("personas/scribe/persona.md"), "# scribe, again\n")
                .expect("an edit");
        },
        "a persona edited",
    );
}

#[test]
fn a_persona_made_drops_the_kept_answer() {
    after(
        |root| {
            std::fs::create_dir_all(root.join("personas/reviewer")).expect("a persona");
        },
        "a persona made",
    );
}

#[test]
fn the_projects_sandbox_changed_drops_the_kept_answer() {
    after(
        |root| {
            std::fs::write(root.join("charter.toml"), "[sandbox]\nmode = \"on\"\n")
                .expect("the sandbox on");
        },
        "the project's sandbox",
    );
}

#[test]
fn the_harness_settings_changed_drop_the_kept_answer() {
    after(
        |root| {
            std::fs::write(root.join(".claude/settings.json"), "{}").expect("settings");
        },
        "the harness settings",
    );
}

#[test]
fn a_profile_approved_drops_the_kept_answer() {
    // Recorded in the project's state folder, which the watch does not follow: the stamp does.
    after(
        |root| {
            let set = purlis_core::profiles::current(root);
            let mine = set.get("mine").expect("the profile");
            purlis_core::profiletrust::record_launched(
                root,
                "mine",
                &purlis_core::profiletrust::fingerprint(mine),
            )
            .expect("approved");
        },
        "an approval",
    );
}

#[test]
fn a_todo_written_leaves_the_kept_answer() {
    let (_dir, _planes, held) = held_plane();
    let reads = Cell::new(0);
    details(&held, &reads);
    std::fs::write(
        held.root().join("workspaces/alpha/todos/a2.md"),
        "# Alpha two\n",
    )
    .expect("a todo");

    // Once the watch has told the sidebar's model the todo, it has told everything it will.
    let deadline = Instant::now() + Duration::from_secs(60);
    while !held
        .sidebar_model()
        .rows()
        .expect("listed")
        .any(|row| row.todos.contains(&"Alpha two".to_owned()))
    {
        assert!(
            Instant::now() < deadline,
            "the todo never reached the sidebar"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    details(&held, &reads);
    assert_eq!(reads.get(), 1, "served kept");
}

// ---- the stamp is taken before what it covers is read --------------------------------------

/// The profiles of the project at `root` as `for_launch` reads them, with `mid_read` landing on
/// it once they are read: a change made while the picker reads.
fn read_then(
    root: &Path,
    mid_read: impl FnOnce(&Path),
) -> impl FnOnce(&Path) -> crate::LaunchProfiles {
    move |at: &Path| {
        let read = purlis_core::profiles::for_launch(at);
        mid_read(root);
        read
    }
}

/// A project with one profile of its own.
fn a_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a project");
    std::fs::write(dir.path().join("charter.toml"), "").expect("a manifest");
    std::fs::write(
        dir.path().join(purlis_core::profiles::LOCAL_FILE),
        "[harness.mine]\nkind = \"claude\"\ncommand = [\"claude\"]\n",
    )
    .expect("a profile");
    dir
}

/// Another profile written beside it, as a chat or the person would.
fn another_profile(root: &Path) {
    std::fs::write(
        root.join(purlis_core::profiles::LOCAL_FILE),
        "[harness.mine]\nkind = \"claude\"\ncommand = [\"claude\"]\n\n\
         [harness.other]\nkind = \"claude\"\ncommand = [\"claude\"]\n",
    )
    .expect("another profile");
}

#[test]
fn a_profile_changed_while_the_first_answer_reads_breaks_its_stamp() {
    let project = a_project();
    let root = project.path();

    let (_first, stamp) =
        crate::start_first_stamped(root, read_then(root, another_profile)).expect("read");

    assert!(
        !stamp.holds(),
        "an answer read before the change must not be kept under a stamp taken after it"
    );
}

#[test]
fn a_profile_changed_while_the_details_read_breaks_their_stamp() {
    let project = a_project();
    let root = project.path();

    let (_details, stamp) = crate::start_details_stamped(
        root,
        read_then(root, another_profile),
        &purlis_core::sandbox::Machine::this(),
        &|_| true,
    );

    assert!(
        !stamp.holds(),
        "details read before the change must not be kept under a stamp taken after it"
    );
}
