// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Several skills at once from the command line** (#5357): `purlis skill rm`, `off`, `on`, `pin`
//! and `archive` with several names or `--source`, as the window's Select and source pages make
//! them. At the person's terminal, one change after its review. **From a chat, never**: the
//! operator's ruling of 2026-11-05 is that a chat can only propose one, so the change goes to
//! the app as a proposal for the person and nothing here is written, whatever the chat passes.
//! That the app holds it and only Apply makes it is the app's test (`proposals/tests.rs`).
#![cfg(unix)]

use crate::support;

use std::path::PathBuf;
use std::process::{Output, Stdio};
use std::sync::{Mutex, mpsc};

use purlis_core::hookwire::{self, Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV};
use purlis_core::skills::verbs::Level;
use purlis_core::skills::verbs::set::{Member, SetChange};

const SOURCE: &str = "https://example.invalid/acme/skills";

struct Project {
    _tmp: tempfile::TempDir,
    at: PathBuf,
}

impl Project {
    /// A project with a workspace `ops` and the skills `grilling`, `tdd` and `review`, the first
    /// two installed from [`SOURCE`] and granted to every workspace.
    fn new() -> Project {
        use purlis_core::skills::source::lock::{self, AtLevel, Kept, Locked, Lockfile, Wrote};
        let tmp = tempfile::tempdir().unwrap();
        let at = tmp.path().canonicalize().unwrap();
        let root = at.join("plane");
        std::fs::create_dir_all(root.join("workspaces/ops")).unwrap();
        std::fs::write(root.join("workspaces/ops/workspace.md"), "ops\n").unwrap();
        std::fs::write(root.join("charter.toml"), "[plane]\n").unwrap();
        std::fs::create_dir_all(at.join("home")).unwrap();
        purlis_core::appstate::made_in_for_tests(&at.join("data"), &root).unwrap();
        let mut locked = Lockfile::default();
        for name in ["grilling", "tdd", "review"] {
            let dir = root.join("skills").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let text = format!("---\nname: {name}\ndescription: Does {name}.\n---\n\nbody\n");
            std::fs::write(dir.join("SKILL.md"), &text).unwrap();
            purlis_core::skills::grant(&root, name, &purlis_core::skills::Grant::All).unwrap();
            if name == "review" {
                continue;
            }
            locked.skills.push(Locked {
                name: name.to_owned(),
                source_name: None,
                level: AtLevel(Level::project()),
                source: purlis_core::skills::source::Remote {
                    url: SOURCE.to_owned(),
                    branch: None,
                },
                folder: format!("skills/{name}"),
                commit: purlis_core::skills::source::Commit::parse(&"a".repeat(40)).unwrap(),
                digest: "sha256:0".to_owned(),
                files: vec![Wrote {
                    path: "SKILL.md".to_owned(),
                    executable: false,
                    sha256: String::new(),
                }],
                brought_by: None,
            });
        }
        lock::write(&root, Kept::Shared, None, &locked).unwrap();
        Project { _tmp: tmp, at }
    }

    fn root(&self) -> PathBuf {
        self.at.join("plane")
    }

    fn there(&self, name: &str) -> bool {
        self.root()
            .join("skills")
            .join(name)
            .join("SKILL.md")
            .exists()
    }

    fn off_in_ops(&self) -> Vec<String> {
        purlis_core::skills::turned_off(&self.root(), "ops")
    }

