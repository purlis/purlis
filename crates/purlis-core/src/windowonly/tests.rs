// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A window-only refusal raises one ask carrying its control, against a plane of real clones
//! (git runs locally, no forge is asked) and a config home of its own.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::*;

/// Runs git in `dir`, which must succeed.
fn git(dir: &Path, args: &[&str]) {
    let mut c = std::process::Command::new("git");
    c.arg("-C").arg(dir).args(args);
    assert!(
        crate::forklock::output(&mut c).unwrap().status.success(),
        "git {args:?}"
    );
}

/// A clone `name` in workspace `ws` of the plane at `root`, its `origin` at `url`.
fn a_clone(root: &Path, name: &str, url: &str) -> PathBuf {
    let clone = root.join("workspaces/ws").join(name);
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q"]);
    git(&clone, &["remote", "add", "origin", url]);
    clone
}

fn trust_api() -> Control {
    Control::TrustClone {
        folder: "workspaces/ws/api".to_owned(),
        place: "github.com/acme/api".to_owned(),
    }
}

#[test]
fn a_window_only_refusal_raises_one_ask_carrying_its_control() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/api.git");
    let told = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    telling(root, {
        let told = Arc::clone(&told);
        Arc::new(move || {
            told.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
    });

    // The same chat refused twice for the same reason is one ask, and the window is told once.
    assert!(raise(root, 4, trust_api()).new);
    assert!(!raise(root, 4, trust_api()).new);
    let raised = every(root);
    assert_eq!(raised.len(), 1, "{raised:?}");
    assert_eq!(raised[0].session, 4);
    assert_eq!(raised[0].control, trust_api());
    assert_eq!(raised[0].control.label(), "Trust this clone");
    assert!(
        raised[0]
            .control
            .says()
            .starts_with("Trust this clone so chats can use GitHub"),
        "{}",
        raised[0].control.says()
    );
    assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), 1);

    // Pressing the control clears the refusal: the clone is recorded, and the ask is gone.
    let trusted = trust_this_clone(
        &HostScope::for_a_test(),
        home.path(),
        root,
        "workspaces/ws/api",
        "github.com/acme/api",
    )
    .unwrap();
    assert_eq!(trusted, "github.com/acme/api");
    assert!(crate::forge::gitcredential::vouch::trusted(
        home.path(),
        &root.join("workspaces/ws/api")
    ));
    assert_eq!(every(root), []);
    assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn a_press_that_trusts_nothing_clears_no_refusal() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/other.git");
    raise(root, 4, trust_api());

    // Its remote changed since the ask was raised: nothing is trusted, and the ask stays.
    let refused = trust_this_clone(
        &HostScope::for_a_test(),
        home.path(),
        root,
        "workspaces/ws/api",
        "github.com/acme/api",
    );
    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(every(root).len(), 1);
}

#[test]
fn a_chat_s_refusals_go_when_it_ends_and_no_other_chat_s() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();
    raise(root, 4, trust_api());
    raise(root, 5, trust_api());

    forget(root, 4);

    let left: Vec<u32> = every(root).iter().map(|one| one.session).collect();
    assert_eq!(left, [5]);
}

#[test]
fn a_new_ask_for_a_clone_at_another_place_replaces_the_chat_s_old_one() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();
    let moved = Control::TrustClone {
        folder: "workspaces/ws/api".to_owned(),
        place: "github.com/acme/moved".to_owned(),
    };
    raise(root, 4, trust_api());
    raise(root, 5, trust_api());

    // The clone's remote changed: chat 4's old ask would be a dead button, so the new one
    // replaces it rather than stacking. Chat 5's stays until chat 5 is refused again.
    assert!(raise(root, 4, moved.clone()).new);

    let left: Vec<(u32, Control)> = every(root)
        .into_iter()
        .map(|one| (one.session, one.control))
        .collect();
    assert_eq!(left, [(5, trust_api()), (4, moved)]);
}

