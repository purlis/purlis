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

use std::collections::BTreeMap;

use purlis_session_protocol::session as wire;

use super::made::{Measured, whole_of};
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

/// **The most bytes of settings files sent in one `keep_profile`**, to a runner before 1.56
/// (#3448): half of what one frame of the link carries, since their bytes cross as base64. A
/// runner of 1.56 or later takes them in batches ([`in_batches`], #4081).
pub const MOST_SETTINGS_BYTES: usize = purlis_session_protocol::link::MOST_CONTROL_FRAME_BYTES / 2;

/// **The most settings files sent in one `keep_profile`**, to a runner before 1.56 (#3448).
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

/// **What the desktop sends a runner before 1.56 to keep `kept`** (`keep_profile`): its name,
/// harness, sandbox-only mark and pin, and, where [`Kept::settings_in`] names its settings
/// folder, the settings files a made profile imports, read as an import reads them (#3448):
/// never a sign-in or a file named like a credential, and at most [`MOST_SETTINGS_BYTES`], or why
/// not.
///
/// **Each settings file is sent with no sign-in in it** ([`super::sent::without_credentials`],
/// D-3448-8): every environment block, and every setting that names or runs a credential source,
/// is removed, and a settings file that does not parse is refused in words. What is sent, each
/// file's path and bytes with what frames them, is held to [`MOST_SETTINGS_BYTES`] and
/// [`MOST_SETTINGS_FILES`].
pub fn to_send(kept: &Kept) -> Result<purlis_session_protocol::session::ProfileToKeep, String> {
    let settings = match &kept.settings_in {
        Some(folder) => {
            let read = super::made::settings_read(
                &kept.harness,
                folder,
                MOST_SETTINGS_BYTES,
                usize::MAX,
                &|_| true,
            )?;
            let too_large = || {
                format!(
                    "its settings in {} are larger than a link to a runner carries ({} KiB)",
                    folder.display(),
                    MOST_SETTINGS_BYTES / 1024
                )
            };
            if !read.left_behind.is_empty() {
                return Err(too_large());
            }
            let read = read.files;
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
                return Err(too_large());
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

/// **What the desktop sends a runner of 1.56 or later to keep a profile** (#4081).
#[derive(Debug)]
pub struct ToKeep {
    /// The keep itself, which names how many files were staged for it.
    pub profile: purlis_session_protocol::session::ProfileToKeep,
    /// Its settings, each batch within a control frame, sent first.
    pub batches: Vec<purlis_session_protocol::session::SettingsBatch>,
    /// What stayed behind, over the limits, so the person is told.
    pub left_behind: Vec<LeftBehind>,
}

/// **A skill, an agent or a settings file that stayed on the desktop** (#4081): a whole skill's
/// folder, since half a skill is no skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftBehind {
    /// Its path under the settings folder: `skills/review`, `CLAUDE.md`.
    pub path: String,
    pub bytes: u64,
}

/// **Which of the settings folder's `wholes` go, and what size of each stays** (#4081): the
/// harness's top-level files first, then the smallest first, so a large skill keeps out as little
/// as it can, within `most_bytes` and `most_files`.
fn wholes_that_fit(
    wholes: Measured,
    most_bytes: u64,
    most_files: usize,
) -> (std::collections::BTreeSet<String>, BTreeMap<String, u64>) {
    let mut smallest: Vec<_> = wholes.into_iter().collect();
    smallest.sort_by_key(|(path, (bytes, _))| (path.contains('/'), *bytes));
    let (mut going, mut behind) = (std::collections::BTreeSet::new(), BTreeMap::new());
    let (mut size, mut count) = (0, 0);
    for (path, (bytes, files)) in smallest {
        if size + bytes > most_bytes || count + files > most_files {
            behind.insert(path, bytes);
        } else {
            size += bytes;
            count += files;
            going.insert(path);
        }
    }
    (going, behind)
}

/// What the read of a profile leaves of the overall limit, as a runner counts it: a settings file
/// rewritten without its credentials can come out longer than it went in.
const REWRITING_ROOM: usize = 8 * 1024 * 1024;

/// **`kept`, as a runner of 1.56 or later is sent it** (`stage_profile_settings`, #4081): what
/// [`to_send`] reads and removes, whatever its size, in batches, under one overall limit
/// ([`MOST_PROFILE_SETTINGS_BYTES`](purlis_session_protocol::session::MOST_PROFILE_SETTINGS_BYTES),
/// [`MOST_PROFILE_SETTINGS_FILES`](purlis_session_protocol::session::MOST_PROFILE_SETTINGS_FILES)).
/// What is over it stays behind, by whole skills ([`LeftBehind`]), and the rest goes: no chat is
/// refused for it. A harness's own settings file is the exception: it says what a chat may do, so
/// one that cannot go, over [`MOST_SETTINGS_BYTES`], or one that does not parse, is refused in
/// words, and nothing is sent.
pub fn in_batches(kept: &Kept) -> Result<ToKeep, String> {
    in_batches_beside(kept, &[])
}

/// [`in_batches`], for a runner that said it holds `held` (#4088): only the files that differ go,
/// with the paths it should no longer hold and the digest of the whole. `held` is only what a
/// runner said, so it decides nothing but what is sent: what the runner keeps must come to that
/// digest, and any file the desktop has that is sent is sent as [`in_batches`] sends it. All of
/// them go where `held` has nothing in common with them, or is not a list a runner could hold.
pub fn in_batches_beside(kept: &Kept, held: &[wire::HeldSetting]) -> Result<ToKeep, String> {
    let mut profile = wire::ProfileToKeep::from(kept.clone());
    let Some(folder) = &kept.settings_in else {
        return Ok(ToKeep {
            profile,
            batches: Vec::new(),
            left_behind: Vec::new(),
        });
    };
    let most = wire::MOST_PROFILE_SETTINGS_BYTES - REWRITING_ROOM;
    let too_large = |path: &str, bytes: usize| {
        (super::sent::rewritten(&kept.harness, path) && bytes > MOST_SETTINGS_BYTES).then(|| {
            format!(
                "its settings file {} in {} is larger than a settings file can be ({} KiB)",
                crate::shown::short(path),
                folder.display(),
                MOST_SETTINGS_BYTES / 1024
            )
        })
    };
    let wholes = super::made::settings_measured(&kept.harness, folder)?;
    for (path, (bytes, _)) in &wholes {
        if let Some(why) = too_large(path, usize::try_from(*bytes).unwrap_or(usize::MAX)) {
            return Err(why);
        }
    }
    let (going, mut behind) =
        wholes_that_fit(wholes, most as u64, wire::MOST_PROFILE_SETTINGS_FILES);
    let read = super::made::settings_read(
        &kept.harness,
        folder,
        most,
        wire::MOST_PROFILE_SETTINGS_FILES,
        &|path| going.contains(whole_of(path)),
    )?;
    // What grew since it was measured stays behind whole too.
    for (path, bytes) in &read.left_behind {
        if let Some(why) = too_large(path, usize::MAX) {
            return Err(why);
        }
        *behind.entry(whole_of(path).to_owned()).or_default() += bytes;
    }
    let mut sent = Vec::with_capacity(read.files.len());
    for (path, bytes) in read.files {
        if let Some(why) = too_large(&path, bytes.len()) {
            return Err(why);
        }
        match behind.get_mut(whole_of(&path)) {
            Some(size) => *size += bytes.len() as u64,
            None => {
                let bytes = super::sent::without_credentials(&kept.harness, &path, &bytes)?;
                sent.push(wire::SettingsFile { path, bytes });
            }
        }
    }
    let sent = match only_what_changed(&kept.harness, sent, held) {
        Ok((changed, sent)) => {
            profile.settings_changed = Some(changed);
            sent
        }
        Err(all) => all,
    };
    profile.settings_staged = Some(u32::try_from(sent.len()).unwrap_or(u32::MAX));
    Ok(ToKeep {
        batches: wire::settings_in_batches(&kept.name, sent),
        left_behind: behind
            .into_iter()
            .map(|(path, bytes)| LeftBehind { path, bytes })
            .collect(),
        profile,
    })
}

/// The most bytes of paths a keep names removed, well within the control frame it goes in: more
/// than that, and all the settings go.
const MOST_REMOVED_BYTES: usize = wire::MOST_PROFILE_SETTINGS_FILES * 16;

/// **`sent`, cut down to what differs from `held`**, with what the keep then says of the rest; or
/// `Err`, all of `sent`, where `held` shares no file with it or is not a list of settings files.
fn only_what_changed(
    harness: &str,
    sent: Vec<wire::SettingsFile>,
    held: &[wire::HeldSetting],
) -> Result<(wire::ChangedSettings, Vec<wire::SettingsFile>), Vec<wire::SettingsFile>> {
    let Some(held_digests) = super::made::held_digests(harness, held) else {
        return Err(sent);
    };
    let digests: Vec<String> = sent
        .iter()
        .map(|file| wire::settings_digest(&file.bytes))
        .collect();
    let same = |(file, digest): (&wire::SettingsFile, &String)| {
        held_digests.get(file.path.as_str()) == Some(&digest.as_str())
    };
    if !sent.iter().zip(&digests).any(same) {
        return Err(sent);
    }
    let whole = wire::whole_settings_digest(
        sent.iter()
            .zip(&digests)
            .map(|(file, digest)| (file.path.as_str(), digest.as_str())),
    );
    let ours: std::collections::HashSet<&str> = sent.iter().map(|f| f.path.as_str()).collect();
    let removed: Vec<String> = held_digests
        .keys()
        .filter(|path| !ours.contains(*path))
        .map(|path| (*path).to_owned())
        .collect();
    if removed.iter().map(String::len).sum::<usize>() > MOST_REMOVED_BYTES {
        return Err(sent);
    }
    let changed = sent
        .iter()
        .zip(&digests)
        .filter(|pair| !same(*pair))
        .map(|(file, _)| file.clone())
        .collect();
    Ok((wire::ChangedSettings { removed, whole }, changed))
}

/// **A page of what this runner, with `homes` its store, holds of the profile `name` it keeps for
/// the desktop `desktop`** (`settings_held`, #4088), after the path `after`, and whether more
/// follow: read off its disk now, never remembered, since a chat may be able to write there. One
/// it does not keep for that desktop holds nothing, so a runner's own settings are never told.
pub fn held_for_the_desktop(
    homes: &Homes,
    desktop: &str,
    name: &str,
    after: Option<&str>,
) -> Result<(Vec<wire::HeldSetting>, bool), String> {
    let Some(record) = kept_record(homes, desktop, name)? else {
        return Ok((Vec::new(), false));
    };
    let held = held_files(&record)?
        .files
        .iter()
        .map(|(path, bytes)| wire::HeldSetting {
            path: path.clone(),
            digest: wire::settings_digest(bytes),
        })
        .collect();
    Ok(wire::settings_held_page(held, after))
}

/// The record of the profile `name` this runner keeps for `desktop`, with settings of its own.
fn kept_record(homes: &Homes, desktop: &str, name: &str) -> Result<Option<MachineProfile>, String> {
    Ok(super::list(homes.config_root())?
        .into_iter()
        .find(|p| p.name == name && p.kept_for.as_deref() == Some(desktop) && !p.sandbox_only))
}

/// The settings files `record` holds here, read as a desktop reads its own, within the limits a
/// link holds.
fn held_files(record: &MachineProfile) -> Result<super::made::SettingsRead, String> {
    super::made::settings_read(
        &record.harness,
        &record.folders.config,
        wire::MOST_PROFILE_SETTINGS_BYTES,
        wire::MOST_PROFILE_SETTINGS_FILES,
        &|_| true,
    )
}

/// **The settings of a profile whose keep sent only what changed** (#4088): what this runner
/// holds now, read off its disk, with `staged` in place and what `changed` names removed. Refused
/// where it keeps no such profile for `desktop`, or holds what no read takes in, so the desktop
/// sends them all.
fn settings_after_change(
    homes: &Homes,
    desktop: &str,
    sent: &wire::ProfileToKeep,
    changed: &wire::ChangedSettings,
    staged: Vec<(String, Vec<u8>)>,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let not_as_told = || {
        format!(
            "the settings of the profile '{}' here are not what its desktop was told",
            crate::shown::short(&sent.name)
        )
    };
    let record = kept_record(homes, desktop, &sent.name)?
        .filter(|record| record.harness == sent.harness)
        .ok_or_else(not_as_told)?;
    let read = held_files(&record)?;
    if !read.left_behind.is_empty() {
        return Err(not_as_told());
    }
    let mut whole: BTreeMap<String, Vec<u8>> = read.files.into_iter().collect();
    for path in &changed.removed {
        whole.remove(path);
    }
    whole.extend(staged);
    let size: usize = whole.values().map(Vec::len).sum();
    if size > wire::MOST_PROFILE_SETTINGS_BYTES || whole.len() > wire::MOST_PROFILE_SETTINGS_FILES {
        return Err(not_as_told());
    }
    Ok(whole.into_iter().collect())
}

/// How many of what stayed behind a Notice names before it counts the rest.
const NAMED_LEFT_BEHIND: usize = 6;

/// **What a chat's tab says of what stayed behind on pool `pool`** (#4081): what, its size, and
/// how to bring it. `None` where nothing did.
pub fn left_behind_note(pool: &str, name: &str, behind: &[LeftBehind]) -> Option<String> {
    use purlis_session_protocol::session::{
        MOST_PROFILE_SETTINGS_BYTES, MOST_PROFILE_SETTINGS_FILES,
    };
    const MIB: u64 = 1024 * 1024;
    if behind.is_empty() {
        return None;
    }
    let mut largest: Vec<_> = behind.iter().collect();
    largest.sort_by_key(|one| std::cmp::Reverse(one.bytes));
    let mut named: Vec<String> = largest
        .iter()
        .take(NAMED_LEFT_BEHIND)
        .map(|one| {
            format!(
                "{} ({} MiB)",
                crate::shown::short(&one.path),
                one.bytes.div_ceil(MIB)
            )
        })
        .collect();
    if let Some(more) = behind
        .len()
        .checked_sub(NAMED_LEFT_BEHIND)
        .filter(|n| *n > 0)
    {
        named.push(format!("{more} more"));
    }
    Some(format!(
        "This chat runs on pool {pool} without some of the profile '{}''s settings, which came to \
         more than the {} MiB and {MOST_PROFILE_SETTINGS_FILES} files a runner keeps: {}. Make \
         them smaller or remove them from the profile, and its next chat on the pool brings the \
         rest.",
        crate::shown::short(name),
        MOST_PROFILE_SETTINGS_BYTES as u64 / MIB,
        named.join(", ")
    ))
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
            // A pool's old runner carries its sign-ins once its last chat has ended, after this
            // one kept the profile (#4055): taken in where it has none of its own.
            super::made::take_carried(homes, home, record);
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
        // The sign-ins its pool's old runner carried to this one as it drained (#4011).
        super::made::take_carried(homes, home, &record);
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
/// sign-in. Asked of the version it is pinned to where this runner installs it (#4007), since a
/// runner image carries no harness on `PATH`.
///
/// Where its harness cannot tell because the version it pins is still being fetched, which this
/// look may have started, it is said to be fetching (#4008), so the desktop asks again once the
/// fetch has ended.
pub fn standing(record: &MachineProfile, look: &dyn Look) -> Standing {
    let asked = || match sign_in::asked_of(record, look) {
        Some(profile) => {
            sign_in::forget(&profile);
            sign_in::standing_remembered(&profile, look)
        }
        None => Standing::CannotTell,
    };
    let installed = look.pinned_program(record).is_some();
    match asked() {
        Standing::CannotTell if look.fetching(record) => Standing::Fetching,
        // A fetch that ended while it was asked is asked of once more, so the row is not left
        // saying the harness cannot tell.
        Standing::CannotTell if !installed && look.pinned_program(record).is_some() => asked(),
        said => said,
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
            settings_staged: None,
            settings_changed: None,
        }
    }
}

/// The standing a runner's `profile_kept` names.
pub fn standing_on_the_wire(said: purlis_session_protocol::session::ProfileStanding) -> Standing {
    use purlis_session_protocol::session::ProfileStanding as Said;
    match said {
        Said::SignedIn => Standing::SignedIn,
        Said::SignedOut => Standing::SignedOut,
        Said::Fetching => Standing::Fetching,
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
        Standing::Fetching => Said::Fetching,
        Standing::CannotTell => Said::CannotTell,
    }
}

/// **What a runner's host answers its desktop's `keep_profile` with** (1.43): the profile
/// `sent` kept for the desktop `desktop` in this runner's store under `homes`, with `home` the
/// runner's home, at `now`, and whether its harness says, through `look`, that it is signed in
/// here. The settings `sent` carries (1.50, #3448) are kept in place of the ones kept before;
/// none sent leaves them as they are. Those of a keep that sent only what changed (#4088) are
/// what it holds with those in place, read off its disk now, checked whole as any others and kept
/// as any others, only where they come to the digest the keep names, so nothing it said of them
/// decides it.
pub fn kept_for_the_desktop(
    homes: &Homes,
    home: &Path,
    desktop: &str,
    sent: &purlis_session_protocol::session::ProfileToKeep,
    look: &dyn Look,
    now: u64,
) -> Result<purlis_session_protocol::session::ProfileStanding, String> {
    // The settings are checked before anything of the profile is kept, so settings it refuses
    // leave the runner's record as it was.
    if sent.settings_changed.is_some() && sent.settings.is_none() {
        return Err("a keep of what changed named no settings staged for it".to_owned());
    }
    let checked = match &sent.settings {
        Some(_) if sent.sandbox_only => {
            return Err(format!(
                "the profile '{}' is for sandboxed chats only, and its settings are purlis's, so \
                 this runner keeps no settings sent for it",
                crate::shown::short(&sent.name)
            ));
        }
        Some(settings) => {
            let files: Vec<(String, Vec<u8>)> = settings
                .iter()
                .map(|file| (file.path.clone(), file.bytes.clone()))
                .collect();
            let files = match &sent.settings_changed {
                Some(changed) => settings_after_change(homes, desktop, sent, changed, files)?,
                None => files,
            };
            let checked = super::made::settings_checked(&sent.harness, &sent.name, &files)?;
            if let Some(changed) = &sent.settings_changed {
                let digests: Vec<(&str, String)> = checked
                    .iter()
                    .map(|(path, bytes)| (path.as_str(), wire::settings_digest(bytes)))
                    .collect();
                let whole = wire::whole_settings_digest(
                    digests
                        .iter()
                        .map(|(path, digest)| (*path, digest.as_str())),
                );
                if whole != changed.whole {
                    return Err(format!(
                        "the settings of the profile '{}' here would not be what its desktop has",
                        crate::shown::short(&sent.name)
                    ));
                }
            }
            Some(checked)
        }
        None => None,
    };
    let record = keep(homes, home, &Kept::from(sent), desktop, now)?;
    if let Some(files) = &checked {
        super::made::settings_kept(homes, &record, files)?;
    }
    Ok(standing_for_the_wire(standing(&record, look)))
}

/// **The name of each profile this runner, with `homes` its store, keeps for the desktop
/// `desktop`** (#4060): what the desktop asks before it forgets the ones no longer chosen for
/// the pool. Never one of the runner's own, or one kept for another desktop.
pub fn kept_for(homes: &Homes, desktop: &str) -> Result<Vec<String>, String> {
    Ok(super::list(homes.config_root())?
        .into_iter()
        .filter(|record| record.kept_for.as_deref() == Some(desktop))
        .map(|record| record.name)
        .collect())
}

/// **Forgets the profile `name` this runner keeps for the desktop `desktop`** (#4047), with
/// `homes` its store and `home` its home: what a desktop that takes the profile off the pool
/// asks. Its record goes, and with it where its harness keeps its sign-in here: a made
/// profile's folder, or, for the desktop's adopted folder kept as the harness's default folder,
/// that harness's own folders in `home` (`~/.claude`, `~/.codex`, opencode's `opencode` under
/// the XDG folders), never the XDG folders themselves. A profile the runner does not keep is
/// nothing, and so is one of its own or one kept for another desktop, which stays as it is:
/// nothing of this desktop's has that name, so its pool's record may leave it out (#4060).
pub fn forget(homes: &Homes, home: &Path, name: &str, desktop: &str) -> Result<(), String> {
    let Some(record) = super::list(homes.config_root())?
        .into_iter()
        .find(|p| p.name == name)
    else {
        super::forget_carried(homes, name);
        return Ok(());
    };
    let shown = crate::shown::short(name);
    let theirs = || {
        format!(
            "this runner has a profile named '{shown}' of its own, so it forgets none of that \
             name for the desktop"
        )
    };
    if record.kept_for.as_deref() != Some(desktop) {
        return Ok(());
    }
    // The harness's own folders here, for the desktop's adopted folder kept in its place.
    let own_folders = match &record.adopted {
        None => Vec::new(),
        Some(adopted) => {
            let (config, data, sub) = super::defaults(&record.harness, home).ok_or_else(theirs)?;
            let defaults = super::Folders {
                config: config.clone(),
                data: data.clone(),
            };
            if !adopted.variables.is_empty() || record.folders != defaults {
                return Err(theirs());
            }
            [Some(config), data]
                .into_iter()
                .flatten()
                .map(|folder| {
                    if sub.is_empty() {
                        folder
                    } else {
                        folder.join(sub)
                    }
                })
                .collect()
        }
    };
    sign_in::forget_of(&record);
    super::forget_carried(homes, name);
    // Its folders go before its record, so one that could not be deleted leaves the record to
    // ask the removal again by.
    for folder in own_folders {
        match std::fs::remove_dir_all(&folder) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "the profile '{shown}' was not forgotten: its folder {} could not be \
                     deleted: {e}",
                    folder.display()
                ));
            }
        }
    }
    super::remove(homes, name, &super::removal(homes, name)?)?;
    Ok(())
}

