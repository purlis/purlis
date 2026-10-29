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
//!   [`TRIES_DIR`], beside an empty folder of its own in the project's [`PIECES`]: the
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
#[cfg(unix)]
use crate::skills::at;

/// The folder of this machine's state the drafts are kept in, one folder each.
pub const DRAFTS_DIR: &str = "skill-drafts";

/// The folder of this machine's state each Try it's skill is laid out in, one folder each.
pub const TRIES_DIR: &str = "skill-tries";

/// The start of a throwaway piece's folder's name, in the project's [`PIECES`].
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
pub const DEEPEST: usize = 8;

/// **What a chat whose write became a draft is told** (decision 38), by its refused tool and by
/// `purlis propose`: the person has it in the Skill studio, and nothing was written.
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
/// (`skills/`, `skills.local/`, a workspace's or a persona's `skills/` or `skills.local/`),
/// judged on its lexical path and on the disk: which skill and which of its files, or `None`.
/// Never a dot-name, a package manifest or a hooks folder below the skill's folder; never a
/// level of a workspace that is not there, or of a persona with no folder of its own (SK-32,
/// #3048); and never a path with a link in it.
pub fn draftable(root: &Path, path: &Path) -> Option<SkillFile> {
    let rel = relative_to(root, path)?;
    let parts: Vec<&str> = rel
        .iter()
        .map(|part| part.to_str())
        .collect::<Option<_>>()?;
    let at = match parts.first().copied()? {
        "workspaces" | "personas" => 3,
        _ => 1,
    };
    if parts.len() < at + 2 || parts.len() > at + 1 + DEEPEST {
        return None;
    }
    let level = Level::of_rel(&parts[..at].join("/"))?;
    let (folder, inner) = (parts[at], &parts[at + 1..]);
    if folder.starts_with('.')
        || inner.iter().any(|part| {
            part.is_empty() || part.starts_with('.') || proposed::SKILL_NEVER.contains(part)
        })
    {
        return None;
    }
    if let Some(ws) = level.at.workspace()
        && !root.join("workspaces").join(ws).is_dir()
    {
        return None;
    }
    if let Some(persona) = level.at.persona()
        && !super::holds_skills(root, persona)
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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Record {
    level: Level,
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
    hex(&hash.finalize()[..8])
}

/// `bytes` as lowercase hex: the one spelling of every id this module makes.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether `id` is a name this module makes: sixteen lowercase hex characters.
fn an_id(id: &str) -> bool {
    id.len() == 16
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The text of the file at `rel` under `at`, read pinned; `None` where nothing is there.
/// Said without the file's path, which the window never shows.
fn read_at(at: &Path, rel: &str) -> Result<Option<String>, String> {
    let bytes = proposed::read_pinned(at, Path::new(rel))
        .map_err(|_| "a file of the skill is not a plain file, so purlis does not read it")?;
    bytes
        .map(|bytes| {
            String::from_utf8(bytes).map_err(|_| "a file of the skill is not text".to_owned())
        })
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
    file.level
        .rel()
        .ok_or_else(|| format!("{} is no level of this project", file.level))?;
    let id = id_of(&file.level, &file.folder);
    let state = state_with(root, DRAFTS_DIR)?;
    super::store::made(&state, &[DRAFTS_DIR, &id], true).map_err(|why| why.to_string())?;
    let was_record = record_of(&state, &id)?;
    let mut record = was_record.clone().unwrap_or_else(|| Record {
        level: file.level.clone(),
        folder: file.folder.clone(),
        chat: None,
        bases: BTreeMap::new(),
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
    let base_now = proposed::base_of(in_project.as_deref().map(str::as_bytes));
    // **A whole file written again over one the project changed since the draft first changed it
    // is drafted afresh** (S9, "Ask the chat to redo it", whose prompt asks for the whole file):
    // its base is the project's now, so the redo can be accepted. An edit to such a file is made
    // to the draft's copy as before, and the file stays out of date: an edit never throws away
    // what was drafted.
    let redone = matches!(change, Change::Whole { .. })
        && record
            .bases
            .get(&file.inner)
            .is_some_and(|base| *base != base_now);
    let after = if redone {
        change.apply(in_project.as_deref())?
    } else {
        change.apply(drafted.as_deref().or(in_project.as_deref()))?
    };
    if redone {
        record.bases.insert(file.inner.clone(), base_now);
    } else {
        record.bases.entry(file.inner.clone()).or_insert(base_now);
    }
    record.chat = Some(chat);
    // The draft's own folders are made below its `files/`.
    write_at(&state, &drafted_rel, drafted.as_deref(), &after, 3)
        .map_err(|why| super::not_written(&format!("the draft of {}", file.folder), why))?;
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
    let level = record.level.clone();
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
        let Ok(at) = at::folder(&state.join(dir)) else {
            return Vec::new();
        };
        at::entries(&at)
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
        let at = at::folder(&state.join(dir)).map_err(|why| why.to_string())?;
        at::remove_tree_in(&at, id).map_err(|why| why.to_string())
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
        if level.named(plane, &name).is_some() {
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
        // A new skill's folder holds the draft's files alone, so it is named whole, as a new
        // skill's is (`create`).
        let paths = if draft.new {
            vec![folder_rel.clone()]
        } else {
            written
        };
        cx.commit(&paths, &message);
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
/// [`TRIES_DIR`], as the one skill of a project of its own, and an empty throwaway piece in the
/// project's [`PIECES`], made new. A chat started in the piece gets the built-in skills and that skill alone
/// ([`tried`]).
pub fn try_out(root: &Path, id: &str) -> Result<Try, String> {
    let draft = one(root, id)?;
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
    lay_out(root, &[(&draft.name, &files)])
}

/// **A copy of a skill, read once for a grade** (SK-16, #2779): its name and its files, without
/// its [`EVALS`] folder, so the chat a fixture is run in is never handed the fixtures it is graded
/// by. Every arm with the skill is laid out from it ([`lay_out_arm`]), so an edit while the grade
/// runs changes nothing it grades.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub name: String,
    pub files: BTreeMap<String, String>,
}

/// **The copy of `name` at `level`, read once**: [`Snapshot`].
pub fn snapshot(root: &Path, level: &Level, name: &str) -> Result<Snapshot, String> {
    let folder = find(root, level, name)?;
    Ok(Snapshot {
        name: name.to_owned(),
        files: files_below(root, &folder)
            .into_iter()
            .filter(|(path, _)| path.split('/').next() != Some(EVALS))
            .collect(),
    })
}

/// **Lays out a grade's arm** (SK-16, #2779): a throwaway piece whose chat gets the built-in
/// skills and `skill` alone, or, with none, the built-in skills alone. Marked as a grade's
/// ([`graded_piece`]), so its chat is armed as a grade's chat is. Ended as a Try it is, when its
/// chat closes ([`end_try_at`]).
pub fn lay_out_arm(root: &Path, skill: Option<&Snapshot>) -> Result<Try, String> {
    lay_out_graded(
        root,
        skill.map(std::slice::from_ref).unwrap_or_default(),
        None,
        None,
    )
}

/// **Lays out a router's grade's arm** (SK-15e, #3233): a throwaway piece whose chat gets the
/// built-in skills and the library `skills`, each as it was read once for the grade, routed by
/// the router `router`, an extension's id, or by the built-in router where it is `None`
/// ([`arm_router`]). Marked as a grade's, as [`lay_out_arm`]'s is.
pub fn lay_out_library_arm(
    root: &Path,
    skills: &[Snapshot],
    router: Option<&str>,
    pins: &[String],
) -> Result<Try, String> {
    if router.is_some_and(|id| !crate::extension::an_id(id)) {
        return Err("a router's grade names no router".to_owned());
    }
    let pins = pins.join("\n");
    lay_out_graded(
        root,
        skills,
        router,
        Some(pins.as_str()).filter(|pins| !pins.is_empty()),
    )
}

/// A grade's piece of `skills`, marked as a grade's, as routed by `router` where it is one, and
/// with `pins`, one name a line, where there are any.
fn lay_out_graded(
    root: &Path,
    skills: &[Snapshot],
    router: Option<&str>,
    pins: Option<&str>,
) -> Result<Try, String> {
    let named: Vec<(&str, &BTreeMap<String, String>)> = skills
        .iter()
        .map(|skill| (skill.name.as_str(), &skill.files))
        .collect();
    let laid = lay_out(root, &named)?;
    let state = crate::plane::state_dir(root);
    let marks = [(GRADED, Some("")), (ROUTER, router), (PINS, pins)];
    for (file, text) in marks {
        let Some(text) = text else { continue };
        if let Err(why) = write_at(
            &state,
            &format!("{TRIES_DIR}/{}/{file}", laid.id),
            None,
            text,
            usize::MAX,
        ) {
            let _ = end_try(root, &laid.id);
            return Err(super::not_written("the grade's record of its folder", why));
        }
    }
    Ok(laid)
}

/// A try's file that marks it as a grade's arm.
const GRADED: &str = "graded";

/// A grade's arm's file that names the router its chat is routed by (SK-15e).
const ROUTER: &str = "router";

/// A router's grade's arm's file of the pins its chat reads: the scope's that chose the router.
const PINS: &str = "pins";

/// **The pins of the chat of a router's grade's arm whose project is `tried`**, the try's own
/// folder (SK-15e, #3233): the pins of the workspace or persona that chose the router, so the
/// router is asked about the skills a real chat there may hide, and both routers keep them listed.
/// `None` for any folder that is not a try's.
pub(crate) fn pins_of_arm(tried: &Path) -> Option<Vec<String>> {
    let id = tried.file_name()?.to_str()?;
    let tries = tried.parent()?;
    if tries.file_name()? != TRIES_DIR {
        return None;
    }
    let text = read_at(tries.parent()?, &format!("{TRIES_DIR}/{id}/{PINS}"))
        .ok()
        .flatten()?;
    Some(
        text.lines()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect(),
    )
}

/// **The router the chat of a router's grade's arm in `cwd` is routed by** (SK-15e, #3233), by
/// its extension's id, in the project at `root`: `None` for every other folder, and for the arm
/// routed by the built-in router. Only this machine's state says so, and no chat writes it.
pub fn arm_router(root: &Path, cwd: &Path) -> Option<String> {
    if !graded_piece(root, cwd) {
        return None;
    }
    let id = try_of(root, cwd)?;
    read_at(
        &crate::plane::state_dir(root),
        &format!("{TRIES_DIR}/{id}/{ROUTER}"),
    )
    .ok()
    .flatten()
    .filter(|router| crate::extension::an_id(router))
}

/// **Whether `cwd` is the throwaway piece of a grade's arm** in the project at `root`
/// ([`lay_out_arm`]): only this machine's state says so, and no chat writes it.
pub fn graded_piece(root: &Path, cwd: &Path) -> bool {
    a_piece(cwd)
        && try_of(root, cwd).is_some_and(|id| {
            read_at(
                &crate::plane::state_dir(root),
                &format!("{TRIES_DIR}/{id}/{GRADED}"),
            )
            .ok()
            .flatten()
            .is_some()
        })
}

/// The folder of a skill's fixtures, which a grade's arm leaves out of its copy.
const EVALS: &str = purlis_skills::grade::EVALS_DIR;

/// **Lays out a throwaway piece** in the project at `root`: `skill`'s files, by its name, in this
/// machine's [`TRIES_DIR`], as the one skill of a project of its own, or no skill at all, and an
/// empty throwaway piece in the project's [`PIECES`], made new. A chat started in the piece gets
/// the built-in skills and that skill alone ([`tried`]).
fn lay_out(root: &Path, skills: &[(&str, &BTreeMap<String, String>)]) -> Result<Try, String> {
    let state = state_with(root, TRIES_DIR)?;
    let try_id = random_id()?;
    super::store::made(&state, &[TRIES_DIR, &try_id], false).map_err(|why| why.to_string())?;
    let laid = (|| {
        for &(name, files) in skills {
            let skill_rel = format!("{TRIES_DIR}/{try_id}/{}/{name}", purlis_skills::PROJECT_DIR);
            for (path, text) in files {
                write_at(&state, &format!("{skill_rel}/{path}"), None, text, 3)
                    .map_err(|why| super::not_written(&format!("{name}'s {path}"), why))?;
            }
        }
        let piece = made_piece(root, &try_id)?;
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
            let _ = remove_piece(root, &try_id);
            let _ = remove_in(&state, TRIES_DIR, &try_id);
            Err(why)
        }
    }
}

/// Sixteen random hex characters.
fn random_id() -> Result<String, String> {
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|why| why.to_string())?;
    Ok(hex(&random))
}

/// The folder of the project the throwaway pieces are made in, relative to its root: inside the
/// project, since a sandboxed chat starts only in a folder of the project, and below
/// `workspaces/` under a dot-name no workspace takes, so git ignores it with every LOCAL
/// workspace's files (`/workspaces/*/*`) and no save stages it (`planegit::save_stages`).
pub const PIECES: [&str; 2] = ["workspaces", ".throwaway"];

/// The folder of the throwaway pieces of the project at `root`, held open, made where it is not
/// there; no link followed on the way.
#[cfg(unix)]
fn pieces_at(root: &Path, make: bool) -> Result<rustix::fd::OwnedFd, String> {
    let names: Vec<String> = PIECES.iter().map(|one| (*one).to_owned()).collect();
    super::store::open_dir(root, &names, if make { 1 } else { usize::MAX })
        .map_err(|why| format!("the throwaway folders could not be opened ({why})"))
}

/// The throwaway piece of the try `id` in the project at `root`, made new and private, by the
/// name the kernel knows it by.
fn made_piece(root: &Path, id: &str) -> Result<PathBuf, String> {
    #[cfg(unix)]
    {
        let at = pieces_at(root, true)?;
        rustix::fs::mkdirat(
            &at,
            format!("{PIECE_PREFIX}{id}").as_str(),
            rustix::fs::Mode::from_raw_mode(0o700),
        )
        .map_err(|why| format!("the throwaway folder could not be made ({why})"))?;
        let mut path = root.to_path_buf();
        path.extend(PIECES);
        std::fs::canonicalize(path.join(format!("{PIECE_PREFIX}{id}")))
            .map_err(|why| why.to_string())
    }
    #[cfg(not(unix))]
    {
        let _ = (root, id);
        Err("Try it needs folders that can be held open".to_owned())
    }
}

/// Removes the throwaway piece of the try `id` from the project at `root`: only a folder itself
/// of the name this module made, through its folder's handle.
fn remove_piece(root: &Path, id: &str) -> Result<(), String> {
    if !an_id(id) {
        return Err(NO_SUCH.to_owned());
    }
    #[cfg(unix)]
    {
        let Ok(at) = pieces_at(root, false) else {
            return Ok(());
        };
        let name = format!("{PIECE_PREFIX}{id}");
        match at::entry_in(&at, &name).map_err(|why| why.to_string())? {
            None => Ok(()),
            Some(entry) if entry.kind == rustix::fs::FileType::Directory => {
                at::remove_tree_in(&at, &name).map_err(|why| why.to_string())
            }
            Some(_) => Err("the throwaway folder is not a folder, so it was left".to_owned()),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = root;
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
            &names
                .iter()
                .map(|one| (*one).to_owned())
                .collect::<Vec<_>>(),
            usize::MAX,
        ) else {
            continue;
        };
        for entry in at::entries(&dir).unwrap_or_default() {
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

/// **Ends the Try it whose throwaway piece `cwd` is**, in the project at `root`: the piece and the
/// laid-out skill are removed. Nothing for any other folder.
pub fn end_try_at(root: &Path, cwd: &Path) -> Result<bool, String> {
    let Some(id) = try_of(root, cwd) else {
        return Ok(false);
    };
    end_try(root, &id)?;
    Ok(true)
}

/// **Every Try it laid out in the project at `root`**: its id and its throwaway piece's folder.
/// What a new Try it sweeps: one whose piece no open chat stands in was left by a chat that never
/// closed (a quit, a crash), and is ended.
pub fn tries(root: &Path) -> Vec<Try> {
    let state = crate::plane::state_dir(root);
    ids_in(&state, TRIES_DIR)
        .into_iter()
        .map(|id| {
            let piece = read_at(&state, &format!("{TRIES_DIR}/{id}/{PIECE}"))
                .ok()
                .flatten()
                .map(|text| PathBuf::from(text.trim_end()))
                .unwrap_or_default();
            Try { id, piece }
        })
        .collect()
}

/// How long a try is kept before a sweep may take it for one no chat stands in: the time between
/// laying it out and its chat being on the app's record, with room to spare.
pub const SWEEP_GRACE: std::time::Duration = std::time::Duration::from_secs(120);

/// **Ends every try of the project at `root` whose throwaway piece no chat in `open` stands in**
/// (each chat's folder), once it is older than [`SWEEP_GRACE`]: what a quit, a crash or a lost
/// close left. The app's, each time its record of open chats is written. Each throwaway folder
/// left with no try is removed too, by the names this module makes.
pub fn sweep_tries(root: &Path, open: &[PathBuf]) {
    let open: Vec<PathBuf> = open
        .iter()
        .filter_map(|cwd| std::fs::canonicalize(cwd).ok())
        .collect();
    let state = crate::plane::state_dir(root);
    let young = |path: &Path| {
        std::fs::symlink_metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age < SWEEP_GRACE)
    };
    let mut kept: Vec<String> = Vec::new();
    for one in tries(root) {
        if open.contains(&one.piece) || young(&state.join(TRIES_DIR).join(&one.id)) {
            kept.push(one.id);
            continue;
        }
        if let Err(why) = end_try(root, &one.id) {
            tracing::warn!(target: "purlis::skills", "a Try it left behind was not ended ({why})");
        }
    }
    // A throwaway folder whose try is gone: made by this module, named so, never followed.
    #[cfg(unix)]
    if let Ok(at) = pieces_at(root, false) {
        for entry in at::entries(&at).unwrap_or_default() {
            let Some(id) = entry.name.strip_prefix(PIECE_PREFIX) else {
                continue;
            };
            if an_id(id) && !kept.iter().any(|one| one == id) {
                let _ = remove_piece(root, id);
            }
        }
    }
}

/// **Whether `path` is a throwaway piece's folder**: named so, in the project's [`PIECES`]. The
/// one place that says it; [`tried`] is the one that says which try's.
pub fn a_piece(path: &Path) -> bool {
    let named = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(PIECE_PREFIX))
        .is_some_and(an_id);
    let mut above = path.parent().into_iter().flat_map(Path::components).rev();
    named
        && above.next().map(|part| part.as_os_str()) == Some(std::ffi::OsStr::new(PIECES[1]))
        && above.next().map(|part| part.as_os_str()) == Some(std::ffi::OsStr::new(PIECES[0]))
}

/// **Where a chat started in `cwd` of the project at `root` takes the plane's skills from**: the
/// project, or, for a Try it's scratch chat ([`tried`]), its try's folder, as a project of its own
/// with no workspace and no persona. The one answer arming, the briefing and a skill run's read
/// each ask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub root: PathBuf,
    /// A Try it's scratch chat: no workspace's, persona's or harness's skills beside the draft.
    pub tried: bool,
}

/// [`Scope`] of a chat in `cwd` of the project at `root`.
pub fn scope_of(root: &Path, cwd: Option<&Path>) -> Scope {
    match cwd
        .filter(|cwd| a_piece(cwd))
        .and_then(|cwd| tried(root, cwd))
    {
        Some(tried) => Scope {
            root: tried,
            tried: true,
        },
        None => Scope {
            root: root.to_path_buf(),
            tried: false,
        },
    }
}

/// **What a Try it's scratch chat on `harness` turns off** (decision 50): every plugin the
/// harness has installed and every skill of the person's own folders for it, so the chat has the
/// built-in skills and the draft alone. Each adapter hands it over by its harness's switch (SK-6):
/// Claude Code's `enabledPlugins` and `skillOverrides`, Codex's `skills.config`, opencode's
/// `permission.skill`. A built-in skill's name is never turned off (`skills::skills_off_for`).
pub fn alone_on(
    harness: crate::harness::Harness,
    env: &crate::harness_plugin::Env<'_>,
) -> crate::harness_plugin::Chosen {
    // The plugins a grade's chat has off, where the harness can turn one off for one chat; a
    // try is no measurement, so a record that cannot be read turns nothing off here. purlis's
    // pins are written over these at its arming, as for every chat.
    let plugins = crate::harness_plugin::only_purlis_on(harness.adapter().plugins(), env)
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, on)| !on)
        .collect();
    let skills_off = crate::skills::on_machine(harness, env)
        .into_iter()
        .filter_map(|found| match found {
            crate::skills::OnMachine::Skill { plugin: None, name } => Some(name),
            _ => None,
        })
        .collect();
    crate::harness_plugin::Chosen {
        plugins,
        skills_off,
        own_index: crate::skills::has_own_index(harness, env),
    }
}

/// The folder of `level`'s skills as a draft chat is told it, from where that chat starts: a
/// persona's in full, as its chat starts at the project's root (SK-32, #3048); else the level's
/// own, from its workspace's folder or the root.
fn skills_dir_told(level: &Level) -> String {
    match level.at.persona() {
        Some(_) => level.rel().unwrap_or_default(),
        None if level.local => purlis_skills::LOCAL_DIR.to_owned(),
        None => purlis_skills::PROJECT_DIR.to_owned(),
    }
}

/// **The prompt "Ask a chat to draft a skill" types** (decision 13), unsent, into a chat that
/// starts where `level`'s skills are reached from (the project's root, or its workspace's
/// folder): one line, which the person ends with what the skill is for.
pub fn asking(level: &Level) -> String {
    let dir = skills_dir_told(level);
    format!(
        "Draft a skill as {dir}/<name>/SKILL.md, with name and description frontmatter (where \
         you cannot write it, write it in your own folder and run purlis propose with --from); \
         purlis keeps it as a draft in the Skill studio for me to try. The skill should "
    )
}

/// **The prompt "Ask the chat to redo it" types** (S9), unsent, for the draft of the skill in
/// `folder` at `level` whose files `stale` (below the skill's folder) the project changed since:
/// into the chat that drafted it where it is still open (`same_chat`), else into a new chat that
/// starts where `level`'s skills are reached from, as [`asking`]'s does. It asks for each whole
/// file, which [`take`] drafts afresh on the project's copy. One line; a new chat's ends where
/// the person says what the change was.
pub fn redoing(level: &Level, folder: &str, stale: &[&str], same_chat: bool) -> String {
    let dir = skills_dir_told(level);
    let files = stale
        .iter()
        .map(|path| format!("{dir}/{folder}/{path}"))
        .collect::<Vec<_>>()
        .join(", ");
    if same_chat {
        format!(
            "The project's copy of {files} changed since you drafted your change to it, so purlis \
             cannot accept your draft. Read it again and write each whole file once more with \
             your change on top of it; purlis keeps it as a draft in the Skill studio for me to \
             try."
        )
    } else {
        format!(
            "The project's copy of {files} changed since a chat drafted a change to it. Read it and \
             write each whole file again with the change on top of it; purlis keeps it as a draft \
             in the Skill studio for me to try. The change should "
        )
    }
}

/// **The prompt Try it types** (decision 50), unsent, into its scratch chat: one line.
pub fn trying(name: &str) -> String {
    format!("Use the {name} skill on a small example of what it is for, and say how it went.")
}

/// Ends the try `id`: [`end_try_at`] by its id.
pub fn end_try(root: &Path, id: &str) -> Result<(), String> {
    remove_piece(root, id)?;
    remove_in(&crate::plane::state_dir(root), TRIES_DIR, id)
}

#[cfg(test)]
mod tests;
