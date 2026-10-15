// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The upgrade that adopts this machine's harness folders as profiles loses nothing (ADR 0095
//! §2, #2033). What this proves: every chat's record and the profiles a chat starts from are as
//! they were, so each chat's next run starts exactly as it did, on the same sign-in; the record
//! adoption writes names the folder that holds each chat's conversation; nothing in a harness's
//! folder changes; and the profiles a clone's local file declares are not adopted.
//!
//! A machine of fixtures: a home with Claude Code's, Codex's and opencode's folders, each
//! holding a chat's conversation, and a project whose record holds those chats and one on a
//! profile of its local file. Never the real home.

use std::fs;
use std::path::{Path, PathBuf};

use purlis_core::harness::SessionId;
use purlis_core::reopen::{self, Chat, Record};
use purlis_core::{profiles, profilestore};

struct Machine {
    _dir: tempfile::TempDir,
    home: PathBuf,
    config: PathBuf,
    project: PathBuf,
}

impl Machine {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        let config = dir.path().join("config");
        let project = dir.path().join("project");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("charter.toml"), "").unwrap();
        fs::write(
            project.join(profiles::LOCAL_FILE),
            "[harness.work]\nkind = \"claude\"\ncommand = [\"claude\"]\n\
             env = { CLAUDE_CONFIG_DIR = \"~/.claude-work\" }\n",
        )
        .unwrap();
        Self {
            _dir: dir,
            home,
            config,
            project,
        }
    }

    /// Writes `text` at `relative` under the home, and answers where.
    fn keeps(&self, relative: &str, text: &str) -> PathBuf {
        let at = self.home.join(relative);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(&at, text).unwrap();
        at
    }

    fn upgrade(&self) -> Vec<profilestore::MachineProfile> {
        profilestore::adopt(&self.config, &self.home, &|_: &str| None)
            .expect("it adopts")
            .adopted
    }
}

fn chat(profile: &str, resume: &str) -> Chat {
    Chat {
        program: profile.to_owned(),
        name: format!("{profile}.1"),
        resume: Some(SessionId::new(resume).unwrap()),
        active: true,
        profile: Some(profile.to_owned()),
        ..Default::default()
    }
}

/// Every file under `dir` with its bytes.
fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(at) = todo.pop() {
        for entry in fs::read_dir(&at).unwrap() {
            let entry = entry.unwrap().path();
            if entry.is_dir() {
                todo.push(entry);
            } else {
                out.push((entry.clone(), fs::read(&entry).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn after_the_upgrade_each_chat_starts_from_the_same_profile_and_the_record_names_the_folder_of_its_conversation()
 {
    purlis_core::unsteered!();
    let machine = Machine::new();
    let cwd_key = purlis_core::harness_declaration::cwd_key(&machine.project);
    let claude_transcript = machine.keeps(
        &format!(".claude/projects/{cwd_key}/c1a0de00-0000-4000-8000-000000000001.jsonl"),
        "{\"type\":\"user\"}\n",
    );
    machine.keeps(".claude/history.jsonl", "{\"display\":\"hello\"}\n");
    let codex_session = machine.keeps(
        ".codex/sessions/2026/10/01/rollout-2026-10-01T10-00-00-0c0de000-0000-4000-8000-000000000002.jsonl",
        "{}\n",
    );
    let opencode_session = machine.keeps(
        ".local/share/opencode/storage/session/ses_open3.json",
        "{}\n",
    );
    reopen::write(
        &machine.project,
        &Record {
            chats: vec![
                chat("claude", "c1a0de00-0000-4000-8000-000000000001"),
                chat("codex", "0c0de000-0000-4000-8000-000000000002"),
                chat("opencode", "ses_open3"),
                chat("work", "a0000000-0000-4000-8000-000000000004"),
            ],
            ..Default::default()
        },
    )
    .unwrap();
    let launched_before = profiles::current(&machine.project);
    let home_before = tree(&machine.home);

    let adopted = machine.upgrade();

    let names: Vec<&str> = adopted.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["claude", "opencode", "codex"]);
    // Each chat's record is read back unchanged, and its profile is the one it ran on before.
    let record = reopen::read_or_refusal(&machine.project).unwrap();
    let launched_after = profiles::current(&machine.project);
    assert_eq!(
        launched_after, launched_before,
        "a chat's next run starts from the same profiles, so on the same sign-in"
    );
    let listed = profilestore::list(&machine.config).unwrap();
    for chat in &record.chats {
        let name = chat.profile.as_deref().unwrap();
        let resume = chat.resume.as_ref().unwrap().as_str();
        let on = launched_after
            .get(name)
            .expect("the chat's profile is still there");
        if name == "work" {
            // The local file's profile is not adopted, and runs as it is declared.
            assert!(profilestore::adopted_as(&listed, name).is_none());
            assert_eq!(on.source, profiles::Source::Local);
            assert_eq!(
                on.env,
                vec![("CLAUDE_CONFIG_DIR".to_owned(), "~/.claude-work".to_owned())]
            );
            continue;
        }
        let profile = profilestore::adopted_as(&listed, name).expect("adopted");
        assert_eq!(on.source, profiles::Source::BuiltIn);
        // The conversation is where the adopted folder keeps it.
        let conversation = match name {
            "claude" => profile
                .folders
                .config
                .join("projects")
                .join(&cwd_key)
                .join(format!("{resume}.jsonl")),
            "codex" => profile.folders.config.join(
                "sessions/2026/10/01/rollout-2026-10-01T10-00-00-0c0de000-0000-4000-8000-000000000002.jsonl",
            ),
            _ => profile
                .folders
                .data
                .clone()
                .unwrap()
                .join(format!("opencode/storage/session/{resume}.json")),
        };
        let expected = match name {
            "claude" => &claude_transcript,
            "codex" => &codex_session,
            _ => &opencode_session,
        };
        assert_eq!(&conversation, expected, "{name}");
        assert!(conversation.is_file(), "{name}: {}", conversation.display());
    }
    assert_eq!(
        tree(&machine.home),
        home_before,
        "nothing in a harness's folder was copied, moved or rewritten"
    );
}

#[test]
fn a_second_upgrade_is_a_no_op() {
    purlis_core::unsteered!();
    let machine = Machine::new();
    machine.keeps(".claude/history.jsonl", "{}\n");
    machine.upgrade();
    let first = profilestore::list(&machine.config).unwrap();

    let again = machine.upgrade();

    assert_eq!(again, Vec::new());
    assert_eq!(profilestore::list(&machine.config).unwrap(), first);
}
