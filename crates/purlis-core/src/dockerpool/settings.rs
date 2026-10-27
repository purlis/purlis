// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What each run of `docker` purlis makes is given** (#2752, the person's ruling of
//! 2026-10-20). purlis never hands `docker` the person's own settings as they are. Each run
//! gets:
//!
//! - **a checked copy of them** ([`checked`]), made fresh for the run in a folder of its own
//!   under `<data>/docker-settings/`, `0600` in a `0700` folder, and removed once the run ends.
//!   It keeps `auths`, and `credsStore` and `credHelpers` only where the helper they name is in
//!   one of Docker's own folders (where its installers put `docker`, never the home's folders
//!   or the window's `PATH`). Nothing else is kept: no plugin folder, no hook, no context, no
//!   host;
//! - **the engine pinned** ([`engine`]): the window's own `DOCKER_HOST` where it has one, else
//!   the system's socket, else the socket of the installer whose `docker` this is, each only
//!   where no chat can put another socket in its place, and never a context (`DOCKER_CONTEXT`
//!   is cleared; `docker context` is not followed);
//! - **a `PATH` of Docker's own folders and the system's** ([`run_path`]), where each helper it
//!   is handed was found.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{Refused, refused};

/// The folder under `<data>` each run's copy of Docker's settings is made in.
pub const DIR: &str = "docker-settings";

/// The most of Docker's own `config.json` purlis reads: one larger is taken as holding nothing.
const MOST_READ: u64 = 1024 * 1024;

/// The variables a run is given, and those it is not handed from the window.
pub const CLEARED: &[&str] = &["DOCKER_CONTEXT"];

/// **Docker's settings `read`, as purlis hands them on**: `auths` as it is, and `credsStore`
/// and each of `credHelpers` where `held` says the helper of that name is in Docker's own
/// folders. Nothing else.
pub fn checked(read: &serde_json::Value, held: &dyn Fn(&str) -> bool) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    if let Some(auths @ serde_json::Value::Object(_)) = read.get("auths") {
        out.insert("auths".into(), auths.clone());
    }
    let kept = |name: &str| a_helper_name(name) && held(name);
    if let Some(store) = read
        .get("credsStore")
        .and_then(serde_json::Value::as_str)
        .filter(|name| kept(name))
    {
        out.insert("credsStore".into(), store.into());
    }
    if let Some(helpers) = read
        .get("credHelpers")
        .and_then(serde_json::Value::as_object)
    {
        let helpers: serde_json::Map<String, serde_json::Value> = helpers
            .iter()
            .filter(|(_, helper)| helper.as_str().is_some_and(kept))
            .map(|(registry, helper)| (registry.clone(), helper.clone()))
            .collect();
        if !helpers.is_empty() {
            out.insert("credHelpers".into(), helpers.into());
        }
    }
    serde_json::Value::Object(out)
}

/// Whether `name` can be a helper's: what Docker puts after `docker-credential-`, letters,
/// digits, dashes, dots and underscores, so it can never name a path.
fn a_helper_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Docker's own folders: where its installers put `docker` ([`super::DOCKER_INSTALLED`]),
/// never the home's folders or the window's `PATH`.
pub fn docker_folders() -> Vec<PathBuf> {
    super::DOCKER_INSTALLED
        .iter()
        .filter_map(|at| Path::new(at).parent().map(Path::to_path_buf))
        .collect()
}

/// **The `PATH` a run of `docker` is given**: Docker's own folders ([`docker_folders`]), where
/// every helper it is handed was found, then the system's own (`/bin`, `/usr/sbin`, `/sbin`),
/// which only root writes. Nothing of the home and nothing of the window's `PATH`.
pub fn run_path() -> Vec<PathBuf> {
    let mut out = docker_folders();
    for system in ["/bin", "/usr/sbin", "/sbin"] {
        let system = PathBuf::from(system);
        if !out.contains(&system) {
            out.push(system);
        }
    }
    out
}

/// Whether the helper `name` is a program in one of `folders`.
pub fn held_in(folders: &[PathBuf], name: &str) -> bool {
    a_helper_name(name)
        && folders.iter().any(|folder| {
            let program = folder.join(format!("docker-credential-{name}"));
            crate::secrets::run::which(&program.display().to_string(), None).is_some()
        })
}

/// The system's own Docker socket.
pub const SYSTEM_SOCKET: &str = "/var/run/docker.sock";

/// Where Docker Desktop, OrbStack and Rancher Desktop keep their engine's socket in the home,
/// in the order their `docker` is looked for ([`super::DOCKER_IN_HOME`]).
pub const HOME_SOCKETS: [&str; 3] = [
    ".docker/run/docker.sock",
    ".orbstack/run/docker.sock",
    ".rd/docker.sock",
];

