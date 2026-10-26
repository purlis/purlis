// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's conversation, carried to the device its chat moves to** (#2048, spec #2041): its
//! harness keeps it in files its declaration names (`[conversation] file`, ADR 0073 as amended
//! for #2031), which the device the chat leaves finds ([`find`]) and the device it moves to
//! places in its own harness folder ([`place_of`], [`keep`]), where the harness resumes it by its
//! id.
//!
//! **What crosses is a file and its shape, never a path.** The shape is the declaration's file
//! with every `*` part as it was found and `{cwd_key}` kept as written: the device that takes it
//! holds it to its own declaration of the harness, part by part, and makes `{cwd_key}` again from
//! the directory the chat runs in there. So a peer names no folder, no `..` and no link: where
//! the file goes is the taking device's own harness folder for the profile, and nothing else.
//!
//! **Only a conversation that is one file moves today.** A declaration that keeps none, one
//! whose conversation needs the session it was forked from (Codex), and one exported through the
//! harness's own commands (opencode) are refused in a sentence, and their chats do not move.

use std::path::{Path, PathBuf};

use crate::harness::SessionId;
use crate::harness_declaration::{Conversation, Declaration};

/// The most a carried conversation may be: what a code transfer carries
/// (`purlis_session_protocol::code::MOST_BUNDLE_BYTES`), far past a long transcript.
pub const MOST_BYTES: u64 = 1024 * 1024 * 1024;

/// The folders a profile's harness keeps its files in on one device: what `{config}` and
/// `{data}` in its declaration's `[conversation] file` stand for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folders {
    pub config: PathBuf,
    pub data: Option<PathBuf>,
}

impl Folders {
    /// A harness that keeps everything in one folder.
    pub fn config(config: &Path) -> Self {
        Folders {
            config: config.to_path_buf(),
            data: None,
        }
    }

    /// **Where a chat on `profile` keeps its harness's files on this device**, as its program
    /// reaches them: the folder each of the declaration's `[folders]` variables names, from the
    /// profile's own environment first, then `env` (what the chat inherits), else the harness's
    /// own default under `home`. `None` for a harness that declares no folder, or a variable that
    /// names no absolute path.
    pub fn for_profile(
        profile: &crate::profiles::Profile,
        declaration: &Declaration,
        home: &Path,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Option<Folders> {
        let defaults = crate::profilestore::defaults(&declaration.name, home);
        let named = |name: Option<&str>, default: Option<PathBuf>| -> Option<PathBuf> {
            let set = name.and_then(|name| {
                profile
                    .env
                    .iter()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.clone())
                    .or_else(|| env(name))
                    .filter(|value| !value.is_empty())
            });
            match set {
                Some(value) => {
                    let value = PathBuf::from(value);
                    value.is_absolute().then_some(value)
                }
                None => default,
            }
        };
        let config_env = declaration.folders.config_env.as_deref()?;
        let config = named(
            Some(config_env),
            defaults.as_ref().map(|(config, _, _)| config.clone()),
        )?;
        let data = match &declaration.folders.data_env {
            Some(data_env) => named(
                Some(data_env),
                defaults.as_ref().and_then(|(_, data, _)| data.clone()),
            ),
            None => None,
        };
        Some(Folders { config, data })
    }

    fn of(&self, root: &str) -> Option<&Path> {
        match root {
            "{config}" => Some(&self.config),
            "{data}" => self.data.as_deref(),
            _ => None,
        }
    }
}

/// Why a conversation was not carried, in a sentence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// The harness keeps its conversations in a way that does not move yet.
    #[error("{0}")]
    DoesNotMove(String),
    /// There is no such conversation where its harness keeps it.
    #[error("{0}")]
    NotFound(String),
    /// A shape or a place this device does not take.
    #[error("{0}")]
    NotAllowed(String),
    /// The file could not be read or written.
    #[error("{0}")]
    Io(String),
}

/// A conversation found where its harness keeps it on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The file.
    pub path: PathBuf,
    /// The declaration's file with each `*` part as it was found and `{cwd_key}` as written:
    /// what the other device is told, and places by ([`place_of`]).
    pub shape: String,
}

