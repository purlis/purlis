// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Draft skills** (SK-9, #2772; spec #2762 decisions 13, 38, 50 and 52): a chat's write to a
//! skill's files, kept on this machine for the person to try, accept or reject in the Skill
//! studio.
//!
//! - **A chat never writes a skill.** Its `Write`, `Edit` or `MultiEdit` of a file below a plane
//!   level's skills ([`draftable`]), or its `purlis propose` of one, reaches the app on the
//!   proposed-change road (#1975), and the app keeps it here ([`take`]) instead of holding it as
//!   a proposal: in this machine's [`DRAFTS_DIR`], never in the plane, so no save and no chat
//!   sees it. A sandboxed chat is denied writing those folders at all (`sandbox::skills_held`).
//! - **One draft a skill.** A second write to the same skill's files, from any chat, adds to the
//!   draft that is there. Each file keeps the base it had when the draft first changed it, so
//!   Accept refuses a draft of a file that changed since ([`DraftFile::stale`]).
//! - **Accept is one commit** of exactly the files the draft wrote, through the plane's one save
//!   ([`accept`]), and a draft of a local skill is written and left uncommitted, as every local
//!   skill is. **Reject deletes the draft** ([`reject`]).
//! - **Try it** ([`try_out`]) lays the skill as the draft makes it out in this machine's
//!   [`TRIES_DIR`], beside an empty folder of its own in the person's temp folder: the
//!   **throwaway piece** a scratch chat starts in. A chat started there is armed with the
//!   built-in skills and that one skill alone ([`tried`]), and when it closes both are removed
//!   ([`end_try_at`]). What is left is the skill run its use recorded, under the skill's name.
//!
//! **No link is followed.** Every file is read and written through [`crate::proposed`]'s pinned
//! reads and writes, folders opened one component at a time with `O_NOFOLLOW`, and every removal
//! goes through the handle of the folder that holds it, of a name this module made.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use super::{Ctx, Level, find, level_is_there, rel};
use crate::proposed::{self, Change, NotMade};

/// The folder of this machine's state the drafts are kept in, one folder each.
pub const DRAFTS_DIR: &str = "skill-drafts";

/// The folder of this machine's state each Try it's skill is laid out in, one folder each.
pub const TRIES_DIR: &str = "skill-tries";

/// The start of a throwaway piece's folder's name, in the person's temp folder.
pub const PIECE_PREFIX: &str = "purlis-try-";

/// A draft's record, in its folder.
const RECORD: &str = "draft.json";

/// The folder of a draft's files, as they are below the skill's folder.
const FILES: &str = "files";

/// A try's file that names its throwaway piece's folder.
const PIECE: &str = "piece";

/// The most files one draft holds.
pub const MOST_FILES: usize = 32;

/// How many folders deep a draft's file, or a skill's file Try it lays out, may be.
const DEEPEST: usize = 8;

/// Names never drafted below a skill's folder: what a program runs as its own (PC-1's rule for
/// `.claude/skills`, [`crate::proposed`]).
const NEVER: [&str; 2] = ["package.json", "hooks"];

/// **What a chat whose write became a draft is told** (decision 38): the person has it in the
/// Skill studio, and nothing was written.
pub const DRAFTED_SAID: &str = "purlis kept this as a draft skill in the Skill studio, where the \
     person tries it and accepts or rejects it: nothing was written to the project. Say that the \
     draft is ready in the Skill studio. Do not write the file another way.";

/// One file below a plane level's skills, as a chat's write names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFile {
    pub level: Level,
    /// The skill's folder's name.
    pub folder: String,
    /// The file below the skill's folder, `/` between the parts: `SKILL.md`, `scripts/run.sh`.
    pub inner: String,
}

impl SkillFile {
    /// The file relative to the plane, `/` between the parts.
    pub fn rel(&self) -> String {
        format!(
            "{}/{}/{}",
            self.level.rel().unwrap_or_default(),
            self.folder,
            self.inner
        )
    }
}

