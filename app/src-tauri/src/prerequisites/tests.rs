// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use super::*;

const OPEN: &str = "[policy]\ntemplate = \"open\"\n";
const WATCH: &str = "[policy]\ntemplate = \"watch\"\n";

/// A plane whose settings file says `committed`.
fn plane(committed: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(dir.path().join("charter.toml"), committed).expect("the settings file");
    dir
}

fn plane_id(root: &str) -> PlaneId {
    serde_json::from_value(serde_json::json!(root)).expect("a plane id")
}

/// Waits up to ten seconds for `done`.
fn until(what: &str, done: impl Fn() -> bool) {
    let by = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < by, "never: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A quiet spell longer than a burst and its evaluation, after which nothing more is coming.
fn settled() {
    std::thread::sleep(QUIET_FOR * 4);
}

/// A watcher's settings for a test: the poller, where it is one, looks every 50 ms.
fn looking_often() -> notify::Config {
    notify::Config::default().with_poll_interval(Duration::from_millis(50))
}

#[test]
fn every_window_hears_a_projects_list_whichever_project_it_shows() {
    // ADR 0130 §3, §8: a project not in front shows a dot on its tab, and an answer in one
    // window closes the modal in each, so the list goes to every window.
    use tauri::{Listener, Manager};
    let app = tauri::test::mock_builder()
        .build(tauri_context!(test = true))
        .expect("the app builds");
    app.manage(crate::planes::Showing::default());
    let window = |label: &str| {
        tauri::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::default())
            .build()
            .expect("a window")
    };
    let windows = [window("main"), window("split-1")];
    let ours = plane_id("/projects/ours");
    app.state::<crate::planes::Showing>().in_window(
        "split-1",
        crate::planes::Holding {
            planes: vec![plane_id("/projects/other")],
            active: Some(0),
        },
    );
    let counts: [Arc<AtomicUsize>; 2] = Default::default();
    for (window, count) in windows.iter().zip(counts.iter()) {
        let count = Arc::clone(count);
        window.listen(EVENT, move |_| {
            count.fetch_add(1, Ordering::SeqCst);
        });
    }
    to_every_window(app.handle().clone())(Prerequisites {
        plane: ours,
        list: Vec::new(),
    });
    assert_eq!(
        counts.each_ref().map(|count| count.load(Ordering::SeqCst)),
        [1, 1]
    );
}

#[test]
fn an_answer_from_one_windows_context_is_followed_by_the_event_and_the_read_returns_the_new_list() {
    // ADR 0130 §2: every answer is made through the held list, which is evaluated again after
    // it and told whole. A test is not the app, which alone seals an Accept, so the answer here
    // is one refused because the file changed since it was shown: the list changed with it.
    let root = plane(OPEN);
    // Its folder is not watched, so the change below is heard by nothing but the answer.
    let real = std::fs::canonicalize(root.path()).expect("real");
    crate::watchset::refuse::refuse(&real);
    let heard: Arc<Mutex<Vec<Prerequisites>>> = Arc::default();
    let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
        .telling_prerequisites({
            let heard = Arc::clone(&heard);
            Arc::new(move |told| heard.lock().expect("heard").push(told))
        });
    let id = planes.open(root.path());
    let held = planes.held(&id).expect("held");
    until("the open's list", || {
        !heard.lock().expect("heard").is_empty()
    });
    let shown = heard.lock().expect("heard").remove(0);
    assert_eq!(shown.plane, id);
    let [Prerequisite::Policy(policy)] = shown.list.as_slice() else {
        panic!("the policy prerequisite alone: {shown:?}");
    };
    assert_eq!(policy.reasons, vec![PolicyReason::Waiting]);
    assert_eq!(policy.state, PrerequisiteState::Open);
    assert!(!policy.policy.waiting.is_empty(), "the rows: {policy:?}");

    std::fs::write(root.path().join("charter.toml"), WATCH).expect("tightened");
    let why =
        crate::policypress::pressed(root.path(), &held.prerequisites(), &policy.digest, Vec::new)
            .expect_err("not what was shown");
    assert!(why.contains("changed since it was shown"), "{why}");
    until("the event after the answer", || {
        !heard.lock().expect("heard").is_empty()
    });
    settled();
    let told = std::mem::take(&mut *heard.lock().expect("heard"));
    assert_eq!(
        told,
        vec![Prerequisites {
            plane: id.clone(),
            list: Vec::new(),
        }],
        "one event, the whole list"
    );
    let read = Prerequisites::of(id.clone(), &held.prerequisites().current(root.path()));
    crate::watchset::refuse::allow(&real);
    assert!(read.list.is_empty(), "{read:?}");
}

