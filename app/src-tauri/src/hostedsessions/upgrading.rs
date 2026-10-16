// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An upgrade to chats in the session host loses nothing** (#2004; the operator, 2026-10-14:
//! "users should not lose their profiles and sessions").
//!
//! A project holds two chats, each on a profile the person declared in `charter.local.toml`,
//! with a command and an environment of its own and approved once; one of them works in a
//! worktree on a branch of its own, and the project keeps a session record. The launch before
//! the upgrade runs its chats in the window's own process, as every build before #2004 did, and
//! quits to install the update. The launch after it runs them in the session host, as this
//! build does by default ([`super::chats_in_the_host`], the same composition the app's launch
//! makes), and the person reopens what was open. Every chat comes back on its own conversation,
//! on its own profile, with that profile's command and environment and nothing asked again, in
//! its own folder on its own branch, and the branch, its commit and the session record are as
//! they were.
//!
//! **Each launch is a process of its own**, a child of this test, as a launch is: a project is
//! never opened twice in one process. The host is the host's own code, as the window's binary
//! composes it, over an in-memory link (`crate::behaviour`'s host). Its programs run in real
//! terminals, which a sandboxed agent cannot open: this first runs on CI.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use purlis_core::reopen::{self, Chat, Choice};

use crate::hooks::Moved;
use crate::planes::Planes;

/// Which launch a child of this test is, when it is one.
const LAUNCH: &str = "PURLIS_TEST_UPGRADE_LAUNCH";
/// Where the test's project and homes are, for a child.
const AT: &str = "PURLIS_TEST_UPGRADE_AT";
/// This test, by the name a child runs it by.
const THIS: &str =
    "hostedsessions::upgrading::an_upgrade_to_chats_in_the_host_reopens_every_chat_as_it_was";

const SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
    columns: 80,
    rows: 24,
};
const PATIENCE: Duration = Duration::from_secs(60);

/// The two profiles the person declared, each with the mark its environment sets.
const PROFILES: [(&str, &str); 2] = [("work", "from-work"), ("side", "from-side")];
/// The branch the `side` chat works on, in a worktree of its own.
const BRANCH: &str = "side-work";

/// The world both launches share: the project, a config home and a data home of the test's own.
struct World {
    at: PathBuf,
}

impl World {
    fn root(&self) -> PathBuf {
        self.at.join("plane")
    }
    /// Where each stand-in harness writes down how it started: in the project, which its
    /// sandbox lets it write.
    fn runs(&self) -> PathBuf {
        self.root().join("runs")
    }
    fn svc(&self) -> PathBuf {
        self.root().join("workspaces").join("alpha").join("svc")
    }
    fn worktree(&self) -> PathBuf {
        self.root()
            .join("workspaces")
            .join("alpha")
            .join("svc-side")
    }
    fn config(&self) -> PathBuf {
        self.at.join("config")
    }
    fn data(&self) -> PathBuf {
        self.at.join("data")
    }

    /// Every start a stand-in harness wrote down so far, oldest first.
    fn starts(&self) -> Vec<Started> {
        let mut found: Vec<(std::time::SystemTime, Started)> = std::fs::read_dir(self.runs())
            .expect("the runs")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("run."))
            .map(|entry| {
                let when = entry
                    .metadata()
                    .and_then(|meta| meta.modified())
                    .expect("when it was written");
                let said = std::fs::read_to_string(entry.path()).expect("what it wrote");
                (when, Started::read(&said))
            })
            .collect();
        found.sort_by_key(|(when, _)| *when);
        found.into_iter().map(|(_, started)| started).collect()
    }
}

/// What a stand-in harness wrote down as it started: the mark its profile's environment set,
/// the folder it started in, and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Started {
    mark: String,
    cwd: PathBuf,
    args: Vec<String>,
}

