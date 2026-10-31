// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The profiles purlis makes** (#2034, ADR 0095): made in the window or with `purlis profile
//! create`, recorded in this machine's store ([`super::update`]) beside the folders adopted in
//! place, and kept in a folder of their own in the data home, `<data>/profiles/<name>/`, 0700,
//! holding `config/` and, for a harness that declares `data_env`, `data/`.
//!
//! **A made profile's settings are imported from this machine's harness settings, never its
//! credential files.** What is copied is a list of settings files per harness ([`ALLOWED`]),
//! each as it is, and nothing else: a file the list does not name stays where it is. Inside a
//! listed folder, a hidden entry or one named like a credential is left out too. No link is
//! followed, and no file with another name on the disk is copied. A harness purlis does not
//! ship has no list, so no profile of it is made.
//!
//! **Removing** a made profile deletes its folder, and only a folder directly under
//! `<data>/profiles/`: a record that names any other folder deletes nothing. Removing an adopted
//! profile forgets its record and its folder ([`super::Store::forget`]), so it is not adopted
//! again, and never deletes the folder, which is the harness's own.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::Map;

use super::{Folders, Imported, MachineProfile};

/// The made profiles' folder under the data home.
pub const DIR: &str = "profiles";

/// A made profile's folder for its harness's `config_env`, under the profile's own folder.
pub const CONFIG: &str = "config";

/// A made profile's folder for its harness's `data_env`, where it declares one.
pub const DATA: &str = "data";

/// How deep an imported settings folder is read.
const DEEPEST: usize = 8;

/// Where this machine keeps its profiles: the config home its store is in, and the data home
/// its made profiles' folders are in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Homes {
    config_root: PathBuf,
    folders: PathBuf,
    /// The data home, where a profile's kept browser logins are (#2021).
    data: PathBuf,
    /// The keyring those logins' key is in: this machine's, or a file a test names.
    keyring: Option<PathBuf>,
}

impl Homes {
    /// The config home `config_root` and the data home `data`.
    pub fn at(config_root: &Path, data: &Path) -> Self {
        Self {
            config_root: config_root.to_path_buf(),
            folders: data.join(DIR),
            data: data.to_path_buf(),
            keyring: None,
        }
    }

    /// These homes, the key of kept browser logins kept in the file `keyring` in place of this
    /// machine's keyring: for a test.
    #[cfg(test)]
    #[must_use]
    pub fn keyring_in(mut self, keyring: PathBuf) -> Self {
        self.keyring = Some(keyring);
        self
    }

    /// **Wipes profile `name`'s kept browser logins** (#2021): every browser folder of it a
    /// browser left behind, its stores, and the key once no store is left. A keyring that cannot
    /// be asked is warned of, and the stores are wiped all the same.
    /// **Marks profile `name` removed for its kept browser logins** (#2693), under the keyring's
    /// lock, before anything of it goes: a browser of it running now never makes a store again,
    /// kept before or not, and one that took its hold before the mark is wiped or refused at its
    /// write-back. A removal that cannot mark it removes nothing.
    fn mark_kept_logins_removed(&self, name: &str) -> Result<(), String> {
        crate::browserlogins::removed(&self.data, name).map_err(|why| {
            format!("the profile '{name}' was not removed, so nothing of it went: {why}")
        })
    }

    fn wipe_kept_logins(&self, name: &str) -> Result<(), String> {
        let folders = crate::browsers::browsers_in(&self.data);
        crate::browserlogins::sweep(&folders, None, &|of, _| of == name);
        // Marked removed already ([`Homes::mark_kept_logins_removed`]). Nothing to wipe asks no
        // keyring for a device id or a key.
        let dir = self.data.join(crate::browserlogins::DIR).join(name);
        if !dir.exists() {
            return Ok(());
        }
        let store: Box<dyn crate::secrets::keyring::Store> = match &self.keyring {
            Some(file) => Box::new(crate::secrets::keyring::FileStore::at(file.clone())),
            None => Box::new(crate::secrets::keyring::OsStore),
        };
        let device = match &self.keyring {
            Some(_) => Ok("test-device".to_owned()),
            None => crate::machine::this_device_id(),
        };
        let device = device.unwrap_or_else(|why| {
            tracing::warn!(
                "purlis: this device's id was not read ({why}); profile {name}'s kept logins \
                 are wiped all the same"
            );
            String::new()
        });
        crate::browserlogins::wipe_profile(
            &self.data,
            &folders,
            name,
            &crate::browserlogins::Keys {
                store: &*store,
                device,
            },
        )
        .map(|_| ())
    }

    /// This process's config home and data home, to write: none where either is missing, or
    /// the data home is inside a project or a git work tree (ADR 0075 §6).
    pub fn here() -> Option<Self> {
        let config = crate::machine::config_root()?;
        let data = crate::datahome::root()?;
        crate::datahome::refusal(&data)
            .is_none()
            .then(|| Self::at(&config, &data))
    }

    /// The config home the store is in.
    pub fn config_root(&self) -> &Path {
        &self.config_root
    }

    /// The folder purlis makes its profiles in.
    pub fn dir(&self) -> &Path {
        &self.folders
    }

    /// The folder purlis makes for the profile `name`.
    pub fn folder_of(&self, name: &str) -> PathBuf {
        self.folders.join(name)
    }
}

/// Where this machine's harness settings are read from: its home and its environment.
pub struct MachineSettings<'a> {
    pub home: &'a Path,
    pub env: &'a dyn Fn(&str) -> Option<String>,
}

impl MachineSettings<'_> {
    /// The folder `kind`'s own settings are in on this machine: the folder its declaration's
    /// `config_env` names where it is set and absolute, the harness's default otherwise
    /// ([`super::defaults`]), and the folder of its own name under it where it keeps one.
    pub fn of(&self, kind: &str) -> Option<PathBuf> {
        let variable = crate::harness_declaration::builtin(kind)?
            .folders
            .config_env
            .as_deref()?;
        let (default, _, own) = super::defaults(kind, self.home)?;
        let named = (self.env)(variable)
            .map(PathBuf::from)
            .filter(|dir| dir.is_absolute())
            .unwrap_or(default);
        Some(own_under(named, own))
    }
}

/// `folder`, or the folder of its own name `own` the harness keeps under it.
fn own_under(folder: PathBuf, own: &str) -> PathBuf {
    if own.is_empty() {
        folder
    } else {
        folder.join(own)
    }
}

/// A harness's settings, as purlis imports them: which files they are.
pub struct Allowed {
    /// The harness, as a profile's `kind` names it.
    pub kind: &'static str,
    /// The settings files, by name.
    pub files: &'static [&'static str],
    /// The settings folders, by name: their files are copied, but for a hidden entry or one
    /// named like a credential.
    pub folders: &'static [&'static str],
}

/// The settings purlis imports, per harness it ships. Anything not named here is not copied:
/// sign-ins (Claude Code's `.credentials.json`, Codex's `auth.json`, opencode's data folder),
/// conversations, histories, caches and logs among it.
pub const ALLOWED: [Allowed; 3] = [
    Allowed {
        kind: "claude",
        files: &["settings.json", "CLAUDE.md"],
        folders: &["agents", "commands", "output-styles", "skills"],
    },
    Allowed {
        kind: "codex",
        files: &["config.toml", "AGENTS.md"],
        folders: &["prompts"],
    },
    Allowed {
        kind: "opencode",
        files: &["opencode.json", "opencode.jsonc", "AGENTS.md", "tui.json"],
        folders: &[
            "agent", "agents", "command", "commands", "mode", "modes", "themes",
        ],
    },
];