/// **Whether `path` is a file of a skill at a plane level** of the project at `root`
/// (`skills/`, `skills.local/`, a workspace's `skills/` or `skills.local/`), judged on its
/// lexical path and on the disk: which skill and which of its files, or `None`. Never a
/// dot-name, a package manifest or a hooks folder below the skill's folder; never a level of a
/// workspace that is not there; and never a path with a link in it.
pub fn draftable(root: &Path, path: &Path) -> Option<SkillFile> {
    let rel = relative_to(root, path)?;
    let parts: Vec<&str> = rel.iter().map(|part| part.to_str()).collect::<Option<_>>()?;
    let at = match parts.first().copied()? {
        "workspaces" => 3,
        _ => 1,
    };
    if parts.len() < at + 2 || parts.len() > at + 1 + DEEPEST {
        return None;
    }
    let level = Level::of_rel(&parts[..at].join("/"))?;
    let (folder, inner) = (parts[at], &parts[at + 1..]);
    if folder.starts_with('.')
        || inner
            .iter()
            .any(|part| part.is_empty() || part.starts_with('.') || NEVER.contains(part))
    {
        return None;
    }
    if let Some(ws) = level.workspace_name()
        && !root.join("workspaces").join(ws).is_dir()
    {
        return None;
    }
    // No link anywhere from the root down, each part that is there.
    let mut here = root.to_path_buf();
    for part in &parts {
        here.push(part);
        match std::fs::symlink_metadata(&here) {
            Ok(meta) if meta.file_type().is_symlink() => return None,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Some(SkillFile {
        level,
        folder: folder.to_owned(),
        inner: inner.join("/"),
    })
}

/// `path` relative to `root`, by its words alone, under either spelling of `root`; `None`
/// where it climbs out of it or is `root` itself.
fn relative_to(root: &Path, path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    let real = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    [root, real.as_path()]
        .into_iter()
        .find_map(|root| out.strip_prefix(root).ok().map(Path::to_path_buf))
        .filter(|rel| !rel.as_os_str().is_empty())
}

/// A draft's record, as its folder keeps it.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Record {
    /// The level's folder, relative to the plane ([`Level::rel`]).
    level: String,
    folder: String,
    /// The chat that wrote it last, by its number.
    chat: Option<u32>,
    /// Each file the draft changed, and the SHA-256 of what the project held there when the
    /// draft first changed it (`None`: nothing).
    bases: BTreeMap<String, Option<String>>,
}

/// **A draft skill**, as the studio lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// What Accept, Reject and Try it are handed back.
    pub id: String,
    pub level: Level,
    /// The skill's folder's name, which is its name.
    pub name: String,
    /// The chat that wrote it last.
    pub chat: Option<u32>,
    /// Whether the level holds no skill of that folder yet.
    pub new: bool,
    pub files: Vec<DraftFile>,
}

/// One file of a draft: what the project holds there now, and what the draft makes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftFile {
    /// Below the skill's folder.
    pub path: String,
    pub before: Option<String>,
    pub after: String,
    /// The project's file changed since the draft first changed it: Accept refuses.
    pub stale: bool,
}

impl Draft {
    /// Whether any of its files changed in the project since it was drafted.
    pub fn stale(&self) -> bool {
        self.files.iter().any(|file| file.stale)
    }

    /// What its `SKILL.md` would be once accepted: the draft's, else the project's.
    fn instructions(&self) -> Option<&str> {
        self.files
            .iter()
            .find(|file| file.path == "SKILL.md")
            .map(|file| file.after.as_str())
    }
}

