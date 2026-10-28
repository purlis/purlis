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
//! - **It follows the project** ([`follow_arrival`]): a project that moved, as its record of open
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

/// **The folder of the app's state of the project at `root`**: `<data>/app-state/<clone key>`,
/// where it is this project's own: its [`OWNER`] is the project's [`PLANE_ID`]. `None` where this
/// machine has no usable data home, and where the folder is not there or is another project's
/// (one at the same path before), whose state no reader sees; [`made`] sets such a folder aside.
/// Nothing is made, and once a folder is found this project's, it is not read again in the process.
pub fn dir(root: &Path) -> Option<PathBuf> {
    let mine = in_data(&data_home(root)?, root);
    bound(&mine, root).then_some(mine)
}

/// Whether this machine has a data home it may keep the app's state of the project at `root` in.
pub fn kept_here(root: &Path) -> bool {
    data_home(root).is_some()
}

/// The folders found to be their project's own in this process ([`bound`]).
fn found_bound() -> &'static std::sync::Mutex<std::collections::HashSet<PathBuf>> {
    static FOUND: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> =
        std::sync::OnceLock::new();
    FOUND.get_or_init(Default::default)
}

/// Whether the folder `mine` is the project at `root`'s own: its [`OWNER`] is the project's
/// [`PLANE_ID`]. Remembered for the process once it is.
fn bound(mine: &Path, root: &Path) -> bool {
    if found_bound().lock().is_ok_and(|found| found.contains(mine)) {
        return true;
    }
    let owner = record_text(&mine.join(OWNER));
    let ok = owner.is_some() && owner == record_text(&plane_id_file(root));
    if ok && let Ok(mut found) = found_bound().lock() {
        found.insert(mine.to_path_buf());
    }
    ok
}

/// The folder of the app's state of the project at `root`, in the data home `data`.
pub fn in_data(data: &Path, root: &Path) -> PathBuf {
    data.join(DIR)
        .join(crate::plane::CloneKey::of(root).as_str())
}

/// The file in a project's app state that names the project's root, as the kernel names it: what
/// a move of the project is checked against ([`follow_arrival`]).
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
    let data = data_home(root).ok_or_else(|| NO_DATA_HOME.to_owned())?;
    // A folder found this project's in this process, and still a folder: made already.
    let mine = in_data(&data, root);
    if found_bound()
        .lock()
        .is_ok_and(|found| found.contains(&mine))
        && std::fs::symlink_metadata(&mine).is_ok_and(|meta| meta.is_dir())
    {
        return Ok(mine);
    }
    made_in(&data, root)
}

/// [`made`], in the data home `data`: one call at a time for a project, across processes, under
/// the `flock` of `<key>.lock` beside its folder.
fn made_in(data: &Path, root: &Path) -> Result<PathBuf, String> {
    let all = data.join(DIR);
    held_private(&all).map_err(|why| format!("{}: {why}", all.display()))?;
    let mine = in_data(data, root);
    let _held = locked(&mine)?;
    made_locked(&mine, root)
}

/// How long a call waits for another to let go of a project's app state.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// The `flock` of `<key>.lock` beside the project's folder `mine`, held.
fn locked(mine: &Path) -> Result<crate::filelock::Held, String> {
    let name = mine.file_name().ok_or("no name")?.to_string_lossy();
    let path = mine.with_file_name(format!("{name}.lock"));
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let file = crate::contain::nofollow(&mut options)
        .open(&path)
        .map_err(|why| format!("{}: {why}", path.display()))?;
    crate::filelock::lock_within(file, LOCK_WAIT, "the project's app state")
        .map_err(|why| why.to_string())
}

