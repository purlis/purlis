// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A window-only refusal raises one ask carrying its control, against a plane of real clones
//! (git runs locally, no forge is asked) and a config home of its own.

use std::path::{Path, PathBuf};

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

    let control = record_repo_for(root, "ws", "api").unwrap();
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
        record_this_repo(root, &control).unwrap(),
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
    let control = record_repo_for(root, "ws", "api").unwrap();
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
    let why = record_this_repo(root, &control).unwrap_err();
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
    let control = record_repo_for(root, "ws", "api").unwrap();
    record_this_repo(root, &control).unwrap();

    assert!(
        record_repo_for(root, "ws", "api")
            .unwrap_err()
            .contains("already")
    );
    assert!(record_repo_for(root, "ws", "far").is_err());
    assert!(
        record_repo_for(root, "ws", "nope")
            .unwrap_err()
            .contains("not a repo")
    );
    assert!(record_repo_for(root, "ws", "gone").is_ok());
}

#[test]
fn asking_to_record_says_asked_in_asks_only_where_a_window_listens() {
    let plane = a_plane();
    let root = plane.path();
    a_clone(root, "api", "https://github.com/acme/api.git");

    let none = ask_to_record(root, 4, "ws", "api").unwrap();
    assert!(none.starts_with("Nobody was asked"), "{none}");
    assert!(none.contains("purlis discover"), "{none}");
    assert!(!none.contains("asked the person in Asks"), "{none}");

    telling(root, Arc::new(|| {}));
    let some = ask_to_record(root, 5, "ws", "api").unwrap();
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

        let why = ask_to_record(root, 4, "ws", "api").unwrap_err();

        assert!(why.contains("`purlis discover`"), "{policy}: {why}");
        assert_eq!(every(root), [], "{policy}");
    }
}

/// A plane under `strict`, with `policy` appended, holding the persona `steward` and one memory.
fn a_plane_with_a_persona(policy: &str) -> tempfile::TempDir {
    let plane = tempfile::tempdir().unwrap();
    let root = plane.path();
    std::fs::write(
        root.join("charter.toml"),
        format!("[policy]\ntemplate = \"strict\"\n{policy}"),
    )
    .unwrap();
    let steward = root.join("personas/steward");
    std::fs::create_dir_all(steward.join("memory")).unwrap();
    std::fs::write(
        steward.join("persona.md"),
        "---\nrole: steward\n---\n# steward\n",
    )
    .unwrap();
    std::fs::write(steward.join("memory/one.md"), "one\n").unwrap();
    plane
}

#[test]
fn under_log_a_removal_runs_as_the_chat_asked_with_no_ask_and_a_quiet_log_line() {
    // Q308b: the family of every removal here is `guards.plane`. Under `log` the check runs and
    // allows, so the persona is removed as the chat asked and no ask is raised, with one quiet
    // line naming the rule. Under `enforce` (the template's) the person is asked and nothing is
    // removed. (`off` waits on what the person accepted: the app never runs a family looser than
    // `log` before they have, so it is not what a project's file alone can show.)
    let removing = crate::hookwire::Removing::Persona {
        name: "steward".to_owned(),
    };
    for (policy, removed, logged_line) in [
        ("", false, false),
        ("guards.plane = \"enforce\"\n", false, false),
        ("guards.plane = \"log\"\n", true, true),
    ] {
        let plane = a_plane_with_a_persona(policy);
        let root = plane.path();
        // The log is the app's, shared by every plane of a test run: count what this adds.
        let logged = || -> usize {
            std::fs::read_dir(crate::enforcement::log_dir(root))
                .into_iter()
                .flatten()
                .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
                .map(|lines| lines.matches("plane.removal").count())
                .sum()
        };
        let before = logged();
        let who = Who::here(None, Some("4".to_owned()), None)
            .for_chat(Some("01M881F55NPV4TTPA2EP7BW3CF".to_owned()));
        let asking = Asking {
            session: 4,
            who: &who,
            live: Some(&[]),
            claimed: false,
        };
        let said = ask_to_remove(root, &asking, None, &removing).words();
        assert_eq!(
            !root.join("personas/steward").exists(),
            removed,
            "{policy:?}"
        );
        assert_eq!(every(root).is_empty(), removed, "{policy:?}: {said}");
        assert_eq!(logged() - before, usize::from(logged_line), "{policy:?}");
        project_ended(root);
    }
}

const A: &str = "01M881F55NPV4TTPA2EP7BW3CF";
const B: &str = "01M881F55NPV4TTPA2EP7BW3CG";

