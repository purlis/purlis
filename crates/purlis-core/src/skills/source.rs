// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Skill sources** (SK-10, #2773; [ADR 0100]; spec #2762 decisions 7, 20 and 39): a git
//! repository of skill folders that a person adds, browses, and installs skills from, each install
//! a reviewed copy pinned to a commit in `skills.lock`, and each newer commit offered as a
//! reviewed diff, never applied by itself. One copy of each verb, which the Skill studio and
//! `purlis skill install | update` both call.
//!
//! **A fetched source is never checked out.** It is fetched into a bare repository of its own in
//! a new temporary folder, by purlis's hardened git (no hooks, no submodules, every object
//! checked by `transfer.fsckObjects`), and its tree is read from git's object store
//! ([`purlis_skills::tree`]): a link, another repository or a name that could lead out is left out
//! and said, and every file is written by [`write`] under names purlis made, through handles that
//! follow no link.
//!
//! **What is installed is what was reviewed.** The review answers the commit and a digest of
//! each skill's files; an install is handed both back ([`Accepted`]), fetches that commit again,
//! and refuses where any digest differs. The pin moves only through an accepted update.
//!
//! **A private source signs in with a vault entry** (ADR 0100 §5): resolved through `purlis
//! secret exec`'s own steps ([`crate::secrets::exec`]: the read is recorded by name, and a
//! sandboxed chat never reads the keyring), handed to git as one variable its one credential
//! helper prints, never on a command line, and taken out of anything git says before it is said.
//!
//! [ADR 0100]: ../../../../docs/adr/0100-a-skill-is-installed-from-a-git-source-as-a-pinned-copy-and-never-updates-by-itself.md

use std::path::{Path, PathBuf};

use purlis_skills::review::{self, Changes, FileIn, Summary};
use purlis_skills::tree::{self, Entry, LeftOut};
use serde::{Deserialize, Serialize};

use super::Grant;
use super::verbs::{self, Ctx, Level};
use crate::worktree::git;

pub mod lock;
mod write;

pub use lock::{Locked, Lockfile};

/// The table of the settings files the project's sources are in.
pub const TABLE: &str = "skill_sources";

/// The variable a private source's credential is handed to git in.
const TOKEN_VAR: &str = "PURLIS_SKILL_SOURCE_TOKEN";

/// The variable the user name for it is handed in.
const USER_VAR: &str = "PURLIS_SKILL_SOURCE_USER";

/// The variable the source's host is handed in: the only host the credential is answered for.
const HOST_VAR: &str = "PURLIS_SKILL_SOURCE_HOST";

/// The ref a fetch lands at in the scratch repository.
const FETCHED: &str = "refs/purlis/fetched";

/// The ref the pinned commit lands at, for an update's diff.
const PINNED: &str = "refs/purlis/pinned";

/// The most bytes one skill's files may hold in all.
const MOST_SKILL_BYTES: u64 = 16 * 1024 * 1024;

/// **How a private source signs in**: the vault and the entry in it that holds the token, and the
/// user name the forge wants beside a token, where it wants one. Never the token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultEntry {
    pub vault: String,
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// **A skill source** (ADR 0100 §1): a git repository's URL, and optionally its branch and the
/// folder inside it the skills are in, and how a private one signs in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in: Option<VaultEntry>,
}

/// Why `url` cannot be a skill source's, or `None` where it can: an `https://` URL with no
/// credential in it, a `file://` URL, or a folder on this machine by its absolute path.
pub fn url_problem(url: &str) -> Option<String> {
    if url.is_empty() {
        return Some("it is empty".to_owned());
    }
    if url.len() > 2048 || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Some("it holds a blank or a control character".to_owned());
    }
    if url.starts_with('-') || url.contains("::") {
        return Some("it is not a URL git reads as a place".to_owned());
    }
    if let Some(rest) = url.strip_prefix("https://") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        if authority.contains('@') {
            return Some(
                "it carries a sign-in, which a source never does: name a vault entry for it \
                 instead"
                    .to_owned(),
            );
        }
        if authority.is_empty() {
            return Some("it names no host".to_owned());
        }
        return None;
    }
    if url.starts_with("file:///") || url.starts_with('/') {
        return None;
    }
    Some(
        "a skill source is an https:// URL, or a folder on this machine by its full path"
            .to_owned(),
    )
}

/// Why `branch` cannot be a source's branch, or `None`.
fn branch_problem(branch: &str) -> Option<String> {
    let plain = !branch.is_empty()
        && branch.len() <= 200
        && branch
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c))
        && !branch.starts_with(['-', '/', '.'])
        && !branch.ends_with(['/', '.'])
        && !branch.ends_with(".lock")
        && !branch.contains("..")
        && !branch.contains("//")
        && !branch.contains("/.");
    (!plain).then(|| format!("{branch:?} is not a branch name purlis reads"))
}

impl Source {
    /// **Checks the source**: its URL, its branch and its folder, as a sentence where one cannot
    /// be used.
    pub fn check(&self) -> Result<(), String> {
        if let Some(why) = url_problem(&self.url) {
            return Err(format!("{:?} cannot be a skill source: {why}", self.url));
        }
        if let Some(why) = self.branch.as_deref().and_then(branch_problem) {
            return Err(why);
        }
        if self.folder_parts().is_none() {
            return Err(format!(
                "{:?} is not a folder purlis reads in a source",
                self.folder.as_deref().unwrap_or_default()
            ));
        }
        if let Some(sign_in) = &self.sign_in
            && (sign_in.vault.is_empty() || sign_in.key.is_empty())
        {
            return Err("a private source names both its vault and the entry in it".to_owned());
        }
        Ok(())
    }

    /// Its folder's parts, or `None` for one that is not a plain path.
    fn folder_parts(&self) -> Option<Vec<String>> {
        tree::folder_parts(self.folder.as_deref().unwrap_or_default())
    }

    /// The ref its newest commit is at: its branch's, or the repository's own `HEAD`.
    fn tip_ref(&self) -> String {
        match &self.branch {
            Some(branch) => format!("refs/heads/{branch}"),
            None => "HEAD".to_owned(),
        }
    }

    /// Whether it is the source an install from `from` was made from.
    fn made(&self, from: &lock::From) -> bool {
        self.url == from.url && self.branch == from.branch
    }
}

/// `source` as a person reads it: its URL, then its branch and folder where it names them.
impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.url)?;
        if let Some(branch) = &self.branch {
            write!(f, " ({branch})")?;
        }
        if let Some(folder) = self.folder.as_deref().filter(|f| !f.is_empty()) {
            write!(f, ", {folder}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------
// The project's sources, a setting.
// ---------------------------------------------------------------------------------------------

/// A source as a settings file writes it: its sign-in's keys beside the rest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Written {
    url: String,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    vault: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    user: Option<String>,
}

impl Written {
    fn source(self) -> Source {
        let sign_in = match (self.vault, self.key) {
            (Some(vault), Some(key)) => Some(VaultEntry {
                vault,
                key,
                user: self.user,
            }),
            _ => None,
        };
        Source {
            url: self.url,
            branch: self.branch,
            folder: self.folder,
            sign_in,
        }
    }
}

/// One source the project browses, and whether it is this machine's alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    pub source: Source,
    pub local: bool,
}

