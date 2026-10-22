// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Theme drafts** (spec #2313, #2332, ADR 0098 §4): a chat's writes to a theme, drawn only in
//! that chat's workspace until the person keeps or reverts them.
//!
//! # Where a draft is
//!
//! In purlis's git-ignored local folder in the plane (its state folder, `.purlis/`), one folder
//! for each chat, named by the chat's id (its ULID, ADR 0066), holding a theme folder for each
//! theme it drafts:
//!
//! ```text
//! .purlis/theme-drafts/<chat id>/<theme folder>/theme.json
//! ```
//!
//! **Nothing there is committed**: the state folder is ignored whole. **No chat writes it**: the
//! sandbox holds the whole folder from every chat, and the app writes a chat's draft for it, in
//! the folder of the chat whose token asked ([`start`], [`save`]), never one a chat names. A
//! chat edits a working copy of its own and hands it over (`purlis theme draft <folder>
//! --save`). What it hands over is still a chat's, untrusted: it is bounded before it is
//! written ([`save`]), and read as any project theme folder is, through handles opened refusing
//! a link, under the caps ([`super::Folders`]).
//!
//! # Where a draft is drawn
//!
//! A chat works in one workspace, or at the plane root. **The app says which**: the drafts drawn
//! in a workspace are those of the chats the app has open there ([`Place`]), never what a path
//! says. So a draft is drawn only in its chat's workspace; other workspaces and the plane root
//! draw as they did.
//!
//! # What a draft is kept to
//!
//! **The real folder it is a draft of** ([`KeptTo`]): the theme folder of that name in the
//! chat's workspace, or else the project's; a draft of a name neither has is a new theme of the
//! chat's own level (the workspace's, or the project's at the plane root). Keep copies the draft
//! there ([`keep`]), Revert deletes it ([`revert`]).
//!
//! **A draft remembers the real folder it started from**: its start records where that folder
//! is and a digest of its files ([`changed`]). When the folder there is not what the draft
//! started from (the person's own edit, a pull, another chat's write), Keep asks before it
//! replaces it, as it asks before it replaces any folder.
//!
//! **The drafts of a chat that is gone are deleted** ([`sweep`]): no strip names them, and no
//! close asks about them. Each chat's drafts folder names the clone whose app made it (its
//! [`crate::plane::CloneKey`]), and a sweep deletes only its own clone's: a state folder that
//! `$PURLIS_HOME` shares between clones keeps every other clone's drafts. A chat's folder that
//! names no clone is adopted by the sweep only in the plane's own state folder, the one used
//! while `$PURLIS_HOME` is not set: the sweep writes this clone's owner record into it, then
//! treats it as its own.
//!
//! # The draft's fonts and its floors
//!
//! - **A draft's fonts follow the project's approval gate**: a face is drawn only when the
//!   project's approval holds the file, with that digest, in the folder the draft is kept to. A
//!   font the draft brings that is not approved there refuses the draft, as it would the kept
//!   folder.
//! - **Every draw goes through [`super::draw`]**, so the consent floors hold on a draft as on
//!   any theme.
//! - **A draft that breaks a hard floor is not drawn** ([`super::hard`]), in any combination of
//!   its appearances and option values: the last good version of it stays ([`drafted`]), or the
//!   kept theme when there is none, and the chat is told which floor.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::folders::{FontGate, Found, Reach, THEMES, ThemesFolder, approved_fonts, read_level};
use super::{
    Appearance, Complaint, Contrast, Drawn, Folders, Placed, PlacedIn, Resolved, Theme, Values,
    hard,
};
use crate::theme::levels::appear;

/// The folder in the state folder that holds every chat's theme drafts.
pub const DRAFTS: &str = "theme-drafts";

/// The most chats whose drafts one place reads.
pub const MOST_CHATS: usize = 32;

/// **Where the window is**, as the app knows it: the plane, and the workspace in front (`None`
/// at the plane root).
#[derive(Debug, Clone, Copy)]
pub struct Place<'a> {
    pub plane: &'a Path,
    pub workspace: Option<&'a str>,
}

/// **The folder a draft is kept to**: a workspace's `themes/`, or the project's.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "level", content = "workspace", rename_all = "kebab-case")]
pub enum KeptTo {
    /// `workspaces/<ws>/themes/`.
    Workspace(String),
    /// `themes/` at the plane root.
    Project,
}

impl KeptTo {
    /// Its `themes/` folder, below the plane root.
    pub fn below(&self) -> PathBuf {
        match self {
            Self::Workspace(name) => Path::new("workspaces").join(name).join(THEMES),
            Self::Project => PathBuf::from(THEMES),
        }
    }
}

/// **One theme draft**: the chat that writes it, the theme folder it is, and where Keep puts it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Draft {
    /// The chat's id (its ULID).
    pub chat: String,
    /// The theme folder's name.
    pub folder: String,
    /// Where it is kept to.
    pub kept_to: KeptTo,
}

/// Whether `id` is a chat's id as a drafts folder is named: a ULID, spelled as purlis spells it.
pub fn is_chat_id(id: &str) -> bool {
    crate::reopen::a_ulid(id).as_deref() == Some(id)
}

