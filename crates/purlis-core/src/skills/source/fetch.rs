// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A source fetched at one commit**, read from git's object store and never checked out
//! (ADR 0100 §2, #2773).
//!
//! It is fetched into a bare repository of its own in a new temporary folder by purlis's hardened
//! git: no hooks, no submodules, every object checked by `transfer.fsckObjects`, one commit deep,
//! and every blob past [`tree::MOST_FILE_BYTES`] left on the server (`--filter=blob:limit`, the
//! scratch repository told it is a partial clone on the command line alone, so nothing it lacks is
//! ever fetched later). Its tree is read with `ls-tree` and `cat-file` ([`purlis_skills::tree`]).
//!
//! **Every read has a limit, and fails closed past it with a sentence**: the bytes the fetch may
//! land ([`MOST_FETCH_BYTES`]), the entries a tree may list ([`tree::MOST_ENTRIES`]), the skills a
//! source may hold ([`tree::MOST_SKILLS`]), the bytes one skill may hold ([`MOST_SKILL_BYTES`]),
//! and the bytes all the reads of one fetch may buffer ([`MOST_READ_BYTES`]).
//!
//! **A private source signs in with a vault entry** (ADR 0100 §5, as amended 2026-10-23): resolved
//! through `purlis secret exec`'s own steps ([`crate::secrets::exec`]: the read is recorded by
//! name, and a sandboxed chat never reads the keyring), handed to purlis's git as one variable its
//! one credential helper prints for the source's own host over https alone, never on a command
//! line, and taken out of anything git says before it is said.

use std::cell::Cell;
use std::path::{Path, PathBuf};

use purlis_skills::tree::{self, Entry, LeftOut};
use serde::Serialize;

use super::{Commit, Remote, Source};
use crate::worktree::git;

/// The variable a private source's credential is handed to git in.
pub(super) const TOKEN_VAR: &str = "PURLIS_SKILL_SOURCE_TOKEN";

/// The variable the user name for it is handed in.
pub(super) const USER_VAR: &str = "PURLIS_SKILL_SOURCE_USER";

/// The variable the source's host is handed in: the only host the credential is answered for.
pub(super) const HOST_VAR: &str = "PURLIS_SKILL_SOURCE_HOST";

/// The ref a fetch lands at in the scratch repository.
const FETCHED: &str = "refs/purlis/fetched";

/// The name the scratch repository knows the source by, on the command line alone.
const REMOTE: &str = "source";

/// The most bytes one skill's files may hold in all.
pub const MOST_SKILL_BYTES: u64 = 16 * 1024 * 1024;

/// The most bytes all the reads of one fetched source may buffer.
pub const MOST_READ_BYTES: u64 = 4 * MOST_SKILL_BYTES;

/// The most bytes one fetch may land.
pub const MOST_FETCH_BYTES: u64 = 16 * MOST_SKILL_BYTES;

/// What git is handed for a private source: its one credential helper, and the variables it
/// prints. Empty for a public one.
#[derive(Default)]
pub(super) struct Handing {
    pub(super) config: Vec<String>,
    pub(super) pass: Vec<(String, std::ffi::OsString)>,
    /// The values, which nothing git says may carry.
    pub(super) secrets: Vec<String>,
}

/// The one credential helper a private source's fetch runs: it answers `get` with the user name
/// and the token from [`USER_VAR`] and [`TOKEN_VAR`], only where git asks for the source's own
/// host ([`HOST_VAR`]) over https, and nothing else, so the token is in git's environment and the
/// helper's output, never on a command line, and never handed to a host a redirect names.
pub(super) fn helper() -> String {
    format!(
        "credential.helper=!f() {{ test \"$1\" = get || exit 0; p=; h=; \
         while IFS= read -r l && test -n \"$l\"; do case \"$l\" in protocol=*) p=\"${{l#protocol=}}\";; \
         host=*) h=\"${{l#host=}}\";; esac; done; test \"$p\" = https && test -n \"$h\" && \
         test \"$h\" = \"${HOST_VAR}\" || exit 0; printf 'username=%s\\npassword=%s\\n' \
         \"${USER_VAR}\" \"${TOKEN_VAR}\"; }}; f"
    )
}

