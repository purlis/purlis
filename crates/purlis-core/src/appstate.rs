// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The app's own state of a project, in the data home** (#3718, ADR 0067 as amended
//! 2026-10-28): `<data>/app-state/<clone key>/`, one folder per project clone by its
//! [`crate::plane::CloneKey`]. Trusted app state lives in the app's data folder, outside the
//! plane: what purlis reads back to decide something is kept where only purlis writes it, and a
//! chat's sandbox reads it and never writes it (`sandbox::kept_by_purlis`).
//!
//! The Skill studio's stores are the first kept here ([`STORES`]): the tries, the drafts, the
//! local grade record, the grades under way, the grade-after-edit opt-ins, the records of the
//! levels a sandboxed chat could have made, and the kept grade chats.
//!
//! - **No data home, no store.** Where this machine has no data home, [`dir`] answers `None`, and
//!   each store's callers refuse or say so: none falls back to the plane.
//! - **A fenced build** (every test build, [`crate::fence::FENCED`]) whose data home is outside
//!   its fence keeps the folder under the temporary directory instead, keyed the same way, so a
//!   test never reaches the person's data home and no test passes a folder in.
//! - **Moved once** ([`move_out_of_the_plane`]): at a project's first open after the upgrade, each
//!   store an older purlis kept in the plane's state folder is moved here or discarded, the copy
//!   in the plane is removed, and [`MOVED_MARK`] says so. Nothing reads the plane's copy after
//!   that.

use std::path::{Path, PathBuf};

/// The folder of the data home the projects' app state is in, one folder each.
pub const DIR: &str = "app-state";

/// The folder of the temporary directory a fenced build keeps its data home in, where the one
/// its environment names is outside its fence.
const FENCED_DATA: &str = "purlis-fenced-data";

/// What a store answers where this machine has no data home.
pub const NO_DATA_HOME: &str = "purlis has no data folder on this machine, so the Skill \
     studio's state is not kept: set PURLIS_DATA_HOME to a folder outside any project";

/// `<data>` for the app's state: the data home, or, in a fenced build whose data home is outside
/// its fence, a folder of the temporary directory.
fn data_home() -> Option<PathBuf> {
    if crate::fence::FENCED {
        let named = crate::datahome::root_in(&crate::envvar::var);
        return match named {
            Some(data) if crate::fence::inside(&data, &crate::fence::fence()) => Some(data),
            _ => Some(std::env::temp_dir().join(FENCED_DATA)),
        };
    }
    crate::datahome::root()
}

/// **The folder of the app's state of the project at `root`**: `<data>/app-state/<clone key>`.
/// `None` where this machine has no data home. Nothing is read or made.
pub fn dir(root: &Path) -> Option<PathBuf> {
    Some(in_data(&data_home()?, root))
}

/// The folder of the app's state of the project at `root`, in the data home `data`.
pub fn in_data(data: &Path, root: &Path) -> PathBuf {
    data.join(DIR)
        .join(crate::plane::CloneKey::of(root).as_str())
}

/// **[`dir`], made**: `<data>/app-state` and the project's folder in it, each private and refused
/// where it is a link or not a folder. Refused where there is no data home, or the data home is in
/// a project or a git work tree (ADR 0075 §6).
pub fn made(root: &Path) -> Result<PathBuf, String> {
    let data = data_home().ok_or_else(|| NO_DATA_HOME.to_owned())?;
    if let Some(why) = crate::datahome::refusal(&data) {
        return Err(why);
    }
    let all = data.join(DIR);
    let mine = in_data(&data, root);
    for folder in [&all, &mine] {
        crate::profiletrust::private_dir(folder).map_err(|why| why.to_string())?;
        if !std::fs::symlink_metadata(folder).is_ok_and(|meta| meta.is_dir()) {
            return Err(format!("{} is not a folder of its own", folder.display()));
        }
    }
    Ok(mine)
}

/// What [`move_out_of_the_plane`] does with one store an older purlis kept in the plane's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carried {
    /// Moved into the app's state: the person's own work, which nothing else holds.
    Moved,
    /// Moved, each file added to the one of its name that is there already: a record of rows.
    Appended,
    /// Removed: transient, or a record of the person's word that they give again.
    Discarded,
    /// Removed, and every workspace and persona folder of the project is held until the person
    /// accepts it again in the Skill studio (`skills::levels`).
    AskedAgain,
}

/// **Each store of the app's state**, by its name in the plane's state folder, and what becomes
/// of an older purlis's copy there.
pub const STORES: [(&str, Carried); 9] = [
    (crate::skills::verbs::drafts::DRAFTS_DIR, Carried::Moved),
    (crate::skills::grades::LOCAL_DIR, Carried::Appended),
    (crate::skills::verbs::drafts::TRIES_DIR, Carried::Discarded),
    (crate::skills::grades::underway::DIR, Carried::Discarded),
    (crate::skills::grades::underway::CLAIM, Carried::Discarded),
    (crate::skills::grades::after_edit::FILE, Carried::Discarded),
    (crate::skills::grades::after_edit::LOCK, Carried::Discarded),
    (crate::skills::grades::kept::DIR_NAME, Carried::Discarded),
    (crate::skills::levels::DIR, Carried::AskedAgain),
];

/// What [`move_out_of_the_plane`] did: each store's name, and what became of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Moved {
    pub done: Vec<(String, Carried)>,
    /// What could not be moved or removed, and why: left where it was, and read by nothing.
    pub left: Vec<String>,
}