/// The sources the settings text `text` lists. One that is not whole is left out.
fn listed_in(text: &str) -> Vec<Source> {
    #[derive(Deserialize)]
    struct Doc {
        #[serde(default, rename = "skill_sources")]
        sources: Vec<toml::Value>,
    }
    let Ok(doc) = toml::from_str::<Doc>(text) else {
        return Vec::new();
    };
    doc.sources
        .into_iter()
        .filter_map(|value| value.try_into::<Written>().ok())
        .map(Written::source)
        .filter(|source| source.check().is_ok())
        .collect()
}

/// **The sources the project at `plane` browses**: the shared ones, then this machine's.
///
/// **A sign-in is read from this machine's file alone.** A shared source's vault entry is
/// dropped, and this machine's entry for the same URL and branch gives it its sign-in: a
/// committed file could otherwise name the person's vault beside a URL of its own choosing,
/// and the token would be handed to that host. Each person signs in with their own entry.
pub fn sources(plane: &Path) -> Vec<Listed> {
    use crate::settings::{Which, layer_text};
    let read = |which| {
        layer_text(plane, which)
            .text()
            .map(listed_in)
            .unwrap_or_default()
    };
    let mut locals = read(Which::Local);
    let mut out: Vec<Listed> = Vec::new();
    for mut source in read(Which::Shared) {
        source.sign_in = None;
        if let Some(at) = locals
            .iter()
            .position(|one| one.url == source.url && one.branch == source.branch)
        {
            source.sign_in = locals.remove(at).sign_in;
        }
        out.push(Listed {
            source,
            local: false,
        });
    }
    out.extend(locals.into_iter().map(|source| Listed {
        source,
        local: true,
    }));
    out
}

/// The source the project lists as `url` on `branch`, for its sign-in.
fn listed_as(plane: &Path, url: &str, branch: Option<&str>) -> Option<Source> {
    sources(plane)
        .into_iter()
        .map(|one| one.source)
        .find(|one| one.url == url && one.branch.as_deref() == branch)
}

/// **Adds `source` to the project's sources**, shared or this machine's (`local`), committed where
/// shared. One already listed by its URL and branch is said and changes nothing.
pub fn add(cx: &mut Ctx<'_>, source: &Source, local: bool) -> Result<bool, String> {
    source.check()?;
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    if listed_as(plane, &source.url, source.branch.as_deref()).is_some() {
        return Ok(false);
    }
    use crate::settings::Which;
    let which = if local { Which::Local } else { Which::Shared };
    let sign_in = |written: &mut Written| {
        written.vault = source.sign_in.as_ref().map(|s| s.vault.clone());
        written.key = source.sign_in.as_ref().map(|s| s.key.clone());
        written.user = source.sign_in.as_ref().and_then(|s| s.user.clone());
    };
    let mut written = Written {
        url: source.url.clone(),
        branch: source.branch.clone(),
        folder: source.folder.clone(),
        ..Written::default()
    };
    // The sign-in goes in this machine's file alone ([`sources`]); a shared source's own line
    // names none.
    if local {
        sign_in(&mut written);
    } else if source.sign_in.is_some() {
        let mut mine = Written {
            url: source.url.clone(),
            branch: source.branch.clone(),
            ..Written::default()
        };
        sign_in(&mut mine);
        crate::settings::skill_sources::edit(plane, Which::Local, |list| {
            list.push(table_of(&mine));
            true
        })?;
    }
    crate::settings::skill_sources::edit(plane, which, |list| {
        list.push(table_of(&written));
        true
    })?;
    cx.done(format!("{source} is a skill source of this project"));
    if !local {
        cx.commit(&[which.file_at(plane).to_owned()], "skill source add");
    }
    Ok(true)
}

/// **Takes the source `url` on `branch` out of the project's sources**, from whichever file lists
/// it. Skills installed from it stay, with their pins.
pub fn remove(cx: &mut Ctx<'_>, url: &str, branch: Option<&str>) -> Result<bool, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let Some(listed) = sources(plane)
        .into_iter()
        .find(|one| one.source.url == url && one.source.branch.as_deref() == branch)
    else {
        return Ok(false);
    };
    let which = if listed.local {
        crate::settings::Which::Local
    } else {
        crate::settings::Which::Shared
    };
    let take_out = |list: &mut toml_edit::ArrayOfTables| {
        let before = list.len();
        list.retain(|table| {
            let named = table.get("url").and_then(|v| v.as_str()) == Some(url)
                && table.get("branch").and_then(|v| v.as_str()) == branch;
            !named
        });
        list.len() != before
    };
    crate::settings::skill_sources::edit(plane, which, take_out)?;
    if !listed.local && listed.source.sign_in.is_some() {
        crate::settings::skill_sources::edit(plane, crate::settings::Which::Local, take_out)?;
    }
    cx.done(format!(
        "{} is no longer a skill source here",
        listed.source
    ));
    if !listed.local {
        cx.commit(&[which.file_at(plane).to_owned()], "skill source remove");
    }
    Ok(true)
}

/// `written` as a settings table.
fn table_of(written: &Written) -> toml_edit::Table {
    let mut table = toml_edit::Table::new();
    let mut put = |key: &str, value: &Option<String>| {
        if let Some(value) = value {
            table.insert(key, toml_edit::value(value.as_str()));
        }
    };
    put("url", &Some(written.url.clone()));
    put("branch", &written.branch);
    put("folder", &written.folder);
    put("vault", &written.vault);
    put("key", &written.key);
    put("user", &written.user);
    table
}

// ---------------------------------------------------------------------------------------------
// Fetching.
// ---------------------------------------------------------------------------------------------

/// What git is handed for a private source: its one credential helper, and the variables it
/// prints. Empty for a public one.
#[derive(Default)]
struct Handing {
    config: Vec<String>,
    pass: Vec<(String, std::ffi::OsString)>,
    /// The values, which nothing git says may carry.
    secrets: Vec<String>,
}

