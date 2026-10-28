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
//! - **A fenced build** (every test build, [`crate::fence::FENCED`]) whose environment names no
//!   data home inside its fence keeps the folder in the project's own state folder instead
//!   ([`FENCED_DATA`]), so a test never reaches the person's data home, no test passes a folder
//!   in, and a test's temporary project takes its app state away with it.
//! - **Bound to its project** ([`OWNER`], [`PLANE_ID`]): a new project made at a path an older one
//!   had never inherits the older one's state.
//! - **Moved once** ([`move_out_of_the_plane`]): at a project's first open after the upgrade, each
//!   store an older purlis kept in the plane's state folder is moved here or discarded, the copy
//!   in the plane is removed, and [`MOVED_MARK`] says so. Nothing reads the plane's copy after
//!   that.
//! - **It follows the project** ([`follow_move`]): a project that moved, as its record of open
//!   chats says (V43), keeps its drafts and grades under its new clone key.

use std::path::{Path, PathBuf};

/// The folder of the data home the projects' app state is in, one folder each.
pub const DIR: &str = "app-state";

/// The folder of a project's own state folder a fenced build keeps its data home in, where its
/// environment names none inside its fence: inside the test's own project, which git ignores with
/// the rest of the state folder, and beside the stores an older purlis kept there, never one of
/// them. Never in a shipped build.
pub const FENCED_DATA: &str = ".fenced-app-state";

/// What a store answers where this machine has no usable data home.
pub const NO_DATA_HOME: &str = "purlis has no usable data folder on this machine (none, or one \
     inside a project or a git work tree), so the Skill studio's state is not kept: set \
     PURLIS_DATA_HOME to a folder outside any project";

/// `<data>` for the app's state of the project at `root`: the data home where it may hold it
/// (none in a project or a git work tree, ADR 0075 §6, asked of the disk once per data home in a
/// process, so a chat's open adds no walk of its folders); or, in a fenced build whose environment
/// names no data home inside its fence, [`FENCED_DATA`] in the project's own tree.
fn data_home(root: &Path) -> Option<PathBuf> {
    if crate::fence::FENCED {
        return match crate::datahome::root_in(&crate::envvar::var) {
            Some(data)
                if crate::envvar::var(crate::datahome::HOME_VAR).is_some_and(|v| !v.is_empty())
                    && crate::fence::inside(&data, &crate::fence::fence()) =>
            {
                usable(&data).then_some(data)
            }
            _ => Some(crate::names::state(root).join(FENCED_DATA)),
        };
    }
    let data = crate::datahome::root()?;
    usable(&data).then_some(data)
}

/// Whether `data` may be the data home ([`crate::datahome::refusal`]), remembered for the process.
fn usable(data: &Path) -> bool {
    static SEEN: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<PathBuf, bool>>> =
        std::sync::OnceLock::new();
    let seen = SEEN.get_or_init(Default::default);
    if let Some(ok) = seen.lock().ok().and_then(|seen| seen.get(data).copied()) {
        return ok;
    }
    let ok = crate::datahome::refusal(data).is_none();
    if let Ok(mut seen) = seen.lock() {
        seen.insert(data.to_path_buf(), ok);
    }
    ok
}

/// **The folder of the app's state of the project at `root`**: `<data>/app-state/<clone key>`.
/// `None` where this machine has no usable data home. Nothing is read or made.
pub fn dir(root: &Path) -> Option<PathBuf> {
    Some(in_data(&data_home(root)?, root))
}

/// The folder of the app's state of the project at `root`, in the data home `data`.
pub fn in_data(data: &Path, root: &Path) -> PathBuf {
    data.join(DIR)
        .join(crate::plane::CloneKey::of(root).as_str())
}

/// The file in a project's app state that names the project's root, as the kernel names it: what
/// a move of the project is checked against ([`follow_move`]).
pub const ROOT_FILE: &str = "root";