/// `kind`'s settings list, for a harness purlis ships.
pub fn allowed(kind: &str) -> Option<&'static Allowed> {
    ALLOWED.iter().find(|a| a.kind == kind)
}

/// What an import copied and what it left out, each as a path under the folder it was read
/// from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Copied {
    pub copied: Vec<String>,
    /// Each with why, in a sentence's tail: `"skills/x.md: a link, …"`.
    pub skipped: Vec<String>,
}

/// A profile made, and what its settings import did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Made {
    pub record: MachineProfile,
    pub copied: Copied,
}

/// Makes the profile `name` of the harness `kind`, its settings imported from `machine`'s, at
/// `now`. The record, or the sentence saying why not; a refusal leaves nothing behind. A
/// built-in's name (`claude`) is kept for the folder adopted in place.
pub fn create(
    homes: &Homes,
    name: &str,
    kind: &str,
    machine: &MachineSettings,
    now: u64,
) -> Result<Made, String> {
    make(homes, name, kind, Making::imported_from(machine), now)
}

/// **Makes the profile `name` of the harness `kind` for sandboxed chats only** (#2037, ruled
/// 2026-10-15): a folder of its own, with nothing imported, since its settings are purlis's,
/// and no sign-in until its harness's own sign-in is run in it. The record, or the sentence
/// saying why not; a refusal leaves nothing behind.
pub fn create_sandbox_only(
    homes: &Homes,
    name: &str,
    kind: &str,
    now: u64,
) -> Result<Made, String> {
    make(homes, name, kind, Making::SANDBOX_ONLY, now)
}

/// How a profile is made: where its settings are imported from, whether it is for sandboxed
/// chats only, and whether it may take a built-in's name.
pub(super) struct Making<'a> {
    /// This machine's settings, imported into it; none imports nothing.
    pub(super) machine: Option<&'a MachineSettings<'a>>,
    pub(super) sandbox_only: bool,
}

impl<'a> Making<'a> {
    const SANDBOX_ONLY: Making<'static> = Making {
        machine: None,
        sandbox_only: true,
    };

    fn imported_from(machine: &'a MachineSettings<'a>) -> Self {
        Making {
            machine: Some(machine),
            sandbox_only: false,
        }
    }
}

/// [`create`], with its settings imported from `machine`'s, or, with none, a profile for
/// sandboxed chats only ([`create_sandbox_only`]); or a runner's keeping of a desktop's
/// profile ([`super::onrunner`]).
pub(super) fn make(
    homes: &Homes,
    name: &str,
    kind: &str,
    making: Making<'_>,
    now: u64,
) -> Result<Made, String> {
    if !crate::profiles::name_ok(name) {
        return Err(format!(
            "'{}' cannot be a profile's name: use letters, digits, '-' and '_', starting with a \
             letter or a digit.",
            crate::shown::short(name)
        ));
    }
    if crate::profiles::KINDS.iter().any(|k| k.word == name) {
        return Err(format!(
            "'{name}' is the name of the profile purlis gives every project for that harness. \
             Pick another name."
        ));
    }
    let folder = homes.folder_of(name);
    let mut made_folder = false;
    let answer = super::update(&homes.config_root, |store| {
        make_in(store, homes, name, kind, &making, now, &mut made_folder)
    });
    match answer {
        Ok(Ok(made)) => Ok(made),
        Ok(Err(why)) | Err(why) => {
            if made_folder {
                let _ = std::fs::remove_dir_all(&folder);
            }
            Err(why)
        }
    }
}

/// [`make`]'s change of the store `store`, under its lock: the profile `name` made, its folder
/// with it, and its record pushed. `made_folder` says whether its folder was made, for the
/// caller to remove it where the change is not written.
pub(super) fn make_in(
    store: &mut super::Store,
    homes: &Homes,
    name: &str,
    kind: &str,
    making: &Making<'_>,
    now: u64,
    made_folder: &mut bool,
) -> Result<Made, String> {
    let machine = making.machine;
    let (folders, allowed) = harness_of(kind)?;
    let folder = homes.folder_of(name);
    if store.profiles.iter().any(|p| p.name == name) {
        return Err(format!(
            "this machine already has a profile named '{name}'. Pick another name, or \
             remove that one first."
        ));
    }
    if std::fs::symlink_metadata(&folder).is_ok() {
        return Err(format!(
            "{} is already there, so purlis made no profile '{name}'. Move it away, or \
             pick another name.",
            folder.display()
        ));
    }
    let made = (|| {
        private_dirs(&homes.folders)?;
        private_dir(&folder)?;
        *made_folder = true;
        let config = folder.join(CONFIG);
        private_dir(&config)?;
        let data = match folders.data_env {
            Some(_) => {
                let data = folder.join(DATA);
                private_dir(&data)?;
                Some(data)
            }
            None => None,
        };
        let from = machine.and_then(|machine| machine.of(kind));
        let copied = match &from {
            Some(from) => import_into(allowed, from, &folder, &config)?,
            None => Copied::default(),
        };
        Ok::<_, io::Error>((config, data, from, copied))
    })();
    let (config, data, from, copied) = made.map_err(|e| could_not(homes, &e))?;
    let record = MachineProfile {
        name: name.to_owned(),
        harness: kind.to_owned(),
        title: name.to_owned(),
        folders: Folders { config, data },
        adopted: None,
        imported: from.map(|from| Imported { at: now, from }),
        sign_in: None,
        pinned: None,
        sandbox_only: making.sandbox_only,
        kept_for: None,
        rest: Map::new(),
    };
    store.profiles.push(record.clone());
    Ok(Made { record, copied })
}

/// **Takes in the sign-in carried to this runner for `record`** (#4011, #4055), a profile it
/// keeps for its desktop: purlis's own, or the desktop's adopted folder kept as its harness's
/// default folder under `home`. Each batch a drained runner carried
/// (`<data>/carried/<batch>/<name>/`) is looked in, and only the sign-in file of `record`'s
/// harness ([`super::sign_in_file_of`]), a plain file reached through no link, is moved in, by
/// a link that never replaces a file: one the profile has here, signed in on this runner, stays
/// over it. What was carried for the profile is then removed. What cannot be taken in is left
/// out: the profile is signed in again, as before.
pub(super) fn take_carried(homes: &Homes, home: &Path, record: &MachineProfile) {
    let carried = homes.data.join(super::CARRIED);
    let Ok(batches) = std::fs::read_dir(&carried) else {
        return;
    };
    let mut batches: Vec<PathBuf> = batches.flatten().map(|entry| entry.path()).collect();
    batches.sort();
    let into = super::sign_in_file_of(&record.harness).and_then(|file| {
        let (under, rest) = file.split_once('/')?;
        let folder = match under {
            "config" => Some(&record.folders.config),
            _ => record.folders.data.as_ref(),
        }?;
        Some((file, folder.join(rest)))
    });
    // Where the folders on its way may be made: a profile purlis made has its own folder, and an
    // adopted one the runner's home.
    let made_folder = homes.folder_of(&record.name);
    let root = if record.adopted.is_none() {
        made_folder.as_path()
    } else {
        home
    };
    for batch in batches {
        if !std::fs::symlink_metadata(&batch).is_ok_and(|meta| meta.is_dir()) {
            continue;
        }
        let mine = batch.join(&record.name);
        if std::fs::symlink_metadata(&mine).is_err() {
            continue;
        }
        if let Some((file, to)) = &into {
            let from = mine.join(file);
            let plain = crate::contain::no_link_on_the_way(&carried, &from).is_ok()
                && std::fs::symlink_metadata(&from).is_ok_and(|meta| meta.file_type().is_file());
            if plain
                && std::fs::symlink_metadata(to).is_err()
                && let Some(parent) = to.parent()
                && private_dirs_within(root, parent).is_ok()
                && std::fs::hard_link(&from, to).is_ok()
            {
                let _ = std::fs::remove_file(&from);
            }
        }
        let _ = std::fs::remove_dir_all(&mine);
        let _ = std::fs::remove_dir(&batch);
    }
}