/// The one credential helper a private source's fetch runs: it answers `get` with the user name
/// and the token from [`USER_VAR`] and [`TOKEN_VAR`], only where git asks for the source's own
/// host ([`HOST_VAR`]) over https, and nothing else, so the token is in git's environment and the
/// helper's output, never on a command line, and never handed to a host a redirect names.
fn helper() -> String {
    format!(
        "credential.helper=!f() {{ test \"$1\" = get || exit 0; p=; h=; \
         while IFS= read -r l && test -n \"$l\"; do case \"$l\" in protocol=*) p=\"${{l#protocol=}}\";; \
         host=*) h=\"${{l#host=}}\";; esac; done; test \"$p\" = https && test -n \"$h\" && \
         test \"$h\" = \"${HOST_VAR}\" || exit 0; printf 'username=%s\\npassword=%s\\n' \
         \"${USER_VAR}\" \"${TOKEN_VAR}\"; }}; f"
    )
}

/// The host, and port where it names one, of an `https://` URL, lowercased: what git's
/// credential request names as `host`. `None` for any other URL.
fn https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    (!authority.is_empty()).then(|| authority.to_ascii_lowercase())
}

/// **Resolves the sign-in of `source`** through `secret exec`'s own steps: the vault is found as
/// `purlis secret exec` finds it, a sandboxed chat is refused the keyring as it is there, and the
/// read is recorded by the vault's and the entry's names, never the value.
fn handing(plane: &Path, source: &Source) -> Result<Handing, String> {
    use crate::secrets::{Ctx as Vaults, Env, brokered, cmd, exec};
    let Some(sign_in) = &source.sign_in else {
        return Ok(Handing::default());
    };
    let ctx = Vaults::new(plane, Env::from_process());
    let vault = cmd::provider(&ctx, &sign_in.vault).map_err(|e| e.message)?;
    if let Some(why) = brokered::refused_in_this_chat(&ctx, &vault, brokered::Reader::Exec) {
        return Err(why);
    }
    let request = exec::Request {
        vault: sign_in.vault.clone(),
        env: vec![format!("{TOKEN_VAR}={}", sign_in.key)],
        command: vec!["git".to_owned()],
        ..exec::Request::default()
    };
    let prepared = match exec::prepare(&ctx, &vault, &request, Vec::new(), None, &|| None) {
        Ok(prepared) => prepared,
        Err(exec::Stopped::Said(_, why)) => {
            return Err(format!(
                "{source} signs in with {} in vault '{}', and it could not be read: {why}",
                sign_in.key, sign_in.vault
            ));
        }
        Err(exec::Stopped::Signal(_)) => return Err("stopped".to_owned()),
    };
    exec::record(
        &ctx,
        &sign_in.vault,
        &prepared,
        "git",
        "skill-source",
        &[("source", serde_json::Value::String(source.url.clone()))],
    );
    let mut pass: Vec<(String, std::ffi::OsString)> = prepared
        .env
        .iter()
        .filter(|(name, _)| name == TOKEN_VAR)
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value.clone()))
        .collect();
    pass.push((
        HOST_VAR.to_owned(),
        https_host(&source.url).unwrap_or_default().into(),
    ));
    pass.push((
        USER_VAR.to_owned(),
        sign_in
            .user
            .clone()
            .unwrap_or_else(|| "x-access-token".to_owned())
            .into(),
    ));
    Ok(Handing {
        config: vec![helper()],
        pass,
        secrets: prepared.secret_values.clone(),
    })
}

/// A bare repository of its own in a new temporary folder, which a fetch lands in and which is
/// removed with it.
struct Scratch {
    _held: tempfile::TempDir,
    dir: PathBuf,
    git_dir: String,
}

impl Scratch {
    fn new() -> Result<Scratch, String> {
        let held = tempfile::Builder::new()
            .prefix("purlis-skill-source-")
            .tempdir()
            .map_err(|why| format!("a folder to fetch the source into could not be made: {why}"))?;
        let dir = held.path().to_path_buf();
        let made = git::run(
            &dir,
            &["init", "--bare", "--quiet", "--template=", "source.git"],
            git::READ,
        )
        .map_err(|why| why.to_string())?;
        if !made.ok() {
            return Err(format!(
                "a repository to fetch the source into could not be made: {}",
                made.err.trim()
            ));
        }
        let git_dir = format!("--git-dir={}", dir.join("source.git").display());
        Ok(Scratch {
            _held: held,
            dir,
            git_dir,
        })
    }

    /// git in the scratch repository, reaching nothing.
    fn git(&self, args: &[&str]) -> Result<git::Run, String> {
        let mut all = vec![self.git_dir.as_str()];
        all.extend_from_slice(args);
        git::run(&self.dir, &all, git::READ).map_err(|why| why.to_string())
    }

    /// git in the scratch repository, bytes back, `input` on its standard input.
    fn git_raw(&self, args: &[&str], input: Vec<u8>) -> Result<git::RawRun, String> {
        let mut all = vec![self.git_dir.as_str()];
        all.extend_from_slice(args);
        git::run_with_input(&self.dir, &all, input, git::READ).map_err(|why| why.to_string())
    }

    /// git across the network for `source`, with what `handing` hands it, git's words redacted.
    fn network(&self, handing: &Handing, args: &[&str]) -> Result<git::Run, String> {
        let mut all = vec![self.git_dir.as_str()];
        all.extend_from_slice(args);
        let config: Vec<String> = ["transfer.fsckObjects=true", "fetch.fsckObjects=true"]
            .iter()
            .map(|s| (*s).to_owned())
            .chain(handing.config.iter().cloned())
            .collect();
        let mut run = git::run_network_handing(&self.dir, &config, handing.pass.clone(), &all)
            .map_err(|why| why.to_string())?;
        run.err = crate::secrets::redact_str(&run.err, &handing.secrets);
        run.out = crate::secrets::redact_str(&run.out, &handing.secrets);
        Ok(run)
    }
}

