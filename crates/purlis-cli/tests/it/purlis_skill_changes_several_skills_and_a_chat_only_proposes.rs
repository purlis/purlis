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
                source: purlis_core::skills::source::Origin::Git(
                    purlis_core::skills::source::Remote {
                        url: SOURCE.to_owned(),
                        branch: None,
                    },
                ),
                folder: format!("skills/{name}"),
                pin: purlis_core::skills::source::Pin::Commit(
                    purlis_core::skills::source::Commit::parse(&"a".repeat(40)).unwrap(),
                ),
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

    /// `purlis skill …` with nothing of a chat this test may itself run inside, and `env` added.
    fn command(&self, args: &[&str], env: &[(String, String)]) -> std::process::Command {
        let root = self.root();
        let mut command = support::purlis();
        command
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
            .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
            .env("NO_COLOR", "1")
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        command
    }

    /// [`Self::command`] with no terminal: standard input is empty.
    fn run_with(&self, args: &[&str], env: &[(String, String)]) -> Output {
        self.command(args, env)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// [`Self::command`] on a real terminal a person types `typed` at: what it said, and its exit
    /// code.
    fn at_a_terminal(&self, args: &[&str], typed: &str) -> (String, u32) {
        use portable_pty::{CommandBuilder, PtySize, native_pty_system};
        use std::io::{Read, Write};
        let command = self.command(args, &[]);
        let mut starts = CommandBuilder::new(command.get_program());
        starts.args(command.get_args());
        starts.cwd(command.get_current_dir().unwrap());
        starts.env_clear();
        for (name, value) in command.get_envs() {
            if let Some(value) = value {
                starts.env(name, value);
            }
        }
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 50,
                cols: 200,
                ..PtySize::default()
            })
            .unwrap();
        let mut child = pair.slave.spawn_command(starts).unwrap();
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut writer = pair.master.take_writer().unwrap();
        // It waits on the terminal's line until it asks.
        writer.write_all(typed.as_bytes()).unwrap();
        let mut said = Vec::new();
        // A terminal whose program ended is an end of file on macOS and an error on Linux.
        let _ = reader.read_to_end(&mut said);
        let code = child.wait().unwrap().exit_code();
        drop((writer, pair.master));
        (String::from_utf8_lossy(&said).into_owned(), code)
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
    let (said, code) = project.at_a_terminal(&["rm", "grilling", "tdd"], "y\n");
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("Delete 2 skills:"), "{said}");
    assert!(said.contains("grilling · project: deleted"), "{said}");
    assert!(!project.there("grilling"));
    assert!(!project.there("tdd"));
    assert!(project.there("review"));
}

/// **A person who says no changes nothing.**
#[test]
fn the_persons_no_changes_nothing() {
    let project = Project::new();
    let (said, code) = project.at_a_terminal(&["rm", "grilling", "tdd"], "n\n");
    assert_ne!(code, 0, "{said}");
    assert!(said.contains("Nothing was changed"), "{said}");
    assert!(project.there("grilling") && project.there("tdd"));
}

/// **No flag stands for the person's yes** to a change to several skills.
#[test]
fn a_bulk_change_has_no_flag_to_skip_the_question() {
    let project = Project::new();
    for flag in ["--yes", "-y"] {
        let out = project.run(&["rm", "grilling", "tdd", flag]);
        assert!(
            said(&out).contains("unexpected argument"),
            "{flag}: {}",
            said(&out)
        );
        assert!(project.there("grilling") && project.there("tdd"));
    }
}

/// **A caller with no terminal is no person** (the operator's ruling of 2026-11-06): with nothing
/// of a chat about it, it still proposes, and with no app to take the proposal nothing changes.
#[test]
fn a_caller_with_no_terminal_proposes_and_changes_nothing_itself() {
    let project = Project::new();
    let out = project.run(&["rm", "grilling", "tdd"]);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("nothing was changed"), "{}", said(&out));
    assert!(project.there("grilling") && project.there("tdd"));
}

/// **A caller detached from its chat** (a session of its own, no terminal, every mark gone)
/// proposes too: it has no terminal to answer at.
#[test]
fn a_detached_caller_with_no_terminal_only_proposes() {
    let project = Project::new();
    let command = project.command(&["rm", "grilling", "tdd"], &[]);
    let mut detached = std::process::Command::new("/usr/bin/perl");
    detached
        .args(["-MPOSIX", "-e", "POSIX::setsid(); exec @ARGV or die"])
        .arg(command.get_program())
        .args(command.get_args())
        .current_dir(command.get_current_dir().unwrap())
        .env_clear()
        .envs(command.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))))
        .stdin(Stdio::null());
    let out = detached.output().unwrap();
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("nothing was changed"), "{}", said(&out));
    assert!(project.there("grilling") && project.there("tdd"));
}