/// **The harness whose own folders [`forget`] would delete for the profile `name`** (#4142): the
/// desktop `desktop`'s adopted folder, kept here as that harness's default, which a chat here
/// with no profile uses too. None for a made profile, whose folder is its own alone, and for one
/// [`forget`] leaves as it is.
pub fn deleting_own_folders(
    homes: &Homes,
    name: &str,
    desktop: &str,
) -> Result<Option<String>, String> {
    Ok(super::list(homes.config_root())?
        .into_iter()
        .find(|p| p.name == name && p.kept_for.as_deref() == Some(desktop))
        .filter(|record| record.adopted.is_some())
        .map(|record| record.harness))
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
    let Some(record) = ready_on_for(config_root, pool, name, keep, For::Start)? else {
        return Ok(());
    };
    waits_on_pool(root, pool, &record);
    let shown = crate::shown::short(name);
    Err(format!(
        "the profile '{shown}' is not signed in on pool {pool}. Nothing was started. It waits in \
         the Inbox as Profile {shown} on pool {pool}, whose Sign in on pool {pool} runs its \
         harness's own sign-in on the runner; then start the chat again."
    ))
}

/// [`ready_on`], for a chat that moves to pool `pool`'s runner (#2048): the same keep, said of the
/// move. **A profile signed out there makes the move wait** ([`NotYet::SignIn`], #3477): the
/// move's own notice asks the sign-in, so no Inbox wait is added, and the move goes ahead once it
/// is signed in. Anything else that keeps it from the runner refuses the move, in words.
pub fn ready_to_move_on(
    config_root: &Path,
    pool: &str,
    name: &str,
    keep: &dyn Fn(&Kept) -> Result<Option<Standing>, String>,
) -> Result<(), NotYet> {
    match ready_on_for(config_root, pool, name, keep, For::Move)? {
        Some(record) => Err(NotYet::SignIn(SignInAsked {
            pool: pool.to_owned(),
            profile: record.name.clone(),
            kind: record.harness.clone(),
            harness: harness_title(&record.harness),
        })),
        None => Ok(()),
    }
}

