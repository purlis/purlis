// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A workspace's artifacts (ADR 0120, spec #5948): HTML templates in its `artifacts/` folder,
//! each made an artifact by a pin in `artifacts/pins.toml` (`docs/plane-format.md`).
//!
//! This module pins, unpins, lists and reorders them, and resolves a pin to its folder. It
//! renders nothing and runs nothing: that is the window's, and charterd's.
//!
//! **A pin never leads out of `artifacts/`.** The folder a pin names is one plain name, so no
//! `..` or separator reaches the file system, and the folder is then asked where it really is
//! once every link is followed: a folder, or `artifacts/` itself, that a link takes out of the
//! workspace is refused with a sentence and nothing is written.

pub mod approval;
pub mod badge;
pub mod calls;
pub mod check;
pub mod data;
pub mod discuss;
pub mod fresh;
pub mod history;
pub mod kit;
pub mod links;
pub mod live;
pub mod offers;
pub mod pins;
pub mod places;
pub mod regenerate;
pub mod script;
pub mod search;
pub mod serve;

use std::io;
use std::path::{Path, PathBuf};

pub use pins::{Pin, Pins};

/// The workspace's folder of artifacts.
pub const DIR: &str = "artifacts";

/// The pins file inside it.
pub const PINS: &str = "pins.toml";

/// The page an artifact folder opens on.
pub const PAGE: &str = "index.html";

/// The data file of an artifact folder: inert JSON, each person's own, never committed.
pub const DATA_FILE: &str = "data.json";

/// The folder of an artifact's further data files, never committed either.
pub const DATA_DIR: &str = "data";

/// The longest name a pin's icon may carry.
const NAME_MOST: usize = 60;

/// **The most bytes an artifact's folder may hold** (spec #5948, #5954): every file of it, data
/// files too, since all of it is read into memory to be served. Below the plane save's
/// [`crate::planegit::SCAN_LIMIT`], so nothing a pinned folder holds is too big for the secret
/// scan to read. A folder over it is not pinned, and a pinned one that grew past it is shown as
/// too large, with both sizes, rather than cut.
pub const SIZE_CAP: u64 = 8 << 20;

const _: () = assert!(SIZE_CAP < crate::planegit::SCAN_LIMIT as u64);

/// What a folder over [`SIZE_CAP`] is said to be: its size and the cap, both.
pub fn too_large(folder: &str, size: u64) -> String {
    format!(
        "'{}' is {}, over the {} an artifact may be",
        crate::personas::one_line(folder),
        crate::browserfiles::bytes_said(size),
        crate::browserfiles::bytes_said(SIZE_CAP)
    )
}

/// One pin as `list` shows it: the pin, and why its folder cannot be opened, if it cannot.
#[derive(Clone, Debug)]
pub struct Listed {
    pub pin: Pin,
    /// `None` for a pin whose folder is there and inside `artifacts/`; otherwise the sentence.
    pub missing: Option<String>,
    /// For a folder that is there but over [`SIZE_CAP`], the sentence naming both sizes.
    pub too_large: Option<String>,
}

/// What `pin` asks for. Everything but the folder may be left for purlis to choose.
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub folder: String,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

/// What `pin` did.
#[derive(Clone, Debug)]
pub enum Pinned {
    /// A pin that was not there before.
    New(Pin),
    /// The folder's pin, with its name or icon changed (or nothing changed).
    Updated(Pin),
    /// The folder was pinned with another command or working folder: that pin is gone and this
    /// new one, under a new id, holds its place.
    Replaced { new: Pin, old_id: String },
}

/// The icon a pin gets when none is named.
pub const DEFAULT_ICON: &str = "chart";

fn pins_path(workspace: &Path) -> PathBuf {
    workspace.join(DIR).join(PINS)
}

