// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Settings › This machine › Pools** (#2679, the person's rulings of 2026-10-20): the Docker
//! pools added on this machine, each made, paired and kept as it is added
//! ([`purlis_core::pools::add`]) and removed whole ([`purlis_core::pools::remove`]), unpaired
//! first over a link of its own while its runner answers ([`crate::runnerlinks::unpair`]). Every
//! rule is the core's; this says what is kept and does what the person pressed. The window's
//! alone (`ui::WINDOW_ONLY`): only the person adds or removes a pool, which runs a container on
//! their machine.

#[cfg(unix)]
use purlis_core::dockerpool::{Image, Limits};
#[cfg(unix)]
use purlis_core::runners::{Pool, Set};

#[cfg(unix)]
use crate::runnerlinks::{Machine, ToARunner};

/// Why a pool is not kept here: the Docker pool runs on macOS and Linux alone.
#[cfg(not(unix))]
const NOT_HERE: &str = "purlis keeps Docker pools on macOS and Linux only.";

/// A pool's limits, as a row draws them: the CPUs as typed (`2`, `1.5`), memory and `/tmp` in
/// MiB.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolLimitsShown {
    pub cpus: String,
    pub memory_mib: u32,
    pub processes: u32,
    pub tmp_mib: u32,
}

/// One pool, as its row draws it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolRow {
    pub name: String,
    pub image: String,
    /// What its runner is held to: what the person set, and the defaults for the rest.
    pub limits: PoolLimitsShown,
}

/// What the Pools group draws: the pools, and the limits a new pool gets by default.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolsShown {
    pub pools: Vec<PoolRow>,
    pub defaults: PoolLimitsShown,
    /// Whether this machine's pools' runners are shown to be the person's own (D-2050-4): their
    /// engine is a local socket on this machine. A chat moves only to such a pool (#2048).
    #[specta(optional)]
    pub yours: Option<bool>,
}

/// A pool as the person typed it: each limit left empty takes the default.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, specta::Type)]
pub struct PoolAdding {
    pub name: String,
    pub image: String,
    pub cpus: Option<String>,
    pub memory_mib: Option<u32>,
    pub processes: Option<u32>,
    pub tmp_mib: Option<u32>,
}

/// Where a pool's runner stands, as one link to it says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PoolLink {
    /// It answered with the key pinned at its pairing.
    Linked,
    /// It answered with another key: it is refused, and the window offers to remove it.
    KeyChanged,
    /// It did not answer, and why.
    Down { why: String },
    /// It suspended itself, a day without a link (#2051): its container is stopped, its home
    /// kept, and the window's next chat there starts it again. A check never wakes it.
    Suspended,
}

/// **Which version a pool's runner runs, against this desktop's** (#2749): what its row says
/// when they differ, and whether purlis moves the pool along with the desktop.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolVersion {
    /// The version its runner's `purlis --version` says; none where it could not be read.
    pub runner: Option<String>,
    /// Why it could not be read, in Docker's or purlis's words.
    pub why: Option<String>,
    /// This desktop's version.
    pub desktop: String,
    /// Whether the pool is of purlis's own runner image, which purlis moves to the desktop's
    /// version after an update; a team's own image is never moved.
    pub follows: bool,
    /// How many of its older runners drain: each keeps its chats until they end, then is
    /// removed.
    pub draining: u32,
    /// Whether this version's release named a runner image, as purlis last found it (#2880):
    /// one that named none leaves the pool where it is, not moving to this version; `None` until purlis has
    /// looked.
    pub named: Option<bool>,
    /// How many chats those older runners still keep, as purlis last found them (#2865).
    pub waiting: u32,
}

#[cfg(unix)]
/// `limits`, as a row draws them.
pub(crate) fn shown(limits: Limits) -> PoolLimitsShown {
    let (whole, part) = (limits.millicpus() / 1000, limits.millicpus() % 1000);
    let cpus = if part == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{part:03}")
            .trim_end_matches('0')
            .to_owned()
    };
    let mib = |value: u64| u32::try_from(value).unwrap_or(u32::MAX);
    PoolLimitsShown {
        cpus,
        memory_mib: mib(limits.memory_mib()),
        processes: limits.pids(),
        tmp_mib: mib(limits.tmp_mib()),
    }
}