/// `text` written to `path` where no file is there yet, made new and private, following no link;
/// what the file holds either way. Two writers never both write one.
fn written_once(path: &Path, text: &str) -> Result<String, String> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    match crate::contain::nofollow(&mut options).open(path) {
        Ok(mut file) => {
            file.write_all(text.as_bytes())
                .and_then(|()| file.sync_all())
                .map_err(|why| why.to_string())?;
            Ok(text.to_owned())
        }
        Err(why) if why.kind() == std::io::ErrorKind::AlreadyExists => {
            record_text(path).ok_or_else(|| format!("{} could not be read", path.display()))
        }
        Err(why) => Err(why.to_string()),
    }
}

/// [`made_in`], with the project's lock held.
fn made_locked(mine: &Path, root: &Path) -> Result<PathBuf, String> {
    let plane_id = record_text(&plane_id_file(root));
    let mut held_levels = false;
    if std::fs::symlink_metadata(mine).is_ok() {
        let owner = record_text(&mine.join(OWNER));
        if owner.is_none() || owner != plane_id {
            let aside = set_aside(mine)?;
            held_levels = std::fs::symlink_metadata(aside.join(crate::skills::levels::DIR)).is_ok();
        }
    }
    held_private(mine).map_err(|why| format!("{}: {why}", mine.display()))?;
    if std::fs::symlink_metadata(mine.join(OWNER)).is_err() {
        let id = match plane_id {
            Some(id) => id,
            None => {
                let file = plane_id_file(root);
                let app = file.parent().ok_or("no folder")?;
                crate::profiletrust::private_dir(app).map_err(|why| why.to_string())?;
                written_once(&file, &random_id()?)?
            }
        };
        if written_once(&mine.join(OWNER), &id)? != id {
            return Err("the project's app state names another project".to_owned());
        }
    }
    if std::fs::symlink_metadata(mine.join(ROOT_FILE)).is_err() {
        let real = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let text = real
            .to_str()
            .ok_or_else(|| "the project's root is not text".to_owned())?;
        written_once(&mine.join(ROOT_FILE), text)?;
    }
    // The folder set aside held levels records, which no reader sees now: every workspace and
    // persona folder waits for the person's Accept again.
    if held_levels {
        crate::skills::levels::ask_again_in(mine, root)?;
    }
    if let Ok(mut found) = found_bound().lock() {
        found.insert(mine.to_path_buf());
    }
    Ok(mine.to_path_buf())
}

/// **The project at `root` is given an id of its own**, and its app state with it: what a copy of
/// a project gets, so it never shares the original's ([`follow_arrival`]).
fn reminted(data: &Path, root: &Path) -> Result<(), String> {
    let mine = made_in(data, root)?;
    let _held = locked(&mine)?;
    let id = random_id()?;
    crate::plane::write_private(root, &plane_id_file(root), id.as_bytes())
        .map_err(|why| why.to_string())?;
    crate::plane::write_private(&mine, &mine.join(OWNER), id.as_bytes())
        .map_err(|why| why.to_string())
}