/// The root a drafts folder is walked from and the path below it: the plane and the state
/// folder's place in it, or the state folder itself when `$PURLIS_HOME` puts it elsewhere.
fn root_of(plane: &Path) -> (PathBuf, PathBuf) {
    let state = crate::plane::state_dir(plane);
    match state.strip_prefix(plane) {
        Ok(below) if !below.as_os_str().is_empty() => (plane.to_path_buf(), below.join(DRAFTS)),
        _ => (state, PathBuf::from(DRAFTS)),
    }
}

/// **The folder of `chat`'s drafts**, `None` for an id that is not a chat's.
pub fn folder_of(plane: &Path, chat: &str) -> Option<PathBuf> {
    if !is_chat_id(chat) {
        return None;
    }
    let (root, below) = root_of(plane);
    Some(root.join(below).join(chat))
}

/// Where a draft of `folder`, written by a chat at `place`, is kept to: the nearest of the
/// workspace's and the project's theme folders that has one of that name, else the place's own.
pub fn kept_to(folders: &Folders, workspace: Option<&str>, folder: &str) -> KeptTo {
    let own = || match workspace {
        Some(name) => KeptTo::Workspace(name.to_owned()),
        None => KeptTo::Project,
    };
    let reach = if workspace.is_some() {
        Reach::Workspace
    } else {
        Reach::Project
    };
    match folders
        .named(reach)
        .into_iter()
        .find(|(_, name)| name == folder)
    {
        Some((Reach::Workspace, _)) => own(),
        Some((Reach::Project, _)) => KeptTo::Project,
        _ => own(),
    }
}

/// **One chat's drafts folder as a level**, read as a project's theme folders are, with its fonts
/// gated where each draft is kept to. `None` for an id that is not a chat's.
pub fn level(
    config_root: Option<&Path>,
    place: Place<'_>,
    folders: &Folders,
    chat: &str,
) -> Option<ThemesFolder> {
    if !is_chat_id(chat) {
        return None;
    }
    let (root, below) = root_of(place.plane);
    let mut level = ThemesFolder {
        root,
        below: below.join(chat),
        placed: Placed::level(PlacedIn::Draft),
        fonts: FontGate::Draft {
            files: Arc::new(BTreeMap::new()),
            kept_at: Arc::new(BTreeMap::new()),
            measured: folders.measured(),
        },
    };
    let names: Vec<String> = read_level(&level, &mut Vec::new()).into_keys().collect();
    let kept_at = names
        .iter()
        .map(|name| {
            (
                name.clone(),
                kept_to(folders, place.workspace, name).below(),
            )
        })
        .collect();
    level.fonts = FontGate::Draft {
        files: Arc::new(approved_fonts(config_root, Some(place.plane))),
        kept_at: Arc::new(kept_at),
        measured: folders.measured(),
    };
    Some(level)
}

/// **What live reload watches for the drafts of the plane at `plane`**: its drafts folder (or
/// the nearest folder on the way to it while it is not there), so a chat's first draft redraws,
/// and the drafts folder of each of `chats`, the chats the app has open in the plane, at most
/// [`MOST_CHATS`]. Nothing else in the folder is listed or watched. Which draft is drawn is the
/// window's ask's to decide ([`at`]); a save in any only asks it again.
pub fn watched(plane: &Path, chats: &[&str]) -> Vec<ThemesFolder> {
    let (root, below) = root_of(plane);
    let own = |below: PathBuf| ThemesFolder {
        root: root.clone(),
        below,
        placed: Placed::level(PlacedIn::Draft),
        fonts: FontGate::Own,
    };
    std::iter::once(own(below.clone()))
        .chain(
            chats
                .iter()
                .filter(|chat| is_chat_id(chat))
                .take(MOST_CHATS)
                .map(|chat| own(below.join(chat))),
        )
        .collect()
}

/// **The drafts at `place`**: each draft of each of `chats` (the chats the app has open there,
/// in its order), by folder name within a chat. At most [`MOST_CHATS`] chats are read.
pub fn at(config_root: Option<&Path>, place: Place<'_>, chats: &[&str]) -> Vec<Draft> {
    // No chat works here: nothing is read.
    if chats.is_empty() {
        return Vec::new();
    }
    let folders = Folders::of(config_root, Some(place.plane), place.workspace);
    let mut out = Vec::new();
    for chat in chats.iter().take(MOST_CHATS) {
        let Some(level) = level(config_root, place, &folders, chat) else {
            continue;
        };
        for folder in read_level(&level, &mut Vec::new()).into_keys() {
            out.push(Draft {
                chat: (*chat).to_owned(),
                kept_to: kept_to(&folders, place.workspace, &folder),
                folder,
            });
        }
    }
    out
}

/// **The draft `draft` read**, with what it may extend: as the window draws it, or `checking`
/// only to check, so none of its faces is registered to be served.
pub fn read(
    config_root: Option<&Path>,
    place: Place<'_>,
    draft: &Draft,
    checking: bool,
) -> Result<Resolved, Vec<Complaint>> {
    let folders = Folders::of(config_root, Some(place.plane), place.workspace);
    let Some(level) = level(config_root, place, &folders, &draft.chat) else {
        return Err(vec![Complaint::said(format!(
            "{} is not a chat's id, so it has no drafts",
            draft.chat
        ))]);
    };
    match folders.find_draft(&level, &draft.folder, checking) {
        Found::Read(resolved) => Ok(resolved),
        Found::Missing(mut said) => {
            said.push(Complaint::said(format!(
                "there is no theme draft {}",
                draft.folder
            )));
            Err(said)
        }
    }
}