/// The id of the draft of the skill in `folder` at `level`: one a skill, so a second write adds
/// to the first. Sixteen hex characters of a SHA-256 over the two.
fn id_of(level: &Level, folder: &str) -> String {
    use sha2::Digest as _;
    let mut hash = sha2::Sha256::new();
    hash.update(level.rel().unwrap_or_default().as_bytes());
    hash.update([0]);
    hash.update(folder.as_bytes());
    hash.finalize()[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Whether `id` is a name this module makes: sixteen lowercase hex characters.
fn an_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The text of the file at `rel` under `at`, read pinned; `None` where nothing is there.
fn read_at(at: &Path, rel: &str) -> Result<Option<String>, String> {
    let bytes = proposed::read_pinned(at, Path::new(rel))
        .map_err(|_| format!("{rel} is not a plain file, so purlis does not read it"))?;
    bytes
        .map(|bytes| String::from_utf8(bytes).map_err(|_| format!("{rel} is not text")))
        .transpose()
}

/// Writes `now` as the file at `rel` under `at`, only while it still holds `was`, making the
/// folders from the `make_from`th (counting from 1) on.
fn write_at(
    at: &Path,
    rel: &str,
    was: Option<&str>,
    now: &str,
    make_from: usize,
) -> Result<(), NotMade> {
    let base = was.and_then(|text| proposed::base_of(Some(text.as_bytes())));
    proposed::write_pinned_making(at, Path::new(rel), base.as_deref(), now, &|_, index| {
        index + 1 >= make_from
    })
}

/// This machine's state folder of the project at `root`, made private, with `dir` in it.
fn state_with(root: &Path, dir: &str) -> Result<PathBuf, String> {
    let state = crate::plane::state_dir(root);
    std::fs::create_dir_all(&state).map_err(|why| why.to_string())?;
    crate::profiletrust::private_dir(&state).map_err(|why| why.to_string())?;
    super::store::made(&state, &[dir], true).map_err(|why| why.to_string())?;
    Ok(state)
}

/// The record of the draft `id`, or `None` where there is none.
fn record_of(state: &Path, id: &str) -> Result<Option<Record>, String> {
    read_at(state, &format!("{DRAFTS_DIR}/{id}/{RECORD}"))?
        .map(|text| {
            serde_json::from_str(&text).map_err(|_| "a draft's record could not be read".to_owned())
        })
        .transpose()
}

/// **Keeps chat `chat`'s change `change` to `file` as a draft** in the project at `root`: made
/// to the draft's copy of the file where the draft has one, else to the project's, and kept in
/// the draft. Refused where the change does not apply, or the draft would hold more than
/// [`MOST_FILES`] files. Nothing in the project is written.
pub fn take(root: &Path, chat: u32, file: &SkillFile, change: &Change) -> Result<String, String> {
    let level_rel = file
        .level
        .rel()
        .ok_or_else(|| format!("{} is no level of this project", file.level))?;
    let id = id_of(&file.level, &file.folder);
    let state = state_with(root, DRAFTS_DIR)?;
    super::store::made(&state, &[DRAFTS_DIR, &id], true).map_err(|why| why.to_string())?;
    let was_record = record_of(&state, &id)?;
    let mut record = was_record.clone().unwrap_or_else(|| Record {
        level: level_rel,
        folder: file.folder.clone(),
        ..Record::default()
    });
    if !record.bases.contains_key(&file.inner) && record.bases.len() >= MOST_FILES {
        return Err(format!(
            "this draft holds {} already, the most a draft holds: accept or reject it first",
            crate::counts::counted(MOST_FILES, "file", "files")
        ));
    }
    let drafted_rel = format!("{DRAFTS_DIR}/{id}/{FILES}/{}", file.inner);
    let drafted = read_at(&state, &drafted_rel)?;
    let in_project = read_at(root, &file.rel())?;
    let after = change.apply(drafted.as_deref().or(in_project.as_deref()))?;
    record
        .bases
        .entry(file.inner.clone())
        .or_insert_with(|| proposed::base_of(in_project.as_deref().map(str::as_bytes)));
    record.chat = Some(chat);
    // The draft's own folders are made below its `files/`.
    write_at(&state, &drafted_rel, drafted.as_deref(), &after, 3).map_err(|why| {
        super::not_written(&format!("the draft of {}", file.folder), why)
    })?;
    let was_text = match &was_record {
        Some(was) => Some(serde_json::to_string(was).map_err(|why| why.to_string())?),
        None => None,
    };
    let text = serde_json::to_string(&record).map_err(|why| why.to_string())?;
    write_at(
        &state,
        &format!("{DRAFTS_DIR}/{id}/{RECORD}"),
        was_text.as_deref(),
        &text,
        usize::MAX,
    )
    .map_err(|why| super::not_written(&format!("the draft of {}", file.folder), why))?;
    Ok(id)
}

/// **Every draft of the project at `root`**, by name. One whose record cannot be read is left
/// out, and so is one of a level that is no longer the project's.
pub fn list(root: &Path) -> Vec<Draft> {
    let state = crate::plane::state_dir(root);
    let mut out: Vec<Draft> = ids_in(&state, DRAFTS_DIR)
        .into_iter()
        .filter_map(|id| one_in(root, &state, &id).ok())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    out
}

/// The draft `id` of the project at `root`, or why there is none.
pub fn one(root: &Path, id: &str) -> Result<Draft, String> {
    if !an_id(id) {
        return Err(NO_SUCH.to_owned());
    }
    one_in(root, &crate::plane::state_dir(root), id)
}

/// What a draft that is no longer there is answered with.
pub const NO_SUCH: &str = "that draft is not waiting any more: it was accepted or rejected";

fn one_in(root: &Path, state: &Path, id: &str) -> Result<Draft, String> {
    let record = record_of(state, id)?.ok_or(NO_SUCH)?;
    let level = Level::of_rel(&record.level).ok_or(NO_SUCH)?;
    let level_rel = level.rel().ok_or(NO_SUCH)?;
    let mut files = Vec::new();
    for (path, base) in &record.bases {
        let Some(after) = read_at(state, &format!("{DRAFTS_DIR}/{id}/{FILES}/{path}"))? else {
            continue;
        };
        let before = read_at(root, &format!("{level_rel}/{}/{path}", record.folder))?;
        let stale = proposed::base_of(before.as_deref().map(str::as_bytes)) != *base;
        files.push(DraftFile {
            path: path.clone(),
            before,
            after,
            stale,
        });
    }
    let new = level
        .dir(root)
        .is_none_or(|dir| std::fs::symlink_metadata(dir.join(&record.folder)).is_err());
    Ok(Draft {
        id: id.to_owned(),
        level,
        name: record.folder,
        chat: record.chat,
        new,
        files,
    })
}

/// The names in `state`'s `dir` that are ids this module makes, each a folder itself.
fn ids_in(state: &Path, dir: &str) -> Vec<String> {
    #[cfg(unix)]
    {
        let Ok(at) = super::super::at::folder(&state.join(dir)) else {
            return Vec::new();
        };
        super::super::at::entries(&at)
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| entry.kind == rustix::fs::FileType::Directory && an_id(&entry.name))
            .map(|entry| entry.name)
            .collect()
    }
    #[cfg(not(unix))]
    {
        let _ = (state, dir);
        Vec::new()
    }
}

/// Removes `id`'s folder from `state`'s `dir`, through that folder's handle: a name this module
/// made, never followed where it is a link.
fn remove_in(state: &Path, dir: &str, id: &str) -> Result<(), String> {
    if !an_id(id) {
        return Err(NO_SUCH.to_owned());
    }
    #[cfg(unix)]
    {
        let at = super::super::at::folder(&state.join(dir)).map_err(|why| why.to_string())?;
        super::super::at::remove_tree_in(&at, id).map_err(|why| why.to_string())
    }
    #[cfg(not(unix))]
    {
        let _ = (state, dir);
        Err("a draft is removed only where its folders can be held open".to_owned())
    }
}

/// **Rejects the draft `id`**: deletes it. Nothing in the project changes.
pub fn reject(root: &Path, id: &str) -> Result<(), String> {
    let draft = one(root, id)?;
    remove_in(&crate::plane::state_dir(root), DRAFTS_DIR, &draft.id)
}

/// **Accepts the draft `id`** (decision 52): writes each of its files into the skill's folder,
/// only while the project's file is still what it was when the draft first changed it, and
/// commits exactly those files as one change through the plane's save (a draft of a local skill
/// is written and left uncommitted, as a local skill is). The draft is then deleted. Refused for a
/// draft that changed a file the project changed since, one whose instructions the live lint
/// finds a problem in, a new skill without instructions, or one a built-in skill or another skill
/// of the level has the name of.
pub fn accept(cx: &mut Ctx<'_>, id: &str) -> Result<PathBuf, String> {
    let plane = cx.plane;
    crate::settings::writable(plane)?;
    let draft = one(plane, id)?;
    let name = draft.name.clone();
    if draft.stale() {
        return Err(format!(
            "{name} changed in the project since the chat drafted it, so nothing was accepted: \
             reject the draft, or ask the chat to draft it again"
        ));
    }
    let instructions = match draft.instructions() {
        Some(text) => text.to_owned(),
        None if draft.new => {
            return Err(format!(
                "the draft of {name} has no instructions, so nothing was accepted"
            ));
        }
        None => super::text_of(plane, &draft.level, &name)?.1,
    };
    let problems = crate::skills::check(&instructions, &name);
    if !problems.is_empty() {
        let lines: Vec<String> = problems.iter().map(ToString::to_string).collect();
        return Err(format!("nothing was accepted: {}", lines.join("; ")));
    }
    let level = draft.level.clone();
    let dir = level_is_there(plane, &level)?;
    if draft.new {
        cx.not_built_in(&name)?;
        if crate::skills::named(&dir, &name).is_some() {
            return Err(format!(
                "the {level} skills already have one named {name}: reject the draft, or rename \
                 that one"
            ));
        }
    } else {
        find(plane, &level, &name)?;
    }
    let folder = dir.join(&name);
    let folder_rel = rel(plane, &folder);
    let mut written = Vec::new();
    for file in &draft.files {
        let file_rel = format!("{folder_rel}/{}", file.path);
        write_at(
            plane,
            &file_rel,
            file.before.as_deref(),
            &file.after,
            level.depth(),
        )
        .map_err(|why| super::not_written(&format!("{name}'s {}", file.path), why))?;
        written.push(file_rel);
    }
    remove_in(&crate::plane::state_dir(plane), DRAFTS_DIR, &draft.id)?;
    cx.done(format!("the draft of {name} is accepted, in {folder_rel}"));
    if !level.local {
        let message = if draft.new {
            format!("skill new {name}")
        } else {
            format!("skill edit {name}")
        };
        cx.commit(&written, &message);
    }
    Ok(folder)
}

/// **A Try it, laid out** ([`try_out`]): its id, and the throwaway piece's folder a scratch chat
/// starts in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Try {
    pub id: String,
    pub piece: PathBuf,
}

