// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The package caches of a project's sandboxed chats (spec #1330's coupled widening, #1337):
//! with the toolchains preset on, `cargo build` and `npm install` download into caches of the
//! project's own instead of stopping with "Operation not permitted".
//!
//! **A cache home of the project's own, never the person's caches** (D-1337-6). Every tool
//! uses a cached package without checking it again somewhere: cargo an extracted
//! `registry/src` and a cached `.crate`, Go an extracted module tree, Gradle its script and
//! plugin class caches, pnpm its store index. So a cache a chat writes is code a later build
//! runs, and a shared one would be code the person's own, unsandboxed builds run, in every
//! project (ADR 0067 §5). Each project's sandboxed chats get their own caches instead, in a
//! folder under purlis's data home ([`home_of`]), and each tool is pointed at it through the
//! chat's environment ([`TOOLS`]). Only that project's chats, all sandboxed, use them. The
//! person's own caches are read, never written, and no unsandboxed program is pointed at
//! these. The cost is disk, and one first download per project.
//!
//! **cargo's home is seeded, never shared.** `CARGO_HOME` moves cargo's config with it, so
//! purlis writes the new home's `config.toml` at every start from the person's own, keeping
//! only where crates come from: the `[registries]` indexes, `registry.default` and the
//! `[source]` replacements ([`cargo_config`]). Never a token, a credential provider, a build
//! setting or an alias, and never a `bin` folder. A private registry that needs a login is not
//! reached from a sandboxed chat. A relative `[source]` folder is resolved where cargo resolves
//! it for the person's file, and kept only when it stays inside that folder, links followed
//! (#1364). Its text names a folder; what a chat may read there is still the sandbox's to say.
//! Of `[http]` and `[net]` it keeps the certificate file, the revocation check, fetching git
//! with the git program, and offline ([`NETWORK_KEYS`]). Never the person's proxy: a sandboxed
//! chat reaches the network only through purlis's own, which cargo finds in `HTTPS_PROXY`, and
//! the written file says the proxy was left out.
//!
//! **Gradle's home is seeded the same way** (#1364). `GRADLE_USER_HOME` moves Gradle's
//! `gradle.properties` and init scripts with it, so purlis writes the new home's
//! `gradle.properties` at every start from the person's own, keeping only how Gradle builds
//! and caches ([`gradle_properties`]): never a JVM option, a Java home, a project property
//! (where credentials live) or a proxy. The person's init scripts are programs and are not
//! copied. Beside it purlis writes [`GRADLE_INIT`], which points Java's proxy settings at the
//! chat's own proxy, since Java does not read `HTTPS_PROXY`. Both are written at the top of
//! the Gradle folder, which is pinned, and a chat may rewrite either until the next start: the
//! folder is one only the project's sandboxed chats use.
//!
//! **What a chat writes there** ([`CacheHome`]): each tool's folder, whole; and of cargo's
//! home only its registry, its git checkouts, what is inside each bare repository of its git
//! database, and the lock and tracking files cargo keeps beside them. Never cargo's config,
//! credentials or `bin`, a bare repository's `config` or `hooks`, and never a folder purlis
//! made: each is pinned, so none is swapped for a link.
//!
//! **A home outlives nothing it was made for** (#1364): each start writes which project the
//! home is beside its folders ([`crate::leftovers::HOME_NOTE`]), and the homes of a project
//! that is gone, cache and Codex both, are removed once nothing has used them for a month
//! ([`crate::leftovers::sweep_homes`]).

use std::path::{Path, PathBuf};

use super::{Denied, Machine};

/// One tool's folder in a project's cache home, and the variables that point the tool at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tool {
    pub dir: &'static str,
    pub vars: &'static [&'static str],
}

/// Every toolchain's folder, in the order a sentence lists them.
pub const TOOLS: [Tool; 10] = [
    Tool {
        dir: "cargo",
        vars: &["CARGO_HOME"],
    },
    Tool {
        dir: "npm",
        vars: &["npm_config_cache", "NPM_CONFIG_CACHE"],
    },
    Tool {
        dir: "pip",
        vars: &["PIP_CACHE_DIR"],
    },
    Tool {
        dir: "go-mod",
        vars: &["GOMODCACHE"],
    },
    Tool {
        dir: "go-build",
        vars: &["GOCACHE"],
    },
    Tool {
        dir: "gradle",
        vars: &["GRADLE_USER_HOME"],
    },
    Tool {
        dir: "yarn",
        vars: &["YARN_CACHE_FOLDER"],
    },
    Tool {
        dir: "yarn-berry",
        vars: &["YARN_GLOBAL_FOLDER"],
    },
    Tool {
        dir: "pnpm-store",
        // pnpm 10 reads `pnpm_config_`, pnpm 9 and older the `npm_config_` spelling. Spelled in
        // two halves: a commit scan from before #1364 read these names as an npm token.
        vars: &["pnpm_config_store_dir", concat!("npm", "_config_store_dir")],
    },
    Tool {
        dir: "pnpm-cache",
        vars: &["pnpm_config_cache_dir", concat!("npm", "_config_cache_dir")],
    },
];

