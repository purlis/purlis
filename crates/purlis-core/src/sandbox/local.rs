// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What this machine keeps about a project's sandbox, beside the project and never committed:
//! the answer to the one-time offer (ADR 0067 §1, ruling V21 1), how many new chats started
//! with the sandbox and without it — the opt-out rate (ADR 0067 §7, V12) — and the project's
//! hosts as this machine last told the person of them (#1341).
//!
//! **Local, and never sent** (ruling V78 d). The count is shown by `charter doctor` and the
//! Settings tab, and nothing reads it for anything else.
//!
//! **Where (ADR 0093):** in the person's own config home,
//! `~/.config/purlis/sandbox-grants/<clone key>.json` ([`path`]), **never in the project**. A
//! file in the project's tree is one a repository can carry, and one it carried could grant
//! itself a folder, a host, a socket, or accept a dispatch grant, on every machine that opened
//! the clone. purlis's folder in the config home is denied to every chat's writes
//! ([`super::Class::HumanPowers`]), so a sandboxed chat can neither answer the offer nor change
//! what it holds. An unsandboxed chat runs as the person and can write it, as it can write
//! anything of theirs: nothing here is a boundary against that.
//!
//! **The file an older build kept in the project** (`<state>/app/sandbox.json`, [`legacy_path`])
//! is never read (the operator's decision of 2026-10-12): nothing a file in the project says can
//! tell a copy that came with the repository, a zip or a synced folder from this machine's own,
//! so after this update each project's grants and acceptances are asked about again, once.

use std::io;
use std::path::{Path, PathBuf};

use super::{Plane, TABLE};

/// The file an older build kept, relative to the project's state folder ([`legacy_path`]).
pub const IN_STATE: &str = "app/sandbox.json";

/// The file an older build kept in the project at `root`: read once, where it is this
/// machine's, and never written again.
pub fn legacy_path(root: &Path) -> PathBuf {
    crate::names::state(root).join(IN_STATE)
}

/// The folder in purlis's config folder this machine's files are kept in, one per clone.
pub const MACHINE_GRANTS: &str = "sandbox-grants";

#[cfg(test)]
thread_local! {
    /// The config home a test stands in for the person's, so no test reads or writes theirs.
    pub(crate) static TEST_CONFIG_HOME: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// The config home this machine's files go under: the person's, or, in a fenced build (the
/// app's tests, e2e) whose fence it is outside, one of the build's own in the temporary folder.
fn config_home() -> Option<PathBuf> {
    #[cfg(test)]
    {
        if let Some(home) = TEST_CONFIG_HOME.with(|home| home.borrow().clone()) {
            return Some(home);
        }
        static ONE: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
        Some(
            ONE.get_or_init(|| tempfile::tempdir().expect("a config home for the tests"))
                .path()
                .to_path_buf(),
        )
    }
    #[cfg(not(test))]
    {
        let home = crate::machine::config_root_unheld()?;
        if crate::fence::FENCED && !crate::fence::inside(&home, &crate::fence::fence()) {
            // One per user and process: a run that keeps its store across launches (e2e) names
            // one inside its fence, so this is a test process's alone, never shared with another
            // test's, whatever that one does to its folders.
            let uid = rustix::process::getuid().as_raw();
            let pid = std::process::id();
            return Some(std::env::temp_dir().join(format!("purlis-fenced-config-{uid}-{pid}")));
        }
        Some(home)
    }
}

/// **The clone's key here**: its real path, and, where it is a repository, the identity of its
/// `.git` (device and inode), so a different repository cloned into the same folder later is
/// another key, and takes nothing allowed for the one before.
pub fn clone_key(root: &Path) -> String {
    let real = super::real(root);
    key_of(&real, &real)
}

/// The identity of `folder` itself (device and inode), so a folder made again at the same path
/// is another one.
fn identity_of(folder: &Path) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::symlink_metadata(folder) {
            return format!("{}:{}", meta.dev(), meta.ino());
        }
    }
    let _ = folder;
    String::new()
}

/// [`clone_key`] for a clone whose real path was `spelled`, its `.git` now in `at`.
fn key_of(spelled: &Path, at: &Path) -> String {
    use sha2::Digest;
    let mut hash = sha2::Sha256::new();
    hash.update(spelled.as_os_str().as_encoded_bytes());
    // The folder itself, which a rename keeps and a folder made again does not.
    hash.update(format!("\0{}", identity_of(at)).as_bytes());
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(git) = std::fs::symlink_metadata(at.join(".git")) {
            hash.update(format!("\0{}:{}", git.dev(), git.ino()).as_bytes());
        }
    }
    let mut key = crate::extension::hex(&hash.finalize());
    key.truncate(16);
    key
}

/// **This machine's file follows a project moved** from `old` (its real path then) to `new`
/// ([`crate::localproject`]): a rename keeps the `.git`, so the file it was kept under is found
/// by the old path and the folder's own `.git`. Nothing is moved over a file the new place has.
pub fn record_moved(old: &Path, new: &Path) -> io::Result<()> {
    let to = path(new);
    if to.as_os_str().is_empty() || to.exists() {
        return Ok(());
    }
    let from = to.with_file_name(format!("{}.json", key_of(old, new)));
    if from == to || !from.exists() {
        return Ok(());
    }
    std::fs::rename(from, to)
}

/// The folder [`path`] is in: what a rewrite of it is held to.
fn machine_folder(root: &Path) -> PathBuf {
    path(root)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// **This machine's file for the clone at `root`** (ADR 0093): in purlis's folder in the config
/// home, keyed by [`clone_key`]. An empty path where there is no config home: nothing is read
/// from it, and nothing can be written.
pub fn path(root: &Path) -> PathBuf {
    config_home()
        .map(|home| {
            crate::machine::dir(&home)
                .join(MACHINE_GRANTS)
                .join(format!("{}.json", clone_key(root)))
        })
        .unwrap_or_default()
}

/// What the file holds.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct OnDisk {
    /// `"taken"` or `"kept-off"` once the offer is answered; absent before.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    offer: String,
    #[serde(default)]
    chats: Tally,
    /// The project's hosts as this machine last showed them to the person (#1341), each as
    /// [`super::hosts::Host`] spells it; absent before the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hosts_seen: Option<Vec<String>>,
    /// The project's browser sites shown to the person on this machine (#2443), each as
    /// [`super::hosts::Host`] spells it: the only ones of the committed file that reach a
    /// browser here ([`super::browser_sites::in_force`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    browser_sites_seen: Vec<String>,
    /// The project's Internet access presets, and `certificate-checks` where a chat has them, as this
    /// machine last showed them to the person (#1385), each by the word the committed file
    /// names it by; absent before the first, which reads as the defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    presets_seen: Option<Vec<String>>,
    /// Your own hosts for this project that you confirmed in Settings on this machine (#1341),
    /// each as [`super::hosts::Host`] spells it: the only ones of `charter.local.toml` that
    /// grant anything ([`super::hosts::personal`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    hosts_mine: Vec<String>,
    /// The folders you let every chat of this project on this machine write (#1342), each whole
    /// as [`super::grant::write`] took it: the only place a write grant of yours is kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    writes_mine: Vec<String>,
    /// The local sockets you let every chat of this project connect to (ADR 0093), each as the
    /// kernel names it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sockets_mine: Vec<String>,
    /// The system services you let every chat of this project look up by name (ADR 0093).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    services_mine: Vec<String>,
    /// The services on this Mac you let every chat of this project connect to (qw384), each by
    /// its one port.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    ports_mine: Vec<u16>,
    /// The ports of this Mac's loopback you let every chat of this project listen on (#2019).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    listens_mine: Vec<u16>,
    /// What the project's committed file declared it expects (`[sandbox] expects`) when this
    /// machine last asked the person about it and was answered, each as the file writes it;
    /// absent before the first answer. **An answer, never a grant**: it only stops the same
    /// declaration being asked about twice ([`super::services::awaiting`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expects_seen: Option<Vec<String>>,
    /// The same answer, **kept by what was shown** (#1927): each declared item with every name
    /// it was shown under when the person answered ([`Answered`]). Written in place of
    /// `expects_seen`, which is still read where this is absent (an older build's record).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expects_answered: Option<Vec<Answered>>,
    /// Each grant made on this machine from a block's Notice that lasts past one chat (#1342):
    /// what Settings' Granted list says of who granted it, when, and from which chat.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    granted: Vec<Made>,
    /// The folders you listed in Settings › Sandbox as ones chats may be granted (D-1342-10):
    /// added to the allowlist a write grant must be inside, on this machine only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    grantable: Vec<String>,
    /// The vaults you let a persona's chats use on this machine although the vault registry
    /// does not tag them for it (#1430): the only place such a grant is kept. Never the
    /// registry, whose shared half is committed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    vaults_mine: Vec<VaultGrant>,
    /// The dispatch grants you made for every chat of this project on this machine (#1437):
    /// the only place a dispatch grant of yours is kept.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_mine: Vec<DispatchPair>,
    /// The project's dispatch grants as this machine last showed them to the person (#1437),
    /// each as [`crate::dispatchgrant::Pair`] is displayed; absent before the first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dispatch_seen: Option<Vec<String>>,
    /// **Not this file's any more**: the nevers one dev build kept here, before they moved to
    /// `app/dispatch-never.json` ([`crate::dispatchnever`]). Carried as it was written until
    /// that module moves it ([`legacy_dispatch_nevers`], [`drop_legacy_dispatch_nevers`]), so
    /// no write of this file drops a never on the way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dispatch_never: Option<serde_json::Value>,
    /// The personas whose chats you let dispatch to **any** persona, on this machine (#1503).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_any: Vec<String>,
    /// The personas whose "any persona" grant in the project's file you accepted on this
    /// machine (#1503). Kept apart from `dispatch_seen`, so nothing that acknowledges a pair
    /// puts one in force.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_any_seen: Vec<String>,
    /// The grants of yours set aside because a name in them came to be another persona's
    /// (#1504): in force for no chat until you give them back or remove each in Settings
    /// ([`crate::dispatchdormant`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_dormant: Vec<Dormant>,
    /// What this machine accepted of the project's grants for a name whose grants were set
    /// aside: taken out of `dispatch_seen` and `dispatch_any_seen` then, and put back by Give
    /// back where the file still holds it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_accepted_aside: Vec<Accepted>,
    /// The project's grants you said "Not on my machine" to (#1504), each as
    /// [`crate::dispatchgrant::Pair`] is displayed, or `<asking> -> *` for any persona. None of
    /// them is in `dispatch_seen` or `dispatch_any_seen`, which is what keeps them out of
    /// force; this list only stops them being told as new.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_declined: Vec<String>,
    /// What this machine knows of each persona a grant names (#1504): its definition as it
    /// was when a dispatch was last judged with it there, and whether it has been seen gone
    /// since ([`crate::dispatchdormant`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_known: Vec<Known>,
    /// The dispatch grants of yours that hold in one workspace only (#1505), a pair or "any
    /// persona". **A key of their own, never a field on `dispatch_mine` or a name in
    /// `dispatch_any`**: a build that does not know the condition reads those two as holding
    /// everywhere, and does not read this key at all.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_mine_in: Vec<DispatchIn>,
    /// The project's grants limited to one workspace that you accepted on this machine
    /// (#1505). Kept apart from `dispatch_seen` and `dispatch_any_seen`, so accepting a grant
    /// for one workspace never accepts the same pair for every workspace, or the reverse.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_seen_in: Vec<DispatchIn>,
    /// What this machine knows of each workspace a limited grant names (#1505): how many
    /// times the name was seen to be no workspace of the project
    /// ([`crate::dispatchwithin`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_workspaces: Vec<KnownWorkspace>,
    /// **What the acceptances and declines above are bound to** (#1506): the commit of the
    /// project's history through which this machine has checked that no commit took any of
    /// them out of the project's file ([`crate::dispatcharrival::settle`]). Absent where
    /// nothing is accepted or declined, or the project is in no git repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dispatch_seen_at: Option<String>,
    /// The project's grants you accepted here whose acceptance was dropped (#1506), each with
    /// why: a commit took it out, or the history could not be read. One a commit took out is
    /// told once as taken away. One the file holds waits for a new yes, and is said to have
    /// been accepted before, with the reason.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dispatch_gone: Vec<Gone>,
    /// Each persona whose committed hosts you allowed on this machine (#1362), with the digest
    /// of the list you were shown ([`super::persona::digest`]): the only place such an Allow is
    /// kept. A list that changed since grants nothing until it is allowed again.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    persona_hosts_mine: Vec<PersonaHostsAllowed>,
    /// **Always for this persona** on a chat's browser Ask (#2241): what every chat running as
    /// each persona in this project on this machine may have its browser reach, without being
    /// asked. The only place it is kept, where no chat writes; one per persona.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    browsers_mine: Vec<BrowserAlways>,
    /// The browser sites you let every chat's browser of this project reach on this machine
    /// (#2018, ruled 2026-10-17), each `host:port`: the browser's alone, never read by a chat's
    /// own commands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    browser_sites_mine: Vec<String>,
    /// Your own sandbox levels for this project, and for its personas, as you set or confirmed
    /// them in Settings on this machine (#2071, ADR 0096 §7): what `purlis.local.toml`'s
    /// `[sandbox] level` and `[sandbox.personas]` are honoured down to
    /// ([`super::personal::in_force`]). A chat can write that file; it cannot write this one.
    #[serde(default, skip_serializing_if = "super::personal::Personal::is_empty")]
    levels_mine: super::personal::Personal,
    /// Your answer to the project's sandbox off (#2071, SO-8; #2156): what `[sandbox] mode =
    /// "off"` in the committed file does on this machine, for the off as purlis saw it, until
    /// purlis sees it leave. Kept here, where no chat writes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    team_off: Option<TeamOffAnswer>,
    /// The upstream commit purlis's own fetch of the project found (#2156), as the remote named
    /// it: what an off is checked against before the ask names its commit and author. Kept
    /// here, where no chat writes, never read from the clone's own refs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    upstream_fetched: Option<String>,
}

