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
    std::fs::create_dir_all(root.join(".purlis")).expect("a state folder");
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
fn a_local_vault_registry_mended_drops_the_kept_answer() {
    // In the state folder, which the watch does not tell: the stamp does. A broken registry
    // refuses a row, so the kept refusal must not outlive the mending.
    after(
        |root| {
            std::fs::write(root.join(".purlis/vaults.json"), "{}").expect("a registry");
        },
        "the local vault registry",
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

#[test]
fn a_guard_sighting_written_every_second_leaves_the_kept_answer() {
    // Every tool call of every chat writes `.purlis/guard-seen.json`; on a machine with dozens
    // of chats that is a write a second, and each one dropped the kept answer, so every open of
    // the picker read it again under load (#7603 field evidence).
    let (_dir, _planes, held) = held_plane();
    let reads = Cell::new(0);
    details(&held, &reads);
    let state = held.root().join(".purlis");
    for n in 0..5 {
        std::fs::write(state.join("guard-seen.json"), format!("{{\"n\":{n}}}")).expect("seen");
    }
    std::fs::write(
        held.root().join("workspaces/alpha/todos/a2.md"),
        "# Alpha two\n",
    )
    .expect("a todo, which the watch tells after the writes before it");
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

// ---- the first open after a restart (h19) ----------------------------------------------------

/// Asks `held` for the first answer, `reads` counting each time it had to be read.
fn first(held: &Held, reads: &std::sync::atomic::AtomicU32) {
    held.kept_start_first(|| {
        reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        crate::start_first_stamped(held.root(), purlis_core::profiles::for_launch)
    })
    .expect("answered");
}

#[test]
fn a_held_plane_keeps_its_first_answer_before_the_picker_first_opens() {
    // The first open after a restart read it cold, behind the restore of every chat, and took
    // 43 s with 36 chats coming back (h19 field evidence): it is read once the plane is held.
    let (_dir, _planes, held) = held_plane();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !held.start_first_is_kept() {
        assert!(
            Instant::now() < deadline,
            "the first answer was never read ahead of the picker"
        );
        std::thread::sleep(Duration::from_millis(50));
    }

    let reads = std::sync::atomic::AtomicU32::new(0);
    first(&held, &reads);
    assert_eq!(
        reads.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "the first open is served kept"
    );
}

#[test]
fn a_kept_first_answer_is_served_while_a_long_re_read_holds_the_model() {
    // The watch's re-read of the plane, a panel served, a restore's writes applied: each holds
    // the sidebar's model, and the picker waited for it before it looked at what it kept.
    let (_dir, _planes, held) = held_plane();
    let reads = std::sync::atomic::AtomicU32::new(0);
    first(&held, &reads);

    let model = held.model_held();
    let (answered, heard) = std::sync::mpsc::channel();
    let asking = Arc::clone(&held);
    std::thread::spawn(move || {
        let reads = std::sync::atomic::AtomicU32::new(0);
        first(&asking, &reads);
        let _ = answered.send(());
    });
    let served = heard.recv_timeout(Duration::from_secs(10));
    drop(model);

    assert!(
        served.is_ok(),
        "the first answer waited on the model's lock"
    );
}

#[test]
fn a_watch_whole_again_drops_the_kept_details_before_the_model_is_read_again() {
    // The watch lapsed and `.claude/settings.json` moved in the gap, where the details' stamp
    // does not look. A sidebar ask says `WholeAgain` and reads the model again, 91-457 ms; the
    // picker, which does not wait on the model, sees `Whole` from then on, and an open during
    // that re-read was served the details kept from before the gap.
    let (_dir, _planes, held) = held_plane();
    let reads = Cell::new(0);
    details(&held, &reads);
    // A change in the gap (`.claude/settings.json`, say) was told to nobody: none is made here,
    // or the watch could tell it and drop the details by itself.
    held.watch_lapsed();

    // What the sidebar does before its re-read.
    assert_eq!(
        held.watch_standing(),
        Some(crate::planewatch::Standing::WholeAgain)
    );
    details(&held, &reads);

    assert_eq!(
        reads.get(),
        2,
        "the details kept over the gap were served during the re-read"
    );
}

#[test]
fn the_first_answers_read_is_timed_by_phase() {
    let project = a_project();
    std::fs::create_dir_all(project.path().join("personas/scribe")).expect("a persona");
    std::fs::write(
        project.path().join("personas/scribe/persona.md"),
        "# scribe\n",
    )
    .expect("its md");

    let timed = purlis_core::opening::timed(|| {
        crate::start_first_stamped(project.path(), purlis_core::profiles::for_launch)
    });

    timed.out.expect("read");
    let names: Vec<&str> = timed.phases.iter().map(|took| took.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "stamp",
            "profiles",
            "harness cards",
            "personas",
            "offers",
            "persona profiles",
            "rows",
        ]
    );
}

#[test]
fn a_slow_first_answer_is_written_with_its_phases_and_a_quick_one_is_not() {
    let phases = vec![purlis_core::slowlog::Took {
        name: "personas".to_owned(),
        ms: 1_200,
    }];

    assert_eq!(crate::first_answer_slow(999, 7, 36, phases.clone()), None);
    assert_eq!(
        crate::first_answer_slow(1_300, 7, 36, phases.clone()),
        Some(purlis_core::slowlog::Slow::Answer {
            command: "startOptionsFirst".to_owned(),
            ms: 1_300,
            at: 7,
            hot: 36,
            phases,
        })
    );
}

/// **The measurement behind h19**, by hand: `cargo test --lib startkept::tests::measure -- \
/// --ignored --nocapture`. A plane of 40 personas and 60 workspaces: each phase of the first
/// answer's cold read, and how long one whole re-read of the sidebar's model holds its lock,
/// which the picker waited on before it looked at what it kept.
#[test]
#[ignore = "a measurement, printed: run by hand"]
// The numbers are the measurement's whole point, read with `--nocapture`.
#[allow(clippy::disallowed_macros)]
fn measure_the_first_answer_cold_on_a_big_plane() {
    let (_dir, _planes, held) = held_plane();
    let root = held.root().to_path_buf();
    for n in 0..40 {
        let persona = root.join(format!("personas/p{n}"));
        std::fs::create_dir_all(persona.join("memory")).expect("a persona");
        std::fs::write(
            persona.join("persona.md"),
            format!("---\nrole: Persona {n}\ndelegate-when: asked\n---\n# p{n}\n"),
        )
        .expect("its md");
    }
    for n in 0..60 {
        let todos = root.join(format!("workspaces/w{n}/todos"));
        std::fs::create_dir_all(&todos).expect("a workspace");
        for t in 0..10 {
            std::fs::write(todos.join(format!("t{t}.md")), format!("# Todo {t}\n")).expect("todo");
        }
    }

    for round in ["cold", "warm"] {
        let timed = purlis_core::opening::timed(|| {
            crate::start_first_stamped(&root, purlis_core::profiles::for_launch)
        });
        timed.out.expect("read");
        let phases: Vec<String> = timed
            .phases
            .iter()
            .map(|took| format!("{} {} ms", took.name, took.ms))
            .collect();
        eprintln!(
            "first answer, {round}: {} ms: {}",
            timed.ms,
            phases.join(", ")
        );
    }
    let began = Instant::now();
    held.workspaces_moved();
    eprintln!(
        "one whole re-read of the model, under its lock: {} ms",
        began.elapsed().as_millis()
    );
}