/// Imports `name`'s settings again from `machine`'s, at `now`: each listed settings file and
/// folder replaced by the machine's, and the record's time moved. Only for a profile purlis
/// made; an adopted one's settings are the harness's own.
pub fn import(
    homes: &Homes,
    name: &str,
    machine: &MachineSettings,
    now: u64,
) -> Result<Made, String> {
    super::update(&homes.config_root, |store| {
        let Some(record) = store.profiles.iter_mut().find(|p| p.name == name) else {
            return Err(no_such(name));
        };
        if record.adopted.is_some() {
            return Err(format!(
                "'{name}' is the harness's own folder, adopted in place, so its settings are \
                 already the machine's. There is nothing to import."
            ));
        }
        if record.sandbox_only {
            return Err(format!(
                "'{name}' is a profile for sandboxed chats only, and its settings are purlis's, \
                 so nothing is imported into it."
            ));
        }
        let (_, allowed) = harness_of(&record.harness)?;
        let folder = own_folder(homes, record)?;
        let Some(from) = machine.of(&record.harness) else {
            return Err(format!(
                "purlis cannot tell where this machine keeps {}'s settings, so nothing was \
                 imported.",
                record.harness
            ));
        };
        let config = record.folders.config.clone();
        let copied = (|| {
            let dest = settings_dest(allowed, &config);
            for one in allowed.files.iter().chain(allowed.folders) {
                remove_within(&folder, &dest.join(one))?;
            }
            import_into(allowed, &from, &folder, &config)
        })()
        .map_err(|e| could_not(homes, &e))?;
        record.imported = Some(Imported { at: now, from });
        Ok(Made {
            record: record.clone(),
            copied,
        })
    })?
}

/// What removing a profile takes away, said before it is removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub name: String,
    /// The harness's own folder, adopted in place: only the record goes, and the folder stays.
    pub adopted: bool,
    /// The folder that goes, or for an adopted profile the one that stays.
    pub folder: PathBuf,
    /// The conversations in it that go, each as a path under [`Self::folder`]; `None` where
    /// the harness keeps them in a way purlis cannot list. Empty for an adopted profile, whose
    /// conversations stay.
    pub conversations: Option<Vec<String>>,
}

impl Removal {
    /// The sentence a confirmation says.
    pub fn says(&self) -> String {
        if self.adopted {
            return format!(
                "purlis forgets the profile '{}'. Its folder, {}, is the harness's own and \
                 stays, with its sign-in, settings and conversations.",
                self.name,
                self.folder.display()
            );
        }
        let conversations = match &self.conversations {
            Some(found) if found.is_empty() => "no conversation".to_owned(),
            Some(found) if found.len() == 1 => "its one conversation".to_owned(),
            Some(found) => format!("its {} conversations", found.len()),
            None => "every conversation in it".to_owned(),
        };
        format!(
            "Removing the profile '{}' deletes its folder, {}, with its sign-in, its settings \
             and {conversations}. This cannot be undone.",
            self.name,
            self.folder.display()
        )
    }
}

/// What removing `name` would take away.
pub fn removal(homes: &Homes, name: &str) -> Result<Removal, String> {
    let record = super::list(&homes.config_root)?
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| no_such(name))?;
    removal_of(homes, &record)
}

fn removal_of(homes: &Homes, record: &MachineProfile) -> Result<Removal, String> {
    if record.adopted.is_some() {
        return Ok(Removal {
            name: record.name.clone(),
            adopted: true,
            folder: record.folders.config.clone(),
            conversations: Some(Vec::new()),
        });
    }
    let folder = own_folder(homes, record)?;
    Ok(Removal {
        name: record.name.clone(),
        adopted: false,
        conversations: conversations(record, &folder),
        folder,
    })
}

/// Removes `name`, **only if `shown` is what removing it takes away now**: a profile whose
/// conversations changed while the person read the confirmation is not removed. A made
/// profile's folder is deleted; an adopted one's record goes and its folder is forgotten
/// ([`super::Store::forget`]), never deleted.
pub fn remove(homes: &Homes, name: &str, shown: &Removal) -> Result<Removal, String> {
    super::update(&homes.config_root, |store| {
        let Some(record) = store.profiles.iter().find(|p| p.name == name).cloned() else {
            return Err(no_such(name));
        };
        let now = removal_of(homes, &record)?;
        if &now != shown {
            return Err(format!(
                "the profile '{name}' changed while you were reading what goes, so nothing was \
                 removed. {}",
                now.says()
            ));
        }
        homes.mark_kept_logins_removed(name)?;
        if now.adopted {
            store.forget(&now.folder);
        } else {
            match std::fs::remove_dir_all(&now.folder) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(could_not(homes, &e)),
            }
        }
        store.profiles.retain(|p| p.name != name);
        // Its kept browser logins go with it, made or adopted (#2021).
        homes.wipe_kept_logins(name)?;
        Ok(now)
    })?
}

/// The harness `kind`'s folders and settings list, or why purlis makes no profile of it.
fn harness_of(
    kind: &str,
) -> Result<
    (
        &'static crate::harness_declaration::Folders,
        &'static Allowed,
    ),
    String,
> {
    let refused = || {
        format!(
            "purlis makes profiles of Claude Code (claude), Codex (codex) and opencode \
             (opencode), and '{}' is none of them.",
            crate::shown::short(kind)
        )
    };
    let declared = crate::harness_declaration::builtin(kind).ok_or_else(refused)?;
    let allowed = allowed(kind).ok_or_else(refused)?;
    if declared.folders.config_env.is_none() {
        return Err(refused());
    }
    Ok((&declared.folders, allowed))
}