/// **Why a chat does not move to a pool's runner yet** (#3477).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotYet {
    /// Its profile is not signed in there: the move waits for it to be.
    SignIn(SignInAsked),
    /// It does not move, and why, in a sentence.
    Refused(String),
}

/// **The sign-in a move waits for** (#3477): profile `profile` on pool `pool`'s runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignInAsked {
    pub pool: String,
    pub profile: String,
    /// Its harness, as a profile's kind names it.
    pub kind: String,
    /// Its harness as people call it: `Claude Code`.
    pub harness: String,
}

impl NotYet {
    /// Why the chat did not move, in a sentence, where it does not wait any more: a move
    /// switched while its profile was signed out there, say.
    pub fn said(&self) -> String {
        match self {
            NotYet::SignIn(asked) => format!(
                "the profile '{}' is not signed in on pool {}. The chat did not move; sign it in \
                 on pool {}, then ask for the move again.",
                crate::shown::short(&asked.profile),
                asked.pool,
                asked.pool
            ),
            NotYet::Refused(why) => why.clone(),
        }
    }
}

impl From<String> for NotYet {
    fn from(why: String) -> Self {
        NotYet::Refused(why)
    }
}

impl From<&str> for NotYet {
    fn from(why: &str) -> Self {
        NotYet::Refused(why.to_owned())
    }
}