#[test]
fn a_project_that_ended_keeps_no_ask_and_tells_no_one() {
    let plane = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let root = plane.path();
    let told = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    telling(root, {
        let told = Arc::clone(&told);
        Arc::new(move || {
            told.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
    });
    raise(root, 4, trust_api());
    raise(other.path(), 4, trust_api());
    let told_before = told.load(std::sync::atomic::Ordering::SeqCst);

    project_ended(root);

    assert_eq!(every(root), []);
    assert_eq!(every(other.path()).len(), 1, "another project's ask stays");
    // Its listener is gone: an ask raised later tells no one.
    raise(root, 6, trust_api());
    assert_eq!(told.load(std::sync::atomic::Ordering::SeqCst), told_before);
    project_ended(root);
    project_ended(other.path());
}

#[test]
fn a_window_only_refusal_is_raised_as_an_ask_as_policy_consents_runs() {
    // Q308b: the ask is `consents`'. Logged, it is not raised, and one line says it would be.
    for (policy, asked, lines) in [("", true, 0), ("consents = \"log\"\n", false, 1)] {
        let root = tempfile::tempdir().expect("a project");
        std::fs::write(
            crate::names::manifest(root.path()),
            format!("[policy]\ntemplate = \"strict\"\n{policy}"),
        )
        .expect("the manifest");
        assert_eq!(asks(root.path()), asked, "{policy:?}");
        let logged = crate::enforcement::read_log(root.path(), crate::enforcement::UNKNOWN_CHAT);
        assert_eq!(logged.len(), lines, "{policy:?}");
        assert!(
            logged
                .iter()
                .all(|line| line.family == "consents" && line.rule == "window-only.ask")
        );
    }
}

#[test]
fn raise_answers_whether_the_ask_is_new_and_whether_a_window_listens() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();

    // No window listens for this project: the ask is kept, and the answer says nobody hears it.
    assert!(!listens(root));
    assert_eq!(
        raise(root, 4, trust_api()),
        Raised {
            new: true,
            listened: false
        }
    );

    telling(root, Arc::new(|| {}));
    assert!(listens(root));
    assert_eq!(
        raise(root, 4, trust_api()),
        Raised {
            new: false,
            listened: true
        }
    );
    assert_eq!(
        raise(root, 5, trust_api()),
        Raised {
            new: true,
            listened: true
        }
    );

    // A project that ended has no listener again.
    project_ended(root);
    assert!(!listens(root));
}

#[test]
fn each_new_clone_kind_has_its_label_and_says_what_its_press_does() {
    let one = Control::OneAddress {
        folder: "workspaces/ws/api".to_owned(),
        fetch: "github.com/acme/api".to_owned(),
        push: "github.com/acme/fork".to_owned(),
        to: "github.com/acme/api".to_owned(),
    };
    assert_eq!(one.label(), "Use one address");
    let said = one.says();
    // The row shows the address it will set, and the two it found (Q4).
    for shown in [
        "workspaces/ws/api",
        "github.com/acme/api",
        "github.com/acme/fork",
    ] {
        assert!(said.contains(shown), "{said}");
    }

    let clone = Control::CloneWithSignIn {
        workspace: "ws".to_owned(),
        name: "private".to_owned(),
        repo: "github.com/acme/private".to_owned(),
    };
    assert_eq!(clone.label(), "Clone it");
    let said = clone.says();
    // Host, owner, repo and the destination folder, and plainly the person's own sign-in.
    for shown in ["github.com", "acme", "private", "workspaces/ws"] {
        assert!(said.contains(shown), "{said}");
    }
    assert!(said.contains("your sign-in"), "{said}");
}

#[test]
fn a_press_on_a_new_kind_clears_every_chat_that_waited_on_it() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();
    let clone = Control::CloneWithSignIn {
        workspace: "ws".to_owned(),
        name: "private".to_owned(),
        repo: "github.com/acme/private".to_owned(),
    };
    raise(root, 4, clone.clone());
    raise(root, 5, clone.clone());
    raise(root, 5, trust_api());

    pressed_by_the_window(root, &clone);

    let left: Vec<Control> = every(root).into_iter().map(|one| one.control).collect();
    assert_eq!(left, [trust_api()]);
    project_ended(root);
}