/// The one file `declaration` keeps a conversation in, or why its conversations do not move.
fn one_file(declaration: &Declaration) -> Result<&str, Refused> {
    match &declaration.conversation {
        Some(Conversation::File {
            file,
            with_forked_from: false,
        }) => Ok(file),
        Some(Conversation::File { .. }) => Err(Refused::DoesNotMove(format!(
            "{} keeps a conversation with the one it was forked from, and purlis does not move \
             such a conversation yet",
            declaration.title
        ))),
        Some(Conversation::Export { .. }) => Err(Refused::DoesNotMove(format!(
            "{} moves a conversation through its own export and import, which purlis does not \
             run yet",
            declaration.title
        ))),
        None => Err(Refused::DoesNotMove(format!(
            "{}'s declaration says no way its conversations move",
            declaration.title
        ))),
    }
}

/// The declaration's file as its root folder's word and the parts after it, `{id}` filled.
fn parts_of<'a>(file: &'a str, id: &SessionId) -> Option<(&'a str, Vec<String>)> {
    let (root, rest) = file.split_once('/')?;
    Some((
        root,
        rest.split('/')
            .map(|part| part.replace("{id}", id.as_str()))
            .collect(),
    ))
}

/// Whether `name` is a plain name a part may be: letters, digits, `.`, `_`, `-`, never empty,
/// `.` or `..`, and never a hidden one.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// Whether `name` is matched by `pattern`, a part whose every `*` stands for any run of
/// characters within one name.
fn matches(pattern: &str, name: &str) -> bool {
    let mut pieces = pattern.split('*');
    let first = pieces.next().unwrap_or_default();
    let Some(mut rest) = name.strip_prefix(first) else {
        return false;
    };
    let pieces: Vec<&str> = pieces.collect();
    let Some((last, middle)) = pieces.split_last() else {
        return rest.is_empty();
    };
    for piece in middle {
        match rest.find(piece) {
            Some(at) => rest = &rest[at + piece.len()..],
            None => return false,
        }
    }
    rest.len() >= last.len() && rest.ends_with(last)
}

/// **The conversation `id` of the profile whose harness `declaration` is, in `folders`**: the
/// one file its declaration names, every `*` and `{cwd_key}` part matched against what is there.
/// Where more than one matches (the same conversation kept for two directories), the one written
/// last is the conversation as it is now. Links are never followed.
pub fn find(
    declaration: &Declaration,
    folders: &Folders,
    id: &SessionId,
) -> Result<Found, Refused> {
    let file = one_file(declaration)?;
    let (root_word, parts) = parts_of(file, id).ok_or_else(|| {
        Refused::DoesNotMove(format!(
            "{}'s conversation file is not one",
            declaration.title
        ))
    })?;
    let root = folders.of(root_word).ok_or_else(|| {
        Refused::NotFound(format!(
            "{} keeps its conversations in a folder this profile has none of",
            declaration.title
        ))
    })?;
    let mut reached: Vec<(PathBuf, Vec<String>)> = vec![(root.to_path_buf(), Vec::new())];
    for (at, part) in parts.iter().enumerate() {
        let last = at + 1 == parts.len();
        let mut next = Vec::new();
        for (dir, shape) in reached {
            let literal = part != "{cwd_key}" && !part.contains('*');
            let names: Vec<String> = if literal {
                vec![part.clone()]
            } else {
                let Ok(listed) = std::fs::read_dir(&dir) else {
                    continue;
                };
                listed
                    .filter_map(Result::ok)
                    .filter_map(|entry| entry.file_name().into_string().ok())
                    .filter(|name| plain(name))
                    .filter(|name| part == "{cwd_key}" || matches(part, name))
                    .collect()
            };
            for name in names {
                let path = dir.join(&name);
                let Ok(meta) = std::fs::symlink_metadata(&path) else {
                    continue;
                };
                let kind_ok = if last {
                    meta.file_type().is_file()
                } else {
                    meta.file_type().is_dir()
                };
                if !kind_ok {
                    continue;
                }
                let mut shape = shape.clone();
                shape.push(if part == "{cwd_key}" {
                    "{cwd_key}".to_owned()
                } else {
                    name
                });
                next.push((path, shape));
            }
        }
        reached = next;
    }
    let written = |path: &Path| {
        std::fs::symlink_metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    };
    let (path, shape) = reached
        .into_iter()
        .max_by_key(|(path, _)| written(path))
        .ok_or_else(|| {
            Refused::NotFound(format!(
                "{} has no conversation {} on this device, so there is nothing to carry",
                declaration.title,
                id.as_str()
            ))
        })?;
    Ok(Found {
        path,
        shape: format!("{root_word}/{}", shape.join("/")),
    })
}

