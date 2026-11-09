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
pub mod data;
pub mod pins;
pub mod script;
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

/// One pin as `list` shows it: the pin, and why its folder cannot be opened, if it cannot.
#[derive(Clone, Debug)]
pub struct Listed {
    pub pin: Pin,
    /// `None` for a pin whose folder is there and inside `artifacts/`; otherwise the sentence.
    pub missing: Option<String>,
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
        .map(|pin| Listed {
            missing: resolve(workspace, &pin.folder).err(),
            pin,
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
/// The folder is resolved first ([`resolve`]); a refusal writes nothing. Pinning a folder
/// already pinned with the same command and working folder changes its name or icon; with
/// another command or working folder it is a new pin, which takes the old one's place.
pub fn pin(root: &Path, workspace: &Path, request: &Request) -> Result<Pinned, String> {
    let real = resolve(workspace, &request.folder)?;
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

/// The page's `<title>`, as a pin's name: its first one, whitespace folded, cut to
/// [`NAME_MOST`] characters. None when it has none.
fn title_of(page: &Path) -> Option<String> {
    let text = std::fs::read_to_string(page).ok()?;
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
    fn a_working_folder_is_relative_with_plain_parts() {
        for good in [".", "sub", "a/b"] {
            assert!(cwd_refusal(good).is_none(), "{good}");
        }
        for bad in ["/abs", "..", "a/../b", "a//b", "./a", "a\\b"] {
            assert!(cwd_refusal(bad).is_some(), "{bad}");
        }
    }
}