/// **Resolves the sign-in of `source`** through `secret exec`'s own steps: the vault is found as
/// `purlis secret exec` finds it, a sandboxed chat is refused the keyring as it is there, and the
/// read is recorded by the vault's and the entry's names, never the value. A source that is not an
/// `https://` URL with a plain host is handed nothing.
pub(super) fn handing(plane: &Path, source: &Source) -> Result<Handing, String> {
    use crate::secrets::{Ctx as Vaults, Env, brokered, cmd, exec};
    let Some(sign_in) = &source.sign_in else {
        return Ok(Handing::default());
    };
    let host = crate::savefix::https_host(&source.remote.url).ok_or_else(|| {
        format!("{source} signs in, and only an https:// source with a plain host may")
    })?;
    let ctx = Vaults::new(plane, Env::from_process());
    let vault = cmd::provider(&ctx, &sign_in.vault).map_err(|e| e.message.to_string())?;
    if let Some(why) = brokered::refused_in_this_chat(&ctx, &vault, brokered::Reader::Exec) {
        return Err(why.to_string());
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
        &[(
            "source",
            serde_json::Value::String(source.remote.url.clone()),
        )],
    );
    let mut pass: Vec<(String, std::ffi::OsString)> = prepared
        .env
        .iter()
        .filter(|(name, _)| name == TOKEN_VAR)
        .map(|(name, value)| (name.to_string_lossy().into_owned(), value.clone()))
        .collect();
    pass.push((HOST_VAR.to_owned(), host.into()));
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
pub(super) struct Scratch {
    _held: tempfile::TempDir,
    dir: PathBuf,
    git_dir: String,
}

impl Scratch {
    pub(super) fn new() -> Result<Scratch, String> {
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

    /// git in the scratch repository, its `-z` output read as it comes and held to
    /// `most_records` records: `None` past them, git stopped.
    fn git_capped(
        &self,
        args: &[&str],
        most_records: usize,
    ) -> Result<Option<git::RawRun>, String> {
        let mut all = vec![self.git_dir.as_str()];
        all.extend_from_slice(args);
        git::run_capped(&self.dir, &all, git::READ, most_records).map_err(|why| why.to_string())
    }

    /// git across the network for `remote`, with what `handing` hands it, git's words redacted.
    /// The source is named [`REMOTE`] on the command line, a promisor of a partial clone only for
    /// this call, so a blob the filter left out is never fetched by a later read.
    pub(super) fn network(
        &self,
        remote: &Remote,
        handing: &Handing,
        args: &[&str],
    ) -> Result<git::Run, String> {
        let mut all = vec![self.git_dir.as_str()];
        all.extend_from_slice(args);
        let filter = format!("blob:limit={}", tree::MOST_FILE_BYTES);
        let config: Vec<String> = [
            "transfer.fsckObjects=true".to_owned(),
            "fetch.fsckObjects=true".to_owned(),
            format!("remote.{REMOTE}.url={}", remote.url),
            format!("remote.{REMOTE}.promisor=true"),
            format!("remote.{REMOTE}.partialclonefilter={filter}"),
            format!("extensions.partialClone={REMOTE}"),
        ]
        .into_iter()
        .chain(handing.config.iter().cloned())
        .collect();
        let mut run = git::run_network_handing(&self.dir, &config, handing.pass.clone(), &all)
            .map_err(|why| why.to_string())?;
        run.err = crate::secrets::redact_str(&run.err, &handing.secrets);
        run.out = crate::secrets::redact_str(&run.out, &handing.secrets);
        Ok(run)
    }

    /// **Fetches `want` from `remote`** to `to`, one commit deep, blobs past the file limit left
    /// on the server, then holds what landed to [`MOST_FETCH_BYTES`].
    pub(super) fn fetch(
        &self,
        remote: &Remote,
        handing: &Handing,
        want: &str,
        to: &str,
    ) -> Result<git::Run, String> {
        let refspec = format!("+{want}:{to}");
        let filter = format!("--filter=blob:limit={}", tree::MOST_FILE_BYTES);
        let run = self.network(
            remote,
            handing,
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                "--no-recurse-submodules",
                "--no-write-fetch-head",
                "--depth=1",
                &filter,
                "--",
                REMOTE,
                &refspec,
            ],
        )?;
        if bytes_under(&self.dir.join("source.git").join("objects")) > MOST_FETCH_BYTES {
            return Err(format!(
                "{remote} is larger than {} MiB at one commit, the most purlis fetches of a skill \
                 source",
                MOST_FETCH_BYTES >> 20
            ));
        }
        Ok(run)
    }
}

/// How many bytes the plain files under `dir` hold, following no link.
fn bytes_under(dir: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            if meta.is_dir() {
                stack.push(entry.path());
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

/// **The name and description of a fetched `SKILL.md`**, where every reader reads the same ones:
/// its frontmatter read as the YAML it is by the one reader ([`purlis_skills::handed::parse`],
/// failing closed), its `name` and `description` single scalars, and purlis's own line reader
/// ([`purlis_skills::parsed`]) reading them alike. Anything else is refused in a sentence: a
/// source's skill is installed only where the name a harness loads is the name the review showed.
pub(super) fn identity(text: &str) -> Result<(String, String), String> {
    use purlis_skills::Field;
    let read = purlis_skills::handed::parse(text).map_err(|why| why.to_string())?;
    let (Some(name), Some(description)) =
        (read.scalar(Field::Name), read.scalar(Field::Description))
    else {
        return Err("its frontmatter names no name or no description".to_owned());
    };
    let lines = purlis_skills::parsed(text, PathBuf::new())
        .map_err(|_| "its frontmatter is not one purlis reads".to_owned())?;
    if lines.name != name || lines.description != description {
        return Err("its frontmatter reads one way as YAML and another line by line".to_owned());
    }
    Ok((name.to_owned(), description.to_owned()))
}

/// The first line of git's complaint, for a sentence.
pub(super) fn complaint(run: &git::Run) -> String {
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

/// One file of a skill as the source holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFile {
    pub(super) parts: Vec<String>,
    pub(super) executable: bool,
    pub(super) bytes: Vec<u8>,
}

impl SourceFile {
    /// Its path inside the skill's folder, `/` between the parts.
    pub fn path(&self) -> String {
        self.parts.join("/")
    }
}

/// A folder of a source that is not a skill purlis offers, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotASkill {
    pub folder: String,
    pub why: String,
}

/// One skill a fetched source holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Found {
    pub name: String,
    pub description: String,
    /// Its folder in the source, `/` between the parts.
    pub folder: String,
    #[serde(skip)]
    pub(super) dir: Vec<String>,
}

/// **A source fetched at one commit**, read from its object store.
pub struct Fetched {
    pub(super) scratch: Scratch,
    pub source: Source,
    pub commit: Commit,
    pub(super) entries: Vec<Entry>,
    pub(super) left: Vec<LeftOut>,
    /// The bytes read from it so far, held to [`MOST_READ_BYTES`].
    read: Cell<u64>,
}

/// The files and left-out entries of the commit at `at` in `scratch`, sizes asked of git's own
/// store: a blob the fetch left on the server is past the file limit.
fn listed(
    scratch: &Scratch,
    remote: &Remote,
    at: &Commit,
) -> Result<(Vec<Entry>, Vec<LeftOut>), String> {
    // Read as it comes and stopped at the entry cap: a tree of a few kilobytes can list more
    // paths than memory holds.
    let listing = scratch
        .git_capped(
            &["ls-tree", "-r", "-z", "--full-tree", at.as_str()],
            tree::MOST_ENTRIES,
        )?
        .ok_or_else(|| format!("{remote}: {}", tree::Unread::TooManyEntries))?;
    if listing.code != Some(0) {
        return Err(format!("{remote}'s files could not be listed"));
    }
    let records = tree::records(&listing.out).map_err(|why| format!("{remote}: {why}"))?;
    let blobs: Vec<&str> = records
        .iter()
        .filter(|one| matches!(one.mode, tree::Mode::File | tree::Mode::Executable))
        .map(|one| one.oid.as_str())
        .collect();
    let input: Vec<u8> = blobs
        .iter()
        .flat_map(|oid| format!("{oid}\n").into_bytes())
        .collect();
    let sizes = scratch.git_raw(&["cat-file", "--batch-check"], input)?;
    if sizes.code != Some(0) {
        return Err(format!("{remote}'s files could not be listed"));
    }
    let mut known: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&sizes.out).lines() {
        let mut fields = line.split(' ');
        if let (Some(oid), Some("blob"), Some(size)) = (fields.next(), fields.next(), fields.next())
            && let Ok(size) = size.parse::<u64>()
        {
            known.insert(oid.to_owned(), size);
        }
    }
    Ok(tree::read_listing(records, |oid| known.get(oid).copied()))
}