/// One dispatch grant that holds in one workspace only (#1505), as this file keeps it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DispatchIn {
    pub asking: String,
    /// The target persona's name; `*` where [`DispatchIn::any`].
    pub target: String,
    /// Whether it is "any persona". Said by this key and never by `target`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub any: bool,
    /// The workspace it holds in, by the name its folder has.
    pub workspace: String,
    /// How many times this machine had seen that name gone when the grant was made or last
    /// confirmed ([`KnownWorkspace::gone`]). The grant counts only while the two are equal, so
    /// a workspace made later under the name of one that was removed inherits nothing.
    #[serde(default)]
    pub seen: u32,
}

impl DispatchIn {
    /// Whether this is the grant of `asking` to `target` (`any`: any persona) in `workspace`.
    pub fn is(&self, asking: &str, target: &str, any: bool, workspace: &str) -> bool {
        self.asking == asking
            && self.any == any
            && (any || self.target == target)
            && self.workspace == workspace
    }
}

/// What this machine knows of one workspace a limited dispatch grant names (#1505).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KnownWorkspace {
    pub name: String,
    /// How many times the name was seen to be no workspace of the project, having been one or
    /// not been looked at before.
    #[serde(default)]
    pub gone: u32,
    /// Whether it was gone when last looked at.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub away: bool,
}

/// One grant of yours set aside because a name in it came to be another persona's (#1504):
/// `was` is that name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Dormant {
    pub asking: String,
    /// The target persona's name; `*` where [`Dormant::any`].
    pub target: String,
    /// Whether it was "any persona". **Said by this key and never by `target`**: a `*` someone
    /// wrote as the target of a named grant granted nothing, and set aside it is still a pair
    /// nobody can be given.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub any: bool,
    /// Empty where a hand-edit dropped it: the entry is then nobody's to be given back.
    #[serde(default)]
    pub was: String,
}

/// One of the project's grants whose acceptance on this machine was dropped (#1506).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Gone {
    /// The grant, as `dispatch_declined` spells one.
    pub said: String,
    /// [`Gone::REMOVED`] or [`Gone::UNREAD`]. Any other word is read as the second: nothing
    /// is said to have been taken away that purlis did not see taken away.
    #[serde(default)]
    pub why: String,
}

impl Gone {
    /// A commit of the project's history took the grant out of its file.
    pub const REMOVED: &'static str = "removed";
    /// The project's history since the acceptance could not be read.
    pub const UNREAD: &'static str = "unread";
}

/// One of the project's grants this machine had accepted, set aside with the grants of the
/// name `was` (#1504).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Accepted {
    /// The grant, as `dispatch_declined` spells one: a pair as it is displayed, or
    /// `<asking> -> *`.
    pub said: String,
    #[serde(default)]
    pub was: String,
}

/// What this machine knows of one persona a dispatch grant names (#1504).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Known {
    pub name: String,
    /// The SHA-256 of its definition, in hex, as it was when a dispatch was last judged with
    /// it there. Empty where none was, or where purlis itself removed the persona.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub hash: String,
    /// Whether the name has been seen to be no persona of the project since: **the absence
    /// mark**. While it stands and the name is a persona again, its grants are not in force
    /// until the same definition is back or the person says so.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub away: bool,
}

/// One persona whose hosts you allowed on this machine (#1362): its name, the digest of what
/// the Notice showed you, and whether that has been seen to change since — after which it
/// grants nothing, even should the list come back (D-1362-13).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PersonaHostsAllowed {
    pub persona: String,
    pub digest: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub changed: bool,
}

/// One persona whose chats' browser you allowed always (#2241), and what it reaches: this
/// project's sites and its dev servers, and with `open_web` the open web too.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BrowserAlways {
    pub persona: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub open_web: bool,
}

/// What a [`Made`] record and the audit call your Always for a persona's browser (#2241).
pub const BROWSER: &str = "browser";

/// What a [`Made`] record and the audit call your Allow of a persona's hosts (#1362).
pub const PERSONA_HOSTS: &str = "persona-hosts";

/// One dispatch grant of yours, as the file keeps it: chats running as `asking` may dispatch
/// to `target`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct DispatchPair {
    asking: String,
    target: String,
}

/// One vault you let one persona's chats use on this machine (#1430), beside whatever the vault
/// registry tags it for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VaultGrant {
    pub vault: String,
    pub persona: String,
}

impl VaultGrant {
    /// The grant as the Granted list and the audit name it: `<vault> for <persona>`.
    pub fn target(&self) -> String {
        format!("{} for {}", self.vault, self.persona)
    }
}

/// What a [`Made`] record and the audit call a vault grant.
pub const VAULT: &str = "vault";

/// One grant made on this machine that lasts past its chat (#1342), as `app/sandbox.json` keeps
/// it: what Settings' Granted list says of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Made {
    /// `host`, `write`, `vault` or `persona-hosts`.
    pub what: String,
    /// The host or the folder, as the sandbox writes it; a vault and the persona it was allowed
    /// for, as [`VaultGrant::target`] says them; the persona whose hosts were allowed.
    pub target: String,
    /// `you` or `project`.
    pub level: String,
    /// When, in seconds since 1970.
    pub at: u64,
    /// The chat it was granted from, by the name its tab showed, where it came from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
}

fn read(root: &Path) -> OnDisk {
    let file = path(root);
    if file.as_os_str().is_empty() {
        return OnDisk::default();
    }
    std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Reads this machine's file, changes it and writes it back, under purlis's lock on its folder.
fn change(root: &Path, how: impl FnOnce(&mut OnDisk)) -> io::Result<()> {
    let file = path(root);
    if file.as_os_str().is_empty() {
        return Err(io::Error::other(
            "this machine has no config home to keep it in",
        ));
    }
    write_new(&file, how)
}

/// [`change`] on `file`, which this machine keeps: its folder made `0700`.
fn write_new(file: &Path, how: impl FnOnce(&mut OnDisk)) -> io::Result<()> {
    let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(&dir)?;
    crate::rewrite::update(&dir, file, |now| {
        // A file that is not this shape reads as empty, as [`read`] reads it: the count
        // starts again rather than refusing every chat start that would add to it.
        let mut held: OnDisk = now
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_else(|| OnDisk {
                // What starts again never takes a never an earlier build left here with it.
                dispatch_never: now.and_then(legacy_nevers_in),
                ..OnDisk::default()
            });
        how(&mut held);
        serde_json::to_string_pretty(&held)
            .map(|text| Some(format!("{text}\n")))
            .map_err(io::Error::other)
    })
    .map(|_| ())
}

// ---- the one-time offer ----------------------------------------------------------------------

/// The operator's answer to the offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Turn the sandbox on for this project: `[sandbox] mode = "on"` in `charter.toml`.
    TurnOn,
    /// Keep running chats as before. Nothing in the project changes.
    KeepItOff,
}

/// Whether the project at `root` is offered the sandbox: it is a project, no sandbox is in
/// force in it (it has not turned the sandbox on, and no policy requires it here, D-1423-1:
/// an offer to turn on what is on already would ask nothing), and nobody on this machine has
/// answered the offer. A project charter made
/// turned it on already, so only one made before the sandbox existed is ever offered it.
///
/// A `charter.toml` charter cannot read is never offered anything: what it says about the
/// sandbox is unknown, and every chat in it is refused until it is fixed.
pub fn offer_due(root: &Path) -> bool {
    let plane = Plane::read(root);
    crate::names::has_manifest(root)
        && !plane.unreadable()
        && plane.in_force(&super::policy::Locks::of(root)).is_none()
        && read(root).offer.is_empty()
}

/// Answers the offer for the project at `root`, once: turning the sandbox on writes it into
/// `charter.toml` first, so an answer is never recorded for a file that did not change.
pub fn answer(root: &Path, answer: Answer) -> io::Result<()> {
    if answer == Answer::TurnOn {
        // A `charter.toml` the sandbox cannot read — a link, a device, over the cap, not TOML —
        // already refuses every chat. It is never edited into a file that reads as on.
        if Plane::read(root).unreadable() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "charter.toml is not a file purlis reads (a link, a special file, too large, or \
                 not TOML), so nothing was changed. Fix it, and every chat in this project starts \
                 again.",
            ));
        }
        turn_on(root)?;
    }
    change(root, |held| {
        held.offer = match answer {
            Answer::TurnOn => "taken",
            Answer::KeepItOff => "kept-off",
        }
        .to_owned();
    })
}

/// Writes `mode = "on"` into the project's `charter.toml`, keeping every line already there:
/// the whole block with the default egress where the file has no `sandbox` at all, the mode
/// alone where it has a `[sandbox]` table already.
///
/// **What was written is read back before it is kept.** The mode is set by a text edit, and a
/// `[sandbox]` or `mode =` line can sit inside a multi-line string, where the edit would land
/// in the string and leave the table as it was. So the result is parsed again, and kept only
/// if the sandbox now reads as on and nothing more is refused than before. Any other form is
/// refused, and nothing is written: an answer is never recorded for a sandbox still off.
///
/// **Only a regular `charter.toml` at the project root**, never a link's target: the text is
/// read with the sandbox's own reader ([`super::read_plane_file`]), which refuses anything else,
/// and must be the text the write is made against.
fn turn_on(root: &Path) -> io::Result<()> {
    let target = crate::names::manifest(root);
    let dir = root.to_path_buf();
    let unedited = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "charter.toml's {TABLE} is written in a form purlis does not edit, so nothing \
                 was changed. Add mode = \"on\" to its [{TABLE}] table by hand to turn the \
                 sandbox on."
            ),
        )
    };
    crate::rewrite::update(&dir, &target, |now| {
        let Some(now) = now else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not there", target.display()),
            ));
        };
        // Under the write's lock: still a regular file holding exactly this text.
        if super::read_plane_file(&target).ok().flatten().as_deref() != Some(now) {
            return Err(unedited());
        }
        let Ok(top) = now.parse::<toml::Table>() else {
            return Err(unedited());
        };
        let header = format!("[{TABLE}]");
        let next = match top.get(TABLE) {
            None => {
                let gap = if now.is_empty() || now.ends_with("\n\n") {
                    ""
                } else if now.ends_with('\n') {
                    "\n"
                } else {
                    "\n\n"
                };
                format!("{now}{gap}{}", super::on_block())
            }
            Some(toml::Value::Table(_)) if now.lines().any(|line| line.trim() == header) => {
                crate::scaffold::planefile::edited(now, TABLE, "mode", "on")
            }
            Some(_) => return Err(unedited()),
        };
        // Never a file that would refuse every chat: what is kept must read, and read as on.
        let before = Plane::of(Some(now)).said();
        let written = Plane::of(Some(&next));
        let after = written.said();
        // A `[[forge]]` host the sandbox does not let through is said once it is on (#1405):
        // a consequence of turning it on, not a sign the edit missed.
        let edit_refused = |said: &super::Said| -> Vec<super::Refusal> {
            said.refused
                .iter()
                .filter(|refusal| !matches!(refusal, super::Refusal::ForgeHost(..)))
                .cloned()
                .collect()
        };
        if written.unreadable()
            || after.policy.is_none()
            || edit_refused(&after) != edit_refused(&before)
        {
            return Err(unedited());
        }
        Ok(Some(next))
    })
    .map(|_| ())
}

