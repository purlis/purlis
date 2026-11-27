// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use super::*;
use crate::enforcement::{digest_of, press, press_with_new_key};
use crate::sandbox::local::OwnConfigHome;
use crate::sandbox::policy::Locks;

/// A plane whose settings file says `committed`, not a repository: its file is all there is.
fn plane(committed: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    std::fs::write(dir.path().join("charter.toml"), committed).expect("the settings file");
    dir
}

/// A plane whose acceptance of `committed` was pressed and sealed.
fn accepted(committed: &str) -> tempfile::TempDir {
    let root = plane(committed);
    press(root.path(), &digest_of(root.path())).expect("accepted");
    root
}

const WATCH: &str = "[policy]\ntemplate = \"watch\"\n";
const OPEN: &str = "[policy]\ntemplate = \"open\"\n";
const STRICT: &str = "[policy]\ntemplate = \"strict\"\n";

/// The acceptance record of the plane at `root`, its seal made not to hold while what it says
/// is left as it was: a record that does not hold, whose content is still there to be read.
fn unseal(root: &Path) {
    let file = crate::sandbox::local::path(root);
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).expect("kept")).expect("JSON");
    let object = doc.as_object_mut().expect("an object");
    assert!(object.contains_key("policy_accepted"), "{object:?}");
    object.insert("policy_seal".to_owned(), serde_json::json!("not the seal"));
    std::fs::write(&file, doc.to_string()).expect("written");
}

/// Each list told, in order.
#[derive(Clone, Default)]
struct Heard(Arc<Mutex<Vec<List>>>);

impl Heard {
    fn told(&self) -> Told {
        let heard = Arc::clone(&self.0);
        Arc::new(move |list: &List| heard.lock().expect("heard").push(list.clone()))
    }

    fn taken(&self) -> Vec<List> {
        std::mem::take(&mut *self.0.lock().expect("heard"))
    }
}

/// The one prerequisite of `list`, the policy one.
fn the_policy(list: &List) -> &Policy {
    match list.entries() {
        [Prerequisite::Policy(policy)] => policy,
        other => panic!("not the policy prerequisite alone: {other:?}"),
    }
}

#[test]
fn a_broken_acceptance_holds_the_policy_entry_and_accept_empties_it_with_one_event() {
    let _home = OwnConfigHome::new();
    let root = accepted(WATCH);
    unseal(root.path());
    let heard = Heard::default();
    let kept = Kept::new(heard.told());
    let list = kept.evaluate(root.path());
    let entry = the_policy(&list);
    assert_eq!(entry.reasons, vec![Reason::Broken]);
    assert_eq!(entry.state, State::Open);
    assert_eq!(entry.digest, digest_of(root.path()));
    assert!(!entry.needs_new_key && !entry.after_new_key);
    assert_eq!(heard.taken(), vec![list]);

    let after = kept
        .accept(root.path(), &digest_of(root.path()))
        .expect("accepted");
    assert!(after.is_empty(), "{after:?}");
    assert_eq!(
        heard.taken(),
        vec![List::default()],
        "one event, the whole list"
    );
    assert_eq!(kept.current(root.path()), List::default());
}

#[test]
fn waiting_raises_it_and_a_tightening_does_not() {
    let _home = OwnConfigHome::new();
    let root = accepted(WATCH);
    std::fs::write(root.path().join("charter.toml"), OPEN).expect("loosened");
    let list = evaluate(root.path());
    let entry = the_policy(&list);
    assert_eq!(entry.reasons, vec![Reason::Waiting]);
    assert!(entry.standing.waits(), "the rows are there: {entry:?}");

    let tightened = accepted(OPEN);
    std::fs::write(tightened.path().join("charter.toml"), STRICT).expect("tightened");
    assert!(evaluate(tightened.path()).is_empty());
}

#[test]
fn never_alone_raises_none_and_nothing_kept_with_a_policy_looser_than_watch_is_waiting() {
    let _home = OwnConfigHome::new();
    for said in ["", WATCH, STRICT] {
        let root = plane(said);
        assert!(evaluate(root.path()).is_empty(), "{said:?}");
    }
    let root = plane(OPEN);
    assert_eq!(
        the_policy(&evaluate(root.path())).reasons,
        vec![Reason::Waiting]
    );
}