/// One hard floor a draft breaks, in one combination of its appearances and option values.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BrokenIn {
    /// The appearance's id.
    pub appearance: String,
    /// The option values it was drawn with.
    pub options: Values,
    pub broken: hard::Broken,
}

impl BrokenIn {
    /// The sentence a chat is told.
    pub fn said(&self) -> String {
        let mut combination = vec![self.appearance.clone()];
        combination.extend(
            self.options
                .iter()
                .map(|(option, value)| format!("{option} {value}")),
        );
        format!("{} ({})", self.broken.said(), combination.join(", "))
    }
}

/// **Every hard floor `theme` breaks**, in every combination of its appearances and option
/// values, each as the window draws it: the consent floors held first.
pub fn broken(theme: &Theme) -> Vec<BrokenIn> {
    super::check_every(theme)
        .into_iter()
        .flat_map(|one| {
            let (appearance, options) = (one.appearance, one.options);
            one.hard.into_iter().map(move |broken| BrokenIn {
                appearance: appearance.clone(),
                options: options.clone(),
                broken,
            })
        })
        .collect()
}

/// What [`drafted`] answers.
#[derive(Debug, Clone, PartialEq)]
pub struct Drafted {
    /// What the window draws for the draft: the draft, or its last good version; `None` when
    /// neither can be, and the kept theme is drawn.
    pub drawn: Option<Drawn>,
    /// The draft's version to hold as its last good one: the draft when it is drawn.
    pub good: Option<Theme>,
    /// The hard floors the draft breaks, which is why it is not drawn.
    pub broken: Vec<BrokenIn>,
    /// Why the engine refused the draft, when it did.
    pub refused: Vec<Complaint>,
}

/// **What the window draws for a draft**: `read` is the draft as read ([`read`]), `last_good`
/// the version of it last drawn. A draft that is read and holds every hard floor is drawn, in
/// `appearance` (a choice's id, `system` or none); one that is refused or breaks a hard floor
/// is not, and its last good version is drawn in its place, or nothing (the kept theme). Every
/// draw goes through [`super::draw`]: the consent floors hold.
pub fn drafted(
    read: Result<Resolved, Vec<Complaint>>,
    last_good: Option<&Theme>,
    appearance: Option<&str>,
    system: Appearance,
    contrast: Contrast,
) -> Drafted {
    let stays = |broken, refused| Drafted {
        drawn: last_good.map(|theme| appear(theme, appearance, system, contrast)),
        good: last_good.cloned(),
        broken,
        refused,
    };
    let resolved = match read {
        Err(said) => return stays(Vec::new(), said),
        Ok(resolved) if resolved.refused => return stays(Vec::new(), resolved.complaints),
        Ok(resolved) => resolved,
    };
    let broken = broken(&resolved.theme);
    if !broken.is_empty() {
        return stays(broken, Vec::new());
    }
    let drawn = appear(&resolved.theme, appearance, system, contrast);
    Drafted {
        drawn: Some(Drawn {
            complaints: [resolved.complaints, drawn.complaints.clone()].concat(),
            ..drawn
        }),
        good: Some(resolved.theme),
        broken: Vec::new(),
        refused: Vec::new(),
    }
}

/// The folder of the draft `draft` of the plane at `plane`, checked: no link on the way to it
/// from its root.
fn draft_folder(plane: &Path, draft: &Draft) -> Result<PathBuf, String> {
    if !is_chat_id(&draft.chat) || !super::is_folder_name(&draft.folder) {
        return Err(format!("{} is not a theme draft", draft.folder));
    }
    let (root, below) = root_of(plane);
    let folder = root.join(below).join(&draft.chat).join(&draft.folder);
    crate::contain::no_link_on_the_way(&root, &folder)
        .map_err(|why| format!("the theme draft {} is not read: {why}", draft.folder))?;
    Ok(folder)
}

/// **Reverts a draft**: deletes it, and its chat's drafts folder once it is empty. A draft that
/// is not there is reverted already.
pub fn revert(plane: &Path, draft: &Draft) -> Result<(), String> {
    let folder = draft_folder(plane, draft)?;
    match std::fs::symlink_metadata(&folder) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(why) => {
            return Err(format!(
                "the theme draft {} is not read: {why}",
                draft.folder
            ));
        }
        Ok(meta) if !meta.file_type().is_dir() => {
            return Err(format!("the theme draft {} is not a folder", draft.folder));
        }
        Ok(_) => {}
    }
    // The standard library's removal opens each folder below refusing a link.
    std::fs::remove_dir_all(&folder).map_err(|why| {
        format!(
            "purlis could not delete the theme draft {}: {why}",
            draft.folder
        )
    })?;
    if let Some(chat) = folder.parent() {
        let _ = std::fs::remove_file(started_record(chat, &draft.folder));
        // A chat's folder that holds only its owner record holds no draft: it goes with it.
        let only_owner = std::fs::read_dir(chat).is_ok_and(|listed| {
            listed
                .filter_map(Result::ok)
                .all(|entry| entry.file_name() == OWNER)
        });
        if only_owner {
            let _ = std::fs::remove_file(chat.join(OWNER));
            let _ = std::fs::remove_dir(chat);
        }
    }
    Ok(())
}