// ---- the project's hosts, told once per change (#1341) --------------------------------------

/// The project's own hosts as they changed since this machine last told the person: what was
/// added, what was taken away, and the whole list now — what acknowledging it records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostsChange {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub now: Vec<String>,
}

/// **How the project's hosts changed since this machine last told the person** — its own
/// `[sandbox] hosts` and the `[[forge]]` hosts the `forge` preset lets every chat reach (ADR 0067 §1 as
/// amended: each teammate sees a one-time Notice naming what changed). `None` when nothing did,
/// and in a project whose chats are not sandboxed, where its hosts reach nothing. A project
/// first seen on this machine with hosts is a change: nothing widens unseen. Order is no change.
///
/// A persona's own hosts (#1362) are not told here: they reach nothing on this machine until
/// the person allows them, which their own Notice asks ([`super::persona::shown`]).
///
/// **Held to an administrator's policy** (#1423): a host policy locks out reaches no chat, so
/// the Notice never names it as one that does. A policy that comes to lock a host out, or lets
/// one back in, is a change to what chats reach, and is told as one.
pub fn hosts_changed(root: &Path) -> Option<HostsChange> {
    let plane = Plane::read(root);
    // Only the hosts a chat reaches: none an administrator's policy locks out (#1423).
    let locks = super::policy::Locks::of(root);
    plane.in_force(&locks)?;
    let now = plane.granted_hosts(&locks);
    // A host never holds a space: an entry that does is an older purlis's `<host> for <persona>
    // chats`, from before a persona's hosts were asked for apart (D-1362-7), and is no host of
    // the project's to say was taken away.
    let seen: Vec<String> = read(root)
        .hosts_seen
        .unwrap_or_default()
        .into_iter()
        .filter(|one| !one.contains(' '))
        .collect();
    let added: Vec<String> = now
        .iter()
        .filter(|one| !seen.contains(one))
        .cloned()
        .collect();
    let removed: Vec<String> = seen
        .iter()
        .filter(|one| !now.contains(one))
        .cloned()
        .collect();
    (!added.is_empty() || !removed.is_empty()).then_some(HostsChange {
        added,
        removed,
        now,
    })
}

/// Records that the person was told of `shown`, the project's hosts as the Notice showed them:
/// a change after it was shown is told again ([`hosts_changed`]).
pub fn acknowledge_hosts(root: &Path, shown: &[String]) -> io::Result<()> {
    change(root, |held| held.hosts_seen = Some(shown.to_vec()))
}

// ---- the project's Internet access presets, told once per change (#1385) ---------------------

/// The project's Internet access presets as they changed since this machine last told the
/// person: each named as the window names it, what each one turned on widens past its hosts,
/// and the whole set now — what acknowledging it records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetsChange {
    /// Turned on: each preset's title ([`super::Preset::title`]), and `Certificate checks`.
    pub added: Vec<String>,
    /// Turned off, named the same way.
    pub removed: Vec<String>,
    /// One sentence for each of `added` that widens what a chat may do past the hosts it
    /// reaches (spec #1330's coupled widenings, #1337): the package caches, the certificate
    /// check.
    pub widens: Vec<String>,
    /// The whole set now, each by the word the committed file names it by.
    pub now: Vec<String>,
}

/// What the window calls the certificate check where it lists it beside the presets.
const CERTIFICATE_CHECKS_TITLE: &str = "Certificate checks";

/// **How the project's Internet access presets changed since this machine last told the
/// person** — its `[sandbox] egress` presets and its `certificate-checks` (ADR 0067 §1 as
/// amended, spec #1330: each teammate sees a one-time Notice naming what changed, so nothing
/// widens unseen). `None` when nothing did, and where chats here are not sandboxed.
///
/// Told as [`hosts_changed`] tells the hosts, with one difference: a machine that has seen
/// nothing yet has seen **the defaults** a project starts with ([`super::Preset::DEFAULT`],
/// less any an administrator's policy turns off, certificate checks on unless policy locks
/// them: R1, 2026-10-13), so a project on the defaults tells nothing on first sight, and one
/// that differs is told once — a narrowed one too, so that turning a preset back on later is
/// a change from what was seen.
pub fn presets_changed(root: &Path) -> Option<PresetsChange> {
    let locks = super::policy::Locks::of(root);
    presets_change(
        &Plane::read(root),
        &locks,
        read(root).presets_seen.as_deref(),
    )
}

/// [`presets_changed`] of `plane` under `locks`, where this machine last showed `seen`.
///
/// **Only what a chat reaches** (#1423): a preset an administrator's policy turns off reaches
/// no chat, so it is not named, as the hosts Notice names no host policy locks out.
pub fn presets_change(
    plane: &Plane,
    locks: &super::policy::Locks,
    seen: Option<&[String]>,
) -> Option<PresetsChange> {
    use super::Preset;
    let policy = plane.in_force(locks)?;
    let mut now: Vec<String> = locks
        .presets(&policy.egress)
        .into_iter()
        .map(|preset| preset.word().to_owned())
        .collect();
    // Certificate checks only where a chat has them: not under policy's lock (R1).
    if super::CertificateChecks::of(&policy, locks).is_on() {
        now.push(super::CERTIFICATE_CHECKS.to_owned());
    }
    // The defaults as they reach a chat here: a preset policy turns off was never on to see,
    // and certificate checks are on by default since R1 (2026-10-13), policy allowing.
    let mut defaults: Vec<String> = locks
        .presets(&Preset::DEFAULT)
        .into_iter()
        .map(|preset| preset.word().to_owned())
        .collect();
    if !locks.forbids_socket_grants() {
        defaults.push(super::CERTIFICATE_CHECKS.to_owned());
    }
    let seen = seen.unwrap_or(&defaults);
    let title = |word: &str| {
        Preset::ALL
            .into_iter()
            .find(|preset| preset.word() == word)
            .map_or(CERTIFICATE_CHECKS_TITLE, Preset::title)
            .to_owned()
    };
    let added: Vec<&String> = now.iter().filter(|one| !seen.contains(one)).collect();
    let removed: Vec<String> = seen
        .iter()
        .filter(|one| !now.contains(one))
        // A word no preset is any more was shown once; it is named as it was written.
        .map(|one| {
            if one == super::CERTIFICATE_CHECKS || Preset::ALL.iter().any(|p| p.word() == one) {
                title(one)
            } else {
                one.clone()
            }
        })
        .collect();
    if added.is_empty() && removed.is_empty() {
        return None;
    }
    let widens = added
        .iter()
        .filter_map(|word| widening(word))
        .map(str::to_owned)
        .collect();
    Some(PresetsChange {
        added: added.iter().map(|word| title(word)).collect(),
        removed,
        widens,
        now,
    })
}

/// What turning on the preset or setting called `word` widens past the hosts it lets a chat
/// reach, in one sentence; `None` for one that widens nothing else.
fn widening(word: &str) -> Option<&'static str> {
    if word == super::Preset::Toolchains.word() {
        Some("Package registries also lets chats write the project's own package caches.")
    } else if word == super::CERTIFICATE_CHECKS {
        Some(
            "Certificate checks let programs such as gh verify a host's certificate through \
             the system on macOS, which then fetches addresses a certificate names, outside \
             the hosts chats may reach.",
        )
    } else {
        None
    }
}

/// Records that the person was told of `shown`, the project's presets as the Notice showed
/// them: a change after it was shown is told again ([`presets_changed`]).
pub fn acknowledge_presets(root: &Path, shown: &[String]) -> io::Result<()> {
    change(root, |held| held.presets_seen = Some(shown.to_vec()))
}

/// A change to the presets made in this machine's own Settings is one this machine has seen —
/// unless another was still waiting to be told (`pending`, [`presets_changed`] read before the
/// write), which the Notice then tells with it. Best effort: a record that cannot be written
/// leaves the Notice to say it once more.
///
/// **What is recorded is what this window wrote** (`written`, the whole shared file as it was
/// written), never a read of the disk after it (#1550): a pull that lands between the write and
/// the record is a teammate's change, and is told. Where the file on disk no longer says what
/// was written, nothing is recorded, and the Notice tells what is there.
pub fn presets_seen_by_you(root: &Path, pending: bool, written: &str) {
    if pending {
        return;
    }
    let locks = super::policy::Locks::of(root);
    let seen = read(root).presets_seen;
    let wrote = presets_change(&Plane::of(Some(written)), &locks, seen.as_deref());
    let disk = presets_change(&Plane::read(root), &locks, seen.as_deref());
    if let (Some(wrote), Some(disk)) = (wrote, disk)
        && wrote.now == disk.now
    {
        let _ = acknowledge_presets(root, &wrote.now);
    }
}

/// The project's hosts made in this machine's own Settings are ones this machine has seen, as
/// [`presets_seen_by_you`] records the presets: unless another change was waiting (`pending`,
/// [`hosts_changed`] read before the write), and only as `written` says them, where the file
/// on disk still says the same.
pub fn hosts_seen_by_you(root: &Path, pending: bool, written: &str) {
    if pending {
        return;
    }
    let locks = super::policy::Locks::of(root);
    let hosts_of = |plane: Plane| plane.in_force(&locks).map(|_| plane.granted_hosts(&locks));
    let wrote = hosts_of(Plane::of(Some(written)));
    if wrote.is_some()
        && wrote == hosts_of(Plane::read(root))
        && let Some(now) = wrote
        && hosts_changed(root).is_some()
    {
        let _ = acknowledge_hosts(root, &now);
    }
}

// ---- your own hosts, as you confirmed them (#1341) -------------------------------------------

/// Your own hosts for the project at `root` that you confirmed in Settings on this machine.
pub fn confirmed_hosts(root: &Path) -> Vec<String> {
    read(root).hosts_mine
}

/// Records `host` as one you confirmed: Settings' Add and Confirm, never a chat (a sandboxed
/// chat cannot write this file).
pub fn confirm_host(root: &Path, host: &str) -> io::Result<()> {
    change(root, |held| {
        if !held.hosts_mine.iter().any(|one| one == host) {
            held.hosts_mine.push(host.to_owned());
        }
    })
}

/// Takes `host` off the record: Settings' Remove.
pub fn unconfirm_host(root: &Path, host: &str) -> io::Result<()> {
    change(root, |held| held.hosts_mine.retain(|one| one != host))
}

/// **Takes off the record each host of yours that `local`, this machine's settings file as
/// Settings read or wrote it (`None`: there is none), no longer lists** (#1405), so the record
/// cannot grow without end, and a host taken out of the file by hand and written there again
/// later grants nothing until it is confirmed again.
///
/// Never a live grant: a host grants only while the file lists it and the record holds it
/// ([`super::hosts::personal`]), and only hosts the file does not list go. A file that is not
/// TOML lists nothing purlis can tell, grants nothing while it is so, and takes nothing off.
/// The record is written only when a host goes. Answers how many went.
pub fn forget_hosts_not_in(root: &Path, local: Option<&str>) -> io::Result<usize> {
    let listed: Vec<String> = match local.map(str::parse::<toml::Table>) {
        None => Vec::new(),
        Some(Ok(table)) => super::hosts::of_table(Some(&table))
            .iter()
            .map(ToString::to_string)
            .collect(),
        Some(Err(_)) => return Ok(0),
    };
    let goes = |held: &OnDisk| {
        held.hosts_mine
            .iter()
            .filter(|one| !listed.contains(one))
            .count()
    };
    if goes(&read(root)) == 0 {
        return Ok(0);
    }
    let mut gone = 0;
    change(root, |held| {
        gone = goes(held);
        held.hosts_mine.retain(|one| listed.contains(one));
    })?;
    Ok(gone)
}