fn a_chat(number: u32, id: &str) -> Who {
    Who::here(None, Some(number.to_string()), Some("builder".to_owned()))
        .for_chat(Some(id.to_owned()))
}

/// A plane under `policy`, whose workspace `alpha` holds the clone `thing`.
fn a_plane_with_a_clone(policy: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(
        root.join("charter.toml"),
        format!("schema = 1\n[policy]\ntemplate = \"strict\"\n{policy}"),
    )
    .unwrap();
    let clone = root.join("workspaces/alpha/thing");
    std::fs::create_dir_all(&clone).unwrap();
    git(&clone, &["init", "-q", "-b", "main", "."]);
    std::fs::write(clone.join("README.md"), "one\n").unwrap();
    git(&clone, &["add", "README.md"]);
    git(
        &clone,
        &[
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-q",
            "-m",
            "one",
        ],
    );
    dir
}

fn a_piece(root: &Path, piece: &str) -> PathBuf {
    crate::worktree::add(root, "alpha", "thing", piece, None)
        .expect("a piece is cut")
        .path
}

fn logged_for(root: &Path, chat: &str) -> usize {
    crate::enforcement::log_file(root, chat)
        .and_then(|file| std::fs::read_to_string(file).ok())
        .map_or(0, |lines| lines.matches("plane.removal").count())
}

#[test]
fn under_log_a_folder_another_live_chat_holds_is_refused_and_a_forced_line_is_not_forced() {
    // `guards.plane` lifts only the ask: the removal is held to the folder's one live writer
    // (#3901), never forced whatever the line said, and the log line names the asking chat.
    for policy in ["guards.plane = \"log\"\n", "guards.plane = \"off\"\n"] {
        let plane = a_plane_with_a_clone(policy);
        let root = plane.path();
        let live = [(A, 3), (B, 4)]
            .map(|(id, chat)| crate::piececlaim::Live {
                id: id.to_owned(),
                chat: Some(chat),
                back: true,
            })
            .to_vec();
        let held = a_piece(root, "held");
        let at = crate::worktree::Located {
            workspace: "alpha".to_owned(),
            repo: "thing".to_owned(),
            piece: "held".to_owned(),
        };
        crate::piececlaim::take(
            root,
            &at,
            &a_chat(3, A),
            Some(&live),
            crate::piececlaim::Act::Commit,
            jiff::Timestamp::now(),
        )
        .unwrap();
        let who = a_chat(4, B);
        let asking = Asking {
            session: 4,
            who: &who,
            live: Some(&live),
            claimed: false,
        };
        let forced = |piece: &str| crate::hookwire::Removing::BranchFolder {
            repo: "thing".to_owned(),
            piece: piece.to_owned(),
            force: true,
            ignored: false,
        };
        let before = logged_for(root, B);

        // The ask is lifted, and the removal still holds to the folder's one live writer.
        let Removal::Ran(Err(why)) = ask_to_remove(root, &asking, Some("alpha"), &forced("held"))
        else {
            panic!("{policy}: the removal did not run, or ran through");
        };
        assert!(why.contains("claimed by chat 3"), "{policy}: {why}");
        assert!(held.is_dir(), "{policy}: another chat's folder was removed");
        assert_eq!(every(root), [], "{policy}: an ask was raised");

        // `force` is the chat's own word, which a lifted ask never carries out: an unheld
        // folder with an uncommitted change is refused by git's own guard.
        let dirty = a_piece(root, "dirty");
        std::fs::write(dirty.join("README.md"), "changed\n").unwrap();
        let Removal::Ran(Err(why)) = ask_to_remove(root, &asking, Some("alpha"), &forced("dirty"))
        else {
            panic!("{policy}: a dirty folder was removed on the chat's force");
        };
        assert!(why.contains("Nothing was removed"), "{policy}: {why}");
        assert_eq!(
            std::fs::read_to_string(dirty.join("README.md")).unwrap(),
            "changed\n",
            "{policy}: discarded"
        );

        // A clean folder nobody holds goes, as a plain removal, and its claim with it.
        let clean = a_piece(root, "clean");
        let Removal::Ran(Ok(_)) = ask_to_remove(root, &asking, Some("alpha"), &forced("clean"))
        else {
            panic!("{policy}: a clean unheld folder was not removed");
        };
        assert!(!clean.exists(), "{policy}");

        // One line for each ask, in the asking chat's own log, and none under `unknown`.
        assert_eq!(logged_for(root, B) - before, 3, "{policy}");
        assert_eq!(logged_for(root, "unknown"), 0, "{policy}");
    }
}