/// The file in a project's app state that says its stores were moved out of the plane: once it is
/// there, nothing reads the plane's copies again.
pub const MOVED_MARK: &str = "moved-from-the-plane";

/// **Moves the app's state an older purlis kept in the plane's state folder of the project at
/// `root` into the app's state** ([`STORES`]), once: what a project's open does, before any chat
/// of it starts. Each spelling of the state folder is looked at, and the one `$PURLIS_HOME`
/// names. Once every store is moved or removed, [`MOVED_MARK`] is written, and every later open
/// looks at that one name and nothing in the plane. Nothing at all where there is no data home.
pub fn move_out_of_the_plane(root: &Path) -> Moved {
    let Ok(into) = made(root) else {
        return Moved::default();
    };
    let mark = into.join(MOVED_MARK);
    if std::fs::symlink_metadata(&mark).is_ok() {
        return Moved::default();
    }
    let mut states: Vec<PathBuf> = crate::names::STATE_DIR
        .spellings()
        .map(|state| root.join(state))
        .collect();
    let named = crate::plane::state_dir(root);
    if !states.contains(&named) {
        states.push(named);
    }
    let moved = move_from(root, &states, &into);
    if moved.left.is_empty()
        && let Err(why) = crate::plane::write_private(&into, &mark, b"")
    {
        tracing::warn!(target: "purlis::appstate", "purlis: the move out of the plane was not marked ({why})");
    }
    moved
}

/// [`move_out_of_the_plane`] from the state folders `states`, into the project's app state
/// `into`.
pub fn move_from(root: &Path, states: &[PathBuf], into: &Path) -> Moved {
    let mut out = Moved::default();
    for state in states {
        if !std::fs::symlink_metadata(state).is_ok_and(|meta| meta.is_dir()) {
            continue;
        }
        for (name, carried) in STORES {
            let old = state.join(name);
            let Ok(meta) = std::fs::symlink_metadata(&old) else {
                continue;
            };
            let keeps = matches!(carried, Carried::Moved | Carried::Appended);
            // A store kept is carried only where it is a folder of its own, never through a link.
            if keeps
                && meta.is_dir()
                && let Err(why) = carry(&old, &into.join(name), carried == Carried::Appended)
            {
                out.left
                    .push(format!("{name} was left in the plane ({why})"));
                continue;
            }
            if carried == Carried::AskedAgain
                && let Err(why) = crate::skills::levels::ask_again(root)
            {
                out.left
                    .push(format!("{name} was left in the plane ({why})"));
                continue;
            }
            if let Err(why) = removed(&old) {
                out.left
                    .push(format!("{name} was not removed from the plane ({why})"));
                continue;
            }
            out.done.push((name.to_owned(), carried));
        }
    }
    for line in &out.left {
        tracing::warn!(target: "purlis::appstate", "purlis: {line}");
    }
    out
}

/// `path` removed, a folder with all it holds, never through a link: a link is removed itself.
fn removed(path: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

/// The deepest a carried store's folders go: past any store's.
const DEEPEST: usize = 16;

/// **The folder `from` carried to `to`**, which is made where it is not: each entry `to` does not
/// have yet is moved in, and, where `append`, each file it has is added to. Links, and what is
/// neither a file nor a folder, are left behind.
fn carry(from: &Path, to: &Path, append: bool) -> std::io::Result<()> {
    carry_at(from, to, append, 0)
}

fn carry_at(from: &Path, to: &Path, append: bool, depth: usize) -> std::io::Result<()> {
    if depth > DEEPEST {
        return Ok(());
    }
    crate::profiletrust::private_dir(to)?;
    if !std::fs::symlink_metadata(to)?.is_dir() {
        return Err(std::io::Error::other("its place is not a folder"));
    }
    for entry in std::fs::read_dir(from)?.filter_map(Result::ok) {
        let name = entry.file_name();
        let (old, new) = (from.join(&name), to.join(&name));
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let there = std::fs::symlink_metadata(&new).ok();
        if kind.is_dir() {
            match there {
                None => carry_at(&old, &new, append, depth + 1)?,
                Some(meta) if meta.is_dir() && append => {
                    carry_at(&old, &new, append, depth + 1)?;
                }
                Some(_) => {}
            }
        } else if kind.is_file() {
            match there {
                None => copied(&old, &new, false)?,
                Some(meta) if meta.is_file() && append => copied(&old, &new, true)?,
                Some(_) => {}
            }
        }
    }
    Ok(())
}

/// The file `from` written to `to`, private, opened following no link: made new, or, where
/// `append`, added to the end of the one there.
fn copied(from: &Path, to: &Path, append: bool) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut reading = std::fs::OpenOptions::new();
    reading.read(true);
    let mut source = crate::contain::nofollow(&mut reading).open(from)?;
    let mut options = std::fs::OpenOptions::new();
    if append {
        options.append(true);
    } else {
        options.write(true).create_new(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut target = crate::contain::nofollow(&mut options).open(to)?;
    std::io::copy(&mut source, &mut target)?;
    target.flush()
}

/// **A test's seam: the app state's `name` of the project at `root` moved to where an older purlis
/// kept it**, in the plane's state folder: what a store must never read again.
#[cfg(test)]
pub(crate) fn put_in_the_plane(root: &Path, name: &str) -> PathBuf {
    let from = dir(root).expect("a fenced build has one").join(name);
    let state = crate::plane::state_dir(root);
    std::fs::create_dir_all(&state).expect("a state folder");
    let to = state.join(name);
    std::fs::rename(&from, &to).expect("moved into the plane");
    to
}

#[cfg(test)]
mod tests;
