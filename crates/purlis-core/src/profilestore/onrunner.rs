// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A desktop's profiles on its runners** (#2049, spec #2041's *Profiles on runners*).
//!
//! A chat on a pool's runner is composed there by its names (#2171), so the runner needs a
//! profile of the name the chat runs on. The desktop asks its runner to keep one ([`Kept`],
//! the session protocol's `keep_profile`), and the runner makes it in its own store: a folder of
//! its own, in its own data home, which on a pool is the runner's home volume.
//!
//! **No sign-in of the desktop's profile folder goes to the runner.** What is kept is the
//! profile's name, its harness, whether it is for sandboxed chats only, the harness version it is
//! pinned to, and, at a chat's start or move there, its settings files (#3448): exactly the ones
//! a made profile imports ([`super::ALLOWED`]), read as an import reads them, never a
//! sign-in, a file named like a credential or a vault reference. A subscription is signed in
//! on the runner through its harness's own flow, run there in the runner's folder
//! ([`super::sign_in`]), once per device, so no sign-in is ever copied between devices and a
//! single-use refresh token never has two holders (ADR 0095 §3).
//!
//! **Only a profile that signs in by a subscription goes to a runner in this version**
//! ([`Kept::of`]). A key or a cloud's credentials are handed to no chat yet, here or there
//! (#2151); a local model is a model on the desktop, which a runner does not reach.
//!
//! **The desktop's adopted folder is the runner's own default folder there.** The desktop's own
//! harness folder, adopted in place as `claude`, is kept on the runner as that harness's default
//! folder in the runner's home, adopted in place there: a chat on `claude` runs as the runner's
//! built-in did, so a runner signed in before stays signed in and its conversations resume.
//! It carries the desktop's pin, which follows the desktop's harness as an adopted folder's pin
//! does, so the runner installs the version the desktop runs (#3446).
//!
//! **Every record kept for a desktop says so** (`kept_for`, the desktop's device id). Only such
//! a record of a folder adopted in place is signed in by purlis ([`super::sign_in::profile_of`]);
//! a folder the person adopted on their own machine never is.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use super::sign_in::{self, Look, Standing};
use super::{Homes, MachineProfile};

/// **A desktop's profile, as a runner keeps it**: what the desktop sends, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The name the desktop's chats, personas and dispatch lists use.
    pub name: String,
    /// The harness, as a profile's kind names it: `claude`, `codex` or `opencode`.
    pub harness: String,
    /// Only a sandboxed chat starts on it (#2037).
    pub sandbox_only: bool,
    /// The harness version the desktop's profile is pinned to (#2038), where it is pinned.
    pub pinned: Option<String>,
    /// The desktop's settings folder of it, whose settings files [`to_send`] reads and sends
    /// (#3448), where they are sent: at a chat's start or move there, never at a look.
    pub settings_in: Option<PathBuf>,
}

/// **The most bytes of settings files sent with a profile** (#3448): half of what one frame of
/// the link carries, since their bytes cross as base64.
pub const MOST_SETTINGS_BYTES: usize = purlis_session_protocol::link::MOST_CONTROL_FRAME_BYTES / 2;

/// **The most settings files sent with a profile** (#3448).
pub const MOST_SETTINGS_FILES: usize = 2048;

/// What frames one settings file on the wire beside its path and bytes, counted against
/// [`MOST_SETTINGS_BYTES`]: its JSON keys, quotes and commas.
const FRAMING_BYTES: usize = 32;