/// What in cargo's home a chat writes whole: its registry (index, archives and sources) and its
/// git checkouts.
pub const CARGO_TREES: [&str; 2] = ["registry", "git/checkouts"];

/// cargo's git database: one bare repository per git dependency. A chat writes inside each one
/// cargo made, never an entry itself, so none is made, moved in or linked in.
pub const CARGO_GIT_DB: &str = "git/db";

/// The files cargo keeps at the top of its home while it downloads: its package cache locks and
/// its tracking database with SQLite's companions.
pub const CARGO_FILES: [&str; 6] = [
    ".package-cache",
    ".package-cache-mutate",
    ".global-cache",
    ".global-cache-wal",
    ".global-cache-shm",
    ".global-cache-journal",
];

/// What a bare repository holds that a later git runs: its config, which names programs
/// (`core.sshCommand`, `credential.helper`, `core.fsmonitor`), and its hooks.
pub const BARE_RUN: [&str; 2] = ["config", "hooks"];

/// A project's cache home, resolved for one chat: what it may write there, and what points its
/// tools at it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CacheHome {
    /// The project's cache home, which purlis makes and a chat never writes itself.
    pub root: PathBuf,
    /// Folders a chat writes whole. Each is made by purlis and pinned.
    pub trees: Vec<PathBuf>,
    /// Folders of bare repositories, of which a chat writes only what is inside each entry,
    /// never an entry, and never an entry's [`BARE_RUN`].
    pub bare: Vec<PathBuf>,
    /// Single files a chat writes.
    pub files: Vec<PathBuf>,
    /// The variables that point each tool at its folder.
    pub env: Vec<(String, String)>,
    /// The `config.toml` purlis writes in cargo's home ([`cargo_config`]).
    pub cargo_config: String,
    /// The `gradle.properties` purlis writes in Gradle's home ([`gradle_properties`]), beside
    /// [`GRADLE_INIT`].
    pub gradle_properties: String,
    /// The project the home is named for, as the system resolves it: what
    /// [`crate::leftovers::HOME_NOTE`] says, so the home can be removed once the project is
    /// gone (#1364). Empty where it is not known, and then no note is written.
    pub project: PathBuf,
}

impl CacheHome {
    /// Every folder purlis makes before the chat starts, outermost first.
    pub fn folders(&self) -> Vec<PathBuf> {
        let mut out = vec![self.root.clone()];
        let cargo = self.root.join("cargo");
        out.push(cargo.clone());
        out.push(cargo.join("git"));
        out.extend(self.trees.iter().cloned());
        out.extend(self.bare.iter().cloned());
        out.sort();
        out.dedup();
        out
    }

    /// What a chat may write, as folders and files: for a check that no program a chat starts
    /// on lies there.
    pub fn writable(&self) -> Vec<PathBuf> {
        let mut out = self.trees.clone();
        out.extend(self.bare.iter().cloned());
        out.extend(self.files.iter().cloned());
        out
    }

