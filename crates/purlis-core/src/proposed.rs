// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A proposed change** (#1975, the operator's rulings PC-1..PC-14 of 2026-10-14): an edit to
//! one of purlis's own configuration files that a chat may not write, which only the person
//! applies, in the window.
//!
//! A chat's sandbox keeps it from writing the files a later program loads code or settings from
//! (ADR 0067 §5). purlis's own configuration is among them, and a chat used to write such a
//! change to its scratch folder and ask the person to run commands. Now:
//!
//! - **In Claude Code** the chat's own `Write`, `Edit` or `MultiEdit` of such a file is read by
//!   its `pretooluse-edit` hook ([`from_edit`]): the change is worked out against the file as it
//!   is now, handed to the app as a proposal ([`crate::hookwire::ProposeAsk`]), and the tool is
//!   refused with [`PROPOSED_SAID`], so the harness never writes the file (PC-2).
//! - **In another harness** the chat runs `purlis propose <file> --from <draft>`, the draft in
//!   its own folder (PC-2).
//! - **The app shows it whole and the person applies it** (PC-3..PC-10, PC-13): never a chat,
//!   never the harness, never "always". What is shown as the file now is the app's own read of
//!   it; the base a proposal carries is only checked against the file at Apply, so one that is
//!   wrong can only make the proposal stale.
//!
//! **Which files** ([`proposable`], PC-1, PC-11): purlis's own configuration, in the plane only:
//! the manifest at the project root and in each workspace, a persona's `mcp.json` and
//! `persona.md`, and a `.claude/skills` folder's files at the project root, in a workspace or in
//! a persona. Never a file that runs as code the moment it is there (git hooks, a harness's
//! settings, an editor's tasks, package scripts, `.envrc`), never a repo clone's own file, never
//! purlis's state, and never a path with a link in it.
//!
//! **Policy** (PC-14): `sandbox.proposed-changes = "off"` turns it off on this machine
//! ([`crate::sandbox::policy::Locks::proposed_changes_refused`]); the chat then gets the refusal
//! it got before.

use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

/// What the chat's refused tool says (PC-10): the person has it, and the chat waits.
pub const PROPOSED_SAID: &str = "purlis showed this change to the person as a proposed change: \
     only they apply it, in the window. Wait for their answer, which comes back to this chat as \
     applied, dismissed or stale. Do not write the file another way.";

/// **The most bytes a proposal carries** ([`Change::carried`]): what the hook channel carries
/// whole on one line at its worst, every byte a character JSON spells in six, beside the
/// longest path ([`MOST_PATH_BYTES`]) and the base. An edit carries only its own text, so the
/// file it changes may be far larger ([`MOST_FILE_BYTES`]); a whole new content is held to
/// this. A proposal is never cut: a longer one is not made (`hookwire`'s test holds the two
/// together).
pub const MOST_BYTES: usize = 12 * 1024;

/// The most bytes a file a proposal changes may hold, before or after: read whole to show the
/// change and to write it. Far past any configuration file.
pub const MOST_FILE_BYTES: usize = 1024 * 1024;

/// The longest path a proposal names, in bytes.
pub const MOST_PATH_BYTES: usize = 256;

/// **What a chat is told of an edit that could not be made a proposal**, whatever the reason:
/// fixed words, which say nothing of what the file holds.
pub const UNPREPARED: &str = "this edit could not be prepared as a proposed change, so nothing \
     was proposed and nothing was written. Make the change yourself, or ask the person to.";