#[test]
fn the_person_deletes_every_skill_installed_from_a_source() {
    let project = Project::new();
    let (said, code) = project.at_a_terminal(&["rm", "--source", SOURCE], "y\n");
    assert_eq!(code, 0, "{said}");
    assert!(!project.there("grilling"));
    assert!(!project.there("tdd"));
    assert!(project.there("review"), "review was not installed from it");
}

#[test]
fn a_source_nothing_was_installed_from_changes_nothing() {
    let project = Project::new();
    let (said, code) =
        project.at_a_terminal(&["rm", "--source", "https://example.invalid/other"], "y\n");
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("No skill installed from"), "{said}");
    assert!(project.there("grilling"));
}

#[test]
fn the_person_turns_a_sources_skills_off_and_back_on_in_a_workspace() {
    let project = Project::new();
    let (said, code) = project.at_a_terminal(&["off", "--source", SOURCE, "-w", "ops"], "y\n");
    assert_eq!(code, 0, "{said}");
    let mut off = project.off_in_ops();
    off.sort();
    assert_eq!(off, ["grilling", "tdd"]);
    let (said, code) = project.at_a_terminal(&["on", "grilling", "tdd", "-w", "ops"], "y\n");
    assert_eq!(code, 0, "{said}");
    assert!(project.off_in_ops().is_empty());
}

