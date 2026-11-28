// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The core's spawn-count gate** (#7892, FS-G2): what each read the window makes when a project
//! comes in front costs in programs started, held to its class ([`Class`]) by count, never by
//! the clock.
//!
//! - **Asked as the command's own body runs**: each read is its command's function, driven to
//!   its answer on this thread, so the counting scope it opens ([`counting::counted`]) is the
//!   one the door carries to the blocking thread ([`crate::off_the_window`]). Tauri's invoke
//!   would run the body on a runtime thread the scope never reaches.
//! - **Starts and warm**: cold is the first ask of a project the app has just opened; warm is the
//!   same ask again at once, with whatever the first kept.
//! - **Per fixture**: a project whose sandbox is on, with a local settings file beside its
//!   manifest, which makes the sandbox's state ask git (`sandbox on`); and one whose committed
//!   manifest turns the sandbox off for the team, which makes it read git's history
//!   (`team off committed`).
//! - **What a process asks once is asked before the count**: each read is asked first of a
//!   project of its own, uncounted ([`primed`]), so no count depends on which test ran first.

use std::path::Path;

use purlis_core::forklock::counting::{self, Tally};
use tauri::Manager;
use tauri::test::{MockRuntime, mock_builder};

use crate::off_the_main_thread::{ANSWER_CLASSES, Class, answer_class};
use crate::planes::{PlaneId, Planes};

/// A project the ceilings are recorded on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fixture {
    SandboxOn,
    TeamOffCommitted,
}

const FIXTURES: [Fixture; 2] = [Fixture::SandboxOn, Fixture::TeamOffCommitted];

impl Fixture {
    fn made(self) -> tempfile::TempDir {
        match self {
            Self::SandboxOn => super::costs::a_project(),
            Self::TeamOffCommitted => team_off_committed(),
        }
    }
}

/// A git repository whose one commit's manifest turns the sandbox off for the team, with a
/// workspace and a persona.
fn team_off_committed() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    let root = dir.path();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[sandbox]\nmode = \"off\"\n",
    )
    .expect("charter.toml");
    std::fs::create_dir_all(root.join("workspaces/alpha")).expect("alpha");
    std::fs::write(root.join("workspaces/alpha/workspace.md"), "# alpha\n").expect("its md");
    std::fs::create_dir_all(root.join("personas/scribe")).expect("a persona");
    std::fs::write(
        root.join("personas/scribe/persona.md"),
        "---\nname: scribe\ndelegate_when: writing\n---\n# scribe\n",
    )
    .expect("its md");
    for args in [
        &["init", "--quiet", "--initial-branch=main"][..],
        &["add", "--all"],
        &["commit", "--quiet", "--message", "Turn the sandbox off"],
    ] {
        let mut git = std::process::Command::new("git");
        git.current_dir(root)
            .args([
                "-c",
                "user.name=Team",
                "-c",
                "user.email=team@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1");
        let done = purlis_core::forklock::output(&mut git).expect("git runs");
        assert!(done.status.success(), "git {args:?}: {done:?}");
    }
    dir
}

/// The app on Tauri's mock runtime, holding the project at `root`.
fn app_holding(root: &Path) -> (tauri::App<MockRuntime>, PlaneId) {
    let planes = Planes::telling(std::sync::Arc::new(|_| {}), crate::Shipped::default(), None);
    let plane = planes.open(root);
    let app = mock_builder()
        .manage(planes)
        .build(tauri_context!(test = true))
        .expect("the app builds");
    (app, plane)
}

/// `asked` driven to its answer on this thread, counted.
fn counted<T>(asked: impl Future<Output = Result<T, String>>) -> Tally {
    counting::counted(|| tauri::async_runtime::block_on(asked)).1
}

/// One read, asked of the project `plane` the app `app` holds, and what it cost.
type Ask = fn(&tauri::AppHandle<MockRuntime>, &PlaneId) -> Tally;

