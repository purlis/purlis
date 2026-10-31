// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A Claude Code chat on a runner, wrapped whole by purlis** (#4083, ruled by the operator
//! 2026-10-30, ADR 0067 as amended for it). A runner has neither bubblewrap nor socat, and a pod
//! or a default container gives no user namespace to make, so Claude Code's own sandbox cannot
//! start there. The chat runs inside purlis's Linux wrap as Codex and opencode do (#1040), and
//! its own sandbox is switched off (`"sandbox": {"enabled": false}` in its `--settings`). On a
//! person's own Linux machine a Claude Code chat keeps its own sandbox.
//!
//! What being wrapped whole changes, from what [`super::claude`] says of a chat on its own:
//!
//! - **Its hooks, its MCP servers and its own process run inside.** So they reach the hook socket
//!   through the wrap, and nothing that waits for a chat is denied
//!   ([`super::Denied::and_what_waits_for_a_chat`]), as for Codex.
//! - **Its traffic goes through the proxy**, its model's among it, whichever version it is, so
//!   the project's hosts must list its model's (the `model-providers` preset does). What it
//!   would send on its own account past that list is off ([`QUIET_ENV`]).
//! - **Its sign-in is not kept from its commands**: the harness reads `.credentials.json`, and
//!   the commands it runs are held to the same wrap, as Codex's are. Its Read and Edit tools are
//!   still denied it, and its declaration says so.
//! - **Its Bash tool asks.** Its own sandbox's `autoAllowBashIfSandboxed`, on by default, is off
//!   with the sandbox, and every command would stop on a prompt; so `Bash` is allowed instead
//!   ([`super::claude::Settings::is_off`]). A deny or an ask still outranks it, and every hook
//!   runs as before.
//! - **Its config folder is written only where a turn writes** ([`HOME_DIRS`], [`HOME_FILES`]),
//!   at the floor too, and its settings, hooks, skills, commands, agents, plugins and
//!   `.claude.json` never are. `.claude.json` is where user-scoped MCP servers and each folder's
//!   trust are kept, and a chat that wrote it would choose what a later chat on the profile
//!   runs, so it is held like Codex's config: only purlis writes it, before the wrap
//!   ([`prepare`]), and Claude Code's own saves of it are refused.
//! - **The residual** is Codex's (D-88s): every folder in [`HOME_DIRS`] is shared by the chats on
//!   one profile, so one chat can leave a transcript or a project memory a later chat on it loads.
//!   Only wrapped chats: on a runner a Claude Code chat never starts without the sandbox
//!   ([`crate::start::profile_refusal`], ruled 2026-10-30), as a sandboxed Codex chat starts only
//!   on a profile for sandboxed chats (D-88q).
//! - **What only its own sandbox needed goes**: the pins that hold a git folder
//!   ([`super::Denied::git_folder_pins`]), which a wrap holds by its own rules; the home's empty
//!   `.gitconfig` (#4006), which only bubblewrap's handling of a missing path asked for; and
//!   `network.allowUnixSockets`, since the wrap's profile names each socket itself. Its ports
//!   (#2019) stay as they are: no door opens for a Claude Code chat.

use std::path::{Path, PathBuf};

use super::claude::Settings;
use super::seatbelt::{self, Own, quote, string, under};
use super::{Compiled, Denial, Os, Uncompilable, Unheld};
use crate::harness::Harness;