impl Started {
    fn read(said: &str) -> Started {
        let mut started = Started {
            mark: String::new(),
            cwd: PathBuf::new(),
            args: Vec::new(),
        };
        for line in said.lines() {
            if let Some(mark) = line.strip_prefix("mark=") {
                started.mark = mark.to_owned();
            } else if let Some(cwd) = line.strip_prefix("cwd=") {
                started.cwd = PathBuf::from(cwd);
            } else if let Some(arg) = line.strip_prefix("arg=") {
                started.args.push(arg.to_owned());
            }
        }
        started
    }

    /// The word that follows `flag` in its arguments.
    fn after(&self, flag: &str) -> Option<&str> {
        let at = self.args.iter().position(|arg| arg == flag)?;
        self.args.get(at + 1).map(String::as_str)
    }
}

#[test]
fn an_upgrade_to_chats_in_the_host_reopens_every_chat_as_it_was() {
    match std::env::var(LAUNCH).as_deref() {
        Ok("before") => return the_launch_before_the_upgrade(&a_child_s_world()),
        Ok("after") => return the_launch_after_the_upgrade(&a_child_s_world()),
        _ => {}
    }
    let dir = tempfile::tempdir().expect("a folder for the test");
    let world = World {
        at: std::fs::canonicalize(dir.path()).expect("a resolved folder"),
    };
    a_project_with_two_declared_profiles_a_branch_and_a_session_record(&world);
    let record_before = session_records(&world);
    let tip_before = git(&world.svc(), &["rev-parse", BRANCH]);

    launch(&world, "before");
    let before = world.starts();
    let kept = reopen::read_or_refusal(&world.root()).expect("the record the quit wrote");
    launch(&world, "after");
    let after = world.starts()[before.len()..].to_vec();

    // Each chat the quit kept, by its profile: its conversation, and the folder it was in.
    let mut chats: Vec<(String, String, PathBuf)> = kept
        .chats
        .iter()
        .map(|chat| {
            (
                chat.profile.clone().expect("a chat on a profile"),
                chat.resume
                    .as_ref()
                    .expect("its conversation was kept")
                    .as_str()
                    .to_owned(),
                chat.cwd.clone().expect("its folder"),
            )
        })
        .collect();
    chats.sort();
    assert_eq!(
        chats
            .iter()
            .map(|(profile, ..)| profile.as_str())
            .collect::<Vec<_>>(),
        ["side", "work"],
        "the quit kept both chats"
    );
    assert_eq!(
        before.len(),
        2,
        "the launch before started both: {before:?}"
    );
    assert_eq!(
        after.len(),
        2,
        "the launch after started both again: {after:?}"
    );
    for (profile, conversation, cwd) in &chats {
        let mark = PROFILES
            .iter()
            .find(|(name, _)| name == profile)
            .map(|(_, mark)| *mark)
            .expect("a declared profile");
        let began = before
            .iter()
            .find(|started| started.mark == mark)
            .unwrap_or_else(|| panic!("{profile} started before the upgrade: {before:?}"));
        assert_eq!(
            began.after("--session-id"),
            Some(conversation.as_str()),
            "{profile}'s conversation began in the launch before"
        );
        // After the upgrade: the profile's own command, with its environment, so nothing was
        // set up again; its own conversation, resumed; its own folder.
        let back = after
            .iter()
            .find(|started| started.mark == mark)
            .unwrap_or_else(|| panic!("{profile} came back on its profile: {after:?}"));
        assert_eq!(
            back.after("--resume"),
            Some(conversation.as_str()),
            "{profile} came back on its own conversation"
        );
        assert_eq!(&back.cwd, cwd, "{profile} came back in its own folder");
    }
    // The side chat is on its branch, which holds the commit it held; the session record is
    // as it was.
    let side = &chats
        .iter()
        .find(|(profile, ..)| profile == "side")
        .expect("side")
        .2;
    assert_eq!(side, &world.worktree());
    assert_eq!(
        git(&world.worktree(), &["branch", "--show-current"]),
        BRANCH
    );
    assert_eq!(git(&world.svc(), &["rev-parse", BRANCH]), tip_before);
    assert_eq!(session_records(&world), record_before);
}