// ---- your own levels, as you set or confirmed them (#2071) ------------------------------------

/// **Your answer to the project's sandbox off** (#2071, SO-8; #2156): whether you accepted
/// it or kept the sandbox on. It is for the off as purlis saw it, never for a commit.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TeamOffAnswer {
    pub accepted: bool,
}

/// Your answer to the project's sandbox off at `root`, where you gave one.
pub fn team_off_answer(root: &Path) -> Option<TeamOffAnswer> {
    read(root).team_off
}

/// Keeps your answer to the project's sandbox off: the window's Accept, Keep it on for me, or a
/// change of mind in Settings, never a chat (a sandboxed chat cannot write this file).
pub fn answer_team_off(root: &Path, answer: TeamOffAnswer) -> io::Result<()> {
    change(root, |held| held.team_off = Some(answer))
}

/// Lets your answer to the project's sandbox off go: purlis read the project's file not turning
/// the sandbox off, so an off that comes back is asked again.
pub fn forget_team_off(root: &Path) -> io::Result<()> {
    change(root, |held| held.team_off = None)
}

/// The upstream commit purlis's own fetch of the project at `root` found, where one was kept.
pub fn fetched_upstream(root: &Path) -> Option<String> {
    read(root).upstream_fetched
}

/// Keeps `commit` as the upstream commit purlis's own fetch of the project found: the fetch's,
/// never a chat's (a sandboxed chat cannot write this file).
pub fn keep_fetched_upstream(root: &Path, commit: &str) -> io::Result<()> {
    if fetched_upstream(root).as_deref() == Some(commit) {
        return Ok(());
    }
    change(root, |held| held.upstream_fetched = Some(commit.to_owned()))
}

/// Your own levels for the project at `root` that you set or confirmed in Settings on this
/// machine.
pub fn confirmed_levels(root: &Path) -> super::personal::Personal {
    read(root).levels_mine
}

/// Changes the record of your levels: Settings' save and Confirm, never a chat (a sandboxed
/// chat cannot write this file).
pub fn change_levels(
    root: &Path,
    how: impl FnOnce(&mut super::personal::Personal),
) -> io::Result<()> {
    change(root, |held| how(&mut held.levels_mine))
}

// ---- the folders you let every chat write (#1342) --------------------------------------------

/// The folders you let every chat of the project at `root` write on this machine, as granted.
/// [`super::Compiled::granted`] judges each again before a start compiles it in.
pub fn granted_writes(root: &Path) -> Vec<std::path::PathBuf> {
    read(root)
        .writes_mine
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect()
}

/// Lets every chat of the project at `root` write `folder` on this machine: a block's Notice,
/// never a chat (a sandboxed chat cannot write this file). `folder` is one
/// [`super::grant::write`] took.
pub fn grant_write(root: &Path, folder: &Path) -> io::Result<()> {
    let folder = folder.display().to_string();
    change(root, |held| {
        if !held.writes_mine.contains(&folder) {
            held.writes_mine.push(folder);
        }
    })
}

/// Takes `folder` off what you let every chat write: Settings' Revoke.
pub fn revoke_write(root: &Path, folder: &Path) -> io::Result<()> {
    let folder = folder.display().to_string();
    change(root, |held| {
        held.writes_mine.retain(|one| *one != folder);
        held.granted
            .retain(|made| !(made.what == "write" && made.target == folder));
    })
}

// ---- the sockets and services you let every chat reach (D-sock-2) ---------------------------

/// The local sockets you let every chat of the project at `root` connect to on this machine.
/// [`super::Compiled::granted`] judges each again before a start compiles it in.
pub fn granted_sockets(root: &Path) -> Vec<std::path::PathBuf> {
    read(root)
        .sockets_mine
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect()
}

/// Lets every chat of the project at `root` connect to `socket` on this machine: a block's
/// Notice, never a chat (a sandboxed chat cannot write the config home). `socket` is one
/// [`super::services::socket`] took.
pub fn grant_socket(root: &Path, socket: &Path) -> io::Result<()> {
    let socket = socket.display().to_string();
    change(root, |held| {
        if !held.sockets_mine.contains(&socket) {
            held.sockets_mine.push(socket);
        }
    })
}

/// Takes `socket` off what you let every chat connect to: Settings' Remove.
pub fn revoke_socket(root: &Path, socket: &Path) -> io::Result<()> {
    let socket = socket.display().to_string();
    change(root, |held| {
        held.sockets_mine.retain(|one| *one != socket);
        held.granted
            .retain(|made| !(made.what == "socket" && made.target == socket));
    })
}

/// The system services you let every chat of the project at `root` look up on this machine.
pub fn granted_services(root: &Path) -> Vec<String> {
    read(root).services_mine
}

/// Lets every chat of the project at `root` look up `service` on this machine. `service` is
/// one [`super::services::service`] took.
pub fn grant_service(root: &Path, service: &str) -> io::Result<()> {
    change(root, |held| {
        if !held.services_mine.iter().any(|one| one == service) {
            held.services_mine.push(service.to_owned());
        }
    })
}

/// Takes `service` off what you let every chat look up: Settings' Remove.
pub fn revoke_service(root: &Path, service: &str) -> io::Result<()> {
    change(root, |held| {
        held.services_mine.retain(|one| one != service);
        held.granted
            .retain(|made| !(made.what == "service" && made.target == service));
    })
}

/// The services on this Mac you let every chat of the project at `root` connect to, each by its
/// port (qw384). [`super::Compiled::granted`] judges each again before a start compiles it in.
pub fn granted_ports(root: &Path) -> Vec<u16> {
    read(root).ports_mine
}

/// Lets every chat of the project at `root` connect to the service on this Mac at `port`: a
/// block's Notice, never a chat. `port` is one [`super::localport::port`] took.
pub fn grant_port(root: &Path, port: u16) -> io::Result<()> {
    change(root, |held| {
        if !held.ports_mine.contains(&port) {
            held.ports_mine.push(port);
        }
    })
}

/// Takes `port` off what you let every chat connect to: Settings' Remove.
pub fn revoke_port(root: &Path, port: u16) -> io::Result<()> {
    let target = port.to_string();
    change(root, |held| {
        held.ports_mine.retain(|one| *one != port);
        held.granted
            .retain(|made| !(made.what == "port" && made.target == target));
    })
}

/// The ports of this Mac's loopback you let every chat of the project at `root` listen on
/// (#2019). [`super::Compiled::granted`] judges each again before a start compiles it in.
pub fn granted_listens(root: &Path) -> Vec<u16> {
    read(root).listens_mine
}

/// Lets every chat of the project at `root` listen on `port` of this Mac's loopback: a block's
/// Notice, never a chat. `port` is one [`super::localport::port`] took.
pub fn grant_listen(root: &Path, port: u16) -> io::Result<()> {
    change(root, |held| {
        if !held.listens_mine.contains(&port) {
            held.listens_mine.push(port);
        }
    })
}

/// Takes `port` off what you let every chat listen on: Settings' Remove.
pub fn revoke_listen(root: &Path, port: u16) -> io::Result<()> {
    let target = port.to_string();
    change(root, |held| {
        held.listens_mine.retain(|one| *one != port);
        held.granted
            .retain(|made| !(made.what == "listen" && made.target == target));
    })
}

// ---- what the project expects, as this machine was asked about it ---------------------------

/// **One declared item the person here answered about** (#1927): the item as the committed
/// file writes it, and each name it was shown under when they answered (the socket as the
/// kernel named it then, links resolved; a service's own name). A declared path can come to
/// lead to another socket, and an answer is to what was shown: under a name never answered it
/// is asked about again. **An answer, never a grant.**
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Answered {
    pub written: String,
    /// Oldest first, at most [`MOST_NAMES_ANSWERED`].
    #[serde(default)]
    pub named: Vec<String>,
}

/// The most names kept for one declared path: enough that a path moving between a few
/// sockets asks once for each, and few enough that what a repository's link can make this
/// machine keep stays small. Past it the oldest goes, and is asked about again.
pub const MOST_NAMES_ANSWERED: usize = 8;

/// What the person here answered about what the project at `root` expects: empty before the
/// first answer. An older build's record, of the declared items alone, reads as each answered
/// under the name it is written by.
pub fn expects_answered(root: &Path) -> Vec<Answered> {
    let held = read(root);
    held.expects_answered.unwrap_or_else(|| {
        held.expects_seen
            .unwrap_or_default()
            .into_iter()
            .map(|written| Answered {
                named: vec![written.clone()],
                written,
            })
            .collect()
    })
}

/// The declared items the person here answered about, each as the file writes it.
pub fn expects_seen(root: &Path) -> Vec<String> {
    expects_answered(root)
        .into_iter()
        .map(|one| one.written)
        .collect()
}

/// Records that the person answered the question about `shown`: each item as the committed
/// file writes it, with the name it was shown under. An item not among them is forgotten, so
/// one declared later is asked about; an item answered before keeps the names it was answered
/// under ([`MOST_NAMES_ANSWERED`]), so a path that goes back to a socket already answered is
/// not asked about twice. Nothing is granted.
pub fn acknowledge_expects(root: &Path, shown: &[(String, String)]) -> io::Result<()> {
    change(root, |held| {
        let before = held.expects_answered.take().unwrap_or_else(|| {
            held.expects_seen
                .take()
                .unwrap_or_default()
                .into_iter()
                .map(|written| Answered {
                    named: vec![written.clone()],
                    written,
                })
                .collect()
        });
        held.expects_seen = None;
        let mut now: Vec<Answered> = Vec::new();
        for (written, named) in shown {
            let at = now
                .iter()
                .position(|one| one.written == *written)
                .unwrap_or_else(|| {
                    now.push(Answered {
                        written: written.clone(),
                        named: before
                            .iter()
                            .find(|one| one.written == *written)
                            .map(|one| one.named.clone())
                            .unwrap_or_default(),
                    });
                    now.len() - 1
                });
            let names = &mut now[at].named;
            names.retain(|one| one != named);
            names.push(named.clone());
            if names.len() > MOST_NAMES_ANSWERED {
                names.drain(..names.len() - MOST_NAMES_ANSWERED);
            }
        }
        held.expects_answered = Some(now);
    })
}

// ---- the sockets and services you let every project's chats reach (R2) ----------------------

/// The name of this machine's one file for every project, beside the files kept per clone
/// ([`MACHINE_GRANTS`]). A clone's key is sixteen hex characters, so no clone's file is it.
pub const EVERY_PROJECT: &str = "every-project.json";

/// **This machine's file of what you allowed for every project** (the operator's ruling R2 of
/// 2026-10-13, ADR 0093 as amended): the local sockets and system services every sandboxed
/// chat here may reach, whichever project it is in, one opened later too. In purlis's folder
/// in the config home, which every chat is denied writing and no repository brings. An empty
/// path where there is no config home.
pub fn everywhere_path() -> PathBuf {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(file) = TEST_EVERYWHERE.with(|file| file.borrow().clone()) {
        return file;
    }
    config_home()
        .map(|home| {
            crate::machine::dir(&home)
                .join(MACHINE_GRANTS)
                .join(EVERY_PROJECT)
        })
        .unwrap_or_default()
}

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    /// A test's seam: the file [`everywhere_path`] answers on this thread, so a test that
    /// allows something for every project writes a file of its own, and no other test's
    /// project takes it. Only a test build has it.
    static TEST_EVERYWHERE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// **A test's file for what is allowed for every project**, on this thread, in place of this