/// Read the workspace's pins file; no file is no pins.
pub fn read(workspace: &Path) -> Result<Pins, String> {
    match std::fs::read_to_string(pins_path(workspace)) {
        Ok(text) => pins::parse(&text),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Pins::default()),
        Err(e) => Err(format!(
            "could not read the pins file ({})",
            crate::rewrite::os_words(&e)
        )),
    }
}

/// The workspace's pins in their order, each with whether its folder can be opened.
pub fn list(workspace: &Path) -> Result<Vec<Listed>, String> {
    Ok(read(workspace)?
        .pins
        .into_iter()
        .map(|pin| {
            let (missing, over) = match resolve(workspace, &pin.folder) {
                Err(why) => (Some(why), None),
                Ok(real) => match serve::size(&real) {
                    Err(why) => (Some(why), None),
                    Ok(size) => (
                        None,
                        (size > SIZE_CAP).then(|| {
                            format!("{}, so none of it is shown", too_large(&pin.folder, size))
                        }),
                    ),
                },
            };
            Listed {
                pin,
                missing,
                too_large: over,
            }
        })
        .collect())
}

/// Where artifact folder `folder` of the workspace at `workspace` is, once every link is
/// followed — or the sentence saying why it is not one a pin may name.
pub fn resolve(workspace: &Path, folder: &str) -> Result<PathBuf, String> {
    if let Some(why) = folder_refusal(folder) {
        return Err(why);
    }
    let shown = crate::personas::one_line(folder);
    let home = workspace.canonicalize().map_err(|e| {
        format!(
            "the workspace cannot be read ({})",
            crate::rewrite::os_words(&e)
        )
    })?;
    let dir = workspace.join(DIR);
    let artifacts = dir
        .canonicalize()
        .map_err(|_| format!("there is no {DIR}/ folder in this workspace to hold '{shown}'"))?;
    if !artifacts.starts_with(&home) || artifacts == home {
        return Err(format!(
            "{DIR}/ is a link that leads out of the workspace, so nothing in it is pinned"
        ));
    }
    let at = dir.join(folder);
    if at.symlink_metadata().is_err() {
        return Err(format!("there is no folder '{shown}' in {DIR}/"));
    }
    let real = at
        .canonicalize()
        .map_err(|_| format!("'{shown}' in {DIR}/ is a link that leads nowhere"))?;
    if !real.starts_with(&artifacts) || real == artifacts {
        return Err(format!(
            "'{shown}' is a link that leads out of {DIR}/, and only a folder inside it can be \
             pinned"
        ));
    }
    if !real.is_dir() {
        return Err(format!("'{shown}' in {DIR}/ is not a folder"));
    }
    let page = real.join(PAGE);
    match page.canonicalize() {
        Ok(page) if page.starts_with(&real) && page.is_file() => Ok(real),
        Ok(_) => Err(format!(
            "'{shown}'s {PAGE} is a link that leads out of its folder"
        )),
        Err(_) => Err(format!(
            "'{shown}' has no {PAGE}, the page an artifact opens on"
        )),
    }
}

/// Why `folder` cannot name an artifact folder at all: it must be one plain name.
fn folder_refusal(folder: &str) -> Option<String> {
    let plain = !folder.is_empty()
        && !folder.starts_with('.')
        && folder
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    (!plain).then(|| {
        format!(
            "'{}' is not the name of a folder in {DIR}/: name one folder there by its name \
             alone (letters, digits, '.', '-', '_', not starting with a dot), never a path",
            crate::personas::one_line(folder)
        )
    })
}