/// **Whether `path` is a file a chat may propose a change to** in the project at `root`, judged
/// on its lexical path and on the disk: the path relative to `root` where it is one, or why not.
/// Every folder from `root` down to the file, and the file itself where it is there, must be no
/// link: a link would make the window write somewhere the person did not see named.
pub fn proposable(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let rel = relative_to(root, path).ok_or_else(|| not_proposable(path))?;
    let parts: Vec<&str> = rel
        .components()
        .map(|part| match part {
            Component::Normal(name) => name.to_str().unwrap_or("\u{0}"),
            _ => "\u{0}",
        })
        .collect();
    if rel.as_os_str().len() > MOST_PATH_BYTES
        || parts.iter().any(|part| *part == "\u{0}" || part.is_empty())
    {
        return Err(not_proposable(path));
    }
    let manifest = |name: &str| {
        crate::names::PLANE_MANIFEST
            .spellings()
            .any(|one| one == name)
    };
    let a_name = |name: &str| !name.starts_with('.');
    // Below `.claude/skills`, skill files alone: no part a program would load as its own (a
    // dot-file or dot-folder: `.envrc`, `.git`, `.vscode`), no package manifest and no hooks
    // folder, which PC-1 leaves for the person to change.
    let skills = |rest: &[&str]| {
        rest.len() >= 3
            && rest[0] == ".claude"
            && rest[1] == "skills"
            && rest[2..]
                .iter()
                .all(|p| !p.starts_with('.') && !SKILL_NEVER.contains(p))
    };
    let ours = match parts.as_slice() {
        [name] => manifest(name),
        ["workspaces", workspace, name] => a_name(workspace) && manifest(name),
        ["personas", persona, "mcp.json" | "persona.md"] => a_name(persona),
        rest if skills(rest) => true,
        ["workspaces" | "personas", owner, rest @ ..] => a_name(owner) && skills(rest),
        _ => false,
    };
    if !ours {
        return Err(not_proposable(path));
    }
    // No link anywhere from the root down: judged on the disk, each folder that is there. And
    // no folder made but below a skills folder: a proposal never makes a persona or a
    // workspace, which purlis's own ways make.
    let mut at = root.to_path_buf();
    for (index, part) in parts.iter().enumerate() {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if meta.file_type().is_symlink() => return Err(not_proposable(path)),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let a_folder = index + 1 < parts.len();
                if a_folder && !below_skills_at(&parts, index) {
                    return Err(NO_NEW_FOLDER.to_owned());
                }
                break;
            }
            Err(_) => return Err(not_proposable(path)),
        }
    }
    Ok(rel)
}

/// **Whether `path` is a file purlis itself may propose a change to** (#3467), for the person
/// to apply as a chat's proposal is applied: one of the project's two settings files at its
/// root, as the project spells them, or a workspace's manifest (#3486), there, and no link from
/// the root down. What a chat may propose ([`proposable`]) is not widened by it: no chat's
/// proposal is judged here.
pub fn proposable_by_purlis(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let rel = relative_to(root, path).ok_or_else(|| not_proposable(path))?;
    let settings = [
        crate::settings::Which::Shared,
        crate::settings::Which::Local,
    ]
    .into_iter()
    .any(|which| rel == Path::new(which.file_at(root)));
    let manifest = manifest_of(root, &rel).is_some();
    if !settings && !manifest {
        return Err(not_proposable(path));
    }
    let mut at = root.to_path_buf();
    for part in rel.components() {
        at.push(part);
        match std::fs::symlink_metadata(&at) {
            Ok(meta) if !meta.file_type().is_symlink() => {}
            _ => return Err(not_proposable(path)),
        }
    }
    match std::fs::symlink_metadata(&at) {
        Ok(meta) if meta.file_type().is_file() => Ok(rel),
        _ => Err(not_proposable(path)),
    }
}

/// The workspace of the plane at `root` whose manifest `rel` names, `workspaces/<ws>/workspace.json`,
/// or none.
fn manifest_of(root: &Path, rel: &Path) -> Option<String> {
    let workspace = crate::workspaces::Plane::open(root).workspace_of(&root.join(rel))?;
    (Path::new(&crate::settings::workspace::named(&workspace)) == rel).then_some(workspace)
}