impl Kept {
    /// **The desktop's profile `record`, as its runners keep it**, or why it is not kept on a
    /// runner: one signed in by a key, a cloud's credentials or a local model, and one of a
    /// harness purlis does not ship.
    pub fn of(record: &MachineProfile) -> Result<Kept, String> {
        use crate::harness_declaration::SignInKind;
        let name = crate::shown::short(&record.name);
        match record
            .sign_in
            .as_ref()
            .map(crate::profilesignin::SignIn::kind)
        {
            None | Some(Some(SignInKind::Subscription)) => {}
            Some(Some(SignInKind::ApiKey | SignInKind::Cloud)) => {
                return Err(format!(
                    "the profile '{name}' signs in with credentials from your vault, which \
                     purlis hands to no chat on a pool's runner in this version"
                ));
            }
            Some(Some(SignInKind::Local)) => {
                return Err(format!(
                    "the profile '{name}' runs a model on this machine, which a pool's runner \
                     does not reach"
                ));
            }
            Some(None) => {
                return Err(format!(
                    "the profile '{name}' signs in a way this purlis does not know"
                ));
            }
        }
        if crate::harness_declaration::builtin(&record.harness).is_none() {
            return Err(format!(
                "the profile '{name}' runs no harness purlis ships, so no runner keeps it"
            ));
        }
        Ok(Kept {
            name: record.name.clone(),
            harness: record.harness.clone(),
            sandbox_only: record.sandbox_only,
            // An adopted folder's pin follows its harness here, and the runner installs the
            // version it follows to (#3446).
            pinned: record.pinned.as_ref().map(|p| p.version.clone()),
            settings_in: None,
        })
    }

    /// This profile, its settings sent with it from the desktop's `record` (#3448): the settings
    /// folder of a made profile or an adopted one. A profile for sandboxed chats only has
    /// purlis's settings, so none is sent for it.
    #[must_use]
    pub fn with_settings_of(mut self, record: &MachineProfile) -> Self {
        self.settings_in = (!record.sandbox_only).then(|| record.folders.config.clone());
        self
    }
}

/// **What the desktop sends its runner to keep `kept`** (`keep_profile`): its name, harness,
/// sandbox-only mark and pin, and, where [`Kept::settings_in`] names its settings folder, the
/// settings files a made profile imports, read as an import reads them (#3448): never a sign-in
/// or a file named like a credential, and at most [`MOST_SETTINGS_BYTES`], or why not.
///
/// **Each settings file is sent with no sign-in in it** ([`super::sent::without_credentials`],
/// D-3448-8): every environment block, and every setting that names or runs a credential source,
/// is removed, and a settings file that does not parse is refused in words. What is sent, each
/// file's path and bytes with what frames them, is held to [`MOST_SETTINGS_BYTES`] and
/// [`MOST_SETTINGS_FILES`].
pub fn to_send(kept: &Kept) -> Result<purlis_session_protocol::session::ProfileToKeep, String> {
    let settings = match &kept.settings_in {
        Some(folder) => {
            let read = super::made::settings_read(&kept.harness, folder, MOST_SETTINGS_BYTES)?;
            if read.len() > MOST_SETTINGS_FILES {
                return Err(format!(
                    "its settings in {} are more than {MOST_SETTINGS_FILES} files, more than a \
                     link to a runner carries at once",
                    folder.display()
                ));
            }
            let mut sent = Vec::with_capacity(read.len());
            let mut size = 0;
            for (path, bytes) in read {
                let bytes = super::sent::without_credentials(&kept.harness, &path, &bytes)?;
                size += bytes.len() + path.len() + FRAMING_BYTES;
                sent.push(purlis_session_protocol::session::SettingsFile { path, bytes });
            }
            if size > MOST_SETTINGS_BYTES {
                return Err(format!(
                    "its settings in {} are larger than a link to a runner carries ({} KiB)",
                    folder.display(),
                    MOST_SETTINGS_BYTES / 1024
                ));
            }
            Some(sent)
        }
        None => None,
    };
    Ok(purlis_session_protocol::session::ProfileToKeep {
        settings,
        ..purlis_session_protocol::session::ProfileToKeep::from(kept.clone())
    })
}