/// Pin artifact folder `request.folder` to the workspace at `workspace` in the plane at `root`.
///
/// The folder is resolved first ([`resolve`]), then read whole and scanned ([`scanned`]): a
/// folder over [`SIZE_CAP`], or one holding something shaped like a secret, is refused. A
/// refusal writes nothing. Pinning a folder
/// already pinned with the same command and working folder changes its name or icon; with
/// another command or working folder it is a new pin, which takes the old one's place.
pub fn pin(root: &Path, workspace: &Path, request: &Request) -> Result<Pinned, String> {
    let real = resolve(workspace, &request.folder)?;
    scanned(&real, &request.folder)?;
    let command = request
        .command
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if let Some(command) = command
        && command.chars().any(char::is_control)
    {
        return Err("a pin's command is one line".into());
    }
    let cwd = request
        .cwd
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty());
    if let Some(cwd) = cwd
        && let Some(why) = cwd_refusal(cwd)
    {
        return Err(why);
    }
    let icon = request.icon.as_deref().unwrap_or(DEFAULT_ICON);
    if !crate::personamark::ICONS.contains(&icon) {
        return Err(format!(
            "'{}' is not one of purlis's icons; choose one of: {}",
            crate::personas::one_line(icon),
            crate::personamark::ICONS.join(", ")
        ));
    }
    let name = match request.name.as_deref() {
        Some(name) => checked_name(name)?,
        None => title_of(&real.join(PAGE)).unwrap_or_else(|| request.folder.clone()),
    };
    let id = pins::id_of(&request.folder, command, cwd);
    let mut done = None;
    write(root, workspace, |pins| {
        let old = pins
            .pins
            .iter()
            .position(|p| p.folder == request.folder)
            .map(|at| pins.pins.remove(at));
        let order = old
            .as_ref()
            .map_or_else(|| pins.next_order(), |old| old.order);
        let mut pin = Pin::new(
            request.folder.clone(),
            name.clone(),
            icon.to_string(),
            order,
            command.map(str::to_string),
            cwd.map(str::to_string),
        );
        done = Some(match old {
            Some(old) if old.id == id => {
                let mut kept = old;
                kept.name = pin.name;
                kept.icon = pin.icon;
                pin = kept;
                Pinned::Updated(pin.clone())
            }
            Some(old) => Pinned::Replaced {
                new: pin.clone(),
                old_id: old.id,
            },
            None => Pinned::New(pin.clone()),
        });
        pins.pins.push(pin);
        Ok(())
    })?;
    Ok(done.expect("the change ran"))
}

/// **The secret check of a folder about to be pinned** (#5954): every file of its template read
/// ([`serve::Snapshot::read`], which refuses a folder over [`SIZE_CAP`] before it reads it) and
/// asked what the plane save asks of a staged file ([`crate::secretshape::found_as_read`]), so
/// pinning and saving never disagree. The data files are not asked: they are never committed.
pub(crate) fn scanned(real: &Path, folder: &str) -> Result<(), String> {
    let shown = crate::personas::one_line(folder);
    let size = serve::size(real)?;
    if size > SIZE_CAP {
        return Err(format!(
            "{}, so it is not pinned. Make its folder smaller, then pin it again.",
            too_large(folder, size)
        ));
    }
    let snapshot = serve::Snapshot::read(real, folder)?;
    for path in snapshot.paths().filter(|path| !serve::is_data(path)) {
        let bytes = snapshot.get(path).unwrap_or_default();
        let text = String::from_utf8_lossy(bytes);
        let form = crate::secretshape::Structured::of(path);
        if let Some((line, kind)) = crate::secretshape::found_as_read(form, &text) {
            let at = line
                .map(|line| format!(", line {line}"))
                .unwrap_or_default();
            return Err(format!(
                "'{shown}' holds something shaped like a secret ({kind}) in {}{at}, so it is \
                 not pinned: a pinned page goes wherever its workspace does. Keep the secret in \
                 the vault (`purlis persona secret set`), take it out of the folder, then pin it \
                 again.",
                crate::personas::one_line(path)
            ));
        }
    }
    Ok(())
}

/// Unpin the pin named by `key` (its id or its folder); its folder stays.
pub fn unpin(root: &Path, workspace: &Path, key: &str) -> Result<Pin, String> {
    let mut gone = None;
    write(root, workspace, |pins| {
        let at = find(pins, key)?;
        gone = Some(pins.pins.remove(at));
        Ok(())
    })?;
    Ok(gone.expect("the change ran"))
}