#[cfg(unix)]
fn row(pool: &Pool) -> PoolRow {
    PoolRow {
        name: pool.name().to_owned(),
        image: pool.image().as_str().to_owned(),
        limits: shown(pool.limits()),
    }
}

/// `typed`, a number of CPUs to the thousandth (`2`, `0.5`, `1.25`), in thousandths.
pub fn millicpus_of(typed: &str) -> Result<u32, String> {
    let not = || {
        format!(
            "\"{typed}\" is not a number of CPUs; write one such as 2 or 1.5, to the thousandth"
        )
    };
    let typed = typed.trim();
    let (whole, part) = typed.split_once('.').unwrap_or((typed, "0"));
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || !digits(part) || part.len() > 3 {
        return Err(not());
    }
    let whole: u32 = whole.parse().map_err(|_| not())?;
    let part: u32 = format!("{part:0<3}").parse().map_err(|_| not())?;
    whole
        .checked_mul(1000)
        .and_then(|whole| whole.checked_add(part))
        .ok_or_else(not)
}

#[cfg(unix)]
/// The pool `adding` names, held to the core's rules.
pub fn pool_of(adding: &PoolAdding) -> Result<Pool, String> {
    let image = Image::pinned(adding.image.trim()).map_err(|why| why.0)?;
    let set = Set {
        millicpus: adding
            .cpus
            .as_deref()
            .filter(|cpus| !cpus.trim().is_empty())
            .map(millicpus_of)
            .transpose()?,
        memory_mib: adding.memory_mib.map(u64::from),
        processes: adding.processes,
        tmp_mib: adding.tmp_mib.map(u64::from),
    };
    Pool::new(adding.name.trim(), image, set).map_err(|why| why.to_string())
}

#[cfg(unix)]
fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "There is no config home on this machine to keep a pool in.".to_owned())
}

#[cfg(unix)]
/// The pools on `config`.
pub fn pools_on(config: &std::path::Path) -> Result<PoolsShown, String> {
    Ok(PoolsShown {
        pools: purlis_core::runners::pools(config)
            .map_err(|why| why.to_string())?
            .iter()
            .map(row)
            .collect(),
        defaults: shown(Limits::default()),
        yours: Some(crate::poolprofiles::owned().is_ok()),
    })
}

/// **The Docker pools on this machine**, and a new pool's default limits.
#[tauri::command]
#[specta::specta]
pub async fn docker_pools() -> Result<PoolsShown, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("reading this machine's pools", || pools_on(&config_root()?)).await
    }
    #[cfg(not(unix))]
    {
        Err(NOT_HERE.to_owned())
    }
}

/// **Adds a Docker pool on this machine**: its runner is made from the image, confined and held
/// to the pool's limits, paired with this desktop and kept. What fails is taken back.
#[tauri::command]
#[specta::specta]
pub async fn add_docker_pool(adding: PoolAdding) -> Result<PoolRow, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("adding a pool", move || {
            let pool = pool_of(&adding)?;
            let machine = Machine::here(&config_root()?)?;
            add_on(&machine, &pool)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(NOT_HERE.to_owned())
    }
}

#[cfg(unix)]
/// [`add_docker_pool`], on `machine`.
pub fn add_on(machine: &Machine, pool: &Pool) -> Result<PoolRow, String> {
    let desktop = machine.desktop_made()?.pinned();
    purlis_core::pools::add(&machine.config, &machine.docker, pool, &desktop)
        .map_err(|why| refused_said(&why.to_string()))?;
    Ok(row(pool))
}

/// **Removes the pool `name`**: this desktop is taken out of its runner's peers over a link of
/// its own while the runner answers, then its container and home volume are removed, and the
/// pool with them. What could not be removed is answered, with how to remove it; nothing when
/// all of it is gone.
#[tauri::command]
#[specta::specta]
pub async fn remove_docker_pool(name: String) -> Result<Option<String>, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("removing a pool", move || {
            let machine = Machine::here(&config_root()?)?;
            remove_on(&machine, &name)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(NOT_HERE.to_owned())
    }
}