/// **Fetches `source`** into a scratch repository: the commit `at`, or its branch's newest where
/// `at` is `None` (or where the source will not hand a commit by its name and its branch is at
/// that commit anyway). Nothing is checked out, no submodule is fetched and no hook runs.
pub fn fetch(plane: &Path, source: &Source, at: Option<&Commit>) -> Result<Fetched, String> {
    source.check()?;
    let remote = &source.remote;
    let scratch = Scratch::new()?;
    let handing = handing(plane, source)?;
    let mut run = scratch.fetch(
        remote,
        &handing,
        at.map_or_else(|| remote.tip_ref(), |c| c.as_str().to_owned())
            .as_str(),
        FETCHED,
    )?;
    if !run.ok() && at.is_some() {
        run = scratch.fetch(remote, &handing, &remote.tip_ref(), FETCHED)?;
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
    let commit = Commit::parse(&resolved.out)
        .filter(|_| resolved.ok())
        .ok_or_else(|| format!("{source} answered no commit"))?;
    if at.is_some_and(|wanted| *wanted != commit) {
        return Err(format!(
            "{source} no longer offers the commit that was reviewed: review it again"
        ));
    }
    let (entries, left) = listed(&scratch, remote, &commit)?;
    Ok(Fetched {
        scratch,
        source: source.clone(),
        commit,
        entries,
        left,
        read: Cell::new(0),
    })
}

/// **The newest commit `source` offers**, read from its refs alone: nothing is fetched.
pub fn newest(plane: &Path, source: &Source) -> Result<Commit, String> {
    let remote = &source.remote;
    let scratch = Scratch::new()?;
    let handing = handing(plane, source)?;
    let tip = remote.tip_ref();
    let run = scratch.network(remote, &handing, &["ls-remote", "--", REMOTE, &tip])?;
    if !run.ok() {
        return Err(format!("{source} could not be asked: {}", complaint(&run)));
    }
    run.out
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(_, name)| *name == tip)
        .and_then(|(commit, _)| Commit::parse(commit))
        .ok_or_else(|| format!("{source} has no {tip}"))
}