/// Put the pins named by `keys` (ids or folders) first, in that order, numbered from 1, and
/// every other pin after them in the order it had. The pins as they are now.
pub fn reorder(root: &Path, workspace: &Path, keys: &[String]) -> Result<Vec<Pin>, String> {
    if keys.is_empty() {
        return Err("name the pins in the order they should take".into());
    }
    let mut now = Vec::new();
    write(root, workspace, |pins| {
        let mut first = Vec::new();
        for key in keys {
            let at = find(pins, key)?;
            first.push(pins.pins.remove(at));
        }
        let rest = std::mem::take(&mut pins.pins);
        for (n, mut pin) in first.into_iter().chain(rest).enumerate() {
            pin.order = u32::try_from(n + 1).unwrap_or(u32::MAX);
            pins.pins.push(pin);
        }
        now = pins.pins.clone();
        Ok(())
    })?;
    Ok(now)
}

/// Where the pin named by `key` (an id or a folder) is in `pins`.
fn find(pins: &Pins, key: &str) -> Result<usize, String> {
    pins.pins
        .iter()
        .position(|p| p.id == key || p.folder == key)
        .ok_or_else(|| {
            format!(
                "no pin '{}' in this workspace (`purlis artifact list` shows them)",
                crate::personas::one_line(key)
            )
        })
}

/// Read the pins file, change it, and write it back sorted, under the folder's lock and whole.
fn write(
    root: &Path,
    workspace: &Path,
    change: impl FnOnce(&mut Pins) -> Result<(), String>,
) -> Result<(), String> {
    let path = pins_path(workspace);
    crate::rewrite::update(root, &path, |text| {
        let mut pins = match text {
            Some(text) => pins::parse(text).map_err(io::Error::other)?,
            None => Pins::default(),
        };
        if let Some(why) = pins.unwritable() {
            return Err(io::Error::other(why));
        }
        change(&mut pins).map_err(io::Error::other)?;
        pins.sort();
        Ok(Some(pins.render()))
    })
    .map(|_| ())
    .map_err(|e| crate::rewrite::os_words(&e))
}

/// The name a person gave a pin, held to one line of at most [`NAME_MOST`] characters.
fn checked_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err("a pin's name is one line of text".into());
    }
    if name.chars().count() > NAME_MOST {
        return Err(format!(
            "a pin's name is at most {NAME_MOST} characters; this one has {}",
            name.chars().count()
        ));
    }
    Ok(name.to_string())
}

/// Why `cwd` cannot be a command's working folder: workspace-relative, plain components.
fn cwd_refusal(cwd: &str) -> Option<String> {
    let plain = cwd == "."
        || (!cwd.starts_with('/')
            && !cwd.contains('\\')
            && cwd
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".."));
    (!plain).then(|| {
        format!(
            "'{}' is not a working folder inside the workspace: give it relative to the \
             workspace, with no '..'",
            crate::personas::one_line(cwd)
        )
    })
}

/// **Who pinned pin `id`**, by the plane's history (ADR 0120 §1: the bar names who pinned it,
/// from git history): the author of the first commit whose pins file gained the id. `None` when
/// no commit holds it yet (pinned here and not saved, or a workspace git does not keep).
pub fn pinned_by(workspace: &Path, id: &str) -> Option<String> {
    let path = format!("{DIR}/{PINS}");
    let quoted = format!("\"{id}\"");
    let run = crate::worktree::git::run(
        workspace,
        &[
            "log",
            "--reverse",
            "--format=%an",
            "-S",
            &quoted,
            "--",
            &path,
        ],
        std::time::Duration::from_secs(5),
    )
    .ok()?;
    if !run.ok() {
        return None;
    }
    run.out
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(crate::personas::one_line)
}

/// The longest folder name [`free_folder`] makes.
const FOLDER_MOST: usize = 48;