#[test]
fn nobody_asked_says_so_and_names_the_way_out_in_words() {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path().join("acme");
    let said = nobody_asked(&root, "press Use one address on the clone");
    assert!(said.starts_with("Nobody was asked"), "{said}");
    assert!(
        said.contains("open purlis on acme, then press Use one address"),
        "{said}"
    );
    assert!(!said.contains("asked the person in Asks"), "{said}");
}

#[test]
fn use_one_address_sets_exactly_the_shown_address_and_fails_if_origin_changed() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    let clone = a_clone(root, "api", "https://github.com/acme/api.git");
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/acme/fork.git",
        ],
    );
    let shown = || Control::OneAddress {
        folder: "workspaces/ws/api".to_owned(),
        fetch: "github.com/acme/api".to_owned(),
        push: "github.com/acme/fork".to_owned(),
        to: "github.com/acme/api".to_owned(),
    };
    raise(root, 4, shown());

    // Origin moved since the row was raised: the press fails, changes nothing, says so.
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/acme/elsewhere.git",
        ],
    );
    let refused = use_one_address(&HostScope::for_a_test(), home.path(), root, &shown());
    assert!(refused.is_err(), "{refused:?}");
    assert!(refused.unwrap_err().contains("changed since"), "said");
    assert_eq!(
        crate::forge::gitcredential::remote::places_now(&clone).unwrap(),
        ["github.com/acme/elsewhere", "github.com/acme/fork"]
    );
    assert_eq!(every(root).len(), 1, "the ask stays");

    // Origin as the row showed it: the push address is set to the one shown, and nothing else.
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/acme/api.git",
        ],
    );
    let set = use_one_address(&HostScope::for_a_test(), home.path(), root, &shown()).unwrap();
    assert_eq!(set, "github.com/acme/api");
    assert_eq!(
        crate::forge::gitcredential::remote::places_now(&clone).unwrap(),
        ["github.com/acme/api", "github.com/acme/api"]
    );
    // Setting an address is never trusting: the clone still has no record, and the chat that
    // waited is now offered Trust this clone for the one address it has: a second press.
    assert!(!crate::forge::gitcredential::vouch::trusted(
        home.path(),
        &clone
    ));
    let left: Vec<(u32, Control)> = every(root)
        .into_iter()
        .map(|one| (one.session, one.control))
        .collect();
    assert_eq!(
        left,
        [(
            4,
            Control::TrustClone {
                folder: "workspaces/ws/api".to_owned(),
                place: "github.com/acme/api".to_owned()
            }
        )]
    );
    project_ended(root);
}

#[test]
fn use_one_address_shows_the_recorded_address_or_the_fetch_one() {
    let plane = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    let clone = a_clone(root, "api", "https://github.com/acme/api.git");
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/acme/fork.git",
        ],
    );

    // Nothing recorded: the fetch address.
    let shown = one_address_for(home.path(), root, "workspaces/ws/api").unwrap();
    assert_eq!(
        shown,
        Control::OneAddress {
            folder: "workspaces/ws/api".to_owned(),
            fetch: "github.com/acme/api".to_owned(),
            push: "github.com/acme/fork".to_owned(),
            to: "github.com/acme/api".to_owned(),
        }
    );

    // One recorded for the clone: that address.
    crate::forge::gitcredential::remote::record_for_a_test(
        home.path(),
        &clone,
        "github.com/acme/saved",
    )
    .unwrap();
    let Control::OneAddress { to, .. } =
        one_address_for(home.path(), root, "workspaces/ws/api").unwrap()
    else {
        panic!("not a Use one address");
    };
    assert_eq!(to, "github.com/acme/saved");

    // An origin that already has one address needs no press.
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/acme/api.git",
        ],
    );
    assert_eq!(
        one_address_for(home.path(), root, "workspaces/ws/api"),
        None
    );
}

/// A plane at a temp folder with a `charter.toml`, which `purlis discover` needs, under the
/// `strict` template, set explicitly (Q308).
fn a_plane() -> tempfile::TempDir {
    let plane = tempfile::tempdir().unwrap();
    std::fs::write(
        plane.path().join("charter.toml"),
        "[policy]\ntemplate = \"strict\"\n",
    )
    .unwrap();
    plane
}