#[test]
fn broken_and_waiting_together_are_one_entry_with_both_reasons() {
    let _home = OwnConfigHome::new();
    let root = accepted(WATCH);
    unseal(root.path());
    std::fs::write(root.path().join("charter.toml"), OPEN).expect("loosened");
    let list = evaluate(root.path());
    assert_eq!(
        the_policy(&list).reasons,
        vec![Reason::Broken, Reason::Waiting]
    );
}

#[test]
fn under_broken_waiting_is_weighed_against_watch_never_against_the_record_that_does_not_hold() {
    let _home = OwnConfigHome::new();
    // The record that does not hold says `strict`: read, `watch` would be looser than it.
    let strict = accepted(STRICT);
    unseal(strict.path());
    std::fs::write(strict.path().join("charter.toml"), WATCH).expect("loosened");
    assert_eq!(
        the_policy(&evaluate(strict.path())).reasons,
        vec![Reason::Broken]
    );
    // It says `open`: read, `open` would be nothing new.
    let open = accepted(OPEN);
    unseal(open.path());
    assert_eq!(
        the_policy(&evaluate(open.path())).reasons,
        vec![Reason::Broken, Reason::Waiting]
    );
}

#[test]
fn after_a_new_key_is_said_for_a_project_the_new_key_broke_and_for_no_other() {
    let _home = OwnConfigHome::new();
    let here = plane(OPEN);
    let other = accepted(WATCH);
    let tampered = accepted(WATCH);
    unseal(tampered.path());
    crate::secrets::seal::set_foreign_for_this_test(true);
    press_with_new_key(here.path(), &digest_of(here.path())).expect("a new key");
    let there = evaluate(other.path());
    let entry = the_policy(&there);
    assert_eq!(entry.reasons, vec![Reason::Broken]);
    assert!(entry.after_new_key && !entry.needs_new_key, "{entry:?}");
    // Edited after the new key sealed it: broken for another reason.
    unseal(here.path());
    let entry = evaluate(here.path());
    assert!(!the_policy(&entry).after_new_key, "{entry:?}");
}

// ---- each raise trigger ------------------------------------------------------------------------

/// What each file [`watched`] names holds now: `None` where it is not there.
fn watched_now(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    watched(root)
        .into_iter()
        .map(|path| {
            let held = std::fs::read(&path).ok();
            (path, held)
        })
        .collect()
}

fn git(root: &Path, args: &[&str]) {
    let mut git = std::process::Command::new("git");
    git.args(args)
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null");
    let ran = crate::forklock::output(&mut git).expect("git runs");
    assert!(ran.status.success(), "{args:?}: {ran:?}");
}

fn commit(root: &Path, message: &str) {
    git(root, &["add", "charter.toml"]);
    git(
        root,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit",
            "-q",
            "-m",
            message,
        ],
    );
}

#[test]
fn a_projects_open_evaluates_its_list_and_tells_it() {
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    let heard = Heard::default();
    let kept = Kept::new(heard.told());
    let list = kept.current(root.path());
    assert_eq!(the_policy(&list).reasons, vec![Reason::Waiting]);
    assert_eq!(heard.taken(), vec![list]);
}

#[test]
fn a_commit_that_moves_the_plane_roots_head_is_heard_and_raises_it() {
    let _home = OwnConfigHome::new();
    let root = plane(WATCH);
    git(root.path(), &["init", "-q"]);
    commit(root.path(), "watch");
    let heard = Heard::default();
    let kept = Kept::new(heard.told());
    assert!(kept.evaluate(root.path()).is_empty());
    heard.taken();
    let before = watched_now(root.path());
    std::fs::write(root.path().join("charter.toml"), OPEN).expect("loosened");
    // Not committed yet: the project's policy is what `HEAD` says.
    assert!(kept.evaluate(root.path()).is_empty());
    commit(root.path(), "open");
    assert_ne!(
        watched_now(root.path()),
        before,
        "a file the list watches moved"
    );
    let list = kept.evaluate(root.path());
    assert_eq!(the_policy(&list).reasons, vec![Reason::Waiting]);
    assert_eq!(heard.taken(), vec![list]);
}

