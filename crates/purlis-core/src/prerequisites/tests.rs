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
    assert_eq!(heard.taken(), vec![List::default()], "one event, the whole list");
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
    assert_eq!(the_policy(&evaluate(root.path())).reasons, vec![Reason::Waiting]);
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
    assert_ne!(watched_now(root.path()), before, "a file the list watches moved");
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
    assert_ne!(watched_now(root.path()), before, "a file the list watches moved");
    let after = kept.evaluate(root.path());
    assert!(after.is_empty(), "the local file tightened it: {after:#?}");
    assert_eq!(heard.taken(), vec![List::default()]);
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
    assert_ne!(watched_now(root.path()), before, "a file the list watches moved");
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
fn two_open_prerequisites_come_in_the_cores_order_as_one_list() {
    let _home = OwnConfigHome::new();
    let root = plane(OPEN);
    STAND_IN.with(|on| on.set(true));
    let list = evaluate(root.path());
    STAND_IN.with(|on| on.set(false));
    let kinds: Vec<Kind> = list.entries().iter().map(Prerequisite::kind).collect();
    assert_eq!(kinds, vec![Kind::Policy, Kind::StandIn]);
}