#[test]
fn record_this_repo_is_raised_for_an_unrecorded_clone_and_its_press_records_that_one_repo() {
    let plane = a_plane();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/api.git");
    a_clone(root, "web", "https://github.com/acme/web.git");

    let control = record_repo_for(root, "ws", "api", Reach::default()).unwrap();
    assert_eq!(
        control,
        Control::RecordRepo {
            workspace: "ws".to_owned(),
            clone: "api".to_owned(),
            place: "github.com/acme/api".to_owned(),
        }
    );
    assert_eq!(control.label(), "Record this repo");
    let says = control.says();
    assert!(
        says.contains("github.com/acme/api") && says.contains("`purlis discover`"),
        "{says}"
    );
    raise(root, 4, control.clone());
    raise(root, 5, control.clone());

    // The press records that clone's repo, as `purlis discover` does, and no other clone's.
    assert_eq!(
        record_this_repo(root, &control, &[]).unwrap(),
        "github.com/acme/api"
    );
    assert!(crate::catalogue::milestones::recorded(root, "api").is_some());
    assert!(crate::catalogue::milestones::recorded(root, "web").is_none());
    // And clears the ask of every chat that waited on it.
    assert_eq!(every(root), []);
}

#[test]
fn record_this_repo_fails_and_records_nothing_where_origin_changed_since_its_row() {
    let plane = a_plane();
    let root = plane.path();
    let clone = a_clone(root, "api", "https://github.com/acme/api.git");
    let control = record_repo_for(root, "ws", "api", Reach::default()).unwrap();
    raise(root, 4, control.clone());

    git(
        &clone,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/evil/api.git",
        ],
    );
    let why = record_this_repo(root, &control, &[]).unwrap_err();
    assert!(why.contains("changed since"), "{why}");
    assert!(crate::catalogue::milestones::recorded(root, "api").is_none());
    assert!(!root.join("inventory/repos.json").exists());
    assert_eq!(every(root).len(), 1, "the ask is kept");
}

#[test]
fn a_clone_the_project_records_or_no_declared_host_names_has_nothing_to_record() {
    let plane = a_plane();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/api.git");
    a_clone(root, "far", "https://example.org/acme/far.git");
    a_clone(root, "gone", "https://github.com/acme/gone.git");
    let control = record_repo_for(root, "ws", "api", Reach::default()).unwrap();
    record_this_repo(root, &control, &[]).unwrap();

    assert!(
        record_repo_for(root, "ws", "api", Reach::default())
            .unwrap_err()
            .contains("already")
    );
    assert!(record_repo_for(root, "ws", "far", Reach::default()).is_err());
    assert!(
        record_repo_for(root, "ws", "nope", Reach::default())
            .unwrap_err()
            .contains("not a repo")
    );
    assert!(record_repo_for(root, "ws", "gone", Reach::default()).is_ok());
}

#[test]
fn asking_to_record_says_asked_in_asks_only_where_a_window_listens() {
    let plane = a_plane();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/api.git");

    let none = ask_to_record(root, 4, "ws", "api", Reach::default()).unwrap();
    assert!(none.starts_with("Nobody was asked"), "{none}");
    assert!(none.contains("purlis discover"), "{none}");
    assert!(!none.contains("asked the person in Asks"), "{none}");

    telling(root, Arc::new(|| {}));
    let some = ask_to_record(root, 5, "ws", "api", Reach::default()).unwrap();
    assert!(
        some.starts_with("purlis asked the person in Asks"),
        "{some}"
    );
    assert_eq!(every(root).len(), 2, "one ask per chat");
    project_ended(root);
}

#[test]
fn an_older_apps_answer_to_a_new_control_reads_as_older_with_restart_words() {
    let root = Path::new("/plane/acme");
    let older = record_repo_refusal(root, "api", AppSaid::Older);
    assert!(older.contains("different builds"), "{older}");
    assert!(older.contains("restart purlis"), "{older}");
    assert!(older.contains("update the purlis plugin"), "{older}");
    assert!(older.contains("nobody was asked"), "{older}");
    // It is never a refusal of the repo: no word of the app's own is in it.
    assert!(!older.contains("could not read this line"), "{older}");

    let unheard = record_repo_refusal(root, "api", AppSaid::Unheard);
    assert!(
        unheard.contains("Nobody was asked: no purlis window is open on acme"),
        "{unheard}"
    );
    assert!(unheard.contains("`purlis discover`"), "{unheard}");

    let told = record_repo_refusal(
        root,
        "api",
        AppSaid::Told("purlis asked the person in Asks.".to_owned()),
    );
    assert_eq!(told, "purlis asked the person in Asks.");
}

