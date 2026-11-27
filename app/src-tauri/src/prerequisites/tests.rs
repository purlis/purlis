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