/// **Where conversation `id`, carried as `shape`, is kept on this device**: in `folders`, the
/// profile's harness folders here, for a chat that runs in `cwd`. `shape` must be the
/// declaration's own file part by part: each `*` a plain name, `{cwd_key}` as written (made
/// again from `cwd`), and every other part as the declaration names it.
pub fn place_of(
    declaration: &Declaration,
    folders: &Folders,
    cwd: &Path,
    id: &SessionId,
    shape: &str,
) -> Result<PathBuf, Refused> {
    let file = one_file(declaration)?;
    let refused = || {
        Refused::NotAllowed(format!(
            "`{shape}` is not where {} keeps a conversation, so it was not taken",
            declaration.title
        ))
    };
    let (root_word, wanted) = parts_of(file, id).ok_or_else(refused)?;
    let (given_root, given) = shape.split_once('/').ok_or_else(refused)?;
    if given_root != root_word {
        return Err(refused());
    }
    let given: Vec<&str> = given.split('/').collect();
    if given.len() != wanted.len() {
        return Err(refused());
    }
    let root = folders.of(root_word).ok_or_else(|| {
        Refused::NotFound(format!(
            "{} keeps its conversations in a folder this profile has none of here",
            declaration.title
        ))
    })?;
    let mut at = root.to_path_buf();
    for (pattern, part) in wanted.iter().zip(given) {
        if pattern == "{cwd_key}" {
            if part != "{cwd_key}" {
                return Err(refused());
            }
            at.push(crate::harness_declaration::cwd_key(cwd));
        } else if plain(part) && matches(pattern, part) {
            at.push(part);
        } else {
            return Err(refused());
        }
    }
    Ok(at)
}

/// **Keeps the conversation at `from` as `at`**, a place [`place_of`] answered under `root`, the
/// harness folder it is in: every folder from `root` down to it made where it is not there, and
/// none of them a link, and the file written beside its place and moved over it whole, readable
/// by this user alone. A conversation kept there before (the same chat, back from a move) is
/// replaced by the one carried, which is the chat's conversation as it is now.
pub fn keep(root: &Path, at: &Path, from: &Path) -> Result<(), Refused> {
    let below = at.strip_prefix(root).map_err(|_| {
        Refused::NotAllowed(format!(
            "{} is not in the harness's folder, so the conversation was not kept",
            at.display()
        ))
    })?;
    let io = |why: std::io::Error| {
        Refused::Io(format!("the conversation could not be kept here: {why}"))
    };
    let parts: Vec<_> = below.components().collect();
    let Some((_, folders)) = parts.split_last() else {
        return Err(Refused::NotAllowed("the conversation has no name".into()));
    };
    let mut walked = root.to_path_buf();
    for part in folders {
        walked.push(part);
        match std::fs::symlink_metadata(&walked) {
            Ok(meta) if meta.file_type().is_dir() => {}
            Ok(_) => {
                return Err(Refused::NotAllowed(format!(
                    "{} is not a folder, so the conversation was not kept",
                    walked.display()
                )));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&walked).map_err(io)?;
            }
            Err(e) => return Err(io(e)),
        }
    }
    if std::fs::symlink_metadata(at).is_ok_and(|meta| !meta.file_type().is_file()) {
        return Err(Refused::NotAllowed(format!(
            "{} is not a file, so the conversation was not kept",
            at.display()
        )));
    }
    let size = std::fs::metadata(from).map_err(io)?.len();
    if size > MOST_BYTES {
        return Err(Refused::NotAllowed(format!(
            "the conversation is larger than {MOST_BYTES} bytes, so it was not kept"
        )));
    }
    let mut kept = tempfile::Builder::new()
        .prefix(".purlis-carried-")
        .tempfile_in(&walked)
        .map_err(io)?;
    let mut read = std::fs::File::open(from).map_err(io)?;
    std::io::copy(&mut read, kept.as_file_mut()).map_err(io)?;
    kept.as_file().sync_all().map_err(io)?;
    // `tempfile` makes it readable by this user alone.
    kept.persist(at).map_err(|why| io(why.error))?;
    Ok(())
}