/// **The name a pin of a page with text `page` is offered** (#5953): its `<title>`, else the
/// folder's name. The person may change it before pinning.
pub fn suggested_name(page: &str, folder: &str) -> String {
    title_in(page).unwrap_or_else(|| folder.to_string())
}

/// **A folder in the workspace's `artifacts/` that is free to take a page named `name`**
/// (#5953): the name in lowercase ASCII letters and digits joined by `-`, and `-2`, `-3`… after
/// it while a folder of that name is there. `page` for a name with nothing plain in it.
pub fn free_folder(workspace: &Path, name: &str) -> String {
    let mut plain = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            plain.push(c.to_ascii_lowercase());
        } else if !plain.is_empty() && !plain.ends_with('-') {
            plain.push('-');
        }
    }
    let mut stem: String = plain
        .trim_end_matches('-')
        .chars()
        .take(FOLDER_MOST)
        .collect();
    stem = stem.trim_end_matches('-').to_string();
    if stem.is_empty() {
        stem = "page".to_string();
    }
    let dir = workspace.join(DIR);
    let taken = |folder: &str| dir.join(folder).symlink_metadata().is_ok();
    if !taken(&stem) {
        return stem;
    }
    (2..)
        .map(|n| format!("{stem}-{n}"))
        .find(|folder| !taken(folder))
        .expect("some number is free")
}

/// **Puts a page into a new artifact folder** (#5953, spec story 15): `page` becomes
/// `artifacts/<folder>/index.html` of the workspace at `workspace`, so a page from anywhere
/// can be pinned. Never over a folder that is there: that is refused, and nothing is written.
pub fn adopt(root: &Path, workspace: &Path, folder: &str, page: &str) -> Result<(), String> {
    if let Some(why) = folder_refusal(folder) {
        return Err(why);
    }
    let shown = crate::personas::one_line(folder);
    let dir = workspace.join(DIR);
    std::fs::create_dir_all(&dir).map_err(|e| {
        format!(
            "purlis could not make {DIR}/ in the workspace ({})",
            crate::rewrite::os_words(&e)
        )
    })?;
    let at = dir.join(folder);
    match std::fs::create_dir(&at) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(format!(
                "there is already a folder '{shown}' in {DIR}/; choose another name"
            ));
        }
        Err(e) => {
            return Err(format!(
                "purlis could not make '{shown}' in {DIR}/ ({})",
                crate::rewrite::os_words(&e)
            ));
        }
    }
    // The folder is new and the workspace's own, so it holds nothing a link could lead out of.
    crate::rewrite::replace(
        root,
        &at.join(PAGE),
        page.as_bytes(),
        crate::rewrite::Mode::Kept,
    )
    .map_err(|e| {
        let _ = std::fs::remove_dir(&at);
        format!(
            "purlis could not write the page into '{shown}' ({})",
            crate::rewrite::os_words(&e)
        )
    })
}

/// The page's `<title>`, as a pin's name: its first one, whitespace folded, cut to
/// [`NAME_MOST`] characters. None when it has none.
fn title_of(page: &Path) -> Option<String> {
    title_in(&std::fs::read_to_string(page).ok()?)
}