#[test]
fn a_change_to_the_local_settings_file_is_heard_and_evaluated() {
    let _home = OwnConfigHome::new();
    // The local file is read only where git ignores it. It can tighten a family, never take
    // the merge opt-in `open` makes, so the loosening is one family.
    let root = plane("[policy]\ntemplate = \"watch\"\n[policy.guards]\nrelease = \"off\"\n");
    std::fs::write(root.path().join(".gitignore"), "charter.local.toml\n").expect("ignored");
    git(root.path(), &["init", "-q"]);
    commit(root.path(), "open");
    let heard = Heard::default();
    let kept = Kept::new(heard.told());
    assert!(!kept.evaluate(root.path()).is_empty());
    heard.taken();
    let before = watched_now(root.path());
    std::fs::write(root.path().join("charter.local.toml"), WATCH).expect("the local file");
    assert_ne!(
        watched_now(root.path()),
        before,
        "a file the list watches moved"
    );
    let after = kept.evaluate(root.path());
    assert!(after.is_empty(), "the local file tightened it: {after:#?}");
    assert_eq!(heard.taken(), vec![List::default()]);
}

#[test]
fn the_never_dispatch_list_is_watched() {
    // What the policy prerequisite carries reads it (`nevers_to_keep`), and a press that keeps
    // the nevers writes it outside the list: a change to it is heard.
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    assert!(
        watched(root.path()).contains(&crate::dispatchnever::path(root.path())),
        "{:?}",
        watched(root.path())
    );
}

#[test]
fn a_list_whose_watch_lapsed_is_evaluated_again_at_each_read() {
    // A folder the platform would not watch is one whose change nothing hears: the list kept
    // is then no answer, and each read evaluates it again (#7885's review, F6).
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    let heard = Heard::default();
    let kept = Kept::new(heard.told()).fresh_for(Duration::ZERO);
    assert!(!kept.current(root.path()).is_empty());
    std::fs::write(root.path().join("charter.toml"), WATCH).expect("tightened");
    assert!(
        !kept.current(root.path()).is_empty(),
        "a whole watch is trusted to tell"
    );
    kept.lapsed(true);
    assert!(kept.current(root.path()).is_empty(), "read again");
    assert_eq!(
        heard.taken().len(),
        2,
        "the open, and the read that changed it"
    );
    kept.lapsed(false);
    std::fs::write(root.path().join("charter.toml"), OPEN).expect("loosened");
    assert!(kept.current(root.path()).is_empty(), "whole again");
}

#[test]
fn a_change_to_this_machines_policy_file_is_heard_and_evaluated() {
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    assert!(
        watched(root.path()).contains(&PathBuf::from(crate::sandbox::policy::MACHINE_FILE)),
        "{:?}",
        watched(root.path())
    );
    let heard = Heard::default();
    let kept = Kept::new(heard.told());
    let before = kept.evaluate(root.path());
    heard.taken();
    crate::sandbox::policy::set_for_this_test(Locks::parse(
        r#"{"enforcement": ["guards.git"]}"#,
        Path::new(crate::sandbox::policy::MACHINE_FILE),
    ));
    let after = kept.evaluate(root.path());
    crate::sandbox::policy::set_for_this_test(Locks::none());
    assert_ne!(
        the_policy(&after).digest,
        the_policy(&before).digest,
        "what Accept names moved with the lock"
    );
    assert_eq!(heard.taken(), vec![after]);
}

#[test]
fn after_an_app_update_a_seal_key_become_foreign_raises_it() {
    let _home = OwnConfigHome::new();
    let root = accepted(WATCH);
    let heard = Heard::default();
    let kept = Kept::new(heard.told());
    assert!(kept.evaluate(root.path()).is_empty());
    heard.taken();
    crate::secrets::seal::set_foreign_for_this_test(true);
    let list = kept.evaluate(root.path());
    let entry = the_policy(&list);
    assert_eq!(entry.reasons, vec![Reason::Broken]);
    assert!(entry.needs_new_key, "{entry:?}");
    assert_eq!(heard.taken(), vec![list]);
    // The press that makes a new key evaluates the list again.
    let after = kept
        .accept_with_new_key(root.path(), &digest_of(root.path()))
        .expect("a new key, and accepted");
    assert!(after.is_empty());
    assert_eq!(heard.taken(), vec![List::default()]);
}