    /// Takes out every link where purlis keeps a folder or file of this home, without following
    /// it: the home is purlis's own, so a link there is one a chat planted to send a tool's
    /// writes elsewhere, and taking it out lets the chat start rather than dead-end. A link
    /// that cannot be taken out is left, and [`Self::prepare`] then refuses the chat.
    pub fn unlink_links(&self) {
        for path in self
            .folders()
            .iter()
            .chain(&self.files)
            .chain([&self.cargo_config_path()])
        {
            if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink()) {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// Where purlis writes cargo's config in this home.
    pub fn cargo_config_path(&self) -> PathBuf {
        self.root.join("cargo").join("config.toml")
    }

    /// Makes every folder and writes cargo's config, outside the sandbox, before the chat
    /// starts: links are taken out first ([`Self::unlink_links`]), every folder is checked to
    /// be no link before and after it is made, and the config is written to a new file beside
    /// it, opened without following a link, then renamed into place. Or the one sentence,
    /// naming the path, why the chat does not start.
    pub fn prepare(&self) -> Result<(), String> {
        self.unlink_links();
        let linked = |when: &str| -> Result<(), String> {
            match self
                .folders()
                .iter()
                .chain(&self.files)
                .find(|path| is_linked(path))
            {
                Some(path) => Err(super::caches_linked(path, when)),
                None => Ok(()),
            }
        };
        linked("before")?;
        for folder in self.folders() {
            std::fs::create_dir_all(&folder).map_err(|err| unwritable(&folder, &err))?;
        }
        linked("after")?;
        let config = self.cargo_config_path();
        write_beside(&config, self.cargo_config.as_bytes())
            .map_err(|err| unwritable(&config, &err))?;
        self.seed_gradle()?;
        self.note_the_project();
        Ok(())
    }

    /// Writes Gradle's settings and init script at the top of its folder in this home, where a
    /// home has one. That folder is pinned, so each is written through a new file beside it and
    /// renamed into place: whatever a chat left at either name, a link included, is replaced,
    /// never written through. Never in a folder below it, which a chat could swap for a link.
    fn seed_gradle(&self) -> Result<(), String> {
        let gradle = self.root.join("gradle");
        if !self.trees.contains(&gradle) {
            return Ok(());
        }
        for (name, text) in [
            (GRADLE_PROPERTIES, self.gradle_properties.as_str()),
            (GRADLE_INIT_NAME, GRADLE_INIT),
        ] {
            let path = gradle.join(name);
            write_beside(&path, text.as_bytes()).map_err(|err| unwritable(&path, &err))?;
        }
        Ok(())
    }

    /// Writes which project this home is ([`crate::leftovers::HOME_NOTE`]), again at every
    /// start so its time says when the home was last used, then has the homes of projects that
    /// are gone swept away, once per data home in this process and off this thread
    /// ([`crate::leftovers::sweep_homes`], #1364). Best effort: a home with no note is only
    /// kept.
    fn note_the_project(&self) {
        if self.project.as_os_str().is_empty() {
            return;
        }
        let note = self.root.join(crate::leftovers::HOME_NOTE);
        let _ = write_beside(&note, self.project.as_os_str().as_encoded_bytes());
        let Some(data_home) = self.root.parent().and_then(Path::parent) else {
            return;
        };
        static SWEPT: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
        let mut swept = SWEPT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if swept.iter().any(|one| one == data_home) {
            return;
        }
        swept.push(data_home.to_path_buf());
        let data_home = data_home.to_path_buf();
        let _ = std::thread::Builder::new()
            .name("purlis-home-sweep".to_owned())
            .spawn(move || {
                crate::leftovers::sweep_homes(&data_home, std::time::SystemTime::now());
            });
    }
}

/// Why a chat does not start when purlis cannot make or write `path` of its project's caches.
fn unwritable(path: &Path, err: &std::io::Error) -> String {
    format!(
        "this project runs every chat sandboxed, and purlis could not make {} for the project's \
         package caches ({err}), so nothing was started.",
        path.display()
    )
}

/// Writes `bytes` to `path` through a new file beside it, made without following a link and
/// renamed into place, so a link at `path` is replaced, never written through.
fn write_beside(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("no folder"))?;
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("no name"))?;
    let temp = parent.join(format!(
        ".{}.purlis-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&temp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW).mode(0o644);
    }
    let written = options
        .open(&temp)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()));
    if let Err(err) = written {
        let _ = std::fs::remove_file(&temp);
        return Err(err);
    }
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

/// Where [`home_of`] puts the cache home of the project at `root` on `machine`, whether or not
/// it is made or given: `cache-homes/<project key>` under purlis's data home. `None` where
/// there is no data home.
pub fn root_of(machine: &Machine, root: &Path) -> Option<PathBuf> {
    Some(
        super::Homes::charter_data(machine)?
            .join("cache-homes")
            .join(super::Homes::project_key(root)),
    )
}

/// The cache home of the project at `root` on `machine`'s sandboxed chats: a folder named for
/// the project as the kernel names it, under purlis's data home. `None` where there is no data
/// home, or where the folder is not one a chat may be given ([`granted`]).
pub fn home_of(machine: &Machine, root: &Path, denied: &Denied) -> Option<CacheHome> {
    let base = root_of(machine, root)?;
    let cargo = base.join("cargo");
    let mut home = CacheHome {
        root: base.clone(),
        trees: TOOLS
            .iter()
            .filter(|tool| tool.dir != "cargo")
            .map(|tool| base.join(tool.dir))
            .collect(),
        bare: vec![cargo.join(CARGO_GIT_DB)],
        files: CARGO_FILES.iter().map(|file| cargo.join(file)).collect(),
        env: Vec::new(),
        cargo_config: {
            let (person, root) = person_cargo_config(machine);
            cargo_config(&person, root.as_deref())
        },
        gradle_properties: gradle_properties(&person_gradle_properties(machine)),
        // As [`super::Homes::project_key`] names the home for it.
        project: root.canonicalize().unwrap_or_else(|_| root.to_path_buf()),
    };
    home.trees
        .splice(0..0, CARGO_TREES.iter().map(|tree| cargo.join(tree)));
    for tool in TOOLS {
        for var in tool.vars {
            home.env
                .push(((*var).to_owned(), base.join(tool.dir).display().to_string()));
        }
    }
    // A link purlis's own home holds is one a chat planted: taken out, not followed, so the
    // grants below name the folders purlis made and not where a link points.
    home.unlink_links();
    granted(&home, machine, root, denied).then_some(home)
}