/// **Lays out Try it for the draft `id`** in the project at `root`: the skill as the draft makes
/// it (the project's files of the skill, with the draft's over them) in this machine's
/// [`TRIES_DIR`], as the one skill of a project of its own, and an empty throwaway piece in
/// `temp`, made new. A chat started in the piece gets the built-in skills and that skill alone
/// ([`tried`]).
pub fn try_out(root: &Path, id: &str, temp: &Path) -> Result<Try, String> {
    let draft = one(root, id)?;
    let state = state_with(root, TRIES_DIR)?;
    let try_id = random_id()?;
    super::store::made(&state, &[TRIES_DIR, &try_id], false).map_err(|why| why.to_string())?;
    let laid = (|| {
        let skill_rel = format!(
            "{TRIES_DIR}/{try_id}/{}/{}",
            purlis_skills::PROJECT_DIR,
            draft.name
        );
        let mut files: BTreeMap<String, String> = BTreeMap::new();
        if !draft.new {
            let folder = find(root, &draft.level, &draft.name)?;
            for (path, text) in files_below(root, &folder) {
                files.insert(path, text);
            }
        }
        for file in &draft.files {
            files.insert(file.path.clone(), file.after.clone());
        }
        for (path, text) in &files {
            write_at(&state, &format!("{skill_rel}/{path}"), None, text, 3)
                .map_err(|why| super::not_written(&format!("{}'s {path}", draft.name), why))?;
        }
        let piece = made_piece(temp, &try_id)?;
        let piece_text = piece.display().to_string();
        write_at(
            &state,
            &format!("{TRIES_DIR}/{try_id}/{PIECE}"),
            None,
            &piece_text,
            usize::MAX,
        )
        .map_err(|why| super::not_written("the try's record", why))?;
        Ok::<_, String>(piece)
    })();
    match laid {
        Ok(piece) => Ok(Try { id: try_id, piece }),
        Err(why) => {
            let _ = remove_piece(temp, &try_id);
            let _ = remove_in(&state, TRIES_DIR, &try_id);
            Err(why)
        }
    }
}