#[test]
fn a_change_to_the_acceptance_record_is_heard_and_raises_it() {
    let _home = OwnConfigHome::new();
    let root = accepted(WATCH);
    let heard = Heard::default();
    let kept = Kept::new(heard.told());
    assert!(kept.evaluate(root.path()).is_empty());
    heard.taken();
    let before = watched_now(root.path());
    unseal(root.path());
    assert_ne!(
        watched_now(root.path()),
        before,
        "a file the list watches moved"
    );
    let list = kept.evaluate(root.path());
    assert_eq!(the_policy(&list).reasons, vec![Reason::Broken]);
    assert_eq!(heard.taken(), vec![list]);
}

#[test]
fn a_list_that_did_not_change_is_not_told_again() {
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    let heard = Heard::default();
    let kept = Kept::new(heard.told());
    kept.evaluate(root.path());
    kept.evaluate(root.path());
    assert_eq!(heard.taken().len(), 1);
}

#[test]
fn the_event_is_pushed_after_the_list_is_kept_so_a_read_in_its_handler_gets_the_same_list() {
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    let me: Arc<OnceLock<Weak<Kept>>> = Arc::default();
    let read_in_the_handler: Arc<Mutex<Vec<(List, List)>>> = Arc::default();
    let kept = Arc::new(Kept::new({
        let (me, read) = (Arc::clone(&me), Arc::clone(&read_in_the_handler));
        let root = root.path().to_path_buf();
        Arc::new(move |told: &List| {
            let kept = me.get().and_then(Weak::upgrade).expect("the list");
            read.lock()
                .expect("read")
                .push((told.clone(), kept.current(&root)));
        })
    }));
    me.set(Arc::downgrade(&kept)).expect("set once");
    kept.evaluate(root.path());
    std::fs::write(root.path().join("charter.toml"), WATCH).expect("tightened");
    kept.evaluate(root.path());
    let read = read_in_the_handler.lock().expect("read").clone();
    assert_eq!(read.len(), 2);
    for (told, current) in read {
        assert_eq!(told, current);
    }
}

#[test]
fn a_read_in_the_handler_of_a_lapsed_watch_gets_the_list_just_kept() {
    // The handler runs inside the evaluation: a read there evaluating again would wait on it.
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    let me: Arc<OnceLock<Weak<Kept>>> = Arc::default();
    let read: Arc<Mutex<Vec<(List, List)>>> = Arc::default();
    let kept = Arc::new(Kept::new({
        let (me, read) = (Arc::clone(&me), Arc::clone(&read));
        let root = root.path().to_path_buf();
        Arc::new(move |told: &List| {
            let kept = me.get().and_then(Weak::upgrade).expect("the list");
            read.lock()
                .expect("read")
                .push((told.clone(), kept.current(&root)));
        })
    }));
    me.set(Arc::downgrade(&kept)).expect("set once");
    kept.lapsed(true);
    kept.evaluate(root.path());
    let read = read.lock().expect("read").clone();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].0, read[0].1);
}

#[test]
fn a_lapsed_read_while_another_thread_evaluates_waits_and_gets_what_that_evaluation_kept() {
    // #7885's review, round 3: a lapsed read that found an evaluation under way returned the
    // list kept before it, older than the change that evaluation was reading.
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    let kept = Arc::new(Kept::new(Heard::default().told()));
    assert!(!kept.current(root.path()).is_empty());
    kept.lapsed(true);
    let evaluating = kept.evaluating.lock().expect("the lock");
    let reader = std::thread::spawn({
        let (kept, root) = (Arc::clone(&kept), root.path().to_path_buf());
        move || kept.current(&root)
    });
    // Long enough for the read to reach the lock, which it must wait on.
    std::thread::sleep(Duration::from_millis(200));
    std::fs::write(root.path().join("charter.toml"), WATCH).expect("tightened");
    let evaluated = kept.evaluate_holding(evaluating, root.path());
    assert!(evaluated.is_empty(), "{evaluated:?}");
    assert_eq!(reader.join().expect("the read"), evaluated);
}