    /// `purlis skill …` from a terminal of the person's own, with `env` added: nothing of a chat
    /// this test may itself run inside reaches it.
    fn run_with(&self, args: &[&str], env: &[(String, String)]) -> Output {
        let root = self.root();
        support::purlis()
            .arg("skill")
            .args(args)
            .current_dir(&root)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("CHARTER_ROOT", &root)
            .env("HOME", self.at.join("home"))
            .env("TMPDIR", std::env::temp_dir())
            .env("PURLIS_CONFIG_HOME", self.at.join("home/config"))
            .env("PURLIS_DATA_HOME", self.at.join("data"))
            .env("PURLIS_SKILL_STUDIO", "1")
            .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
            .env("NO_COLOR", "1")
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_with(args, &[])
    }
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn the_person_deletes_several_skills_as_one_change_after_its_review() {
    let project = Project::new();
    let out = project.run(&["rm", "grilling", "tdd", "--yes"]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("Delete 2 skills:"), "{}", said(&out));
    assert!(
        said(&out).contains("grilling · project: deleted"),
        "{}",
        said(&out)
    );
    assert!(!project.there("grilling"));
    assert!(!project.there("tdd"));
    assert!(project.there("review"));
}

#[test]
fn with_no_terminal_to_ask_and_no_yes_nothing_is_deleted() {
    let project = Project::new();
    let out = project.run(&["rm", "grilling", "tdd"]);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert!(said(&out).contains("add --yes"), "{}", said(&out));
    assert!(project.there("grilling") && project.there("tdd"));
}

#[test]
fn the_person_deletes_every_skill_installed_from_a_source() {
    let project = Project::new();
    let out = project.run(&["rm", "--source", SOURCE, "--yes"]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(!project.there("grilling"));
    assert!(!project.there("tdd"));
    assert!(project.there("review"), "review was not installed from it");
}

#[test]
fn a_source_nothing_was_installed_from_changes_nothing() {
    let project = Project::new();
    let out = project.run(&["rm", "--source", "https://example.invalid/other", "--yes"]);
    assert_eq!(out.status.code(), Some(1), "{}", said(&out));
    assert!(
        said(&out).contains("No skill installed from"),
        "{}",
        said(&out)
    );
    assert!(project.there("grilling"));
}

#[test]
fn the_person_turns_a_sources_skills_off_and_back_on_in_a_workspace() {
    let project = Project::new();
    let out = project.run(&["off", "--source", SOURCE, "-w", "ops", "--yes"]);
    assert!(out.status.success(), "{}", said(&out));
    let mut off = project.off_in_ops();
    off.sort();
    assert_eq!(off, ["grilling", "tdd"]);
    let out = project.run(&["on", "grilling", "tdd", "-w", "ops", "--yes"]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(project.off_in_ops().is_empty());
}

#[test]
fn the_person_pins_and_archives_several_skills() {
    let project = Project::new();
    let out = project.run(&["pin", "grilling", "review", "-w", "ops", "--yes"]);
    assert!(out.status.success(), "{}", said(&out));
    let by = purlis_core::skills::PinnedBy::Workspace("ops".to_owned());
    let mut pins = purlis_core::skills::pins_written(&project.root(), &by);
    pins.sort();
    assert_eq!(pins, ["grilling", "review"]);

    let out = project.run(&["archive", "tdd", "review", "--yes"]);
    assert!(out.status.success(), "{}", said(&out));
    for name in ["tdd", "review"] {
        let about = purlis_core::skills::about_of(&project.root().join("skills").join(name));
        assert_eq!(
            about.unwrap().lifecycle,
            purlis_core::skills::Lifecycle::Archived,
            "{name}"
        );
    }
}

/// An app listening on a socket in `project`'s home that takes every ask and says it is held.
fn an_app(
    project: &Project,
) -> (
    Vec<(String, String)>,
    mpsc::Receiver<Ask>,
    hookwire::Reading,
) {
    let home = project.at.join("home");
    let socket = home.join("app").join("hooks.sock");
    let (tx, asked) = mpsc::channel();
    let tx = Mutex::new(tx);
    let listener = Listener::bind(&home, &socket).unwrap();
    let token = listener.tokens().issue_to_this_process(3).unwrap();
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            tx.lock().unwrap().send(ask.clone()).unwrap();
            Answer::Said {
                lines: vec![purlis_core::repocmd::Say::Done(
                    "held for the person".into(),
                )],
                code: 0,
            }
        }),
    );
    let env = vec![
        (SOCKET_ENV.to_owned(), socket.display().to_string()),
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (TOKEN_ENV.to_owned(), token.expose().to_owned()),
    ];
    (env, asked, reading)
}

/// **The ruling's test**: a chat that runs the bulk delete a skill or a web page told it to, with
/// `--yes`, deletes nothing. The app is handed the change to show the person, and that is all.
#[test]
fn a_chats_bulk_delete_is_handed_to_the_person_and_deletes_nothing() {
    let project = Project::new();
    let (env, asked, _reading) = an_app(&project);
    let out = project.run_with(&["rm", "grilling", "tdd", "review", "--yes"], &env);
    assert!(out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("held for the person"), "{}", said(&out));
    assert!(project.there("grilling") && project.there("tdd") && project.there("review"));
    match asked.try_recv() {
        Ok(Ask::ProposeSkillSet(ask)) => {
            assert_eq!(ask.chat, 3);
            let project_skill = |name: &str| Member {
                level: Level::project(),
                name: name.to_owned(),
            };
            assert_eq!(
                ask.change,
                SetChange::Delete {
                    members: vec![
                        project_skill("grilling"),
                        project_skill("tdd"),
                        project_skill("review")
                    ]
                }
            );
        }
        _ => panic!("the app was asked nothing"),
    }
}

