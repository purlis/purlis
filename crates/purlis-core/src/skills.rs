// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! A chat's skills, as purlis-core asks [`purlis_skills`] for them (ADR 0063, SK-1).
//!
//! How a chat's skill set is worked out is `purlis-skills`' alone: which skills a chat gets, by
//! which [`Route`] and from which [`Origin`], and why each left-out one is left out. This module
//! hands that crate what purlis-core knows about a chat, where the chat is armed
//! ([`for_chat`]) and where a briefed chat's `SessionStart` lists its skills ([`briefed`]), and
//! it holds the `skill-studio` flag ([`studio_on`]). A harness adapter asks [`for_chat`] and
//! turns the answer into its harness's argv, env and config; the route each harness takes is its
//! declaration's fact, [`crate::harness::Harness::skills`].

use std::path::{Path, PathBuf};

pub use purlis_skills::change::{Problem, Template, check};
pub use purlis_skills::{
    Argument, At, BUILT_IN_PREFIXES, Budget, Chat, Clash, Compat, DIR_IN_BUNDLE, Declared,
    Effective, Field, Finding, Fit, FromHarness, Given, Grant, HarnessSkills, Honours, LOCAL_DIR,
    LeftOut, Lint, Machine, ManualOnly, Missing, Narrowing, OnMachine, Origin, Outcome,
    OutsideSkill, Over, Owner, POINTER, PROJECT_DIR, Reason, Reference, Refusal, Reload, Route,
    Severity, Skill, SkillSet, Spent, Stamp, Switch, Whose, beside, compat, declared, grant_file,
    grant_of, handed_text, in_bundle, level_dir, level_shown, listing, named, persona_dir, read,
    scan, set_grant, share,
};

/// Skill runs: the span from the moment a skill starts to the end of the turn (SK-3).
pub use purlis_skills::runs;
/// The install review's summary and a source's tree (SK-10).
pub use purlis_skills::{review, tree};
pub mod source;
pub use purlis_skills::{About, Deprecated, Lifecycle, about_of};
pub mod graph;
pub mod history;
pub mod lifecycle;
pub mod verbs;

use crate::harness::{Harness, Kit};

pub mod levels;
mod on_machine;
pub mod persona;

pub use on_machine::on_machine;
pub(crate) use on_machine::{folders, skill_dirs};

/// The variable a chat whose harness cannot load its skills is started with: the directory of
/// the built-in skills its set holds ([`SkillSet::built_in_dir`]), whose skills its `SessionStart` briefing then
/// lists ([`briefed`]). Set by the app for such a chat alone, so a harness that loads the skills
/// itself is never briefed on them twice.
pub const LISTED_ENV: &str = "PURLIS_SKILLS_DIR";

/// The variable a briefed chat is started with when its set holds the plane's skills (ADR 0099):
/// the set's folder purlis laid out for it ([`lay_out`]), and its `SessionStart` briefing then
/// lists them with the built-in ones, each by the `SKILL.md` purlis wrote for it there (SK-8), so
/// a briefed chat reads what every other route hands. Set only where the `skill-studio` flag let
/// the plane's skills into the chat's set and their folder was laid out, so a briefing never reads
/// the plane where arming did not.
pub const PLANE_LISTED_ENV: &str = "PURLIS_SKILLS_PLANE";

/// The plane a chat's skills come from beside the built-in ones (ADR 0099), as a chat is armed.
#[derive(Debug, Clone, Copy)]
pub struct PlaneSkills<'a> {
    /// The plane's root.
    pub plane: &'a Path,
    /// The workspace the chat works in, or `None` for a chat at the plane root.
    pub workspace: Option<&'a str>,
    /// The folder of links to the plane's skills of the chat's set, which the host laid out
    /// before arming it ([`lay_out_for_chat`]), or `None` where it laid out none. A harness that
    /// needs the folder is then handed the built-in skills alone.
    pub linked: Option<&'a Path>,
    /// The repo skills of the clone the chat works in, for its harness, which the host read
    /// ([`repo_skills_of_chat`]): where one has a plane skill's name and purlis builds what the
    /// harness is handed, the narrower repo copy wins and the plane's is left out (SK-6, decision
    /// 51). Empty for a chat in no clone.
    pub repo: &'a [OutsideSkill],
}

/// The skill set of a chat on `harness` armed with `kit`, asked of [`purlis_skills::skill_set`].
///
/// The built-in skills come from the bundle in `kit`, and the plane's from `kit.skills`, where
/// the `skill-studio` flag let them in.
pub fn for_chat(kit: Kit<'_>, harness: Harness) -> SkillSet {
    let built_in = kit.plugin.and_then(in_bundle);
    ask(
        built_in.as_deref(),
        kit.skills,
        kit.persona.map(|persona| persona.persona),
        harness.skills(),
        None,
    )
}

/// **The harness's own skills a chat on `harness` armed with `kit` is handed as turned off**
/// (SK-6, #2769): each of `chosen`'s, except a built-in skill's name, which is never turned off
/// (a harness that switches by the name alone would reach purlis's own), and one the harness
/// cannot leave out on its own ([`crate::harness_plugin::Adapter::skill_named_alone`]). What
/// every adapter turns into its own switch.
pub fn skills_off_for(
    kit: Kit<'_>,
    harness: Harness,
    chosen: &crate::harness_plugin::Chosen,
) -> Vec<String> {
    let built_in = built_in_names(kit.plugin.and_then(in_bundle).as_deref());
    let adapter = harness.adapter().plugins();
    chosen
        .skills_off
        .iter()
        .filter(|name| !built_in.contains(name))
        .filter(|name| adapter.skill_named_alone(name).is_none())
        .cloned()
        .collect()
}

/// The names of the built-in skills of the bundle installed beside this binary
/// ([`built_in_here`]): what a save of `[harness_skills]` refuses to turn off.
pub fn built_in_names_here() -> Vec<String> {
    built_in_names(built_in_here().as_deref())
}

/// The names of the built-in skills in `dir`, the bundle's `skills/` ([`in_bundle`]): none where
/// there is no bundle. The one list a switch, a save and an adapter refuse to turn off.
pub fn built_in_names(dir: Option<&Path>) -> Vec<String> {
    dir.map(|dir| read(dir).into_iter().map(|skill| skill.name).collect())
        .unwrap_or_default()
}

/// The folder of links a harness on [`Route::Plugin`] or [`Route::Config`] is handed the plane's
/// skills of `set` by: the one the host laid out (`kit.skills`' `linked`), where the set holds a
/// plane skill. Nothing is read or written here: an adapter only turns the set into argv, env and
/// config (ADR 0063 as amended 2026-10-20).
pub fn linked<'a>(set: &SkillSet, kit: Kit<'a>) -> Option<&'a Path> {
    set.from_the_plane().next()?;
    kit.skills?.linked
}

/// **The host's part of arming a chat with the plane's skills**: the folder of links for the
/// plane's skills of the set of a chat in `plane`, at its root or in `workspace`, running as
/// `persona`, with the built-in skills in `built_in`, laid out under `sets`. `None` where the set
/// holds no plane skill, or the folder could not be laid out (said in the log: the chat then has
/// the built-in skills alone). The set is the one [`for_chat`] works out for that chat on
/// `harness`, with `repo` the repo skills of the clone it works in: a plane skill a narrower
/// repo copy hides is not laid out (SK-6). Each skill is handed as `harness` reads it
/// ([`crate::harness::Harness::skill_frontmatter`], SK-8).
pub fn lay_out_for_chat(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    built_in: Option<&Path>,
    sets: &Path,
    harness: Harness,
    repo: &[OutsideSkill],
) -> Option<PathBuf> {
    let at = PlaneSkills {
        plane,
        workspace,
        linked: None,
        repo,
    };
    let set = ask(built_in, Some(at), persona, harness.skills(), None);
    match lay_out(&set, sets, &harness.skill_frontmatter()) {
        Ok(folder) => folder,
        Err(why) => {
            tracing::warn!(
                "purlis: the project's skills could not be laid out for this chat under {} \
                 ({why}); it has the built-in skills alone",
                sets.display()
            );
            None
        }
    }
}

/// The name of the plugin a set's folder is, in Claude Code: its skills are `skills:<name>`.
pub const LINKED_PLUGIN: &str = "skills";