/// **Keeps the desktop's profile `kept` for the desktop `desktop` in this runner's store under
/// `homes`**, at `now`, with `home` the runner's home: one named like a built-in (the desktop's
/// adopted folder) as that harness's default folder under `home`, adopted in place; any other
/// made, with nothing imported and no sign-in. One it has already is marked kept for `desktop`
/// and brought to `kept`'s pin (none clears it) and sandbox-only mark. A profile of that name of
/// another harness, or one of a kind the record is not (made for adopted, adopted for made), is
/// refused and left as it is. The record as kept.
pub fn keep(
    homes: &Homes,
    home: &Path,
    kept: &Kept,
    desktop: &str,
    now: u64,
) -> Result<MachineProfile, String> {
    let name = crate::shown::short(&kept.name);
    let builtin = crate::profiles::KINDS.iter().any(|k| k.word == kept.name);
    if builtin && kept.name != kept.harness {
        return Err(format!(
            "the profile '{name}' is named like a harness it does not run, so no runner keeps it"
        ));
    }
    if !builtin && !crate::profiles::name_ok(&kept.name) {
        return Err(format!(
            "'{name}' cannot be a profile's name, so no runner keeps it"
        ));
    }
    let pinned = match &kept.pinned {
        Some(version) => Some(
            super::pin::Version::parse(version)
                .map_err(|_| {
                    format!("the profile '{name}' is pinned to a version that is not one")
                })?
                .to_string(),
        ),
        None => None,
    };
    let defaults = if builtin {
        let (config, data, _) = super::defaults(&kept.harness, home).ok_or_else(|| {
            format!("the profile '{name}' runs no harness purlis ships, so no runner keeps it")
        })?;
        Some(super::Folders { config, data })
    } else {
        None
    };
    let brought = |record: &mut MachineProfile| {
        record.kept_for = Some(desktop.to_owned());
        record.sandbox_only = kept.sandbox_only && !builtin;
        if record.pinned.as_ref().map(|p| &p.version) != pinned.as_ref() {
            record.pinned = pinned
                .clone()
                .map(|version| super::pin::Pinned { version, at: now });
        }
    };
    let title = crate::harness_declaration::builtin(&kept.harness)
        .map_or_else(|| kept.harness.clone(), |d| d.title.clone());
    let folder = homes.folder_of(&kept.name);
    let mut made_folder = false;
    // One change of the store, found or made under its lock: two keeps at once make one record,
    // and the second finds the first's.
    let answer = super::update(homes.config_root(), |store| {
        if let Some(record) = store.profiles.iter_mut().find(|p| p.name == kept.name) {
            if !is_ours(record, kept, desktop, defaults.as_ref()) {
                return Err(format!(
                    "this runner has a profile named '{name}' of its own, so it keeps no other \
                     of that name"
                ));
            }
            brought(record);
            return Ok(record.clone());
        }
        let mut record = match &defaults {
            Some(folders) => MachineProfile {
                name: kept.name.clone(),
                harness: kept.harness.clone(),
                title: format!("{title} (this runner)"),
                folders: folders.clone(),
                adopted: Some(super::Adopted {
                    at: now,
                    variables: Vec::new(),
                }),
                imported: None,
                sign_in: None,
                pinned: None,
                sandbox_only: false,
                kept_for: None,
                rest: Default::default(),
            },
            None => {
                let making = super::made::Making {
                    machine: None,
                    sandbox_only: kept.sandbox_only,
                };
                let made = super::made::make_in(
                    store,
                    homes,
                    &kept.name,
                    &kept.harness,
                    &making,
                    now,
                    &mut made_folder,
                )?;
                store.profiles.retain(|p| p.name != kept.name);
                made.record
            }
        };
        brought(&mut record);
        store.profiles.push(record.clone());
        Ok(record)
    });
    match answer {
        Ok(Ok(record)) => Ok(record),
        Ok(Err(why)) | Err(why) => {
            if made_folder {
                let _ = std::fs::remove_dir_all(&folder);
            }
            Err(why)
        }
    }
}

/// **Whether `record`, this runner's of `kept`'s name, is one it keeps for `desktop`**, so that
/// keeping `kept` may bring it up to date: of the same harness, and, for a profile purlis made,
/// kept for that desktop; for a folder adopted in place (`defaults` its harness's default
/// folders here), adopted at exactly those folders with no variable, and kept for no other
/// desktop. A runner's own profile, or another desktop's, is never taken over.
fn is_ours(
    record: &MachineProfile,
    kept: &Kept,
    desktop: &str,
    defaults: Option<&super::Folders>,
) -> bool {
    if record.harness != kept.harness {
        return false;
    }
    match (&record.adopted, defaults) {
        (None, None) => record.kept_for.as_deref() == Some(desktop),
        (Some(adopted), Some(defaults)) => {
            record
                .kept_for
                .as_deref()
                .is_none_or(|kept_for| kept_for == desktop)
                && adopted.variables.is_empty()
                && record.folders == *defaults
        }
        _ => false,
    }
}