#[test]
fn a_lapsed_list_is_evaluated_again_no_more_often_than_its_freshness_allows() {
    // A window asks at each render: while lapsed, a list just evaluated answers the reads
    // after it, and one older than that is evaluated again.
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    let heard = Heard::default();
    let kept = Kept::new(heard.told()).fresh_for(Duration::from_millis(300));
    kept.lapsed(true);
    assert!(!kept.current(root.path()).is_empty());
    std::fs::write(root.path().join("charter.toml"), WATCH).expect("tightened");
    assert!(
        !kept.current(root.path()).is_empty(),
        "just evaluated: reused"
    );
    assert_eq!(heard.taken().len(), 1, "the open alone");
    std::thread::sleep(Duration::from_millis(350));
    assert!(kept.current(root.path()).is_empty(), "older: read again");
    assert_eq!(heard.taken().len(), 1, "the read that changed it");
}

#[test]
fn two_open_prerequisites_come_in_the_cores_order_as_one_list() {
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    STAND_IN.with(|on| on.set(true));
    let list = evaluate(root.path());
    STAND_IN.with(|on| on.set(false));
    let kinds: Vec<Kind> = list.entries().iter().map(Prerequisite::kind).collect();
    assert_eq!(kinds, vec![Kind::Policy, Kind::StandIn]);
}

/// What a start held on its own thread was told it waits on, and how its hold ends.
struct Holding {
    told: std::sync::mpsc::Receiver<String>,
    ended: std::thread::JoinHandle<Held>,
}

impl Holding {
    /// A start `starter` makes in the project at `root`, held by `kept` on a thread of its own
    /// for at most `within`.
    fn start(kept: &Arc<Kept>, root: &Path, starter: Starter, within: Option<Duration>) -> Self {
        let (tell, told) = std::sync::mpsc::channel();
        let (kept, root) = (Arc::clone(kept), root.to_path_buf());
        let ended = std::thread::spawn(move || {
            kept.hold(&root, starter, within, &|| false, |why| {
                let _ = tell.send(why.to_owned());
            })
        });
        Self { told, ended }
    }

    /// **Held before**: it is told why within a few seconds, and has not started a moment later.
    fn is_held(&self) -> String {
        let why = self
            .told
            .recv_timeout(Duration::from_secs(5))
            .expect("the held start is told why it waits");
        std::thread::sleep(Duration::from_millis(250));
        assert!(!self.ended.is_finished(), "it waits: {why}");
        why
    }

    /// How it ended, within a few seconds of being told.
    fn ended(self) -> Held {
        let by = Instant::now() + Duration::from_secs(5);
        while !self.ended.is_finished() {
            assert!(Instant::now() < by, "the held start never ended");
            std::thread::sleep(Duration::from_millis(10));
        }
        self.ended.join().expect("the hold ends")
    }
}

/// A plane whose acceptance of `committed` was kept and no longer holds: Broken.
fn broken(committed: &str) -> tempfile::TempDir {
    let root = accepted(committed);
    unseal(root.path());
    root
}

/// What the window shows for the policy prerequisite of `list`.
fn shown(list: &List) -> Shown {
    let policy = the_policy(list);
    Shown::Policy {
        digest: policy.digest.clone(),
        reasons: policy.reasons.clone(),
    }
}

/// The policy in force for the project at `root`, as [`enforcement::ENV`] spells it.
fn in_force(root: &Path) -> String {
    enforcement::in_force(root).spelled()
}

/// Strict mode: what a project runs under where its acceptance is Broken.
fn strict_mode() -> String {
    enforcement::broken_accepted().spelled()
}

const SAID: &str = "'s policy needs your answer in the purlis window on this machine";

#[test]
fn a_chat_the_person_starts_from_the_window_is_held_says_why_and_is_released_by_the_answer_that_grants_nothing()
 {
    // ADR 0130 §5, §6: held before, released after, under the policy in force.
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Arc::new(Kept::new(Heard::default().told()));
    let list = kept.evaluate(root.path());
    let start = Holding::start(
        &kept,
        root.path(),
        Starter::of(&crate::sandbox::Made::Window),
        None,
    );
    let why = start.is_held();
    assert!(why.ends_with(SAID), "{why}");
    let project = root.path().file_name().expect("named").to_string_lossy();
    assert!(why.starts_with(&*project), "names the project: {why}");

    let after = kept.defer(root.path(), shown(&list));
    assert_eq!(the_policy(&after).state, State::Deferred);
    assert_eq!(start.ended(), Held::Released);
    assert_eq!(in_force(root.path()), strict_mode(), "it starts strict");
}