/// **The folder of the plane's skills of `set`**, under `sets`, laid out if it is not there
/// yet, for a harness that reads `honours`, or `None` for a set with no plane skill purlis hands
/// (ADR 0063 as amended 2026-10-20 and 2026-10-22).
///
/// `<sets>/<digest>/` holds `.claude-plugin/plugin.json`, naming the plugin [`LINKED_PLUGIN`],
/// and `skills/<name>`, a folder of each skill's own ([`handout`]): the `SKILL.md` purlis writes
/// for it, never with what would widen the chat, and a link to each plain file of the skill's
/// folder. So Claude Code loads it as a plugin, opencode scans its `skills/`, and a Codex
/// briefing names its files. A skill purlis refuses is left out, and its chat's tab says why
/// ([`not_here`]). `<digest>` is a SHA-256 over every name, folder, text and entry, so the folder
/// of one set is never another's, and it is complete before it takes that name: a running
/// chat's folder never changes. One found complete is used as it is; one that is not is made
/// again.
pub fn lay_out(
    set: &SkillSet,
    sets: &Path,
    honours: &purlis_skills::Honours,
) -> std::io::Result<Option<PathBuf>> {
    let handed: Vec<handout::Handed> = set
        .from_the_plane()
        .filter(|effective| plain_name(&effective.skill.name))
        .filter_map(|effective| handout::of(&effective.skill, honours).ok())
        .collect();
    if handed.is_empty() {
        return Ok(None);
    }
    let digest = {
        use sha2::Digest;
        let mut hash = sha2::Sha256::new();
        for one in &handed {
            handout::digested(one, &mut hash);
        }
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let folder = sets.join(&digest);
    std::fs::create_dir_all(sets)?;
    // The sets' folder, opened as a folder itself once: its lock and the removal of an
    // incomplete folder go through this handle, never back through the path.
    #[cfg(unix)]
    let sets_dir = at::folder(sets).map_err(|why| {
        if at::not_a_folder(&why) {
            std::io::Error::other(format!("{} is a link, not a folder", sets.display()))
        } else {
            why
        }
    })?;
    // Held against [`tidy`] while it is laid out and found: a folder this returns has just been
    // made or touched, so a tidy after it keeps it for [`TIDY_GRACE`].
    #[cfg(unix)]
    let _laying_out = lock_shared_in(&sets_dir)?;
    #[cfg(not(unix))]
    let _laying_out = lock_shared(sets)?;
    if laid_out(&folder, &handed) && touched(&folder) {
        return Ok(Some(folder));
    }
    let building = tempfile::Builder::new()
        .prefix(&format!(".{digest}."))
        .tempdir_in(sets)?;
    build(building.path(), &handed)?;
    if std::fs::symlink_metadata(&folder).is_ok() {
        // Not complete, so not a set's folder any chat was started with: made again.
        #[cfg(unix)]
        at::remove_tree_in(&sets_dir, &digest)?;
        #[cfg(not(unix))]
        std::fs::remove_dir_all(&folder)?;
    }
    let built = building.keep();
    match std::fs::rename(&built, &folder) {
        Ok(()) => Ok(Some(folder)),
        // Another chat's start laid the same set out first.
        Err(_) if laid_out(&folder, &handed) => {
            let _ = std::fs::remove_dir_all(&built);
            Ok(Some(folder))
        }
        Err(why) => {
            let _ = std::fs::remove_dir_all(&built);
            Err(why)
        }
    }
}

/// Builds a set's folder of `handed` in `building`, a folder of purlis's own that nothing else
/// knows of yet: through the handles of the folders made in it, where there are handles.
#[cfg(unix)]
fn build(building: &Path, handed: &[handout::Handed]) -> std::io::Result<()> {
    use std::io::Write as _;
    let mode = rustix::fs::Mode::from_raw_mode(0o755);
    let root = at::folder(building)?;
    rustix::fs::mkdirat(&root, ".claude-plugin", mode)?;
    let manifest = at::folder_in(&root, ".claude-plugin")?;
    at::new_file_in(&manifest, "plugin.json")?.write_all(manifest_text().as_bytes())?;
    rustix::fs::mkdirat(&root, DIR_IN_BUNDLE, mode)?;
    let skills = at::folder_in(&root, DIR_IN_BUNDLE)?;
    for one in handed {
        handout::make_in(&skills, one)?;
    }
    Ok(())
}

/// [`build`] where there are no handles to open a folder by.
#[cfg(not(unix))]
fn build(building: &Path, handed: &[handout::Handed]) -> std::io::Result<()> {
    let manifest = building.join(".claude-plugin");
    std::fs::create_dir(&manifest)?;
    std::fs::write(manifest.join("plugin.json"), manifest_text())?;
    let skills = building.join(DIR_IN_BUNDLE);
    std::fs::create_dir(&skills)?;
    for one in handed {
        handout::make_at(&skills, one)?;
    }
    Ok(())
}

/// Marks `folder` as used now, by its modification time, which [`tidy`] keeps a folder for.
fn touched(folder: &Path) -> bool {
    std::fs::File::open(folder)
        .and_then(|dir| dir.set_modified(std::time::SystemTime::now()))
        .is_ok()
}

/// The variable a chat's start carries the folder of links it is handed by (SK-24): set by the
/// host as it arms the chat ([`crate::hoststart::armed`]), and taken out of the chat's
/// environment again by [`take_hold`] where its terminal is started, so its program never sees
/// it and the terminal notes, while the program runs, that it uses the folder.
pub const SET_ENV: &str = "PURLIS_SKILL_SET";

/// The folder of a set's folder's notes, beside the sets: `<sets>/.held/<digest>/<pid>`, one
/// file for each running program that was handed it ([`crate::session::Spec::holds`]).
pub(crate) const HELD: &str = ".held";

/// The file [`lay_out`] and [`tidy`] take turns on, in the folder of the sets.
const LOCK: &str = ".lock";

/// How long a set's folder is kept after it was last laid out or found, whether or not a
/// running program is noted as using it: the time between a host laying it out and the chat's
/// terminal noting it, with room to spare.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) const TIDY_GRACE: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// **Where a program handed the set's folder `linked` is noted as using it**: `<sets>/.held/
/// <digest>`, or `None` for a folder that is not a set's folder of links ([`lay_out`]'s
/// `<sets>/<digest>`, a real folder).
pub(crate) fn hold_of(linked: &Path) -> Option<PathBuf> {
    let digest = linked.file_name()?.to_str()?;
    let sets = linked.parent()?;
    (is_digest(digest)
        && sets.file_name() == Some(std::ffi::OsStr::new(crate::hoststart::SKILL_SETS))
        && crate::leftovers::is_plain_dir(linked))
    .then(|| sets.join(HELD).join(digest))
}

/// **Takes [`SET_ENV`] out of a chat's `env`**, and answers where its program is noted as using
/// the folder it names ([`hold_of`]), where it names a set's folder.
pub fn take_hold(env: &mut Vec<(std::ffi::OsString, std::ffi::OsString)>) -> Option<PathBuf> {
    // Under every name it goes by (V93k), as the chat's environment twins it.
    let names = crate::envvar::spellings(SET_ENV);
    let mut handed = None;
    env.retain(|(key, value)| {
        if !names.iter().any(|name| key == name.as_str()) {
            return true;
        }
        handed.get_or_insert_with(|| PathBuf::from(value));
        false
    });
    handed.as_deref().and_then(hold_of)
}

/// What one [`tidy`] did: the sets' folders it removed, by digest, and each it could not, with
/// why.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Tidied {
    pub removed: Vec<String>,
    pub failed: Vec<(String, String)>,
    /// Nothing was looked at: a chat's start was laying its folder out, and a tidy never makes
    /// one wait. The next tidy looks.
    pub busy: bool,
}

/// **Removes every set's folder under `sets` no running program uses** (SK-24): one no live
/// program is noted in ([`HELD`]) and that was not laid out or found within [`TIDY_GRACE`],
/// with its notes, and what a lay-out that never finished left. A tidy never makes a chat's
/// start wait: while one is laying a folder out ([`lay_out`]), it looks at nothing (`busy`).
///
/// **Only `sets`' own entries are removed, by the names purlis gives them**: a folder named by
/// a digest, a lay-out's leftover beside it, and the notes in [`HELD`] named by a process id.
/// **Nothing is read, removed or written through a link**, even one put in place after a check:
/// `sets`, [`HELD`], each folder of notes and each folder removed are opened as folders
/// themselves and worked through their handles (`skills::at`). A link at [`HELD`] or at a
/// folder of notes keeps the folders it may speak for. The links inside a set's folder are
/// removed as links. Off unix nothing is removed: whether a program still runs cannot be
/// asked there ([`crate::process::alive`]).
pub fn tidy(sets: &Path) -> std::io::Result<Tidied> {
    #[cfg(unix)]
    {
        tidy_through_handles(sets)
    }
    #[cfg(not(unix))]
    {
        let _ = sets;
        Ok(Tidied::default())
    }
}

/// What a folder of notes says of its set's folder.
#[cfg(unix)]
#[derive(PartialEq)]
enum Notes {
    /// None of a running program, or none at all.
    None,
    /// One of a running program, or notes that cannot be read for a link in their place.
    Kept,
}

#[cfg(unix)]
fn tidy_through_handles(sets: &Path) -> std::io::Result<Tidied> {
    use rustix::fs::FileType;
    let mut tidied = Tidied::default();
    let sets_dir = match at::folder(sets) {
        Ok(dir) => dir,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound || at::not_a_folder(&why) => {
            return Ok(tidied);
        }
        Err(why) => return Err(why),
    };
    let Some(_tidying) = try_lock_in(&sets_dir)? else {
        tidied.busy = true;
        return Ok(tidied);
    };
    let now = std::time::SystemTime::now();
    let fresh = |entry: &at::Entry| {
        entry.modified.is_some_and(|modified| {
            now.duration_since(modified)
                .is_ok_and(|age| age < TIDY_GRACE)
        })
    };
    // `None` where there are no notes at all; `Err` where something not a folder is in their
    // place, which keeps every set's folder.
    let held_dir = match at::folder_in(&sets_dir, HELD) {
        Ok(dir) => Ok(Some(dir)),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(why) => Err(why),
    };
    let notes_of = |digest: &str| match &held_dir {
        Err(_) => Notes::Kept,
        Ok(None) => Notes::None,
        Ok(Some(held)) => match at::folder_in(held, digest) {
            Ok(notes) => still_held(&notes),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => Notes::None,
            Err(_) => Notes::Kept,
        },
    };
    for entry in at::entries(&sets_dir)? {
        let name = entry.name.clone();
        let digest = is_digest(&name);
        if entry.kind != FileType::Directory || !(digest || leftover(&name)) || fresh(&entry) {
            continue;
        }
        if digest && notes_of(&name) == Notes::Kept {
            continue;
        }
        match at::remove_tree_in(&sets_dir, &name) {
            Ok(()) => {
                if digest {
                    if let Ok(Some(held)) = &held_dir {
                        let _ = at::remove_empty_folder_in(held, &name);
                    }
                    tidied.removed.push(name);
                }
            }
            Err(why) => tidied.failed.push((name, why.to_string())),
        }
    }
    // Notes of a folder that has gone, of programs that have gone too.
    if let Ok(Some(held)) = &held_dir {
        for entry in at::entries(held)? {
            let gone = at::entry_in(&sets_dir, &entry.name)?.is_none();
            if entry.kind == FileType::Directory
                && is_digest(&entry.name)
                && gone
                && notes_of(&entry.name) == Notes::None
            {
                let _ = at::remove_empty_folder_in(held, &entry.name);
            }
        }
    }
    Ok(tidied)
}