/// What a Claude Code chat is wrapped in, before the place it opens is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wrap {
    /// The system it is applied on, whose mechanism applies it ([`super::wrapper`]).
    pub os: Os,
    /// How much it holds ([`super::Level`]): at the floor every folder is written and the
    /// network is open, but what [`Self::denied`] and [`Self::held`] name.
    pub level: super::Level,
    /// At the floor, the folders the chat may not write that no class names
    /// ([`super::Compiled::held`]).
    pub held: Vec<PathBuf>,
    /// Every path the chat is denied.
    pub denied: Vec<Denial>,
    /// What its presets widen past the hosts: the package caches and the certificate check.
    pub widened: Box<super::Widened>,
    /// The folders a person let this chat write besides its own (#1342).
    pub granted: Vec<PathBuf>,
    /// Claude Code's config folder for this chat, of which only what a turn writes is writable:
    /// its profile's ([`Self::on_profile`]). None until it is known, and a chat with none is not
    /// started.
    pub home: Option<PathBuf>,
    /// Claude Code's state file for this chat ([`STATE_FILE`]): in its config folder, or in the
    /// home on a profile whose environment names no config folder. Only purlis writes it.
    pub state: Option<PathBuf>,
    /// The folders of this machine's profiles (`<data>/profiles`), denied to a chat but its own
    /// profile's.
    pub profiles: Option<PathBuf>,
    /// The plane roots whose `workspaces` folder and each workspace's folder in it no chat
    /// makes, moves or removes (#2046).
    pub planes: Vec<PathBuf>,
    /// What its own tools are denied, as [`super::claude::settings`] compiles it: its Read and
    /// Edit tools and, at the full sandbox, its web tools still answer to these. Its `sandbox`
    /// object is never handed over ([`Self::settings`]).
    pub rules: Settings,
}

/// The folders in Claude Code's config folder a turn writes into, each everything under it and
/// never itself: made by purlis before the wrap ([`prepare`]).
///
/// `plugins/data` is the one place under `plugins` it writes: Claude Code makes a plugin's data
/// folder there before it runs that plugin's hook, and a hook is not run where it cannot
/// (measured on the parity job). A plugin's code is never below it.
pub const HOME_DIRS: [&str; 16] = [
    "backups",
    "cache",
    "debug",
    "file-history",
    "logs",
    "paste-cache",
    "plans",
    "plugins/data",
    "projects",
    "session-env",
    "sessions",
    "shell-snapshots",
    "statsig",
    "tasks",
    "telemetry",
    "todos",
];

/// The files in Claude Code's config folder a turn writes: its history, its usage figures, and
/// its sign-in, which a refresh rewrites.
pub const HOME_FILES: [&str; 3] = [
    "history.jsonl",
    "stats-cache.json",
    super::signin::CLAUDE_CODE_CREDENTIALS,
];

/// The file Claude Code keeps its account, its user-scoped MCP servers and each folder's trust
/// in: in its config folder where `CLAUDE_CONFIG_DIR` names one, else in the home.
pub const STATE_FILE: &str = ".claude.json";

/// The variable that moves Claude Code's own temporary files. Left alone it writes them under
/// `/tmp/claude-<uid>`, which a wrapped chat may not.
pub const TMP_ENV: &str = "CLAUDE_CODE_TMPDIR";

/// What keeps Claude Code's telemetry and error reports from asking a person about hosts they
/// never listed, through the proxy that now carries them.
pub const QUIET_ENV: &str = "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC";

/// Why a chat is not wrapped: its directory holds Claude Code's own files, or is inside them.
pub const OVERLAP: &str = "purlis cannot wrap a Claude Code chat whose directory holds Claude \
                           Code's own files, or is inside them";

/// `compiled`, for a Claude Code chat purlis wraps, or why it cannot be wrapped on this system.
pub fn wrap(compiled: &Compiled) -> Result<Wrap, Uncompilable> {
    if compiled.os != Os::Linux {
        return Err(Uncompilable {
            harness: Harness::ClaudeCode,
            unheld: Unheld::Wrap(compiled.os),
        });
    }
    Ok(Wrap {
        os: compiled.os,
        level: compiled.level,
        held: compiled.held.clone(),
        denied: compiled.denied.paths.clone(),
        widened: Box::new(compiled.widened.clone()),
        granted: compiled.writable.clone(),
        home: None,
        state: None,
        profiles: compiled.homes.profiles.clone(),
        planes: Vec::new(),
        rules: super::claude::rules(compiled),
    })
}