#[test]
fn a_cli_start_is_held_as_a_window_start_is_and_runs_out_saying_why_where_nobody_answers() {
    // ADR 0130 §5, §8: a chat the person starts from the CLI waits too, for as long as its
    // caller gives it (90 seconds with a terminal, #7721).
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Arc::new(Kept::new(Heard::default().told()));
    let list = kept.evaluate(root.path());

    let unanswered = Holding::start(
        &kept,
        root.path(),
        Starter::Person,
        Some(Duration::from_millis(300)),
    );
    let Held::RanOut { why } = unanswered.ended() else {
        panic!("it runs out");
    };
    assert!(why.ends_with(SAID), "{why}");

    let answered = Holding::start(
        &kept,
        root.path(),
        Starter::Person,
        Some(Duration::from_secs(90)),
    );
    answered.is_held();
    kept.defer(root.path(), shown(&list));
    assert_eq!(answered.ended(), Held::Released);
    assert_eq!(in_force(root.path()), strict_mode());
}

#[test]
fn a_start_its_caller_stops_while_held_says_what_is_still_open() {
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Kept::new(Heard::default().told());
    let held = kept.hold(root.path(), Starter::Person, None, &|| true, |_| {});
    let Held::Stopped { why } = held else {
        panic!("stopped: {held:?}");
    };
    assert!(why.ends_with(SAID), "{why}");
}

#[test]
fn a_restored_chat_a_scheduled_start_and_a_loop_start_do_not_wait_and_start_under_the_policy_in_force()
 {
    // ADR 0130 §5. Each is made by the app on its own, from its record ([`Made::Relaunch`]): a
    // chat put back at a project's open, and one woken for its schedule or its loop.
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Kept::new(Heard::default().told());
    assert!(kept.evaluate(root.path()).holds(), "open");
    let starter = Starter::of(&crate::sandbox::Made::Relaunch);
    assert_eq!(starter, Starter::App);
    let held = kept.hold(
        root.path(),
        starter,
        None,
        &|| panic!("never waits"),
        |why| panic!("never told: {why}"),
    );
    assert_eq!(held, Held::AtOnce);
    assert_eq!(in_force(root.path()), strict_mode());
}

#[test]
fn regression_guard_a_running_chats_dispatched_task_starts_at_once_under_the_policy_in_force() {
    // A regression guard (#7724): a dispatched task never waited on the person, and passes
    // without the hold. ADR 0130 §5 keeps it so while a prerequisite is open.
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Kept::new(Heard::default().told());
    assert!(kept.evaluate(root.path()).holds(), "open");
    let dispatched = crate::sandbox::Made::Asked {
        how: crate::sandbox::Asking::Dispatch,
        above: Some(vec![None]),
    };
    let held = kept.hold(
        root.path(),
        Starter::of(&dispatched),
        None,
        &|| panic!("never waits"),
        |why| panic!("never told: {why}"),
    );
    assert_eq!(held, Held::AtOnce);
    assert_eq!(in_force(root.path()), strict_mode());
}

#[test]
fn a_chat_a_running_chat_hands_work_to_starts_at_once_and_a_pool_or_runner_chat_the_person_starts_is_held()
 {
    // ADR 0130 §5: a handoff a running chat makes starts as a dispatched task does, strict
    // under Broken. A pool or runner chat the person starts is made as a local one is, in the
    // window ([`Made::Window`]): where it runs is no part of who started it.
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Arc::new(Kept::new(Heard::default().told()));
    let list = kept.evaluate(root.path());
    let handed = crate::sandbox::Made::Asked {
        how: crate::sandbox::Asking::Handoff,
        above: Some(vec![None]),
    };
    let held = kept.hold(
        root.path(),
        Starter::of(&handed),
        None,
        &|| panic!("never waits"),
        |why| panic!("never told: {why}"),
    );
    assert_eq!(held, Held::AtOnce);
    assert_eq!(in_force(root.path()), strict_mode());

    let on_a_pool = Holding::start(
        &kept,
        root.path(),
        Starter::of(&crate::sandbox::Made::Window),
        None,
    );
    on_a_pool.is_held();
    kept.defer(root.path(), shown(&list));
    assert_eq!(on_a_pool.ended(), Held::Released);
}

