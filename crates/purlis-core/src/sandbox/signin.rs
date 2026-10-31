// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A profile's sign-in, kept from the agent's commands** (ADR 0095 §5, #2037).
//!
//! A chat runs on one profile, and the profile's folders hold its harness's sign-in. Each
//! harness keeps its sign-in in files of its own, named here ([`credential_files`]): Claude
//! Code's `.credentials.json` in its config folder, Codex's `auth.json` in its home, and
//! opencode's `auth.json` and `mcp-auth.json` in its data folder. They are the sign-in class of
//! a profile, and every sandbox compiler denies them to the agent's commands, reading and
//! writing, wherever the harness runs its commands apart from itself
//! ([`super::Applied::on_profile`]): Claude Code's sandbox holds only the commands its Bash tool
//! runs, so its own process still reads them. Codex and opencode run whole inside purlis's wrap
//! with their commands, so the wrap cannot deny what the harness itself reads, and their
//! declarations say so (`[sign_in] kept_from_commands`, ADR 0073).
//!
//! Where a profile's sign-in is not kept from its commands, the profile page says so plainly
//! ([`commands_can_read`]).

use std::path::PathBuf;

use super::{Homes, Machine};
use crate::harness::Harness;
use crate::harness_declaration::Answer;

/// The folders a chat's profile keeps its harness's files in: the folders the harness's own
/// folder variables (`[folders]` of its declaration) name in the chat's environment, and the
/// harness's own default folders where they name none, which is where a chat on an adopted
/// profile reaches its sign-in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileFolders {
    /// Its settings and sign-in: Claude Code's config folder, Codex's home, opencode's XDG
    /// config folder.
    pub config: Option<PathBuf>,
    /// Its data, where the harness declares a folder of its own for it: opencode's XDG data
    /// folder.
    pub data: Option<PathBuf>,
    /// The profile's name, where it is one this machine keeps for sandboxed chats only
    /// (#2037): what a wrapped Codex's home is checked against.
    pub sandbox_only: Option<String>,
    /// Where Claude Code keeps its state file (`.claude.json`) for this chat: in its config
    /// folder where the chat's environment names one, and in the home where it names none, as
    /// Claude Code reads it (#4083). None for every other harness.
    pub state: Option<PathBuf>,
    /// The harness's own default config folder on this machine, whatever the chat's environment
    /// names: what a wrapped Claude Code on another profile is kept from ([`super::Applied`]).
    pub default_config: Option<PathBuf>,
}

impl ProfileFolders {
    /// The folders a chat of `harness` started with `env` on `machine` reaches.
    pub fn of(harness: Harness, env: &[(String, String)], machine: &Machine) -> Self {
        let homes = Homes::of(machine);
        let folders = &crate::harness_declaration::of(harness).folders;
        let set = |variable: &Option<String>| {
            let variable = variable.as_deref()?;
            env.iter()
                .rev()
                .find(|(name, _)| name == variable)
                .map(|(_, value)| PathBuf::from(value))
                .filter(|path| path.is_absolute())
        };
        let (config, data) = match harness {
            Harness::ClaudeCode => (homes.home.as_ref().map(|home| home.join(".claude")), None),
            Harness::Codex => (homes.codex.clone(), None),
            Harness::Opencode => (homes.config.clone(), homes.data.clone()),
        };
        let default_config = config.clone();
        let named = set(&folders.config_env);
        let state = (harness == Harness::ClaudeCode)
            .then(|| match &named {
                Some(folder) => Some(folder.join(super::claude_wrap::STATE_FILE)),
                None => homes
                    .home
                    .as_ref()
                    .map(|home| home.join(super::claude_wrap::STATE_FILE)),
            })
            .flatten();
        Self {
            config: named.or(config),
            data: set(&folders.data_env).or(data),
            sandbox_only: None,
            state,
            default_config,
        }
    }
}

/// **The folder `harness`'s default sign-in is in on `machine`**, whatever a chat's environment
/// names: Claude Code's and Codex's config folders, and opencode's own folder of its data.
fn default_sign_in_folder(harness: Harness, machine: &Machine) -> Option<PathBuf> {
    let folders = ProfileFolders::of(harness, &[], machine);
    match harness {
        Harness::ClaudeCode | Harness::Codex => folders.config,
        Harness::Opencode => folders.data.map(|data| data.join("opencode")),
    }
}

/// **Every other harness's default sign-in folder on `machine`**: no chat of `harness` reads or
/// writes them, the folder as well as the file, so none moves the folder aside either.
pub fn other_harnesses_folders(harness: Harness, machine: &Machine) -> Vec<PathBuf> {
    Harness::ALL
        .into_iter()
        .filter(|other| *other != harness)
        .filter_map(|other| default_sign_in_folder(other, machine))
        .collect()
}

/// The files `harness` keeps a profile's sign-in in, under `folders`: the class each sandbox
/// compiler denies to the agent's commands where the harness runs them apart from itself.
pub fn credential_files(harness: Harness, folders: &ProfileFolders) -> Vec<PathBuf> {
    let under = |folder: &Option<PathBuf>, names: &[&str]| -> Vec<PathBuf> {
        folder
            .iter()
            .flat_map(|folder| names.iter().map(move |name| folder.join(name)))
            .collect()
    };
    match harness {
        Harness::ClaudeCode => under(&folders.config, &[CLAUDE_CODE_CREDENTIALS]),
        Harness::Codex => under(&folders.config, &[CODEX_CREDENTIALS]),
        Harness::Opencode => under(
            &folders.data.as_ref().map(|data| data.join("opencode")),
            &super::opencode::CREDENTIALS,
        ),
    }
}

/// Claude Code's sign-in file in its config folder, where it keeps one in a file.
pub const CLAUDE_CODE_CREDENTIALS: &str = ".credentials.json";

/// Codex's sign-in file in its home.
pub const CODEX_CREDENTIALS: &str = "auth.json";

/// **What the profile page says** for a profile of the harness `kind` whose sign-in the
/// commands its chats run can read: a sentence, with the reason its declaration gives; `None`
/// where the declaration answers yes. A harness purlis does not know, or one whose answer is
/// unknown, is said to let them read it, since only a yes counts (ADR 0073 §6).
pub fn commands_can_read(kind: &str) -> Option<String> {
    let answer = crate::harness_declaration::builtin(kind).map_or(Answer::Unknown, |declared| {
        declared.sign_in.kept_from_commands.clone()
    });
    match answer {
        Answer::Yes => None,
        Answer::No(reason) => Some(format!(
            "The commands this profile's chats run can read its sign-in: {}.",
            reason.trim_end_matches('.')
        )),
        Answer::Unknown => Some(
            "The commands this profile's chats run can read its sign-in: purlis does not know \
             that its harness keeps it from them."
                .to_owned(),
        ),
    }
}