/// Whether `text` is a commit's full name.
fn is_commit(text: &str) -> bool {
    matches!(text.len(), 40 | 64) && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The first line of git's complaint, for a sentence.
fn complaint(run: &git::Run) -> String {
    let line = run
        .err
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("git gave no reason");
    crate::shown::short(
        line.trim_start_matches("fatal: ")
            .trim_start_matches("error: "),
    )
}

/// **A source fetched at one commit**, read from its object store.
pub struct Fetched {
    scratch: Scratch,
    pub source: Source,
    pub commit: String,
    entries: Vec<Entry>,
    left: Vec<LeftOut>,
}

/// **Fetches `source`** into a scratch repository: the commit `at`, or its branch's newest where
/// `at` is `None` (or where the source will not hand a commit by its name and its branch is at
/// that commit anyway). Nothing is checked out, no submodule is fetched and no hook runs.
pub fn fetch(plane: &Path, source: &Source, at: Option<&str>) -> Result<Fetched, String> {
    source.check()?;
    if let Some(commit) = at
        && !is_commit(commit)
    {
        return Err(format!("{commit:?} is not a commit's name"));
    }
    let scratch = Scratch::new()?;
    let handing = handing(plane, source)?;
    let fetch = |want: &str| {
        let refspec = format!("+{want}:{FETCHED}");
        scratch.network(
            &handing,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                "--no-recurse-submodules",
                "--no-write-fetch-head",
                "--depth=1",
                "--",
                &source.url,
                &refspec,
            ],
        )
    };
    let mut run = fetch(at.unwrap_or(&source.tip_ref()))?;
    if !run.ok() && at.is_some() {
        run = fetch(&source.tip_ref())?;
    }
    if !run.ok() {
        return Err(format!(
            "{source} could not be fetched: {}",
            complaint(&run)
        ));
    }
    let resolved = scratch.git(&[
        "rev-parse",
        "--verify",
        "--end-of-options",
        "refs/purlis/fetched^{commit}",
    ])?;
    let commit = resolved.out.trim().to_owned();
    if !resolved.ok() || !is_commit(&commit) {
        return Err(format!("{source} answered no commit"));
    }
    if let Some(wanted) = at
        && !commit.eq_ignore_ascii_case(wanted)
    {
        return Err(format!(
            "{source} no longer offers the commit that was reviewed: review it again"
        ));
    }
    let listing = scratch.git_raw(
        &["ls-tree", "-r", "-z", "-l", "--full-tree", &commit],
        Vec::new(),
    )?;
    if listing.code != Some(0) {
        return Err(format!("{source}'s files could not be listed"));
    }
    let (entries, left) =
        tree::read_listing(&listing.out).map_err(|why| format!("{source}: {why}"))?;
    Ok(Fetched {
        scratch,
        source: source.clone(),
        commit,
        entries,
        left,
    })
}

/// **The newest commit `source` offers**, read from its refs alone: nothing is fetched.
pub fn newest(plane: &Path, source: &Source) -> Result<String, String> {
    source.check()?;
    let scratch = Scratch::new()?;
    let handing = handing(plane, source)?;
    let run = scratch.network(
        &handing,
        &["ls-remote", "--", &source.url, &source.tip_ref()],
    )?;
    if !run.ok() {
        return Err(format!("{source} could not be asked: {}", complaint(&run)));
    }
    run.out
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(_, name)| *name == source.tip_ref())
        .map(|(commit, _)| commit.trim().to_owned())
        .filter(|commit| is_commit(commit))
        .ok_or_else(|| format!("{source} has no {}", source.tip_ref()))
}

/// One file of a skill as the source holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceFile {
    parts: Vec<String>,
    executable: bool,
    bytes: Vec<u8>,
}

impl SourceFile {
    fn path(&self) -> String {
        self.parts.join("/")
    }
}

/// `sha256:` and the hex digest of `bytes`.
fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!(
        "sha256:{}",
        crate::extension::hex(&sha2::Sha256::digest(bytes))
    )
}

/// **The digest of a copy** whose files are `files`: over each file's path, mode and digest, in
/// path order, so the same bytes at the same paths are the same digest wherever they lie.
pub(crate) fn digest_of(files: &[lock::Wrote]) -> String {
    let mut sorted: Vec<&lock::Wrote> = files.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut all = String::new();
    for file in sorted {
        all.push_str(&format!(
            "{}\0{}\0{}\n",
            file.path,
            if file.executable { "755" } else { "644" },
            file.sha256
        ));
    }
    sha256(all.as_bytes())
}

/// What a lockfile records of `files`.
fn wrote(files: &[SourceFile]) -> Vec<lock::Wrote> {
    files
        .iter()
        .map(|file| lock::Wrote {
            path: file.path(),
            executable: file.executable,
            sha256: sha256(&file.bytes),
        })
        .collect()
}

/// One skill a fetched source holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    pub name: String,
    pub description: String,
    /// Its folder in the source, `/` between the parts.
    pub folder: String,
    #[serde(skip)]
    dir: Vec<String>,
}

impl Fetched {
    /// The blobs `oids` name, in their order, each as bytes.
    fn blobs(&self, oids: &[&str]) -> Result<Vec<Vec<u8>>, String> {
        if oids.is_empty() {
            return Ok(Vec::new());
        }
        let input: Vec<u8> = oids
            .iter()
            .flat_map(|oid| format!("{oid}\n").into_bytes())
            .collect();
        let run = self.scratch.git_raw(&["cat-file", "--batch"], input)?;
        if run.code != Some(0) {
            return Err(format!("{}'s files could not be read", self.source));
        }
        let mut out = Vec::with_capacity(oids.len());
        let mut rest: &[u8] = &run.out;
        for oid in oids {
            let end = rest
                .iter()
                .position(|b| *b == b'\n')
                .ok_or_else(|| format!("{}'s files could not be read", self.source))?;
            let header = String::from_utf8_lossy(&rest[..end]).into_owned();
            let mut fields = header.split(' ');
            let (Some(named), Some("blob"), Some(size)) =
                (fields.next(), fields.next(), fields.next())
            else {
                return Err(format!("{}'s file {oid} is not there", self.source));
            };
            let size: usize = size
                .parse()
                .map_err(|_| "a file's size could not be read".to_owned())?;
            if named != *oid || rest.len() < end + 1 + size + 1 {
                return Err(format!("{}'s files could not be read", self.source));
            }
            out.push(rest[end + 1..end + 1 + size].to_vec());
            rest = &rest[end + 1 + size + 1..];
        }
        Ok(out)
    }

    /// The files of the skill whose folder is `dir`, with their bytes.
    fn files_of(&self, dir: &[String]) -> Result<Vec<SourceFile>, String> {
        let listed = tree::files_of(&self.entries, dir);
        if listed.len() > tree::MOST_FILES {
            return Err(format!(
                "{} holds more than {} files, the most a skill purlis installs may",
                dir.join("/"),
                tree::MOST_FILES
            ));
        }
        let total: u64 = listed.iter().map(|(_, entry)| entry.size).sum();
        if total > MOST_SKILL_BYTES {
            return Err(format!(
                "{} holds more than 16 MiB, the most a skill purlis installs may",
                dir.join("/")
            ));
        }
        let oids: Vec<&str> = listed.iter().map(|(_, entry)| entry.oid.as_str()).collect();
        let bytes = self.blobs(&oids)?;
        Ok(listed
            .into_iter()
            .zip(bytes)
            .map(|((parts, entry), bytes)| SourceFile {
                parts,
                executable: entry.executable,
                bytes,
            })
            .collect())
    }