#[test]
fn asking_to_record_raises_no_ask_where_policy_consents_do_not_ask() {
    // `consents` under `log` or `off` raises none (Q308b): the chat is told to record it itself.
    for policy in ["log", "off"] {
        let plane = tempfile::tempdir().unwrap();
        let root = plane.path();
        std::fs::write(
            root.join("charter.toml"),
            format!("[policy]\ntemplate = \"strict\"\nconsents = \"{policy}\"\n"),
        )
        .unwrap();
        a_clone(root, "api", "https://github.com/acme/api.git");

        let why = ask_to_record(root, 4, "ws", "api", Reach::default()).unwrap_err();

        assert!(why.contains("`purlis discover`"), "{policy}: {why}");
        assert_eq!(every(root), [], "{policy}");
    }
}

/// The blocks a test's writer was handed, by chat.
type Blocks = Arc<Mutex<Vec<(u32, crate::eventlog::stops::Stop)>>>;

fn a_writer_of_blocks(root: &Path) -> Blocks {
    let blocks: Blocks = Arc::default();
    let kept = Arc::clone(&blocks);
    blocking(
        root,
        Arc::new(move |chat, stop| kept.lock().unwrap().push((chat, stop))),
    );
    blocks
}

#[test]
fn each_clone_and_remote_refusal_raises_its_own_ask() {
    // A1 Use one address, A2 Clone it, A3 Record this repo: one ask each for the chat, each with
    // its own control, and the person's press of one clears that one and no other.
    let plane = a_plane();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    let api = a_clone(root, "api", "https://github.com/acme/api.git");
    git(
        &api,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/acme/fork.git",
        ],
    );
    a_clone(root, "web", "https://github.com/acme/web.git");
    let one = one_address_for(home.path(), root, "workspaces/ws/api").unwrap();
    let clone = Control::CloneWithSignIn {
        workspace: "ws".to_owned(),
        name: "widget".to_owned(),
        repo: "github.com/acme/widget".to_owned(),
    };
    let record = record_repo_for(root, "ws", "web", Reach::default()).unwrap();
    for control in [&one, &clone, &record] {
        assert!(raise(root, 4, control.clone()).new, "{control:?}");
    }
    let labels = || -> Vec<&'static str> {
        every(root)
            .iter()
            .map(|one| one.control.label())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        labels(),
        ["Use one address", "Clone it", "Record this repo"]
    );

    // Record this repo's press clears its own ask.
    record_this_repo(root, &record, &[]).unwrap();
    assert_eq!(labels(), ["Use one address", "Clone it"]);
    // Clone it's press (the window cloned) clears its own.
    pressed_by_the_window(root, &clone);
    assert_eq!(labels(), ["Use one address"]);
    // Use one address's press clears its own, and offers Trust this clone next: a second press.
    use_one_address(&HostScope::for_a_test(), home.path(), root, &one).unwrap();
    assert_eq!(labels(), ["Trust this clone"]);
    project_ended(root);
}

#[test]
fn use_one_address_shows_its_address_and_trust_stays_a_second_press() {
    // Q4: the row shows the address the press sets; the press sets that and trusts nothing; the
    // chat that waited is offered Trust this clone, and trusting is its own press.
    let plane = a_plane();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    let clone = a_clone(root, "api", "https://github.com/acme/api.git");
    git(
        &clone,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/acme/fork.git",
        ],
    );
    let shown = one_address_for(home.path(), root, "workspaces/ws/api").unwrap();
    let Control::OneAddress { to, .. } = &shown else {
        panic!("{shown:?}");
    };
    assert!(shown.says().contains(to.as_str()), "{}", shown.says());
    raise(root, 4, shown.clone());

    let set = use_one_address(&HostScope::for_a_test(), home.path(), root, &shown).unwrap();

    assert_eq!(&set, to, "the press sets exactly the address shown");
    assert!(!crate::forge::gitcredential::vouch::trusted(
        home.path(),
        &clone
    ));
    let left = every(root);
    assert_eq!(left.len(), 1, "{left:?}");
    assert_eq!(left[0].session, 4);
    assert!(
        matches!(&left[0].control, Control::TrustClone { place, .. } if place == to),
        "{:?}",
        left[0].control
    );
    project_ended(root);
}