/// **Whether the profile `record`, kept on this runner, is signed in here**, as its harness says
/// through `look` now: never a remembered answer, since this is what the desktop asks after a
/// sign-in.
pub fn standing(record: &MachineProfile, look: &dyn Look) -> Standing {
    match sign_in::profile_of(record) {
        Some(profile) => {
            sign_in::forget(&profile);
            sign_in::standing_remembered(&profile, look)
        }
        None => Standing::CannotTell,
    }
}

impl From<&purlis_session_protocol::session::ProfileToKeep> for Kept {
    fn from(sent: &purlis_session_protocol::session::ProfileToKeep) -> Self {
        Kept {
            name: sent.name.clone(),
            harness: sent.harness.clone(),
            sandbox_only: sent.sandbox_only,
            pinned: sent.pinned.clone(),
            settings_in: None,
        }
    }
}

impl From<Kept> for purlis_session_protocol::session::ProfileToKeep {
    fn from(kept: Kept) -> Self {
        purlis_session_protocol::session::ProfileToKeep {
            name: kept.name,
            harness: kept.harness,
            sandbox_only: kept.sandbox_only,
            pinned: kept.pinned,
            settings: None,
        }
    }
}

/// The standing a runner's `profile_kept` names.
pub fn standing_on_the_wire(said: purlis_session_protocol::session::ProfileStanding) -> Standing {
    use purlis_session_protocol::session::ProfileStanding as Said;
    match said {
        Said::SignedIn => Standing::SignedIn,
        Said::SignedOut => Standing::SignedOut,
        Said::CannotTell => Standing::CannotTell,
    }
}

/// `standing` as `profile_kept` names it.
pub fn standing_for_the_wire(
    standing: Standing,
) -> purlis_session_protocol::session::ProfileStanding {
    use purlis_session_protocol::session::ProfileStanding as Said;
    match standing {
        Standing::SignedIn => Said::SignedIn,
        Standing::SignedOut => Said::SignedOut,
        Standing::CannotTell => Said::CannotTell,
    }
}

/// **What a runner's host answers its desktop's `keep_profile` with** (1.43): the profile
/// `sent` kept for the desktop `desktop` in this runner's store under `homes`, with `home` the
/// runner's home, at `now`, and whether its harness says, through `look`, that it is signed in
/// here. The settings `sent` carries (1.50, #3448) are kept in place of the ones kept before;
/// none sent leaves them as they are.
pub fn kept_for_the_desktop(
    homes: &Homes,
    home: &Path,
    desktop: &str,
    sent: &purlis_session_protocol::session::ProfileToKeep,
    look: &dyn Look,
    now: u64,
) -> Result<purlis_session_protocol::session::ProfileStanding, String> {
    let record = keep(homes, home, &Kept::from(sent), desktop, now)?;
    if let Some(settings) = &sent.settings {
        let files: Vec<(String, Vec<u8>)> = settings
            .iter()
            .map(|file| (file.path.clone(), file.bytes.clone()))
            .collect();
        super::made::settings_kept(homes, &record, &files)?;
    }
    Ok(standing_for_the_wire(standing(&record, look)))
}

/// **Whether a chat on the desktop's profile `name` may start on pool `pool`'s runner**, for the
/// project at `root`, with this desktop's profiles in the store under `config_root`, the runner
/// asked to keep it through `keep`, which answers `None` where it sent nothing (a runner not
/// shown to be the person's own, or one of a purlis before 1.43). A profile this desktop keeps
/// no record of (a built-in, or one a clone declares), and one `keep` sent nothing of, is the
/// runner's own to say. One that goes to no runner is refused in words; one its harness says is
/// signed out on the runner is refused with a wait the window shows as the Ask *Profile
/// <profile> on pool <pool>* ([`waiting_on_pools`]). Signed in, or where its harness cannot
/// tell, the chat starts, and nobody is asked anything.
pub fn ready_on(
    config_root: &Path,
    root: &Path,
    pool: &str,
    name: &str,
    keep: &dyn Fn(&Kept) -> Result<Option<Standing>, String>,
) -> Result<(), String> {
    ready_on_for(config_root, root, pool, name, keep, For::Start)
}

