// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::process::Command;

fn answers_of(what: crate::planewatch::What) -> Option<Vec<crate::planewatch::PlaneAnswer>> {
    crate::planewatch::PlaneChanged::of(
        serde_json::from_value(serde_json::json!("/home/dev/plane")).expect("an id"),
        what,
    )
    .answers
}

#[test]
fn a_look_that_left_the_tree_alone_tells_the_git_standings_alone() {
    // #933: a commit, a push or a fetch that brought nothing in moved the git standing
    // and no file, and used to be told as "anything may have moved".
    assert_eq!(
        answers_of(Tells::of(plane("").path()).what_it_did(false)),
        Some(vec![crate::planewatch::PlaneAnswer::Git])
    );
}

#[test]
fn a_save_that_commits_the_tree_as_it_was_has_not_moved_it() {
    let dir = plane("[plane]\nmode = \"commit\"\n");
    let mut state = State::default();
    let at = Instant::now();
    state.look(dir.path(), at, None);
    std::fs::write(dir.path().join("note.md"), "n").unwrap();

    assert!(state.look(
        dir.path(),
        at + Duration::from_secs(1),
        Some(Poke::SessionEnded)
    ));

    assert_eq!(commits(dir.path()), 1);
    assert!(
        !state.moved_tree,
        "a commit of the tree as it was moved nothing"
    );
}

#[test]
fn a_head_that_moved_past_the_saves_own_commit_moved_the_tree() {
    let dir = plane("[plane]\nmode = \"commit\"\n");
    let root = dir.path();
    let before = head_of(root);
    assert!(!tree_moved(root, before.as_deref()), "nothing happened");

    git(
        root,
        &["commit", "-q", "--allow-empty", "-m", "the save's own"],
    );
    assert!(
        !tree_moved(root, before.as_deref()),
        "one commit on top is the save's"
    );

    // What a rebase onto the remote leaves: other commits under the save's own.
    git(
        root,
        &["commit", "-q", "--allow-empty", "-m", "replayed on top"],
    );
    assert!(tree_moved(root, before.as_deref()));
    assert!(
        tree_moved(root, None),
        "a head that could not be read is not known"
    );
}

fn git(dir: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid");
    let out = purlis_core::forklock::output(&mut command).expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A plane with one commit, its own identity, and no remote.
fn plane(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("charter.toml"), toml).unwrap();
    std::fs::write(root.join(".gitignore"), ".charter/\n").unwrap();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.name", "t"]);
    git(root, &["config", "user.email", "t@example.invalid"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "one"]);
    dir
}

#[test]
fn a_fetch_that_fast_forwards_the_plane_tells_the_window_the_files_it_brought_in() {
    // #1028: the fetch end to end. `origin` is on a forge charter knows, mapped by the
    // plane's own config onto a local bare repository, never the network; somebody else's
    // commit waits there, and auto-save is on, so the look fast-forwards onto it.
    let dir = plane("[plane]\nmode = \"commit\"\n");
    let root = dir.path().canonicalize().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let elsewhere = outside.path().canonicalize().unwrap();
    let forge = elsewhere.join("forge/acme");
    std::fs::create_dir_all(&forge).unwrap();
    git(&forge, &["init", "-q", "--bare", "-b", "main", "plane.git"]);
    let bare = forge.join("plane.git").display().to_string();
    git(
        &root,
        &["remote", "add", "origin", "git@github.com:acme/plane.git"],
    );
    git(
        &root,
        &[
            "config",
            &format!("url.file://{}/.insteadOf", forge.display()),
            "https://github.com/acme/",
        ],
    );
    git(&root, &["push", "-q", &bare, "main"]);
    let theirs = elsewhere.join("theirs");
    git(
        &elsewhere,
        &["clone", "-q", &bare, &theirs.display().to_string()],
    );
    let memory = theirs.join("workspaces/alpha/memory");
    std::fs::create_dir_all(&memory).unwrap();
    std::fs::write(memory.join("theirs.md"), "theirs\n").unwrap();
    git(&theirs, &["add", "-A"]);
    git(&theirs, &["commit", "-q", "-m", "theirs"]);
    git(&theirs, &["push", "-q", "origin", "main"]);

    let tells = Tells::of(&root);
    let mut state = State::default();
    assert!(
        state.look(&root, Instant::now(), None),
        "a fetch that brought a commit in is something to tell"
    );

    assert!(
        root.join("workspaces/alpha/memory/theirs.md").is_file(),
        "the clean tree was not fast-forwarded"
    );
    assert!(state.moved_tree, "a fast-forward moved the tree");
    // What the loop tells the window: the files the fast-forward brought in, placed as the
    // watch places a write (felt speed round 4, H9). A memory moves its workspace's panels
    // and the views, and the git standing moved: never every answer, which drew every view
    // of every chat again.
    assert_eq!(
        answers_of(tells.what_it_did(state.moved_tree)),
        Some(vec![
            crate::planewatch::PlaneAnswer::Panels {
                workspace: Some("alpha".into())
            },
            crate::planewatch::PlaneAnswer::Git,
            crate::planewatch::PlaneAnswer::Views,
        ])
    );
}