impl Wrap {
    /// This wrap for a chat whose config folder is `folders`' own: as the kernel names it, where
    /// it is this user's folder, or nothing there yet. Anything else leaves the chat with no
    /// config folder, and it is not started.
    pub fn on_profile(&mut self, folders: &super::signin::ProfileFolders) {
        self.home = folders.config.as_deref().and_then(held_folder);
        self.state = self.home.as_ref().map(|home| {
            folders
                .state
                .clone()
                .unwrap_or_else(|| home.join(STATE_FILE))
        });
    }

    /// `more` held at the floor besides what was compiled.
    pub fn hold(&mut self, more: Vec<PathBuf>) {
        self.rules = self.rules.holding(&more);
        self.held.extend(more);
    }

    /// `more` denied besides what was compiled.
    pub fn denying(&mut self, more: Vec<Denial>) {
        self.rules = self.rules.denying(&more);
        self.denied.extend(more);
    }

    /// What this chat's `--settings` carry for the sandbox: its own switched off, and its tools'
    /// rules as compiled.
    pub fn settings(&self) -> Settings {
        Settings {
            sandbox: serde_json::json!({ "enabled": false }),
            deny: self.rules.deny.clone(),
            made_empty: Vec::new(),
        }
    }

    /// Every path the chat is denied, as the profile is written: the folders of this machine's
    /// profiles left out where the chat's own is among them, since [`own`] denies them but that
    /// one.
    fn denied_here(&self) -> Vec<Denial> {
        let carved = self.carved();
        self.denied
            .iter()
            .filter(|denial| carved.is_none_or(|profiles| denial.path != profiles))
            .cloned()
            .collect()
    }

    /// The folder of this machine's profiles, where the chat's config folder is one of them.
    fn carved(&self) -> Option<&Path> {
        let (home, profiles) = (self.home.as_deref()?, self.profiles.as_deref()?);
        (home.starts_with(profiles) || home.starts_with(super::real(profiles))).then_some(profiles)
    }

    /// What the wrap lets a chat write in its config folder.
    pub fn writable(&self) -> Vec<PathBuf> {
        let Some(home) = &self.home else {
            return Vec::new();
        };
        HOME_DIRS
            .iter()
            .chain(HOME_FILES.iter())
            .map(|name| home.join(name))
            .collect()
    }
}

/// `path` as the kernel names it, where it may be a chat's config folder: an absolute path, this
/// user's own folder or nothing yet, and never a link.
fn held_folder(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    match std::fs::symlink_metadata(path) {
        Ok(found) if found.is_dir() && owned_here(&found) => Some(super::real(path)),
        Ok(_) => None,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Some(super::real(path)),
        Err(_) => None,
    }
}

#[cfg(unix)]
fn owned_here(found: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    found.uid() == rustix::process::geteuid().as_raw()
}

#[cfg(not(unix))]
fn owned_here(_found: &std::fs::Metadata) -> bool {
    false
}

/// The profile for a chat in `cwd`, with its own temp directory `tmp`, reaching the network
/// through its own proxy's `proxy_ports` and reporting on `hook_socket`.
pub fn profile(
    wrap: &Wrap,
    cwd: &Path,
    tmp: &Path,
    proxy_ports: &[u16],
    hook_socket: Option<&Path>,
) -> Result<String, &'static str> {
    let own = own(wrap)?;
    if wrap.level == super::Level::Floor {
        return seatbelt::floor_profile(
            &wrap.denied_here(),
            &wrap.held,
            &own,
            proxy_ports,
            hook_socket,
        );
    }
    seatbelt::profile(
        &wrap.denied_here(),
        &own,
        cwd,
        tmp,
        proxy_ports,
        hook_socket,
    )
}