/// **The git source, through the SkillSource interface** (ADR 0101 §1): its first user. What it
/// offers is [`Fetched::skills`], and one skill's files are read from the commit it was fetched
/// at, with the entries of its folder that are not installed.
impl super::SkillSource for Fetched {
    fn shown(&self) -> String {
        self.source.to_string()
    }

    fn skills(&self) -> Result<(Vec<Found>, Vec<NotASkill>), String> {
        Fetched::skills(self)
    }

    fn files(&self, skill: &Found) -> Result<(Vec<SourceFile>, Vec<LeftOut>), String> {
        Ok((
            self.files_of(&skill.dir)?,
            tree::left_in(&self.left, &skill.dir),
        ))
    }
}

impl Fetched {
    /// The blobs `oids` name, in their order, each as bytes, `sizes` their sizes. Refused where
    /// they would take the bytes read from this fetch past [`MOST_READ_BYTES`].
    fn blobs(&self, oids: &[&str], sizes: u64) -> Result<Vec<Vec<u8>>, String> {
        if oids.is_empty() {
            return Ok(Vec::new());
        }
        let read = self.read.get().saturating_add(sizes);
        if read > MOST_READ_BYTES {
            return Err(format!(
                "{} holds more than {} MiB of skills to read, the most purlis reads of one skill \
                 source at once",
                self.source,
                MOST_READ_BYTES >> 20
            ));
        }
        self.read.set(read);
        let input: Vec<u8> = oids
            .iter()
            .flat_map(|oid| format!("{oid}\n").into_bytes())
            .collect();
        let run = self.scratch.git_raw(&["cat-file", "--batch"], input)?;
        let unread = || format!("{}'s files could not be read", self.source);
        if run.code != Some(0) {
            return Err(unread());
        }
        let mut out = Vec::with_capacity(oids.len());
        let mut rest: &[u8] = &run.out;
        for oid in oids {
            let end = rest.iter().position(|b| *b == b'\n').ok_or_else(unread)?;
            let header = std::str::from_utf8(&rest[..end]).map_err(|_| unread())?;
            let mut fields = header.split(' ');
            let (Some(named), Some("blob"), Some(size), None) =
                (fields.next(), fields.next(), fields.next(), fields.next())
            else {
                return Err(unread());
            };
            let size: usize = size.parse().map_err(|_| unread())?;
            if named != *oid || rest.len() < end + 1 + size + 1 {
                return Err(unread());
            }
            out.push(rest[end + 1..end + 1 + size].to_vec());
            rest = &rest[end + 1 + size + 1..];
        }
        Ok(out)
    }