#[test]
fn the_answer_is_dropped_when_the_digest_changes_and_the_prerequisite_is_open_again() {
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Kept::new(Heard::default().told());
    let list = kept.evaluate(root.path());
    assert!(!kept.defer(root.path(), shown(&list)).holds(), "answered");

    std::fs::write(root.path().join("charter.toml"), STRICT).expect("another policy");
    let after = kept.evaluate(root.path());
    let entry = the_policy(&after);
    assert_ne!(entry.digest, the_policy(&list).digest, "the digest moved");
    assert_eq!(entry.reasons, vec![Reason::Broken], "the reason did not");
    assert_eq!(entry.state, State::Open);
    assert!(after.holds());
}

#[test]
fn the_answer_is_dropped_when_the_reason_changes_from_waiting_to_broken_and_waiting() {
    let _home = OwnConfigHome::new();
    let root = accepted(WATCH);
    std::fs::write(root.path().join("charter.toml"), OPEN).expect("loosened");
    let kept = Kept::new(Heard::default().told());
    let list = kept.evaluate(root.path());
    assert_eq!(the_policy(&list).reasons, vec![Reason::Waiting]);
    assert!(!kept.defer(root.path(), shown(&list)).holds(), "answered");

    unseal(root.path());
    let after = kept.evaluate(root.path());
    let entry = the_policy(&after);
    assert_eq!(entry.reasons, vec![Reason::Broken, Reason::Waiting]);
    assert_eq!(
        entry.digest,
        the_policy(&list).digest,
        "the digest did not move"
    );
    assert_eq!(entry.state, State::Open);
}

#[test]
fn a_change_back_to_a_digest_already_answered_this_run_does_not_reopen_it() {
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Kept::new(Heard::default().told());
    let list = kept.evaluate(root.path());
    kept.defer(root.path(), shown(&list));
    std::fs::write(root.path().join("charter.toml"), STRICT).expect("another policy");
    assert!(
        kept.evaluate(root.path()).holds(),
        "open for the new digest"
    );

    std::fs::write(root.path().join("charter.toml"), WATCH).expect("back");
    let back = kept.evaluate(root.path());
    assert_eq!(the_policy(&back).digest, the_policy(&list).digest);
    assert_eq!(the_policy(&back).state, State::Deferred);
    assert!(!back.holds());
}

#[test]
fn an_answer_for_a_digest_and_reasons_the_list_never_held_is_not_kept() {
    // The answer is served on a link (ADR 0130 §8): one against a pair nobody was shown, such
    // as the digest of a policy not committed yet, defers nothing when that policy lands.
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Kept::new(Heard::default().told());
    let list = kept.evaluate(root.path());
    std::fs::write(root.path().join("charter.toml"), STRICT).expect("another policy");
    let not_yet = digest_of(root.path());
    std::fs::write(root.path().join("charter.toml"), WATCH).expect("back");
    assert_ne!(not_yet, the_policy(&list).digest);

    let answered = kept.defer(
        root.path(),
        Shown::Policy {
            digest: not_yet.clone(),
            reasons: vec![Reason::Broken],
        },
    );
    assert_eq!(the_policy(&answered).state, State::Open, "nothing answered");

    std::fs::write(root.path().join("charter.toml"), STRICT).expect("it lands");
    let landed = kept.evaluate(root.path());
    assert_eq!(the_policy(&landed).digest, not_yet);
    assert_eq!(the_policy(&landed).state, State::Open);
    assert!(landed.holds());
}

#[test]
fn answers_for_pairs_never_held_keep_nothing_and_the_real_answer_still_holds() {
    // What a link sends is unchecked, and kept for the app's whole run: it must not grow.
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Kept::new(Heard::default().told());
    let list = kept.evaluate(root.path());
    for n in 0..200 {
        kept.defer(
            root.path(),
            Shown::Policy {
                digest: format!("{n:064x}"),
                reasons: vec![Reason::Broken],
            },
        );
    }
    let answers = kept.answered.lock().expect("answers").len();
    assert!(answers <= ANSWERS_KEPT_AT_MOST, "{answers} kept");
    assert!(!kept.defer(root.path(), shown(&list)).holds(), "answered");
}