    /// **The skills the source holds** under its folder, by name, and each folder that has a
    /// `SKILL.md` and is not a whole skill, with why.
    pub fn skills(&self) -> (Vec<Found>, Vec<(String, String)>) {
        let folder = self.source.folder_parts().unwrap_or_default();
        let dirs = tree::skill_folders(&self.entries, &folder);
        let mut found = Vec::new();
        let mut not = Vec::new();
        let instructions: Vec<&str> = dirs
            .iter()
            .filter_map(|dir| {
                self.entries
                    .iter()
                    .find(|e| {
                        e.path.len() == dir.len() + 1
                            && e.path.starts_with(dir)
                            && e.path.last().is_some_and(|n| n == purlis_skills::FILE)
                    })
                    .map(|e| e.oid.as_str())
            })
            .collect();
        let texts = self.blobs(&instructions).unwrap_or_default();
        for (dir, bytes) in dirs.iter().zip(texts) {
            let shown = dir.join("/");
            // Read as the YAML it is, failing closed (ADR 0100 §3): a source's skill is offered
            // only where every reader reads the same name and description.
            let Ok(text) = String::from_utf8(bytes) else {
                not.push((
                    shown,
                    super::Reason::NotASkill(purlis_skills::Missing::File).to_string(),
                ));
                continue;
            };
            match purlis_skills::tree::plain_identity(&text) {
                Ok((name, description)) => match purlis_skills::change::name_problem(&name) {
                    None => found.push(Found {
                        name,
                        description,
                        folder: shown,
                        dir: dir.clone(),
                    }),
                    Some(why) => not.push((shown, format!("its name cannot be a skill's: {why}"))),
                },
                Err(why) => not.push((shown, why)),
            }
        }
        found.sort_by(|a, b| a.name.cmp(&b.name));
        // Two folders of one name: neither is installed, as which one a chat would get is
        // unclear.
        let names: Vec<String> = found.iter().map(|f| f.name.clone()).collect();
        found.retain(|f| names.iter().filter(|n| **n == f.name).count() == 1);
        for name in names
            .iter()
            .filter(|n| names.iter().filter(|m| m == n).count() > 1)
        {
            if !not.iter().any(|(folder, _)| folder == name) {
                not.push((
                    name.clone(),
                    "two folders of the source name this skill".to_owned(),
                ));
            }
        }
        (found, not)
    }
}

// ---------------------------------------------------------------------------------------------
// The review.
// ---------------------------------------------------------------------------------------------

/// One file of a skill, as the review shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewFile {
    pub path: String,
    pub size: u64,
    pub executable: bool,
    /// Its text, `None` for a file that is not text.
    pub text: Option<String>,
    /// Whether the summary marks something in it.
    pub risky: bool,
}

/// **One skill in an install review**: what it is, what it does beyond its instructions, every
/// file it installs, and every entry of its folder that is not installed and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkillReview {
    pub name: String,
    pub description: String,
    /// Its folder in the source.
    pub folder: String,
    /// The skill whose install brings it along, as one it refers to.
    pub brought_by: Option<String>,
    pub summary: Summary,
    pub files: Vec<ReviewFile>,
    pub left_out: Vec<LeftOut>,
    /// What the lint says of its `SKILL.md`, and each skill it refers to that the source does
    /// not hold.
    pub lint: Vec<String>,
    /// Its grade, where the source carries one. None does yet (#2779).
    pub grade: Option<String>,
    /// The digest of what it installs, which the install is handed back.
    pub digest: String,
}

/// **An install review**: the source, the commit it was read at, and each skill, those it refers
/// to after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Review {
    pub source: Source,
    pub commit: String,
    pub skills: Vec<SkillReview>,
}

/// **What an install or an update was accepted as**: the commit and each skill's digest, as the
/// review showed them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accepted {
    pub commit: String,
    pub skills: Vec<(String, String)>,
}

impl Review {
    /// What accepting it says.
    pub fn accepted(&self) -> Accepted {
        Accepted {
            commit: self.commit.clone(),
            skills: self
                .skills
                .iter()
                .map(|skill| (skill.name.clone(), skill.digest.clone()))
                .collect(),
        }
    }
}

/// The texts of `files`, `None` for one that is not text.
fn texts_of(files: &[SourceFile]) -> Vec<Option<String>> {
    files
        .iter()
        .map(|file| {
            String::from_utf8(file.bytes.clone())
                .ok()
                .filter(|text| !text.contains('\0'))
        })
        .collect()
}

/// The review of the skill `found`, whose files are `files` and whose left-out entries are
/// `left`, brought along by `brought_by`, in a source that holds the skills `held`.
fn review_of(
    found: &Found,
    files: &[SourceFile],
    left: Vec<LeftOut>,
    brought_by: Option<String>,
    held: &[String],
) -> SkillReview {
    let texts = texts_of(files);
    let summary = summary_of(files);
    let marked = |path: &str| {
        summary
            .scripts
            .iter()
            .any(|s| s.path == path && !s.risks.is_empty())
            || summary.commands.iter().any(|c| c.path == path)
            || summary.outside.iter().any(|o| o.path == path)
            || summary
                .hosts
                .iter()
                .any(|h| h.risky && h.files.iter().any(|f| f == path))
    };
    let mut lint: Vec<String> = files
        .iter()
        .zip(&texts)
        .find(|(file, _)| file.parts == [purlis_skills::FILE])
        .and_then(|(_, text)| text.as_deref())
        .map(|text| {
            purlis_skills::change::check(text, &found.name)
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    for name in referenced(files, &texts) {
        if name != found.name && !held.contains(&name) {
            lint.push(format!(
                "it refers to the skill {name}, which this source does not hold"
            ));
        }
    }
    SkillReview {
        name: found.name.clone(),
        description: found.description.clone(),
        folder: found.folder.clone(),
        brought_by,
        files: files
            .iter()
            .zip(texts)
            .map(|(file, text)| ReviewFile {
                path: file.path(),
                size: file.bytes.len() as u64,
                executable: file.executable,
                risky: marked(&file.path()) || file.executable,
                text,
            })
            .collect(),
        left_out: left,
        lint,
        grade: None,
        digest: digest_of(&wrote(files)),
        summary,
    }
}

/// The skills `files` refer to (`skills:<name>`, ADR 0099), by name.
fn referenced(files: &[SourceFile], texts: &[Option<String>]) -> Vec<String> {
    let needle = format!("{}:", super::LINKED_PLUGIN);
    let mut out: Vec<String> = Vec::new();
    for (_, text) in files.iter().zip(texts) {
        let Some(text) = text else { continue };
        for (at, _) in text.match_indices(&needle) {
            let before = text[..at].chars().next_back();
            if before.is_some_and(|c| c.is_ascii_alphanumeric() || "_.-".contains(c)) {
                continue;
            }
            let name: String = text[at + needle.len()..]
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
                .collect();
            if !name.is_empty()
                && purlis_skills::change::name_problem(&name).is_none()
                && !out.contains(&name)
            {
                out.push(name);
            }
        }
    }
    out
}

impl Fetched {
    /// **The review of installing `names`** from this commit, and every skill they refer to that
    /// the source holds, each after the one that brought it.
    pub fn review(&self, names: &[String]) -> Result<Review, String> {
        let (found, _) = self.skills();
        let held: Vec<String> = found.iter().map(|f| f.name.clone()).collect();
        let mut queue: Vec<(String, Option<String>)> =
            names.iter().map(|name| (name.clone(), None)).collect();
        let mut skills: Vec<SkillReview> = Vec::new();
        while !queue.is_empty() {
            let (name, by) = queue.remove(0);
            if skills.iter().any(|s| s.name == name) {
                continue;
            }
            let Some(one) = found.iter().find(|f| f.name == name) else {
                return Err(format!("{} holds no skill named {name:?}", self.source));
            };
            let files = self.files_of(&one.dir)?;
            // Each skill it refers to is reviewed next, so one that was also named is still
            // marked as brought along, and an install's second review reads the same.
            let mut next: Vec<(String, Option<String>)> = referenced(&files, &texts_of(&files))
                .into_iter()
                .filter(|other| *other != name && held.contains(other))
                .map(|other| (other, Some(name.clone())))
                .collect();
            next.append(&mut queue);
            queue = next;
            let left = tree::left_in(&self.left, &one.dir);
            skills.push(review_of(one, &files, left, by, &held));
        }
        Ok(Review {
            source: self.source.clone(),
            commit: self.commit.clone(),
            skills,
        })
    }
}

/// **The review of installing `names` from `source`** at its newest commit.
pub fn review(plane: &Path, source: &Source, names: &[String]) -> Result<Review, String> {
    fetch(plane, source, None)?.review(names)
}

/// One skill a source offers, as the studio lists it to browse: and where it is installed here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Offered {
    #[serde(flatten)]
    pub skill: Found,
    /// The levels it is installed at from this source.
    pub installed: Vec<Level>,
}

/// **What a source offers**: its newest commit, its skills, and each folder that is not a whole
/// skill with why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Browse {
    pub commit: String,
    pub skills: Vec<Offered>,
    pub not_skills: Vec<(String, String)>,
}