/// **A moving chat's conversation, as the device it is on keeps it** (#2048): the chat's
/// profile and workspace in the project at `root`, and its conversation's id.
#[derive(Debug, Clone, Copy)]
pub struct OfAChat<'a> {
    pub root: &'a Path,
    pub workspace: &'a str,
    pub profile: &'a str,
    pub conversation: &'a str,
}

/// Where a chat's conversation is on this device: its harness's declaration, the profile's
/// folders, the directory it runs in, and its id.
struct Located {
    declaration: Declaration,
    folders: Folders,
    cwd: PathBuf,
    id: SessionId,
}

impl OfAChat<'_> {
    /// What this device's own files say of it: the profile as this project and this machine
    /// declare it, its harness's declaration, and the workspace's folder in the project, reached
    /// through no link.
    fn located(&self) -> Result<Located, Refused> {
        let id = SessionId::new(self.conversation)
            .map_err(|why| Refused::NotAllowed(format!("the conversation is not one: {why}")))?;
        let declared = crate::harness_declaration::read(self.root);
        let profiles = crate::profiles::derive_in(self.root, &declared);
        let profile = profiles.get(self.profile).ok_or_else(|| {
            Refused::NotFound(format!(
                "there is no profile {} on this device, so the conversation has nowhere to be",
                self.profile
            ))
        })?;
        let declaration = declared.get(&profile.kind).cloned().ok_or_else(|| {
            Refused::DoesNotMove(format!(
                "profile {} runs a harness this device has no declaration of",
                self.profile
            ))
        })?;
        let home = dirs::home_dir().ok_or_else(|| {
            Refused::NotFound("this device has no home folder for the harness's files".into())
        })?;
        let folders = Folders::for_profile(profile, &declaration, &home, &|name| {
            std::env::var(name).ok()
        })
        .ok_or_else(|| {
            Refused::DoesNotMove(format!(
                "{} keeps its files in no folder purlis can name for profile {}",
                declaration.title, self.profile
            ))
        })?;
        let (_, cwd) = crate::dispatchplace::workspace_folder(self.root, self.workspace)
            .map_err(|refused| Refused::NotFound(refused.in_window()))?;
        // The harness names a directory as the system resolves it.
        let cwd = cwd.canonicalize().unwrap_or(cwd);
        Ok(Located {
            declaration,
            folders,
            cwd,
            id,
        })
    }

    /// **The chat's conversation on this device**, to carry ([`find`]).
    pub fn find(&self) -> Result<Found, Refused> {
        let located = self.located()?;
        find(&located.declaration, &located.folders, &located.id)
    }

    /// **Keeps the conversation carried as `shape`, at `from`, on this device** ([`place_of`],
    /// [`keep`]), where the chat's harness resumes it.
    pub fn keep(&self, shape: &str, from: &Path) -> Result<(), Refused> {
        let located = self.located()?;
        let at = place_of(
            &located.declaration,
            &located.folders,
            &located.cwd,
            &located.id,
            shape,
        )?;
        let root_word = shape.split('/').next().unwrap_or_default();
        let root = located.folders.of(root_word).ok_or_else(|| {
            Refused::NotAllowed(format!("`{shape}` is in no folder of this profile"))
        })?;
        std::fs::create_dir_all(root)
            .map_err(|why| Refused::Io(format!("the harness's folder could not be made: {why}")))?;
        keep(root, &at, from)
    }
}

#[cfg(test)]
mod tests;