/// The text of the person's own cargo config: `$CARGO_HOME`'s, or `~/.cargo`'s, under either
/// name cargo reads. Empty where there is none. With it, the folder its relative paths are
/// resolved against: the one above the folder the config is in, as cargo resolves a path
/// written in a config file (#1364).
fn person_cargo_config(machine: &Machine) -> (String, Option<PathBuf>) {
    let Some(home) = machine
        .env
        .get("CARGO_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| machine.home.as_ref().map(|home| home.join(".cargo")))
    else {
        return (String::new(), None);
    };
    let text = ["config.toml", "config"]
        .iter()
        .find_map(|name| super::read_plane_file(&home.join(name)).ok().flatten())
        .unwrap_or_default();
    (text, home.parent().map(Path::to_path_buf))
}

/// The keys of a `[source]` entry that name a folder, which cargo resolves against the
/// config's folder when they are relative.
const SOURCE_PATHS: [&str; 2] = ["directory", "local-registry"];

/// `written`, a [`SOURCE_PATHS`] value of the person's config, as the project's cargo home's
/// config must say it. An absolute path is copied as written: what a chat may read there is the
/// sandbox's to decide, and this grants nothing. A relative one is resolved against `root`
/// ([`person_cargo_config`]), since the copy sits in another folder and would resolve it
/// elsewhere. It is taken only when it climbs nowhere (no `..`), names a folder that is there,
/// and that folder, with every link on the way followed, is still inside `root`: otherwise
/// `None`, and the entry is left out.
fn source_path(written: &str, root: Option<&Path>) -> Option<String> {
    config_path(written, root, Path::is_dir)
}

/// [`source_path`] for a path written in the person's config that must name what `is` says:
/// a folder for a `[source]`, a file for `http.cainfo`.
fn config_path(written: &str, root: Option<&Path>, is: fn(&Path) -> bool) -> Option<String> {
    use std::path::Component;
    let path = Path::new(written);
    if path.is_absolute() {
        return Some(written.to_owned());
    }
    if written.is_empty()
        || !path
            .components()
            .all(|part| matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    let root = root?.canonicalize().ok()?;
    let resolved = root.join(path).canonicalize().ok()?;
    if !resolved.starts_with(&root) || !is(&resolved) {
        return None;
    }
    resolved.to_str().map(str::to_owned)
}

/// The keys of a `[source]` entry that say where crates come from, and nothing else.
const SOURCE_KEYS: [&str; 8] = [
    "replace-with",
    "registry",
    "local-registry",
    "directory",
    "git",
    "branch",
    "tag",
    "rev",
];

/// The `config.toml` of a project's cargo home, from `person`, the person's own: where crates
/// come from and nothing else (D-1337-8). Each `[registries]` entry's `index` and `protocol`,
/// `registry.default`, and each `[source]` entry's [`SOURCE_KEYS`]. Never a token, a credential
/// provider, a build setting, an alias or a runner, which name programs or hold a secret. A
/// config purlis cannot read as TOML gives nothing.
///
/// `root` is the folder a relative `directory` or `local-registry` is resolved against
/// ([`source_path`]). A `[source]` entry with one that cannot be resolved there is left out
/// whole: a `replace-with` naming it then makes cargo say the source is missing, rather than
/// fetching from somewhere the person did not name.
pub fn cargo_config(person: &str, root: Option<&Path>) -> String {
    let mut out = toml::Table::new();
    let top = person.parse::<toml::Table>().unwrap_or_default();
    let keep = |table: Option<&toml::Value>, keys: &[&str]| -> toml::Table {
        let mut kept = toml::Table::new();
        let Some(entries) = table.and_then(toml::Value::as_table) else {
            return kept;
        };
        for (name, entry) in entries {
            let Some(entry) = entry.as_table() else {
                continue;
            };
            let fields: toml::Table = entry
                .iter()
                .filter(|(key, value)| keys.contains(&key.as_str()) && value.is_str())
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            if !fields.is_empty() {
                kept.insert(name.clone(), toml::Value::Table(fields));
            }
        }
        kept
    };
    let registries = keep(top.get("registries"), &["index", "protocol"]);
    if !registries.is_empty() {
        out.insert("registries".to_owned(), toml::Value::Table(registries));
    }
    if let Some(default) = top
        .get("registry")
        .and_then(|registry| registry.get("default"))
        .filter(|value| value.is_str())
    {
        let mut registry = toml::Table::new();
        registry.insert("default".to_owned(), default.clone());
        out.insert("registry".to_owned(), toml::Value::Table(registry));
    }
    let mut source = keep(top.get("source"), &SOURCE_KEYS);
    let before = source.len();
    source.retain(|_, entry| {
        let Some(entry) = entry.as_table_mut() else {
            return false;
        };
        SOURCE_PATHS.iter().all(|key| {
            let Some(value) = entry.get(*key) else {
                return true;
            };
            let Some(resolved) = value
                .as_str()
                .and_then(|written| source_path(written, root))
            else {
                return false;
            };
            entry.insert((*key).to_owned(), toml::Value::String(resolved));
            true
        })
    });
    let dropped = if source.len() < before {
        "# Left out: a [source] entry whose relative folder purlis could not find inside the \
         folder above your cargo home, without climbing out of it or following a link out.\n"
    } else {
        ""
    };
    if !source.is_empty() {
        out.insert("source".to_owned(), toml::Value::Table(source));
    }
    for (table, keys) in NETWORK_KEYS {
        let kept: toml::Table = keys
            .iter()
            .filter_map(|(key, kind)| {
                let value = top.get(table)?.get(*key)?;
                let kept = match kind {
                    Kept::Bool => value.as_bool().map(toml::Value::Boolean),
                    Kept::File => value
                        .as_str()
                        .and_then(|written| config_path(written, root, Path::is_file))
                        .map(toml::Value::String),
                };
                Some(((*key).to_owned(), kept?))
            })
            .collect();
        if !kept.is_empty() {
            out.insert(table.to_owned(), toml::Value::Table(kept));
        }
    }
    let proxy = if top.get("http").and_then(|http| http.get("proxy")).is_some() {
        "# Left out: your [http] proxy. A sandboxed chat reaches the network only through \
         purlis's own proxy, which cargo finds in HTTPS_PROXY.\n"
    } else {
        ""
    };
    format!(
        "# Written by purlis for this project's sandboxed chats at every start: where crates come \
         from, and how cargo fetches them, copied from your own cargo config. Nothing else is \
         read from here.\n{dropped}{proxy}{out}"
    )
}

/// How a kept network setting is read.
#[derive(Clone, Copy)]
enum Kept {
    /// `true` or `false`.
    Bool,
    /// A file it reads as data, resolved as a `[source]` folder is ([`config_path`]).
    File,
}

/// The keys of cargo's `[http]` and `[net]` that say how it fetches and route nothing (#1364):
/// the certificate file it trusts, the revocation check, fetching git with the git program, and
/// offline. Never `http.proxy`: a sandboxed chat reaches the network only through purlis's
/// proxy, which cargo already finds in `HTTPS_PROXY`, and a proxy of the person's would be
/// refused.
const NETWORK_KEYS: [(&str, &[(&str, Kept)]); 2] = [
    (
        "http",
        &[("cainfo", Kept::File), ("check-revoke", Kept::Bool)],
    ),
    (
        "net",
        &[("git-fetch-with-cli", Kept::Bool), ("offline", Kept::Bool)],
    ),
];

/// Where Gradle's settings are at the top of its home.
pub const GRADLE_PROPERTIES: &str = "gradle.properties";

/// The init script Gradle runs from the top of its home before every build.
pub const GRADLE_INIT_NAME: &str = "init.gradle";

/// The init script purlis writes in the project's Gradle home (#1364). Java does not read
/// `HTTPS_PROXY`, so without it Gradle connects directly, which the sandbox refuses. Each chat
/// has a proxy of its own on a port of its own, which one file shared by the project's chats
/// cannot name, so the script reads it from the chat's environment at every build and points
/// Java's proxy settings at it. Never a user, a password or a host left out of the proxy.
pub const GRADLE_INIT: &str = r#"// Written by purlis for this project's sandboxed chats at every start. A sandboxed chat
// reaches the network only through its own proxy, which its HTTPS_PROXY names. Java does not
// read HTTPS_PROXY, so this points Gradle's downloads at it before every build.
def proxyOf = { List<String> names ->
    def named = names.collect { System.getenv(it) }.find { it }
    if (!named) {
        return null
    }
    try {
        def proxy = new URI(named)
        return proxy.host && proxy.port > 0 ? proxy : null
    } catch (URISyntaxException ignored) {
        return null
    }
}
def https = proxyOf(['HTTPS_PROXY', 'https_proxy', 'HTTP_PROXY', 'http_proxy'])
def http = proxyOf(['HTTP_PROXY', 'http_proxy', 'HTTPS_PROXY', 'https_proxy'])
if (https) {
    System.setProperty('https.proxyHost', https.host)
    System.setProperty('https.proxyPort', String.valueOf(https.port))
}
if (http) {
    System.setProperty('http.proxyHost', http.host)
    System.setProperty('http.proxyPort', String.valueOf(http.port))
}
"#;

/// The `gradle.properties` keys of the project's Gradle home that say how Gradle builds and
/// caches, each kept when its value is one plain word: never a JVM option, a Java home or a
/// toolchain path, which name programs, and never a project property, which may hold a
/// credential.
const GRADLE_KEYS: [&str; 13] = [
    "org.gradle.caching",
    "org.gradle.caching.debug",
    "org.gradle.parallel",
    "org.gradle.workers.max",
    "org.gradle.configureondemand",
    "org.gradle.configuration-cache",
    "org.gradle.console",
    "org.gradle.logging.level",
    "org.gradle.warning.mode",
    "org.gradle.vfs.watch",
    "org.gradle.daemon",
    "org.gradle.daemon.idletimeout",
    "systemProp.javax.net.ssl.trustStoreType",
];

/// The trust store Java reads certificates from: data, as cargo's `cainfo` is, kept when it is
/// an absolute path. Never its password.
const GRADLE_TRUST_STORE: &str = "systemProp.javax.net.ssl.trustStore";

/// The `gradle.properties` of a project's Gradle home, from `person`, the person's own (#1364):
/// the [`GRADLE_KEYS`] and [`GRADLE_TRUST_STORE`], in the person's order, the last of a key
/// winning as Java reads the file. A line purlis would have to unescape or join with the next
/// is left out. A proxy of the person's is never copied, and the file says so: a sandboxed
/// chat reaches the network only through purlis's proxy, which [`GRADLE_INIT`] points Gradle
/// at.
pub fn gradle_properties(person: &str) -> String {
    let mut kept: Vec<(&str, &str)> = Vec::new();
    let mut proxy = false;
    let mut joined = false;
    for line in person.lines() {
        let continued = joined;
        let trimmed = line.trim_start();
        // A comment never runs on to the next line; any other line does after an odd run of
        // backslashes.
        joined = !trimmed.starts_with(['#', '!'])
            && line.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1;
        if continued || trimmed.starts_with(['#', '!']) || trimmed.is_empty() {
            continue;
        }
        let Some((key, value)) = property(trimmed) else {
            continue;
        };
        if key.starts_with("systemProp.") && key.to_ascii_lowercase().contains("proxy") {
            proxy = true;
        }
        let plain = if key == GRADLE_TRUST_STORE {
            Path::new(value).is_absolute() && !value.chars().any(|c| c.is_control() || c == '\\')
        } else {
            GRADLE_KEYS.contains(&key)
                && !value.is_empty()
                && value.len() <= 64
                && value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        };
        kept.retain(|(seen, _)| *seen != key);
        if plain && !joined {
            kept.push((key, value));
        }
    }
    let mut out = String::from(
        "# Written by purlis for this project's sandboxed chats at every start: how Gradle builds \
         and caches, copied from your own gradle.properties. Your init scripts and other \
         settings are not copied.\n",
    );
    if proxy {
        out.push_str(
            "# Left out: your proxy. A sandboxed chat reaches the network only through purlis's \
             own proxy, which init.gradle beside this file points Gradle at.\n",
        );
    }
    for (key, value) in kept {
        out.push_str(key);
        out.push('=');
        out.push_str(value);
        out.push('\n');
    }
    out
}

/// One `key=value`, `key: value` or `key value` line of a `.properties` file, both trimmed, or
/// `None` where the key holds a backslash, which Java would read as an escape. A value with one
/// is read as written, and no kept key's value may hold one.
fn property(line: &str) -> Option<(&str, &str)> {
    let end = line
        .find(|c: char| c == '=' || c == ':' || c.is_whitespace())
        .unwrap_or(line.len());
    let (key, rest) = line.split_at(end);
    let rest = rest.trim_start();
    let value = rest
        .strip_prefix(['=', ':'])
        .unwrap_or(rest)
        .trim_start()
        .trim_end();
    (!key.is_empty() && !key.contains('\\')).then_some((key, value))
}

/// The text of the person's own `gradle.properties`: `$GRADLE_USER_HOME`'s, or `~/.gradle`'s.
/// Empty where there is none.
fn person_gradle_properties(machine: &Machine) -> String {
    machine
        .env
        .get("GRADLE_USER_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .or_else(|| machine.home.as_ref().map(|home| home.join(".gradle")))
        .and_then(|home| {
            super::read_plane_file(&home.join(GRADLE_PROPERTIES))
                .ok()
                .flatten()
        })
        .unwrap_or_default()
}

/// The folders a later program is found in, which no cache home may be or hold: every folder a
/// chat's `PATH` searches ([`crate::programs::search_dirs_from`]), and each tool's own bin
/// folder on `PATH` or not: cargo's home and its `bin`, Go's `bin`, and pnpm's home.
pub fn never_covered(machine: &Machine) -> Vec<PathBuf> {
    let absolute = |name: &str| {
        machine
            .env
            .get(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let home = |rel: &str| machine.home.as_ref().map(|home| home.join(rel));
    let path = machine.env.get("PATH");
    let mut out = crate::programs::search_dirs_from(
        path.as_deref().map(std::ffi::OsStr::new),
        machine.home.as_deref(),
    );
    if let Some(cargo) = absolute("CARGO_HOME").or_else(|| home(".cargo")) {
        out.push(cargo.join("bin"));
        out.push(cargo);
    }
    out.extend(absolute("GOBIN"));
    let gopath = machine
        .env
        .get("GOPATH")
        .and_then(|gopath| std::env::split_paths(&gopath).next())
        .filter(|first| first.is_absolute())
        .or_else(|| home("go"));
    out.extend(gopath.map(|gopath| gopath.join("bin")));
    out.extend(absolute("PNPM_HOME").or_else(|| home("Library/pnpm")));
    out
}

/// Whether `home` may be given to a chat: every folder of it plain, none a link, and its root
/// holding neither the person's home, the project, a folder in [`never_covered`] nor a denied
/// path, nor inside a denied path.
fn granted(home: &CacheHome, machine: &Machine, root: &Path, denied: &Denied) -> bool {
    use std::path::Component;
    let base = &home.root;
    if !base.is_absolute()
        || base
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return false;
    }
    if home
        .folders()
        .iter()
        .chain(&home.files)
        .any(|path| is_linked(path))
    {
        return false;
    }
    let names = |path: &Path| {
        let real = super::real(path);
        if real == path {
            vec![path.to_path_buf()]
        } else {
            vec![path.to_path_buf(), real]
        }
    };
    let bases = names(base);
    let holds = |other: &Path| {
        names(other)
            .iter()
            .any(|other| bases.iter().any(|base| other.starts_with(base)))
    };
    let inside = |other: &Path| {
        names(other)
            .iter()
            .any(|other| bases.iter().any(|base| base.starts_with(other)))
    };
    if machine.home.as_deref().is_some_and(holds) || holds(root) || inside(root) {
        return false;
    }
    if never_covered(machine)
        .iter()
        .any(|dir| holds(dir) || inside(dir))
    {
        return false;
    }
    !denied.paths.iter().any(|denial| {
        // An entry held alone holds nothing below it (#2752).
        holds(&denial.path) || (denial.reach == super::Reach::AndBelow && inside(&denial.path))
    })
}

/// Whether `path` is there and is a link, or is reached through one its own parent does not
/// name: a chat that swapped a cache folder for a link would have every write into it land
/// wherever the link points.
pub fn is_linked(path: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return false;
    };
    if meta.file_type().is_symlink() {
        return true;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => super::real(path) != super::real(parent).join(name),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #1364: every start writes which project the home is, as the system resolves it, beside
    /// the folders a chat writes and never in one of them.
    #[test]
    fn a_start_notes_which_project_its_cache_home_is() {
        let dir = tempfile::tempdir().expect("a directory");
        let base = dir.path().canonicalize().expect("the directory");
        let home = CacheHome {
            root: base.join("data/cache-homes").join("0".repeat(32)),
            project: base.join("projects/shop"),
            ..CacheHome::default()
        };

        home.prepare().expect("prepared");

        assert_eq!(
            std::fs::read_to_string(home.root.join(crate::leftovers::HOME_NOTE)).expect("the note"),
            base.join("projects/shop").display().to_string()
        );
        assert!(
            home.writable().iter().all(|path| !home
                .root
                .join(crate::leftovers::HOME_NOTE)
                .starts_with(path)),
            "a chat could write the note"
        );
    }

    /// The settings of `text`, a `gradle.properties`, as `key=value` lines without comments.
    fn settings(text: &str) -> Vec<&str> {
        text.lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    }

    /// #1364: `GRADLE_USER_HOME` moves Gradle's `gradle.properties` with it. The project's Gradle
    /// home gets the person's settings that say how Gradle builds and caches, and nothing that
    /// holds a secret, names a program or a JVM option, or sends Gradle around purlis's proxy.
    #[test]
    fn a_projects_gradle_home_keeps_how_gradle_builds_and_caches_and_nothing_else() {
        let person = "\
# the person's own
org.gradle.caching=true
org.gradle.parallel = true
org.gradle.workers.max: 4
org.gradle.configuration-cache=true
org.gradle.console=plain
org.gradle.daemon=false
org.gradle.jvmargs=-Xmx4g -javaagent:/opt/agent.jar
org.gradle.java.home=/opt/jdk
systemProp.https.proxyHost=proxy.corp.example
systemProp.https.proxyPort=8080
systemProp.http.proxyUser=me
systemProp.http.proxyPassword=hunter2
systemProp.javax.net.ssl.trustStore=/etc/corp/truststore.jks
systemProp.javax.net.ssl.trustStorePassword=changeit
artifactoryPassword=hunter3
signing.keyId=ABCDEF
org.gradle.caching.debug=yes please
org.gradle.logging.level=\\u0069nfo
org.gradle.warning.mode=all \\
    fail
";
        let seeded = gradle_properties(person);
        assert_eq!(
            settings(&seeded),
            [
                "org.gradle.caching=true",
                "org.gradle.parallel=true",
                "org.gradle.workers.max=4",
                "org.gradle.configuration-cache=true",
                "org.gradle.console=plain",
                "org.gradle.daemon=false",
                "systemProp.javax.net.ssl.trustStore=/etc/corp/truststore.jks",
            ]
        );
        for word in [
            "hunter",
            "javaagent",
            "jdk",
            "proxyUser",
            "changeit",
            "signing",
            "corp.example",
        ] {
            assert!(!seeded.contains(word), "{word} is in:\n{seeded}");
        }
        // The proxy left out is said, in the file a person reads.
        assert!(
            seeded.contains("# Left out: your proxy"),
            "no word of the proxy:\n{seeded}"
        );
        // Nothing to copy is only the header.
        assert!(settings(&gradle_properties("")).is_empty());
        assert!(!gradle_properties("").contains("Left out"));
    }

    /// A trust store is data, like cargo's `cainfo`: kept as an absolute path, never a relative
    /// one, which Java resolves against whatever folder the build runs in.
    #[test]
    fn a_gradle_trust_store_is_kept_only_as_an_absolute_path() {
        assert!(
            settings(&gradle_properties(
                "systemProp.javax.net.ssl.trustStore=certs/store.jks\n"
            ))
            .is_empty()
        );
        assert_eq!(
            settings(&gradle_properties(
                "systemProp.javax.net.ssl.trustStore=/etc/store.jks\n\
                 systemProp.javax.net.ssl.trustStoreType=PKCS12\n"
            )),
            [
                "systemProp.javax.net.ssl.trustStore=/etc/store.jks",
                "systemProp.javax.net.ssl.trustStoreType=PKCS12",
            ]
        );
        // The last of a key wins, as Java reads the file.
        assert_eq!(
            settings(&gradle_properties(
                "org.gradle.caching=false\norg.gradle.caching=true\n"
            )),
            ["org.gradle.caching=true"]
        );
    }

    /// Java does not read `HTTPS_PROXY`, so the project's Gradle home carries an init script that
    /// points Gradle at the chat's own proxy, which a shared file cannot name by port.
    #[test]
    fn a_projects_gradle_home_points_gradle_at_the_chats_proxy() {
        for word in [
            "HTTPS_PROXY",
            "HTTP_PROXY",
            "https.proxyHost",
            "https.proxyPort",
            "http.proxyHost",
            "http.proxyPort",
        ] {
            assert!(
                GRADLE_INIT.contains(word),
                "{word} is not in the init script"
            );
        }
        for word in ["proxyUser", "proxyPassword", "nonProxyHosts", "exec"] {
            assert!(!GRADLE_INIT.contains(word), "{word} is in the init script");
        }
    }

    /// Every start writes the Gradle home's settings and init script at the top of its
    /// folder, replacing whatever a chat left there, a link included, never written through.
    #[test]
    fn a_start_writes_the_gradle_homes_settings_and_init_script() {
        let dir = tempfile::tempdir().expect("a directory");
        let base = dir.path().canonicalize().expect("the directory");
        let home = CacheHome {
            root: base.join("data/cache-homes").join("0".repeat(32)),
            trees: vec![
                base.join("data/cache-homes")
                    .join("0".repeat(32))
                    .join("gradle"),
            ],
            gradle_properties: "org.gradle.caching=true\n".to_owned(),
            ..CacheHome::default()
        };
        let gradle = home.root.join("gradle");
        std::fs::create_dir_all(&gradle).expect("the gradle home");
        std::fs::write(base.join("elsewhere"), "kept").expect("a file elsewhere");
        #[cfg(unix)]
        std::os::unix::fs::symlink(base.join("elsewhere"), gradle.join(GRADLE_PROPERTIES))
            .expect("a planted link");
        std::fs::write(gradle.join(GRADLE_INIT_NAME), "a chat's own").expect("a chat's script");

        home.prepare().expect("prepared");

        assert_eq!(
            std::fs::read_to_string(gradle.join(GRADLE_PROPERTIES)).expect("the settings"),
            "org.gradle.caching=true\n"
        );
        assert!(
            !std::fs::symlink_metadata(gradle.join(GRADLE_PROPERTIES))
                .expect("there")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read_to_string(base.join("elsewhere")).expect("elsewhere"),
            "kept"
        );
        assert_eq!(
            std::fs::read_to_string(gradle.join(GRADLE_INIT_NAME)).expect("the script"),
            GRADLE_INIT
        );
    }

    /// The person's own Gradle home is `$GRADLE_USER_HOME`, or `~/.gradle`: its
    /// `gradle.properties` is what the project's is seeded from.
    #[test]
    fn the_persons_own_gradle_settings_are_read_where_gradle_reads_them() {
        let person = tempfile::tempdir().expect("a home");
        let data = tempfile::tempdir().expect("a data home");
        let person_home = person.path().canonicalize().expect("the home");
        let data_home = data.path().canonicalize().expect("the data home");
        std::fs::create_dir_all(person_home.join(".gradle")).expect("~/.gradle");
        std::fs::write(
            person_home.join(".gradle/gradle.properties"),
            "org.gradle.caching=true\n",
        )
        .expect("the person's settings");
        std::fs::create_dir_all(person_home.join("moved")).expect("a moved home");
        std::fs::write(
            person_home.join("moved/gradle.properties"),
            "org.gradle.parallel=true\n",
        )
        .expect("the moved settings");
        let project = person_home.join("project");
        std::fs::create_dir_all(&project).expect("a project");
        let data_var = data_home.display().to_string();
        let moved = person_home.join("moved").display().to_string();
        let on = |env: &[(&str, &str)]| {
            let machine = Machine {
                env: crate::secrets::Env::of(env),
                home: Some(person_home.clone()),
                os: super::super::Os::this(),
            };
            home_of(&machine, &project, &Denied::default())
                .expect("a cache home")
                .gradle_properties
        };

        let plain = on(&[(crate::datahome::HOME_VAR, &data_var)]);
        assert_eq!(settings(&plain), ["org.gradle.caching=true"]);
        let moved = on(&[
            (crate::datahome::HOME_VAR, &data_var),
            ("GRADLE_USER_HOME", &moved),
        ]);
        assert_eq!(settings(&moved), ["org.gradle.parallel=true"]);
    }
}