/// **Browses `source`** at its newest commit.
pub fn browse(plane: &Path, source: &Source) -> Result<Browse, String> {
    let fetched = fetch(plane, source, None)?;
    let (found, not_skills) = fetched.skills();
    let (locked, _) = lock::every(plane);
    Ok(Browse {
        commit: fetched.commit.clone(),
        skills: found
            .into_iter()
            .map(|skill| Offered {
                installed: locked
                    .iter()
                    .filter(|one| one.name == skill.name && source.made(&one.source))
                    .map(|one| one.level.0.clone())
                    .collect(),
                skill,
            })
            .collect(),
        not_skills,
    })
}

// ---------------------------------------------------------------------------------------------
// Installing.
// ---------------------------------------------------------------------------------------------

/// The parts of `level`'s folder in the plane.
fn level_parts(level: &Level) -> Result<Vec<String>, String> {
    level
        .rel()
        .map(|rel| rel.split('/').map(str::to_owned).collect())
        .ok_or_else(|| format!("{level} is no level of this project"))
}

/// Refused where what `fetched` reviews now is not what was `accepted`.
fn as_accepted(review: &Review, accepted: &Accepted) -> Result<(), String> {
    let now = review.accepted();
    let mut a = now.skills.clone();
    let mut b = accepted.skills.clone();
    a.sort();
    b.sort();
    if now.commit != accepted.commit || a != b {
        return Err(
            "what the source holds is not what was reviewed, so nothing was installed: review it \
             again"
                .to_owned(),
        );
    }
    Ok(())
}

/// **Installs what `accepted` says from `source` at `level`** (ADR 0100 §2): fetches the
/// reviewed commit again, checks each skill's files are the reviewed ones, copies each into
/// `level` under its own name, records it in the level's lockfile, and commits the copies and the
/// lockfile as one change. A project skill lands granted to the root unless `grant` says
/// otherwise. Answers each skill's folder.
pub fn install(
    cx: &mut Ctx<'_>,
    source: &Source,
    level: &Level,
    accepted: &Accepted,
    grant: Option<&Grant>,
) -> Result<Vec<PathBuf>, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    verbs::level_is_there(plane, level)?;
    let names: Vec<String> = accepted
        .skills
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    if names.is_empty() {
        return Err("no skill was named to install".to_owned());
    }
    let fetched = fetch(plane, source, Some(&accepted.commit))?;
    let review = fetched.review(&names)?;
    as_accepted(&review, accepted)?;
    let lock_file = lock::file_for(level);
    let held = lock::read(plane, lock_file)?;
    for skill in &review.skills {
        cx.not_built_in(&skill.name)?;
        verbs::free_at(plane, level, &skill.name, &skill.name)?;
        if held
            .lock
            .skills
            .iter()
            .any(|one| one.is(level, &skill.name))
        {
            return Err(format!(
                "the {level} skills already have {} installed: update it instead",
                skill.name
            ));
        }
    }
    let parts = level_parts(level)?;
    let (found, _) = fetched.skills();
    let mut made: Vec<PathBuf> = Vec::new();
    let mut lock = held.lock.clone();
    for skill in &review.skills {
        let one = found
            .iter()
            .find(|f| f.name == skill.name)
            .ok_or_else(|| format!("{} is no longer in the source", skill.name))?;
        let files = fetched.files_of(&one.dir)?;
        let writes: Vec<write::File<'_>> = files
            .iter()
            .map(|file| write::File {
                parts: &file.parts,
                executable: file.executable,
                bytes: &file.bytes,
            })
            .collect();
        if let Err(why) = write::new_skill(plane, &parts, level.depth(), &skill.name, &writes) {
            for folder in &made {
                cx.warn(format!(
                    "{} was installed before the next one failed, and is left for you to keep or \
                     delete",
                    verbs::rel(plane, folder)
                ));
            }
            return Err(format!("{} could not be installed: {why}", skill.name));
        }
        let folder = plane.join(parts.join("/")).join(&skill.name);
        let files_wrote = wrote(&files);
        lock.skills.push(Locked {
            name: skill.name.clone(),
            level: lock::AtLevel(level.clone()),
            source: lock::From {
                url: source.url.clone(),
                branch: source.branch.clone(),
                folder: one.folder.clone(),
            },
            commit: review.commit.clone(),
            digest: digest_of(&files_wrote),
            files: files_wrote,
            brought_by: skill.brought_by.clone(),
        });
        made.push(folder);
    }
    lock::write(plane, lock_file, held.was.as_deref(), &lock)?;
    let mut paths: Vec<String> = made
        .iter()
        .map(|folder| verbs::rel(plane, folder))
        .collect();
    if level.at == verbs::At::Project
        && !level.local
        && let Some(grant) = grant.filter(|g| **g != Grant::Root)
    {
        for skill in &review.skills {
            let folder = super::grant(plane, &skill.name, grant)?;
            paths.push(verbs::rel(plane, &super::beside(&folder)));
        }
    }
    let shown: Vec<&str> = review.skills.iter().map(|s| s.name.as_str()).collect();
    cx.done(format!(
        "{} installed at {level}, pinned to {}",
        shown.join(", "),
        short(&review.commit)
    ));
    if !level.local {
        paths.push(lock::FILE.to_owned());
        cx.commit(&paths, &format!("skill install {}", shown.join(" ")));
    }
    Ok(made)
}