/// The folder `mine`, another project's, renamed beside itself as `<key>.foreign-<n>`, and where it
/// went.
fn set_aside(mine: &Path) -> Result<PathBuf, String> {
    if let Ok(mut found) = found_bound().lock() {
        found.remove(mine);
    }
    let name = mine
        .file_name()
        .ok_or("no name")?
        .to_string_lossy()
        .into_owned();
    for n in 1..1000 {
        let aside = mine.with_file_name(format!("{name}.foreign-{n}"));
        if std::fs::symlink_metadata(&aside).is_err() {
            tracing::info!(target: "purlis::appstate", "purlis: the app state at {} is another project's, so it was set aside", mine.display());
            return std::fs::rename(mine, &aside)
                .map(|()| aside)
                .map_err(|why| why.to_string());
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
    let _ = moves_taken_up(root);
    let mark = into.join(MOVED_MARK);
    if std::fs::symlink_metadata(&mark).is_ok() {
        return Moved::default();
    }
    let picked = crate::names::state(root);
    let named = crate::plane::state_dir(root);
    if named != picked {
        // `$PURLIS_HOME` names the state folder: nothing is moved, and nothing marked, so the
        // plane's own folder is moved once purlis reads it. Its levels records are asked about
        // again, once.
        return Moved {
            left: asked_again_for(root, &into, &named, "purlis-home")
                .err()
                .into_iter()
                .collect(),
            ..Moved::default()
        };
    }
    let mut moved = move_from(root, std::slice::from_ref(&picked), &into);
    // The other spelling is never read, so the levels records it holds are asked about again.
    for other in crate::names::STATE_DIR
        .spellings()
        .map(|state| root.join(state))
    {
        if other != picked
            && let Err(why) = asked_again_for(root, &into, &other, "other-spelling")
        {
            moved.left.push(why);
        }
    }
    if moved.left.is_empty()
        && let Err(why) = written_once(&mark, "")
    {
        tracing::warn!(target: "purlis::appstate", "purlis: the move out of the plane was not marked ({why})");
    }
    moved
}

/// Where the state folder `state`, which is not moved, holds levels records: every workspace and
/// persona folder of the project waits for the person's Accept again, once, marked
/// `asked-again-<which>` in the project's app state `into`.
fn asked_again_for(root: &Path, into: &Path, state: &Path, which: &str) -> Result<(), String> {
    let mark = into.join(format!("asked-again-{which}"));
    if std::fs::symlink_metadata(state.join(crate::skills::levels::DIR)).is_err()
        || std::fs::symlink_metadata(&mark).is_ok()
    {
        return Ok(());
    }
    crate::skills::levels::ask_again_in(into, root)
        .map_err(|why| format!("the levels records were not asked about again ({why})"))?;
    written_once(&mark, "").map(|_| ())
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

/// The stores a move of the project carries to its new clone key ([`follow_arrival`]): the person's
/// own work and records. Nothing that holds the person's word is carried: the grade-after-edit
/// opt-ins are turned on again from a plan, and the levels records are asked about again.
pub const CARRIED_ON_A_MOVE: [(&str, Carried); 3] = [
    (crate::skills::verbs::drafts::DRAFTS_DIR, Carried::Moved),
    (crate::skills::grades::LOCAL_DIR, Carried::Appended),
    (crate::skills::grades::kept::DIR_NAME, Carried::Moved),
];

/// What a project's open found about where its record of open chats came from, for
/// [`follow_arrival`]: the two V43 answers that concern its app state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrived {
    /// `reopen::Arrival::Moved`.
    Moved,
    /// `reopen::Arrival::Copied`.
    Copied,
}

/// The mark in a project's app state of a carry from the old clone key `<key>` under way:
/// `moving-from-<key>`, holding the old root, renamed `moved-from-<key>` once all of it is carried.
const MOVING: &str = "moving-from-";

/// The mark of a carry from `<key>` that is whole.
const MOVED: &str = "moved-from-";

/// **The app's state of the project at `root` follows where its record of open chats came from**
/// (#3718), with `was` the clone the record names, at its open, before any chat is put back:
///
/// - **A copy** is given an id of its own and starts with none of the original's state. A record
///   V43 reads as a move is a copy too while its old root is still a project holding the same id:
///   the original keeps everything.
/// - **A move** carries the old clone key's drafts, local grades and kept grade chats
///   ([`CARRIED_ON_A_MOVE`]), only where the old key's folder says it is this project's (its
///   [`ROOT_FILE`] names `was.root` and its [`OWNER`] is the project's [`PLANE_ID`], which moved
///   with it). Its opt-ins and levels records are not carried, and where it held levels records
///   every workspace and persona folder waits for the person's Accept again. The old folder is
///   removed once all of it is carried; a carry cut short is taken up again at the next open
///   ([`moves_taken_up`]).
///
/// Nothing where there is no data home.
pub fn follow_arrival(root: &Path, was: &crate::reopen::CloneSeat, arrived: Arrived) -> Moved {
    match data_home(root) {
        Some(data) => follow_arrival_in(&data, root, was, arrived),
        None => Moved::default(),
    }
}

/// [`follow_arrival`], in the data home `data`.
fn follow_arrival_in(
    data: &Path,
    root: &Path,
    was: &crate::reopen::CloneSeat,
    arrived: Arrived,
) -> Moved {
    let mut out = Moved::default();
    let here = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let id = record_text(&plane_id_file(root));
    let original_there = was.root != here
        && was.root.is_dir()
        && id.is_some()
        && record_text(&plane_id_file(&was.root)) == id;
    if arrived == Arrived::Copied || original_there {
        // A copy gets an id of its own, and its folders wait for the person's Accept, as a
        // moved project's do: what the original vouched for is not the copy's.
        let held = reminted(data, root)
            .and_then(|()| made_in(data, root))
            .and_then(|mine| crate::skills::levels::ask_again_in(&mine, root));
        if let Err(why) = held {
            out.left
                .push(format!("the copy was not given an id of its own ({why})"));
            // Never left sharing the original's id: its app state is set aside, unread.
            let mine = in_data(data, root);
            if let Err(also) = locked(&mine).and_then(|_held| set_aside(&mine).map(|_| ())) {
                out.left
                    .push(format!("the copy's app state was not set aside ({also})"));
            }
        }
        for line in &out.left {
            tracing::warn!(target: "purlis::appstate", "purlis: {line}");
        }
        return out;
    }
    carried_from(data, root, was.key.as_str(), &was.root)
}

/// **Each carry from an old clone key that was cut short** ([`MOVING`]) taken up again, in the
/// project at `root`: what its open does, a look at its own folder's names.
pub fn moves_taken_up(root: &Path) -> Vec<Moved> {
    let Some(data) = data_home(root) else {
        return Vec::new();
    };
    let here = in_data(&data, root);
    let Ok(entries) = std::fs::read_dir(&here) else {
        return Vec::new();
    };
    let keys: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_prefix(MOVING))
                .and_then(crate::plane::CloneKey::parse)
                .map(|key| key.to_string())
        })
        .collect();
    keys.iter()
        .filter_map(|key| {
            let old_root = record_text(&here.join(format!("{MOVING}{key}")))?;
            Some(carried_from(&data, root, key, Path::new(&old_root)))
        })
        .collect()
}