#[test]
fn a_tree_that_moved_with_the_planes_head_where_it_was_tells_every_reader() {
    // A workspace repo's merge moved files the plane's history does not hold: not known.
    let dir = plane("[plane]\nmode = \"commit\"\n");
    let tells = Tells::of(dir.path());
    assert_eq!(answers_of(tells.what_it_did(true)), None);
}

#[test]
fn a_save_after_a_fast_forward_tells_the_git_standing_alone() {
    let dir = plane("[plane]\nmode = \"commit\"\n");
    let root = dir.path();
    let tells = Tells::of(root);
    std::fs::create_dir_all(root.join("workspaces/alpha/todos")).unwrap();
    std::fs::write(root.join("workspaces/alpha/todos/one.md"), "t\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "a todo"]);
    assert_ne!(answers_of(tells.what_it_did(true)), None);

    // What was told is not told again.
    assert_eq!(
        answers_of(tells.what_it_did(false)),
        Some(vec![crate::planewatch::PlaneAnswer::Git])
    );
}

fn commits(root: &Path) -> usize {
    planegit::journal(root)
        .iter()
        .filter(|l| l["outcome"] == "committed")
        .count()
}

#[test]
fn a_chat_that_ends_saves_the_plane_at_once_when_auto_save_is_on() {
    let dir = plane("[plane]\nmode = \"commit\"\n");
    let mut state = State::default();
    let at = Instant::now();
    state.look(dir.path(), at, None);
    std::fs::write(dir.path().join("note.md"), "n").unwrap();

    assert!(state.look(
        dir.path(),
        at + Duration::from_secs(1),
        Some(Poke::SessionEnded)
    ));

    let last = &planegit::journal(dir.path())[0];
    assert_eq!(
        (last["trigger"].as_str(), last["outcome"].as_str()),
        (Some("session-end"), Some("committed"))
    );
}

#[test]
fn a_quiet_plane_is_saved_after_its_quiet_period_and_not_before() {
    let dir = plane("[plane]\nmode = \"commit\"\nautosave_after = \"30s\"\n");
    let mut state = State::default();
    let at = Instant::now();
    state.look(dir.path(), at, None);
    std::fs::write(dir.path().join("note.md"), "n").unwrap();
    // The plane's watcher seeing the write (FD-11): the shared standing is read again.
    planegit::touch(dir.path());

    assert!(!state.look(dir.path(), at + Duration::from_secs(2), None));
    assert!(!state.look(dir.path(), at + Duration::from_secs(31), None));
    assert_eq!(commits(dir.path()), 0);
    assert!(state.look(dir.path(), at + Duration::from_secs(32), None));
    assert_eq!(commits(dir.path()), 1);
}

#[test]
fn a_file_the_shared_standing_has_not_seen_yet_restarts_the_quiet_period() {
    // FD-11: the shared standing may be up to ten seconds behind a new file, so the save a
    // quiet period earns is checked against the tree as it is first.
    let dir = plane("[plane]\nmode = \"commit\"\nautosave_after = \"30s\"\n");
    let mut state = State::default();
    let at = Instant::now();
    state.look(dir.path(), at, None);
    std::fs::write(dir.path().join("note.md"), "n").unwrap();
    planegit::touch(dir.path());
    assert!(!state.look(dir.path(), at + Duration::from_secs(2), None));

    // Written a moment before the quiet period ends, and not yet in the shared standing.
    std::fs::write(dir.path().join("late.md"), "l").unwrap();

    assert!(!state.look(dir.path(), at + Duration::from_secs(33), None));
    assert_eq!(commits(dir.path()), 0, "a file seconds old was saved");
    assert!(state.look(dir.path(), at + Duration::from_secs(64), None));
    assert_eq!(commits(dir.path()), 1);
}

#[test]
fn a_plane_nobody_has_chosen_a_mode_for_is_never_saved_by_itself() {
    let dir = plane("[memory]\nshare = \"local\"\n");
    let mut state = State::default();
    let at = Instant::now();
    state.look(dir.path(), at, None);
    std::fs::write(dir.path().join("note.md"), "n").unwrap();
    for s in [2, 40, 600] {
        state.look(dir.path(), at + Duration::from_secs(s), None);
    }
    state.look(
        dir.path(),
        at + Duration::from_secs(601),
        Some(Poke::SessionEnded),
    );
    assert!(planegit::journal(dir.path()).is_empty());
}

/// A plane saying `toml`, with a workspace `alpha` holding a clone `widget`.
fn plane_with_a_repo(toml: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = plane(toml);
    let clone = dir.path().join("workspaces/alpha/widget");
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q", "-b", "main"]);
    git(&clone, &["config", "user.name", "t"]);
    git(&clone, &["config", "user.email", "t@example.invalid"]);
    std::fs::write(clone.join("README.md"), "one").unwrap();
    git(&clone, &["add", "-A"]);
    git(&clone, &["commit", "-q", "-m", "one"]);
    (dir, clone)
}

fn repo_commits(root: &Path) -> usize {
    planegit::journal(root)
        .iter()
        .filter(|l| l["target"] == "repo:alpha/widget" && l["outcome"] == "committed")
        .count()
}

#[test]
fn a_repo_with_auto_save_on_is_saved_after_its_quiet_period_but_never_mid_turn() {
    let (dir, clone) = plane_with_a_repo(
        "[repos.widget]\nmode = \"commit\"\nautosave = true\nautosave_after = \"30s\"\n",
    );
    let root = dir.path();
    let mut state = State::default();
    let at = Instant::now();
    let working = |_: &str| vec!["alpha.1".to_owned()];
    let idle = |_: &str| Vec::new();
    state.look_at_repos(root, at, None, &idle);
    std::fs::write(clone.join("a.md"), "a").unwrap();
    // Told, as the watcher or the shared standing's age tells it in the app (FD-11).
    planegit::touch(root);

    // A chat in alpha is mid-turn: every cycle is skipped, however long it runs.
    for s in [1, 31, 90, 300] {
        assert!(!state.look_at_repos(root, at + Duration::from_secs(s), None, &working));
    }
    assert!(
        !state.look_at_repos(
            root,
            at + Duration::from_secs(301),
            Some(Poke::SessionEnded),
            &working
        ),
        "a chat ending elsewhere while one here still works is not a reason"
    );
    assert_eq!(repo_commits(root), 0);

    // The turn ends; the quiet period starts then.
    assert!(!state.look_at_repos(root, at + Duration::from_secs(302), None, &idle));
    assert!(!state.look_at_repos(root, at + Duration::from_secs(331), None, &idle));
    assert!(state.look_at_repos(root, at + Duration::from_secs(332), None, &idle));
    assert_eq!(repo_commits(root), 1);
}

#[test]
fn a_file_in_a_repo_the_shared_standing_has_not_seen_restarts_its_quiet_period() {
    let (dir, clone) = plane_with_a_repo(
        "[repos.widget]\nmode = \"commit\"\nautosave = true\nautosave_after = \"30s\"\n",
    );
    let root = dir.path();
    let mut state = State::default();
    let at = Instant::now();
    let idle = |_: &str| Vec::new();
    state.look_at_repos(root, at, None, &idle);
    std::fs::write(clone.join("a.md"), "a").unwrap();
    planegit::touch(root);
    assert!(!state.look_at_repos(root, at + Duration::from_secs(1), None, &idle));

    // A new file, not yet in the shared standing, a moment before the period ends.
    std::fs::write(clone.join("late.md"), "l").unwrap();

    assert!(!state.look_at_repos(root, at + Duration::from_secs(32), None, &idle));
    assert_eq!(repo_commits(root), 0, "a file seconds old was saved");
    assert!(state.look_at_repos(root, at + Duration::from_secs(63), None, &idle));
    assert_eq!(repo_commits(root), 1);
}

#[test]
fn a_turn_that_starts_during_a_repo_auto_save_is_seen_right_before_it_stages() {
    let (dir, clone) = plane_with_a_repo(
        "[repos.widget]\nmode = \"commit\"\nautosave = true\nautosave_after = \"30s\"\n",
    );
    let root = dir.path();
    let mut state = State::default();
    let at = Instant::now();
    let idle = |_: &str| Vec::new();
    state.look_at_repos(root, at, None, &idle);
    std::fs::write(clone.join("a.md"), "a").unwrap();
    // Told, as the watcher or the shared standing's age tells it in the app (FD-11).
    planegit::touch(root);
    state.look_at_repos(root, at + Duration::from_secs(1), None, &idle);

    // Nobody at this look; somebody by the time the save asks again.
    let asked = std::cell::Cell::new(0);
    let starting = |_: &str| {
        asked.set(asked.get() + 1);
        if asked.get() > 2 {
            vec!["alpha.1".to_owned()]
        } else {
            Vec::new()
        }
    };
    state.look_at_repos(root, at + Duration::from_secs(40), None, &starting);

    assert_eq!(
        repo_commits(root),
        0,
        "a turn's half-written file was committed"
    );
    let line = planegit::journal(root).pop().expect("the skipped save");
    assert_eq!(line["outcome"], "skipped");
}

#[test]
fn a_repo_whose_table_leaves_auto_save_off_is_never_saved_by_itself() {
    let (dir, clone) = plane_with_a_repo("[repos.widget]\nmode = \"commit\"\n");
    let root = dir.path();
    let mut state = State::default();
    let at = Instant::now();
    let idle = |_: &str| Vec::new();
    std::fs::write(clone.join("a.md"), "a").unwrap();
    for s in [0, 31, 62, 400] {
        state.look_at_repos(root, at + Duration::from_secs(s), None, &idle);
    }
    state.look_at_repos(
        root,
        at + Duration::from_secs(401),
        Some(Poke::SessionEnded),
        &idle,
    );
    assert_eq!(repo_commits(root), 0);
}

/// A saver whose stop is answered at once, or never.
struct Stops {
    answers: bool,
}

impl Saver for Stops {
    fn poker(&self) -> Sender<Poke> {
        mpsc::channel().0
    }

    fn asks_mid_turn_of(&self, _ask: MidTurn) {}

    fn stop_by(self: Box<Self>, deadline: Instant) -> bool {
        if !self.answers {
            std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
        }
        self.answers
    }
}

#[test]
fn a_quit_stops_every_saver_at_once_and_ends_a_host_that_did_not_answer() {
    let ended = std::sync::atomic::AtomicUsize::new(0);
    let started = Instant::now();
    let deadline = started + Duration::from_millis(400);

    stop_every_saver_by(
        vec![
            Box::new(Stops { answers: false }),
            Box::new(Stops { answers: true }),
            Box::new(Stops { answers: false }),
        ],
        &[],
        deadline,
        &|| {
            ended.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        },
    );

    assert_eq!(ended.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(
        started.elapsed() < Duration::from_millis(750),
        "the savers were stopped one after another: {:?}",
        started.elapsed()
    );
}

#[test]
fn a_quit_whose_savers_all_stopped_leaves_the_host_alone() {
    let ended = std::sync::atomic::AtomicBool::new(false);

    stop_every_saver_by(
        vec![
            Box::new(Stops { answers: true }),
            Box::new(Stops { answers: true }),
        ],
        &[],
        Instant::now() + Duration::from_millis(400),
        &|| ended.store(true, std::sync::atomic::Ordering::SeqCst),
    );

    assert!(!ended.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn a_worker_let_go_of_stops_even_while_a_chats_end_still_holds_its_sender() {
    let dir = plane("[plane]\nmode = \"commit\"\n");
    let worker = Worker::start(dir.path().to_path_buf(), Arc::new(|_| {}));
    let held_by_a_chat = worker.poker();

    assert!(
        worker.stop_within(Duration::from_secs(5)),
        "the worker kept looking"
    );
    drop(held_by_a_chat);
}