/// What a wrapped Claude Code keeps for itself.
fn own(wrap: &Wrap) -> Result<Own, &'static str> {
    let mut own = Own {
        overlap: OVERLAP,
        ..Own::default()
    };
    seatbelt::widen(&mut own, &wrap.widened)?;
    seatbelt::granted(&mut own, &wrap.granted)?;
    own.deny
        .extend(seatbelt::workspace_folder_rules(&wrap.planes)?);
    let Some(home) = &wrap.home else {
        return Ok(own);
    };
    own.dirs.push(home.clone());
    if let Some(profiles) = wrap.carved() {
        super::codex::carve(&mut own, profiles, home)?;
    }
    let written = written_in(home);
    own.allow.push(format!("(regex {})", string(&written)?));
    own.roots.extend(HOME_DIRS.iter().map(|dir| home.join(dir)));
    // At the floor too, where nothing else lists what is written: nothing else in the folder
    // is, no second name is made for anything in it, and neither it nor a folder above it moves.
    own.deny.push(format!(
        "(deny file-write* file-link (require-all (subpath {}) (require-not (regex {}))))",
        quote(home)?,
        string(&written)?
    ));
    for above in home.ancestors().filter(|above| above.parent().is_some()) {
        own.deny
            .push(format!("(deny file-write* (literal {}))", quote(above)?));
    }
    // The floor holds `.claude/plugins` by name wherever it is, which a home of that name is.
    if wrap.level == super::Level::Floor {
        own.after.push(format!(
            "(allow file-write* (regex {}))",
            string(&under(home, "plugins/data/.+"))?
        ));
        // And still no link made there, after the allow that would let one through.
        own.after.push(format!(
            "(deny file-write-create (require-all (vnode-type SYMLINK) (subpath {})))",
            quote(home)?
        ));
    }
    // No link made where a later Claude Code writes, which it would write through.
    own.deny.push(format!(
        "(deny file-write-create (require-all (vnode-type SYMLINK) (subpath {})))",
        quote(home)?
    ));
    Ok(own)
}

/// The regular expression of what a turn writes in `home`.
fn written_in(home: &Path) -> String {
    let files: Vec<String> = HOME_FILES
        .iter()
        .map(|file| seatbelt::escaped(file))
        .collect();
    under(
        home,
        &format!("(({})/.+|({})$)", HOME_DIRS.join("|"), files.join("|")),
    )
}

/// The chat's config folder, made ready before the wrap for a chat in `cwd`, which lets a chat
/// make none of it, and only once the chat's line is built:
/// - its folders, empty, where missing, and the folder itself readable by its owner alone;
/// - in its folders, every link taken out, so nothing moved in with one stays;
/// - in [`STATE_FILE`], which the chat never writes, what a chat could not save itself: that
///   this profile is past its first run, and that `cwd` is trusted, a folder the person moved
///   here themselves, whose project settings are screened all the same
///   ([`crate::harness::claude::project`]). What else the file holds stays as it is, and a file
///   purlis cannot read is left alone, for Claude Code to say so. The mark is read only by the
///   wrapped chats on this profile: on a runner no Claude Code chat starts without the sandbox
///   ([`crate::start::profile_refusal`]).
pub fn prepare(wrap: &Wrap, cwd: &Path) {
    let Some(home) = &wrap.home else { return };
    for dir in HOME_DIRS {
        let _ = std::fs::create_dir_all(home.join(dir));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700));
    }
    for dir in HOME_DIRS {
        no_links_below(&home.join(dir));
    }
    let state = wrap.state.clone().unwrap_or_else(|| home.join(STATE_FILE));
    let mut doc = match std::fs::symlink_metadata(&state) {
        Ok(found) if found.is_file() => {
            match std::fs::read_to_string(&state)
                .ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            {
                Some(doc) if doc.is_object() => doc,
                _ => return,
            }
        }
        Ok(_) => return,
        Err(_) => serde_json::json!({}),
    };
    doc["hasCompletedOnboarding"] = true.into();
    if !doc["projects"].is_object() {
        doc["projects"] = serde_json::json!({});
    }
    let folder = super::real(cwd).display().to_string();
    if !doc["projects"][&folder].is_object() {
        doc["projects"][&folder] = serde_json::json!({});
    }
    doc["projects"][&folder]["hasTrustDialogAccepted"] = true.into();
    super::codex::replace(&state, doc.to_string().as_bytes(), 0o600);
}

/// Below `dir`, without following a link: every link taken out.
fn no_links_below(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            let _ = std::fs::remove_file(entry.path());
        } else if kind.is_dir() {
            no_links_below(&entry.path());
        }
    }
}