    /// The files `entries` names, inside the folder `dir`, with their bytes, within one skill's
    /// limits.
    fn read_files(&self, entries: &[Entry], dir: &[String]) -> Result<Vec<SourceFile>, String> {
        let listed = tree::files_of(entries, dir);
        let shown = if dir.is_empty() {
            self.source.to_string()
        } else {
            dir.join("/")
        };
        if listed.len() > tree::MOST_FILES {
            return Err(format!(
                "{shown} holds more than {} files, the most a skill purlis installs may",
                tree::MOST_FILES
            ));
        }
        let total: u64 = listed.iter().map(|(_, entry)| entry.size).sum();
        if total > MOST_SKILL_BYTES {
            return Err(format!(
                "{shown} holds more than {} MiB, the most a skill purlis installs may",
                MOST_SKILL_BYTES >> 20
            ));
        }
        let oids: Vec<&str> = listed.iter().map(|(_, entry)| entry.oid.as_str()).collect();
        let bytes = self.blobs(&oids, total)?;
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

    /// The files of the skill whose folder is `dir`, with their bytes.
    pub(super) fn files_of(&self, dir: &[String]) -> Result<Vec<SourceFile>, String> {
        self.read_files(&self.entries, dir)
    }

    /// The files at the folder `folder` of the commit `at`, fetched into the same scratch
    /// repository beside this one ([`super::update`]'s pinned side), or `None` where the source
    /// will not hand that commit by its name.
    pub(super) fn files_at(
        &self,
        plane: &Path,
        at: &Commit,
        folder: &str,
    ) -> Result<Option<Vec<SourceFile>>, String> {
        let handing = handing(plane, &self.source)?;
        let fetched = self.scratch.fetch(
            &self.source.remote,
            &handing,
            at.as_str(),
            "refs/purlis/pinned",
        )?;
        if !fetched.ok() {
            return Ok(None);
        }
        let (entries, _) = listed(&self.scratch, &self.source.remote, at)?;
        let dir = tree::folder_parts(folder).ok_or_else(|| format!("{folder:?} is no folder"))?;
        self.read_files(&entries, &dir).map(Some)
    }

    /// **The skills the source holds** under its folder, by name, and each folder that has a
    /// `SKILL.md` and is not a skill purlis offers, with why. Refused past [`tree::MOST_SKILLS`].
    ///
    /// Each skill's frontmatter is read as the YAML it is and failing closed
    /// ([`identity`]): a skill is offered only where every reader reads the same name
    /// and description.
    pub fn skills(&self) -> Result<(Vec<Found>, Vec<NotASkill>), String> {
        let folder = self.source.folder_parts().unwrap_or_default();
        let dirs = tree::skill_folders(&self.entries, &folder);
        if dirs.len() > tree::MOST_SKILLS {
            return Err(format!(
                "{} holds more than {} skills, the most purlis reads of one skill source",
                self.source,
                tree::MOST_SKILLS
            ));
        }
        let instructions: Vec<&Entry> = dirs
            .iter()
            .filter_map(|dir| {
                self.entries.iter().find(|e| {
                    e.path.len() == dir.len() + 1
                        && e.path.starts_with(dir)
                        && e.path.last().is_some_and(|n| n == purlis_skills::FILE)
                })
            })
            .collect();
        let total: u64 = instructions.iter().map(|e| e.size).sum();
        let oids: Vec<&str> = instructions.iter().map(|e| e.oid.as_str()).collect();
        let texts = self.blobs(&oids, total)?;
        let mut found = Vec::new();
        let mut not = Vec::new();
        for (dir, bytes) in dirs.iter().zip(texts) {
            let shown = dir.join("/");
            let read = String::from_utf8(bytes)
                .map_err(|_| "its instructions are not text".to_owned())
                .and_then(|text| identity(&text));
            match read {
                Ok((name, description)) => match purlis_skills::change::name_problem(&name) {
                    None => found.push(Found {
                        name,
                        description,
                        folder: shown,
                        dir: dir.clone(),
                    }),
                    Some(why) => not.push(NotASkill {
                        folder: shown,
                        why: format!("its name cannot be a skill's: {why}"),
                    }),
                },
                Err(why) => not.push(NotASkill { folder: shown, why }),
            }
        }
        found.sort_by(|a, b| a.name.cmp(&b.name));
        // Two folders of one name: neither is offered, as which one a chat would get is unclear.
        let mut twice: Vec<String> = Vec::new();
        for (at, one) in found.iter().enumerate() {
            if found[at + 1..].iter().any(|other| other.name == one.name)
                && !twice.contains(&one.name)
            {
                twice.push(one.name.clone());
            }
        }
        found.retain(|one| {
            let clash = twice.contains(&one.name);
            if clash {
                not.push(NotASkill {
                    folder: one.folder.clone(),
                    why: "another folder of the source has the same skill's name".to_owned(),
                });
            }
            !clash
        });
        Ok((found, not))
    }
}