/// **What the folder of notes `notes` (one set's folder in [`HELD`], opened as a folder itself)
/// says**: [`Notes::Kept`] where a file in it is named by the id of a process that runs and
/// that had started by the time the file was written ([`crate::process::could_be_recorded`]).
/// Every other file named by a process id is removed, through the folder's handle. Anything
/// not named by one is never removed, only said in the log. A process whose start cannot be
/// read is taken as the one noted.
#[cfg(unix)]
fn still_held(notes: &std::os::fd::OwnedFd) -> Notes {
    use rustix::fs::FileType;
    let Ok(entries) = at::entries(notes) else {
        return Notes::Kept;
    };
    let mut held = Notes::None;
    for entry in entries {
        let Ok(pid) = entry.name.parse::<u32>() else {
            tracing::warn!(
                "purlis: {:?} among a skills folder's notes is not one purlis wrote, so it was \
                 left as it is",
                entry.name
            );
            continue;
        };
        if entry.kind != FileType::RegularFile {
            continue;
        }
        let runs = crate::process::alive(pid)
            && match (started_at(pid), entry.modified) {
                (Some(started), Some(written)) => {
                    crate::process::could_be_recorded(started, written)
                }
                _ => true,
            };
        if runs {
            held = Notes::Kept;
        } else {
            let _ = at::remove_file_in(notes, &entry.name);
        }
    }
    held
}

#[cfg(unix)]
pub(crate) mod at;
mod handout;

#[cfg(unix)]
fn started_at(pid: u32) -> Option<std::time::SystemTime> {
    purlis_same_user::Parents::started_at(pid)
}

/// **[`tidy`] of this user's sets' folders, said in the log**, on a thread of its own after
/// `wait`, so nothing waits on it: what the app does as it starts and after a chat closes.
///
/// A fenced test build tidies nothing outside its fence, as it reads no store there
/// ([`studio_on_here`]).
pub fn tidy_here_later(wait: std::time::Duration) {
    let Some(sets) = crate::hoststart::skill_sets_here()
        .filter(|sets| !crate::fence::FENCED || crate::fence::inside(sets, &crate::fence::fence()))
    else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("purlis-skill-sets-tidy".into())
        .spawn(move || {
            std::thread::sleep(wait);
            said_in_the_log(&sets, tidy(&sets));
        });
}

/// How long after a chat closes its sets are tidied ([`tidy_here_later`]): time for its program
/// to be ended and its note to go, so the folder it used goes in the same tidy.
pub const TIDY_AFTER_A_CLOSE: std::time::Duration = std::time::Duration::from_secs(5);

/// What a [`tidy`] of `sets` did, in the log.
pub fn said_in_the_log(sets: &Path, tidied: std::io::Result<Tidied>) {
    match tidied {
        Ok(tidied) => {
            if tidied.busy {
                tracing::debug!(
                    "purlis: a chat was starting, so unused skills folders in {} are looked \
                     for after the next chat closes",
                    sets.display()
                );
            }
            for digest in &tidied.removed {
                tracing::info!(
                    "purlis: removed the skills folder {digest} in {}: no running chat uses it",
                    sets.display()
                );
            }
            for (digest, why) in &tidied.failed {
                tracing::warn!(
                    "purlis: the skills folder {digest} in {} could not be removed ({why}); \
                     it is tried again after the next chat closes",
                    sets.display()
                );
            }
        }
        Err(why) => tracing::warn!(
            "purlis: unused skills folders in {} could not be looked for ({why}); they are \
             looked for again after the next chat closes",
            sets.display()
        ),
    }
}

/// Whether `name` is a set's folder's: a SHA-256, in lowercase hex.
fn is_digest(name: &str) -> bool {
    crate::leftovers::is_lowercase_hex(name, 64)
}

/// Whether `name` is what a lay-out that never finished left: `.<digest>.<suffix>`.
#[cfg_attr(not(unix), allow(dead_code))]
fn leftover(name: &str) -> bool {
    name.strip_prefix('.')
        .and_then(|rest| rest.split_once('.'))
        .is_some_and(|(digest, suffix)| {
            is_digest(digest)
                && !suffix.is_empty()
                && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
        })
}

/// The sets' lock ([`LOCK`]) in the folder `sets`, opened through its handle and never through
/// a link, taken only where nothing holds it: `None` while a lay-out does.
#[cfg(unix)]
fn try_lock_in(sets: &std::os::fd::OwnedFd) -> std::io::Result<Option<std::fs::File>> {
    let file = at::file_in(sets, LOCK)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(why)) => Err(why),
    }
}

/// The sets' lock ([`LOCK`]) in the folder `sets`, opened through its handle and never through
/// a link, taken shared as a lay-out takes it, until the file is dropped: waiting for a tidy to
/// finish, which never takes long.
#[cfg(unix)]
fn lock_shared_in(sets: &std::os::fd::OwnedFd) -> std::io::Result<std::fs::File> {
    let file = at::file_in(sets, LOCK)?;
    file.lock_shared()?;
    Ok(file)
}

/// [`lock_shared_in`] where there are no handles to open a folder by.
#[cfg(not(unix))]
fn lock_shared(sets: &Path) -> std::io::Result<std::fs::File> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(sets.join(LOCK))?;
    file.lock_shared()?;
    Ok(file)
}

/// The plugin manifest a set's folder holds.
fn manifest_text() -> String {
    serde_json::json!({
        "name": LINKED_PLUGIN,
        "description": "This project's skills, as purlis hands them to this chat",
    })
    .to_string()
}

/// Whether `folder` is the complete folder of `handed`: its manifest, and exactly the folder
/// of each skill as handed ([`handout::holds`]).
fn laid_out(folder: &Path, handed: &[handout::Handed]) -> bool {
    let manifest = std::fs::read_to_string(folder.join(".claude-plugin/plugin.json"));
    manifest.ok().as_deref() == Some(manifest_text().as_str())
        && handout::holds(&folder.join(DIR_IN_BUNDLE), handed)
}

/// Whether `name` can name a link in a set's folder: one plain path component.
fn plain_name(name: &str) -> bool {
    let mut parts = Path::new(name).components();
    matches!(
        (parts.next(), parts.next()),
        (Some(std::path::Component::Normal(_)), None)
    ) && !name.contains(['/', '\\'])
}

/// The briefing section that lists the skills of a chat started with [`LISTED_ENV`], or none
/// where it is unset or the set is empty.
///
/// The set is asked of [`purlis_skills::skill_set`] with what the chat was armed with — the
/// directory, the persona the chat runs as in the plane at `root`, and the briefing route,
/// the only one the variable is set for — so it lists what the chat's set holds, never every
/// folder in the directory.
pub fn briefed(
    root: &Path,
    workspace: Option<&str>,
    cwd: Option<&Path>,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let dir = env(LISTED_ENV).filter(|dir| !dir.is_empty())?;
    let persona = crate::envvar::lookup(crate::active::PERSONA_ENV, env).filter(|p| !p.is_empty());
    let handed = handed_folder(env);
    let plane = handed.is_some();
    // The repo skills of the clone the chat works in, as each briefed harness reads them: the
    // briefing lists one copy of a name, the narrower (SK-6, decision 51).
    let repo: Vec<OutsideSkill> = match (plane, workspace, cwd) {
        (true, Some(ws), Some(cwd)) => Harness::ALL
            .iter()
            .filter(|harness| harness.skills() == Route::Briefing)
            .flat_map(|harness| repo_skills_of_chat(*harness, root, ws, cwd))
            .collect(),
        _ => Vec::new(),
    };
    let set = ask(
        Some(Path::new(&dir)),
        plane.then_some(PlaneSkills {
            plane: root,
            workspace,
            linked: None,
            repo: &repo,
        }),
        persona.as_deref(),
        Route::Briefing,
        None,
    );
    // Only what purlis hands: the harness lists its own and the repo's itself. A plane skill is
    // named by the `SKILL.md` purlis wrote for it, and one it refused is not named at all.
    let skills: Vec<Skill> = set
        .effective
        .into_iter()
        .filter(|e| e.route != Route::Own)
        .filter_map(|e| {
            if !e.origin.of_the_plane() {
                return Some(e.skill);
            }
            let path = handed
                .as_ref()?
                .join(DIR_IN_BUNDLE)
                .join(&e.skill.name)
                .join(purlis_skills::FILE);
            purlis_skills::read_skill_md(&path).ok()?;
            Some(Skill { path, ..e.skill })
        })
        .collect();
    listing(&skills)
}