/// What [`keep`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The theme folder it is now.
    pub path: PathBuf,
    /// Where it was kept to.
    pub kept_to: KeptTo,
}

/// **What the person was shown of a Keep**: where it puts the draft, and whether it replaces a
/// theme folder there. Keep does exactly that, or nothing ([`keep`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub kept_to: KeptTo,
    pub replaces: bool,
    /// Whether the folder there is not what the draft started from ([`changed`]).
    pub changed: bool,
}

/// Whether Keep of `draft` would replace a theme folder that is there: the folder it is a draft
/// of, in the workspace's or the project's `themes/`.
pub fn replaces(plane: &Path, draft: &Draft) -> bool {
    let kept = plane.join(draft.kept_to.below()).join(&draft.folder);
    std::fs::symlink_metadata(kept).is_ok()
}

/// The font files `theme_json` names in its faces, by their path in the theme folder.
fn named_faces(theme_json: &[u8]) -> Vec<String> {
    let Ok(document) = serde_json::from_slice::<serde_json::Value>(theme_json) else {
        return Vec::new();
    };
    let Some(fonts) = document.get("fonts").and_then(serde_json::Value::as_object) else {
        return Vec::new();
    };
    fonts
        .values()
        .filter_map(|font| font.get("faces")?.as_array())
        .flatten()
        .filter_map(|face| face.get("file")?.as_str().map(str::to_owned))
        .collect()
}

/// **Keeps a draft**: copies it into the real folder it is a draft of ([`KeptTo`]) and deletes
/// the draft. Only a draft the window draws is kept: one the engine refuses, or that breaks a
/// hard floor, is not, and the sentence says why. **It does what the person was shown
/// (`seen`)**: where it puts the draft, and whether it replaces a folder there; when either has
/// changed since, it is refused, and nothing is written.
///
/// **Each file is read once**: the draft's files are gathered (through handles refusing a link,
/// under the caps of a theme folder), and those bytes are what is checked and what is written.
/// The check reads them from a folder of purlis's own. **Only the font files the theme names in
/// its faces, and which the project's approval holds with those bytes' digest where the draft is
/// kept to, are carried**; any other font file stays behind.
pub fn keep(
    config_root: Option<&Path>,
    place: Place<'_>,
    draft: &Draft,
    seen: &Seen,
) -> Result<Kept, String> {
    let folder = draft_folder(place.plane, draft)?;
    let folders = Folders::of(config_root, Some(place.plane), place.workspace);
    let kept_to = kept_to(&folders, place.workspace, &draft.folder);
    let now = Draft {
        kept_to: kept_to.clone(),
        ..draft.clone()
    };
    let replace = replaces(place.plane, &now);
    // What the person was shown is what Keep does, or nothing: a destination, a folder there, or
    // a change to it since the draft started, that is not what they were shown is asked about
    // again.
    if kept_to != seen.kept_to
        || replace != seen.replaces
        || changed(place.plane, &now) != seen.changed
    {
        return Err(format!(
            "where Keep puts the theme draft {} has changed since you were shown it, so it is not \
             kept: look at it again",
            draft.folder
        ));
    }
    let (mut files, _) = super::gather(&folder)?;
    let in_project = slashed(&kept_to.below().join(&draft.folder));
    let named = files
        .get(super::THEME_JSON)
        .map(|bytes| named_faces(bytes))
        .unwrap_or_default();
    let approved = approved_fonts(config_root, Some(place.plane));
    files.retain(|path, bytes| {
        if !path.starts_with("fonts/") {
            return true;
        }
        let digest = super::fonts::digest(bytes);
        named.iter().any(|one| one == path)
            && approved
                .get(&format!("{in_project}/{path}"))
                .is_some_and(|listed| listed.split(' ').next() == Some(digest.as_str()))
    });
    // The check, of exactly these bytes, from a folder no chat writes.
    let checked = tempfile::tempdir()
        .map_err(|why| format!("purlis could not check the theme draft: {why}"))?;
    super::stage(checked.path(), &draft.folder, &files, false)
        .and_then(super::Staged::commit)
        .map_err(|why| why.to_string())?;
    let resolved = folders
        .library_within(Reach::Workspace)
        .reading_named_fonts()
        .checking()
        .load_folder(checked.path(), &draft.folder);
    if resolved.refused {
        return Err(format!(
            "purlis does not draw the theme draft {}, so it is not kept: {}",
            draft.folder,
            said_of(&resolved.complaints)
        ));
    }
    if let Some(first) = broken(&resolved.theme).first() {
        return Err(format!(
            "the theme draft {} breaks a hard floor, so it is not kept: {}",
            draft.folder,
            first.said()
        ));
    }
    let into = place.plane.join(kept_to.below());
    crate::contain::no_link_on_the_way(place.plane, &into)
        .map_err(|why| format!("{} is not written: {why}", into.display()))?;
    let path = super::stage(&into, &draft.folder, &files, seen.replaces)
        .and_then(super::Staged::commit)
        .map_err(|why| match why {
            super::NotWritten::Exists(path) => format!(
                "{} is there already, and Keep replaces it only when you say so",
                path.display()
            ),
            other => other.to_string(),
        })?;
    revert(place.plane, draft)?;
    Ok(Kept { path, kept_to })
}