/// The folder of `record`, a made profile, checked to be the one purlis made for it: directly
/// under `<data>/profiles/`, named after it, a folder and not a link, and the one its `config`
/// is in. Anything else is refused, and nothing is deleted or written there.
fn own_folder(homes: &Homes, record: &MachineProfile) -> Result<PathBuf, String> {
    let folder = homes.folder_of(&record.name);
    let refused = || {
        format!(
            "the profile '{}' names a folder outside purlis's profiles folder ({}), so purlis \
             deletes and writes nothing there.",
            crate::shown::short(&record.name),
            homes.folders.display()
        )
    };
    if !crate::profiles::name_ok(&record.name) || !record.folders.config.starts_with(&folder) {
        return Err(refused());
    }
    if record
        .folders
        .data
        .as_ref()
        .is_some_and(|data| !data.starts_with(&folder))
    {
        return Err(refused());
    }
    match std::fs::symlink_metadata(&folder) {
        Ok(meta) if meta.file_type().is_dir() => {}
        Ok(_) => return Err(refused()),
        // Gone already: there is nothing to delete, and the record can still go.
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(folder),
        Err(e) => return Err(could_not(homes, &e)),
    }
    let (Ok(dir), Ok(found)) = (homes.folders.canonicalize(), folder.canonicalize()) else {
        return Err(refused());
    };
    if found.parent() != Some(dir.as_path()) {
        return Err(refused());
    }
    Ok(folder)
}

/// **The Codex home a wrapped chat on the sandbox-only profile `name` runs in** (#2037), as the
/// kernel names it: `folder`, where it is exactly `<profiles>/<name>/config` with no `..` in it,
/// neither it nor the profile's folder is a link, both are folders this user owns, and the
/// profile's folder sits directly in `profiles`. `None` for anything else, and the chat is not
/// started.
pub fn sandbox_home(profiles: &Path, name: &str, folder: &Path) -> Option<PathBuf> {
    #[cfg(unix)]
    let me = Some(rustix::process::getuid().as_raw());
    #[cfg(not(unix))]
    let me = None;
    sandbox_home_of(profiles, name, folder, me)
}

/// [`sandbox_home`], for the user `me` where the system has users.
fn sandbox_home_of(profiles: &Path, name: &str, folder: &Path, me: Option<u32>) -> Option<PathBuf> {
    if !crate::profiles::name_ok(name)
        || folder
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
        || folder != profiles.join(name).join(CONFIG)
    {
        return None;
    }
    let ours = |path: &Path| {
        let meta = std::fs::symlink_metadata(path).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if me != Some(meta.uid()) {
                return None;
            }
        }
        #[cfg(not(unix))]
        let _ = me;
        meta.file_type().is_dir().then_some(())
    };
    let profile = profiles.join(name);
    ours(&profile)?;
    ours(folder)?;
    let (dir, found) = (profiles.canonicalize().ok()?, folder.canonicalize().ok()?);
    (found == dir.join(name).join(CONFIG)).then_some(found)
}

/// Where `allowed`'s settings go in a profile whose `config_env` is `config`: under the folder
/// of its own name, for a harness that keeps one.
fn settings_dest(allowed: &Allowed, config: &Path) -> PathBuf {
    let own = super::defaults(allowed.kind, Path::new("")).map_or("", |(_, _, own)| own);
    own_under(config.to_path_buf(), own)
}

/// Copies `allowed`'s settings from `from` into the profile folder `folder`, whose
/// `config_env` is `config`. A missing settings folder imports nothing.
///
/// **Read by descriptor, never by path** below the settings folder: the folder is opened once,
/// and each listed name, and each name in a listed folder, is opened relative to the descriptor
/// of the folder it was listed in, with `O_NOFOLLOW`, and judged by `fstat` on what was opened.
/// A link is never followed, wherever it is in the walk, and a name swapped for a link while the
/// import runs fails to open rather than leading somewhere else.
#[cfg(unix)]
fn import_into(allowed: &Allowed, from: &Path, folder: &Path, config: &Path) -> io::Result<Copied> {
    use rustix::fs::{Mode, OFlags};
    let mut copied = Copied::default();
    let Ok(source) = from.canonicalize() else {
        return Ok(copied);
    };
    let root = match rustix::fs::open(
        &source,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR) => return Ok(copied),
        Err(e) => return Err(e.into()),
    };
    let dest = settings_dest(allowed, config);
    crate::contain::no_link_on_the_way(folder, &dest)?;
    private_dirs_within(folder, &dest)?;
    let mut place = Place {
        into: Sink::Disk { folder },
        copied: &mut copied,
    };
    for name in allowed.files {
        entry(
            &mut place,
            std::os::fd::AsFd::as_fd(&root),
            name,
            &dest.join(name),
            name,
            false,
            0,
        )?;
    }
    for name in allowed.folders {
        entry(
            &mut place,
            std::os::fd::AsFd::as_fd(&root),
            name,
            &dest.join(name),
            name,
            true,
            0,
        )?;
    }
    Ok(copied)
}

/// No import where there is no descriptor-relative open to read the settings with safely.
#[cfg(not(unix))]
fn import_into(
    _allowed: &Allowed,
    _from: &Path,
    _folder: &Path,
    _config: &Path,
) -> io::Result<Copied> {
    Ok(Copied {
        copied: Vec::new(),
        skipped: vec!["every settings file: an import needs a Unix system".to_owned()],
    })
}

/// Where an import writes, and what it has done so far.
#[cfg(unix)]
struct Place<'a> {
    into: Sink<'a>,
    copied: &'a mut Copied,
}

/// Where what an import reads goes: into a profile folder, or kept in memory to be sent to a
/// runner ([`settings_read`]).
#[cfg(unix)]
enum Sink<'a> {
    /// The profile folder written into, which no link on the way leads out of.
    Disk { folder: &'a Path },
    /// Each file read, by its path under the settings folder, and the bytes read so far in all.
    Memory {
        files: &'a mut Vec<(String, Vec<u8>)>,
        bytes: usize,
        most: usize,
        most_files: usize,
        /// Each file past `most` or `most_files`, which stays behind, and its size.
        left_behind: &'a mut Vec<(String, u64)>,
        /// Whether a file is read at all.
        wanted: &'a dyn Fn(&str) -> bool,
    },
    /// What each whole holds, read from no file ([`settings_measured`]).
    Measure {
        wholes: &'a mut Measured,
        files: usize,
    },
}

/// Each whole a settings folder holds ([`whole_of`]), by its path: its bytes and its files.
pub type Measured = std::collections::BTreeMap<String, (u64, usize)>;

/// The whole a settings file goes or stays with: a listed file, or the first thing in a listed
/// folder (`skills/review/SKILL.md` goes with `skills/review`), since half a skill is no skill.
pub fn whole_of(path: &str) -> &str {
    let mut parts = path.match_indices('/');
    match (parts.next(), parts.next()) {
        (Some(_), Some((second, _))) => &path[..second],
        _ => path,
    }
}