/// **Where in the home an engine's socket is held from every chat's writes** (D-2752-7): the
/// folders Docker Desktop and OrbStack keep theirs in, Rancher Desktop's socket itself, and
/// the folders Colima and Lima keep each machine's in. A run is pointed only at a socket under
/// one of these, or in a folder only root writes ([`trusted`]).
pub const HELD_IN_HOME: [&str; 5] = [
    ".docker/run",
    ".orbstack/run",
    ".rd/docker.sock",
    ".colima",
    ".lima",
];

/// The folder in the temporary folder where a Podman machine's helper keeps the socket it
/// links `/var/run/docker.sock` to: held from every chat's writes too.
pub const HELD_IN_TMP: &str = "podman";

/// Where in the runtime folder (`$XDG_RUNTIME_DIR`) a rootless engine keeps its socket:
/// rootless Docker's, and Podman's own service's folder. Held from every chat's writes too.
pub const HELD_IN_RUNTIME: [&str; 2] = ["docker.sock", "podman"];

/// What [`engine`] judges a socket against: the person's home, the window's temporary folder
/// and the system's own socket.
pub struct Places<'a> {
    pub home: Option<&'a Path>,
    pub tmp: Option<&'a Path>,
    pub runtime: Option<&'a Path>,
    pub system: &'a Path,
}

/// **The engine a run is pointed at, as `DOCKER_HOST`**: `None` where the window has a
/// `DOCKER_HOST` of its own (`window`), which the run inherits; else the system's socket where
/// it is there; else the first installer's socket in the home that is there and in the
/// installer's own folder ([`super::its_own_in`]). `None` too where none is, and `docker` then
/// says it reaches no engine. A `docker context` is never followed.
///
/// **The socket is used only where no chat can put another in its place** ([`trusted`]): as it
/// resolves, in a folder only root writes, or in one every chat's sandbox holds. One elsewhere,
/// the window's own included, is refused, naming its folder.
pub fn engine(window: Option<&str>, places: &Places<'_>) -> Result<Option<String>, Refused> {
    if let Some(host) = window.filter(|host| !host.is_empty()) {
        return match host.strip_prefix("unix://") {
            Some(socket) => trusted(Path::new(socket), places).map(|()| None),
            // A host on the network is the person's own to name: nothing here can be swapped.
            None => Ok(None),
        };
    }
    let homes = places.home.into_iter().flat_map(|home| {
        HOME_SOCKETS
            .iter()
            .filter(move |at| super::its_own_in(home, at))
            .map(move |at| home.join(at))
    });
    let Some(socket) = std::iter::once(places.system.to_path_buf())
        .chain(homes)
        .find(|socket| std::fs::symlink_metadata(socket).is_ok())
    else {
        return Ok(None);
    };
    trusted(&socket, places)?;
    Ok(Some(format!("unix://{}", socket.display())))
}

/// **Whether the engine a run is pointed at is on this machine** (#2050), and its socket if it
/// is: a `unix://` `DOCKER_HOST` of the window's (`window`) whose socket is trusted, or, with
/// none, the socket [`engine`] finds. Every other engine (`tcp://`, `ssh://`, any other
/// scheme) is another machine's as far as purlis can tell, and is refused in words, as is no
/// engine at all.
pub fn engine_on_this_machine(
    window: Option<&str>,
    places: &Places<'_>,
) -> Result<PathBuf, String> {
    if let Some(host) = window.filter(|host| !host.is_empty()) {
        return match host.strip_prefix("unix://") {
            Some(socket) => trusted(Path::new(socket), places)
                .map(|()| PathBuf::from(socket))
                .map_err(|why| why.0),
            None => Err(format!(
                "this machine's DOCKER_HOST is {host}, an engine purlis cannot tell is on this \
                 machine"
            )),
        };
    }
    match engine(None, places).map_err(|why| why.0)? {
        Some(host) => Ok(PathBuf::from(host.trim_start_matches("unix://"))),
        None => Err("purlis finds no Docker engine on this machine".to_owned()),
    }
}