/// What a watch did, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Did {
    /// Said whether a folder it needs is not watched.
    Lapsed(bool),
    /// Evaluated the list.
    Evaluated,
}

#[test]
fn a_folder_the_platform_would_not_watch_is_reported_and_followed_again_before_evaluating() {
    // #7885's review, F6 and F8: a folder refused is said, so the list is read at each ask, and
    // every burst follows the folders again before it evaluates, so what it evaluates from is
    // what is watched.
    let (refused, beside) = (
        tempfile::tempdir().expect("a folder"),
        tempfile::tempdir().expect("a folder"),
    );
    let real = std::fs::canonicalize(refused.path()).expect("real");
    crate::watchset::refuse::refuse(&real);
    let did: Arc<Mutex<Vec<Did>>> = Arc::default();
    let doing = |what: Did| {
        let did = Arc::clone(&did);
        move || did.lock().expect("did").push(what)
    };
    let _watch = Watch::<Platform>::start_on(
        {
            let files = vec![
                refused.path().join("charter.toml"),
                beside.path().join("charter.toml"),
            ];
            move || files.clone()
        },
        doing(Did::Evaluated),
        {
            let did = Arc::clone(&did);
            move |lapsed| did.lock().expect("did").push(Did::Lapsed(lapsed))
        },
        looking_often(),
    )
    .expect("watched");
    until("the open", || {
        did.lock().expect("did").contains(&Did::Evaluated)
    });
    crate::watchset::refuse::allow(&real);
    std::fs::write(beside.path().join("charter.toml"), OPEN).expect("written");
    until("the burst", || {
        did.lock()
            .expect("did")
            .iter()
            .filter(|what| **what == Did::Evaluated)
            .count()
            >= 2
    });
    let did = did.lock().expect("did").clone();
    assert_eq!(
        did[..4],
        [
            Did::Lapsed(true),
            Did::Evaluated,
            Did::Lapsed(false),
            Did::Evaluated
        ],
        "{did:?}"
    );
}

/// A real folder, a file of the list in it and one in a folder not there yet, the folder
/// followed, as [`Watch`] holds them: what [`concerns`] is asked against.
struct Followed {
    _dir: tempfile::TempDir,
    folder: PathBuf,
    files: Vec<PathBuf>,
    watched: HashSet<PathBuf>,
}

fn followed() -> Followed {
    let dir = tempfile::tempdir().expect("a folder");
    let folder = std::fs::canonicalize(dir.path()).expect("real");
    let files = vec![
        folder.join("charter.toml"),
        folder.join("etc").join("purlis").join("policy.json"),
    ];
    let watched = folders_of(&files);
    assert_eq!(watched, HashSet::from([folder.clone()]));
    Followed {
        _dir: dir,
        folder,
        files,
        watched,
    }
}

/// A burst naming `paths`, of which `removed` a removal named.
fn burst(paths: &[&Path], removed: &[&Path]) -> crate::watchset::Burst {
    crate::watchset::Burst {
        paths: paths.iter().map(|path| path.to_path_buf()).collect(),
        removed: removed.iter().map(|path| path.to_path_buf()).collect(),
        ..Default::default()
    }
}