/// Sixteen random hex characters.
fn random_id() -> Result<String, String> {
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|why| why.to_string())?;
    Ok(random.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The throwaway piece of the try `id` in `temp`, made new and private, by the name the kernel
/// knows it by.
fn made_piece(temp: &Path, id: &str) -> Result<PathBuf, String> {
    #[cfg(unix)]
    {
        let at = super::super::at::folder(temp).map_err(|why| why.to_string())?;
        rustix::fs::mkdirat(
            &at,
            format!("{PIECE_PREFIX}{id}").as_str(),
            rustix::fs::Mode::from_raw_mode(0o700),
        )
        .map_err(|why| format!("the throwaway folder could not be made ({why})"))?;
        std::fs::canonicalize(temp.join(format!("{PIECE_PREFIX}{id}")))
            .map_err(|why| why.to_string())
    }
    #[cfg(not(unix))]
    {
        let _ = (temp, id);
        Err("Try it needs folders that can be held open".to_owned())
    }
}

/// Removes the throwaway piece of the try `id` from `temp`: only a folder itself of the name this
/// module made, through `temp`'s handle.
fn remove_piece(temp: &Path, id: &str) -> Result<(), String> {
    if !an_id(id) {
        return Err(NO_SUCH.to_owned());
    }
    #[cfg(unix)]
    {
        let at = super::super::at::folder(temp).map_err(|why| why.to_string())?;
        let name = format!("{PIECE_PREFIX}{id}");
        match super::super::at::entry_in(&at, &name).map_err(|why| why.to_string())? {
            None => Ok(()),
            Some(entry) if entry.kind == rustix::fs::FileType::Directory => {
                super::super::at::remove_tree_in(&at, &name).map_err(|why| why.to_string())
            }
            Some(_) => Err("the throwaway folder is not a folder, so it was left".to_owned()),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = temp;
        Ok(())
    }
}

/// The plain files below `folder` in the plane at `plane`, each read pinned: up to
/// [`MOST_FILES`], [`DEEPEST`] folders deep, no dot-name. What Try it copies of a skill a draft
/// changes.
#[cfg(unix)]
fn files_below(plane: &Path, folder: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let base = rel(plane, folder);
    let mut todo = vec![String::new()];
    while let Some(below) = todo.pop() {
        let at_rel = if below.is_empty() {
            base.clone()
        } else {
            format!("{base}/{below}")
        };
        let names: Vec<&str> = at_rel.split('/').collect();
        let Ok(dir) = super::store::open_dir(
            plane,
            &names.iter().map(|one| (*one).to_owned()).collect::<Vec<_>>(),
            usize::MAX,
        ) else {
            continue;
        };
        for entry in super::super::at::entries(&dir).unwrap_or_default() {
            if entry.name.starts_with('.') || out.len() >= MOST_FILES {
                continue;
            }
            let path = if below.is_empty() {
                entry.name.clone()
            } else {
                format!("{below}/{}", entry.name)
            };
            match entry.kind {
                rustix::fs::FileType::Directory if path.split('/').count() < DEEPEST => {
                    todo.push(path);
                }
                rustix::fs::FileType::RegularFile => {
                    if let Ok(Some(text)) = read_at(plane, &format!("{base}/{path}")) {
                        out.push((path, text));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// [`files_below`] where no folder can be held open: none.
#[cfg(not(unix))]
fn files_below(_: &Path, _: &Path) -> Vec<(String, String)> {
    Vec::new()
}

/// The try whose throwaway piece `cwd` is, in the project at `root`: its id. `None` for any other
/// folder.
fn try_of(root: &Path, cwd: &Path) -> Option<String> {
    let real = std::fs::canonicalize(cwd).ok()?;
    let state = crate::plane::state_dir(root);
    ids_in(&state, TRIES_DIR).into_iter().find(|id| {
        read_at(&state, &format!("{TRIES_DIR}/{id}/{PIECE}"))
            .ok()
            .flatten()
            .is_some_and(|piece| Path::new(piece.trim_end()) == real)
    })
}

/// **Where a chat started in `cwd` of the project at `root` takes its project skills from**,
/// where `cwd` is a Try it's throwaway piece: that try's folder, which holds the one skill as a
/// project of its own. `None` for every other chat. Only this machine's state names a piece, and
/// no chat writes it.
pub fn tried(root: &Path, cwd: &Path) -> Option<PathBuf> {
    let id = try_of(root, cwd)?;
    Some(crate::plane::state_dir(root).join(TRIES_DIR).join(id))
}

/// **Ends the Try it whose throwaway piece `cwd` is**, in the project at `root`, with the person's
/// temp folder `temp`: the piece and the laid-out skill are removed. Nothing for any other folder.
pub fn end_try_at(root: &Path, cwd: &Path, temp: &Path) -> Result<bool, String> {
    let Some(id) = try_of(root, cwd) else {
        return Ok(false);
    };
    end_try(root, &id, temp)?;
    Ok(true)
}

/// **Every Try it laid out in the project at `root`**: its id and its throwaway piece's folder.
/// What a new Try it sweeps: one whose piece no open chat stands in was left by a chat that never
/// closed (a quit, a crash), and is ended.
pub fn tries(root: &Path) -> Vec<(String, PathBuf)> {
    let state = crate::plane::state_dir(root);
    ids_in(&state, TRIES_DIR)
        .into_iter()
        .map(|id| {
            let piece = read_at(&state, &format!("{TRIES_DIR}/{id}/{PIECE}"))
                .ok()
                .flatten()
                .map(|text| PathBuf::from(text.trim_end()))
                .unwrap_or_default();
            (id, piece)
        })
        .collect()
}

/// **The prompt "Ask a chat to draft a skill" types** (decision 13), unsent, into a chat that
/// starts where `level`'s skills are reached from (the project's root, or its workspace's
/// folder): one line, which the person ends with what the skill is for.
pub fn asking(level: &Level) -> String {
    let dir = if level.local {
        purlis_skills::LOCAL_DIR
    } else {
        purlis_skills::PROJECT_DIR
    };
    format!(
        "Draft a skill as {dir}/<name>/SKILL.md, with name and description frontmatter (where          you cannot write it, write it in your own folder and run purlis propose with --from);          purlis keeps it as a draft in the Skill studio for me to try. The skill should "
    )
}

/// **The prompt Try it types** (decision 50), unsent, into its scratch chat: one line.
pub fn trying(name: &str) -> String {
    format!("Use the {name} skill on a small example of what it is for, and say how it went.")
}

/// Ends the try `id`: [`end_try_at`] by its id.
pub fn end_try(root: &Path, id: &str, temp: &Path) -> Result<(), String> {
    remove_piece(temp, id)?;
    remove_in(&crate::plane::state_dir(root), TRIES_DIR, id)
}

#[cfg(test)]
mod tests;