#[cfg(unix)]
/// [`remove_docker_pool`], on `machine`.
pub fn remove_on(machine: &Machine, name: &str) -> Result<Option<String>, String> {
    // Each runner of the pool is unpaired over a link to it, a draining one too (#2749).
    let unpair = |kept: &purlis_core::runners::Kept| {
        let to = ToARunner::of_kept(machine, kept)?;
        crate::runnerlinks::unpair(&to)
    };
    let removed = purlis_core::pools::remove(&machine.config, &machine.docker, name, Some(&unpair))
        .map_err(|why| refused_said(&why.to_string()))?;
    // The projects that ran their chats on it run them on no pool now: their marks go (#2846).
    crate::notheld::recheck(&machine.config);
    // Nothing of it waits on chats any more (#2865).
    crate::poolfollow::forget_waiting(&machine.config, name);
    Ok(left_said(removed.and_then(|removed| removed.left())))
}

/// **Whether the pool `name`'s runner answers with the key pinned for it**: one link, made and
/// let go of.
#[tauri::command]
#[specta::specta]
pub async fn docker_pool_link(name: String) -> Result<PoolLink, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("asking a pool's runner", move || {
            let machine = Machine::here(&config_root()?)?;
            let kept = purlis_core::runners::runner_for(&machine.config, &name)
                .map_err(|why| why.to_string())?;
            if matches!(machine.running(&kept), Ok(false)) {
                return Ok(PoolLink::Suspended);
            }
            crate::runnerlinks::link_state(&ToARunner::of_kept(&machine, &kept)?)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(NOT_HERE.to_owned())
    }
}

/// **Which version the pool `name`'s runner runs, against this desktop's** (#2749).
#[tauri::command]
#[specta::specta]
pub async fn docker_pool_version(name: String) -> Result<PoolVersion, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("asking a pool's runner its version", move || {
            version_on(&Machine::here(&config_root()?)?, &name)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(NOT_HERE.to_owned())
    }
}

#[cfg(unix)]
/// [`docker_pool_version`], on `machine`.
pub fn version_on(machine: &Machine, name: &str) -> Result<PoolVersion, String> {
    let kept =
        purlis_core::runners::runner_for(&machine.config, name).map_err(|why| why.to_string())?;
    let pool = purlis_core::runners::pools(&machine.config)
        .map_err(|why| why.to_string())?
        .into_iter()
        .find(|pool| pool.name() == name)
        .ok_or_else(|| format!("no pool named {name} is on this machine"))?;
    let draining = purlis_core::runners::draining(&machine.config)
        .map_err(|why| why.to_string())?
        .iter()
        .filter(|kept| kept.pool() == name)
        .count();
    let (runner, why) = version_read(crate::runnerlinks::version_of(machine, &kept));
    Ok(PoolVersion {
        runner,
        why,
        desktop: crate::runnerlinks::desktop_version().to_owned(),
        follows: pool.image().is_purlis_own(),
        draining: u32::try_from(draining).unwrap_or(u32::MAX),
        named: crate::poolfollow::this_versions_image_named(),
        waiting: crate::poolfollow::waiting_on(&machine.config, name),
    })
}

/// **Why adding or removing a pool was refused**, shown safely
/// ([`crate::devices::shown_safely`]): the reason may be Docker's or the runner's words.
pub fn refused_said(why: &str) -> String {
    crate::devices::shown_safely(why)
}

/// **What a removal says it left**, shown safely ([`crate::devices::shown_safely`]): Docker's
/// words for what it could not remove.
pub fn left_said(left: Option<String>) -> Option<String> {
    left.map(|left| crate::devices::shown_safely(&left))
}

/// **A runner's version as a row says it**, or why it could not be read, shown safely
/// ([`crate::devices::shown_safely`]): the reason is Docker's words.
pub fn version_read(read: Result<String, String>) -> (Option<String>, Option<String>) {
    match read {
        Ok(version) => (Some(version), None),
        Err(why) => (None, Some(crate::devices::shown_safely(&why))),
    }
}

#[cfg(test)]
mod tests;

/// **Where a workspace's chats run on this machine** (#2046): the pool its chats run on, `None`
/// for this machine, and every pool this machine keeps, which the setting offers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatsRunOn {
    pub pool: Option<String>,
    pub pools: Vec<String>,
}