/// The world a child of this test was handed.
fn a_child_s_world() -> World {
    World {
        at: PathBuf::from(std::env::var_os(AT).expect("the test's folder")),
    }
}

/// Runs `which` launch as a child of this test, with the test's config and data homes.
fn launch(world: &World, which: &str) {
    purlis_core::testrun::rerun(
        &[THIS],
        &[
            (LAUNCH, OsStr::new(which)),
            (AT, world.at.as_os_str()),
            (purlis_core::machine::HOME_VAR, world.config().as_os_str()),
            (purlis_core::datahome::HOME_VAR, world.data().as_os_str()),
        ],
    );
}

/// The project: a workspace holding a clone with a worktree on [`BRANCH`], a session record, and
/// the two profiles of [`PROFILES`] in `charter.local.toml`, each a stand-in Claude Code that
/// writes down how it started and stays running, approved as a person approves a profile.
fn a_project_with_two_declared_profiles_a_branch_and_a_session_record(world: &World) {
    let root = world.root();
    for folder in [world.svc(), world.runs(), world.config(), world.data()] {
        std::fs::create_dir_all(folder).expect("a folder of the test's");
    }
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").expect("charter.toml");
    std::fs::write(world.svc().join("README.md"), "svc\n").expect("a file");
    git(&world.svc(), &["init", "-q", "-b", "main"]);
    git(&world.svc(), &["add", "-A"]);
    git(&world.svc(), &["commit", "-q", "-m", "one"]);
    let worktree = world.worktree().display().to_string();
    git(
        &world.svc(),
        &["worktree", "add", "-q", "-b", BRANCH, &worktree],
    );
    std::fs::write(world.worktree().join("side.md"), "side\n").expect("a file");
    git(&world.worktree(), &["add", "-A"]);
    git(&world.worktree(), &["commit", "-q", "-m", "on the side"]);

    let runs = world.runs();
    let mut declared = String::new();
    for (profile, mark) in PROFILES {
        // Outside the project, and answering as Claude Code, as a chat's start asks (#2071).
        let program = stand_in::claude_code(
            &world.at,
            &format!("claude-{profile}"),
            &format!(
                "#!/bin/sh\nat=$(mktemp {runs:?}/.writing.XXXXXX) || exit 1\n\
                 {{ printf 'mark=%s\\n' \"$UPGRADE_MARK\"; printf 'cwd=%s\\n' \"$(pwd -P)\"; \
                 for a in \"$@\"; do printf 'arg=%s\\n' \"$a\"; done; }} > \"$at\"\n\
                 mv \"$at\" {runs:?}/run.$$\nexec sleep 600\n"
            ),
        );
        declared.push_str(&format!(
            "[harness.{profile}]\nkind = \"claude\"\ncommand = [{:?}]\n\
             env = {{ UPGRADE_MARK = {mark:?} }}\n\n",
            program.display().to_string()
        ));
    }
    std::fs::write(root.join(purlis_core::profiles::LOCAL_FILE), declared).expect("the profiles");
    let set = purlis_core::profiles::current(&root);
    for (profile, _) in PROFILES {
        let declared = set.get(profile).expect("the profile reads");
        purlis_core::profiletrust::record_launched(
            &root,
            profile,
            &purlis_core::profiletrust::fingerprint(declared),
        )
        .expect("approved");
    }

    purlis_core::sessionrecord::record(
        &root,
        &purlis_core::sessionrecord::New {
            title: "Before the upgrade",
            body: "## Goal\n\ng\n\n## Done\n\nd\n\n## Decisions\n\nx\n\n## Open\n\no\n\n\
                   ## How to resume\n\nr\n",
            facts: &purlis_core::sessionrecord::Facts {
                place: purlis_core::active::Place::Workspace("alpha".to_owned()),
                at: chrono::NaiveDate::from_ymd_opt(2026, 10, 14)
                    .and_then(|day| day.and_hms_opt(16, 0, 0))
                    .expect("a time"),
                chat: None,
                persona: None,
                pieces: Vec::new(),
            },
        },
    )
    .expect("the session record is written");
}