#[test]
fn the_person_pins_and_archives_several_skills() {
    let project = Project::new();
    let (said, code) = project.at_a_terminal(&["pin", "grilling", "review", "-w", "ops"], "y\n");
    assert_eq!(code, 0, "{said}");
    let by = purlis_core::skills::PinnedBy::Workspace("ops".to_owned());
    let mut pins = purlis_core::skills::pins_written(&project.root(), &by);
    pins.sort();
    assert_eq!(pins, ["grilling", "review"]);

    let (said, code) = project.at_a_terminal(&["archive", "tdd", "review"], "y\n");
    assert_eq!(code, 0, "{said}");
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

/// **The ruling's test**: a chat that runs the bulk delete a skill or a web page told it to,
/// deletes nothing. The app is handed the change to show the person, and that is all.
#[test]
fn a_chats_bulk_delete_is_handed_to_the_person_and_deletes_nothing() {
    let project = Project::new();
    let (env, asked, _reading) = an_app(&project);
    let out = project.run_with(&["rm", "grilling", "tdd", "review"], &env);
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
        &["off", "--source", SOURCE, "-w", "ops"][..],
        &["pin", "grilling", "tdd", "-w", "ops"],
        &["archive", "--source", SOURCE],
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
    let out = project.run_with(&["rm", "grilling", "tdd"], &env);
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
        let out = project.run_with(&["rm", "grilling", "tdd"], &env);
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
        &["rm", "grilling", "tdd"][..],
        &["off", "--source", SOURCE, "-w", "ops"],
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
    let out = project.run(&["rm", "grilling", "tdd"]);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(project.there("grilling") && project.there("tdd"));
}

// **Bring back, move and Delete now** (#5492): the verbs #5357 left, under the same rule. Each is
// run below as a chat, a chat with its marks stripped, a detached caller, a caller with no
// terminal and one purlis can't place, and each time the project is as it was.

impl Project {
    /// Deletes `names` from the project's skills, as the person did earlier.
    fn deleted(&self, names: &[&str]) {
        let root = self.root();
        let mut say = |_: purlis_core::repocmd::Say| {};
        let mut cx = purlis_core::skills::verbs::Ctx {
            plane: &root,
            built_in: None,
            by: purlis_core::skills::verbs::By::Command,
            say: &mut say,
            audit: None,
        };
        for name in names {
            purlis_core::skills::verbs::delete(&mut cx, &Level::project(), name).unwrap();
        }
    }

    /// The names in Deleted, in order.
    fn in_deleted(&self) -> Vec<String> {
        let mut names: Vec<String> = purlis_core::skills::verbs::deleted(&self.root())
            .into_iter()
            .map(|one| one.name)
            .collect();
        names.sort();
        names
    }

    fn in_ops(&self, name: &str) -> bool {
        self.root()
            .join("workspaces/ops/skills")
            .join(name)
            .join("SKILL.md")
            .exists()
    }

    /// What these verbs change: which skills are where, and what Deleted lists.
    fn snapshot(&self) -> (Vec<bool>, Vec<String>) {
        let mut where_ = Vec::new();
        for name in ["grilling", "tdd", "review"] {
            where_.push(self.there(name));
            where_.push(self.in_ops(name));
        }
        (where_, self.in_deleted())
    }
}

/// Each verb #5492 adds, on several skills, with the project it needs: `grilling` and `tdd`
/// deleted first for a restore and a Delete now.
const LATER_VERBS: &[(&[&str], bool)] = &[
    (&["restore", "grilling", "tdd"], true),
    (&["mv", "grilling", "tdd", "--to", "ops"], false),
    (&["mv", "--source", SOURCE, "--to", "ops"], false),
    (&["delete-now", "grilling", "tdd"], true),
    // One name, too: Delete now can't be undone, so it is asked even of one.
    (&["delete-now", "tdd"], true),
    // With no place: the person picks where.
    (&["export", "grilling", "tdd"], false),
];

/// A project for `case`.
fn for_case(deleted: bool) -> Project {
    let project = Project::new();
    if deleted {
        project.deleted(&["grilling", "tdd"]);
    }
    project
}

fn project_skills(names: &[&str]) -> Vec<Member> {
    names
        .iter()
        .map(|name| Member {
            level: Level::project(),
            name: (*name).to_owned(),
        })
        .collect()
}

/// **The ruling's test, for the later verbs**: a chat's bring back, move or Delete now changes
/// nothing, and the app is handed the change, naming the skills and the act and nothing else.
#[test]
fn a_chats_restore_move_or_delete_now_is_handed_to_the_person_and_changes_nothing() {
    let expected = [
        SetChange::Restore {
            members: project_skills(&["grilling", "tdd"]),
        },
        SetChange::Move {
            members: project_skills(&["grilling", "tdd"]),
            to: Level::workspace("ops"),
            grant: Vec::new(),
        },
        SetChange::Move {
            members: project_skills(&["grilling", "tdd"]),
            to: Level::workspace("ops"),
            grant: Vec::new(),
        },
        SetChange::DeleteNow {
            members: project_skills(&["grilling", "tdd"]),
        },
        SetChange::DeleteNow {
            members: project_skills(&["tdd"]),
        },
        SetChange::Export {
            members: project_skills(&["grilling", "tdd"]),
            zip: false,
        },
    ];
    for ((args, deleted), expected) in LATER_VERBS.iter().zip(expected) {
        let project = for_case(*deleted);
        let (env, asked, _reading) = an_app(&project);
        let before = project.snapshot();
        let out = project.run_with(args, &env);
        assert!(out.status.success(), "{args:?}: {}", said(&out));
        assert_eq!(project.snapshot(), before, "{args:?}");
        match asked.try_recv() {
            Ok(Ask::ProposeSkillSet(ask)) => assert_eq!(ask.change, expected, "{args:?}"),
            _ => panic!("{args:?}: the app was asked nothing"),
        }
    }
}

/// **A chat with every mark stripped**, known by where it runs, changes nothing with these
/// verbs either.
#[test]
fn a_chat_with_every_mark_stripped_restores_moves_and_deletes_now_nothing() {
    for (args, deleted) in LATER_VERBS {
        let project = for_case(*deleted);
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
        let before = project.snapshot();
        let out = project.run(args);
        assert!(!out.status.success(), "{args:?}: {}", said(&out));
        assert!(
            said(&out).contains("nothing was changed"),
            "{args:?}: {}",
            said(&out)
        );
        assert_eq!(project.snapshot(), before, "{args:?}");
    }
}

/// **A detached caller with no terminal** only proposes these too.
#[test]
fn a_detached_caller_restores_moves_and_deletes_now_nothing() {
    for (args, deleted) in LATER_VERBS {
        let project = for_case(*deleted);
        let before = project.snapshot();
        let command = project.command(args, &[]);
        let mut detached = std::process::Command::new("/usr/bin/perl");
        detached
            .args(["-MPOSIX", "-e", "POSIX::setsid(); exec @ARGV or die"])
            .arg(command.get_program())
            .args(command.get_args())
            .current_dir(command.get_current_dir().unwrap())
            .env_clear()
            .envs(command.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))))
            .stdin(Stdio::null());
        let out = detached.output().unwrap();
        assert!(!out.status.success(), "{args:?}: {}", said(&out));
        assert!(
            said(&out).contains("nothing was changed"),
            "{args:?}: {}",
            said(&out)
        );
        assert_eq!(project.snapshot(), before, "{args:?}");
    }
}

