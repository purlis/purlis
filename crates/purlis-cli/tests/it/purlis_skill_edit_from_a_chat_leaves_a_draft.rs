// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's `purlis skill edit --from` or `skill new` leaves a draft in the Skill studio**
//! (#5815; spec #2762 decisions 26 and 38): the command hands the app the new `SKILL.md` on the
//! road a chat's write to a skill already takes (`Ask::Propose`, SK-9), the app keeps it in the
//! studio's Drafts, and the person reviews its diff and accepts or declines it there. Nothing in
//! the skill's folder is written. A text that is no whole skill, or that renames it, is refused
//! as before, and no draft is made.
//!
//! The app here keeps what it is asked with the core's own `drafts::take`, as the window's
//! `skilldrafts::drafted` does, so what the studio would list is read back with `drafts::list`.
#![cfg(unix)]

use crate::support;

use std::path::PathBuf;
use std::process::{Output, Stdio};
use std::sync::{Mutex, mpsc};

use purlis_core::hookwire::{self, Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV};
use purlis_core::skills::verbs::drafts;

const WAS: &str = "---\nname: grilling\ndescription: Grills a plan.\n---\n\nAsk hard questions.\n";
const NOW: &str = "---\nname: grilling\ndescription: Grills a plan.\n---\n\nAsk hard questions, \
                   one at a time.\n";

struct Project {
    _tmp: tempfile::TempDir,
    at: PathBuf,
}

impl Project {
    /// A project with the skill `grilling` at its root.
    fn new() -> Project {
        let tmp = tempfile::tempdir().unwrap();
        let at = tmp.path().canonicalize().unwrap();
        let root = at.join("plane");
        std::fs::create_dir_all(root.join("skills/grilling")).unwrap();
        std::fs::write(root.join("charter.toml"), "[plane]\n").unwrap();
        std::fs::write(root.join("skills/grilling/SKILL.md"), WAS).unwrap();
        std::fs::create_dir_all(at.join("home")).unwrap();
        std::fs::create_dir_all(at.join("chat")).unwrap();
        Project { _tmp: tmp, at }
    }

    fn root(&self) -> PathBuf {
        self.at.join("plane")
    }