/// [`title_of`], of a page's text.
fn title_in(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find("</title")?;
    let title: String = text[start..end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .take(NAME_MOST)
        .collect();
    (!title.is_empty()).then_some(title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_becomes_the_name_folded_and_cut() {
        let dir = tempfile::tempdir().unwrap();
        let page = dir.path().join(PAGE);
        std::fs::write(
            &page,
            "<html><head><TITLE lang=en>\n  Backlog   state\n</TITLE>",
        )
        .unwrap();
        assert_eq!(title_of(&page).as_deref(), Some("Backlog state"));
        std::fs::write(&page, format!("<title>{}</title>", "x".repeat(90))).unwrap();
        assert_eq!(title_of(&page).unwrap().len(), NAME_MOST);
        std::fs::write(&page, "<title> </title>").unwrap();
        assert_eq!(title_of(&page), None);
    }

    #[test]
    fn who_pinned_is_the_author_of_the_commit_that_added_the_pin_and_none_before_it_is_saved() {
        use crate::testgit::run as git;
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        std::fs::create_dir_all(ws.join(DIR)).unwrap();
        assert!(git(ws, &["init", "-q"]).ok());
        let pins = ws.join(DIR).join(PINS);
        std::fs::write(&pins, "version = 1\npin = [\n{ id = \"abc123\" },\n]\n").unwrap();
        assert_eq!(pinned_by(ws, "abc123"), None, "not committed yet");

        let commit = |who: &str, message: &str| {
            assert!(
                git(
                    ws,
                    &[
                        "-c",
                        &format!("user.name={who}"),
                        "-c",
                        "user.email=x@example.com",
                        "commit",
                        "-qam",
                        message,
                    ]
                )
                .ok()
            );
        };
        assert!(git(ws, &["add", "-A"]).ok());
        commit("Ada", "pin");
        std::fs::write(
            &pins,
            "version = 1\npin = [\n{ id = \"abc123\" },\n{ id = \"def456\" },\n]\n",
        )
        .unwrap();
        commit("Grace", "another pin");

        assert_eq!(pinned_by(ws, "abc123").as_deref(), Some("Ada"));
        assert_eq!(pinned_by(ws, "def456").as_deref(), Some("Grace"));
        assert_eq!(pinned_by(ws, "nothere"), None);
    }

    #[test]
    fn a_free_folder_is_the_name_made_plain_and_never_one_already_there() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        assert_eq!(free_folder(ws, "Backlog state"), "backlog-state");
        assert_eq!(free_folder(ws, "  Weekly / ../ report!  "), "weekly-report");
        assert_eq!(free_folder(ws, "Café"), "caf");
        assert_eq!(free_folder(ws, "..."), "page");
        std::fs::create_dir_all(ws.join(DIR).join("backlog-state")).unwrap();
        std::fs::create_dir_all(ws.join(DIR).join("backlog-state-2")).unwrap();
        assert_eq!(free_folder(ws, "Backlog state"), "backlog-state-3");
        assert!(folder_refusal(&free_folder(ws, &"x".repeat(200))).is_none());
    }

    #[test]
    fn a_page_is_adopted_into_a_new_folder_and_never_over_one_already_there() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path();
        adopt(ws, ws, "report", "<title>Report</title>").unwrap();
        let page = ws.join(DIR).join("report").join(PAGE);
        assert_eq!(
            std::fs::read_to_string(&page).unwrap(),
            "<title>Report</title>"
        );
        assert!(resolve(ws, "report").is_ok());

        let again = adopt(ws, ws, "report", "<p>other</p>").unwrap_err();
        assert!(again.contains("already"), "{again}");
        assert_eq!(
            std::fs::read_to_string(&page).unwrap(),
            "<title>Report</title>",
            "nothing is written over a folder that is there"
        );
        assert!(adopt(ws, ws, "../out", "x").is_err());
        assert!(!ws.join("out").exists());
    }

    #[test]
    fn a_page_s_text_names_its_pin_from_its_title_or_its_folder() {
        assert_eq!(
            suggested_name("<title> Backlog\n state </title>", "backlog"),
            "Backlog state"
        );
        assert_eq!(suggested_name("<p>no title</p>", "backlog"), "backlog");
    }

    #[test]
    fn a_working_folder_is_relative_with_plain_parts() {
        for good in [".", "sub", "a/b"] {
            assert!(cwd_refusal(good).is_none(), "{good}");
        }
        for bad in ["/abs", "..", "a/../b", "a//b", "./a", "a\\b"] {
            assert!(cwd_refusal(bad).is_some(), "{bad}");
        }
    }
}