/// The set's folder a briefed chat was handed its plane skills in ([`PLANE_LISTED_ENV`]), where
/// it names a folder.
fn handed_folder(env: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    env(PLANE_LISTED_ENV)
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from)
        .filter(|folder| folder.is_absolute())
}

/// **The skills a briefed chat may start by reading their `SKILL.md`** (SK-3): every skill in
/// the directory it was started with in [`LISTED_ENV`], and the plane's in the set's folder
/// [`PLANE_LISTED_ENV`] names, each by the name of its folder there, as the briefing names it.
/// Empty for a chat that was not briefed: a harness that loads its skills itself starts them by
/// its own tool.
pub fn readable_by_a_briefed_chat(env: &dyn Fn(&str) -> Option<String>) -> Vec<Skill> {
    let Some(dir) = env(LISTED_ENV).filter(|dir| !dir.is_empty()) else {
        return Vec::new();
    };
    let mut skills = read(Path::new(&dir));
    if let Some(handed) = handed_folder(env) {
        skills.extend(read(&handed.join(DIR_IN_BUNDLE)).into_iter().map(|skill| {
            Skill {
                name: skill
                    .path
                    .parent()
                    .and_then(Path::file_name)
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or(skill.name.clone()),
                ..skill
            }
        }));
    }
    skills
}

/// **A running chat, as Use skill… asks for its skills** (SK-12, #2775): where it works, as whom,
/// on which harness, and whether the `skill-studio` flag lets the plane's skills into its set.
#[derive(Debug, Clone, Copy)]
pub struct Running<'a> {
    /// The plane's root.
    pub plane: &'a Path,
    /// The workspace it works in, or `None` at the plane root.
    pub workspace: Option<&'a str>,
    /// The persona it was started as.
    pub persona: Option<&'a str>,
    pub harness: Harness,
    /// The built-in skills' directory, where the app found it.
    pub built_in: Option<&'a Path>,
    /// Whether its set holds the plane's skills: the flag, as at a chat's start.
    pub plane_skills: bool,
}

/// One skill Use skill… offers a chat: one of its set, with what it declares and the name its
/// harness knows it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    pub effective: Effective,
    pub declared: Declared,
    /// Whether it asks for anything before it is typed: arguments it names, or a hint at what to
    /// pass. The one place this is decided; the window draws the boxes by it.
    pub takes_arguments: bool,
    /// The name the chat's harness knows it by, which a run of it records
    /// ([`Harness::skill_name`]).
    pub known_as: String,
}

/// **What Use skill… offers a chat, and its skill budget** (SK-12): every skill of its set, the
/// manual-only ones among them, by name, and what the set spends of its harness's budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSkills {
    /// The chat's harness, which the skills are typed in the syntax of.
    pub harness: Harness,
    /// The workspace the chat works in, `None` at the plane root: where a run it starts is.
    pub workspace: Option<String>,
    pub offered: Vec<Offered>,
    pub spent: Spent,
}

/// **The skills of chat `chat`, as Use skill… offers them** (SK-12): exactly its set, worked out
/// as its start worked it out (the plane's skills only where the flag let them in, as
/// [`of_a_chat`] works them out), each with what it declares, and the budget the set spends on
/// its harness.
pub fn for_use_skill(chat: &Running<'_>) -> ChatSkills {
    let route = chat.harness.skills();
    let set = if chat.plane_skills {
        of_a_chat(
            chat.built_in,
            chat.plane,
            chat.workspace,
            chat.persona,
            route,
        )
    } else {
        ask(chat.built_in, None, chat.persona, route, None)
    };
    let offered: Vec<Offered> = set
        .effective
        .into_iter()
        .map(|effective| {
            let declared = declared(&effective.skill);
            Offered {
                takes_arguments: !declared.arguments.is_empty() || declared.hint.is_some(),
                known_as: chat
                    .harness
                    .skill_name(&effective.skill.name, effective.origin),
                declared,
                effective,
            }
        })
        .collect();
    let spent = purlis_skills::budget::spent(
        offered
            .iter()
            .map(|one| (&one.effective.skill, &one.declared)),
        chat.harness.skill_budget(),
    );
    ChatSkills {
        harness: chat.harness,
        workspace: chat.workspace.map(str::to_owned),
        offered,
        spent,
    }
}

/// What Use skill… types into a chat to start one of its skills ([`ChatSkills::typed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Typing {
    /// The text, in the chat's harness's syntax, on one line.
    pub text: String,
    /// The run it starts once its prompt is sent, its id minted now: the name the harness knows
    /// the skill by, which copy it is, and how its start is confirmed
    /// ([`crate::skillspan::Typed`]).
    pub held: crate::skillspan::Typed,
}

impl ChatSkills {
    /// **The text that starts skill `name` of this chat's set**, with `values` for its arguments
    /// in the order it declares them ([`Given::of`]), and the run it starts, or why there is
    /// none: a skill that is not in the chat's set is never typed.
    pub fn typed(&self, name: &str, values: &[String]) -> Result<Typing, String> {
        let offered = self
            .offered
            .iter()
            .find(|one| one.effective.skill.name == name)
            .ok_or_else(|| {
                format!("{name} is not one of this chat's skills, so nothing was typed")
            })?;
        let given = Given::of(&offered.declared, values);
        let text = self
            .harness
            .skill_start_text(&crate::harness::SkillToStart {
                name: &offered.effective.skill.name,
                origin: offered.effective.origin,
                given: &given,
            });
        let held = crate::skillspan::Typed {
            skill: offered.known_as.clone(),
            run: ulid::Ulid::generate(),
            workspace: self.workspace.clone(),
            level: Some(runs::Level {
                origin: offered.effective.origin,
                local: offered.effective.local,
            }),
            text: crate::skillspan::PromptMarks::digest(&text),
            confirmed: self.harness.typed_start(),
        };
        Ok(Typing { text, held })
    }
}

/// **The stamp of the plane skills a chat has now** (#2770): a chat in the plane at `plane`, at
/// its root or in `workspace`, running as `persona`, on `harness`. The window takes one as a chat
/// starts and again when the project's skills change; where they differ, the chat's harness does
/// not take the change in by itself ([`Harness::skills_reload`]) and its tab offers a restart.
/// An empty stamp where the project's skills do not reach its chats ([`reach_chats_here`]).
pub fn stamp_now(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    harness: Harness,
) -> Stamp {
    if !reach_chats_here(plane) {
        return Stamp::default();
    }
    stamp_of(plane, workspace, persona, harness)
}

/// [`stamp_now`] whatever the flag says: for a caller that asked the flag once for many chats.
pub fn stamp_of(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    harness: Harness,
) -> Stamp {
    let at = PlaneSkills {
        plane,
        workspace,
        linked: None,
        repo: &[],
    };
    let set = ask(None, Some(at), persona, harness.skills(), None);
    Stamp::of(&set, harness.skills_reload())
}

/// One ask of purlis-skills, from a directory of built-in skills, the plane with the workspace
/// the chat works in (where the plane's skills are let in), the persona a chat runs as (whose
/// `skills:` list is read where the plane's skills are let in), the chat's route, and the
/// harness skills on this machine where they were looked for.
fn ask(
    built_in: Option<&Path>,
    plane: Option<PlaneSkills<'_>>,
    persona: Option<&str>,
    route: Route,
    harness: Option<&[OnMachine]>,
) -> SkillSet {
    let kept = |folder: &Path| match plane {
        Some(at) => crate::profiles::kept_local(at.plane, folder),
        None => Err("there is no plane to ask git about".to_owned()),
    };
    let off = plane
        .and_then(|at| Some(turned_off(at.plane, at.workspace?)))
        .unwrap_or_default();
    let names = plane
        .zip(persona)
        .map(|(at, persona)| names_of(at.plane, persona))
        .unwrap_or_default();
    // A workspace or persona folder a running sandboxed chat could have made (SK-9).
    let made = plane.map(|at| levels::made_since(at.plane));
    let set = purlis_skills::skill_set(
        plane.map(|at| at.plane),
        &Machine {
            built_in,
            kept_local: Some(&kept),
            harness,
            made_since: made
                .as_deref()
                .map(|made| made as &purlis_skills::MadeSince<'_>),
        },
        &Chat {
            workspace: plane.and_then(|at| at.workspace),
            persona,
            names: &names,
            turned_off: &off,
            harness: HarnessSkills { route },
        },
    );
    match plane.map(|at| at.repo) {
        Some(repo) if !repo.is_empty() => set.with_outside(route, repo.iter().cloned()),
        _ => set,
    }
}

/// **What the persona `persona`'s `skills:` list names** (SK-5, #2768), in the plane at `plane`:
/// each entry it writes, as a [`Reference`], in order. An entry that names nothing is dropped.
pub fn names_of(plane: &Path, persona: &str) -> Vec<Reference> {
    crate::personaverbs::declared_skills(plane, persona)
        .iter()
        .filter_map(|written| Reference::parse(written))
        .collect()
}

/// **The skills a chat starts without, as its tab's Notice says them** (SK-5, #2768; SK-8,
/// #2771, decision 41: never a silent drop): the ones its persona names that are not here, each
/// as the persona's `skills:` list writes it, and the ones that could not be loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NotHere {
    /// A harness skill the chat's harness does not have on this machine.
    pub missing: Vec<String>,
    /// A built-in skill purlis has none of: a typo, or a name from another version of purlis.
    pub no_built_in: Vec<String>,
    /// A plane skill the chat would have loaded that could not be: not a whole skill, or one
    /// of two folders at its level that name it.
    pub broken: Vec<Broken>,
}