/// The reads the window makes as a project comes in front, each as its command asks it. A
/// chat-keyed one names chat 1, which is not open: it measures what the command reads before it
/// looks for the chat. `chat_usage` and `finished_turns` answer such a chat before the door, so
/// they wait, `Unmeasured`, for a fixture with a chat on the board.
const MEASURED: &[(&str, Ask)] = &[
    ("sandbox_state", |app, plane| {
        counted(crate::sandboxing::sandbox_state(app.state(), plane.clone()))
    }),
    ("persona_grants_held", |app, plane| {
        counted(crate::sandboxing::persona_grants_held(
            app.state(),
            plane.clone(),
            1,
        ))
    }),
    ("sandbox_network", |app, plane| {
        counted(crate::network::sandbox_network(app.state(), plane.clone()))
    }),
    ("chat_network", |app, plane| {
        counted(crate::network::chat_network(app.state(), plane.clone(), 1))
    }),
    ("sandbox_kept_blocked", |app, plane| {
        counted(crate::network::sandbox_kept_blocked(
            app.state(),
            plane.clone(),
        ))
    }),
    ("dispatch_grants_needed", |app, plane| {
        counted(crate::dispatchgrants::dispatch_grants_needed(
            app.state(),
            plane.clone(),
            1,
        ))
    }),
    ("dispatch_standing", |app, plane| {
        counted(crate::dispatchgrants::dispatch_standing(
            app.state(),
            plane.clone(),
        ))
    }),
    ("dispatch_away", |app, plane| {
        counted(crate::dispatchaway::dispatch_away(
            app.state(),
            plane.clone(),
        ))
    }),
    ("grantable_folders", |app, plane| {
        counted(crate::sandboxing::grantable_folders(
            app.state(),
            plane.clone(),
        ))
    }),
    ("project_harness_declarations", |app, plane| {
        counted(crate::harnessdecls::project_harness_declarations(
            app.state(),
            plane.clone(),
        ))
    }),
    ("project_read_only", |app, plane| {
        counted(crate::readonly::project_read_only(
            app.state(),
            plane.clone(),
        ))
    }),
    ("plane_pins", |app, plane| {
        counted(crate::plane_pins(app.state(), plane.clone()))
    }),
    ("plane_pin", |app, plane| {
        counted(crate::pin::plane_pin(app.state(), plane.clone()))
    }),
    ("session_record", |app, plane| {
        counted(crate::session_record(
            app.state(),
            plane.clone(),
            "sessions/20261002-090000-record.md".to_owned(),
        ))
    }),
    ("chat_work_item", |app, plane| {
        counted(crate::worklinks::chat_work_item(
            app.state(),
            plane.clone(),
            1,
        ))
    }),
    ("tasks_sharing_a_folder", |app, plane| {
        counted(crate::taskchanges::tasks_sharing_a_folder(
            app.state(),
            plane.clone(),
            1,
        ))
    }),
    ("task_folder_shared", |app, plane| {
        counted(crate::taskchanges::task_folder_shared(
            app.state(),
            plane.clone(),
            1,
            Some("alpha".to_owned()),
        ))
    }),
    ("opened_chats", |app, plane| {
        counted(crate::opened_chats(app.state(), plane.clone()))
    }),
    ("asks_waiting", |app, plane| {
        counted(crate::asking::asks_waiting(app.state(), plane.clone()))
    }),
    ("chats_plane_updated", |app, plane| {
        counted(crate::chats_plane_updated(app.state(), plane.clone()))
    }),
    ("plane_sidebar", |app, plane| {
        counted(crate::plane_sidebar(app.state(), plane.clone()))
    }),
    ("chats_on_older_sandbox", |app, plane| {
        counted(crate::chats_on_older_sandbox(app.state(), plane.clone()))
    }),
    ("chats_level_why", |app, plane| {
        counted(crate::chats_level_why(app.state(), plane.clone()))
    }),
];

/// **Each Git-class read's ceiling**: the programs it starts, cold and warm, on each fixture
/// (`[sandbox on, team off committed]`). Recorded on 2026-11-27 (#7892). **Each is an exact
/// count, not a bound**: the gate fails on fewer as on more, so a change that lowers one lowers
/// it here, and one that raises one says why in its pull request.
const CEILINGS: &[(&str, [Starts; 2])] = &[
    // The sandbox's state (#7900, FS-C1a): git asked once whether the local file is ignored,
    // which is one `git status` and the hardened runner's listing of the folder's config before
    // it (a listing is never kept where a count is taken); a committed off reads the manifest's
    // history three times, once for the team's off and twice for the level it bounds.
    (
        "sandbox_state",
        [Starts { cold: 2, warm: 2 }, Starts { cold: 9, warm: 9 }],
    ),
    (
        "sandbox_network",
        [Starts { cold: 2, warm: 2 }, Starts { cold: 0, warm: 0 }],
    ),
    (
        "dispatch_away",
        [Starts { cold: 3, warm: 3 }, Starts { cold: 2, warm: 2 }],
    ),
];

/// **What a read reads of the project's sandbox manifest and the sandbox's locks**, on each
/// fixture (`[sandbox on, team off committed]`), the same cold and warm (#7900, FS-C1a). Each
/// is an exact count, as [`CEILINGS`] are: read once and passed down to everything that asks.
const READS: &[(&str, [Reads; 2])] = &[(
    "sandbox_state",
    [Reads { plane: 1, locks: 1 }, Reads { plane: 1, locks: 1 }],
)];