/// A commit's short name, as a sentence shows it.
pub fn short(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}

// ---------------------------------------------------------------------------------------------
// Updates.
// ---------------------------------------------------------------------------------------------

/// **A newer commit for an installed skill**, offered, never applied by itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Offer {
    pub name: String,
    pub level: Level,
    pub url: String,
    pub pinned: String,
    pub newer: String,
    /// Whether the copy was edited here since it was installed.
    pub edited: bool,
}

/// What an installed skill could not be asked about, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unasked {
    pub name: String,
    pub level: Level,
    pub why: String,
}

/// The source `locked` was installed from, with the sign-in the project lists it with.
fn source_of(plane: &Path, locked: &Locked) -> Source {
    let listed = listed_as(plane, &locked.source.url, locked.source.branch.as_deref());
    Source {
        url: locked.source.url.clone(),
        branch: locked.source.branch.clone(),
        folder: None,
        sign_in: listed.and_then(|one| one.sign_in),
    }
}

/// The files of the copy `locked` installed that differ from what it installed, or `None` where
/// the copy cannot be read.
fn edited(plane: &Path, locked: &Locked) -> Option<Vec<String>> {
    let mut parts = level_parts(&locked.level.0).ok()?;
    parts.push(locked.name.clone());
    let now = write::read_all(plane, &parts, tree::MOST_FILE_BYTES, tree::MOST_FILES)?;
    let mut changed = Vec::new();
    for (path, executable, bytes) in &now {
        let shown = path.join("/");
        match locked.files.iter().find(|f| f.path == shown) {
            Some(was) if was.sha256 == sha256(bytes) && was.executable == *executable => {}
            _ => changed.push(shown),
        }
    }
    for was in &locked.files {
        if !now.iter().any(|(path, _, _)| path.join("/") == was.path) {
            changed.push(was.path.clone());
        }
    }
    changed.sort();
    Some(changed)
}

/// A source by its URL and branch, as the lockfile names one.
type SourceKey = (String, Option<String>);

/// **Asks each source of an installed skill for its newest commit** — its refs alone, nothing
/// fetched or written — and answers each skill whose pin is older, and each that could not be
/// asked.
pub fn updates(plane: &Path) -> (Vec<Offer>, Vec<Unasked>) {
    let (locked, _) = lock::every(plane);
    let mut asked: Vec<(SourceKey, Result<String, String>)> = Vec::new();
    let mut offers = Vec::new();
    let mut unasked = Vec::new();
    for one in &locked {
        let key = (one.source.url.clone(), one.source.branch.clone());
        let newest = match asked.iter().find(|(k, _)| *k == key) {
            Some((_, answer)) => answer.clone(),
            None => {
                let answer = newest(plane, &source_of(plane, one));
                asked.push((key, answer.clone()));
                answer
            }
        };
        match newest {
            Ok(commit) if commit != one.commit => offers.push(Offer {
                name: one.name.clone(),
                level: one.level.0.clone(),
                url: one.source.url.clone(),
                pinned: one.commit.clone(),
                newer: commit,
                edited: edited(plane, one).is_none_or(|changed| !changed.is_empty()),
            }),
            Ok(_) => {}
            Err(why) => unasked.push(Unasked {
                name: one.name.clone(),
                level: one.level.0.clone(),
                why,
            }),
        }
    }
    (offers, unasked)
}

/// How a file changes in an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Changed {
    Added,
    Removed,
    Changed,
}

/// One file's change in an update: its lines' diff, empty for a file that is not text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileChange {
    pub path: String,
    pub change: Changed,
    pub diff: String,
}

/// **An update's review** (ADR 0100 §4): the summary's changes first, then each file's, from the
/// pinned commit to the newer one; the files edited here since the install, which accepting
/// replaces; and the newer commit's own review, every file one click away.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateReview {
    pub name: String,
    pub level: Level,
    pub pinned: String,
    pub newer: String,
    /// Whether the pinned commit could be read from the source; where not, the diff is from the
    /// copy installed here.
    pub from_pin: bool,
    pub changes: Changes,
    pub files: Vec<FileChange>,
    pub edited: Vec<String>,
    pub review: SkillReview,
}

impl UpdateReview {
    /// What accepting it says.
    pub fn accepted(&self) -> Accepted {
        Accepted {
            commit: self.newer.clone(),
            skills: vec![(self.name.clone(), self.review.digest.clone())],
        }
    }
}

/// The installed skill `name` at `level`, as its lockfile records it.
fn locked_at(plane: &Path, level: &Level, name: &str) -> Result<(lock::Read, usize), String> {
    let held = lock::read(plane, lock::file_for(level))?;
    let at = held
        .lock
        .skills
        .iter()
        .position(|one| one.is(level, name))
        .ok_or_else(|| format!("no {level} skill named {name} was installed from a source"))?;
    Ok((held, at))
}

/// The files `fetched` holds at the folder `folder` (`/` between its parts) of the commit at
/// `at`, read with the review's own rules.
fn files_at(fetched: &Fetched, at: &str, folder: &str) -> Result<Vec<SourceFile>, String> {
    let listing = fetched.scratch.git_raw(
        &["ls-tree", "-r", "-z", "-l", "--full-tree", at],
        Vec::new(),
    )?;
    if listing.code != Some(0) {
        return Err("the pinned commit's files could not be listed".to_owned());
    }
    let (entries, _) = tree::read_listing(&listing.out)?;
    let dir = tree::folder_parts(folder).ok_or_else(|| format!("{folder:?} is no folder"))?;
    let listed = tree::files_of(&entries, &dir);
    let oids: Vec<&str> = listed.iter().map(|(_, e)| e.oid.as_str()).collect();
    let bytes = fetched.blobs(&oids)?;
    Ok(listed
        .into_iter()
        .zip(bytes)
        .map(|((parts, entry), bytes)| SourceFile {
            parts,
            executable: entry.executable,
            bytes,
        })
        .collect())
}

/// The summary of `files`.
fn summary_of(files: &[SourceFile]) -> Summary {
    let texts = texts_of(files);
    let paths: Vec<String> = files.iter().map(SourceFile::path).collect();
    let read: Vec<FileIn<'_>> = files
        .iter()
        .zip(&texts)
        .zip(&paths)
        .map(|((file, text), path)| FileIn {
            path,
            executable: file.executable,
            text: text.as_deref(),
        })
        .collect();
    review::summarize(&read)
}