/// A skill a chat would have loaded that could not be loaded, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Broken {
    /// Its name, or its folder's where it names none.
    pub name: String,
    /// Its level, and whether it is a local skill.
    pub origin: Origin,
    pub local: bool,
    pub why: BrokenWhy,
}

/// Why a skill a chat would have loaded could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokenWhy {
    /// The set leaves it out: not a whole skill, or a twin at its level.
    LeftOut(Reason),
    /// purlis hands it to no chat ([`purlis_skills::Refusal`]).
    Refused(purlis_skills::Refusal),
}

impl std::fmt::Display for BrokenWhy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LeftOut(reason) => write!(f, "{reason}"),
            Self::Refused(why) => write!(f, "purlis hands it to no chat: {why}"),
        }
    }
}

/// **The skills a chat as `persona` (or on none) starts without** (SK-5, #2768; SK-8), in the
/// plane at `plane`, at its root or in `workspace`, on `harness` with the chat's environment
/// `env` and the built-in skills in `built_in`. A broken plane skill is one only where the chat
/// would have loaded it: a project skill whose grant does not reach the chat's workspace is no
/// loss. `None` where it has every one, or purlis could not tell.
pub fn not_here(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    built_in: Option<&Path>,
    harness: crate::harness::Harness,
    env: &crate::harness_plugin::Env<'_>,
) -> Option<NotHere> {
    let here = on_machine(harness, env);
    let at = PlaneSkills {
        plane,
        workspace,
        linked: None,
        repo: &[],
    };
    let set = ask(built_in, Some(at), persona, harness.skills(), Some(&here));
    let reaches = |left: &LeftOut| {
        left.origin != Origin::Project
            || workspace.is_none_or(|ws| grant_of(&left.folder).is_ok_and(|g| g.reaches(ws)))
    };
    let mut not = NotHere::default();
    let honours = harness.skill_frontmatter();
    for effective in set.from_the_plane() {
        if let Err(why) = handout::of(&effective.skill, &honours) {
            not.broken.push(Broken {
                name: effective.skill.name.clone(),
                origin: effective.origin,
                local: effective.local,
                why: BrokenWhy::Refused(why),
            });
        }
    }
    for left in set.left_out {
        match left.reason {
            Reason::NotOnThisMachine => not.missing.push(left.name),
            Reason::NoSuchBuiltIn => not.no_built_in.push(left.name),
            Reason::NotASkill(_) | Reason::Twin { .. } if reaches(&left) => {
                not.broken.push(Broken {
                    name: left.name,
                    origin: left.origin,
                    local: left.local,
                    why: BrokenWhy::LeftOut(left.reason),
                });
            }
            _ => {}
        }
    }
    (not != NotHere::default()).then_some(not)
}