/// **No terminal is no person**, for these verbs too.
#[test]
fn a_caller_with_no_terminal_restores_moves_and_deletes_now_nothing() {
    for (args, deleted) in LATER_VERBS {
        let project = for_case(*deleted);
        let before = project.snapshot();
        let out = project.run(args);
        assert!(!out.status.success(), "{args:?}: {}", said(&out));
        assert!(
            said(&out).contains("nothing was changed"),
            "{args:?}: {}",
            said(&out)
        );
        assert_eq!(project.snapshot(), before, "{args:?}");
    }
}

/// **Where purlis can't tell whether it is a chat**, these verbs change nothing.
#[test]
fn where_purlis_cannot_tell_nothing_is_restored_moved_or_deleted_now() {
    for (args, deleted) in LATER_VERBS {
        let project = for_case(*deleted);
        let record = purlis_core::reopen::path(&project.root());
        std::fs::create_dir_all(record.parent().unwrap()).unwrap();
        std::fs::write(&record, "not a record").unwrap();
        let before = project.snapshot();
        let out = project.run(args);
        assert!(!out.status.success(), "{args:?}: {}", said(&out));
        assert_eq!(project.snapshot(), before, "{args:?}");
    }
}

/// No flag stands for the person's yes to these either.
#[test]
fn restore_move_and_delete_now_have_no_flag_to_skip_the_question() {
    for (args, deleted) in LATER_VERBS {
        let project = for_case(*deleted);
        let before = project.snapshot();
        let mut with = args.to_vec();
        with.push("--yes");
        let out = project.run(&with);
        assert!(
            said(&out).contains("unexpected argument"),
            "{with:?}: {}",
            said(&out)
        );
        assert_eq!(project.snapshot(), before, "{with:?}");
    }
}

#[test]
fn the_person_brings_several_skills_back_as_one_change_after_its_review() {
    let project = for_case(true);
    let (said, code) = project.at_a_terminal(&["restore", "grilling", "tdd"], "y\n");
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("Bring back 2 skills:"), "{said}");
    assert!(project.there("grilling") && project.there("tdd"));
    assert!(project.in_deleted().is_empty());
}

#[test]
fn the_person_moves_a_sources_skills_to_a_workspace_as_one_change() {
    let project = Project::new();
    let (said, code) = project.at_a_terminal(&["mv", "--source", SOURCE, "--to", "ops"], "y\n");
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains("Move 2 skills to the workspace ops skills:"),
        "{said}"
    );
    assert!(project.in_ops("grilling") && project.in_ops("tdd"));
    assert!(project.there("review"), "review was not installed from it");
}

/// **Delete now asks first even at the person's terminal**, even of one skill: there is no undo.
#[test]
fn delete_now_asks_first_even_at_the_persons_terminal() {
    let project = for_case(true);
    let (said, code) = project.at_a_terminal(&["delete-now", "tdd"], "n\n");
    assert_ne!(code, 0, "{said}");
    assert!(said.contains("Delete 1 skill now, with no undo:"), "{said}");
    assert!(said.contains("Nothing was changed"), "{said}");
    assert_eq!(project.in_deleted(), ["grilling", "tdd"]);
    let (said, code) = project.at_a_terminal(&["delete-now", "tdd"], "y\n");
    assert_eq!(code, 0, "{said}");
    assert_eq!(project.in_deleted(), ["grilling"]);
}

/// What makes a caller what it is, done to its project and its command.
type SetUp = fn(&Project, &mut std::process::Command);

/// **A place for an export of several skills is the person's to pick** (#5492, the person's choice
/// of 2026-11-06): with `--to`, a chat, a chat with its marks stripped, a caller with no terminal,
/// a detached one and one purlis can't place are each refused, and nothing is written there or
/// handed to the app.
#[test]
fn an_export_of_several_skills_to_a_place_anyone_but_the_person_names_writes_nothing() {
    let callers: [(&str, SetUp); 4] = [
        ("no terminal", |_, _| {}),
        ("stripped", |project, _| {
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
        }),
        ("unsure", |project, _| {
            let record = purlis_core::reopen::path(&project.root());
            std::fs::create_dir_all(record.parent().unwrap()).unwrap();
            std::fs::write(&record, "not a record").unwrap();
        }),
        ("a chat's mark", |_, command| {
            command.env(CHAT_ENV, "3");
        }),
    ];
    for (who, set_up) in callers {
        let project = Project::new();
        let to = project.at.join("out");
        let to_arg = to.display().to_string();
        let mut command = project.command(&["export", "grilling", "tdd", "--to", &to_arg], &[]);
        set_up(&project, &mut command);
        let out = command.stdin(Stdio::null()).output().unwrap();
        assert!(!out.status.success(), "{who}: {}", said(&out));
        assert!(
            said(&out).contains("goes only where the person picks"),
            "{who}: {}",
            said(&out)
        );
        assert!(!to.exists(), "{who}: nothing was written");
    }
}