#[test]
fn a_change_to_a_file_of_the_list_concerns_it() {
    let at = followed();
    let file = &at.files[0];
    assert!(concerns(&at.files, &at.watched, &burst(&[file], &[]), file));
}

#[test]
fn a_followed_folder_that_is_still_there_concerns_nothing() {
    // #7885's review, round 3: a poller sees the folder's time move with a write beside the
    // file, and a change to the file names the file.
    let at = followed();
    let beside = at.folder.join("notes.md");
    let burst = burst(&[&beside, &at.folder], &[]);
    assert!(!concerns(&at.files, &at.watched, &burst, &at.folder));
    assert!(!concerns(&at.files, &at.watched, &burst, &beside));
    assert!(gone(&at.watched, &burst).is_empty());
}

#[test]
fn a_folder_made_on_the_way_to_a_file_of_the_list_concerns_it() {
    let at = followed();
    let made = at.folder.join("etc");
    std::fs::create_dir(&made).expect("made");
    assert!(concerns(
        &at.files,
        &at.watched,
        &burst(&[&made], &[]),
        &made
    ));
}

#[test]
fn a_followed_folder_a_removal_names_concerns_it_and_is_gone() {
    let at = followed();
    let burst = burst(&[&at.folder], &[&at.folder]);
    assert!(concerns(&at.files, &at.watched, &burst, &at.folder));
    assert_eq!(gone(&at.watched, &burst), vec![at.folder.clone()]);
}

#[test]
fn a_followed_folder_no_longer_a_folder_concerns_it_and_is_gone() {
    // Moved away with no removal named: what is at its path now is no folder.
    let at = followed();
    let moved = at.folder.with_extension("moved");
    std::fs::rename(&at.folder, &moved).expect("moved");
    let burst = burst(&[&at.folder], &[]);
    let concerned = concerns(&at.files, &at.watched, &burst, &at.folder);
    let went = gone(&at.watched, &burst);
    std::fs::rename(&moved, &at.folder).expect("moved back");
    assert!(concerned);
    assert_eq!(went, vec![at.folder.clone()]);
}

#[test]
fn a_change_to_a_watched_file_evaluates_again_and_one_beside_it_does_not() {
    let dir = tempfile::tempdir().expect("a folder");
    let file = dir.path().join("charter.toml");
    // Its folder is not there yet: its coming is heard.
    let deeper = dir.path().join("etc").join("purlis").join("policy.json");
    let evaluated = Arc::new(AtomicUsize::new(0));
    let _watch = Watch::<Platform>::start_on(
        {
            let files = vec![file.clone(), deeper.clone()];
            move || files.clone()
        },
        {
            let evaluated = Arc::clone(&evaluated);
            move || {
                evaluated.fetch_add(1, Ordering::SeqCst);
            }
        },
        |_| {},
        looking_often(),
    )
    .expect("watched");
    until("the open", || evaluated.load(Ordering::SeqCst) == 1);
    settled();

    std::fs::write(dir.path().join("notes.md"), "beside it").expect("written");
    settled();
    assert_eq!(
        evaluated.load(Ordering::SeqCst),
        1,
        "a file nothing is read from"
    );

    std::fs::write(&file, OPEN).expect("written");
    until("the settings file", || {
        evaluated.load(Ordering::SeqCst) == 2
    });
    settled();

    std::fs::create_dir_all(deeper.parent().expect("a folder")).expect("made");
    until("its folder's coming", || {
        evaluated.load(Ordering::SeqCst) >= 3
    });
    settled();
    let before = evaluated.load(Ordering::SeqCst);
    std::fs::write(&deeper, "{}").expect("written");
    until("the file in the folder that came", || {
        evaluated.load(Ordering::SeqCst) > before
    });
}