/// [`ready_on`], for a chat that moves to pool `pool`'s runner (#2048): the same keep and the same
/// wait, said of the move, which the person asks for again once the profile is signed in there.
pub fn ready_to_move_on(
    config_root: &Path,
    root: &Path,
    pool: &str,
    name: &str,
    keep: &dyn Fn(&Kept) -> Result<Option<Standing>, String>,
) -> Result<(), String> {
    ready_on_for(config_root, root, pool, name, keep, For::Move)
}

/// What a profile is readied on a runner for: a chat's start there, or its move there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum For {
    Start,
    Move,
}

fn ready_on_for(
    config_root: &Path,
    root: &Path,
    pool: &str,
    name: &str,
    keep: &dyn Fn(&Kept) -> Result<Option<Standing>, String>,
    purpose: For,
) -> Result<(), String> {
    let (nothing, again) = match purpose {
        For::Start => ("Nothing was started", "then start the chat again"),
        For::Move => ("The chat did not move", "then ask for the move again"),
    };
    let Some(record) = super::list(config_root)?
        .into_iter()
        .find(|p| p.name == name)
    else {
        return Ok(());
    };
    let shown = crate::shown::short(name);
    // A start or a move sends the profile's settings with it, as they are now (#3448).
    let kept = Kept::of(&record)
        .map_err(|why| format!("{why}. {nothing}."))?
        .with_settings_of(&record);
    match keep(&kept).map_err(|why| {
        format!("pool {pool}'s runner did not keep the profile '{shown}': {why}. {nothing}.")
    })? {
        Some(Standing::SignedOut) => {
            waits_on_pool(root, pool, &record);
            Err(format!(
                "the profile '{shown}' is not signed in on pool {pool}. {nothing}. It waits in \
                 the Inbox as Profile {shown} on pool {pool}, whose Sign in on pool {pool} runs \
                 its harness's own sign-in on the runner; {again}."
            ))
        }
        Some(Standing::SignedIn | Standing::CannotTell) | None => Ok(()),
    }
}

/// **A profile a start on a pool was refused on**, waiting to be signed in on that pool's
/// runner: one per project, pool and profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolWait {
    /// The project the start was refused in.
    pub root: PathBuf,
    pub pool: String,
    /// The profile's name.
    pub profile: String,
    /// Its harness, as a profile's kind names it.
    pub kind: String,
    /// Its harness as people call it: `Claude Code`.
    pub harness: String,
    /// When the first start was refused, in seconds since 1970.
    pub since: u64,
}

static POOL_WAITS: Mutex<Vec<PoolWait>> = Mutex::new(Vec::new());

/// Records that a start in the project at `root` on pool `pool` was refused because `record` is
/// not signed in there.
fn waits_on_pool(root: &Path, pool: &str, record: &MachineProfile) {
    let mut all = POOL_WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    if all
        .iter()
        .any(|w| w.root == root && w.pool == pool && w.profile == record.name)
    {
        return;
    }
    all.push(PoolWait {
        root: root.to_path_buf(),
        pool: pool.to_owned(),
        profile: record.name.clone(),
        kind: record.harness.clone(),
        harness: crate::harness_declaration::builtin(&record.harness)
            .map_or_else(|| record.harness.clone(), |d| d.title.clone()),
        since: crate::halt::now(),
    });
}

/// The profiles the project at `root` waits on to be signed in on a pool, oldest first.
pub fn waiting_on_pools(root: &Path) -> Vec<PoolWait> {
    POOL_WAITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter(|w| w.root == root)
        .cloned()
        .collect()
}

/// The profile `name` is signed in on pool `pool` now, or the person put its wait away in the
/// project at `root` (`None`: in every project): its waits go.
pub fn let_go(root: Option<&Path>, pool: &str, name: &str) {
    POOL_WAITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|w| !(w.pool == pool && w.profile == name && root.is_none_or(|r| w.root == r)));
}

#[cfg(test)]
mod tests;