/// The sentences, joined.
fn said_of(said: &[Complaint]) -> String {
    said.iter()
        .map(|one| one.said.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

/// What [`start`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Started {
    /// The draft's folder: where the chat writes.
    pub path: PathBuf,
    /// Whether it was made now: a copy of the kept folder, or a new theme; `false` when the
    /// chat's draft of that name was there already.
    pub made: bool,
    /// Where it is kept to.
    pub kept_to: KeptTo,
}

/// **Starts `chat`'s draft of the theme folder `folder`** at `place`: a copy of the folder it is a
/// draft of when there is one (the workspace's or the project's), else a new theme that extends
/// Purlis, the default (#3034). A draft that is there already is left as it is. `chat` is the app's record of
/// the chat that asked, never a word of the ask.
pub fn start(
    config_root: Option<&Path>,
    place: Place<'_>,
    chat: &str,
    folder: &str,
) -> Result<Started, String> {
    if !is_chat_id(chat) {
        return Err(
            "a theme draft belongs to a chat purlis started, and this is not one".to_owned(),
        );
    }
    if !super::is_folder_id(folder) {
        return Err(format!(
            "{folder} is not a theme folder's name: lower-case letters, digits and dashes, \
             starting with a letter, at most 32 long"
        ));
    }
    let folders = Folders::of(config_root, Some(place.plane), place.workspace);
    let kept_to = kept_to(&folders, place.workspace, folder);
    let draft = Draft {
        chat: chat.to_owned(),
        folder: folder.to_owned(),
        kept_to: kept_to.clone(),
    };
    let path = draft_folder(place.plane, &draft)?;
    if std::fs::symlink_metadata(&path).is_ok() {
        return Ok(Started {
            path,
            made: false,
            kept_to,
        });
    }
    let below_kept = kept_to.below().join(folder);
    let kept = place.plane.join(&below_kept);
    // What the real folder is as the draft starts from it: the files copied, or what is there.
    let (files, from) = if std::fs::symlink_metadata(kept.join(super::THEME_JSON)).is_ok() {
        crate::contain::no_link_on_the_way(place.plane, &kept)
            .map_err(|why| format!("{} is not read: {why}", kept.display()))?;
        let files = super::gather(&kept)?.0;
        let from = Some(started_from(&below_kept, &digest_of(&files)));
        (files, from)
    } else {
        let mut files = super::Files::new();
        files.insert(super::THEME_JSON.to_owned(), new_theme(folder).into_bytes());
        (files, real_now(place.plane, &kept_to, folder))
    };
    let (root, below) = root_of(place.plane);
    let into = root.join(below).join(chat);
    crate::plane::private_dir(&root, &into)
        .map_err(|why| format!("purlis could not make {}: {why}", into.display()))?;
    // Whose clone this chat's drafts folder is: a sweep deletes only its own (D-2708-8).
    if std::fs::symlink_metadata(into.join(OWNER)).is_err() {
        write_no_link(&root, &into.join(OWNER), &owner_of(place.plane))
            .map_err(|why| format!("purlis could not make {}: {why}", into.display()))?;
    }
    let path = super::stage(&into, folder, &files, false)
        .and_then(super::Staged::commit)
        .map_err(|why| why.to_string())?;
    // A record not written is a draft Keep asks about: it cannot tell what it started from.
    if let Some(from) = from {
        let _ = write_no_link(&root, &started_record(&into, folder), &from);
    }
    Ok(Started {
        path,
        made: true,
        kept_to,
    })
}

/// A path below the plane as the project names it: its folders joined by `/`.
fn slashed(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Where a chat's drafts folder `chat` records what its draft `folder` started from: a name no
/// theme folder takes, so no read of the drafts sees it.
fn started_record(chat: &Path, folder: &str) -> PathBuf {
    chat.join(format!(".{folder}.started"))
}

/// The digest of a theme folder's files: each path and its bytes, in order.
fn digest_of(files: &super::Files) -> String {
    let mut all = Vec::new();
    for (path, bytes) in files {
        all.extend_from_slice(path.as_bytes());
        all.push(0);
        all.extend_from_slice(bytes.len().to_string().as_bytes());
        all.push(0);
        all.extend_from_slice(bytes);
    }
    super::fonts::digest(&all)
}

/// What a draft's start records of the real folder at `below` whose files have `digest`.
fn started_from(below: &Path, digest: &str) -> String {
    format!("{}\n{digest}\n", slashed(below))
}

/// The most a start record or an owner record is, read back.
const MOST_RECORD_BYTES: u64 = 1024;

/// The record, in a chat's drafts folder, of the clone whose app made it.
const OWNER: &str = ".owner";

/// What a chat's drafts folder made by the app of the plane at `plane` names as its owner: the
/// clone's key.
fn owner_of(plane: &Path) -> String {
    format!("{}\n", crate::plane::CloneKey::of(plane))
}

/// A small record of purlis's own at `path` below `root`, read refusing a link on the way or at
/// it, and anything that is not a plain file, at most [`MOST_RECORD_BYTES`]; `None` otherwise.
fn read_small(root: &Path, path: &Path) -> Option<String> {
    use std::io::Read;
    let file = crate::contain::open_no_link(root, path).ok()?;
    if !file.metadata().ok()?.file_type().is_file() {
        return None;
    }
    let mut text = String::new();
    file.take(MOST_RECORD_BYTES)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

/// Writes `text` to the record of purlis's own at `path` below `root`, refusing a link on the way
/// or at it.
fn write_no_link(root: &Path, path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    crate::contain::create_no_link(root, path)?.write_all(text.as_bytes())
}

/// **What the real folder `folder` kept to `kept_to` is now**, as a draft's start records it:
/// where it is, and the digest of its files, or `none` when nothing is there. `None` when it
/// cannot be told: a folder that is not a theme folder's, or one that is not read.
fn real_now(plane: &Path, kept_to: &KeptTo, folder: &str) -> Option<String> {
    let below = kept_to.below().join(folder);
    let at = plane.join(&below);
    match std::fs::symlink_metadata(&at) {
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
            Some(started_from(&below, "none"))
        }
        Err(_) => None,
        Ok(_) => {
            crate::contain::no_link_on_the_way(plane, &at).ok()?;
            let (files, _) = super::gather(&at).ok()?;
            Some(started_from(&below, &digest_of(&files)))
        }
    }
}

/// **Whether the real folder `draft` is kept to is not what the draft started from**: it was
/// made, changed or deleted since. A draft with no record of its start, or a folder that cannot
/// be read, counts as changed: Keep then asks.
pub fn changed(plane: &Path, draft: &Draft) -> bool {
    let Some(chat) = folder_of(plane, &draft.chat) else {
        return true;
    };
    let (root, _) = root_of(plane);
    let Some(recorded) = read_small(&root, &started_record(&chat, &draft.folder)) else {
        return true;
    };
    real_now(plane, &draft.kept_to, &draft.folder).is_none_or(|now| now != recorded)
}

/// **The most entries of a chat's drafts folder [`has_drafts`] looks at**: every theme folder a
/// level is read for (`folders::MOST_FOLDERS`), the start record of each,
/// and the owner record.
pub const MOST_ENTRIES_LOOKED_AT: usize = 2 * super::folders::MOST_FOLDERS + 1;

/// **Whether `chat` has a theme draft** in the plane at `plane` (#2761): its drafts folder holds a
/// folder named as a theme folder is. Only that is looked at, never what the draft says, so the
/// answer costs the same whatever the draft holds, and at most [`MOST_ENTRIES_LOOKED_AT`] entries
/// are looked at. **Where it cannot tell, it answers yes**, so the close asks: a folder with more
/// entries than that and no theme folder among the first of them, or an entry that cannot be
/// read. A link on the way, at the chat's folder or in place of a theme folder counts as none.
pub fn has_drafts(plane: &Path, chat: &str) -> bool {
    let Some(folder) = folder_of(plane, chat) else {
        return false;
    };
    let (root, _) = root_of(plane);
    if crate::contain::no_link_on_the_way(&root, &folder).is_err()
        || !std::fs::symlink_metadata(&folder).is_ok_and(|meta| meta.is_dir())
    {
        return false;
    }
    let Ok(listed) = std::fs::read_dir(&folder) else {
        return false;
    };
    for (looked, entry) in listed.enumerate() {
        if looked == MOST_ENTRIES_LOOKED_AT {
            return true;
        }
        let Ok(entry) = entry else {
            return true;
        };
        if entry.file_type().is_ok_and(|kind| kind.is_dir())
            && entry.file_name().to_str().is_some_and(super::is_folder_id)
        {
            return true;
        }
    }
    false
}

/// The most chats' drafts folders one [`sweep`] looks at.
pub const MOST_SWEPT: usize = 256;

/// **Deletes the drafts of every chat that is not open** in the plane at `plane`: each chat's
/// drafts folder whose chat is none of `open`, the chats the app has open or reopens at its next
/// launch. No strip names such a draft and no close asks about it. At most [`MOST_SWEPT`] are
/// looked at in one sweep; a name that is not a chat's id is left as it is. **Only this clone's
/// are deleted**: a folder whose owner record names another clone is not this plane's to
/// delete, as in a state folder `$PURLIS_HOME` shares. **A folder with no owner record is
/// adopted only in the plane's own state folder** ([`adopts_ownerless`]): the sweep writes this
/// clone's owner record into it, open chat or not, and from then on it is this clone's. Under
/// `$PURLIS_HOME` it is left as it is. Answers how many chats' drafts were deleted.
pub fn sweep(plane: &Path, open: &[&str]) -> usize {
    let (root, below) = root_of(plane);
    let drafts = root.join(below);
    if crate::contain::no_link_on_the_way(&root, &drafts).is_err() {
        return 0;
    }
    let Ok(listed) = std::fs::read_dir(&drafts) else {
        return 0;
    };
    let mine = owner_of(plane);
    let adopts = adopts_ownerless();
    let mut deleted = 0;
    for entry in listed.filter_map(Result::ok).take(MOST_SWEPT) {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_chat_id(&name) {
            continue;
        }
        // The entry's own kind: a link is not followed, and not deleted through.
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let owner = entry.path().join(OWNER);
        // Only a record that is not there at all is no owner: one that is a link, a folder or
        // unreadable names nobody this sweep may replace, and the folder stays.
        let ownerless = std::fs::symlink_metadata(&owner)
            .is_err_and(|why| why.kind() == std::io::ErrorKind::NotFound);
        if adopts && ownerless {
            let _ = write_no_link(&root, &owner, &mine);
        }
        if open.contains(&name.as_str()) {
            continue;
        }
        if read_small(&root, &owner).as_deref() != Some(mine.as_str()) {
            continue;
        }
        // The standard library's removal opens each folder below refusing a link.
        if std::fs::remove_dir_all(entry.path()).is_ok() {
            deleted += 1;
        }
    }
    deleted
}

/// **Whether a sweep adopts a chat's drafts folder that names no clone** (#2761): only while
/// `$PURLIS_HOME` is not set, so that the drafts are in the plane's own state folder, inside the
/// plane, which is this clone's own. Never under `$PURLIS_HOME`, inside the plane or not: an
/// operator sets it to share one state folder between clones, and a folder there that names no
/// clone may be another clone's open chat's. A `$PURLIS_HOME` another clone sets to this plane's
/// own state folder counts as this clone's own.
fn adopts_ownerless() -> bool {
    !crate::steer::var_os("PURLIS_HOME").is_some_and(|home| !home.is_empty())
}

/// The most files a save hands over: `theme.json` and the icon sets beside it.
pub const MOST_SAVED_FILES: usize = 8;

/// **The most a save's files are, as JSON writes them**: what one ask carries over the chat's
/// hook socket, with room for the rest of the ask, under the most one line holds. Purlis
/// Classic's whole `theme.json`, both appearances and every token, is about a fifth of it.
pub const MOST_SAVED_JSON_BYTES: usize = 64 * 1024;

/// Why `files` are not handed over in one save: more than [`MOST_SAVED_JSON_BYTES`] as JSON
/// writes them. What the command checks before it sends, and the app again as it writes.
pub fn too_large(files: &BTreeMap<String, String>) -> Option<String> {
    let bytes = serde_json::to_string(files).map_or(usize::MAX, |json| json.len());
    (bytes > MOST_SAVED_JSON_BYTES).then(|| {
        format!(
            "the draft's theme files are {bytes} bytes as a save hands them over, and a save \
             carries at most {MOST_SAVED_JSON_BYTES}: make theme.json smaller (an extends, or a \
             palette, says much in little)"
        )
    })
}

/// What [`save`] did: where the draft is, and what the window draws of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Saved {
    pub path: PathBuf,
    /// Whether the draft was made by this save: it was not there before.
    pub made: bool,
    /// Where it is kept to.
    pub kept_to: KeptTo,
    /// The hard floors it breaks, which keep it from being drawn.
    pub broken: Vec<BrokenIn>,
    /// Why the engine refused it, when it did.
    pub refused: Vec<Complaint>,
}

/// Why a file a save hands over is not taken, or `None`: only `theme.json` and an icon set
/// (`icons/….json`), each a theme file under the one cap.
fn not_saved(path: &str, text: &str) -> Option<String> {
    let shown = crate::shown::short(path);
    let icon_set = path.starts_with("icons/") && path.ends_with(".json");
    if path != super::THEME_JSON && !icon_set {
        return Some(format!(
            "{shown} is not a file a save hands over: theme.json and icon sets under icons/ are"
        ));
    }
    if let Some(why) = super::portable::path_refusal(path, path) {
        return Some(why);
    }
    let most = crate::windowprefs::MAX_BYTES;
    (text.len() as u64 > most).then(|| {
        format!(
            "{shown} is {} bytes, and a theme file is at most {most}",
            text.len()
        )
    })
}

/// **Saves `chat`'s draft of `folder`** at `place`: `files` (its `theme.json`, and the icon sets
/// beside it) replace those of the draft, which is started first when it is not there; the
/// draft's other files (its fonts, as approved where it is kept to, a licence) stay as they are.
/// `chat` is the app's record of the chat that asked. Answers what the window draws of the draft
/// now: the hard floors it breaks, or why it is refused.
pub fn save(
    config_root: Option<&Path>,
    place: Place<'_>,
    chat: &str,
    folder: &str,
    files: &BTreeMap<String, String>,
) -> Result<Saved, String> {
    if !files.contains_key(super::THEME_JSON) {
        return Err("a save hands over the draft's theme.json".to_owned());
    }
    if files.len() > MOST_SAVED_FILES {
        return Err(format!(
            "a save hands over at most {MOST_SAVED_FILES} files: theme.json and its icon sets"
        ));
    }
    if let Some(why) = files
        .iter()
        .find_map(|(path, text)| not_saved(path, text))
        .or_else(|| too_large(files))
    {
        return Err(why);
    }
    let started = start(config_root, place, chat, folder)?;
    let (mut merged, _) = super::gather(&started.path)?;
    merged.retain(|path, _| path != super::THEME_JSON && !path.starts_with("icons/"));
    for (path, text) in files {
        merged.insert(path.clone(), text.clone().into_bytes());
    }
    let into = started
        .path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "the theme draft has no folder".to_owned())?;
    let path = super::stage(&into, folder, &merged, true)
        .and_then(super::Staged::commit)
        .map_err(|why| why.to_string())?;
    let draft = Draft {
        chat: chat.to_owned(),
        folder: folder.to_owned(),
        kept_to: started.kept_to.clone(),
    };
    let drafted = drafted(
        read(config_root, place, &draft, true),
        None,
        None,
        Appearance::Dark,
        Contrast::Standard,
    );
    Ok(Saved {
        path,
        made: started.made,
        kept_to: started.kept_to,
        broken: drafted.broken,
        refused: drafted.refused,
    })
}