/// A harness as people call it, from a profile's kind.
fn harness_title(kind: &str) -> String {
    crate::harness_declaration::builtin(kind).map_or_else(|| kind.to_owned(), |d| d.title.clone())
}

/// What a profile is readied on a runner for: a chat's start there, or its move there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum For {
    Start,
    Move,
}

/// [`ready_on`] for `purpose`: `Some` record where its harness says it is signed out on the
/// runner, which a start refuses and a move waits on.
fn ready_on_for(
    config_root: &Path,
    pool: &str,
    name: &str,
    keep: &dyn Fn(&Kept) -> Result<Option<Standing>, String>,
    purpose: For,
) -> Result<Option<MachineProfile>, String> {
    let nothing = match purpose {
        For::Start => "Nothing was started",
        For::Move => "The chat did not move",
    };
    let Some(record) = super::list(config_root)?
        .into_iter()
        .find(|p| p.name == name)
    else {
        return Ok(None);
    };
    let shown = crate::shown::short(name);
    // A start or a move sends the profile's settings with it, as they are now (#3448).
    let kept = Kept::of(&record)
        .map_err(|why| format!("{why}. {nothing}."))?
        .with_settings_of(&record);
    match keep(&kept).map_err(|why| {
        format!("pool {pool}'s runner did not keep the profile '{shown}': {why}. {nothing}.")
    })? {
        Some(Standing::SignedOut) => Ok(Some(record)),
        Some(Standing::SignedIn | Standing::CannotTell | Standing::Fetching) | None => Ok(None),
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
    /// When a start was last refused, to the instant: what a watcher compares (#4203).
    pub raised: std::time::Instant,
}

static POOL_WAITS: Mutex<Vec<PoolWait>> = Mutex::new(Vec::new());

/// Records that a start in the project at `root` on pool `pool` was refused because `record` is
/// not signed in there.
fn waits_on_pool(root: &Path, pool: &str, record: &MachineProfile) {
    let mut all = POOL_WAITS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(wait) = all
        .iter_mut()
        .find(|w| w.root == root && w.pool == pool && w.profile == record.name)
    {
        wait.raised = std::time::Instant::now();
        return;
    }
    all.push(PoolWait {
        root: root.to_path_buf(),
        pool: pool.to_owned(),
        profile: record.name.clone(),
        kind: record.harness.clone(),
        harness: harness_title(&record.harness),
        since: crate::halt::now(),
        raised: std::time::Instant::now(),
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
/// project at `root` (`None`: in every project): its waits go. The projects whose wait went,
/// whose windows have an Ask to stop drawing (#4205).
pub fn let_go(root: Option<&Path>, pool: &str, name: &str) -> Vec<PathBuf> {
    let mut went = Vec::new();
    POOL_WAITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|w| {
            let goes = w.pool == pool && w.profile == name && root.is_none_or(|r| w.root == r);
            if goes {
                went.push(w.root.clone());
            }
            !goes
        });
    went
}

/// The person left the sign-in of `name` on pool `pool` in the project at `root`: its wait goes,
/// if it was raised before `seen`, when the shell was last seen open (#4203).
pub fn let_go_older(root: &Path, pool: &str, name: &str, seen: std::time::Instant) {
    POOL_WAITS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|w| !(w.pool == pool && w.profile == name && w.root == root && w.raised < seen));
}

#[cfg(test)]
mod tests;
