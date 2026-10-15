// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use purlis_host::{Hosting, Working};
use purlis_session_protocol::auth::{Credentials, Scope};
use purlis_session_protocol::link;
use purlis_session_protocol::session::{self, Client};

use super::*;

const PROJECT: &str = "01J9PRJCT00000000000000000";

static HELD: std::sync::LazyLock<Credentials> =
    std::sync::LazyLock::new(|| Credentials::mint().expect("credentials"));

fn git(dir: &Path, args: &[&str]) {
    let mut command = std::process::Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1");
    let out = purlis_core::forklock::output(&mut command).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A plane that commits by itself after a second of quiet, with one commit and no remote.
fn a_plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a plane");
    let root = dir.path().canonicalize().expect("it resolves");
    std::fs::write(
        root.join(purlis_core::plane::MANIFEST),
        "[plane]\nmode = \"commit\"\nautosave_after = \"1s\"\n",
    )
    .expect("its settings");
    std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").expect("its ignores");
    git(&root, &["init", "-q", "-b", "main"]);
    git(&root, &["config", "user.name", "t"]);
    git(&root, &["config", "user.email", "t@example.invalid"]);
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-q", "-m", "one"]);
    (dir, root)
}

/// A window: a `local-ui` link to `hosting`, the project at `root` held on it and saved.
async fn a_window_saving(hosting: &Hosting, root: &Path) -> Client {
    let (a, b) = tokio::io::duplex(256 * 1024);
    let (client, served) = tokio::join!(
        link::connect(
            a,
            session::speaks(),
            Scope::LocalUi,
            HELD.of(Scope::LocalUi)
        ),
        link::serve_any(b, session::speaks(), &HELD)
    );
    tokio::spawn(session::serve(
        served.expect("the host admits the window"),
        hosting.clone(),
        None,
    ));
    let (client, _views, _events) = Client::new(client.expect("the window is admitted"));
    client
        .hold_project(PROJECT, root.to_str().expect("a path"))
        .await
        .expect("the project is held");
    client.start_saving(PROJECT).await.expect("it is saved");
    client
}

fn committed(root: &Path) -> usize {
    purlis_core::planegit::journal(root)
        .iter()
        .filter(|line| line["outcome"] == "committed")
        .count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_change_made_while_the_window_is_gone_without_a_quit_is_saved_by_the_host() {
    let (_dir, root) = a_plane();
    let hosting = Hosting::new(purlis_host::ComposesNothing).saving_with(InTheHost);
    let window = a_window_saving(&hosting, &root).await;

    // The window goes as a crash takes it: its link ends, with no stop and no let go.
    drop(window);
    tokio::time::sleep(Duration::from_millis(200)).await;
    std::fs::write(root.join("note.md"), "written while the window was gone\n").expect("written");

    let deadline = Instant::now() + Duration::from_secs(60);
    while committed(&root) == 0 {
        assert!(
            Instant::now() < deadline,
            "the host did not save what was written while the window was gone"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // The window comes back and asks again: the same saver goes on, and no second one starts.
    let again = a_window_saving(&hosting, &root).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(crate::autosave::running::on(&root), 1);
    again.stop_saving(PROJECT).await.expect("it stops");
    let deadline = Instant::now() + Duration::from_secs(20);
    while crate::autosave::running::on(&root) != 0 {
        assert!(Instant::now() < deadline, "the host's saver did not stop");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[test]
fn every_program_that_could_be_writing_a_workspace_counts_as_mid_turn_in_it() {
    let dir = tempfile::tempdir().expect("a plane");
    let root = dir.path().canonicalize().expect("it resolves");
    std::fs::write(root.join(purlis_core::plane::MANIFEST), "").expect("a manifest");
    for workspace in ["alpha", "beta"] {
        std::fs::create_dir_all(root.join("workspaces").join(workspace)).expect("a workspace");
    }
    let working = [
        Working {
            number: 1,
            cwd: Some(root.join("workspaces/alpha")),
        },
        Working {
            number: 2,
            cwd: Some(root.join("workspaces/beta")),
        },
        // At the plane root: in no other workspace, so it could be writing any.
        Working {
            number: 3,
            cwd: Some(root.clone()),
        },
        // Where it started is not known: it could be anywhere.
        Working {
            number: 4,
            cwd: None,
        },
    ];

    assert_eq!(
        mid_turn_in(&root, "alpha", &working),
        ["chat 1", "chat 3", "chat 4"]
    );
    assert_eq!(
        mid_turn_in(&root, "beta", &working),
        ["chat 2", "chat 3", "chat 4"]
    );
}

#[test]
fn a_stop_that_arrives_mid_save_returns_once_the_save_has_let_go_of_the_repo() {
    let (_dir, root) = a_plane();
    std::fs::write(
        root.join(purlis_core::plane::MANIFEST),
        "[repos.widget]\nmode = \"commit\"\nautosave = true\nautosave_after = \"1s\"\n",
    )
    .expect("its settings");
    let clone = root.join("workspaces/alpha/widget");
    std::fs::create_dir_all(&clone).expect("a workspace");
    git(&clone, &["init", "-q", "-b", "main"]);
    git(&clone, &["config", "user.name", "t"]);
    git(&clone, &["config", "user.email", "t@example.invalid"]);
    std::fs::write(clone.join("README.md"), "one").expect("written");
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "one"]);

    // The save asks who is mid-turn right before it stages, holding the repo's claim: there it
    // waits until the test lets it go on.
    let (mid_save, in_the_save) = std::sync::mpsc::channel();
    let go_on = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let saving = {
        let clone = clone.clone();
        let go_on = std::sync::Arc::clone(&go_on);
        let mid_save = std::sync::Mutex::new(mid_save);
        InTheHost
            .save(ToSave {
                project: PROJECT.into(),
                project_path: root.clone(),
                told: Box::new(|_| {}),
                working: Box::new(Vec::new),
                ask_the_window: Box::new(move |_workspace| {
                    if purlis_core::planegit::Claim::of(&clone).is_none() {
                        let _ = mid_save.lock().expect("the signal").send(());
                        while !go_on.load(std::sync::atomic::Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                    Some(Vec::new())
                }),
            })
            .expect("it saves")
    };
    std::fs::write(clone.join("a.md"), "a").expect("written");
    purlis_core::planegit::touch(&root);
    in_the_save
        .recv_timeout(Duration::from_secs(60))
        .expect("the repo's save never started");

    let stopping = std::thread::spawn(move || drop(saving));
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !stopping.is_finished(),
        "the stop returned while a save ran"
    );
    assert!(
        purlis_core::planegit::Claim::of(&clone).is_none(),
        "the save let go of the repo before it was done"
    );

    go_on.store(true, std::sync::atomic::Ordering::SeqCst);
    stopping.join().expect("it stops");
    assert!(
        purlis_core::planegit::Claim::of(&clone).is_some(),
        "the stop returned before the save let go of the repo"
    );
}