/// The carry of a move from the old clone key `key`, whose project was at `old_root`.
fn carried_from(data: &Path, root: &Path, key: &str, old_root: &Path) -> Moved {
    let mut out = Moved::default();
    let old = data.join(DIR).join(key);
    let here = in_data(data, root);
    if old == here || !std::fs::symlink_metadata(&old).is_ok_and(|meta| meta.is_dir()) {
        return out;
    }
    let id = record_text(&plane_id_file(root));
    let its_own = old_root.to_str() == record_text(&old.join(ROOT_FILE)).as_deref()
        && id.is_some()
        && record_text(&old.join(OWNER)) == id;
    // Never while the old root is still a project holding this same id: that one is a copy's
    // original, and keeps what it has.
    let original_there = old_root.is_dir() && record_text(&plane_id_file(old_root)) == id;
    if !its_own || original_there {
        tracing::warn!(target: "purlis::appstate", "purlis: the app state of {} is not this project's to carry, so nothing was carried", old_root.display());
        return out;
    }
    let into = match made_in(data, root) {
        Ok(into) => into,
        Err(why) => {
            out.left.push(format!("nothing was carried ({why})"));
            return out;
        }
    };
    let moving = into.join(format!("{MOVING}{key}"));
    if std::fs::symlink_metadata(&moving).is_err() {
        let Some(text) = old_root.to_str() else {
            return out;
        };
        if let Err(why) = written_once(&moving, text) {
            out.left.push(format!("the carry was not begun ({why})"));
            return out;
        }
    }
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
    if out.left.is_empty() {
        if let Err(why) = std::fs::rename(&moving, into.join(format!("{MOVED}{key}"))) {
            out.left
                .push(format!("the carry was not marked whole ({why})"));
        } else if let Err(why) = removed(&old) {
            out.left
                .push(format!("the old app state was not removed ({why})"));
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