#[test]
fn the_pairs_a_list_held_are_bounded_and_the_oldest_goes_first_asked_again() {
    let pair = |n: usize| (format!("{n:064x}"), vec![Reason::Waiting]);
    let mut answers = Answers::default();
    assert!(!answers.held(pair(0)));
    assert!(answers.answer(&pair(0)), "held, so kept");
    assert!(answers.held(pair(0)), "answered");
    for n in 1..ANSWERS_KEPT_AT_MOST * 4 {
        answers.held(pair(n));
    }
    assert_eq!(answers.len(), ANSWERS_KEPT_AT_MOST);
    assert!(!answers.answer(&pair(0)), "the oldest went");
    assert!(!answers.held(pair(0)), "and is asked again");
}

/// Every file under `dir`, with what it holds.
fn files_under(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut folders = vec![dir.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                files.insert(path.clone(), std::fs::read(&path).unwrap_or_default());
            }
        }
    }
    files
}

#[test]
fn after_a_restart_no_answer_is_kept_and_nothing_of_it_is_on_disk() {
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let machine = crate::sandbox::local::path(root.path())
        .ancestors()
        .nth(2)
        .expect("this machine's folder")
        .to_path_buf();
    let before = (files_under(root.path()), files_under(&machine));
    let kept = Kept::new(Heard::default().told());
    let list = kept.evaluate(root.path());
    assert!(!kept.defer(root.path(), shown(&list)).holds(), "answered");
    assert_eq!(
        (files_under(root.path()), files_under(&machine)),
        before,
        "the answer writes nothing"
    );

    // A new app state: the list a restarted app holds.
    let restarted = Kept::new(Heard::default().told());
    let again = restarted.evaluate(root.path());
    assert_eq!(the_policy(&again).state, State::Open);
    assert!(again.holds());
}

#[test]
fn a_process_that_is_not_the_app_given_the_apps_answer_runs_under_the_policy_in_force_the_app_runs()
{
    // ADR 0130 §5, §6: the answer that grants nothing changes no policy, so a hook or a CLI
    // process a chat runs, told the app's answer (`hookwire::Ask::Policy`), applies what the app
    // does.
    let _home = OwnConfigHome::new();
    let root = broken(WATCH);
    let kept = Kept::new(Heard::default().told());
    let list = kept.evaluate(root.path());
    kept.defer(root.path(), shown(&list));
    let the_apps = enforcement::in_force(root.path());
    assert_eq!(the_apps.spelled(), strict_mode());

    enforcement::set_in_the_app_for_this_test(false);
    enforcement::answered_by_the_app(Some(the_apps.clone()));
    let here = in_force(root.path());
    enforcement::set_in_the_app_for_this_test(true);
    assert_eq!(here, the_apps.spelled());
}

#[test]
fn under_broken_loose_or_not_the_answer_that_grants_nothing_releases_the_held_start_strict_and_is_kept_against_its_reasons()
 {
    // ADR 0130 §3: under Broken the reason is `broken` alone, and `waiting` joins it where the
    // committed policy is looser than `watch`.
    for (committed, reasons) in [
        (OPEN, vec![Reason::Broken, Reason::Waiting]),
        (WATCH, vec![Reason::Broken]),
    ] {
        let _home = OwnConfigHome::new();
        let root = broken(WATCH);
        std::fs::write(root.path().join("charter.toml"), committed).expect("committed");
        let kept = Arc::new(Kept::new(Heard::default().told()));
        let list = kept.evaluate(root.path());
        assert_eq!(the_policy(&list).reasons, reasons, "{committed}");

        let start = Holding::start(&kept, root.path(), Starter::Person, None);
        start.is_held();
        // An answer shown for other reasons is not this one's.
        let other = if reasons.len() == 2 {
            vec![Reason::Broken]
        } else {
            vec![Reason::Broken, Reason::Waiting]
        };
        let after = kept.defer(
            root.path(),
            Shown::Policy {
                digest: the_policy(&list).digest.clone(),
                reasons: other,
            },
        );
        assert!(
            after.holds(),
            "kept against what it was shown for: {committed}"
        );
        assert!(!start.ended.is_finished(), "still held: {committed}");

        let after = kept.defer(root.path(), shown(&list));
        assert_eq!(the_policy(&after).state, State::Deferred);
        assert_eq!(the_policy(&after).reasons, reasons);
        assert_eq!(start.ended(), Held::Released, "{committed}");
        assert_eq!(in_force(root.path()), strict_mode(), "{committed}");
    }
}