/// Whether `socket`, as it resolves, is where no chat can put another in its place: in a
/// folder only root writes, or under one of [`HELD_IN_HOME`], [`HELD_IN_TMP`] or
/// [`HELD_IN_RUNTIME`] as they are themselves. Refused otherwise, with a sentence that names its folder.
pub fn trusted(socket: &Path, places: &Places<'_>) -> Result<(), Refused> {
    let resolved = socket
        .canonicalize()
        .unwrap_or_else(|_| socket.to_path_buf());
    let folder = resolved.parent().unwrap_or(Path::new("/")).to_path_buf();
    let held = held_places(places);
    if only_root_writes(&folder) || held.iter().any(|place| resolved.starts_with(place)) {
        return Ok(());
    }
    Err(refused(format!(
        "purlis points docker only at an engine whose socket is in a folder only root writes or \
         one no chat may write, and {} is in {}, so purlis ran no docker command; set \
         DOCKER_HOST, or link {SYSTEM_SOCKET}, to such a socket (purlis does not follow \
         `docker context`)",
        socket.display(),
        folder.display()
    )))
}

/// The places [`trusted`] takes a socket under, each as it resolves and only where it is the
/// installer's own folder by exactly its name.
fn held_places(places: &Places<'_>) -> Vec<PathBuf> {
    let exactly = |base: &Path, at: &str| -> Option<PathBuf> {
        let real_base = base.canonicalize().ok()?;
        let found = base.join(at).canonicalize().ok()?;
        (found.as_os_str() == real_base.join(at).as_os_str()).then_some(found)
    };
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(home) = places.home {
        out.extend(
            HELD_IN_HOME
                .iter()
                .filter(|at| super::its_own_in(home, at))
                .filter_map(|at| exactly(home, at)),
        );
    }
    if let Some(tmp) = places.tmp {
        out.extend(exactly(tmp, HELD_IN_TMP));
    }
    if let Some(runtime) = places.runtime {
        out.extend(HELD_IN_RUNTIME.iter().filter_map(|at| exactly(runtime, at)));
    }
    out
}

/// Whether `folder` is written by root alone: root's, and written by neither its group nor
/// anyone else.
pub fn only_root_writes(folder: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(folder).is_ok_and(|meta| meta.uid() == 0 && meta.mode() & 0o022 == 0)
}

/// What one run of `docker` is given: the folder its settings are in, held until the run
/// ends, and the variables it is run with.
pub struct Prepared {
    /// The run's folder, removed once this is dropped.
    folder: tempfile::TempDir,
    /// Its lock, held until then.
    lock: std::fs::File,
    /// Each variable and its value.
    pub env: Vec<(&'static str, OsString)>,
}

/// Where a run of `docker` is prepared from: this machine's environment, as the window has it.
pub struct Source<'a> {
    /// Where each run's folder is made: `<data>/docker-settings`.
    pub settings_in: &'a Path,
    /// The `PATH` the run is given: Docker's own folders ([`docker_folders`]), where the
    /// helpers it is handed were found.
    pub path: OsString,
    /// The person's home.
    pub home: Option<&'a Path>,
    /// The window's `DOCKER_CONFIG`, where Docker's own settings are when it names a folder.
    pub docker_config: Option<&'a Path>,
    /// The window's `DOCKER_HOST`.
    pub docker_host: Option<&'a str>,
    /// The window's temporary folder (`TMPDIR`).
    pub tmp: Option<&'a Path>,
    /// The window's runtime folder (`XDG_RUNTIME_DIR`).
    pub runtime: Option<&'a Path>,
    /// The system's own socket: [`SYSTEM_SOCKET`].
    pub system: &'a Path,
}

/// **Prepares one run of `docker`**: Docker's settings read from where the window keeps them,
/// checked ([`checked`], with helpers held in [`docker_folders`]), and written to a folder made
/// for the run; `DOCKER_CONFIG` names it, `DOCKER_HOST` the engine ([`engine`]) and `PATH` the
/// folders its helpers are looked for in.
pub fn prepare(from: &Source<'_>) -> Result<Prepared, Refused> {
    let source = from
        .docker_config
        .filter(|folder| folder.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| from.home.map(|home| home.join(".docker")))
        .map(|folder| folder.join("config.json"));
    let read = source.as_deref().map(read_settings).unwrap_or_default();
    let folders = docker_folders();
    let settings = checked(&read, &|name| held_in(&folders, name));

    if let Some(why) = crate::datahome::refusal(from.settings_in) {
        return Err(refused(why));
    }
    let private = |at: &Path| {
        purlis_same_user::PrivateDirectory::open(at).map_err(|why| {
            refused(format!(
                "purlis could not make {} for docker's settings: {why}",
                at.display()
            ))
        })
    };
    private(from.settings_in)?;
    sweep(from.settings_in, std::time::SystemTime::now());
    let folder = tempfile::Builder::new()
        .prefix("run-")
        .tempdir_in(from.settings_in)
        .map_err(|why| {
            refused(format!(
                "purlis could not make a folder for docker's settings in {}: {why}",
                from.settings_in.display()
            ))
        })?;
    let held = private(folder.path())?;
    // Held for as long as the run goes, so no sweep takes its copy (#2893).
    let lock = held
        .replace_file(HELD, b"")
        .and_then(|()| std::fs::File::open(folder.path().join(HELD)))
        .and_then(|file| file.lock().map(|()| file))
        .map_err(|why| {
            refused(format!(
                "purlis could not hold the folder for docker's settings in {}: {why}",
                folder.path().display()
            ))
        })?;
    held.replace_file("config.json", settings.to_string().as_bytes())
        .map_err(|why| {
            refused(format!(
                "purlis could not write docker's settings in {}: {why}",
                folder.path().display()
            ))
        })?;

    let mut env = vec![
        ("PATH", from.path.clone()),
        ("DOCKER_CONFIG", folder.path().as_os_str().to_owned()),
    ];
    let places = Places {
        home: from.home,
        tmp: from.tmp,
        runtime: from.runtime,
        system: from.system,
    };
    if let Some(host) = engine(from.docker_host, &places)? {
        env.push(("DOCKER_HOST", host.into()));
    }
    Ok(Prepared { folder, lock, env })
}