/// A source's skills, and a turn, a pin and an archive, are proposed the same way.
#[test]
fn a_chats_bulk_update_is_handed_to_the_person_and_changes_nothing() {
    let project = Project::new();
    let (env, asked, _reading) = an_app(&project);
    for args in [
        &["off", "--source", SOURCE, "-w", "ops", "--yes"][..],
        &["pin", "grilling", "tdd", "-w", "ops", "--yes"],
        &["archive", "--source", SOURCE, "--yes"],
        &["rm", "tdd"],
    ] {
        let out = project.run_with(args, &env);
        assert!(out.status.success(), "{args:?}: {}", said(&out));
        assert!(
            matches!(asked.try_recv(), Ok(Ask::ProposeSkillSet(_))),
            "{args:?}"
        );
    }
    assert!(project.off_in_ops().is_empty());
    let by = purlis_core::skills::PinnedBy::Workspace("ops".to_owned());
    assert!(purlis_core::skills::pins_written(&project.root(), &by).is_empty());
    let about = purlis_core::skills::about_of(&project.root().join("skills/grilling")).unwrap();
    assert_eq!(about.lifecycle, purlis_core::skills::Lifecycle::Active);
    assert!(project.there("tdd"));
}

/// A chat whose app is not there is refused: it never falls back to making the change itself.
#[test]
fn a_chat_that_cannot_reach_the_app_deletes_nothing() {
    let project = Project::new();
    let env = [
        (
            SOCKET_ENV.to_owned(),
            project.at.join("none.sock").display().to_string(),
        ),
        (CHAT_ENV.to_owned(), "3".to_owned()),
    ];
    let out = project.run_with(&["rm", "grilling", "tdd", "--yes"], &env);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("nothing was changed"), "{}", said(&out));
    assert!(project.there("grilling") && project.there("tdd"));
}

/// **Clearing the chat's number and socket doesn't make it the person**: any one mark of a chat
/// the app sets is enough, and none is a flag.
#[test]
fn a_chat_with_only_one_of_its_marks_left_still_deletes_nothing() {
    let project = Project::new();
    for mark in [
        (purlis_core::active::SESSION_ID_ENV, "a-chat"),
        (purlis_core::hookwire::SANDBOXED_ENV, "1"),
        (CHAT_ENV, "3"),
    ] {
        let env = [(mark.0.to_owned(), mark.1.to_owned())];
        let out = project.run_with(&["rm", "grilling", "tdd", "--yes"], &env);
        assert!(!out.status.success(), "{mark:?}: {}", said(&out));
        assert!(
            project.there("grilling") && project.there("tdd"),
            "{mark:?}"
        );
    }
}

/// **A chat that strips every mark the app set is still a chat** (#5357): below a chat's program
/// the project's record names, whatever its environment says, it changes nothing. With no
/// connection to the app left, it can't even propose, and says so: it never falls back to
/// making the change.
#[test]
fn a_chat_with_every_mark_stripped_is_known_by_where_it_runs_and_changes_nothing() {
    let project = Project::new();
    let record = purlis_core::reopen::Record {
        chats: vec![purlis_core::reopen::Chat {
            program: "/bin/zsh".into(),
            name: "chat 3".into(),
            number: Some(3),
            pid: Some(std::process::id()),
            ..Default::default()
        }],
        dealt: 3,
        ..Default::default()
    };
    purlis_core::reopen::write(&project.root(), &record).unwrap();
    for args in [
        &["rm", "grilling", "tdd", "--yes"][..],
        &["off", "--source", SOURCE, "-w", "ops", "--yes"],
    ] {
        let out = project.run(args);
        assert!(!out.status.success(), "{args:?}: {}", said(&out));
        assert!(said(&out).contains("nothing was changed"), "{}", said(&out));
    }
    assert!(project.there("grilling") && project.there("tdd"));
    assert!(project.off_in_ops().is_empty());
}

/// **Where purlis can't tell, it is a chat's** (#5357, fail closed): a record of open chats it
/// can't read makes the check unsure, and an unsure command changes nothing.
#[test]
fn where_purlis_cannot_tell_whether_it_is_a_chat_nothing_changes() {
    let project = Project::new();
    let record = purlis_core::reopen::path(&project.root());
    std::fs::create_dir_all(record.parent().unwrap()).unwrap();
    std::fs::write(&record, "not a record").unwrap();
    let out = project.run(&["rm", "grilling", "tdd", "--yes"]);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(project.there("grilling") && project.there("tdd"));
}