/// **Writes purlis's own change to `rel`** ([`proposable_by_purlis`]) as `after`, only while
/// the file is still what `base` says, `before` being the text that base was read from: a
/// workspace's manifest by its own writer ([`crate::settings::workspace::save_text`], #3486),
/// under its lock and its stamp rule, so a manifest purlis wrote stays purlis's; any other file
/// as [`write_pinned`] writes a chat's. Nothing is written otherwise, and where the manifest's
/// writer refused, its own sentences say why.
pub fn write_by_purlis(
    root: &Path,
    rel: &Path,
    base: Option<&str>,
    before: Option<&str>,
    after: &str,
) -> Result<(), (NotMade, Option<String>)> {
    let Some(workspace) = manifest_of(root, rel) else {
        return write_pinned(root, rel, base, after).map_err(|made| (made, None));
    };
    if base.is_none() || base_of(before.map(str::as_bytes)).as_deref() != base {
        return Err((NotMade::Unprepared, None));
    }
    crate::settings::workspace::save_text(root, &workspace, before, after).map_err(|said| {
        // Whether it is because the file moved is read here; anything else, in its words.
        match read_pinned(root, rel) {
            Ok(now) if base_of(now.as_deref()).as_deref() != base => (NotMade::Stale, None),
            _ => (NotMade::Unprepared, Some(said.join(" "))),
        }
    })
}

/// What a proposal to a file in a persona or a workspace that is not there is refused with.
pub const NO_NEW_FOLDER: &str = "a proposed change does not make a new persona or workspace: the \
     person makes that in the window first, then the change can be proposed.";

/// Whether the folder at `index` of `parts` is a `.claude/skills` folder or below one: the one
/// place a proposal makes a folder (a new skill's, and the skills folder itself).
fn below_skills_at(parts: &[&str], index: usize) -> bool {
    (0..=index).any(|at| parts.get(at) == Some(&".claude") && parts.get(at + 1) == Some(&"skills"))
}

/// Names never proposed below `.claude/skills` (PC-1): what a program runs as its own.
pub(crate) const SKILL_NEVER: [&str; 2] = ["package.json", "hooks"];

/// Why `path` is not a file a chat may propose a change to: the way that works instead.
pub fn not_proposable(path: &Path) -> String {
    format!(
        "{} is not a file purlis lets a chat propose a change to: only purlis's own \
         configuration in the project (its manifest, a persona's mcp.json and persona.md, and a \
         .claude/skills folder's files) is. Make the change yourself, or ask the person to.",
        path.display()
    )
}

/// `path` relative to `root`, by its words alone: `.` and `..` taken out, and none left that
/// climbs out of `root`.
/// Under either spelling of `root`: as given, or as the kernel names it.
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

/// **What a proposal changes**, as it travels to the app: the tool's own edits, which the app
/// applies to the file as it reads it, or a whole new content. An edit carries only what it
/// replaces and what it puts there, so a proposal to a large file is as small as its edits.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Change {
    /// The file's whole new content: a `Write`, or `purlis propose --from`.
    Whole { content: String },
    /// Replacements made in turn, as `Edit` and `MultiEdit` make them.
    Edits { edits: Vec<Edit> },
}

/// One replacement: `old` replaced by `new`, once, or wherever it appears with `every`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Edit {
    pub old: String,
    pub new: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub every: bool,
}