/// The same, detached from its chat.
#[test]
fn a_detached_callers_export_of_several_skills_to_a_place_writes_nothing() {
    let project = Project::new();
    let to = project.at.join("out");
    let to_arg = to.display().to_string();
    let command = project.command(&["export", "--source", SOURCE, "--to", &to_arg], &[]);
    let mut detached = std::process::Command::new("/usr/bin/perl");
    detached
        .args(["-MPOSIX", "-e", "POSIX::setsid(); exec @ARGV or die"])
        .arg(command.get_program())
        .args(command.get_args())
        .current_dir(command.get_current_dir().unwrap())
        .env_clear()
        .envs(command.get_envs().filter_map(|(k, v)| v.map(|v| (k, v))))
        .stdin(Stdio::null());
    let out = detached.output().unwrap();
    assert!(!out.status.success(), "{}", said(&out));
    assert!(!to.exists());
}

/// One skill's export stays as it was: it writes where its caller can, after nothing is asked.
#[test]
fn one_skills_export_needs_no_question() {
    let project = Project::new();
    let to = project.at.join("out");
    let out = project.run(&["export", "tdd", "--to", &to.display().to_string()]);
    assert!(out.status.success(), "{}", said(&out));
    assert!(to.join("tdd/SKILL.md").exists());
}

#[test]
fn the_person_exports_a_sources_skills_after_its_review() {
    let project = Project::new();
    let to = project.at.join("out");
    let (said, code) = project.at_a_terminal(
        &[
            "export",
            "--source",
            SOURCE,
            "--to",
            &to.display().to_string(),
        ],
        "y\n",
    );
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("Export 2 skills to a folder:"), "{said}");
    assert!(to.join("grilling/SKILL.md").exists() && to.join("tdd/SKILL.md").exists());
    assert!(
        !to.join("review").exists(),
        "review was not installed from it"
    );
}

/// **One skill brought back or moved from a chat is proposed too** (#5492), as one deleted is: a
/// loop of singles would be a bulk change.
#[test]
fn a_chats_restore_or_move_of_one_skill_is_proposed_too() {
    for (args, deleted, expected) in [
        (
            &["restore", "tdd"][..],
            true,
            SetChange::Restore {
                members: project_skills(&["tdd"]),
            },
        ),
        (
            &["mv", "tdd", "--to", "ops"],
            false,
            SetChange::Move {
                members: project_skills(&["tdd"]),
                to: Level::workspace("ops"),
                grant: Vec::new(),
            },
        ),
    ] {
        let project = for_case(deleted);
        let (env, asked, _reading) = an_app(&project);
        let before = project.snapshot();
        let out = project.run_with(args, &env);
        assert!(out.status.success(), "{args:?}: {}", said(&out));
        assert_eq!(project.snapshot(), before, "{args:?}");
        match asked.try_recv() {
            Ok(Ask::ProposeSkillSet(ask)) => assert_eq!(ask.change, expected, "{args:?}"),
            _ => panic!("{args:?}: the app was asked nothing"),
        }
    }
}

/// A chat whose app is not there is refused for these verbs too: never made instead.
#[test]
fn a_chat_that_cannot_reach_the_app_restores_moves_and_deletes_now_nothing() {
    for (args, deleted) in LATER_VERBS {
        let project = for_case(*deleted);
        let env = [
            (
                SOCKET_ENV.to_owned(),
                project.at.join("none.sock").display().to_string(),
            ),
            (CHAT_ENV.to_owned(), "3".to_owned()),
        ];
        let before = project.snapshot();
        let out = project.run_with(args, &env);
        assert!(!out.status.success(), "{args:?}: {}", said(&out));
        assert!(
            said(&out).contains("nothing was changed"),
            "{args:?}: {}",
            said(&out)
        );
        assert_eq!(project.snapshot(), before, "{args:?}");
    }
}

/// A rename and a restore under a new name are of one skill: with several, nothing changes.
#[test]
fn a_rename_or_a_new_name_with_several_skills_changes_nothing() {
    for (args, deleted) in [
        (&["mv", "grilling", "tdd", "--name", "x"][..], false),
        (&["restore", "grilling", "tdd", "--as", "x"], true),
    ] {
        let project = for_case(deleted);
        let before = project.snapshot();
        let out = project.run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", said(&out));
        assert!(
            said(&out).contains("nothing was changed"),
            "{args:?}: {}",
            said(&out)
        );
        assert_eq!(project.snapshot(), before, "{args:?}");
    }
}