/// What one ask read of the manifest ([`Tally::plane_reads`]) and of the locks
/// ([`Tally::locks_reads`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Reads {
    plane: usize,
    locks: usize,
}

impl Reads {
    fn of(tally: &Tally) -> Self {
        Self {
            plane: tally.plane_reads,
            locks: tally.locks_reads,
        }
    }
}

/// A ceiling's two columns: the programs started by the first ask, and by the one straight
/// after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Starts {
    cold: usize,
    warm: usize,
}

/// Every read asked once of a project of each fixture, uncounted, so what this process asks
/// once (git's version, this user's temporary directory) is asked before any count, and no count
/// depends on which test ran first.
fn primed() {
    for fixture in FIXTURES {
        let primer = fixture.made();
        let (app, plane) = app_holding(primer.path());
        for (_, ask) in MEASURED {
            ask(app.handle(), &plane);
        }
    }
}

/// What `ask` cost on `fixture`, cold and warm.
fn measured(ask: Ask, fixture: Fixture) -> (Tally, Tally) {
    let project = fixture.made();
    let (app, plane) = app_holding(project.path());
    let handle = app.handle();
    let cold = ask(handle, &plane);
    let warm = ask(handle, &plane);
    (cold, warm)
}

#[test]
fn every_measured_read_starts_no_more_programs_than_its_class_allows() {
    primed();
    let mut table = vec![format!(
        "{:<30} {:<10} {:>18} {:>18}",
        "command", "class", "sandbox on c/w", "team off c/w"
    )];
    let mut wrong = Vec::new();
    for (command, ask) in MEASURED {
        let class = answer_class(command);
        let counts: Vec<(Tally, Tally)> = FIXTURES.iter().map(|f| measured(*ask, *f)).collect();
        let column = |(cold, warm): &(Tally, Tally)| format!("{}/{}", cold.started, warm.started);
        let mut started_what = Vec::new();
        table.push(format!(
            "{command:<30} {:<10} {:>18} {:>18}   doors {}/{} {}/{}   manifest, locks read cold {}, {} and {}, {}",
            format!("{class:?}"),
            column(&counts[0]),
            column(&counts[1]),
            counts[0].0.doors,
            counts[0].1.doors,
            counts[1].0.doors,
            counts[1].1.doors,
            counts[0].0.plane_reads,
            counts[0].0.locks_reads,
            counts[1].0.plane_reads,
            counts[1].0.locks_reads,
        ));
        let through_the_door = counts
            .iter()
            .all(|(cold, warm)| cold.doors > 0 && warm.doors > 0);
        let ceiling = CEILINGS.iter().find(|(name, _)| name == command);
        for (fixture, (cold, warm)) in FIXTURES.iter().zip(&counts) {
            for (when, tally) in [("cold", cold), ("warm", warm)] {
                for program in &tally.programs {
                    started_what.push(format!("    {fixture:?} {when}: {program}"));
                }
            }
            let started = Starts {
                cold: cold.started,
                warm: warm.started,
            };
            if let Some((_, reads)) = READS.iter().find(|(name, _)| name == command) {
                let at = usize::from(*fixture == Fixture::TeamOffCommitted);
                for (when, tally) in [("cold", cold), ("warm", warm)] {
                    if Reads::of(tally) != reads[at] {
                        wrong.push(format!(
                            "{command} read {:?} {when} on {fixture:?}; its ceiling is {:?}",
                            Reads::of(tally),
                            reads[at]
                        ));
                    }
                }
            }
            match class {
                Class::Model if started != (Starts { cold: 0, warm: 0 }) => wrong.push(format!(
                    "{command} is Model, and started {started:?} on {fixture:?}"
                )),
                Class::Git => {
                    let at = usize::from(*fixture == Fixture::TeamOffCommitted);
                    match ceiling {
                        Some((_, ceilings)) if ceilings[at] == started => {}
                        Some((_, ceilings)) => wrong.push(format!(
                            "{command} started {started:?} on {fixture:?}; its ceiling is {:?}",
                            ceilings[at]
                        )),
                        None => wrong.push(format!("{command} is Git with no ceiling recorded")),
                    }
                }
                _ => {}
            }
        }
        match class {
            Class::Model | Class::Git if !through_the_door => wrong.push(format!(
                "{command} is {class:?}, but its work never reached the door, so nothing it \
                 started was counted"
            )),
            Class::Uncounted if counts.iter().any(|(c, w)| c.doors + w.doors > 0) => {
                wrong.push(format!(
                    "{command} is Uncounted, but its work now goes through the door: record its \
                     class and ceiling"
                ));
            }
            Class::Window | Class::Unmeasured => wrong.push(format!(
                "{command} is measured here, so its class is Model, Git or Uncounted, not \
                 {class:?}"
            )),
            _ => {}
        }
        table.extend(started_what);
    }
    println!("{}", table.join("\n"));
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn a_class_held_to_a_count_is_measured_and_only_git_has_ceilings() {
    let held_to_a_count: Vec<&str> = ANSWER_CLASSES
        .iter()
        .filter(|(class, _)| matches!(class, Class::Model | Class::Git))
        .flat_map(|(_, commands)| commands.iter().copied())
        .collect();
    let unmeasured: Vec<&&str> = held_to_a_count
        .iter()
        .filter(|name| !MEASURED.iter().any(|(measured, _)| measured == *name))
        .collect();
    assert!(
        unmeasured.is_empty(),
        "held to a count, but not measured here: {unmeasured:?}"
    );
    let not_git: Vec<&str> = CEILINGS
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| answer_class(name) != Class::Git)
        .collect();
    assert!(
        not_git.is_empty(),
        "a ceiling for a read that is not Git: {not_git:?}"
    );
    let reads_unceiled: Vec<&str> = READS
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !CEILINGS.iter().any(|(ceiled, _)| ceiled == name))
        .collect();
    assert!(
        reads_unceiled.is_empty(),
        "reads held to a count for a read with no ceiling of its starts: {reads_unceiled:?}"
    );
}