/// Copies the entry `name` of the folder `dir` holds: a file, or (where `folder_ok`) a folder's
/// files. Opened relative to `dir` without following a link; a file with another name on the
/// disk (a hard link) is never copied.
#[cfg(unix)]
fn entry(
    place: &mut Place<'_>,
    dir: std::os::fd::BorrowedFd<'_>,
    name: &str,
    to: &Path,
    shown: &str,
    folder_ok: bool,
    depth: usize,
) -> io::Result<()> {
    use rustix::fs::{FileType, Mode, OFlags};
    use rustix::io::Errno;
    let enough = match &place.into {
        Sink::Disk { .. } => false,
        Sink::Memory { left_behind, .. } => left_behind.len() >= MOST_LEFT_BEHIND,
        Sink::Measure { files, .. } => *files >= MOST_MEASURED,
    };
    if enough {
        return Ok(());
    }
    let fd = match rustix::fs::openat(
        dir,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(Errno::NOENT) => return Ok(()),
        Err(Errno::LOOP | Errno::MLINK) => {
            place
                .copied
                .skipped
                .push(format!("{shown}: a link, which an import never follows"));
            return Ok(());
        }
        Err(e) => {
            place.copied.skipped.push(format!(
                "{shown}: not read ({})",
                crate::rewrite::os_words(&e.into())
            ));
            return Ok(());
        }
    };
    let stat = rustix::fs::fstat(&fd)?;
    let kind = FileType::from_raw_mode(stat.st_mode);
    if kind == FileType::RegularFile {
        if stat.st_nlink > 1 {
            place
                .copied
                .skipped
                .push(format!("{shown}: a file with another name on this disk"));
            return Ok(());
        }
        let mut bytes = Vec::new();
        match &mut place.into {
            Sink::Disk { folder } => {
                std::io::Read::read_to_end(&mut std::fs::File::from(fd), &mut bytes)?;
                crate::rewrite::replace(folder, to, &bytes, crate::rewrite::Mode::Private)?;
            }
            Sink::Memory {
                files,
                bytes: so_far,
                most,
                most_files,
                left_behind,
                wanted,
            } => {
                if !wanted(shown) {
                    return Ok(());
                }
                // Read no more than is left, and one byte past it to tell a file that is over.
                let left = most.saturating_sub(*so_far);
                let size = u64::try_from(stat.st_size).unwrap_or(0);
                if size > left as u64 || files.len() >= *most_files {
                    left_behind.push((shown.to_owned(), size));
                    return Ok(());
                }
                std::io::Read::read_to_end(
                    &mut std::io::Read::take(std::fs::File::from(fd), left as u64 + 1),
                    &mut bytes,
                )?;
                if bytes.len() > left {
                    left_behind.push((shown.to_owned(), bytes.len() as u64));
                    return Ok(());
                }
                *so_far += bytes.len();
                files.push((shown.to_owned(), bytes));
            }
            Sink::Measure { wholes, files } => {
                let whole = wholes.entry(whole_of(shown).to_owned()).or_default();
                whole.0 += u64::try_from(stat.st_size).unwrap_or(0);
                whole.1 += 1;
                *files += 1;
                return Ok(());
            }
        }
        place.copied.copied.push(shown.to_owned());
        return Ok(());
    }
    if kind != FileType::Directory || !folder_ok {
        place
            .copied
            .skipped
            .push(format!("{shown}: not a settings file"));
        return Ok(());
    }
    if depth >= DEEPEST {
        place
            .copied
            .skipped
            .push(format!("{shown}: more than {DEEPEST} folders deep"));
        return Ok(());
    }
    if let Sink::Disk { folder } = &place.into {
        private_dir_within(folder, to)?;
    }
    // What a runner has no use for goes no further, as a chat's start pays for all it sends.
    let to_a_runner = !matches!(place.into, Sink::Disk { .. });
    let leaves_out = |name: &str| to_a_runner && NOT_SENT.contains(&name);
    let mut names = Vec::new();
    let mut listing = rustix::fs::Dir::read_from(&fd)?;
    while let Some(one) = listing.read() {
        let one = one?;
        let Ok(text) = one.file_name().to_str() else {
            continue;
        };
        if text != "." && text != ".." {
            names.push(text.to_owned());
        }
    }
    names.sort();
    for text in names {
        let inner = format!("{shown}/{text}");
        if text.starts_with('.') || credential_shaped(&text) {
            place
                .copied
                .skipped
                .push(format!("{inner}: not imported, by its name"));
            continue;
        }
        if leaves_out(&text) {
            place
                .copied
                .skipped
                .push(format!("{inner}: not sent to a runner, by its name"));
            continue;
        }
        entry(
            place,
            std::os::fd::AsFd::as_fd(&fd),
            &text,
            &to.join(&text),
            &inner,
            true,
            depth + 1,
        )?;
    }
    Ok(())
}

/// Folders of a skill no runner has a use for (#4081), built for this machine and made again
/// where they are needed. A hidden one (`.git`, `.venv`, `.cache`) is left out by its name
/// already.
#[cfg(unix)]
const NOT_SENT: [&str; 3] = ["node_modules", "__pycache__", "venv"];

/// The most files a read for a runner lists as left behind, and the most it measures, so a tree
/// of millions costs no more.
#[cfg(unix)]
const MOST_LEFT_BEHIND: usize = 16 * 1024;
#[cfg(unix)]
const MOST_MEASURED: usize = 1024 * 1024;

/// **What [`settings_read`] found**: the files read, and those past its limits, unread, each
/// with its size.
#[derive(Debug, Default)]
pub struct SettingsRead {
    pub files: Vec<(String, Vec<u8>)>,
    pub left_behind: Vec<(String, u64)>,
}

/// **The settings files `kind`'s settings folder `config` holds, as an import reads them**
/// (#3448): each listed file and each file of a listed folder, by its path under the folder
/// (`agents/review.md`), read by descriptor with no link followed, and none hidden or named like
/// a credential, exactly as [`create`] copies them. Only those `wanted` says, at most `most`
/// bytes in `most_files` files, in that order: a file past either is left behind, unread (#4081).
/// Empty where the folder is not there.
#[cfg(unix)]
pub fn settings_read(
    kind: &str,
    config: &Path,
    most: usize,
    most_files: usize,
    wanted: &dyn Fn(&str) -> bool,
) -> Result<SettingsRead, String> {
    let mut files = Vec::new();
    let mut left_behind = Vec::new();
    settings_walked(
        kind,
        config,
        Sink::Memory {
            files: &mut files,
            bytes: 0,
            most,
            most_files,
            left_behind: &mut left_behind,
            wanted,
        },
    )?;
    Ok(SettingsRead { files, left_behind })
}

/// **What [`settings_read`] would find, by whole, with no file read** (#4081), so what stays
/// behind is chosen before anything is: a large skill costs the ones after it nothing.
#[cfg(unix)]
pub fn settings_measured(kind: &str, config: &Path) -> Result<Measured, String> {
    let mut wholes = Measured::new();
    settings_walked(
        kind,
        config,
        Sink::Measure {
            wholes: &mut wholes,
            files: 0,
        },
    )?;
    Ok(wholes)
}

/// Walks `kind`'s settings folder `config` as an import does, into `into`.
#[cfg(unix)]
fn settings_walked(kind: &str, config: &Path, into: Sink<'_>) -> Result<(), String> {
    use rustix::fs::{Mode, OFlags};
    let (_, allowed) = harness_of(kind)?;
    let from = settings_dest(allowed, config);
    let mut copied = Copied::default();
    let telling = matches!(into, Sink::Memory { .. });
    let read = (|| {
        let Ok(source) = from.canonicalize() else {
            return Ok(());
        };
        let root = match rustix::fs::open(
            &source,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR) => return Ok(()),
            Err(e) => return Err(io::Error::from(e)),
        };
        let mut place = Place {
            into,
            copied: &mut copied,
        };
        for (name, folder_ok) in allowed
            .files
            .iter()
            .map(|n| (n, false))
            .chain(allowed.folders.iter().map(|n| (n, true)))
        {
            entry(
                &mut place,
                std::os::fd::AsFd::as_fd(&root),
                name,
                Path::new(name),
                name,
                folder_ok,
                0,
            )?;
        }
        Ok(())
    })();
    // What was left out is said in the log, so a setting missing on a runner can be told apart
    // from one never written.
    if telling {
        for why in &copied.skipped {
            tracing::info!("purlis: a setting is not sent to a runner: {why}");
        }
    }
    read.map_err(|e| {
        format!(
            "its settings in {} were not read ({})",
            from.display(),
            crate::rewrite::os_words(&e)
        )
    })
}