#[test]
fn a_commit_at_the_plane_root_evaluates_again() {
    let root = plane(WATCH);
    let git = |args: &[&str]| {
        let mut git = std::process::Command::new("git");
        git.args(args)
            .current_dir(root.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null");
        let ran = purlis_core::forklock::output(&mut git).expect("git runs");
        assert!(ran.status.success(), "{args:?}: {ran:?}");
    };
    let commit = |message: &str| {
        git(&["add", "charter.toml"]);
        git(&[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "-m",
            message,
        ]);
    };
    git(&["init", "-q"]);
    commit("watch");
    let evaluated = Arc::new(AtomicUsize::new(0));
    let at = root.path().to_path_buf();
    let _watch = Watch::<Platform>::start_on(
        move || purlis_core::prerequisites::watched(&at),
        {
            let evaluated = Arc::clone(&evaluated);
            move || {
                evaluated.fetch_add(1, Ordering::SeqCst);
            }
        },
        |_| {},
        looking_often(),
    )
    .expect("watched");
    until("the open", || evaluated.load(Ordering::SeqCst) == 1);
    // The working tree's file moves first; the commit is what moves `HEAD`.
    std::fs::write(root.path().join("charter.toml"), OPEN).expect("loosened");
    until("the file", || evaluated.load(Ordering::SeqCst) >= 2);
    settled();
    let before = evaluated.load(Ordering::SeqCst);
    commit("open");
    until("the commit", || evaluated.load(Ordering::SeqCst) > before);
}

#[test]
fn the_answer_that_grants_nothing_marks_the_entry_deferred_releases_a_held_start_and_is_told() {
    // ADR 0130 §5, §6: the core holds the person's start; the window's answer that grants
    // nothing releases it, and every window hears the entry deferred. Waiting here: a test is
    // not the app, which alone seals an acceptance, so Broken cannot be made.
    let root = plane(OPEN);
    let heard: Arc<Mutex<Vec<Prerequisites>>> = Arc::default();
    let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None)
        .telling_prerequisites({
            let heard = Arc::clone(&heard);
            Arc::new(move |told| heard.lock().expect("heard").push(told))
        });
    let id = planes.open(root.path());
    let held = planes.held(&id).expect("held");
    until("the open's list", || {
        !heard.lock().expect("heard").is_empty()
    });
    let shown = heard.lock().expect("heard").remove(0);
    let [Prerequisite::Policy(policy)] = shown.list.as_slice() else {
        panic!("the policy prerequisite alone: {shown:?}");
    };
    assert_eq!(policy.state, PrerequisiteState::Open);

    let start = {
        let held = Arc::clone(&held);
        std::thread::spawn(move || {
            hold_start(
                &held,
                &purlis_core::sandbox::Made::Window,
                &std::sync::atomic::AtomicBool::new(false),
            )
        })
    };
    std::thread::sleep(Duration::from_millis(300));
    assert!(!start.is_finished(), "the person's start waits");

    let answered = deferred(
        id.clone(),
        root.path(),
        &held.prerequisites(),
        ShownPrerequisite::Policy {
            digest: policy.digest.clone(),
            reasons: policy.reasons.clone(),
        },
    );
    let [Prerequisite::Policy(after)] = answered.list.as_slice() else {
        panic!("the policy prerequisite alone: {answered:?}");
    };
    assert_eq!(after.state, PrerequisiteState::Deferred);
    until("the held start released", || start.is_finished());
    assert_eq!(
        start.join().expect("the hold ends"),
        Ok(Passed::AfterTheirAnswer)
    );
    until("the event after the answer", || {
        heard.lock().expect("heard").iter().any(|told| {
            matches!(
                told.list.as_slice(),
                [Prerequisite::Policy(PolicyPrerequisite {
                    state: PrerequisiteState::Deferred,
                    ..
                })]
            )
        })
    });
}

#[test]
fn the_persons_cancel_ends_a_held_start_saying_what_is_still_open() {
    let root = plane(OPEN);
    let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None);
    let id = planes.open(root.path());
    let held = planes.held(&id).expect("held");
    let why = hold_start(
        &held,
        &purlis_core::sandbox::Made::Window,
        &std::sync::atomic::AtomicBool::new(true),
    )
    .expect_err("cancelled");
    assert!(
        why.starts_with("purlis did not start the chat: ")
            && why.contains("'s policy needs your answer in the purlis window on this machine"),
        "{why}"
    );
}