/// Every file of the workspace's session records, by name, with what it holds.
fn session_records(world: &World) -> Vec<(String, String)> {
    let dir = world
        .root()
        .join("workspaces")
        .join("alpha")
        .join("sessions");
    let mut records: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("the session records")
        .filter_map(Result::ok)
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(entry.path()).expect("a record"),
            )
        })
        .collect();
    records.sort();
    assert!(
        !records.is_empty(),
        "no session record in {}",
        dir.display()
    );
    records
}

/// **The launch before the upgrade**: chats in the window's own process, as before #2004. A
/// chat on each profile, the `side` one in its worktree, then the quit that installs an update.
fn the_launch_before_the_upgrade(world: &World) {
    let planes = Planes::telling(
        Arc::new(|_: Moved| {}),
        crate::Shipped::default(),
        Some(world.config()),
    );
    let root = world.root();
    let held = planes
        .held(&planes.open(&root))
        .expect("the project is held");
    for (profile, _) in PROFILES {
        let cwd = if profile == "side" {
            world.worktree()
        } else {
            root.join("workspaces").join("alpha")
        };
        let ready = purlis_core::start::ready(
            &purlis_core::start::Start {
                profile: Some(profile.to_owned()),
                name: profile.to_owned(),
                cwd: Some(cwd),
                ..Default::default()
            },
            &root,
        )
        .expect("the chat is ready to start");
        let chat = Chat {
            program: ready.program.clone(),
            cwd: ready.cwd.clone(),
            name: profile.to_owned(),
            resume: ready.session.clone(),
            profile: Some(profile.to_owned()),
            ..Default::default()
        };
        held.chats()
            .start_ready(&chat, &ready, SIZE)
            .expect("the chat starts in the window");
    }
    until("both chats to start", || world.starts().len() == 2);
    planes.let_go_of_all_to_update();
}

/// **The launch after the upgrade**: chats in the session host, as this build runs them by
/// default; the person reopens every chat, and the quit at the end ends them.
fn the_launch_after_the_upgrade(world: &World) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("a runtime");
    let host_folder = world.at.join("host");
    std::fs::create_dir_all(&host_folder).expect("the host's folder");
    let host = {
        let (folder, data) = (host_folder.clone(), world.data());
        runtime.block_on(async move { crate::behaviour::a_host(&folder, &data) })
    };
    let (connection, _window) = super::tests::in_memory(host, &runtime);
    let planes = super::chats_in_the_host(
        Planes::telling(
            Arc::new(|_: Moved| {}),
            crate::Shipped::default(),
            Some(world.config()),
        ),
        &connection,
    );
    let root = world.root();
    let started_before = world.starts().len();
    let held = planes
        .held(&planes.open(&root))
        .expect("the project is held");
    held.reopen(SIZE, reopen::read_or_refusal(&root), Choice::ReopenAll);
    until("both chats to come back", || {
        world.starts().len() == started_before + 2
    });
    let mut profiles: Vec<String> = held
        .chats()
        .open_now()
        .into_iter()
        .filter_map(|open| open.profile)
        .collect();
    profiles.sort();
    assert_eq!(
        profiles,
        ["side", "work"],
        "each chat is open on its profile"
    );
    planes.let_go_of_all();
    if let Ok(link) = connection.wait(Duration::ZERO) {
        let _ = link.quit_within(Duration::from_secs(7));
    }
    drop(runtime);
}

fn until(what: &str, done: impl Fn() -> bool) {
    let began = Instant::now();
    while !done() {
        assert!(began.elapsed() < PATIENCE, "still waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// What `git` says in `dir`, trimmed, asserted to succeed.
fn git(dir: &Path, args: &[&str]) -> String {
    let mut command = std::process::Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let out = purlis_core::forklock::output(&mut command).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}