#[cfg(unix)]
/// Where `workspace` of the plane at `plane` runs its chats on `config`.
pub fn chats_run_on_at(
    config: &std::path::Path,
    plane: &std::path::Path,
    workspace: &str,
) -> Result<ChatsRunOn, String> {
    let pools = purlis_core::runners::pools(config)
        .map_err(|why| why.to_string())?
        .iter()
        .map(|pool| pool.name().to_owned())
        .collect();
    let loaded = purlis_core::machine::read(config);
    Ok(ChatsRunOn {
        pool: loaded
            .store
            .chats_run_on(plane, workspace)
            .map(str::to_owned),
        pools,
    })
}

#[cfg(unix)]
/// Sets where `workspace` of the plane at `plane` runs its chats on `config`: on `pool`, which
/// this machine must keep with its runner, or on this machine for `None`. A chat running now
/// stays where it runs; the next starts where this says.
pub fn set_chats_run_on_at(
    config: &std::path::Path,
    plane: &std::path::Path,
    workspace: &str,
    pool: Option<&str>,
) -> Result<ChatsRunOn, String> {
    if let Some(pool) = pool {
        purlis_core::runners::runner_for(config, pool).map_err(|why| format!("{why}."))?;
    }
    let mut set = Ok(false);
    purlis_core::machine::update(config, |store| {
        set = store.set_chats_run_on(plane, workspace, pool);
    })
    .map_err(|why| format!("Where this workspace's chats run was not kept: {why}"))?;
    set?;
    chats_run_on_at(config, plane, workspace)
}

/// **Sets where `workspace` of the plane at `plane` runs its chats** to `place` (#2048), as a
/// chat of it moves there: the setting the person can change in the workspace's settings.
#[cfg(unix)]
pub(crate) fn run_chats_on(
    config: &std::path::Path,
    plane: &std::path::Path,
    workspace: &str,
    place: &Option<String>,
) -> Result<(), String> {
    set_chats_run_on_at(config, plane, workspace, place.as_deref()).map(|_| ())
}

/// No pool runs chats here.
#[cfg(not(unix))]
pub(crate) fn run_chats_on(
    _config: &std::path::Path,
    _plane: &std::path::Path,
    _workspace: &str,
    _place: &Option<String>,
) -> Result<(), String> {
    Err("Pools run chats on macOS and Linux only.".to_owned())
}

/// **Where a workspace's chats run** (#2046), as its settings show it.
#[tauri::command]
#[specta::specta]
pub async fn chats_run_on(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: String,
) -> Result<ChatsRunOn, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("reading where the workspace's chats run", move || {
            chats_run_on_at(&config_root()?, &root, &workspace)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (root, workspace);
        Err(NOT_HERE.to_owned())
    }
}

/// **What a workspace's sends to a pool's runner leave out** besides what a save to the forge
/// refuses (#2046, the operator's ruling of 2026-10-25): `settings.runners.leave_out` in its
/// `workspace.json`, as its settings show it.
#[tauri::command]
#[specta::specta]
pub async fn send_leaves_out(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: String,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("reading what the workspace's sends leave out", move || {
        purlis_core::settings::workspace::send_leaves_out(&root, &workspace)
    })
    .await
}

/// **Sets what a workspace's sends to a pool's runner leave out** (#2046): the person's list in
/// its settings, one file or folder in a clone per entry. Answers what it holds after.
#[tauri::command]
#[specta::specta]
pub async fn set_send_leaves_out(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: String,
    entries: Vec<String>,
) -> Result<Vec<String>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    crate::off_the_window("setting what the workspace's sends leave out", move || {
        purlis_core::settings::workspace::set_send_leaves_out(&root, &workspace, &entries)
    })
    .await
}

/// **Sets where a workspace's chats run** (#2046): the person's choice in its settings.
#[tauri::command]
#[specta::specta]
pub async fn set_chats_run_on(
    planes: tauri::State<'_, crate::planes::Planes>,
    plane: crate::planes::PlaneId,
    workspace: String,
    pool: Option<String>,
) -> Result<ChatsRunOn, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("setting where the workspace's chats run", move || {
            set_chats_run_on_at(&config_root()?, &root, &workspace, pool.as_deref())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (root, workspace, pool);
        Err(NOT_HERE.to_owned())
    }
}