/// A run's variables, those it is not handed, and what is held until it ends.
pub type Parts = (
    Vec<(OsString, OsString)>,
    Vec<OsString>,
    Box<dyn Send + Sync>,
);

impl Prepared {
    /// **`contents` kept as `name` in the run's folder, `0600`**, for as long as the run goes:
    /// a file `docker` reads during the run (the runner's seccomp profile). Its path.
    pub fn keep(&self, name: &str, contents: &str) -> std::io::Result<PathBuf> {
        use std::os::unix::fs::OpenOptionsExt;
        let at = self.folder.path().join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&at)?;
        std::io::Write::write_all(&mut file, contents.as_bytes())?;
        Ok(at)
    }

    /// Its variables, those not handed on, and what is held until the run ends, for a
    /// connector that runs `docker` itself (`purlis_host::CommandConnector::preparing`).
    pub fn into_parts(self) -> Parts {
        let env = self
            .env
            .into_iter()
            .map(|(name, value)| (OsString::from(name), value))
            .collect();
        let cleared = CLEARED.iter().map(OsString::from).collect();
        (env, cleared, Box::new((self.folder, self.lock)))
    }
}

/// How old a run's copy must be before it is taken as left behind (#2893).
pub const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// The file in each run's folder the run holds locked for as long as it goes: a copy whose
/// lock can be taken belongs to no run.
const HELD: &str = "run.lock";

/// **Removes the copies runs left behind in `settings_in`** (#2893): each `run-*` folder not
/// changed in [`STALE_AFTER`] before `now` whose run is not going, which is a run that ended
/// outright. A run still going holds its folder's lock, and its copy stays however old. Nothing
/// but a run's folder is touched, nothing is followed through a link, and a `settings_in` that
/// is not a folder of its own sweeps nothing. How many went.
pub fn sweep(settings_in: &Path, now: std::time::SystemTime) -> usize {
    if !std::fs::symlink_metadata(settings_in).is_ok_and(|meta| meta.is_dir()) {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(settings_in) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let a_run = entry.file_name().to_string_lossy().starts_with("run-");
        let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        let stale = meta
            .modified()
            .ok()
            .and_then(|changed| now.duration_since(changed).ok())
            .is_some_and(|age| age >= STALE_AFTER);
        if !a_run || !meta.is_dir() || !stale || still_going(&entry.path()) {
            continue;
        }
        if std::fs::remove_dir_all(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Whether a run still holds the lock in `folder`.
fn still_going(folder: &Path) -> bool {
    let Ok(file) = std::fs::File::open(folder.join(HELD)) else {
        return false;
    };
    matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

/// The copies left behind in this machine's `<data>/docker-settings`, swept as the app starts
/// ([`sweep`]).
pub fn sweep_at_start() -> usize {
    crate::datahome::root().map_or(0, |data| {
        sweep(&data.join(DIR), std::time::SystemTime::now())
    })
}

/// Docker's settings at `path`, read up to [`MOST_READ`]: an empty object where there are none,
/// where the file is not one, or where it is not JSON.
fn read_settings(path: &Path) -> serde_json::Value {
    use std::io::Read;
    let empty = || serde_json::Value::Object(serde_json::Map::new());
    let Ok(file) = std::fs::File::open(path) else {
        return empty();
    };
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return empty();
    }
    let mut text = String::new();
    if file.take(MOST_READ + 1).read_to_string(&mut text).is_err() || text.len() as u64 > MOST_READ
    {
        return empty();
    }
    serde_json::from_str(&text).unwrap_or_else(|_| empty())
}

#[cfg(test)]
mod tests;