#[test]
fn a_clone_or_remote_refusal_with_no_window_says_nobody_was_asked_and_writes_a_block() {
    // Q6, one cell per row: with no window listening for the project, each of A1, A2 and A3 tells
    // the chat plainly that nobody was asked, names its way out in words, and writes one block.
    let plane = a_plane();
    let home = tempfile::tempdir().unwrap();
    let root = plane.path();
    let api = a_clone(root, "api", "https://github.com/acme/api.git");
    git(
        &api,
        &[
            "remote",
            "set-url",
            "--push",
            "origin",
            "https://github.com/acme/fork.git",
        ],
    );
    let blocks = a_writer_of_blocks(root);

    // A1 and A2 say it through `told_of`, as the mint and the clone's landing do.
    let one = one_address_for(home.path(), root, "workspaces/ws/api").unwrap();
    let clone = Control::CloneWithSignIn {
        workspace: "ws".to_owned(),
        name: "widget".to_owned(),
        repo: "github.com/acme/widget".to_owned(),
    };
    for (chat, control, then) in [
        (5, one, "press Use one address on workspaces/ws/api"),
        (6, clone, "press Clone it on github.com/acme/widget"),
    ] {
        let label = control.label();
        let raised = raise(root, chat, control);
        assert!(!raised.listened);
        let said = told_of(root, chat, raised, then);
        assert!(said.starts_with("Nobody was asked"), "{label}: {said}");
        assert!(
            said.contains(&format!("then {then}")),
            "{label}: the way out in words: {said}"
        );
        assert!(
            !said.contains("asked the person in Asks"),
            "{label}: {said}"
        );
    }
    // A3 says it through the app's answer to the chat's refusal.
    let said = ask_to_record(root, 7, "ws", "api", Reach::default()).unwrap();
    assert!(said.starts_with("Nobody was asked"), "{said}");
    assert!(said.contains("`purlis discover`"), "{said}");

    let written = blocks.lock().unwrap();
    let chats: Vec<u32> = written.iter().map(|(chat, _)| *chat).collect();
    assert_eq!(chats, [5, 6, 7], "one block for each row");
    assert!(written.iter().all(|(_, stop)| matches!(
        stop,
        crate::eventlog::stops::Stop::Block(block)
            if block.source == crate::eventlog::stops::Source::Guard
    )));
    drop(written);
    project_ended(root);
}

#[test]
fn record_this_repo_is_raised_only_for_a_clone_a_chat_could_not_have_made() {
    // The filters the GitHub App mint counts a clone by (Q249): there when the chat started, and
    // with a git directory no chat may write.
    let plane = a_plane();
    let root = plane.path();
    let there = a_clone(root, "api", "https://github.com/acme/api.git");
    let started = |clone: &Path| crate::forge::apptoken::Started {
        clone: crate::sandbox::real(clone),
        git: {
            use std::os::unix::fs::MetadataExt as _;
            let meta = std::fs::metadata(clone.join(".git")).unwrap();
            Some((meta.dev(), meta.ino()))
        },
    };
    let then = [started(&there)];
    let kept = Reach {
        started: Some(&then),
        writes: &[],
    };
    assert!(record_repo_for(root, "ws", "api", kept).is_ok());

    // A clone made after the chat started is no repo of the person's: its origin is the chat's.
    a_clone(root, "later", "https://github.com/acme/later.git");
    let why = record_repo_for(root, "ws", "later", kept).unwrap_err();
    assert!(why.contains("not a repo of workspace"), "{why}");

    // A clone whose git directory a chat may write is the chat's too, however old it is.
    let writes = [there.join(".git")];
    let held = Reach {
        started: Some(&then),
        writes: &writes,
    };
    let why = record_repo_for(root, "ws", "api", held).unwrap_err();
    assert!(why.contains("not a repo of workspace"), "{why}");
    // And the person's press holds the same line.
    let control = record_repo_for(root, "ws", "api", kept).unwrap();
    let why = record_this_repo(root, &control, &writes).unwrap_err();
    assert!(why.contains("changed since"), "{why}");
    assert!(crate::catalogue::milestones::recorded(root, "api").is_none());
}