/// No settings are read where there is no descriptor-relative open to read them with safely.
#[cfg(not(unix))]
pub fn settings_read(
    _kind: &str,
    _config: &Path,
    _most: usize,
    _most_files: usize,
    _wanted: &dyn Fn(&str) -> bool,
) -> Result<SettingsRead, String> {
    Err("its settings are read only on a Unix system".to_owned())
}

/// No settings are read where there is no descriptor-relative open to read them with safely.
#[cfg(not(unix))]
pub fn settings_measured(_kind: &str, _config: &Path) -> Result<Measured, String> {
    Err("its settings are read only on a Unix system".to_owned())
}

/// **Whether `path` is one an import of `allowed` would write**: a listed file, or a file inside
/// a listed folder at most [`DEEPEST`] folders down, `/`-separated, with no empty, `.` or `..`
/// part, and none below the listed folder hidden or named like a credential.
fn a_settings_path(allowed: &Allowed, path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    let Some((first, rest)) = parts.split_first() else {
        return false;
    };
    let each_ok = parts.iter().all(|part| {
        !part.is_empty() && *part != "." && *part != ".." && !part.contains(['\\', '\0'])
    });
    if !each_ok {
        return false;
    }
    if rest.is_empty() {
        return allowed.files.contains(first);
    }
    allowed.folders.contains(first)
        && rest.len() <= DEEPEST
        && rest
            .iter()
            .all(|part| !part.starts_with('.') && !credential_shaped(part))
}

/// **`held`, what a runner said it holds of a `kind` profile, by path**, or `None` where it is
/// not such a list: a path an import would not write, one twice, a digest no digest looks like,
/// or more than a runner holds (#4088).
pub fn held_digests<'a>(
    kind: &str,
    held: &'a [purlis_session_protocol::session::HeldSetting],
) -> Option<std::collections::BTreeMap<&'a str, &'a str>> {
    use purlis_session_protocol::session::{MOST_PROFILE_SETTINGS_FILES, is_settings_digest};
    let (_, allowed) = harness_of(kind).ok()?;
    if held.len() > MOST_PROFILE_SETTINGS_FILES {
        return None;
    }
    let mut digests = std::collections::BTreeMap::new();
    for one in held {
        let fits = a_settings_path(allowed, &one.path) && is_settings_digest(&one.digest);
        if !fits
            || digests
                .insert(one.path.as_str(), one.digest.as_str())
                .is_some()
        {
            return None;
        }
    }
    Some(digests)
}

/// **`files`, sent by a desktop for its `kind` profile `name`, as a runner may keep them**:
/// every path one an import would write ([`a_settings_path`]), none twice, and each file with no
/// sign-in in it ([`super::sent::without_credentials`], D-3448-8). One that is not refuses all
/// of them, in words. What a runner checks before it keeps anything of the profile.
pub fn settings_checked(
    kind: &str,
    name: &str,
    files: &[(String, Vec<u8>)],
) -> Result<Vec<(String, Vec<u8>)>, String> {
    let name = crate::shown::short(name);
    let (_, allowed) = harness_of(kind)?;
    let mut seen = std::collections::HashSet::new();
    if let Some((bad, _)) = files
        .iter()
        .find(|(path, _)| !a_settings_path(allowed, path) || !seen.insert(path.as_str()))
    {
        return Err(format!(
            "'{}' is not a settings file of the profile '{name}' this runner keeps, so it kept \
             none of the settings sent",
            crate::shown::short(bad)
        ));
    }
    // Whatever sent them, no sign-in is kept in them here (D-3448-8).
    files
        .iter()
        .map(|(path, bytes)| {
            super::sent::without_credentials(kind, path, bytes).map(|b| (path.clone(), b))
        })
        .collect()
}

/// The folder settings are staged in, in the folder of the profiles purlis makes: no chat may
/// write there, it is on the runner's home volume with the settings it replaces, and no profile
/// can have the name ([`crate::profiles::name_ok`]).
#[cfg(unix)]
const STAGED_IN: &str = ".settings-staged";

/// One keep of settings at a time in this process, so one's stage is never another's debris: two
/// chats starting at once each keep the same profile (#4081).
#[cfg(unix)]
static SWAPPING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// **`files` swapped in for `allowed`'s settings in the settings folder `dest`** (#4081): written
/// whole into a stage of their own, where no chat may write ([`STAGED_IN`], never `dest`, which
/// can be a folder a chat writes in), and only then each listed name moved aside into it and its
/// new one moved in, each move put back if a later one fails. Everything from `dest` down goes
/// through descriptors, from one of `dest` opened with no link followed. Nothing is flushed: the
/// next chat's start writes them all again. What a killed keep left is swept first; the settings
/// it left incomplete are replaced whole by the first keep that goes through.
#[cfg(unix)]
fn settings_swapped_in(
    homes: &Homes,
    allowed: &Allowed,
    dest: &Path,
    files: &[(String, Vec<u8>)],
) -> io::Result<()> {
    use rustix::fs::{Mode, OFlags};
    let _one_at_a_time = SWAPPING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let stages = homes.folders.join(STAGED_IN);
    // Its name is new each time, so what cannot be swept stands in no keep's way.
    let _ = std::fs::remove_dir_all(&stages);
    private_dirs(&stages)?;
    let stage_at = stages.join(format!(
        "{}.{}",
        std::process::id(),
        crate::workspaces::scratch_tag()
    ));
    let opened = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let dir = rustix::fs::open(dest, opened, Mode::empty())?;
    let parent = rustix::fs::open(&stages, opened, Mode::empty())?;
    let apart = || {
        io::Error::other(format!(
            "{} and {} are on different disks, and a keep moves one into the other",
            dest.display(),
            stages.display()
        ))
    };
    if rustix::fs::fstat(&dir)?.st_dev != rustix::fs::fstat(&parent)?.st_dev {
        return Err(apart());
    }
    let name = stage_at.file_name().unwrap_or_default();
    rustix::fs::mkdirat(&parent, name, Mode::RWXU)?;
    let mut lost = false;
    let swapped = (|| {
        let stage = rustix::fs::openat(&parent, name, opened, Mode::empty())?;
        rustix::fs::mkdirat(&stage, "new", Mode::RWXU)?;
        rustix::fs::mkdirat(&stage, "old", Mode::RWXU)?;
        let new = rustix::fs::openat(&stage, "new", opened, Mode::empty())?;
        let old = rustix::fs::openat(&stage, "old", opened, Mode::empty())?;
        for (path, bytes) in files {
            write_staged(std::os::fd::AsFd::as_fd(&new), path, bytes)?;
        }
        swap_each(allowed, &dir, &new, &old, &mut lost)
    })();
    // A move not put back leaves what it moved in the stage, and the settings incomplete until a
    // keep that goes through replaces them all.
    if lost {
        return Err(io::Error::other(format!(
            "some of the settings could not be put back, and what was moved is in {}",
            stage_at.display()
        )));
    }
    let _ = std::fs::remove_dir_all(&stage_at);
    swapped.map_err(|e| {
        if e.raw_os_error() == Some(rustix::io::Errno::XDEV.raw_os_error()) {
            apart()
        } else {
            e
        }
    })
}