/// A new theme's `theme.json`: its name, and the default theme, Purlis, to extend (#3034).
fn new_theme(folder: &str) -> String {
    let theme = serde_json::json!({
        "$schema": super::SCHEMA_URL,
        "name": folder,
        "extends": super::DEFAULT,
    });
    format!(
        "{}\n",
        serde_json::to_string_pretty(&theme).unwrap_or_default()
    )
}

/// What a move up a level did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedUp {
    /// The theme folder it is now.
    pub path: PathBuf,
    /// The font files left behind, by name, each said: a copy to the machine carries only those
    /// approved with the project.
    pub left_out: Vec<String>,
}

/// **Use in the whole project**: moves the workspace's theme folder `folder` to the project's
/// `themes/`. Never over a folder of that name there: that is refused, and nothing moves. Its
/// fonts are at a new path in the project, so they are asked about again with the project.
pub fn move_to_project(plane: &Path, workspace: &str, folder: &str) -> Result<MovedUp, String> {
    if !crate::contain::workspace_name_ok(workspace) || !super::is_folder_id(folder) {
        return Err(format!("{folder} is not a theme folder of a workspace"));
    }
    let from = plane
        .join(KeptTo::Workspace(workspace.to_owned()).below())
        .join(folder);
    crate::contain::no_link_on_the_way(plane, &from)
        .map_err(|why| format!("{} is not read: {why}", from.display()))?;
    let (files, _) = super::gather(&from)?;
    let into = plane.join(THEMES);
    crate::contain::no_link_on_the_way(plane, &into)
        .map_err(|why| format!("{} is not written: {why}", into.display()))?;
    let path = super::stage(&into, folder, &files, false)
        .and_then(super::Staged::commit)
        .map_err(|why| match why {
            super::NotWritten::Exists(path) => format!(
                "the project has a theme folder {folder} already ({}), so nothing was moved",
                path.display()
            ),
            other => other.to_string(),
        })?;
    std::fs::remove_dir_all(&from).map_err(|why| {
        format!(
            "{folder} is in the project's themes now, and purlis could not delete it from the \
             workspace: {why}"
        )
    })?;
    Ok(MovedUp {
        path,
        left_out: Vec::new(),
    })
}