/// **The project skills the workspace `workspace` turned off** (#2767): `settings.skills.off`,
/// as the Workspace settings layer reads its `workspace.json`. Empty where it names none, or
/// holds something other than a list of names, which that layer's save refuses to write.
pub fn turned_off(plane: &Path, workspace: &str) -> Vec<String> {
    use crate::settings::workspace::{SKILLS, SKILLS_OFF};
    crate::settings::workspace::read(plane, workspace)
        .and_then(|table| {
            let names = table.get(SKILLS)?.get(SKILLS_OFF)?.as_array()?;
            names
                .iter()
                .map(|name| name.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .unwrap_or_default()
}

/// The folder of the project skill named `name` in the plane at `plane`: in its `skills/`, else
/// in its `skills.local/`, or `None` where neither holds one.
pub fn project_folder(plane: &Path, name: &str) -> Option<PathBuf> {
    [false, true]
        .into_iter()
        .filter_map(|local| level_dir(plane, &Whose::Project, local))
        .find_map(|dir| named(&dir, name))
}

/// **`purlis skill grant <name> <grant>`** (#2767): writes `grant` as where the project skill
/// `name` reaches, and says its folder. Refused, writing nothing, on a project purlis opens
/// read-only, for a name no project skill has, and for a workspace the plane does not have, so a
/// typo never grants a name nobody has.
pub fn grant(plane: &Path, name: &str, grant: &Grant) -> Result<PathBuf, String> {
    crate::settings::writable(plane)?;
    if let Grant::Only(names) = grant {
        let held = crate::workspaces::Plane::open(plane);
        for ws in names {
            if !held.workspace(ws).is_ok_and(|one| one.dir().is_dir()) {
                return Err(format!(
                    "this project has no workspace {ws:?}, so nothing was granted: \
                     `purlis workspace list` lists them"
                ));
            }
        }
    }
    let folder = project_folder(plane, name).ok_or_else(|| no_project_skill(name))?;
    set_grant(&folder, grant)?;
    Ok(folder)
}

/// **Turns the project skill `name` off (`on` false) or back on in `workspace`** (#2767): its
/// `settings.skills.off`, through the Workspace level's own save. A workspace can only take
/// away what a grant gave, so a skill whose grant does not reach `workspace` is refused either
/// way, saying the grant that would. `Ok(false)` where it already was so, and nothing is written.
pub fn switch(plane: &Path, workspace: &str, name: &str, on: bool) -> Result<bool, String> {
    let held = crate::workspaces::Plane::open(plane);
    if !held
        .workspace(workspace)
        .is_ok_and(|one| one.dir().is_dir())
    {
        return Err(format!(
            "this project has no workspace {workspace:?}: `purlis workspace list` lists them"
        ));
    }
    let folder = project_folder(plane, name).ok_or_else(|| no_project_skill(name))?;
    let grant = grant_of(&folder).unwrap_or(Grant::Root);
    if !grant.reaches(workspace) {
        return Err(not_granted(name, workspace, &grant, on));
    }
    let mut off = turned_off(plane, workspace);
    let was = off.iter().any(|one| one == name);
    if was != on {
        return Ok(false);
    }
    if on {
        off.retain(|one| one != name);
    } else {
        off.push(name.to_owned());
    }
    set_turned_off(plane, workspace, off)?;
    Ok(true)
}

/// **Why the project skill `name`, granted `grant`, cannot be turned on (`on`) or off in
/// `workspace`**, which the grant does not reach: the grant that would. What `purlis skill grant
/// --on` and the Skill studio's switch both say (#2767, #2770).
pub fn not_granted(name: &str, workspace: &str, grant: &Grant, on: bool) -> String {
    let what = if on {
        "cannot be turned on there: a workspace only turns off what the project granted it"
    } else {
        "has nothing to turn off there"
    };
    format!(
        "{name} is not granted to {workspace}, so it {what}. Its grant is {grant}: a grant to \
         {workspace}, or to all, would reach it"
    )
}

/// Writes `off` as the project skills `workspace` turned off, through the Workspace level's own
/// save: none removes the list.
pub(crate) fn set_turned_off(
    plane: &Path,
    workspace: &str,
    off: Vec<String>,
) -> Result<(), String> {
    let read = crate::settings::workspace::read_file(plane, workspace)?;
    let base = read.exists.then_some(read.text.as_str());
    let path = vec![
        crate::settings::Step::Key(crate::settings::workspace::SKILLS.to_owned()),
        crate::settings::Step::Key(crate::settings::workspace::SKILLS_OFF.to_owned()),
    ];
    let value = (!off.is_empty()).then_some(crate::settings::Value::List(off));
    crate::settings::workspace::save(
        plane,
        workspace,
        base,
        &[crate::settings::Edit { path, value }],
    )
    .map(|_| ())
    .map_err(|why| why.join(" "))
}

fn no_project_skill(name: &str) -> String {
    format!(
        "no project skill is named {name:?}: a project skill is a folder in skills/ or \
         skills.local/ at the project's root, and `purlis skill list` lists them"
    )
}

/// **Share** (#2767): moves the local skill `name` of the level `at` (the plane root's, a
/// workspace's or a persona's) into that level's shared `skills/`, so the plane's save commits
/// it. Refused on a project purlis opens read-only, whose committed files it does not write.
pub fn share_here(plane: &Path, at: &Whose, name: &str) -> Result<PathBuf, String> {
    crate::settings::writable(plane)?;
    share(plane, at, name)
}

/// The `requires` feature of a project whose skills the `project-skills` fix moved from
/// `.claude/skills` into `skills/` (#2784): a build that has it hands every chat the project's
/// skills by their route, and copies no `.claude/skills` into a clone
/// ([`crate::doctor::fix::project_skills`]).
pub const PROJECT_SKILLS_FEATURE: &str = "project-skills";

/// **Whether the project's skills reach the chats of the project at `plane`** (ADR 0099 as
/// amended 2026-10-22, the dispatcher's ruling on #2784): where `studio`, the Skill studio's
/// flag, is on, and on every machine once the project requires [`PROJECT_SKILLS_FEATURE`]. The
/// flag shows the studio; the manifest says where the project's skills live. A project whose
/// skills moved out of `.claude/skills` gets no copy of it in a clone, so a teammate with the
/// flag off would otherwise start every chat without them.
pub fn reach_chats(plane: &Path, studio: bool) -> bool {
    studio || crate::compat::requires(plane, PROJECT_SKILLS_FEATURE)
}

/// [`reach_chats`], with this machine's flag ([`studio_on_here`]).
pub fn reach_chats_here(plane: &Path) -> bool {
    reach_chats(plane, studio_on_here())
}

/// The flag the Skill studio's slices land behind until its finish line (#2762, SK-23), and is
/// then removed. It hides nothing yet.
pub const STUDIO_FLAG: &str = "skill-studio";

/// The variable that turns [`STUDIO_FLAG`] on (`1`) or off (`0`), over what this machine says.
/// Any other value is not read.
pub const STUDIO_ENV: &str = "PURLIS_SKILL_STUDIO";

/// Whether [`STUDIO_FLAG`] is on: what this machine's store says
/// ([`crate::machine::Flags::skill_studio`], off unless turned on, on every channel), and
/// [`STUDIO_ENV`] decides over it.
pub fn studio_on(flags: &crate::machine::Flags, env: &dyn Fn(&str) -> Option<String>) -> bool {
    match crate::envvar::lookup(STUDIO_ENV, env).as_deref() {
        Some("1") => true,
        Some("0") => false,
        _ => flags.skill_studio,
    }
}

/// **What the Skill studio and `purlis skill list` show** (SK-2): the skill set of a chat in the
/// plane at `plane`, at its root (`workspace` `None`) or in `workspace`, as `persona` or on none
/// (SK-5), with the built-in skills from `built_in`. Every skill with its level, and every one
/// left out with its reason. Read as a Claude Code chat's: the route is not what they show. A
/// harness skill a persona names is looked for in every harness the project's profiles run
/// ([`harnesses_used`]): there where any has it, unchecked where none has it and one cannot list
/// its plugin's skills, and left out only where none has it.
pub fn in_the_studio(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> SkillSet {
    let at = PlaneSkills {
        plane,
        workspace,
        linked: None,
        repo: &[],
    };
    let here = persona.map(|_| {
        let env = crate::harness_plugin::Env::of(&[]);
        harnesses_used(plane)
            .into_iter()
            .flat_map(|harness| on_machine(harness, &env))
            .collect::<Vec<_>>()
    });
    ask(built_in, Some(at), persona, Route::Plugin, here.as_deref())
}

/// **Every harness the project's profiles run**, in the registry's order, or Claude Code alone
/// for a project that offers none on this machine: what the studio checks a persona's harness
/// skills against.
pub fn harnesses_used(plane: &Path) -> Vec<crate::harness::Harness> {
    let kinds: Vec<String> = crate::personaprofile::offers_shown(plane)
        .into_iter()
        .map(|offer| offer.kind)
        .collect();
    harnesses_of(&kinds)
}

/// The harnesses profiles of `kinds` run, in the registry's order, or Claude Code alone for none.
fn harnesses_of(kinds: &[String]) -> Vec<crate::harness::Harness> {
    let used: Vec<crate::harness::Harness> = crate::harness::Harness::ALL
        .iter()
        .copied()
        .filter(|harness| {
            kinds
                .iter()
                .any(|kind| crate::harness::Harness::of_kind(kind) == Some(*harness))
        })
        .collect();
    if used.is_empty() {
        vec![crate::harness::Harness::ClaudeCode]
    } else {
        used
    }
}

/// **The route a chat's skills take, by its harness** as the chat names it in
/// [`crate::hookwire::HARNESS_ENV`] (ADR 0063 as amended 2026-10-20): a harness purlis ships
/// takes its own ([`Harness::skills`], a chat that names none being Claude Code's,
/// [`Harness::of_chat`]); a harness of `declarations` whose declaration (ADR 0073) says it takes
/// MCP servers ([`crate::harness_declaration::MCP_SERVERS`]) and which purlis has no adapter
/// for, so no skill format purlis can hand skills by, takes [`Route::Mcp`]. Any other has none.
pub fn route_of(
    harness: Option<&str>,
    declarations: &crate::harness_declaration::Declarations,
) -> Option<Route> {
    if let Some(shipped) = Harness::of_chat(harness) {
        return Some(shipped.skills());
    }
    declarations
        .get(harness?)
        .filter(|declared| {
            declared
                .capability(crate::harness_declaration::MCP_SERVERS)
                .holds()
        })
        .map(|_| Route::Mcp)
}

/// **Which copy of the skill a chat started as `skill`, a name as its harness gave it, is of**,
/// and the workspace the chat works in (SK-13): what a run's start carries, so a chat is shown
/// the runs of its own copies alone. The copy is the one the chat's set holds by that name
/// ([`purlis_skills::by_name`]), with the built-in skills purlis ships beside this binary; `None`
/// for a name the set does not hold, such as a harness's own skill.
pub fn whose(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    skill: &str,
) -> runs::Whose {
    // A skill started in purlis's own plugin (`purlis:secrets`) is the built-in copy, whether
    // or not this binary has the bundle beside it: a plane skill never takes a built-in name.
    let built_in_by_name = skill
        .split_once(':')
        .is_some_and(|(plugin, _)| crate::names::PLUGIN_NAME.recognises(plugin));
    let level = if built_in_by_name {
        Some(runs::Level {
            origin: Origin::BuiltIn,
            local: false,
        })
    } else {
        let built_in = built_in_here();
        let set = of_a_chat(
            built_in.as_deref(),
            plane,
            workspace,
            persona,
            Route::Plugin,
        );
        purlis_skills::by_name(&set, skill).map(|effective| runs::Level {
            origin: effective.origin,
            local: effective.local,
        })
    };
    runs::Whose {
        workspace: workspace.map(str::to_owned),
        level,
    }
}

/// **The skill set of one chat, as purlis's MCP server's skill tools work it out** (SK-13): a
/// chat in the plane at `plane`, at its root (`workspace` `None`) or in `workspace`, running as
/// `persona`, its skills taking `route`, with the built-in skills from `built_in`. The plane's
/// skills are in it: the tools are there only while the `skill-studio` flag lets the plane's
/// skills into a chat ([`studio_on`]). Worked out at each call, as the chat's place is.
pub fn of_a_chat(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    route: Route,
) -> SkillSet {
    let at = PlaneSkills {
        plane,
        workspace,
        linked: None,
        repo: &[],
    };
    // The harness's own installed skills are the harness's to hand over, never these tools'.
    ask(built_in, Some(at), persona, route, None)
}

/// **One harness in the Skill studio** (SK-6, #2769): its plugins, as the settings tab surveys
/// them, and the skills from outside the plane it finds itself, beside the plane's.
pub struct HarnessInTheStudio {
    pub harness: Harness,
    /// Its plugins, and what the project has each at ([`crate::harness_plugin::survey`]).
    pub group: crate::harness_plugin::Group,
    /// What a chat on it gets: the plane's skills ([`in_the_studio`]) with the ones from outside
    /// the plane ([`SkillSet::with_outside`]), each clash settled as this harness's route lets
    /// purlis settle it.
    pub set: SkillSet,
    /// The skills from outside the plane, with whose each is and how it switches.
    pub outside: Vec<OutsideSkill>,
}

/// **Every harness in the Skill studio** (SK-6, #2769), for a chat in the plane at `plane`, at
/// its root or in `workspace` (whose clones' own skills are listed), whose plane skills are
/// `set` ([`in_the_studio`]), and with `env` read for where each harness keeps its own: one per
/// harness purlis knows, in the registry's order. Read from the harnesses' own files, never
/// written.
pub fn harnesses_in_the_studio(
    set: &SkillSet,
    plane: &Path,
    workspace: Option<&str>,
    env: &crate::harness_plugin::Env<'_>,
) -> Vec<HarnessInTheStudio> {
    let choices = crate::harness_plugin::Choices::read_in(plane, workspace);
    let clones = workspace
        .and_then(|ws| crate::repos::clones(plane, ws).ok())
        .map(|clones| clones.repos)
        .unwrap_or_default();
    crate::harness_plugin::survey(&choices, env)
        .into_iter()
        .filter_map(|group| {
            // Every adapter the survey lists is a harness's ([`crate::harness_plugin::adapters`]).
            let harness = Harness::of_kind(group.adapter.harness())?;
            let mut outside = crate::harness_plugin::harness_skills(group.adapter, env, &choices);
            outside.extend(crate::harness_plugin::repo_skills(
                group.adapter,
                &choices,
                &clones,
            ));
            Some(HarnessInTheStudio {
                harness,
                set: set.clone().with_outside(harness.skills(), outside.clone()),
                group,
                outside,
            })
        })
        .collect()
}

/// **The repo skills the harness of a chat started in `cwd` reads** (SK-6, #2769): those of the
/// clone `cwd` is in, in `workspace` of the plane at `plane`, each turned off where the project's
/// choices say so. Nothing for a chat in no clone. The host's half: it reads the clone, and an
/// adapter is handed the answer ([`PlaneSkills::repo`]).
pub fn repo_skills_of_chat(
    harness: Harness,
    plane: &Path,
    workspace: &str,
    cwd: &Path,
) -> Vec<OutsideSkill> {
    let Ok(clones) = crate::repos::clones(plane, workspace) else {
        return Vec::new();
    };
    let here = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let mine: Vec<crate::repos::Repo> = clones
        .repos
        .into_iter()
        .filter(|repo| {
            let at = repo
                .path
                .canonicalize()
                .unwrap_or_else(|_| repo.path.clone());
            here.starts_with(at)
        })
        .collect();
    let choices = crate::harness_plugin::Choices::read_in(plane, Some(workspace));
    crate::harness_plugin::repo_skills(harness.adapter().plugins(), &choices, &mine)
}

/// One skill as the Skill studio and `purlis skill list` and `show` show it: every copy of a name,
/// the one a chat gets and each left out with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudioRow {
    /// Its name, or its folder's where it names none.
    pub name: String,
    pub origin: Origin,
    /// Whether it is a local skill, kept on this machine in its level's `skills.local/` (#2767).
    pub local: bool,
    /// Where a project skill reaches (`root`, `all`, or the workspaces' names), or `None` for a
    /// skill of another level. A grant that cannot be read reaches the root alone, and says so
    /// in the left-out line of a workspace's chat.
    pub grant: Option<String>,
    /// Why a chat does not get it, in one line, or `None` where it does.
    pub left_out: Option<String>,
    /// What it does: its `description`, empty for a copy left out.
    pub description: String,
    /// Its folder, relative to the plane where it is inside it; empty for a harness skill.
    pub folder: String,
    /// How a persona's `skills:` list names it (SK-5): `purlis:<name>` for a built-in skill,
    /// `<plugin>:<name>` for a plugin's, else its name.
    pub written: String,
    /// Whether the persona the set is a chat as names it in its `skills:` list.
    pub named: bool,
    /// A harness skill the persona names that purlis could not check this machine for: its
    /// plugin's skills cannot be listed.
    pub unchecked: bool,
    /// For a skill from outside the plane (SK-6, #2769): whose it is, and how its harness can
    /// leave it out. `None` for the plane's and the built-in skills.
    pub outside: Option<OutsideRow>,
    /// For a skill left out because a chat could have made its level's folder (SK-9): that
    /// folder, relative to the plane, which the person may accept ([`levels::accept`]).
    pub accept: Option<String>,
    /// Its compatibility badge on each harness that would load it (SK-8, #2771): every harness
    /// for a skill a chat gets from purlis, its own harness for one from outside the plane, and
    /// none for a skill left out.
    pub compat: Vec<HarnessCompat>,
    /// What the built-in lints say of a skill from outside the plane, as anywhere but the editor
    /// says it (SK-8): that it widens the chat, a skill it names there is none of, a program it
    /// calls that this machine lacks. Empty for the plane's and the built-in skills, which their
    /// editor lints.
    pub lint: Vec<String>,
    /// Its lifecycle state (SK-19, #2782): in use, deprecated or archived. Always in use for a
    /// skill that is not the plane's, and for one whose file beside it cannot be read.
    pub lifecycle: Lifecycle,
    /// Who looks after it, informational (SK-19). Empty for a skill that is not the plane's.
    pub owners: Vec<String>,
}

/// What a studio row says of a skill from outside the plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutsideRow {
    pub owner: Owner,
    pub switch: Switch,
}

/// **The studio's rows of the skills from outside the plane `harness` finds** (SK-6, #2769),
/// each by the name the harness knows it by, on or left out as its set says, with whose it is
/// and how it switches. In the order they were found.
pub fn outside_rows(harness: &HarnessInTheStudio, plane: &Path) -> Vec<StudioRow> {
    harness
        .outside
        .iter()
        .map(|found| {
            let folder = found.skill.path.parent().unwrap_or(&found.skill.path);
            let left_out = harness
                .set
                .left_out
                .iter()
                .find(|l| l.name == found.id && l.origin == found.origin && l.folder == folder)
                .map(|l| l.reason.to_string());
            let (compat, lint) = if left_out.is_none() {
                (
                    badges(&found.skill.path, found.origin, &[harness.harness]),
                    outside_lint(plane, found),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            StudioRow {
                name: found.id.clone(),
                origin: found.origin,
                local: false,
                grant: None,
                left_out,
                description: found.skill.description.clone(),
                folder: folder
                    .strip_prefix(plane)
                    .unwrap_or(folder)
                    .to_string_lossy()
                    .into_owned(),
                written: found.id.clone(),
                named: false,
                unchecked: false,
                outside: Some(OutsideRow {
                    owner: found.owner.clone(),
                    switch: found.switch.clone(),
                }),
                accept: None,
                compat,
                lint,
                lifecycle: Lifecycle::Active,
                owners: Vec::new(),
            }
        })
        .collect()
}

/// The most bytes of one of a skill's other files the lint reads: a script past this is no
/// script it reads for the programs it calls.
const MOST_LINTED: u64 = 256 * 1024;

/// **The live lint of the skill `name` at `level` of the plane at `plane`, as its `SKILL.md`
/// would say `text`** (SK-8, #2771): every finding of the built-in lints
/// ([`purlis_skills::BuiltInLints`]), checked against the plane's skills, the built-in ones and
/// this machine's programs, with the skill's other files read where it is already there. The one
/// lint the editor and the command line show; a save waits only on a finding that
/// [`LintLevel::Blocks`].
pub fn lint(plane: &Path, level: &verbs::Level, name: &str, text: &str) -> Vec<Finding> {
    let path = verbs::text_of(plane, level, name)
        .ok()
        .map(|(path, _)| path);
    linted(plane, path.as_deref(), name, text, level.origin())
}

/// **What the built-in lints say of a skill from outside the plane** (SK-8): a harness or repo
/// skill, read where it lies by the one reader that follows no link, each finding as anywhere
/// but the editor says it ([`Finding::shown`]). What its studio row shows.
fn outside_lint(plane: &Path, skill: &OutsideSkill) -> Vec<String> {
    let findings = match purlis_skills::read_skill_md(&skill.skill.path) {
        Ok(text) => linted(
            plane,
            Some(&skill.skill.path),
            &skill.skill.name,
            &text,
            skill.origin,
        ),
        Err(why) => vec![Finding {
            severity: purlis_skills::Severity::Warns,
            lint: purlis_skills::Lint::Refused(why),
        }],
    };
    findings.iter().filter_map(Finding::shown).collect()
}

/// The built-in lints of the skill `name` from `origin`, whose `SKILL.md` says `text` and, where
/// it is already there, lies at `skill_md`: checked against the plane's skills, the built-in
/// ones and this machine's programs, with its other files read.
fn linted(
    plane: &Path,
    skill_md: Option<&Path>,
    name: &str,
    text: &str,
    origin: Origin,
) -> Vec<Finding> {
    use purlis_skills::SkillLint;
    // Every level, each persona's among them (SK-32).
    let mut known: Vec<String> = verbs::Level::every(plane)
        .iter()
        .filter_map(|level| level.dir(plane))
        .flat_map(|dir| read(&dir))
        .map(|skill| skill.name)
        .collect();
    known.push(name.to_owned());
    let built_in = built_in_names_here();
    let others: Vec<(String, String)> = skill_md
        .map(|path| {
            let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
            purlis_skills::other_files(path)
                .into_iter()
                .filter(|file| {
                    std::fs::symlink_metadata(file)
                        .is_ok_and(|meta| meta.is_file() && meta.len() <= MOST_LINTED)
                })
                .filter_map(|file| {
                    let text = std::fs::read_to_string(&file).ok()?;
                    let shown = file.strip_prefix(&folder).unwrap_or(&file);
                    Some((shown.to_string_lossy().into_owned(), text))
                })
                .collect()
        })
        .unwrap_or_default();
    purlis_skills::BuiltInLints {
        namespace: LINKED_PLUGIN,
        plane: &known,
        built_in: &built_in,
        has: &crate::programs::on_path,
    }
    .findings(&purlis_skills::SkillFiles {
        name,
        text,
        others: &others,
        origin,
    })
}

/// **A skill's compatibility badge on one harness** (SK-8, #2771): what the studio draws for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessCompat {
    pub harness: Harness,
    pub compat: Compat,
}

/// The badges of the skill from `origin` whose `SKILL.md` is at `path` on each of `harnesses`
/// ([`purlis_skills::compat`]), read by the one reader that follows no link: one that cannot be
/// read is refused, or, from outside the plane, may widen the chat.
fn badges(path: &Path, origin: Origin, harnesses: &[Harness]) -> Vec<HarnessCompat> {
    let text = purlis_skills::read_skill_md(path);
    harnesses
        .iter()
        .map(|harness| HarnessCompat {
            harness: *harness,
            compat: match &text {
                Ok(text) => compat(text, origin, &harness.skill_frontmatter()),
                Err(why) => Compat::refused(why.clone(), origin),
            },
        })
        .collect()
}

/// **Each studio row of a harness skill a persona names, with its badge and lint on every
/// harness that finds it** (SK-8): the rows [`studio_rows`] draws from a persona's `skills:`
/// list carry no file of their own, so the skill each harness found by that name gives them one.
pub fn with_harness_badges(rows: &mut [StudioRow], harnesses: &[HarnessInTheStudio], plane: &Path) {
    let found: Vec<(Harness, &[OutsideSkill])> = harnesses
        .iter()
        .map(|one| (one.harness, one.outside.as_slice()))
        .collect();
    badge_named(rows, &found, plane);
}

/// [`with_harness_badges`], with what each harness found.
fn badge_named(rows: &mut [StudioRow], found: &[(Harness, &[OutsideSkill])], plane: &Path) {
    for row in rows
        .iter_mut()
        .filter(|row| row.origin == Origin::Harness && row.outside.is_none())
    {
        for (harness, outside) in found {
            if let Some(skill) = outside.iter().find(|skill| skill.id == row.name) {
                row.compat
                    .extend(badges(&skill.skill.path, skill.origin, &[*harness]));
                if row.lint.is_empty() {
                    row.lint = outside_lint(plane, skill);
                }
            }
        }
    }
}

/// **Everything the Skill studio lists for one chat** (SK-34, #3008): the rows of the plane's
/// and the built-in skills ([`studio_rows`], each badged on the harnesses that find a copy of
/// it), and each harness with the skills it finds itself ([`harnesses_in_the_studio`], whose
/// rows [`outside_rows`] draws). What the window and `purlis skill list` both list, so the two
/// never disagree (spec #2762 decision 26).
pub struct StudioList {
    pub rows: Vec<StudioRow>,
    pub harnesses: Vec<HarnessInTheStudio>,
}

/// **The studio's list for a chat in the plane at `plane`** (SK-34): at its root or in
/// `workspace`, as `persona` or on none, with the built-in skills in `built_in` and `env` read
/// for where each harness keeps its own. Read only.
pub fn studio_list(
    built_in: Option<&Path>,
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
    env: &crate::harness_plugin::Env<'_>,
) -> StudioList {
    let set = in_the_studio(built_in, plane, workspace, persona);
    let names = persona
        .map(|persona| names_of(plane, persona))
        .unwrap_or_default();
    let harnesses = harnesses_in_the_studio(&set, plane, workspace, env);
    let mut rows = studio_rows(&set, plane, &names);
    with_harness_badges(&mut rows, &harnesses, plane);
    StudioList { rows, harnesses }
}

/// What is said of a harness skill a persona names that purlis could not check this machine for
/// ([`StudioRow::unchecked`]): its plugin is installed, and its skills cannot be listed.
pub const UNCHECKED: &str =
    "purlis can't check whether this machine has it: its plugin's skills can't be listed";

/// **The studio's rows of `set`**, a set of a chat in the plane at `plane` as a persona whose
/// `skills:` list says `names` (empty for a chat on no persona): every skill it gets, every one
/// its harness loads for the persona, and every one left out, by name, each folder relative to
/// the plane. Of the copies of one name, the one the chat gets comes first, then each copy left
/// out from the widest level to the narrowest, as the set lists them (#2767). The one list the
/// window and the command line draw.
pub fn studio_rows(set: &SkillSet, plane: &Path, names: &[Reference]) -> Vec<StudioRow> {
    let relative = |path: &Path| {
        path.strip_prefix(plane)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    };
    let grant = |origin: Origin, folder: &Path| {
        (origin == Origin::Project).then(|| grant_of(folder).unwrap_or(Grant::Root).to_string())
    };
    let written = |origin: Origin, name: &str| match origin {
        Origin::BuiltIn => Reference::BuiltIn(name.to_owned()).to_string(),
        _ => name.to_owned(),
    };
    let named = |said: &str| names.iter().any(|one| one.to_string() == said);
    let mut rows: Vec<StudioRow> = set
        .effective
        .iter()
        .map(|e| {
            let folder = e.skill.path.parent().unwrap_or(&e.skill.path);
            let written = written(e.origin, &e.skill.name);
            StudioRow {
                name: e.skill.name.clone(),
                origin: e.origin,
                local: e.local,
                grant: grant(e.origin, folder),
                left_out: None,
                description: e.skill.description.clone(),
                folder: relative(folder),
                named: named(&written),
                written,
                unchecked: false,
                outside: None,
                accept: None,
                compat: badges(&e.skill.path, e.origin, &Harness::ALL),
                lint: Vec::new(),
                lifecycle: e
                    .deprecated
                    .clone()
                    .map_or(Lifecycle::Active, Lifecycle::Deprecated),
                owners: owners(e.origin, folder),
            }
        })
        .chain(set.from_the_harness.iter().map(|h| {
            let written = h.reference.to_string();
            StudioRow {
                name: written.clone(),
                origin: Origin::Harness,
                local: false,
                grant: None,
                left_out: None,
                description: String::new(),
                folder: String::new(),
                named: true,
                written,
                unchecked: !h.checked,
                outside: None,
                accept: None,
                compat: Vec::new(),
                lint: Vec::new(),
                lifecycle: Lifecycle::Active,
                owners: Vec::new(),
            }
        }))
        .chain(set.left_out.iter().map(|l| {
            // A left-out copy a persona names is the reference itself (`purlis:<name>` for a
            // built-in one there is none of); every other copy is named by its own name.
            let written = if matches!(l.reason, Reason::NoSuchBuiltIn) {
                l.name.clone()
            } else {
                written(l.origin, &l.name)
            };
            StudioRow {
                name: l.name.clone(),
                origin: l.origin,
                local: l.local,
                grant: grant(l.origin, &l.folder),
                left_out: Some(l.reason.to_string()),
                description: String::new(),
                folder: relative(&l.folder),
                named: named(&written),
                written,
                unchecked: false,
                outside: None,
                accept: matches!(l.reason, Reason::MadeWhileAChatRan(_))
                    .then(|| levels::to_accept(plane, &l.folder))
                    .flatten(),
                compat: Vec::new(),
                lint: Vec::new(),
                lifecycle: if matches!(l.reason, Reason::Archived) {
                    Lifecycle::Archived
                } else if l.origin.of_the_plane() {
                    about_of(&l.folder)
                        .map(|about| about.lifecycle)
                        .unwrap_or_default()
                } else {
                    Lifecycle::Active
                },
                owners: owners(l.origin, &l.folder),
            }
        }))
        .collect();
    // A deprecated skill is shown after the rest (decision 62), with every copy of its name.
    let deprecated: std::collections::HashSet<String> = rows
        .iter()
        .filter(|row| row.left_out.is_none() && matches!(row.lifecycle, Lifecycle::Deprecated(_)))
        .map(|row| row.name.clone())
        .collect();
    // Stable: the set already lists each name's left-out copies widest first.
    rows.sort_by(|a, b| {
        deprecated
            .contains(&a.name)
            .cmp(&deprecated.contains(&b.name))
            .then(a.name.cmp(&b.name))
            .then(a.left_out.is_some().cmp(&b.left_out.is_some()))
    });
    rows
}

/// The owners the plane's skill of `origin` in `folder` names, or none for a skill of another
/// level or a file beside it that cannot be read.
fn owners(origin: Origin, folder: &Path) -> Vec<String> {
    if origin.of_the_plane() {
        about_of(folder)
            .map(|about| about.owners)
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// One skill run as the studio lists it: the run, and the skill runs it nests in and holds
/// (SK-19): a skill a chat started while another's run was open is a child span of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StudioRun {
    #[serde(flatten)]
    pub run: crate::skilluse::Run,
    /// The skill whose run this one started inside, where it is a child.
    pub inside: Option<String>,
    /// The skills whose runs started inside this one, each once, in the order they started.
    pub ran: Vec<String>,
}

/// **The studio's runs** (SK-3): every run the plane at `plane`'s skill log holds, newest first,
/// of skill `skill` where one is named (by its name, with or without the namespace a harness
/// used it by), at most `limit`, each with the run it nests in and the runs it holds (SK-19). The
/// one list a skill's Runs panel and `purlis skill runs` draw.
pub fn studio_runs(plane: &Path, skill: Option<&str>, limit: usize) -> Vec<StudioRun> {
    let runs = crate::skilluse::runs(plane);
    let mut skill_of: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    let mut children: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for run in &runs {
        if let Some(id) = &run.id {
            skill_of.insert(id, &run.skill);
        }
    }
    // Oldest first, so a parent's children are in the order they started.
    for run in runs.iter().rev() {
        if let Some(parent) = run.end.as_ref().and_then(|end| end.parent.as_deref()) {
            let held = children.entry(parent).or_default();
            if !held.contains(&run.skill.as_str()) {
                held.push(&run.skill);
            }
        }
    }
    runs.iter()
        .filter(|run| skill.is_none_or(|name| run.is_of(name)))
        .take(limit)
        .map(|run| StudioRun {
            inside: run
                .end
                .as_ref()
                .and_then(|end| end.parent.as_deref())
                .and_then(|parent| skill_of.get(parent))
                .map(|skill| (*skill).to_owned()),
            ran: run
                .id
                .as_deref()
                .and_then(|id| children.get(id))
                .map(|held| held.iter().map(|skill| (*skill).to_owned()).collect())
                .unwrap_or_default(),
            run: run.clone(),
        })
        .collect()
}

/// The built-in skills' directory of the plugin installed beside this binary, where there is one
/// ([`crate::plugin_install::bundle_beside`]): what the Skill studio and `purlis skill` list,
/// found the one way in the window and on the command line.
pub fn built_in_here() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    crate::plugin_install::bundle_beside(&exe).and_then(|plugin| in_bundle(&plugin))
}

/// **Turns the Skill studio's flag on or off on this machine** (`flags.skill-studio` in
/// `machine.json`), as Settings › You › Previews does. [`STUDIO_ENV`] still decides over it
/// where it is set.
pub fn set_studio_on(config_root: &Path, on: bool) -> Result<(), String> {
    crate::machine::update(config_root, |store| store.flags.skill_studio = on)
        .map(|_| ())
        .map_err(|why| format!("purlis could not keep it, so nothing was changed: {why}"))
}

/// [`studio_on`] for this process: this machine's store, and this process's environment. Asked
/// at every chat's start, so it never holds the fence: a fenced test build reads a store outside
/// its fence as holding no flag, as [`crate::profilestore::listed_here`] does.
pub fn studio_on_here() -> bool {
    let flags = crate::machine::config_root_unheld()
        .filter(|config| {
            !crate::fence::FENCED || crate::fence::inside(config, &crate::fence::fence())
        })
        .map(|config| crate::machine::read(&config).store.flags)
        .unwrap_or_default();
    studio_on(&flags, &|name| std::env::var(name).ok())
}

#[cfg(test)]
mod tests;