/// **Whether `record`'s settings folder holds `files` and nothing else its keep would replace**
/// (#4088): under each listed name, only a private folder that leads to one of them, and each of
/// them a private regular file of its size with no other name, reached through no link. Anything
/// else there, or anything that cannot be told, is `false`, so a keep replaces them all as ever:
/// what a read of the settings passes over (a hidden file, a link, a skill's `node_modules`) is
/// still taken out, and a second name for a file, which could change it after this, is cut off.
/// Their bytes were read a moment before, and come to the digest the keep names.
#[cfg(unix)]
pub fn settings_exactly_there(record: &MachineProfile, files: &[(String, Vec<u8>)]) -> bool {
    use rustix::fs::{Mode, OFlags};
    let Ok((_, allowed)) = harness_of(&record.harness) else {
        return false;
    };
    let dest = settings_dest(allowed, &record.folders.config);
    let opened = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let Ok(dir) = rustix::fs::open(&dest, opened, Mode::empty()) else {
        return false;
    };
    let sizes: std::collections::HashMap<&str, usize> = files
        .iter()
        .map(|(path, bytes)| (path.as_str(), bytes.len()))
        .collect();
    let mut folders = std::collections::HashSet::new();
    for path in sizes.keys() {
        let mut at = *path;
        while let Some((folder, _)) = at.rsplit_once('/') {
            folders.insert(folder);
            at = folder;
        }
    }
    let mut found = 0;
    allowed.files.iter().chain(allowed.folders).all(|name| {
        only_these(
            std::os::fd::AsFd::as_fd(&dir),
            name,
            name,
            &sizes,
            &folders,
            &mut found,
        )
        .unwrap_or(false)
    }) && found == sizes.len()
}