/// **Copy to my machine**: copies the theme folder `folder` of the project (`workspace` `None`)
/// or of a workspace to the person's own `themes/` in purlis's config folder. Never over a
/// folder of that name there. **Only the font files this machine approved with the project are
/// carried**, each with the digest approved: the person's own folders need no approval, so a
/// file not approved stays behind, and is said.
pub fn copy_to_machine(
    config_root: &Path,
    plane: &Path,
    workspace: Option<&str>,
    folder: &str,
) -> Result<MovedUp, String> {
    if workspace.is_some_and(|name| !crate::contain::workspace_name_ok(name))
        || !super::is_folder_id(folder)
    {
        return Err(format!("{folder} is not a theme folder of this project"));
    }
    let kept_to = workspace.map_or(KeptTo::Project, |name| KeptTo::Workspace(name.to_owned()));
    let below = kept_to.below().join(folder);
    let from = plane.join(&below);
    crate::contain::no_link_on_the_way(plane, &from)
        .map_err(|why| format!("{} is not read: {why}", from.display()))?;
    let (mut files, _) = super::gather(&from)?;
    let approved = approved_fonts(Some(config_root), Some(plane));
    let in_project = slashed(&below);
    let mut left_out = Vec::new();
    files.retain(|path, bytes| {
        if !path.starts_with("fonts/") {
            return true;
        }
        let digest = super::fonts::digest(bytes);
        let listed = approved.get(&format!("{in_project}/{path}"));
        let ok = listed.is_some_and(|listed| listed.split(' ').next() == Some(digest.as_str()));
        if !ok {
            left_out.push(format!(
                "{path} is not a font file you approved with this project, so it is not copied"
            ));
        }
        ok
    });
    let dir = crate::machine::private_dir(config_root)
        .map_err(|why| format!("purlis could not make its config folder: {why}"))?;
    let into = dir.join(THEMES);
    crate::contain::no_link_on_the_way(config_root, &into)
        .map_err(|why| format!("{} is not written: {why}", into.display()))?;
    let path = super::stage(&into, folder, &files, false)
        .and_then(super::Staged::commit)
        .map_err(|why| match why {
            super::NotWritten::Exists(path) => format!(
                "you have a theme folder {folder} already ({}), so nothing was copied",
                path.display()
            ),
            other => other.to_string(),
        })?;
    Ok(MovedUp { path, left_out })
}