/// machine's; `None` puts the machine's back. Only in a test build.
#[cfg(any(test, feature = "test-support"))]
pub fn everywhere_for_this_test(file: Option<PathBuf>) {
    TEST_EVERYWHERE.with(|held| *held.borrow_mut() = file);
}

/// What [`everywhere_path`] holds.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Everywhere {
    /// The local sockets, each as the kernel names it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sockets: Vec<String>,
    /// The system services, each by its whole name.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    services: Vec<String>,
    /// When each was allowed and from which chat, as Settings' Allowed list says it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    granted: Vec<Made>,
}

fn read_everywhere() -> Everywhere {
    let file = everywhere_path();
    if file.as_os_str().is_empty() {
        return Everywhere::default();
    }
    std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn change_everywhere(how: impl FnOnce(&mut Everywhere)) -> io::Result<()> {
    let file = everywhere_path();
    if file.as_os_str().is_empty() {
        return Err(io::Error::other(
            "this machine has no config home to keep it in",
        ));
    }
    let dir = file.parent().map(Path::to_path_buf).unwrap_or_default();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(&dir)?;
    crate::rewrite::update(&dir, &file, |now| {
        // A file that is not this shape reads as empty, as [`read_everywhere`] reads it: it
        // grants nothing, and the next Allow starts it again.
        let mut held: Everywhere = now
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default();
        how(&mut held);
        serde_json::to_string_pretty(&held)
            .map(|text| Some(format!("{text}\n")))
            .map_err(io::Error::other)
    })
    .map(|_| ())
}

/// The local sockets you let every project's chats connect to on this machine. A start judges
/// each again, for the project it starts in ([`super::Compiled::granted`]).
pub fn sockets_everywhere() -> Vec<PathBuf> {
    read_everywhere()
        .sockets
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

/// The system services you let every project's chats look up on this machine.
pub fn services_everywhere() -> Vec<String> {
    read_everywhere().services
}

/// Lets every project's chats connect to `socket` on this machine, recorded as `made`.
/// `socket` is one [`super::services::socket`] took.
pub fn grant_socket_everywhere(socket: &Path, made: Option<Made>) -> io::Result<()> {
    let socket = socket.display().to_string();
    change_everywhere(|held| {
        if !held.sockets.contains(&socket) {
            held.sockets.push(socket.clone());
        }
        record_everywhere(held, made);
    })
}

/// Lets every project's chats look up `service` on this machine, recorded as `made`.
/// `service` is one [`super::services::service`] took.
pub fn grant_service_everywhere(service: &str, made: Option<Made>) -> io::Result<()> {
    change_everywhere(|held| {
        if !held.services.iter().any(|one| one == service) {
            held.services.push(service.to_owned());
        }
        record_everywhere(held, made);
    })
}

fn record_everywhere(held: &mut Everywhere, made: Option<Made>) {
    if let Some(made) = made {
        held.granted
            .retain(|one| !(one.what == made.what && one.target == made.target));
        held.granted.push(made);
    }
}

/// Takes `socket` off what you let every project's chats connect to: Settings' Remove.
pub fn revoke_socket_everywhere(socket: &Path) -> io::Result<()> {
    let socket = socket.display().to_string();
    change_everywhere(|held| {
        held.sockets.retain(|one| *one != socket);
        held.granted
            .retain(|made| !(made.what == "socket" && made.target == socket));
    })
}

/// Takes `service` off what you let every project's chats look up: Settings' Remove.
pub fn revoke_service_everywhere(service: &str) -> io::Result<()> {
    change_everywhere(|held| {
        held.services.retain(|one| one != service);
        held.granted
            .retain(|made| !(made.what == "service" && made.target == service));
    })
}

/// When each grant for every project was made, as recorded.
pub fn made_everywhere() -> Vec<Made> {
    read_everywhere().granted
}

/// The folders you listed as ones chats in the project at `root` may be granted (D-1342-10).
pub fn grantable_folders(root: &Path) -> Vec<std::path::PathBuf> {
    read(root)
        .grantable
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect()
}

/// Lists `folder` as one chats may be granted: Settings, never a chat. `folder` is one
/// [`super::grant::grantable`] took.
pub fn list_grantable(root: &Path, folder: &Path) -> io::Result<()> {
    let folder = folder.display().to_string();
    change(root, |held| {
        if !held.grantable.contains(&folder) {
            held.grantable.push(folder);
        }
    })
}

/// Takes `folder` off that list. A grant already made inside it is judged again at the next
/// start, and no longer reaches a chat.
pub fn unlist_grantable(root: &Path, folder: &Path) -> io::Result<()> {
    let folder = folder.display().to_string();
    change(root, |held| held.grantable.retain(|one| *one != folder))
}

/// Records that `made` was granted here, replacing an earlier record of the same grant.
pub fn record_made(root: &Path, made: Made) -> io::Result<()> {
    change(root, |held| {
        held.granted.retain(|one| {
            !(one.what == made.what && one.target == made.target && one.level == made.level)
        });
        held.granted.push(made);
    })
}

/// Takes the record of a grant off, once it is revoked.
pub fn forget_made(root: &Path, what: &str, target: &str, level: &str) -> io::Result<()> {
    change(root, |held| {
        held.granted
            .retain(|one| !(one.what == what && one.target == target && one.level == level));
    })
}

/// Every grant made on this machine that lasts past its chat, as recorded.
pub fn made(root: &Path) -> Vec<Made> {
    read(root).granted
}

// ---- the folders this file names, when the project moves (#1698) ----------------------------

/// **Every folder this file names by its whole path**, as written: your write grants, the
/// folders you listed as ones chats may be granted, and the Granted list's records of write
/// grants. Read through no link, as [`folders_moved`] writes; `Err` with why where the file is
/// reached through one. A file that is not this shape names nothing, as [`read`] reads it.
pub fn folders_named(root: &Path) -> Result<Vec<String>, String> {
    let held = read(root);
    let mut out: Vec<String> = Vec::new();
    for folder in held.writes_mine.into_iter().chain(held.grantable).chain(
        held.granted
            .into_iter()
            .filter(|made| made.what == "write")
            .map(|made| made.target),
    ) {
        if !out.contains(&folder) {
            out.push(folder);
        }
    }
    Ok(out)
}

/// **Each folder this file names that `moved` maps, named as it maps it** (#1698), in the write
/// grants, the listed folders and the Granted list's write records alike, in one write under
/// the file's lock. Only for the local project's move ([`crate::localproject`]), which decides
/// what a folder maps to; one `moved` answers `None` for is left exactly as it is. The file is
/// read and written through no link, and one that is not this shape is left as it is. How many
/// entries changed.
pub fn folders_moved(root: &Path, moved: impl Fn(&str) -> Option<String>) -> io::Result<usize> {
    let file = path(root);
    let mut changed = 0;
    // This machine's file, in the config home (ADR 0093): held to its own folder.
    crate::rewrite::update(&machine_folder(root), &file, |now| {
        let Some(mut held) = now.and_then(|text| serde_json::from_str::<OnDisk>(text).ok()) else {
            return Ok(None);
        };
        let mut point = |folder: &mut String| {
            if let Some(now) = moved(folder) {
                *folder = now;
                changed += 1;
            }
        };
        held.writes_mine.iter_mut().for_each(&mut point);
        held.grantable.iter_mut().for_each(&mut point);
        held.granted
            .iter_mut()
            .filter(|made| made.what == "write")
            .for_each(|made| point(&mut made.target));
        if changed == 0 {
            return Ok(None);
        }
        serde_json::to_string_pretty(&held)
            .map(|text| Some(format!("{text}\n")))
            .map_err(io::Error::other)
    })?;
    Ok(changed)
}

// ---- the vaults you let a persona's chats use (#1430) ---------------------------------------

/// The vaults you let a persona's chats use in the project at `root` on this machine, beside
/// what the vault registry tags. A file that cannot be read grants nothing.
pub fn granted_vaults(root: &Path) -> Vec<VaultGrant> {
    read(root).vaults_mine
}

/// Lets `persona`'s chats in the project at `root` use `vault` on this machine: a refused
/// vault's Notice, never a chat (a sandboxed chat cannot write this file, and no line on the
/// hook channel reaches this).
pub fn grant_vault(root: &Path, vault: &str, persona: &str) -> io::Result<()> {
    let grant = VaultGrant {
        vault: vault.to_owned(),
        persona: persona.to_owned(),
    };
    change(root, |held| {
        if !held.vaults_mine.contains(&grant) {
            held.vaults_mine.push(grant);
        }
    })
}

// ---- the persona hosts you allowed here (#1362) ----------------------------------------------

/// Each persona whose hosts you allowed in the project at `root` on this machine, with the
/// digest of what you allowed. A file that cannot be read allows nothing.
pub fn allowed_persona_hosts(root: &Path) -> Vec<PersonaHostsAllowed> {
    read(root).persona_hosts_mine
}

/// Marks your Allow of `persona`'s hosts as one whose list has changed since: it grants
/// nothing from now on, and stays listed, with Revoke, until it is allowed anew or revoked.
pub fn persona_hosts_changed(root: &Path, persona: &str) -> io::Result<()> {
    change(root, |held| {
        for one in &mut held.persona_hosts_mine {
            if one.persona == persona {
                one.changed = true;
            }
        }
    })
}

/// Records that you allowed `persona`'s hosts as the list whose digest is `digest`, replacing
/// any Allow of an older list: the arrival Notice's Allow, checked by
/// [`super::persona::shown_now`] first, never a chat (a sandboxed chat cannot write this file,
/// and no line on the hook channel reaches this).
pub fn allow_persona_hosts(root: &Path, persona: &str, digest: &str) -> io::Result<()> {
    change(root, |held| {
        held.persona_hosts_mine.retain(|one| one.persona != persona);
        held.persona_hosts_mine.push(PersonaHostsAllowed {
            persona: persona.to_owned(),
            digest: digest.to_owned(),
            changed: false,
        });
    })
}

/// Takes back your Allow of `persona`'s hosts: Settings' Revoke. Answers whether there was one.
/// A chat already running keeps what it started with until it starts again.
pub fn revoke_persona_hosts(root: &Path, persona: &str) -> io::Result<bool> {
    let mut was = false;
    change(root, |held| {
        let before = held.persona_hosts_mine.len();
        held.persona_hosts_mine.retain(|one| one.persona != persona);
        was = held.persona_hosts_mine.len() != before;
        held.granted
            .retain(|made| !(made.what == PERSONA_HOSTS && made.target == persona));
    })?;
    Ok(was)
}

// ---- Always for a persona's browser (#2241) -----------------------------------------------------

/// Every Always for a persona's browser kept here (#2241), whether or not policy holds it.
pub fn browsers_always(root: &Path) -> Vec<BrowserAlways> {
    read(root).browsers_mine
}

/// **Always for `persona`'s browser**, as kept here (#2241): whether it reaches the open web
/// too. `None` where none is kept. Whether policy holds it is [`browser_always_here`]'s.
pub fn browser_always(root: &Path, persona: &str) -> Option<bool> {
    read(root)
        .browsers_mine
        .into_iter()
        .find(|one| one.persona == persona)
        .map(|one| one.open_web)
}

/// [`browser_always`] **in force under `locks`**: none while an administrator's
/// `allow-scopes` leaves out `you`, the scope it is kept at; it stays kept meanwhile.
pub fn browser_always_here(
    root: &Path,
    persona: &str,
    locks: &super::policy::Locks,
) -> Option<bool> {
    if locks.scope_removed(super::grant::Level::You).is_some() {
        return None;
    }
    browser_always(root, persona)
}

/// Keeps **Always for `persona`'s browser**, reaching the open web too with `open_web`, in place
/// of any kept before (#2241). Whether policy allows it is the caller's to have checked.
pub fn keep_browser_always(root: &Path, persona: &str, open_web: bool) -> io::Result<()> {
    change(root, |held| {
        held.browsers_mine.retain(|one| one.persona != persona);
        held.browsers_mine.push(BrowserAlways {
            persona: persona.to_owned(),
            open_web,
        });
    })
}

/// Removes **Always for `persona`'s browser** and its Granted record (#2241). Answers whether
/// one was kept.
pub fn revoke_browser_always(root: &Path, persona: &str) -> io::Result<bool> {
    let mut was = false;
    change(root, |held| {
        let before = held.browsers_mine.len();
        held.browsers_mine.retain(|one| one.persona != persona);
        was = held.browsers_mine.len() != before;
        held.granted
            .retain(|made| !(made.what == BROWSER && made.target == persona));
    })?;
    Ok(was)
}

// ---- browser sites (#2018) ----------------------------------------------------------------------

/// What a [`Made`] record and the audit call a browser site (#2018).
pub const BROWSER_SITE: &str = "browser-site";

/// **The browser sites you allowed for every chat's browser of the project at `root`** (#2018,
/// ruled 2026-10-17): read by the browser's judge and its reach proxy alone, never by a chat's
/// own commands. One this build cannot read is left out, and said in the host's log.
pub fn browser_sites(root: &Path) -> Vec<super::browser_reach::Site> {
    read(root)
        .browser_sites_mine
        .iter()
        .filter_map(|written| match super::browser_reach::Site::parse(written) {
            Ok(site) => Some(site),
            Err(why) => {
                tracing::warn!("purlis: a kept browser site reaches nothing: {why}");
                None
            }
        })
        .collect()
}

/// **Why policy holds the browser site `site` kept at `you`**, where it does (#2018): an
/// administrator's `allow-scopes` leaves `you` out, or never allows its host, though it was
/// allowed before. Held, it reaches no browser and stays kept, as a host kept at a scope is.
pub fn browser_site_held(
    site: &super::browser_reach::Site,
    locks: &super::policy::Locks,
) -> Option<String> {
    browser_site_held_at(site, super::grant::Level::You, locks)
}

/// **Why policy holds the browser site `site` kept at `level`**, where it does: the scope is
/// left out, or its host is never allowed there. A chat's own (#2364) is held as yours is.
pub fn browser_site_held_at(
    site: &super::browser_reach::Site,
    level: super::grant::Level,
    locks: &super::policy::Locks,
) -> Option<String> {
    locks.scope_removed(level).or_else(|| {
        let host = super::hosts::Host::parse(&site.to_string()).ok()?;
        locks.refuses_grant(&super::grant::What::Host(host), level)
    })
}

/// The browser sites kept here that policy leaves in force (#2018): [`browser_sites`], less
/// each one [`browser_site_held`] holds.
pub fn browser_sites_in_force(
    root: &Path,
    locks: &super::policy::Locks,
) -> Vec<super::browser_reach::Site> {
    browser_sites(root)
        .into_iter()
        .filter(|site| browser_site_held(site, locks).is_none())
        .collect()
}

/// Lets every chat's browser of the project at `root` reach `site` on this machine (#2018):
/// a browser's Ask, never a chat.
pub fn grant_browser_site(root: &Path, site: &super::browser_reach::Site) -> io::Result<()> {
    let written = site.to_string();
    change(root, |held| {
        if !held.browser_sites_mine.contains(&written) {
            held.browser_sites_mine.push(written);
        }
    })
}

/// Takes `site` off what you let every chat's browser reach, and its Granted record: Settings'
/// Remove. Answers whether it was kept.
pub fn revoke_browser_site(root: &Path, site: &super::browser_reach::Site) -> io::Result<bool> {
    let written = site.to_string();
    let mut was = false;
    change(root, |held| {
        let before = held.browser_sites_mine.len();
        held.browser_sites_mine.retain(|one| *one != written);
        was = held.browser_sites_mine.len() != before;
        held.granted
            .retain(|made| !(made.what == BROWSER_SITE && made.target == written));
    })?;
    Ok(was)
}

/// The project's browser sites shown to the person on this machine (#2443).
pub fn browser_sites_seen(root: &Path) -> Vec<String> {
    read(root).browser_sites_seen
}

/// Records `seen` as the project's browser sites shown to the person on this machine (#2443).
pub fn keep_browser_sites_seen(root: &Path, seen: Vec<String>) -> io::Result<()> {
    change(root, |held| held.browser_sites_seen = seen)
}

// ---- your dispatch grants, and the project's as you were told of them (#1437) ----------------

/// The dispatch grants you made for every chat of the project at `root` on this machine, each
/// as asking persona and target persona. [`crate::dispatchgrant::yours`] reads them as pairs.
pub fn granted_dispatch(root: &Path) -> Vec<(String, String)> {
    read(root)
        .dispatch_mine
        .into_iter()
        .map(|pair| (pair.asking, pair.target))
        .collect()
}

/// Lets every chat of the project at `root` that runs as `asking` dispatch to `target`, on
/// this machine: the grant Notice's Allow, never a chat (a sandboxed chat cannot write this
/// file).
pub fn grant_dispatch(root: &Path, asking: &str, target: &str) -> io::Result<()> {
    let pair = DispatchPair {
        asking: asking.to_owned(),
        target: target.to_owned(),
    };
    change(root, |held| {
        if !held.dispatch_mine.contains(&pair) {
            held.dispatch_mine.push(pair);
        }
    })
}

/// Takes `vault` off what `persona`'s chats may use here: Settings' Revoke. The next brokered
/// run reads it, so nothing restarts.
pub fn revoke_vault(root: &Path, vault: &str, persona: &str) -> io::Result<()> {
    let grant = VaultGrant {
        vault: vault.to_owned(),
        persona: persona.to_owned(),
    };
    let target = grant.target();
    change(root, |held| {
        held.vaults_mine.retain(|one| *one != grant);
        held.granted
            .retain(|made| !(made.what == VAULT && made.target == target));
    })
}

/// Takes that grant back: Settings' Revoke. Answers whether there was one.
pub fn revoke_dispatch(root: &Path, asking: &str, target: &str) -> io::Result<bool> {
    let mut was = false;
    change(root, |held| {
        let before = held.dispatch_mine.len();
        held.dispatch_mine
            .retain(|pair| !(pair.asking == asking && pair.target == target));
        was = held.dispatch_mine.len() != before;
    })?;
    Ok(was)
}

/// The project's dispatch grants as this machine last told the person of them; `None` before
/// the first.
pub fn dispatch_seen(root: &Path) -> Option<Vec<String>> {
    read(root).dispatch_seen
}

/// Records that the person was told of `shown`, the project's dispatch grants as the Notice
/// showed them.
pub fn acknowledge_dispatch(root: &Path, shown: &[String]) -> io::Result<()> {
    change(root, |held| {
        held.dispatch_seen = Some(shown.to_vec());
        // A yes is the newer answer: what is allowed here is no longer declined here, and is
        // no longer one that was taken away.
        held.dispatch_declined.retain(|one| !shown.contains(one));
        held.dispatch_gone.retain(|one| !shown.contains(&one.said));
    })
}

// ---- any persona, and what an earlier build left of nevers (#1503) -----------------------

/// What `text`, this file, holds under the key one dev build kept nevers in, whatever else in
/// the file reads or does not.
fn legacy_nevers_in(text: &str) -> Option<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()?
        .get("dispatch_never")
        .cloned()
}

/// **The nevers an earlier build left in this file**, for [`crate::dispatchnever`] to move:
/// each as asking persona and target persona. Empty where there are none; `None` where the
/// key is there and is not a list of pairs, so what it refused is unknown.
pub fn legacy_dispatch_nevers(root: &Path) -> Option<Vec<(String, String)>> {
    let Some(left) = std::fs::read_to_string(path(root))
        .ok()
        .as_deref()
        .and_then(legacy_nevers_in)
    else {
        return Some(Vec::new());
    };
    crate::dispatchnever::pairs_of(&left)
}

/// Takes that key out of this file, once what it held is kept where it belongs. Every other
/// key stays as it is written, read by this build or not.
pub fn drop_legacy_dispatch_nevers(root: &Path) -> io::Result<()> {
    crate::rewrite::update(&machine_folder(root), &path(root), |now| {
        let Some(serde_json::Value::Object(mut whole)) =
            now.and_then(|text| serde_json::from_str(text).ok())
        else {
            return Ok(None);
        };
        if whole.remove("dispatch_never").is_none() {
            return Ok(None);
        }
        serde_json::to_string_pretty(&whole)
            .map(|text| Some(format!("{text}\n")))
            .map_err(io::Error::other)
    })
    .map(|_| ())
}

/// The personas whose chats you let dispatch to any persona in the project at `root`, on this
/// machine, as the file spells them. [`crate::dispatchgrant::any_yours`] keeps the names.
pub fn granted_dispatch_any(root: &Path) -> Vec<String> {
    read(root).dispatch_any
}

/// Lets every chat of the project at `root` that runs as `asking` dispatch to any persona, on
/// this machine: Settings, never a Notice's answer and never a chat.
pub fn grant_dispatch_any(root: &Path, asking: &str) -> io::Result<()> {
    change(root, |held| {
        if !held.dispatch_any.iter().any(|one| one == asking) {
            held.dispatch_any.push(asking.to_owned());
        }
    })
}

/// Takes that back: Settings' Revoke. Answers whether there was one.
pub fn revoke_dispatch_any(root: &Path, asking: &str) -> io::Result<bool> {
    let mut was = false;
    change(root, |held| {
        let before = held.dispatch_any.len();
        held.dispatch_any.retain(|one| one != asking);
        was = held.dispatch_any.len() != before;
    })?;
    Ok(was)
}

/// The personas whose "any persona" grant in the project's file you accepted on this machine.
pub fn dispatch_any_seen(root: &Path) -> Vec<String> {
    read(root).dispatch_any_seen
}

/// Records that you accepted the project's "any persona" grant for `asking` on this machine.
pub fn acknowledge_dispatch_any(root: &Path, asking: &str) -> io::Result<()> {
    change(root, |held| {
        if !held.dispatch_any_seen.iter().any(|one| one == asking) {
            held.dispatch_any_seen.push(asking.to_owned());
        }
        let said = format!("{asking} -> {}", crate::dispatchgrant::ANY);
        held.dispatch_declined.retain(|one| *one != said);
        held.dispatch_gone.retain(|one| one.said != said);
    })
}

/// Takes `asking` off what you accepted of the project's "any persona" grants.
pub fn forget_dispatch_any(root: &Path, asking: &str) -> io::Result<()> {
    change(root, |held| {
        held.dispatch_any_seen.retain(|one| one != asking)
    })
}

// ---- a persona that goes away, what was set aside, and what was declined (#1504) -------------

/// The asking and target names of a pair as [`crate::dispatchgrant::Pair`] is displayed.
fn sides(said: &str) -> Option<(&str, &str)> {
    said.split_once(" -> ")
}

/// Every name the records in force hold: the personas of your grants, of "any persona", and
/// of what this machine accepted of the project's.
fn granted_names(held: &OnDisk) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut add = |name: &str| {
        if name != crate::dispatchgrant::ANY && !names.iter().any(|one| one == name) {
            names.push(name.to_owned());
        }
    };
    for pair in &held.dispatch_mine {
        add(&pair.asking);
        add(&pair.target);
    }
    for name in held.dispatch_any.iter().chain(&held.dispatch_any_seen) {
        add(name);
    }
    for said in held.dispatch_seen.iter().flatten() {
        if let Some((asking, target)) = sides(said) {
            add(asking);
            add(target);
        }
    }
    for one in held.dispatch_mine_in.iter().chain(&held.dispatch_seen_in) {
        add(&one.asking);
        if !one.any {
            add(&one.target);
        }
    }
    names
}