/// The size the starts below are drawn at.
const SIZE: purlis_core::engine::Size = purlis_core::engine::Size {
    columns: 80,
    rows: 24,
};

/// A project whose policy prerequisite is open, held.
fn an_open_project() -> (tempfile::TempDir, Planes, PlaneId, Arc<crate::planes::Held>) {
    let root = plane(OPEN);
    let planes = Planes::telling(Arc::new(|_| {}), crate::Shipped::default(), None);
    let id = planes.open(root.path());
    let held = planes.held(&id).expect("held");
    (root, planes, id, held)
}

/// Runs the person's start `what` on another thread, in the project `held` holds, whose policy
/// prerequisite is open: it waits, and the person's Cancel (`cancel`) ends it saying what is
/// still open. Each start is given what its own checks refuse at once, so a start that read or
/// checked anything before it waited would end before it was cancelled, with another sentence.
fn holds_before_its_checks(
    held: &Arc<crate::planes::Held>,
    what: &str,
    cancel: impl Fn(),
    start: impl FnOnce(&crate::planes::Held) -> Result<(), String> + Send + 'static,
) {
    let started = {
        let held = Arc::clone(held);
        std::thread::spawn(move || start(&held))
    };
    std::thread::sleep(Duration::from_millis(300));
    assert!(!started.is_finished(), "{what} waits");
    until(what, || {
        cancel();
        started.is_finished()
    });
    let why = started
        .join()
        .expect("the start ends")
        .expect_err("cancelled");
    assert!(
        why.starts_with("purlis did not start the chat: ")
            && why.contains("'s policy needs your answer in the purlis window on this machine"),
        "{what}: {why}"
    );
}

#[test]
fn every_start_the_person_presses_holds_before_its_checks_while_a_prerequisite_is_open() {
    // The person's ruling of 2026-11-27 (#7950): every start they press is held, and holds
    // before its checks, so what it is readied under is what is there once they answer.
    let (_root, _planes, id, held) = an_open_project();
    let cancel = || crate::hostedsessions::cancel_starts(held.root());
    holds_before_its_checks(&held, "Reopen of a closed chat", cancel, |held| {
        crate::reopened_closed(held, "01K6CHATS0000000000000000D", SIZE).map(|_| ())
    });
    holds_before_its_checks(&held, "Resume of an ended chat", cancel, |held| {
        crate::resumed_ended(held, 999, SIZE).map(|_| ())
    });
    holds_before_its_checks(&held, "Retry now", cancel, |held| {
        crate::retried(held, "no such chat", SIZE).map(|_| ())
    });
    holds_before_its_checks(&held, "Start fresh", cancel, |held| {
        crate::started_fresh(held, 999, SIZE).map(|_| ())
    });
    holds_before_its_checks(&held, "Restart", cancel, |held| {
        crate::restarted(held, 999, SIZE, crate::planes::When::Now).map(|_| ())
    });
    holds_before_its_checks(&held, "Restart at a pause", cancel, |held| {
        crate::restarted(held, 999, SIZE, crate::planes::When::AtAPause).map(|_| ())
    });
    holds_before_its_checks(&held, "restart without the sandbox", cancel, |held| {
        crate::restarted_without_sandbox(held, 999, SIZE).map(|_| ())
    });
    holds_before_its_checks(&held, "a program the person named", cancel, |held| {
        let chat = crate::unprofiled(
            Some("/no/such/program".to_owned()),
            Vec::new(),
            Some(PathBuf::from("/no/such/folder")),
            "named".to_owned(),
        );
        crate::opened_session(held, &chat, true, SIZE).map(|_| ())
    });
    holds_before_its_checks(&held, "Reopen of a finished task", cancel, |held| {
        crate::finished::reopen(held, "no-such-task", SIZE).map(|_| ())
    });
    holds_before_its_checks(&held, "the person's ask from a tab", cancel, move |held| {
        let asked = crate::handoff::PersonAsk {
            chat: 999,
            persona: String::new(),
            name: String::new(),
            ask: String::new(),
            place: None,
        };
        crate::handoff::asked_from_a_tab(held, &id, &asked, SIZE).map(|_| ())
    });
}