impl Change {
    /// **A Claude Code edit tool's `input`, as a change**: `Write`'s content, `Edit`'s one
    /// replacement, `MultiEdit`'s in turn. Or why not: what the tool itself would refuse.
    pub fn of_tool(tool: &str, input: &Value) -> Result<Self, String> {
        let input = input
            .as_object()
            .ok_or_else(|| "the edit's input is not an object".to_owned())?;
        match tool {
            "Write" => Ok(Self::Whole {
                content: text(input, "content")?.to_owned(),
            }),
            "Edit" => Ok(Self::Edits {
                edits: vec![edit_of(input)?],
            }),
            "MultiEdit" => {
                let edits = input
                    .get("edits")
                    .and_then(Value::as_array)
                    .filter(|edits| !edits.is_empty())
                    .ok_or_else(|| "the edit's input has no edits".to_owned())?;
                let edits = edits
                    .iter()
                    .map(|edit| {
                        edit.as_object()
                            .ok_or_else(|| "one of the edits is not an object".to_owned())
                            .and_then(edit_of)
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Self::Edits { edits })
            }
            other => Err(format!("{other} is not an edit purlis proposes")),
        }
    }

    /// The bytes it carries: what one line of the hook channel must hold whole.
    pub fn carried(&self) -> usize {
        match self {
            Self::Whole { content } => content.len(),
            Self::Edits { edits } => edits
                .iter()
                .map(|edit| edit.old.len() + edit.new.len())
                .sum(),
        }
    }

    /// **The file's content once this change is made to `now`** (the file as it is, or none
    /// where it is not there), exactly as the tool would write it, or why it cannot be: the
    /// same in the hook, where the change is checked, and in the app, where it is shown and
    /// written.
    pub fn apply(&self, now: Option<&str>) -> Result<String, String> {
        let out = match self {
            Self::Whole { content } => content.clone(),
            Self::Edits { edits } => {
                if edits.is_empty() {
                    return Err("the change has no edits".to_owned());
                }
                let mut text = now.unwrap_or("").to_owned();
                for edit in edits {
                    text = replace(&text, edit)?;
                }
                text
            }
        };
        if out.len() > MOST_FILE_BYTES {
            return Err("the file would be longer than a proposed change may make it".to_owned());
        }
        Ok(out)
    }
}

/// **The file's content once a Claude Code edit tool's `input` is applied to `now`**
/// ([`Change::of_tool`], then [`Change::apply`]).
pub fn apply_edit(tool: &str, input: &Value, now: Option<&str>) -> Result<String, String> {
    Change::of_tool(tool, input)?.apply(now)
}

fn text<'a>(input: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("the edit's input has no {key}"))
}

/// One edit of the tool's input.
fn edit_of(input: &Map<String, Value>) -> Result<Edit, String> {
    Ok(Edit {
        old: text(input, "old_string")?.to_owned(),
        new: text(input, "new_string")?.to_owned(),
        every: input.get("replace_all").and_then(Value::as_bool) == Some(true),
    })
}

/// One replacement of `now`, as the tool makes it.
fn replace(now: &str, edit: &Edit) -> Result<String, String> {
    let Edit { old, new, every } = edit;
    if old.is_empty() {
        return Err("the text to replace is empty".to_owned());
    }
    if old == new {
        return Err("the edit changes nothing".to_owned());
    }
    match now.matches(old.as_str()).count() {
        0 => Err("the text to replace is not in the file".to_owned()),
        1 => Ok(now.replacen(old.as_str(), new, 1)),
        _ if *every => Ok(now.replace(old.as_str(), new)),
        many => Err(format!(
            "the text to replace is in the file {many} times: name more of it, or replace every \
             one"
        )),
    }
}

/// The base a proposal carries: the SHA-256 of the file as it was read, in hex, or none where
/// it was not there.
pub fn base_of(now: Option<&[u8]>) -> Option<String> {
    use sha2::Digest as _;
    now.map(|bytes| {
        sha2::Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    })
}

/// **What a Claude Code edit tool's hook `payload` proposes**, in the project at `root`, with
/// `cwd` the folder a relative path is read from: none where the tool's target is not a
/// proposable file (it is not this module's), else the proposal or why there is none.
pub fn from_edit(root: &Path, payload: &Value, cwd: &Path) -> Option<Result<Proposed, String>> {
    let tool = payload["tool_name"].as_str()?;
    if !matches!(tool, "Write" | "Edit" | "MultiEdit") {
        return None;
    }
    let named = payload["tool_input"]["file_path"].as_str()?;
    let path = cwd.join(named);
    let rel = proposable(root, &path).ok()?;
    // Why it could not be made is never said: it could depend on what the file holds.
    Some(
        Change::of_tool(tool, &payload["tool_input"])
            .and_then(|change| read_and_apply(root, &rel, change))
            .map_err(|_| UNPREPARED.to_owned()),
    )
}