/// Every name a dispatch grant in force on this machine holds, in the project at `root`.
pub fn dispatch_granted_names(root: &Path) -> Vec<String> {
    granted_names(&read(root))
}

/// What this machine knows of the personas its dispatch grants name.
pub fn dispatch_known(root: &Path) -> Vec<Known> {
    read(root).dispatch_known
}

/// **Marks each of `names` as seen gone**, in one write: the absence mark. With `forget`, what
/// was known of its definition is dropped too, so nothing that comes back under the name is
/// taken for it (purlis's own removal). Nothing is written where each is marked already.
pub fn mark_dispatch_away(root: &Path, names: &[String], forget: bool) -> io::Result<()> {
    let known = read(root).dispatch_known;
    let marked = |name: &String| {
        known
            .iter()
            .any(|one| one.name == *name && one.away && !(forget && !one.hash.is_empty()))
    };
    if names.iter().all(marked) {
        return Ok(());
    }
    change(root, |held| {
        for name in names {
            match held.dispatch_known.iter_mut().find(|one| one.name == *name) {
                Some(one) => {
                    one.away = true;
                    if forget {
                        one.hash.clear();
                    }
                }
                None => held.dispatch_known.push(Known {
                    name: name.clone(),
                    hash: String::new(),
                    away: true,
                }),
            }
        }
    })
}