/// The file in a project's app state that names the project it belongs to, by the id in the
/// project's [`PLANE_ID`].
pub const OWNER: &str = "owner";

/// The file of the project's own state folder's `app/` that names the project to its app state:
/// 32 random hex characters, made with the folder, and written by purlis alone (a chat's sandbox
/// denies writing `app/`). A project made new at the same path has none, or another.
pub const PLANE_ID: &str = "app-state-id";

/// The most bytes of a small record of the app's state read: a root, an id.
const MOST_RECORD_BYTES: u64 = 4096;

/// The project at `root`'s [`PLANE_ID`] file.
fn plane_id_file(root: &Path) -> PathBuf {
    crate::names::state(root).join("app").join(PLANE_ID)
}

/// The text of the small record `path`, where it is one: read following no link, at most
/// [`MOST_RECORD_BYTES`].
fn record_text(path: &Path) -> Option<String> {
    let bytes = read_bounded(path, MOST_RECORD_BYTES).ok()?;
    Some(String::from_utf8(bytes).ok()?.trim().to_owned()).filter(|text| !text.is_empty())
}

/// **[`dir`], made**, and bound to its project: `<data>/app-state` and the project's folder in
/// it, each a folder of its own (never a link), this user's, and `0700`, which a folder already
/// there is made too; the project's [`ROOT_FILE`]; and its [`OWNER`], the project's [`PLANE_ID`],
/// made where the project has none. A folder whose owner is another project's is set aside as
/// `<key>.foreign-<n>`, read no more, and a new one made. Refused where there is no usable data
/// home.
pub fn made(root: &Path) -> Result<PathBuf, String> {
    made_in(
        &data_home(root).ok_or_else(|| NO_DATA_HOME.to_owned())?,
        root,
    )
}

/// [`made`], in the data home `data`.
fn made_in(data: &Path, root: &Path) -> Result<PathBuf, String> {
    let all = data.join(DIR);
    let mine = in_data(data, root);
    held_private(&all).map_err(|why| format!("{}: {why}", all.display()))?;
    let plane_id = record_text(&plane_id_file(root));
    if std::fs::symlink_metadata(&mine).is_ok() {
        let owner = record_text(&mine.join(OWNER));
        if owner.is_none() || owner != plane_id {
            set_aside(&mine)?;
        }
    }
    held_private(&mine).map_err(|why| format!("{}: {why}", mine.display()))?;
    if std::fs::symlink_metadata(mine.join(OWNER)).is_err() {
        let id = match plane_id {
            Some(id) => id,
            None => {
                let id = random_id()?;
                let file = plane_id_file(root);
                let app = file.parent().ok_or("no folder")?;
                crate::profiletrust::private_dir(app).map_err(|why| why.to_string())?;
                crate::plane::write_private(root, &file, id.as_bytes())
                    .map_err(|why| why.to_string())?;
                id
            }
        };
        crate::plane::write_private(&mine, &mine.join(OWNER), id.as_bytes())
            .map_err(|why| why.to_string())?;
    }
    if std::fs::symlink_metadata(mine.join(ROOT_FILE)).is_err() {
        let real = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let text = real
            .to_str()
            .ok_or_else(|| "the project's root is not text".to_owned())?;
        crate::plane::write_private(&mine, &mine.join(ROOT_FILE), text.as_bytes())
            .map_err(|why| why.to_string())?;
    }
    Ok(mine)
}

/// The folder `mine`, another project's, renamed beside itself as `<key>.foreign-<n>`.
fn set_aside(mine: &Path) -> Result<(), String> {
    let name = mine
        .file_name()
        .ok_or("no name")?
        .to_string_lossy()
        .into_owned();
    for n in 1..1000 {
        let aside = mine.with_file_name(format!("{name}.foreign-{n}"));
        if std::fs::symlink_metadata(&aside).is_err() {
            tracing::info!(target: "purlis::appstate", "purlis: the app state at {} is another project's, so it was set aside", mine.display());
            return std::fs::rename(mine, &aside).map_err(|why| why.to_string());
        }
    }
    Err("another project's app state could not be set aside".to_owned())
}