#[cfg(unix)]
#[test]
fn try_it_holds_before_it_reads_the_pool_and_its_own_cancel_ends_the_wait() {
    let (_root, _planes, _id, held) = an_open_project();
    let config = tempfile::tempdir().expect("a config home");
    let pool = "held-pool";
    let tried = {
        let (held, config) = (Arc::clone(&held), config.path().to_path_buf());
        std::thread::spawn(move || crate::poolsetuptab::try_it(&held, &config, None, pool, None))
    };
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !tried.is_finished(),
        "Try it waits, though there is no such pool"
    );
    // A Cancel on another start in the project leaves Try it alone.
    crate::hostedsessions::cancel_starts(held.root());
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        !tried.is_finished(),
        "another start's Cancel is not Try it's"
    );
    until("Try it's Cancel", || {
        crate::poolsetuptab::pool_setup_try_cancel(pool.to_owned());
        tried.is_finished()
    });
    let tried = tried.join().expect("ends").expect("answered");
    assert!(tried.cancelled && tried.failed.is_none(), "{tried:?}");
}

#[test]
fn the_starts_the_person_does_not_make_start_at_once_while_a_prerequisite_is_open() {
    // ADR 0130 §5's at-once list: a chat restored at a project's open, started again in its
    // place or woken for its schedule or loop (`Made::Relaunch`), a running chat's dispatch or
    // handoff (`Made::Asked`), and a start that does not say how it was made.
    let (_root, _planes, _id, held) = an_open_project();
    let never = std::sync::atomic::AtomicBool::new(false);
    for made in [
        purlis_core::sandbox::Made::Relaunch,
        purlis_core::sandbox::Made::Asked {
            how: purlis_core::sandbox::Asking::Dispatch,
            above: Some(vec![Some("steward".to_owned())]),
        },
        purlis_core::sandbox::Made::Asked {
            how: purlis_core::sandbox::Asking::Handoff,
            above: None,
        },
        purlis_core::sandbox::Made::Unnamed,
    ] {
        let began = Instant::now();
        assert_eq!(
            hold_start(&held, &made, &never),
            Ok(Passed::AtOnce),
            "{made:?}"
        );
        assert!(began.elapsed() < Duration::from_secs(1), "{made:?} waited");
    }
}

/// Each function of the app's source under `src` that holds a start ([`hold_start`] or
/// [`hold_a_start`]), as `file: function`, sorted. Tests and this module are left out.
fn functions_that_hold(src: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut folders = vec![src.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in std::fs::read_dir(&folder).expect("a folder") {
            let path = entry.expect("an entry").path();
            let name = path
                .file_name()
                .expect("named")
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                if name != "tests" && name != "prerequisites" {
                    folders.push(path);
                }
                continue;
            }
            if !name.ends_with(".rs") || name.contains("test") || name == "prerequisites.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("source");
            let mut function = None;
            for line in text.lines() {
                let code = line.trim_start();
                if code.starts_with("//") {
                    continue;
                }
                if let Some(at) = code.find("fn ") {
                    let rest = &code[at + 3..];
                    let ident: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !ident.is_empty() && (at == 0 || code[..at].ends_with(' ')) {
                        function = Some(ident);
                    }
                }
                if code.contains("prerequisites::hold_start(")
                    || code.contains("prerequisites::hold_a_start(")
                {
                    let relative = path.strip_prefix(src).expect("under src");
                    found.push(format!(
                        "{}: {}",
                        relative.display(),
                        function.clone().unwrap_or_default()
                    ));
                }
            }
        }
    }
    found.sort();
    found
}