/// **Records each name of `seen` as there, with its definition's hash**, in one write: the
/// absence mark of each is lifted. Nothing is written where each is known so already.
pub fn know_dispatch_personas(root: &Path, seen: &[(String, String)]) -> io::Result<()> {
    let known = read(root).dispatch_known;
    let same = |(name, hash): &(String, String)| {
        known
            .iter()
            .any(|one| one.name == *name && one.hash == *hash && !one.away)
    };
    if seen.iter().all(same) {
        return Ok(());
    }
    change(root, |held| {
        for (name, hash) in seen {
            match held.dispatch_known.iter_mut().find(|one| one.name == *name) {
                Some(one) => {
                    one.hash.clone_from(hash);
                    one.away = false;
                }
                None => held.dispatch_known.push(Known {
                    name: name.clone(),
                    hash: hash.clone(),
                    away: false,
                }),
            }
        }
    })
}

/// The grants of yours set aside in the project at `root`, oldest first.
pub fn dormant_dispatch(root: &Path) -> Vec<Dormant> {
    read(root).dispatch_dormant
}

/// What this machine accepted of the project's grants and set aside, in the project at `root`.
pub fn accepted_aside_dispatch(root: &Path) -> Vec<Accepted> {
    read(root).dispatch_accepted_aside
}

/// What [`set_aside_dispatch`] took out of force.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetAside {
    /// Your grants, as they are now kept.
    pub grants: Vec<Dormant>,
    /// What this machine had accepted of the project's grants.
    pub accepted: Vec<Accepted>,
    /// Your grants limited to one workspace, and what this machine had accepted of the
    /// project's such grants (#1505): **ended, not set aside**. They are in no record now and
    /// are made again by asking.
    pub ended: Vec<(crate::sandbox::grant::Level, DispatchIn)>,
}

impl SetAside {
    /// Whether nothing was.
    pub fn is_empty(&self) -> bool {
        self.grants.is_empty() && self.accepted.is_empty() && self.ended.is_empty()
    }
}

/// **Sets aside every grant of yours that names a name `gone` says changed hands**, in one
/// write: each pair of `dispatch_mine` and each name of `dispatch_any` goes to
/// `dispatch_dormant`, and what this machine accepted of the project's grants for the name
/// goes from `dispatch_seen` and `dispatch_any_seen` to `dispatch_accepted_aside`, so a pair
/// the project's file still holds waits for a yes again. **Nothing is lost**: what the
/// Granted record says of each (when, and from which chat) stays, for Give back. Answers what
/// was set aside. Nothing is written where nothing names such a name.
pub fn set_aside_dispatch(root: &Path, gone: &dyn Fn(&str) -> bool) -> io::Result<SetAside> {
    if !granted_names(&read(root)).iter().any(|name| gone(name)) {
        return Ok(SetAside::default());
    }
    let mut out = SetAside::default();
    change(root, |held| {
        out = SetAside::default();
        let mut keep = Vec::new();
        for pair in std::mem::take(&mut held.dispatch_mine) {
            let was = if gone(&pair.asking) {
                Some(pair.asking.clone())
            } else if gone(&pair.target) {
                Some(pair.target.clone())
            } else {
                None
            };
            match was {
                Some(was) => out.grants.push(Dormant {
                    asking: pair.asking,
                    target: pair.target,
                    any: false,
                    was,
                }),
                None => keep.push(pair),
            }
        }
        held.dispatch_mine = keep;
        for asking in std::mem::take(&mut held.dispatch_any) {
            if gone(&asking) {
                out.grants.push(Dormant {
                    was: asking.clone(),
                    asking,
                    target: crate::dispatchgrant::ANY.to_owned(),
                    any: true,
                });
            } else {
                held.dispatch_any.push(asking);
            }
        }
        for asking in std::mem::take(&mut held.dispatch_any_seen) {
            if gone(&asking) {
                out.accepted.push(Accepted {
                    said: format!("{asking} -> {}", crate::dispatchgrant::ANY),
                    was: asking,
                });
            } else {
                held.dispatch_any_seen.push(asking);
            }
        }
        if let Some(seen) = held.dispatch_seen.as_mut() {
            for said in std::mem::take(seen) {
                let was = sides(&said).and_then(|(asking, target)| {
                    [asking, target]
                        .into_iter()
                        .find(|side| gone(side))
                        .map(str::to_owned)
                });
                match was {
                    Some(was) => out.accepted.push(Accepted { said, was }),
                    None => seen.push(said),
                }
            }
        }
        // A grant limited to one workspace that names the name ends: the record of what is
        // set aside keeps no condition, and one given back without it would hold everywhere.
        let names = |one: &DispatchIn| gone(&one.asking) || (!one.any && gone(&one.target));
        for (level, list) in [
            (
                crate::sandbox::grant::Level::You,
                &mut held.dispatch_mine_in,
            ),
            (
                crate::sandbox::grant::Level::Project,
                &mut held.dispatch_seen_in,
            ),
        ] {
            for one in std::mem::take(list) {
                if names(&one) {
                    out.ended.push((level, one));
                } else {
                    list.push(one);
                }
            }
        }
        for one in &out.grants {
            if !held.dispatch_dormant.contains(one) {
                held.dispatch_dormant.push(one.clone());
            }
        }
        for said in &out.accepted {
            if !held.dispatch_accepted_aside.contains(said) {
                held.dispatch_accepted_aside.push(said.clone());
            }
        }
    })?;
    Ok(out)
}

/// Takes one grant off what was set aside: Settings' Remove. `any` says which is meant, the
/// one set aside as "any persona" or the pair, so one press takes one entry. Answers whether
/// it was there.
pub fn forget_dormant_dispatch(
    root: &Path,
    asking: &str,
    target: &str,
    any: bool,
) -> io::Result<bool> {
    let mut was = false;
    change(root, |held| {
        let before = held.dispatch_dormant.len();
        held.dispatch_dormant
            .retain(|one| !(one.asking == asking && one.target == target && one.any == any));
        was = held.dispatch_dormant.len() != before;
    })?;
    Ok(was)
}

/// **Gives `name` back what was set aside for it**, in one write: Settings' own action, on
/// the person's press. Each grant set aside for the name that `ok` passes is in force again,
/// each acceptance set aside for it that `holds` says the project's file still has is
/// accepted again, and the name's absence mark is lifted with `hash` as what is known of it.
/// What `ok` does not pass stays set aside; an acceptance the file no longer has is dropped.
/// Answers what came back.
pub fn give_back_dispatch(
    root: &Path,
    name: &str,
    hash: &str,
    ok: &dyn Fn(&Dormant) -> bool,
    holds: &dyn Fn(&str) -> bool,
) -> io::Result<SetAside> {
    let mut back = SetAside::default();
    change(root, |held| {
        back = SetAside::default();
        for one in std::mem::take(&mut held.dispatch_dormant) {
            if one.was != name || !ok(&one) {
                held.dispatch_dormant.push(one);
                continue;
            }
            if one.any {
                if !held.dispatch_any.contains(&one.asking) {
                    held.dispatch_any.push(one.asking.clone());
                }
            } else {
                let pair = DispatchPair {
                    asking: one.asking.clone(),
                    target: one.target.clone(),
                };
                if !held.dispatch_mine.contains(&pair) {
                    held.dispatch_mine.push(pair);
                }
            }
            back.grants.push(one);
        }
        for one in std::mem::take(&mut held.dispatch_accepted_aside) {
            if one.was != name {
                held.dispatch_accepted_aside.push(one);
                continue;
            }
            if !holds(&one.said) || held.dispatch_declined.contains(&one.said) {
                continue;
            }
            let said = one.said.clone();
            match sides(&said) {
                Some((asking, target)) if target == crate::dispatchgrant::ANY => {
                    if !held.dispatch_any_seen.iter().any(|one| one == asking) {
                        held.dispatch_any_seen.push(asking.to_owned());
                    }
                }
                _ => {
                    let seen = held.dispatch_seen.get_or_insert_with(Vec::new);
                    if !seen.contains(&said) {
                        seen.push(said.clone());
                    }
                }
            }
            back.accepted.push(one);
        }
        match held.dispatch_known.iter_mut().find(|one| one.name == name) {
            Some(one) => {
                hash.clone_into(&mut one.hash);
                one.away = false;
            }
            None => held.dispatch_known.push(Known {
                name: name.to_owned(),
                hash: hash.to_owned(),
                away: false,
            }),
        }
    })?;
    Ok(back)
}

/// The project's grants you said "Not on my machine" to in the project at `root`.
pub fn dispatch_declined(root: &Path) -> Vec<String> {
    read(root).dispatch_declined
}

/// **Not on my machine**, for the project's grant `said` (a pair as it is displayed, or
/// `<asking> -> *`): this machine's acceptance of it goes, and it is remembered as declined,
/// in one write.
pub fn decline_dispatch(root: &Path, said: &str) -> io::Result<()> {
    change(root, |held| {
        if let Some(seen) = held.dispatch_seen.as_mut() {
            seen.retain(|one| one != said);
        }
        if let Some((asking, target)) = sides(said)
            && target == crate::dispatchgrant::ANY
        {
            held.dispatch_any_seen.retain(|one| one != asking);
        }
        held.dispatch_accepted_aside.retain(|one| one.said != said);
        if !held.dispatch_declined.iter().any(|one| one == said) {
            held.dispatch_declined.push(said.to_owned());
        }
        // Answered: it is no longer one to say was accepted before.
        held.dispatch_gone.retain(|one| one.said != said);
    })
}

// ---- what an acceptance is bound to (#1506) --------------------------------------------------

/// What this machine keeps of the project's grants, as [`crate::dispatcharrival`] reads it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispatchBound {
    /// The pairs accepted here, each as [`crate::dispatchgrant::Pair`] is displayed.
    pub seen: Vec<String>,
    /// The asking personas whose "any persona" grant was accepted here.
    pub any_seen: Vec<String>,
    /// The grants declined here, a pair as it is displayed or `<asking> -> *`.
    pub declined: Vec<String>,
    /// The commit the acceptances and declines were last checked through.
    pub at: Option<String>,
    /// The grants whose acceptance was dropped, with why.
    pub gone: Vec<Gone>,
    /// The project's grants limited to one workspace accepted here (#1505), each as
    /// [`crate::dispatchwithin::Limited`] is displayed: bound to the history as pairs are.
    pub seen_in: Vec<String>,
    /// What was accepted of the project's grants for a name whose grants are set aside
    /// (#1504): bound too, so a Give back never restores one a commit took out meanwhile.
    pub aside: Vec<String>,
}

/// A limited grant of the project's as [`DispatchBound::seen_in`] spells it.
fn limited_said(one: &DispatchIn) -> String {
    let target = if one.any {
        crate::dispatchgrant::ANY
    } else {
        one.target.as_str()
    };
    format!("{} -> {target} in {}", one.asking, one.workspace)
}

/// What this machine keeps of the project's grants in the project at `root`.
pub fn dispatch_bound(root: &Path) -> DispatchBound {
    let held = read(root);
    DispatchBound {
        seen: held.dispatch_seen.unwrap_or_default(),
        any_seen: held.dispatch_any_seen,
        declined: held.dispatch_declined,
        at: held.dispatch_seen_at,
        gone: held.dispatch_gone,
        seen_in: held.dispatch_seen_in.iter().map(limited_said).collect(),
        aside: held
            .dispatch_accepted_aside
            .iter()
            .map(|one| one.said.clone())
            .collect(),
    }
}