/// Each file's change from `was` to `now`.
fn file_changes(was: &[SourceFile], now: &[SourceFile]) -> Vec<FileChange> {
    let text = |file: &SourceFile| String::from_utf8(file.bytes.clone()).ok();
    let mut out = Vec::new();
    for file in now {
        match was.iter().find(|old| old.parts == file.parts) {
            None => out.push(FileChange {
                path: file.path(),
                change: Changed::Added,
                diff: text(file)
                    .map(|t| review::line_diff("", &t))
                    .unwrap_or_default(),
            }),
            Some(old) if old.bytes != file.bytes || old.executable != file.executable => {
                out.push(FileChange {
                    path: file.path(),
                    change: Changed::Changed,
                    diff: match (text(old), text(file)) {
                        (Some(a), Some(b)) => review::line_diff(&a, &b),
                        _ => String::new(),
                    },
                });
            }
            Some(_) => {}
        }
    }
    for old in was {
        if !now.iter().any(|file| file.parts == old.parts) {
            out.push(FileChange {
                path: old.path(),
                change: Changed::Removed,
                diff: text(old)
                    .map(|t| review::line_diff(&t, ""))
                    .unwrap_or_default(),
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// **The review of updating the installed skill `name` at `level`** to its source's newest
/// commit. Refused where the pin is the newest.
pub fn update_review(plane: &Path, level: &Level, name: &str) -> Result<UpdateReview, String> {
    let (held, at) = locked_at(plane, level, name)?;
    let locked = &held.lock.skills[at];
    let source = source_of(plane, locked);
    let fetched = fetch(plane, &source, None)?;
    if fetched.commit == locked.commit {
        return Err(format!(
            "{name} is pinned to its source's newest commit already"
        ));
    }
    let dir = tree::folder_parts(&locked.source.folder)
        .ok_or_else(|| format!("{:?} is no folder", locked.source.folder))?;
    let (found, _) = fetched.skills();
    let one = found.iter().find(|f| f.dir == dir).ok_or_else(|| {
        format!(
            "the source no longer holds {name} at {}",
            locked.source.folder
        )
    })?;
    if one.name != name {
        return Err(format!(
            "the source renamed {name} to {}: install it under its new name",
            one.name
        ));
    }
    let now = fetched.files_of(&one.dir)?;
    // The pinned commit, fetched beside the newer one where the source hands it by its name.
    let pinned_spec = format!("+{}:{PINNED}", locked.commit);
    let handing = handing(plane, &source)?;
    let pinned = fetched
        .scratch
        .network(
            &handing,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                "--no-recurse-submodules",
                "--no-write-fetch-head",
                "--depth=1",
                "--",
                &source.url,
                &pinned_spec,
            ],
        )
        .ok()
        .filter(git::Run::ok)
        .and_then(|_| files_at(&fetched, &locked.commit, &locked.source.folder).ok());
    let from_pin = pinned.is_some();
    let was = match pinned {
        Some(files) => files,
        None => {
            let mut parts = level_parts(level)?;
            parts.push(name.to_owned());
            write::read_all(plane, &parts, tree::MOST_FILE_BYTES, tree::MOST_FILES)
                .ok_or_else(|| format!("{name}'s copy here could not be read"))?
                .into_iter()
                .map(|(parts, executable, bytes)| SourceFile {
                    parts,
                    executable,
                    bytes,
                })
                .collect()
        }
    };
    let held_names: Vec<String> = found.iter().map(|f| f.name.clone()).collect();
    let review = review_of(
        one,
        &now,
        tree::left_in(&fetched.left, &one.dir),
        None,
        &held_names,
    );
    Ok(UpdateReview {
        name: name.to_owned(),
        level: level.clone(),
        pinned: locked.commit.clone(),
        newer: fetched.commit.clone(),
        from_pin,
        changes: review::changes(&summary_of(&was), &review.summary),
        files: file_changes(&was, &now),
        edited: edited(plane, locked).unwrap_or_default(),
        review,
    })
}

/// **Accepts an update** of the installed skill `name` at `level` to what `accepted` says: the
/// newer commit fetched again and checked against the reviewed digest, each of its files written
/// over the copy, each file the old commit had and the newer one does not removed, and the pin
/// moved, as one commit.
pub fn update(
    cx: &mut Ctx<'_>,
    level: &Level,
    name: &str,
    accepted: &Accepted,
) -> Result<PathBuf, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let (held, at) = locked_at(plane, level, name)?;
    let locked = held.lock.skills[at].clone();
    let source = source_of(plane, &locked);
    let fetched = fetch(plane, &source, Some(&accepted.commit))?;
    let dir = tree::folder_parts(&locked.source.folder)
        .ok_or_else(|| format!("{:?} is no folder", locked.source.folder))?;
    let (found, _) = fetched.skills();
    let one = found
        .iter()
        .find(|f| f.dir == dir && f.name == name)
        .ok_or_else(|| {
            format!(
                "the source no longer holds {name} at {}",
                locked.source.folder
            )
        })?;
    let files = fetched.files_of(&one.dir)?;
    let files_wrote = wrote(&files);
    let digest = digest_of(&files_wrote);
    if accepted.skills != [(name.to_owned(), digest.clone())] {
        return Err(
            "what the source holds is not what was reviewed, so nothing was updated: review it \
             again"
                .to_owned(),
        );
    }
    let mut parts = level_parts(level)?;
    parts.push(name.to_owned());
    let gone: Vec<Vec<String>> = locked
        .files
        .iter()
        .filter(|was| !files_wrote.iter().any(|now| now.path == was.path))
        .filter_map(|was| tree::folder_parts(&was.path))
        .collect();
    let writes: Vec<write::File<'_>> = files
        .iter()
        .map(|file| write::File {
            parts: &file.parts,
            executable: file.executable,
            bytes: &file.bytes,
        })
        .collect();
    write::replace(plane, &parts, &writes, &gone)
        .map_err(|why| format!("{name} could not be updated: {why}"))?;
    let mut lock = held.lock.clone();
    let entry = &mut lock.skills[at];
    entry.commit = fetched.commit.clone();
    entry.digest = digest;
    entry.files = files_wrote;
    lock::write(plane, lock::file_for(level), held.was.as_deref(), &lock)?;
    let folder = plane.join(parts.join("/"));
    cx.done(format!(
        "{name} is updated at {level}, pinned to {}",
        short(&fetched.commit)
    ));
    if !level.local {
        cx.commit(
            &[verbs::rel(plane, &folder), lock::FILE.to_owned()],
            &format!("skill update {name}"),
        );
    }
    Ok(folder)
}

#[cfg(test)]
mod tests;