#[test]
fn the_starts_that_hold_are_these_and_no_others() {
    // Removing one of these holds lets a start of the person's past an open prerequisite;
    // adding one to a start that is not the person's (restore, wake, schedule, loop, a chat's
    // dispatch or handoff; ADR 0130 §5) would hold what must start at once. Either is this red.
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert_eq!(
        functions_that_hold(&src),
        [
            "curation.rs: sent",
            "curation.rs: start_typed_where_it_can_be",
            "finished.rs: reopen",
            "handoff.rs: asked_from_a_tab",
            "lib.rs: opened_session",
            "lib.rs: reopened_closed",
            "lib.rs: restarted",
            "lib.rs: restarted_without_sandbox",
            "lib.rs: resume_session",
            "lib.rs: resumed_ended",
            "lib.rs: retried",
            "lib.rs: start_chat",
            "lib.rs: started_fresh",
            "poolsetuptab.rs: try_on",
        ]
    );
}

/// The person's answer that grants nothing, to the policy prerequisite of the project `held`
/// holds as it is now: what releases a start held on it.
fn answer_the_policy(id: &PlaneId, held: &crate::planes::Held) {
    let kept = held.prerequisites();
    let shown = Prerequisites::of(id.clone(), &kept.current(held.root()));
    let [Prerequisite::Policy(policy)] = shown.list.as_slice() else {
        panic!("the policy prerequisite alone: {shown:?}");
    };
    deferred(
        id.clone(),
        held.root(),
        &kept,
        ShownPrerequisite::Policy {
            digest: policy.digest.clone(),
            reasons: policy.reasons.clone(),
        },
    );
}

#[test]
fn a_restart_at_a_pause_that_waited_on_the_person_is_not_made_yet_and_is_asked_again() {
    // #7950 round 3: while the restart waited, the chat ran on and could start a turn, so the
    // pause it was asked at may be gone by the answer. It is not restarted then: the window
    // asks again at the chat's next pause, which the core cannot tell from here.
    let (_root, _planes, id, held) = an_open_project();
    let restart = {
        let held = Arc::clone(&held);
        std::thread::spawn(move || {
            crate::restarted(&held, 999, SIZE, crate::planes::When::AtAPause)
        })
    };
    std::thread::sleep(Duration::from_millis(300));
    assert!(!restart.is_finished(), "the restart waits");
    answer_the_policy(&id, &held);
    until("the held restart released", || restart.is_finished());
    let answered = restart
        .join()
        .expect("the hold ends")
        .expect("not refused: chat 999 was never read");
    assert!(answered.chat.is_none());
    assert!(answered.ask_again);
    assert_eq!(
        answered.not_yet.as_deref(),
        Some(crate::planes::WAITS_FOR_ITS_NEXT_PAUSE)
    );
}

#[test]
fn a_restart_now_that_waited_and_one_at_a_pause_that_did_not_go_ahead() {
    // Restart now is the person's word to cut the turn, so a wait changes nothing; a restart at
    // a pause that passed at once is asked at the pause it is made at. Both reach the core's
    // restart, which owes chat 999 nothing.
    let (_root, _planes, id, held) = an_open_project();
    let restart = {
        let held = Arc::clone(&held);
        std::thread::spawn(move || crate::restarted(&held, 999, SIZE, crate::planes::When::Now))
    };
    std::thread::sleep(Duration::from_millis(300));
    assert!(!restart.is_finished(), "Restart now waits");
    answer_the_policy(&id, &held);
    until("the held restart released", || restart.is_finished());
    let owed_nothing = "purlis did not restart chat 999: it is owed no restart.";
    assert_eq!(
        restart.join().expect("the hold ends").err().as_deref(),
        Some(owed_nothing)
    );
    // The entry is deferred now, so the next start passes at once.
    let at_once = crate::restarted(&held, 999, SIZE, crate::planes::When::AtAPause);
    assert_eq!(at_once.err().as_deref(), Some(owed_nothing));
}