/// Thirty-two random hex characters.
fn random_id() -> Result<String, String> {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|why| why.to_string())?;
    Ok(crate::extension::hex(&random))
}

/// `folder` made private where it is not there; where it is, held to a folder of its own (no
/// link), this user's, and made `0700`.
fn held_private(folder: &Path) -> std::io::Result<()> {
    let meta = match std::fs::symlink_metadata(folder) {
        Ok(meta) => meta,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => {
            crate::profiletrust::private_dir(folder)?;
            std::fs::symlink_metadata(folder)?
        }
        Err(why) => return Err(why),
    };
    if !meta.is_dir() {
        return Err(std::io::Error::other("it is not a folder of its own"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        if meta.uid() != rustix::process::geteuid().as_raw() {
            return Err(std::io::Error::other("it is another user's"));
        }
        if meta.mode() & 0o777 != 0o700 {
            std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
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
/// of it starts. Only the state folder purlis was reading is looked at, the one
/// [`crate::names::state`] picks; the other spelling is left as it is, unread. A folder
/// `$PURLIS_HOME` names may be shared by several clones, so where it is the one purlis reads,
/// nothing is moved and its stores are read no more. Once every store is moved or removed, [`MOVED_MARK`] is written, and
/// every later open looks at that one name and nothing in the plane. Nothing at all where there is
/// no data home.
pub fn move_out_of_the_plane(root: &Path) -> Moved {
    let Ok(into) = made(root) else {
        return Moved::default();
    };
    let mark = into.join(MOVED_MARK);
    if std::fs::symlink_metadata(&mark).is_ok() {
        return Moved::default();
    }
    let picked = crate::names::state(root);
    let states: Vec<PathBuf> = if crate::plane::state_dir(root) == picked {
        vec![picked]
    } else {
        Vec::new()
    };
    let moved = move_from(root, &states, &into);
    if moved.left.is_empty()
        && let Err(why) = crate::plane::write_private(&into, &mark, b"")
    {
        tracing::warn!(target: "purlis::appstate", "purlis: the move out of the plane was not marked ({why})");
    }
    moved
}

/// [`move_out_of_the_plane`] from the state folders `states`, into the project's app state
/// `into`. A store is removed only once all of it is carried: one that is not stays where it is,
/// said in [`Moved::left`], and the next open carries it again, adding nothing twice.
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
            if keeps && meta.is_dir() {
                let missed = carry(&old, &into.join(name), carried == Carried::Appended);
                if !missed.is_empty() {
                    out.left.push(format!(
                        "{name} was left in the plane: {}",
                        missed.join("; ")
                    ));
                    continue;
                }
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

/// The stores a move of the project carries to its new clone key ([`follow_move`]): the person's
/// own work and records. Nothing that holds the person's word is carried: the grade-after-edit
/// opt-ins are turned on again from a plan, and the levels records are asked about again.
pub const CARRIED_ON_A_MOVE: [(&str, Carried); 3] = [
    (crate::skills::verbs::drafts::DRAFTS_DIR, Carried::Moved),
    (crate::skills::grades::LOCAL_DIR, Carried::Appended),
    (crate::skills::grades::kept::DIR_NAME, Carried::Moved),
];

/// **The app's state of the project at `root` follows it from where it was** (#3718): what a
/// project's open does when its record of open chats says the project moved
/// (`reopen::Arrival::Moved`, V43), with `was` the clone the record names. Only where the old
/// clone key's folder says it is that clone's own (its [`ROOT_FILE`] names `was.root`, and its
/// [`OWNER`] is the project's [`PLANE_ID`], which moved with it): its
/// drafts, local grades and kept grade chats are carried ([`CARRIED_ON_A_MOVE`]); its opt-ins and
/// levels records are not, and where it held levels records every workspace and persona folder of
/// the project waits for the person's Accept again. The old folder is removed once all of it is
/// carried. Nothing for any other folder, and nothing where there is no data home.
pub fn follow_move(root: &Path, was: &crate::reopen::CloneSeat) -> Moved {
    match data_home(root) {
        Some(data) => follow_move_in(&data, root, was),
        None => Moved::default(),
    }
}

/// [`follow_move`], in the data home `data`.
fn follow_move_in(data: &Path, root: &Path, was: &crate::reopen::CloneSeat) -> Moved {
    let mut out = Moved::default();
    let old = data.join(DIR).join(was.key.as_str());
    let here = in_data(data, root);
    if old == here || !std::fs::symlink_metadata(&old).is_ok_and(|meta| meta.is_dir()) {
        return out;
    }
    let its_own = was.root.to_str() == record_text(&old.join(ROOT_FILE)).as_deref()
        && record_text(&old.join(OWNER)).is_some_and(|owner| {
            record_text(&plane_id_file(root)).as_deref() == Some(owner.as_str())
        });
    if !its_own {
        tracing::warn!(target: "purlis::appstate", "purlis: the app state of {} is another project's, so nothing was carried", was.root.display());
        return out;
    }
    let into = match made_in(data, root) {
        Ok(into) => into,
        Err(why) => {
            out.left.push(format!("nothing was carried ({why})"));
            return out;
        }
    };
    if std::fs::symlink_metadata(old.join(crate::skills::levels::DIR)).is_ok() {
        if let Err(why) = crate::skills::levels::ask_again_in(&into, root) {
            out.left.push(format!(
                "{} was not asked about again ({why})",
                crate::skills::levels::DIR
            ));
        } else {
            out.done
                .push((crate::skills::levels::DIR.to_owned(), Carried::AskedAgain));
        }
    }
    for (name, carried) in CARRIED_ON_A_MOVE {
        let from = old.join(name);
        if !std::fs::symlink_metadata(&from).is_ok_and(|meta| meta.is_dir()) {
            continue;
        }
        let missed = carry(&from, &into.join(name), carried == Carried::Appended);
        if missed.is_empty() {
            out.done.push((name.to_owned(), carried));
        } else {
            out.left
                .push(format!("{name} was not carried: {}", missed.join("; ")));
        }
    }
    if out.left.is_empty()
        && let Err(why) = removed(&old)
    {
        out.left
            .push(format!("the old app state was not removed ({why})"));
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

/// The most bytes of one carried file: past any store's.
const MOST_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// **The folder `from` carried to `to`**, which is made where it is not, and what of it was not
/// (empty when all of it was). Each file `to` does not have is written in; one it has with the
/// same bytes is carried already; and, where `append`, each line of a file it has that the file
/// does not hold yet is added to it, so carrying twice adds nothing. A file is written beside its
/// place and renamed in, so none is ever found half written. A link, what is neither a file nor a
/// folder, a folder past [`DEEPEST`], a file past [`MOST_FILE_BYTES`] and a name `to` holds
/// otherwise are each not carried, and said.
fn carry(from: &Path, to: &Path, append: bool) -> Vec<String> {
    let mut missed = Vec::new();
    carry_at(from, to, append, 0, &mut missed);
    missed
}

fn carry_at(from: &Path, to: &Path, append: bool, depth: usize, missed: &mut Vec<String>) {
    if depth > DEEPEST {
        missed.push(format!("{} is deeper than purlis carries", from.display()));
        return;
    }
    if let Err(why) = held_private(to) {
        missed.push(format!("{}: {why}", to.display()));
        return;
    }
    let entries = match std::fs::read_dir(from) {
        Ok(entries) => entries,
        Err(why) => {
            missed.push(format!("{}: {why}", from.display()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(why) => {
                missed.push(format!("{}: {why}", from.display()));
                continue;
            }
        };
        let name = entry.file_name();
        let (old, new) = (from.join(&name), to.join(&name));
        let kind = std::fs::symlink_metadata(&old).map(|meta| meta.file_type());
        let there = std::fs::symlink_metadata(&new).ok();
        let said = match kind {
            Ok(kind) if kind.is_dir() => match there {
                Some(meta) if !meta.is_dir() => Err("its place holds something else".to_owned()),
                _ => {
                    carry_at(&old, &new, append, depth + 1, missed);
                    Ok(())
                }
            },
            Ok(kind) if kind.is_file() => carried_file(&old, &new, there.as_ref(), append),
            Ok(_) => Err("it is a link, or not a file".to_owned()),
            Err(why) => Err(why.to_string()),
        };
        if let Err(why) = said {
            missed.push(format!("{} was not carried ({why})", old.display()));
        }
    }
}

/// The file `from` carried to `to`, which `there` says what is in its place.
fn carried_file(
    from: &Path,
    to: &Path,
    there: Option<&std::fs::Metadata>,
    append: bool,
) -> Result<(), String> {
    let bytes = read_bounded(from, MOST_FILE_BYTES)?;
    let now = match there {
        None => bytes,
        Some(meta) if !meta.is_file() => return Err("its place holds something else".into()),
        Some(_) => {
            let was = read_bounded(to, MOST_FILE_BYTES)?;
            if was == bytes {
                return Ok(());
            }
            if !append {
                return Err("its place holds another file of that name".into());
            }
            let Some(joined) = lines_added(&was, &bytes) else {
                return Ok(());
            };
            joined
        }
    };
    written_in(to, &now).map_err(|why| why.to_string())
}

/// `was` with each line of `more` it does not hold added at its end, or `None` where it holds
/// every one.
fn lines_added(was: &[u8], more: &[u8]) -> Option<Vec<u8>> {
    let held: std::collections::HashSet<&[u8]> = was.split(|b| *b == b'\n').collect();
    let mut out = was.to_vec();
    let mut added = false;
    for line in more.split(|b| *b == b'\n') {
        if line.is_empty() || held.contains(line) {
            continue;
        }
        if !out.is_empty() && !out.ends_with(b"\n") {
            out.push(b'\n');
        }
        out.extend_from_slice(line);
        out.push(b'\n');
        added = true;
    }
    added.then_some(out)
}

/// The bytes of the plain file `path`, read following no link, at most `most`: the one read of the
/// app's state's own files.
fn read_bounded(path: &Path, most: u64) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let meta = std::fs::symlink_metadata(path).map_err(|why| why.to_string())?;
    if !meta.is_file() {
        return Err("it is not a file of its own".into());
    }
    if meta.len() > most {
        return Err("it is larger than purlis carries".into());
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let mut bytes = Vec::new();
    crate::contain::nofollow(&mut options)
        .open(path)
        .and_then(|file| file.take(most).read_to_end(&mut bytes))
        .map_err(|why| why.to_string())?;
    Ok(bytes)
}

/// `bytes` written to `to`, private, beside it first and renamed into its place
/// ([`crate::rewrite::replace`]).
fn written_in(to: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let folder = to
        .parent()
        .ok_or_else(|| std::io::Error::other("no folder"))?;
    crate::rewrite::replace(folder, to, bytes, crate::rewrite::Mode::Private)
}

/// **A test's seam: the app state's `name` of the project at `root` moved to where an older purlis
/// kept it**, in the plane's state folder: what a store must never read again.
#[cfg(test)]
pub(crate) fn put_in_the_plane(root: &Path, name: &str) -> PathBuf {
    let from = dir(root).expect("a fenced build has one").join(name);
    let state = crate::names::state(root);
    std::fs::create_dir_all(&state).expect("a state folder");
    let to = state.join(name);
    std::fs::rename(&from, &to).expect("moved into the plane");
    to
}

#[cfg(test)]
mod tests;