/// The kind of a proposed change's audit (PC-4).
pub const APPLIED_KIND: &str = "trust.proposed.apply";

/// **A proposed change the person applied, as the audit records it** (PC-4): who (the person,
/// in the window), the persona the chat that proposed it runs as, the file, the SHA-256 of what
/// is written, and how far it went ([`Outcome`]): one record before the file is written, and
/// one after, so no record says a file holds what was never written. Which chat, when and on
/// which machine are the envelope's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied<'a> {
    pub persona: Option<&'a str>,
    pub path: &'a str,
    pub written: &'a str,
    pub outcome: Outcome,
    /// Why it failed, in purlis's words, for [`Outcome::Failed`].
    pub why: Option<&'a str>,
}

/// How far an Apply went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Pressed, and about to be written: recorded before the file is touched.
    Intent,
    /// Written.
    Applied,
    /// Not written: the file changed since.
    Stale,
    /// Not written, for [`Applied::why`].
    Failed,
}

impl Outcome {
    /// The word the record keeps it by.
    pub fn word(self) -> &'static str {
        match self {
            Self::Intent => "intent",
            Self::Applied => "applied",
            Self::Stale => "stale",
            Self::Failed => "failed",
        }
    }
}

impl Applied<'_> {
    /// The event's body.
    pub fn body(&self) -> serde_json::Value {
        serde_json::json!({
            "actor_kind": "human",
            "actor": "operator",
            "scope": "local-ui",
            "persona": self.persona,
            "file": self.path,
            "written": self.written,
            "outcome": self.outcome.word(),
            "why": self.why,
        })
    }
}

/// A proposal, as a hook or `purlis propose` hands it over: the file relative to the project,
/// the base the change was worked out against, the change, and what it makes of the file as
/// it was read here (checked, never sent: the app works it out again from its own read).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposed {
    pub path: PathBuf,
    pub base: Option<String>,
    pub change: Change,
    pub after: String,
}

/// The file at `rel` as it is now, and `change` made to it: the proposal, or why there is none.
/// The file is read only as [`read_pinned`] reads it, and a change longer than one line of the
/// hook channel carries whole is not made ([`MOST_BYTES`]).
pub fn read_and_apply(root: &Path, rel: &Path, change: Change) -> Result<Proposed, String> {
    if change.carried() > MOST_BYTES {
        return Err(UNPREPARED.to_owned());
    }
    let prepared = prepare(root, rel, None, &change).map_err(|_| UNPREPARED.to_owned())?;
    Ok(Proposed {
        path: rel.to_path_buf(),
        base: prepared.base,
        change,
        after: prepared.after,
    })
}

/// A change made to the file as it was read: the file then, its base, and what the change
/// makes of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prepared {
    pub before: Option<String>,
    pub base: Option<String>,
    pub after: String,
}

/// Why a change could not be made, or could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotMade {
    /// The file is not what the change was worked out against any more (PC-8): read it again
    /// and propose again.
    Stale,
    /// It could not be made at all: the file is not one regular file with one name, is not
    /// text, or the change does not apply. Why is never said.
    Unprepared,
}

/// **`change` made to the file at `rel` as it is now**, read only as [`read_pinned`] reads it,
/// and, where `base` is given, only while the file is still what that base says ([`NotMade::Stale`]
/// otherwise). What the window shows and what Apply writes are both this.
pub fn prepare(
    root: &Path,
    rel: &Path,
    base: Option<Option<&str>>,
    change: &Change,
) -> Result<Prepared, NotMade> {
    let now = read_pinned(root, rel).map_err(|Unpinned| NotMade::Unprepared)?;
    let read = base_of(now.as_deref());
    if let Some(expected) = base
        && expected != read.as_deref()
    {
        return Err(NotMade::Stale);
    }
    let before = match now {
        None => None,
        Some(bytes) => Some(String::from_utf8(bytes).map_err(|_| NotMade::Unprepared)?),
    };
    let after = change
        .apply(before.as_deref())
        .map_err(|_| NotMade::Unprepared)?;
    Ok(Prepared {
        before,
        base: read,
        after,
    })
}