/// Whether the `name` of the folder `at`, at the path `path`, is only what [`settings_exactly_there`]
/// allows, counting into `found` each file of `sizes` it met. A name not there is nothing.
#[cfg(unix)]
fn only_these(
    at: std::os::fd::BorrowedFd<'_>,
    name: &str,
    path: &str,
    sizes: &std::collections::HashMap<&str, usize>,
    folders: &std::collections::HashSet<&str>,
    found: &mut usize,
) -> io::Result<bool> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags};
    let stat = match rustix::fs::statat(at, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(rustix::io::Errno::NOENT) => return Ok(true),
        Err(e) => return Err(e.into()),
    };
    let mode = Mode::from_raw_mode(stat.st_mode);
    match FileType::from_raw_mode(stat.st_mode) {
        FileType::RegularFile => {
            *found += 1;
            Ok(mode == Mode::RUSR | Mode::WUSR
                && stat.st_nlink == 1
                && sizes.get(path).is_some_and(|size| {
                    u64::try_from(stat.st_size).is_ok_and(|on_disk| on_disk == *size as u64)
                }))
        }
        FileType::Directory if mode == Mode::RWXU && folders.contains(path) => {
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
            let fd = rustix::fs::openat(at, name, flags, Mode::empty())?;
            let mut names = Vec::new();
            let mut listing = rustix::fs::Dir::read_from(&fd)?;
            while let Some(one) = listing.read() {
                let one = one?;
                let Ok(text) = one.file_name().to_str() else {
                    return Ok(false);
                };
                if text != "." && text != ".." {
                    names.push(text.to_owned());
                }
            }
            for inner in names {
                let below = format!("{path}/{inner}");
                let fd = std::os::fd::AsFd::as_fd(&fd);
                if !only_these(fd, &inner, &below, sizes, folders, found)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Nothing is ever there where no settings are kept.
#[cfg(not(unix))]
pub fn settings_exactly_there(_record: &MachineProfile, _files: &[(String, Vec<u8>)]) -> bool {
    false
}

/// `path`, `/`-separated, written new under the folder `root`, private, through no link.
#[cfg(unix)]
fn write_staged(root: std::os::fd::BorrowedFd<'_>, path: &str, bytes: &[u8]) -> io::Result<()> {
    use rustix::fs::{Mode, OFlags};
    use std::os::fd::AsFd;
    let (name, folders): (&str, Vec<&str>) = match path.rsplit_once('/') {
        Some((folders, name)) => (name, folders.split('/').collect()),
        None => (path, Vec::new()),
    };
    let mut held = None;
    for folder in folders {
        let at = held
            .as_ref()
            .map_or(root, |fd: &std::os::fd::OwnedFd| fd.as_fd());
        match rustix::fs::mkdirat(at, folder, Mode::RWXU) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(e) => return Err(e.into()),
        }
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let next = rustix::fs::openat(at, folder, flags, Mode::empty())?;
        held = Some(next);
    }
    let at = held.as_ref().map_or(root, |fd| fd.as_fd());
    let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = rustix::fs::openat(at, name, flags, Mode::RUSR | Mode::WUSR)?;
    std::io::Write::write_all(&mut std::fs::File::from(fd), bytes)
}

/// Moves each of `allowed`'s names of `dir` into `old`, and its new one, where there is one, out
/// of `new`. A move that fails puts back every one done, and says in `lost` where one could not
/// be.
#[cfg(unix)]
fn swap_each(
    allowed: &Allowed,
    dir: &std::os::fd::OwnedFd,
    new: &std::os::fd::OwnedFd,
    old: &std::os::fd::OwnedFd,
    lost: &mut bool,
) -> io::Result<()> {
    use rustix::fs::{AtFlags, renameat, statat};
    use rustix::io::Errno;
    /// Whether `name` is there, a link or not, so that only a name that is not there is passed
    /// over, and a move that fails for any other reason is a failure.
    fn there(at: &std::os::fd::OwnedFd, name: &str) -> rustix::io::Result<bool> {
        match statat(at, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(_) => Ok(true),
            Err(Errno::NOENT) => Ok(false),
            Err(e) => Err(e),
        }
    }
    let mut done = Vec::new();
    let swapped = allowed
        .files
        .iter()
        .chain(allowed.folders)
        .try_for_each(|one| {
            let had = there(dir, one)?;
            if had {
                renameat(dir, *one, old, *one)?;
            }
            done.push((*one, had));
            if there(new, one)? {
                renameat(new, *one, dir, *one)?;
            }
            Ok::<_, Errno>(())
        });
    if swapped.is_err() {
        for (one, had) in done.into_iter().rev() {
            let back = there(dir, one)
                .and_then(|placed| {
                    if placed && !there(new, one)? {
                        renameat(dir, one, new, one)?;
                    }
                    Ok(())
                })
                .and_then(|()| {
                    if had {
                        renameat(old, one, dir, one)
                    } else {
                        Ok(())
                    }
                });
            *lost |= back.is_err();
        }
    }
    swapped.map_err(Into::into)
}

/// No settings are kept where there is no descriptor-relative open to keep them with safely.
#[cfg(not(unix))]
fn settings_swapped_in(
    _homes: &Homes,
    _allowed: &Allowed,
    _dest: &Path,
    _files: &[(String, Vec<u8>)],
) -> io::Result<()> {
    Err(io::Error::other("settings are kept only on a Unix system"))
}

/// **Keeps `files`, sent by a desktop, as the settings of `record`, a profile this runner keeps
/// for it** (#3448), in place of the ones it kept: `files` are written, private, through no
/// link, apart from the settings they replace, and only once they are all written does each listed
/// file and folder go ([`settings_swapped_in`], #4081), so a keep that fails leaves the settings
/// as they were, or says that it could not. Only a made profile's own folder ([`own_folder`]) or an adopted record's
/// settings folder is written. Every path is checked first ([`a_settings_path`]): one an import
/// would not write refuses all of them, and nothing is written.
pub fn settings_kept(
    homes: &Homes,
    record: &MachineProfile,
    files: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let name = crate::shown::short(&record.name);
    let (_, allowed) = harness_of(&record.harness)?;
    if record.sandbox_only {
        return Err(format!(
            "the profile '{name}' is for sandboxed chats only, and its settings are purlis's, so \
             this runner keeps no settings sent for it"
        ));
    }
    let checked = settings_checked(&record.harness, &record.name, files)?;
    let dest = settings_dest(allowed, &record.folders.config);
    // A made profile is written within its own folder; an adopted one within its settings
    // folder, which is made where it is missing and never a link.
    let folder = match record.adopted {
        None => own_folder(homes, record)?,
        Some(_) => dest.clone(),
    };
    let written = (|| {
        if record.adopted.is_some() {
            if let Some(parent) = dest.parent() {
                private_dirs(parent)?;
            }
            private_dir(&dest)?;
            if !std::fs::symlink_metadata(&dest)?.is_dir() {
                return Err(io::Error::other("the settings folder is not a folder"));
            }
        }
        crate::contain::no_link_on_the_way(&folder, &dest)?;
        private_dirs_within(&folder, &dest)?;
        settings_swapped_in(homes, allowed, &dest, &checked)
    })();
    written.map_err(|e| {
        format!(
            "this runner could not keep the profile '{name}''s settings in {} ({})",
            dest.display(),
            crate::rewrite::os_words(&e)
        )
    })
}

/// A file name that may hold a sign-in, which an import never copies from a settings folder.
fn credential_shaped(name: &str) -> bool {
    let lower = name.to_lowercase();
    crate::profiles::named_like_a_credential(name)
        || ["auth", "credential", "cookie", "session", "passw"]
            .iter()
            .any(|word| lower.contains(word))
}

/// Removes `path`, a file, a link or a folder, inside `folder` and reached through no link.
fn remove_within(folder: &Path, path: &Path) -> io::Result<()> {
    crate::contain::no_link_on_the_way(folder, path.parent().unwrap_or(folder))?;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The conversations in a made profile's `folder`, as its harness's declaration says where it
/// keeps them, or `None` where it keeps them in a way that is no file purlis can list.
fn conversations(record: &MachineProfile, folder: &Path) -> Option<Vec<String>> {
    let declared = crate::harness_declaration::builtin(&record.harness)?;
    let Some(crate::harness_declaration::Conversation::File { file, .. }) = &declared.conversation
    else {
        return None;
    };
    let (base, rest) = if let Some(rest) = file.strip_prefix("{config}/") {
        (record.folders.config.clone(), rest)
    } else {
        (record.folders.data.clone()?, file.strip_prefix("{data}/")?)
    };
    let parts: Vec<&str> = rest.split('/').collect();
    let mut found = Vec::new();
    matching(&base, &parts, &mut found);
    let mut found: Vec<String> = found
        .into_iter()
        .filter_map(|path| {
            path.strip_prefix(folder)
                .ok()
                .map(|p| p.display().to_string())
        })
        .collect();
    found.sort();
    Some(found)
}

/// Every path under `dir` whose parts match `parts`, a placeholder or a `*` matching any text.
fn matching(dir: &Path, parts: &[&str], found: &mut Vec<PathBuf>) {
    let Some((first, rest)) = parts.split_first() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for one in entries.filter_map(Result::ok) {
        let Some(name) = one.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !wildcard(first, &name) {
            continue;
        }
        let Ok(kind) = one.file_type() else {
            continue;
        };
        if rest.is_empty() {
            if kind.is_file() {
                found.push(one.path());
            }
        } else if kind.is_dir() {
            matching(&one.path(), rest, found);
        }
    }
}

/// Whether `name` matches `pattern`, where `*` and every `{placeholder}` match any text.
fn wildcard(pattern: &str, name: &str) -> bool {
    let mut pieces = Vec::new();
    let mut literal = String::new();
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '*' => pieces.push(std::mem::take(&mut literal)),
            '{' => {
                for inner in chars.by_ref() {
                    if inner == '}' {
                        break;
                    }
                }
                pieces.push(std::mem::take(&mut literal));
            }
            c => literal.push(c),
        }
    }
    pieces.push(literal);
    let (Some(first), Some(last)) = (pieces.first(), pieces.last()) else {
        return false;
    };
    if pieces.len() == 1 {
        return name == first;
    }
    if !name.starts_with(first.as_str())
        || !name.ends_with(last.as_str())
        || name.len() < first.len() + last.len()
    {
        return false;
    }
    let mut at = first.len();
    for piece in &pieces[1..pieces.len() - 1] {
        match name[at..name.len() - last.len()].find(piece.as_str()) {
            Some(found) => at += found + piece.len(),
            None => return false,
        }
    }
    name[at..].ends_with(last.as_str())
}

/// Makes `dir` and every folder above it that is missing, each 0700.
fn private_dirs(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    if let Some(parent) = dir.parent() {
        private_dirs(parent)?;
    }
    private_dir(dir)
}

/// Makes `dir`, 0700, where it is not there.
fn private_dir(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && dir.is_dir() => Ok(()),
        Err(e) => Err(e),
    }
}

/// [`private_dir`], for a folder inside the profile folder `folder`, reached through no link.
fn private_dir_within(folder: &Path, dir: &Path) -> io::Result<()> {
    crate::contain::no_link_on_the_way(folder, dir)?;
    private_dir(dir)
}

/// [`private_dirs`], for a folder inside the profile folder `folder`, reached through no link.
fn private_dirs_within(folder: &Path, dir: &Path) -> io::Result<()> {
    crate::contain::no_link_on_the_way(folder, dir)?;
    private_dirs(dir)
}

fn no_such(name: &str) -> String {
    format!(
        "this machine has no profile named '{}'. purlis profile list names the ones it has.",
        crate::shown::short(name)
    )
}

fn could_not(homes: &Homes, e: &io::Error) -> String {
    format!(
        "purlis could not change this machine's profiles in {} ({}), so nothing was changed.",
        homes.folders.display(),
        crate::rewrite::os_words(e)
    )
}

#[cfg(test)]
mod tests;