#[test]
fn asking_to_record_is_raised_only_after_a_refusal_policy_makes() {
    // A chat's line is its own word: where `guards.forge` does not enforce the check, the read
    // goes ahead, so there was no refusal and nothing is raised.
    // Answered `off` for the family, as the app answers a chat whose project loosened it.
    let plane = a_plane();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/api.git");
    crate::enforcement::set_in_the_app_for_this_test(false);
    let settings = crate::enforcement::Settings::from_text(Some(
        "[policy]\ntemplate = \"strict\"\nguards.forge = \"off\"\n",
    ));
    crate::enforcement::answered_by_the_app(Some(crate::enforcement::Resolved::of(
        &settings,
        &crate::sandbox::policy::Locks::default(),
    )));
    let why = ask_to_record(root, 4, "ws", "api", Reach::default()).unwrap_err();
    crate::enforcement::set_in_the_app_for_this_test(true);
    assert!(why.contains("does not refuse"), "{why}");
    assert_eq!(every(root), []);
}

#[test]
fn record_this_repo_names_what_a_chat_can_do_to_an_origin() {
    let control = Control::RecordRepo {
        workspace: "ws".to_owned(),
        clone: "api".to_owned(),
        place: "github.com/acme/api".to_owned(),
    };
    let says = control.says();
    // As Trust this clone says it: whoever can write the clone can set where it points.
    assert!(says.contains("A chat can set a clone's origin"), "{says}");
}

#[test]
fn record_this_repo_keys_the_record_by_the_origin_s_bare_name_and_says_what_its_branch_is() {
    let plane = a_plane();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/api.git");
    let control = record_repo_for(root, "ws", "api", Reach::default()).unwrap();
    record_this_repo(root, &control, &[]).unwrap();

    let row = crate::catalogue::milestones::row_named(root, "api").unwrap();
    assert_eq!(row.name, "api");
    assert_eq!(row.path_with_namespace, "acme/api");
    // The forge was not asked, so the branch is unknown: the record names none, and the plane
    // format writes its own `main` for that, which nothing reads to choose a branch.
    assert_eq!(row.default_branch.as_deref(), Some("main"));
    assert_eq!(row.ssh_url, "git@github.com:acme/api.git");

    // A clone in a folder that origin does not name is not recorded under the folder's name, nor
    // under another: `purlis discover` is the way out.
    a_clone(root, "x", "https://github.com/acme/y.git");
    let why = record_repo_for(root, "ws", "x", Reach::default()).unwrap_err();
    assert!(why.contains("`purlis discover`"), "{why}");
    assert!(crate::catalogue::milestones::row_named(root, "x").is_none());
    assert!(crate::catalogue::milestones::row_named(root, "y").is_none());
}

#[test]
fn a_record_names_an_ssh_address_that_holds_for_a_host_with_a_port() {
    assert_eq!(
        ssh_url_for("github.com", "acme/api"),
        "git@github.com:acme/api.git"
    );
    // `git@host:2222:acme/api.git` would read the port as the start of the path.
    assert_eq!(
        ssh_url_for("git.acme.test:2222", "acme/api"),
        "ssh://git@git.acme.test:2222/acme/api.git"
    );
}