#[test]
fn a_burst_of_sandbox_state_asks_reads_the_project_once_after_the_read_running() {
    // The window asks the sandbox's state on every plane change, and a start sends several
    // (H15): every ask that comes while a read runs shares the one read after it.
    let project = Fixture::SandboxOn.made();
    let (app, plane) = app_holding(project.path());
    let app = app.handle().clone();
    let ask = || {
        tauri::async_runtime::block_on(crate::sandboxing::sandbox_state(app.state(), plane.clone()))
    };
    let read_once = counting::counted(ask).1;
    let answer = ask().expect("the state");
    let held = app.state::<Planes>().held(&plane).expect("held");
    let burst = &held.bursts().sandbox_state;

    const ASKS: usize = 8;
    let (inside, started) = std::sync::mpsc::channel::<()>();
    let (release, hold) = std::sync::mpsc::channel::<()>();
    let ((), tally) = counting::counted(|| {
        std::thread::scope(|scope| {
            // A read already running, which no ask after it may take: each waits for the next.
            let running = scope.spawn(move || {
                burst.ask(|| {
                    inside.send(()).expect("told");
                    hold.recv().expect("released");
                    answer.clone()
                })
            });
            started.recv().expect("the read runs");
            let asks: Vec<_> = (0..ASKS)
                .map(|_| {
                    let carried = counting::carried();
                    scope.spawn(move || carried.within(|| ask().expect("the state")))
                })
                .collect();
            // Until every ask waits for the next read, or has answered without one: a burst
            // that no longer joins its asks shows in the count below, not as a hang.
            while burst.waiting() < ASKS && !asks.iter().all(|one| one.is_finished()) {
                std::thread::yield_now();
            }
            release.send(()).expect("released");
            running.join().expect("the running read");
            for one in asks {
                one.join().expect("an ask");
            }
        });
    });

    // The read already running is held and reads nothing, so every manifest read counted is
    // the one read the asks waited for.
    assert!(read_once.plane_reads > 0, "{read_once:?}");
    assert_eq!(
        tally.plane_reads, read_once.plane_reads,
        "{ASKS} asks read the manifest {} times; one read reads it {}",
        tally.plane_reads, read_once.plane_reads
    );
}

#[test]
fn a_team_off_projects_counts_do_not_move_once_its_git_files_are_old_enough_to_keep() {
    // #7892: git's config listing is kept only once its files are some seconds old
    // (`worktree::git::listing`'s SETTLED, 2 s), which a slow run passes between `git init`
    // and the ask. Where a count is taken nothing is kept, so a count is the same either way.
    primed();
    let asks: Vec<(&str, Ask)> = MEASURED
        .iter()
        .filter(|(name, _)| CEILINGS.iter().any(|(ceiled, _)| ceiled == name))
        .copied()
        .collect();
    let starts = |ask: Ask, wait: std::time::Duration| {
        let project = Fixture::TeamOffCommitted.made();
        std::thread::sleep(wait);
        let (app, plane) = app_holding(project.path());
        let cold = ask(app.handle(), &plane);
        let warm = ask(app.handle(), &plane);
        (cold.started, warm.started)
    };
    for (name, ask) in asks {
        let fresh = starts(ask, std::time::Duration::ZERO);
        let settled = starts(ask, std::time::Duration::from_millis(2_500));
        assert_eq!(
            fresh, settled,
            "{name}: fresh, then with its git files settled"
        );
    }
}