/// **Records that the person accepted the project's grant `said`** (a pair as it is displayed,
/// or `<asking> -> *`), in one write: it is added to what was accepted, and is no longer
/// declined, dropped, or set aside. `holds` is asked **inside the write** whether the
/// project's file holds the grant now; where it does not, nothing is written and the answer is
/// `false`. Nothing is read first and written back whole, so a drop made meanwhile stays made.
pub fn accept_dispatch(root: &Path, said: &str, holds: &dyn Fn() -> bool) -> io::Result<bool> {
    let mut kept = false;
    change(root, |held| {
        if !holds() {
            return;
        }
        kept = true;
        match sides(said) {
            Some((asking, target)) if target == crate::dispatchgrant::ANY => {
                if !held.dispatch_any_seen.iter().any(|one| one == asking) {
                    held.dispatch_any_seen.push(asking.to_owned());
                }
            }
            _ => {
                let seen = held.dispatch_seen.get_or_insert_with(Vec::new);
                if !seen.iter().any(|one| one == said) {
                    seen.push(said.to_owned());
                }
            }
        }
        held.dispatch_declined.retain(|one| one != said);
        held.dispatch_gone.retain(|one| one.said != said);
        held.dispatch_accepted_aside.retain(|one| one.said != said);
    })?;
    Ok(kept)
}

/// One settling of what this machine keeps of the project's grants
/// ([`crate::dispatcharrival::settle`]), written at once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DispatchSettled {
    /// The acceptances that no longer hold, each with why: each leaves `dispatch_seen` or
    /// `dispatch_any_seen` and is remembered in `dispatch_gone`.
    pub dropped: Vec<Gone>,
    /// The declines whose grant a commit took out of the project's file: forgotten.
    pub undeclined: Vec<String>,
    /// The commit the acceptances are checked through now; none where no commit could be
    /// read.
    pub at: Option<String>,
}

/// Writes `settled`, in one write. Only ever narrows what is in force.
pub fn settle_dispatch(root: &Path, settled: &DispatchSettled) -> io::Result<()> {
    change(root, |held| {
        for gone in &settled.dropped {
            let said = &gone.said;
            let mut was = false;
            match sides(said) {
                Some((asking, target)) if target == crate::dispatchgrant::ANY => {
                    let before = held.dispatch_any_seen.len();
                    held.dispatch_any_seen.retain(|one| one != asking);
                    was = held.dispatch_any_seen.len() != before;
                }
                _ => {
                    if let Some(seen) = held.dispatch_seen.as_mut() {
                        let before = seen.len();
                        seen.retain(|one| one != said);
                        was = seen.len() != before;
                    }
                }
            }
            // A grant limited to one workspace, and one set aside for a name that changed
            // hands, are dropped the same way.
            let (in_before, aside_before) = (
                held.dispatch_seen_in.len(),
                held.dispatch_accepted_aside.len(),
            );
            held.dispatch_seen_in
                .retain(|one| limited_said(one) != *said);
            held.dispatch_accepted_aside.retain(|one| one.said != *said);
            was |= held.dispatch_seen_in.len() != in_before
                || held.dispatch_accepted_aside.len() != aside_before;
            if was {
                held.dispatch_gone.retain(|one| one.said != *said);
                held.dispatch_gone.push(gone.clone());
            }
        }
        held.dispatch_declined
            .retain(|one| !settled.undeclined.contains(one));
        let bound = held
            .dispatch_seen
            .as_ref()
            .is_some_and(|seen| !seen.is_empty())
            || !held.dispatch_any_seen.is_empty()
            || !held.dispatch_declined.is_empty()
            || !held.dispatch_seen_in.is_empty()
            || !held.dispatch_accepted_aside.is_empty();
        held.dispatch_seen_at = settled.at.clone().filter(|_| bound);
    })
}

/// Forgets that `said`, grants accepted here, were taken away: the person was told.
pub fn forget_dispatch_gone(root: &Path, said: &[String]) -> io::Result<()> {
    change(root, |held| {
        held.dispatch_gone.retain(|one| !said.contains(&one.said))
    })
}

// ---- a grant limited to one workspace (#1505) ------------------------------------------------

/// Your dispatch grants that hold in one workspace only, in the project at `root`, as the
/// file spells them. [`crate::dispatchwithin`] says which of them count.
pub fn dispatch_mine_in(root: &Path) -> Vec<DispatchIn> {
    read(root).dispatch_mine_in
}

/// The project's grants limited to one workspace that you accepted on this machine.
pub fn dispatch_seen_in(root: &Path) -> Vec<DispatchIn> {
    read(root).dispatch_seen_in
}

/// What this machine knows of the workspaces limited grants name.
pub fn dispatch_workspaces(root: &Path) -> Vec<KnownWorkspace> {
    read(root).dispatch_workspaces
}

/// Which of the two records of limited grants is meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limited {
    /// Yours: `dispatch_mine_in`.
    Mine,
    /// What you accepted of the project's: `dispatch_seen_in`.
    Accepted,
}

fn limited(held: &mut OnDisk, which: Limited) -> &mut Vec<DispatchIn> {
    match which {
        Limited::Mine => &mut held.dispatch_mine_in,
        Limited::Accepted => &mut held.dispatch_seen_in,
    }
}

/// **Keeps `grant` in `which` record**, in one write: where the same grant is there already,
/// what it had seen of its workspace is brought up to `grant.seen`, which is how the person
/// confirms it again.
pub fn keep_dispatch_in(root: &Path, which: Limited, grant: &DispatchIn) -> io::Result<()> {
    change(root, |held| {
        let list = limited(held, which);
        list.retain(|one| !one.is(&grant.asking, &grant.target, grant.any, &grant.workspace));
        list.push(grant.clone());
    })
}

/// Takes the grant of `asking` to `target` (`any`: any persona) in `workspace` out of `which`
/// record. Answers whether it was there.
pub fn drop_dispatch_in(
    root: &Path,
    which: Limited,
    asking: &str,
    target: &str,
    any: bool,
    workspace: &str,
) -> io::Result<bool> {
    let mut was = false;
    change(root, |held| {
        let list = limited(held, which);
        let before = list.len();
        list.retain(|one| !one.is(asking, target, any, workspace));
        was = list.len() != before;
    })?;
    Ok(was)
}

/// **Changes where your grant of `asking` to `target` (`any`: any persona) holds**, in one
/// write: `from` is the workspace it holds in now (`None`: any workspace), `to` the one it is
/// to hold in, with what this machine has seen of that name (`None`: any workspace). Answers
/// whether the grant was there as `from` says; where it was not, nothing is written.
///
/// **Widening to any workspace takes the pair's other limited grants with it**: they are
/// covered by the one that now holds everywhere.
pub fn move_dispatch_mine(
    root: &Path,
    asking: &str,
    target: &str,
    any: bool,
    from: Option<&str>,
    to: Option<(&str, u32)>,
) -> io::Result<bool> {
    let mut was = false;
    change(root, |held| {
        was = match from {
            Some(workspace) => held
                .dispatch_mine_in
                .iter()
                .any(|one| one.is(asking, target, any, workspace)),
            None if any => held.dispatch_any.iter().any(|one| one == asking),
            None => held
                .dispatch_mine
                .iter()
                .any(|pair| pair.asking == asking && pair.target == target),
        };
        if !was {
            return;
        }
        match from {
            Some(workspace) => held
                .dispatch_mine_in
                .retain(|one| !one.is(asking, target, any, workspace)),
            None if any => held.dispatch_any.retain(|one| one != asking),
            None => held
                .dispatch_mine
                .retain(|pair| !(pair.asking == asking && pair.target == target)),
        }
        match to {
            Some((workspace, seen)) => {
                held.dispatch_mine_in
                    .retain(|one| !one.is(asking, target, any, workspace));
                held.dispatch_mine_in.push(DispatchIn {
                    asking: asking.to_owned(),
                    target: target.to_owned(),
                    any,
                    workspace: workspace.to_owned(),
                    seen,
                });
            }
            None => {
                held.dispatch_mine_in.retain(|one| {
                    !(one.asking == asking && one.any == any && (any || one.target == target))
                });
                if any {
                    if !held.dispatch_any.iter().any(|one| one == asking) {
                        held.dispatch_any.push(asking.to_owned());
                    }
                } else {
                    let pair = DispatchPair {
                        asking: asking.to_owned(),
                        target: target.to_owned(),
                    };
                    if !held.dispatch_mine.contains(&pair) {
                        held.dispatch_mine.push(pair);
                    }
                }
            }
        }
    })?;
    Ok(was)
}

/// **Records which of `names` is a workspace of the project now** (`there`), in one write:
/// a name that is gone, and was not when last looked at, has its count of times seen gone
/// raised by one. **The one thing reading the limited grants writes**, and it only ever takes
/// grants out of force. Nothing is written where nothing changed.
pub fn note_dispatch_workspaces(
    root: &Path,
    names: &[String],
    there: &dyn Fn(&str) -> bool,
) -> io::Result<()> {
    let known = read(root).dispatch_workspaces;
    let same = |name: &String| {
        let now = there(name);
        match known.iter().find(|one| one.name == *name) {
            Some(one) => one.away != now,
            None => now,
        }
    };
    if names.iter().all(same) {
        return Ok(());
    }
    change(root, |held| {
        for name in names {
            let now = there(name);
            match held
                .dispatch_workspaces
                .iter_mut()
                .find(|one| one.name == *name)
            {
                Some(one) => {
                    if !now && !one.away {
                        one.gone = one.gone.saturating_add(1);
                    }
                    one.away = !now;
                }
                None if now => {}
                None => held.dispatch_workspaces.push(KnownWorkspace {
                    name: name.clone(),
                    gone: 1,
                    away: true,
                }),
            }
        }
    })
}

// ---- the opt-out count -----------------------------------------------------------------------

/// How a new chat in a sandboxed project started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Started {
    /// Under the sandbox.
    Sandboxed,
    /// Without it, because a person turned it off for that chat.
    OptedOut,
    /// Without it, because this machine has no sandbox backend (Windows, ruling V21 3).
    NoBackend,
}

/// How many new chats in a sandboxed project started each way on this machine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Tally {
    #[serde(default)]
    pub sandboxed: u64,
    #[serde(default)]
    pub opted_out: u64,
    #[serde(default)]
    pub no_backend: u64,
}

impl Tally {
    /// The opt-outs among the chats a person could have run sandboxed, as a whole percent
    /// rounded up, so a rate is never shown under the bar when it is not; `None` before the
    /// first such chat. A chat on a machine with no backend was never a choice, so it is not
    /// in the rate.
    pub fn rate_percent(&self) -> Option<u64> {
        let chose = self.sandboxed + self.opted_out;
        (chose > 0).then(|| (self.opted_out * 100).div_ceil(chose))
    }

    /// One sentence: the rate, and the bar it is held to (V12).
    pub fn said(&self) -> String {
        let chose = self.sandboxed + self.opted_out;
        let mut parts = Vec::new();
        if let Some(rate) = self.rate_percent() {
            parts.push(format!(
                "{} of {chose} {} started without the sandbox on this machine ({rate}%); the \
                 bar is under 10%",
                self.opted_out,
                chats(chose)
            ));
        }
        if self.no_backend > 0 {
            parts.push(format!(
                "{} {} started without the sandbox because this machine has no sandbox backend",
                self.no_backend,
                chats(self.no_backend)
            ));
        }
        if parts.is_empty() {
            return "no chat has started under this project's sandbox on this machine yet"
                .to_owned();
        }
        parts.join("; ")
    }
}

fn chats(n: u64) -> &'static str {
    if n == 1 { "chat" } else { "chats" }
}

/// What this machine counted for the project at `root`. A file that cannot be read counts
/// nothing.
pub fn tally(root: &Path) -> Tally {
    read(root).chats
}

/// Counts one new chat in the project at `root`.
pub fn count(root: &Path, started: Started) -> io::Result<()> {
    change(root, |held| {
        let n = match started {
            Started::Sandboxed => &mut held.chats.sandboxed,
            Started::OptedOut => &mut held.chats.opted_out,
            Started::NoBackend => &mut held.chats.no_backend,
        };
        *n = n.saturating_add(1);
    })
}

#[cfg(test)]
#[path = "local_tests.rs"]
mod tests;