#[test]
fn nothing_is_raised_where_the_project_records_another_repo_for_the_clone_or_it_is_gitlab() {
    let plane = a_plane();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/api.git");
    a_clone(root, "lab", "https://gitlab.com/acme/lab.git");

    // A GitLab clone is `purlis discover`'s: the record built here is GitHub's.
    let why = record_repo_for(root, "ws", "lab", Reach::default()).unwrap_err();
    assert!(why.contains("GitLab"), "{why}");

    // The project records `other/api` for the clone `api`: a chat's origin does not replace it.
    let control = record_repo_for(root, "ws", "api", Reach::default()).unwrap();
    record_this_repo(root, &control, &[]).unwrap();
    let a_clone_at = root.join("workspaces/ws/api");
    git(
        &a_clone_at,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/other/api.git",
        ],
    );
    let why = record_repo_for(root, "ws", "api", Reach::default()).unwrap_err();
    assert!(why.contains("different repo"), "{why}");
    // And the press holds the same line: nothing is replaced.
    let why = record_this_repo(root, &control, &[]).unwrap_err();
    assert!(why.contains("different repo"), "{why}");
    assert_eq!(
        crate::catalogue::milestones::row_named(root, "api")
            .unwrap()
            .path_with_namespace,
        "acme/api"
    );
}

#[test]
fn an_origin_with_a_space_or_a_control_character_or_too_long_is_never_shown() {
    let plane = a_plane();
    let root = plane.path();
    let clone = a_clone(root, "api", "https://github.com/acme/api.git");
    for url in [
        "https://github.com/acme/ap i.git".to_owned(),
        format!("https://github.com/acme/{}.git", "a".repeat(300)),
    ] {
        git(&clone, &["remote", "set-url", "origin", &url]);
        assert!(
            record_repo_for(root, "ws", "api", Reach::default()).is_err(),
            "{url:?}"
        );
    }
    assert!(crate::forge::plain_place("github.com/acme/api"));
    assert!(!crate::forge::plain_place("github.com/acme/a b"));
    assert!(!crate::forge::plain_place("github.com/acme/a\nb"));
    assert!(!crate::forge::plain_place(&format!(
        "github.com/{}",
        "a".repeat(300)
    )));
}

/// Writes the inventory rows `rows` (`name`, `owner/repo`) for GitHub, as `purlis discover`
/// would, at `root`.
fn an_inventory(root: &Path, rows: &[(&str, &str)]) {
    let records: Vec<serde_json::Value> = rows
        .iter()
        .map(|(name, path)| {
            crate::inventory::record(
                &crate::forge::RepoRecord {
                    id: None,
                    name: (*name).to_owned(),
                    path_with_namespace: (*path).to_owned(),
                    default_branch: None,
                    description: String::new(),
                    web_url: format!("https://github.com/{path}"),
                    ssh_url: format!("git@github.com:{path}.git"),
                    topics: Vec::new(),
                    forge: crate::forge::Kind::GitHub,
                },
                "unknown",
            )
        })
        .collect();
    let cfg = crate::forge::load_config(root).unwrap();
    crate::inventory::add(root, &crate::forge::group_of(&cfg, 0), &records).unwrap();
}

#[test]
fn clone_it_is_pressed_on_the_inventory_row_it_named_and_only_while_its_address_is_the_one_shown() {
    // Alias rows: `x` is `o/y` and `y` is `p/y`. The row shown for the name `x` is at
    // github.com/o/y; a press that took the bare name of the address (`y`) would clone `p/y`.
    let plane = a_plane();
    let root = plane.path();
    an_inventory(root, &[("x", "o/y"), ("y", "p/y")]);

    // The control carries the inventory name, and the press holds it to the shown address.
    assert_eq!(clone_it_matches(root, "x", "github.com/o/y"), Ok(()));
    assert_eq!(clone_it_matches(root, "y", "github.com/p/y"), Ok(()));
    // The row named `y` is not the address shown for `x`: nothing is cloned.
    let why = clone_it_matches(root, "y", "github.com/o/y").unwrap_err();
    assert!(why.contains("not at github.com/o/y"), "{why}");
    // A name the inventory does not hold is refused, and so is a row that is another repo now.
    assert!(clone_it_matches(root, "gone", "github.com/o/y").is_err());
    let moved = a_plane();
    an_inventory(moved.path(), &[("x", "q/y")]);
    let why = clone_it_matches(moved.path(), "x", "github.com/o/y").unwrap_err();
    assert!(why.contains("cloned nothing"), "{why}");
}