    /// Every file below the project's skills, with its text.
    fn skills(&self) -> Vec<(PathBuf, String)> {
        fn walk(dir: &std::path::Path, out: &mut Vec<(PathBuf, String)>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else {
                    out.push((path.clone(), std::fs::read_to_string(&path).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root().join("skills"), &mut out);
        out.sort();
        out
    }

    /// `text` written in the chat's own folder, as a chat writes its new text.
    fn written(&self, name: &str, text: &str) -> PathBuf {
        let path = self.at.join("chat").join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    /// `purlis skill …` with nothing of a chat this test may itself run inside, and `env` added.
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
            .env("CHARTER_NO_BACKGROUND_CHECKS", "1")
            .env("NO_COLOR", "1")
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }
}

fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// **An app listening on a socket in `project`'s home** that keeps a chat's write to a skill's
/// file as a draft, as the window does, and answers as it does. Every ask it is sent is passed on.
fn an_app(
    project: &Project,
) -> (
    Vec<(String, String)>,
    mpsc::Receiver<Ask>,
    hookwire::Reading,
) {
    let home = project.at.join("home");
    let socket = home.join("app").join("hooks.sock");
    let root = project.root();
    let (tx, asked) = mpsc::channel();
    let tx = Mutex::new(tx);
    let listener = Listener::bind(&home, &socket).unwrap();
    let token = listener.tokens().issue_to_this_process(3).unwrap();
    let reading = listener.each_answering(
        Box::new(|_| {}),
        Box::new(move |_, ask| {
            tx.lock().unwrap().send(ask.clone()).unwrap();
            let Ask::Propose(ask) = ask else {
                return Answer::No {
                    why: "this app keeps only drafts".to_owned(),
                };
            };
            let Some(file) = drafts::draftable(&root, &root.join(&ask.path)) else {
                return Answer::No {
                    why: "not a skill's file".to_owned(),
                };
            };
            match drafts::take(&root, ask.chat, &file, &ask.change) {
                Ok(_) => Answer::Said {
                    lines: vec![purlis_core::repocmd::Say::Done(drafts::DRAFTED_SAID.into())],
                    code: 0,
                },
                Err(why) => Answer::No { why },
            }
        }),
    );
    let env = vec![
        (SOCKET_ENV.to_owned(), socket.display().to_string()),
        (CHAT_ENV.to_owned(), "3".to_owned()),
        (TOKEN_ENV.to_owned(), token.expose().to_owned()),
        (
            purlis_core::sandboxblock::CHAT_DIR_ENV.to_owned(),
            project.at.join("chat").display().to_string(),
        ),
    ];
    (env, asked, reading)
}

/// **Acceptance 1**: the chat's new text is a draft of that skill, which the studio lists with
/// what the skill holds now and what the draft makes it: its diff, to accept or decline.
#[test]
fn a_chats_edit_from_a_file_becomes_a_draft_the_studio_lists_with_its_diff() {
    let project = Project::new();
    let (env, _asked, _reading) = an_app(&project);
    let from = project.written("grilling.md", NOW);
    let out = project.run_with(
        &["edit", "grilling", "--from", from.to_str().unwrap()],
        &env,
    );
    assert!(out.status.success(), "{}", said(&out));
    let listed = drafts::list(&project.root());
    assert_eq!(listed.len(), 1, "{listed:?}");
    let draft = &listed[0];
    assert_eq!(draft.name, "grilling");
    assert_eq!(draft.chat, Some(3));
    assert!(!draft.new);
    assert_eq!(draft.files.len(), 1);
    assert_eq!(draft.files[0].path, "SKILL.md");
    assert_eq!(draft.files[0].before.as_deref(), Some(WAS));
    assert_eq!(draft.files[0].after, NOW);
    assert!(!draft.stale());
}

/// **Acceptance 2**: until the person accepts it, the skill's folder is as it was.
#[test]
fn a_chats_edit_writes_nothing_in_the_skills_folder() {
    let project = Project::new();
    let (env, _asked, _reading) = an_app(&project);
    let before = project.skills();
    let from = project.written("grilling.md", NOW);
    let out = project.run_with(
        &["edit", "grilling", "--from", from.to_str().unwrap()],
        &env,
    );
    assert!(out.status.success(), "{}", said(&out));
    assert_eq!(project.skills(), before);
}

/// **Acceptance 3**: what the command says names the draft and where it waits, and never that
/// nothing was written, as a refusal says.
#[test]
fn a_chats_edit_names_the_draft_and_where_it_waits() {
    let project = Project::new();
    let (env, _asked, _reading) = an_app(&project);
    let from = project.written("grilling.md", NOW);
    let out = project.run_with(
        &["edit", "grilling", "--from", from.to_str().unwrap()],
        &env,
    );
    let said = said(&out);
    assert!(out.status.success(), "{said}");
    assert!(said.contains("A draft of grilling"), "{said}");
    assert!(said.contains("Skill studio, under Drafts"), "{said}");
    assert!(!said.contains("Nothing was written"), "{said}");
}

/// **Acceptance 4**: a text that is no whole skill, or one that renames it, is refused as at the
/// person's terminal, and the app is asked for no draft.
#[test]
fn a_text_that_is_no_whole_skill_or_renames_it_is_refused_and_no_draft_is_made() {
    let project = Project::new();
    let (env, asked, _reading) = an_app(&project);
    let renamed = WAS.replace("name: grilling", "name: roasting");
    for (file, text) in [
        ("partial.md", "Ask hard questions, one at a time.\n"),
        ("renamed.md", renamed.as_str()),
    ] {
        let from = project.written(file, text);
        let out = project.run_with(
            &["edit", "grilling", "--from", from.to_str().unwrap()],
            &env,
        );
        assert!(!out.status.success(), "{file}: {}", said(&out));
        assert!(
            said(&out).contains("nothing was saved"),
            "{file}: {}",
            said(&out)
        );
    }
    assert!(asked.try_recv().is_err(), "the app was asked for a draft");
    assert!(drafts::list(&project.root()).is_empty());
    assert_eq!(
        std::fs::read_to_string(project.root().join("skills/grilling/SKILL.md")).unwrap(),
        WAS
    );
}

/// A text the same as the skill's is no change, and makes no draft.
#[test]
fn a_chats_edit_that_changes_nothing_makes_no_draft() {
    let project = Project::new();
    let (env, asked, _reading) = an_app(&project);
    let from = project.written("same.md", WAS);
    let out = project.run_with(
        &["edit", "grilling", "--from", from.to_str().unwrap()],
        &env,
    );
    assert!(out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("unchanged"), "{}", said(&out));
    assert!(asked.try_recv().is_err(), "the app was asked for a draft");
    assert!(drafts::list(&project.root()).is_empty());
}

/// **`skill new` from a chat** is a draft of a new skill: no folder is made for it.
#[test]
fn a_chats_new_skill_becomes_a_draft_of_a_new_skill_and_makes_no_folder() {
    let project = Project::new();
    let (env, _asked, _reading) = an_app(&project);
    let out = project.run_with(&["new", "roasting", "-d", "Roasts a plan gently."], &env);
    let said = said(&out);
    assert!(out.status.success(), "{said}");
    assert!(said.contains("A draft of roasting"), "{said}");
    assert!(!project.root().join("skills/roasting").exists());
    let listed = drafts::list(&project.root());
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert_eq!(listed[0].name, "roasting");
    assert!(listed[0].new);
    assert!(listed[0].files[0].after.contains("name: roasting"));
    assert!(listed[0].files[0].before.is_none());
}

/// A new skill whose name the project's skills already have is refused, and makes no draft.
#[test]
fn a_chats_new_skill_with_a_name_taken_makes_no_draft() {
    let project = Project::new();
    let (env, asked, _reading) = an_app(&project);
    let out = project.run_with(&["new", "grilling", "-d", "Grills again."], &env);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(asked.try_recv().is_err(), "the app was asked for a draft");
    assert!(drafts::list(&project.root()).is_empty());
}

/// **A chat with every mark the app set stripped is still a chat** (#5357): known by where it
/// runs, it writes nothing to the skill, and with no way left to the app it makes no draft
/// either, and says so. It never falls back to saving the edit itself.
#[test]
fn a_chat_with_every_mark_stripped_edits_nothing_and_makes_no_draft() {
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
    let before = project.skills();
    let from = project.written("grilling.md", NOW);
    let out = project.run_with(&["edit", "grilling", "--from", from.to_str().unwrap()], &[]);
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("no draft was made"), "{}", said(&out));
    assert_eq!(project.skills(), before);
    assert!(drafts::list(&project.root()).is_empty());
}

/// A chat whose app is not there is refused the same way: no draft, nothing written.
#[test]
fn a_chat_that_cannot_reach_the_app_edits_nothing_and_makes_no_draft() {
    let project = Project::new();
    let env = [
        (
            SOCKET_ENV.to_owned(),
            project.at.join("none.sock").display().to_string(),
        ),
        (CHAT_ENV.to_owned(), "3".to_owned()),
    ];
    let from = project.written("grilling.md", NOW);
    let out = project.run_with(
        &["edit", "grilling", "--from", from.to_str().unwrap()],
        &env,
    );
    assert!(!out.status.success(), "{}", said(&out));
    assert!(said(&out).contains("no draft was made"), "{}", said(&out));
    assert_eq!(
        std::fs::read_to_string(project.root().join("skills/grilling/SKILL.md")).unwrap(),
        WAS
    );
}

/// **A chat can't accept its own draft**: Accept and Decline are the window's alone, and no
/// `purlis skill` command a chat runs takes a draft into the skill.
#[test]
fn a_chat_has_no_command_that_accepts_its_draft() {
    let project = Project::new();
    let (env, _asked, _reading) = an_app(&project);
    let from = project.written("grilling.md", NOW);
    let out = project.run_with(
        &["edit", "grilling", "--from", from.to_str().unwrap()],
        &env,
    );
    assert!(out.status.success(), "{}", said(&out));
    let id = drafts::list(&project.root())[0].id.clone();
    for args in [
        &["accept", id.as_str()][..],
        &["draft", "accept", id.as_str()],
        &["drafts", "accept", id.as_str()],
        &["edit", "grilling", "--accept", id.as_str()],
    ] {
        let out = project.run_with(args, &env);
        assert!(!out.status.success(), "{args:?}: {}", said(&out));
    }
    assert_eq!(
        std::fs::read_to_string(project.root().join("skills/grilling/SKILL.md")).unwrap(),
        WAS
    );
    assert_eq!(drafts::list(&project.root()).len(), 1);
}
