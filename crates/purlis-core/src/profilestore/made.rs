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
}

impl Homes {
    /// The config home `config_root` and the data home `data`.
    pub fn at(config_root: &Path, data: &Path) -> Self {
        Self {
            config_root: config_root.to_path_buf(),
            folders: data.join(DIR),
        }
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
    let (folders, allowed) = harness_of(kind)?;
    let folder = homes.folder_of(name);
    let mut made_folder = false;
    let answer = super::update(&homes.config_root, |store| {
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
            made_folder = true;
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
            let from = machine.of(kind);
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
            rest: Map::new(),
        };
        store.profiles.push(record.clone());
        Ok(Made { record, copied })
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

/// Where `allowed`'s settings go in a profile whose `config_env` is `config`: under the folder
/// of its own name, for a harness that keeps one.
fn settings_dest(allowed: &Allowed, config: &Path) -> PathBuf {
    let own = super::defaults(allowed.kind, Path::new("")).map_or("", |(_, _, own)| own);
    own_under(config.to_path_buf(), own)
}

/// Copies `allowed`'s settings from `from` into the profile folder `folder`, whose
/// `config_env` is `config`. A missing settings folder imports nothing.
fn import_into(allowed: &Allowed, from: &Path, folder: &Path, config: &Path) -> io::Result<Copied> {
    let mut copied = Copied::default();
    let Ok(source) = from.canonicalize() else {
        return Ok(copied);
    };
    let dest = settings_dest(allowed, config);
    crate::contain::no_link_on_the_way(folder, &dest)?;
    private_dirs_within(folder, &dest)?;
    for name in allowed.files {
        entry(
            folder,
            &source.join(name),
            &dest.join(name),
            name,
            false,
            0,
            &mut copied,
        )?;
    }
    for name in allowed.folders {
        entry(
            folder,
            &source.join(name),
            &dest.join(name),
            name,
            true,
            0,
            &mut copied,
        )?;
    }
    Ok(copied)
}

/// Copies one entry of the settings: a file, or (where `folder_ok`) a folder's files. A link
/// is never followed, and a file that has another name on the disk (a hard link) is never
/// copied, so what is copied is only ever a file of the name the list or the folder gives.
fn entry(
    folder: &Path,
    from: &Path,
    to: &Path,
    shown: &str,
    folder_ok: bool,
    depth: usize,
    copied: &mut Copied,
) -> io::Result<()> {
    let meta = match std::fs::symlink_metadata(from) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    if meta.file_type().is_symlink() {
        copied
            .skipped
            .push(format!("{shown}: a link, which an import never follows"));
        return Ok(());
    }
    if meta.is_file() {
        // Opened without following a link, and judged by what was opened: a name swapped for
        // a link since the look above is refused by the open itself.
        let mut file =
            match crate::contain::nofollow(std::fs::OpenOptions::new().read(true)).open(from) {
                Ok(file) => file,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(_) => {
                    copied
                        .skipped
                        .push(format!("{shown}: a link, which an import never follows"));
                    return Ok(());
                }
            };
        let opened = file.metadata()?;
        if !opened.is_file() {
            copied.skipped.push(format!("{shown}: not a settings file"));
            return Ok(());
        }
        if other_names(&opened) {
            copied
                .skipped
                .push(format!("{shown}: a file with another name on this disk"));
            return Ok(());
        }
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut bytes)?;
        crate::rewrite::replace(folder, to, &bytes, crate::rewrite::Mode::Private)?;
        copied.copied.push(shown.to_owned());
        return Ok(());
    }
    if !meta.is_dir() || !folder_ok {
        copied.skipped.push(format!("{shown}: not a settings file"));
        return Ok(());
    }
    if depth >= DEEPEST {
        copied
            .skipped
            .push(format!("{shown}: more than {DEEPEST} folders deep"));
        return Ok(());
    }
    private_dir_within(folder, to)?;
    let mut names: Vec<_> = std::fs::read_dir(from)?
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    names.sort();
    for name in names {
        let Some(text) = name.to_str() else {
            continue;
        };
        let inner = format!("{shown}/{text}");
        if text.starts_with('.') || credential_shaped(text) {
            copied
                .skipped
                .push(format!("{inner}: not imported, by its name"));
            continue;
        }
        entry(
            folder,
            &from.join(text),
            &to.join(text),
            &inner,
            true,
            depth + 1,
            copied,
        )?;
    }
    Ok(())
}

/// Whether the file `meta` describes has another name on its disk: a hard link, which may be a
/// sign-in under a settings file's name.
fn other_names(meta: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.nlink() > 1
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
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