/// Why a file was not read: it is not the one regular file its name says (a link, a second
/// name, a pipe or a device, a longer file), or it could not be opened. Never said to a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unpinned;

/// **The file at `rel` under `root`, read only where it is the file the name says**: each
/// folder from `root` down opened without following a link, the file itself opened without
/// following one and without waiting on it, and taken only where the open handle is a regular
/// file with one name, at most [`MOST_FILE_BYTES`] long. None where nothing is there; refused for
/// anything else (a link, a second name, a pipe or a device, a longer file).
#[cfg(unix)]
pub fn read_pinned(root: &Path, rel: &Path) -> Result<Option<Vec<u8>>, Unpinned> {
    use rustix::fs::{CWD, FileType, Mode, OFlags, fstat, openat};
    use std::io::Read as _;
    let parts: Vec<&std::ffi::OsStr> = rel
        .components()
        .map(|part| match part {
            Component::Normal(name) => Ok(name),
            _ => Err(Unpinned),
        })
        .collect::<Result<_, _>>()?;
    let (leaf, folders) = parts.split_last().ok_or(Unpinned)?;
    let folder = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let mut at = openat(CWD, root, folder, Mode::empty()).map_err(|_| Unpinned)?;
    for name in folders {
        at = match openat(&at, *name, folder | OFlags::NOFOLLOW, Mode::empty()) {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(_) => return Err(Unpinned),
        };
    }
    let file = match openat(
        &at,
        *leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(file) => file,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(_) => return Err(Unpinned),
    };
    let stat = fstat(&file).map_err(|_| Unpinned)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_nlink != 1 {
        return Err(Unpinned);
    }
    let mut bytes = Vec::new();
    std::fs::File::from(file)
        .take(MOST_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Unpinned)?;
    if bytes.len() > MOST_FILE_BYTES {
        return Err(Unpinned);
    }
    Ok(Some(bytes))
}

/// **Writes `after` as the file at `rel` under `root`, only while it is still what `base` says**
/// (none: not there): what the window's Apply does (PC-3), as the person. Each folder is opened
/// without following a link, and one not there yet is made; the file is checked through the
/// handle it is read by, exactly as [`read_pinned`] checks it; the new content goes to a file
/// of its own beside it, made new, flushed, and renamed over the old one, keeping its
/// permissions. Anything else is [`NotMade::Stale`] or [`NotMade::Unprepared`], and nothing is
/// written.
#[cfg(unix)]
pub fn write_pinned(
    root: &Path,
    rel: &Path,
    base: Option<&str>,
    after: &str,
) -> Result<(), NotMade> {
    write_pinned_making(root, rel, base, after, &below_skills_at)
}

/// [`write_pinned`], making a folder of `rel` that is not there where `may_make` says so of its
/// place in `rel`'s folders (their names, and its index): a new file only, as [`write_pinned`]
/// makes one below `.claude/skills`. The Skill studio makes a new skill's folder this way
/// (#2770).
#[cfg(unix)]
pub fn write_pinned_making(
    root: &Path,
    rel: &Path,
    base: Option<&str>,
    after: &str,
    may_make: &dyn Fn(&[&str], usize) -> bool,
) -> Result<(), NotMade> {
    use rustix::fs::{
        AtFlags, CWD, FileType, Mode, OFlags, fstat, mkdirat, openat, renameat, unlinkat,
    };
    use std::io::{Read as _, Write as _};
    let unprepared = |_| NotMade::Unprepared;
    let parts: Vec<&std::ffi::OsStr> = rel
        .components()
        .map(|part| match part {
            Component::Normal(name) => Ok(name),
            _ => Err(NotMade::Unprepared),
        })
        .collect::<Result<_, _>>()?;
    let (leaf, folders) = parts.split_last().ok_or(NotMade::Unprepared)?;
    let folder = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let mut at = openat(CWD, root, folder, Mode::empty()).map_err(unprepared)?;
    let names: Vec<&str> = folders
        .iter()
        .map(|one| one.to_str().unwrap_or(""))
        .collect();
    for (index, name) in folders.iter().enumerate() {
        at = match openat(&at, *name, folder | OFlags::NOFOLLOW, Mode::empty()) {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) if base.is_none() && may_make(&names, index) => {
                mkdirat(&at, *name, Mode::from_raw_mode(0o755)).map_err(unprepared)?;
                openat(&at, *name, folder | OFlags::NOFOLLOW, Mode::empty()).map_err(unprepared)?
            }
            Err(rustix::io::Errno::NOENT) => return Err(NotMade::Stale),
            Err(_) => return Err(NotMade::Unprepared),
        };
    }
    let mode = match openat(
        &at,
        *leaf,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Err(rustix::io::Errno::NOENT) if base.is_none() => 0o644,
        Err(rustix::io::Errno::NOENT) => return Err(NotMade::Stale),
        Err(_) => return Err(NotMade::Unprepared),
        Ok(file) => {
            let stat = fstat(&file).map_err(unprepared)?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_nlink != 1
            {
                return Err(NotMade::Unprepared);
            }
            let mut bytes = Vec::new();
            std::fs::File::from(file)
                .take(MOST_FILE_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| NotMade::Unprepared)?;
            if base_of(Some(&bytes)).as_deref() != base {
                return Err(NotMade::Stale);
            }
            stat.st_mode as u32 & 0o7777
        }
    };
    // A name no one can guess, made new: a name already there (left by a crash, or put there)
    // is never written through and never blocks; another is drawn.
    let mut made = None;
    for _ in 0..TEMP_TRIES {
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).map_err(|_| NotMade::Unprepared)?;
        let temp = format!(
            ".{}.purlis-apply-{}",
            leaf.to_string_lossy(),
            random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        match openat(
            &at,
            temp.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(mode as _),
        ) {
            Ok(file) => {
                made = Some((temp, file));
                break;
            }
            Err(rustix::io::Errno::EXIST) => {}
            Err(_) => return Err(NotMade::Unprepared),
        }
    }
    let (temp, written) = made.ok_or(NotMade::Unprepared)?;
    let mut file = std::fs::File::from(written);
    let done = file
        .write_all(after.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|_| NotMade::Unprepared)
        .and_then(|()| renameat(&at, temp.as_str(), &at, *leaf).map_err(unprepared));
    if done.is_err() {
        let _ = unlinkat(&at, temp.as_str(), AtFlags::empty());
    }
    done
}

/// How many names [`write_pinned`] draws for its new file before it gives up.
#[cfg(unix)]
const TEMP_TRIES: usize = 8;

/// [`write_pinned`] where no folder can be opened without following a link: never.
#[cfg(not(unix))]
pub fn write_pinned(_: &Path, _: &Path, _: Option<&str>, _: &str) -> Result<(), NotMade> {
    Err(NotMade::Unprepared)
}

/// [`write_pinned_making`] where no folder can be opened without following a link: never.
#[cfg(not(unix))]
pub fn write_pinned_making(
    _: &Path,
    _: &Path,
    _: Option<&str>,
    _: &str,
    _: &dyn Fn(&[&str], usize) -> bool,
) -> Result<(), NotMade> {
    Err(NotMade::Unprepared)
}

/// [`read_pinned`] where no folder can be opened without following a link: never.
#[cfg(not(unix))]
pub fn read_pinned(_root: &Path, _rel: &Path) -> Result<Option<Vec<u8>>, Unpinned> {
    Err(Unpinned)
}

#[cfg(test)]
mod tests;
