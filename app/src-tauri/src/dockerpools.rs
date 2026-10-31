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
use purlis_core::dockerpool::Missing;
use purlis_core::dockerpool::{Image, Limits};
#[cfg(unix)]
use purlis_core::runners::{Pool, Set};
#[cfg(unix)]
use purlis_session_protocol::browsing::LinkFrames;

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
    /// How long its runner is left without a link from this machine before it suspends
    /// itself, as `runners.json` spells it (`24h`, `90m`; #3471).
    pub suspend_after: String,
}

/// What the Pools group draws: the pools, and the limits a new pool gets by default.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolsShown {
    pub pools: Vec<PoolRow>,
    pub defaults: PoolLimitsShown,
    /// How long a new pool's runner is left without a link before it suspends itself, by
    /// default: a day (`24h`, #3471).
    pub suspend_after: String,
    /// Whether this machine's pools' runners are shown to be the person's own (D-2050-4): their
    /// engine is a local socket on this machine. A chat moves only to such a pool (#2048).
    #[specta(optional)]
    pub yours: Option<bool>,
    /// The most frames a second a new pool's runner's browsers are watched at over the link, by
    /// default (#3297).
    #[specta(optional)]
    pub frames: Option<u32>,
    /// The widest frame a new pool's runner's browsers are watched at over the link, in pixels,
    /// by default (#3297).
    #[specta(optional)]
    pub width: Option<u32>,
    /// This machine's cluster pools (#3292), each with its runner in a team's cluster.
    #[specta(optional)]
    pub clusters: Option<Vec<crate::clusterpools::ClusterPoolRow>>,
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
    /// How long its runner is left without a link before it suspends itself, spelled as
    /// `runners.json` spells it (`12h`); empty is a day (#3471).
    pub suspend_after: Option<String>,
    /// The most frames a second its runner's browsers are watched at over the link (#3297).
    pub frames: Option<u32>,
    /// The widest frame its runner's browsers are watched at over the link, in pixels (#3297).
    pub width: Option<u32>,
}

/// **The runner image a new pool is offered** (the person's ruling of 2026-10-30), by the
/// release that names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunnerImageOffered {
    /// This version's own, as its release names it.
    Ours { image: String },
    /// A newer version's, as the channel's newest release names it: an update of purlis to
    /// `version` is waiting.
    Newer { image: String, version: String },
    /// None: this version's release names none, as a release whose runner image was not built
    /// does.
    NoneNamed,
    /// None: a local build, with no update channel, names none.
    LocalBuild,
}

/// Where a pool's runner stands, as one link to it says.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum PoolLink {
    /// It answered with the key pinned at its pairing.
    Linked,
    /// It answered with another key: it is refused, and the window offers to remove it.
    KeyChanged,
    /// Its cluster's Pool names another cluster operator key than the one pinned as the pool was
    /// added (#3292): its runner is refused, and the window offers to remove the pool.
    OperatorKeyChanged,
    /// It did not answer, and why.
    Down { why: String },
    /// It suspended itself, a day without a link (#2051): its container is stopped, its home
    /// kept, and the window's next chat there starts it again. A check never wakes it.
    Suspended,
    /// It was removed outside purlis (#4149): Docker has no container of its id, so no link to
    /// it comes back, and the window offers Make the runner again. Whether its home volume, with
    /// its sign-ins, is still there for the runner made again to take them from.
    Removed { home_kept: bool },
    /// Docker points at another engine than the one that made it (#4166): its container may be
    /// there still, so it is not said to be removed and Make the runner again is not offered.
    OnAnotherEngine,
}

/// **A pool whose runner was removed outside purlis** (#4149): what the Notice of a refused
/// start names, with Make the runner again.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolRemoved {
    pub name: String,
    /// Its home volume, with its sign-ins, is still there.
    pub home_kept: bool,
}

/// **What Make the runner again did** (#4149): whether the sign-ins of the runner removed
/// outside purlis were carried to the runner made again; where not, its profiles are signed in
/// again.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct MadeAgain {
    pub sign_ins_kept: bool,
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
    /// How its runner was made to suspend itself, as its container's command says (#3717): a
    /// runner made before its pool's `suspendAfter` keeps what it was made with. None where it
    /// could not be read.
    #[specta(optional)]
    pub suspends: Option<PoolSuspends>,
}

/// **How a pool's runner was made to suspend itself** (#3717), which its row says where it is
/// not the pool's own time.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "made", rename_all = "snake_case")]
pub enum PoolSuspends {
    /// It was made before purlis suspended runners (#2051), and never suspends itself.
    Never,
    /// It suspends itself after this long without a link from this machine, spelled as
    /// `runners.json` spells it (`24h`, `90m`).
    After { after: String },
}

#[cfg(unix)]
/// `made`, as a row says it.
pub(crate) fn suspends_shown(made: purlis_core::dockerpool::MadeSuspending) -> PoolSuspends {
    match made {
        purlis_core::dockerpool::MadeSuspending::Never => PoolSuspends::Never,
        purlis_core::dockerpool::MadeSuspending::After(after) => PoolSuspends::After {
            after: after_shown(after),
        },
    }
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
        suspend_after: after_shown(pool.suspend_after()),
    }
}

/// How long a runner is left without a link before it suspends, as `runners.json` spells it.
#[cfg(unix)]
fn after_shown(after: std::time::Duration) -> String {
    purlis_core::spelledtime::spelled(after, purlis_core::spelledtime::UP_TO_HOURS)
        .unwrap_or_else(|| format!("{}s", after.as_secs()))
}

/// `typed`, how long a pool's runner is left without a link before it suspends (`12h`, `90m`).
pub fn suspend_after_of(typed: &str) -> Result<std::time::Duration, String> {
    let typed = typed.trim();
    let after = purlis_core::spelledtime::read(typed, purlis_core::spelledtime::UP_TO_HOURS)
        .ok_or_else(|| {
            format!(
                "\"{typed}\" is not a time; write a whole number followed by h, m or s, such as \
                 12h or 90m"
            )
        })?;
    purlis_core::dockerpool::suspend_after(after).map_err(|why| why.0)
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
        suspend_after: adding
            .suspend_after
            .as_deref()
            .filter(|after| !after.trim().is_empty())
            .map(suspend_after_of)
            .transpose()?,
        frames: adding.frames,
        width: adding
            .width
            .map(|width| {
                u16::try_from(width).map_err(|_| {
                    let widest = purlis_session_protocol::browsing::LinkFrames::WIDTH;
                    format!(
                        "a pool's widest browser frame is {} to {} pixels",
                        widest.start(),
                        widest.end()
                    )
                })
            })
            .transpose()?,
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
        suspend_after: after_shown(purlis_core::dockerpool::SUSPEND_AFTER),
        yours: Some(crate::poolprofiles::owned().is_ok()),
        frames: Some(LinkFrames::DEFAULT.frames),
        width: Some(u32::from(LinkFrames::DEFAULT.width)),
        clusters: Some(
            purlis_core::runners::cluster_pools(config)
                .map_err(|why| why.to_string())?
                .iter()
                .map(crate::clusterpools::row)
                .collect(),
        ),
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
/// all of it is gone. Once it is gone from this machine, the window's links to its runner end, and
/// the chats on them with them ([`links_end_once_gone`]).
#[tauri::command]
#[specta::specta]
pub async fn remove_docker_pool(
    app: tauri::AppHandle,
    name: String,
) -> Result<Option<String>, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("removing a pool", move || {
            let config = config_root()?;
            // A cluster pool's runner is its claim, deleted through kubectl (#3292).
            let removed = if is_a_cluster_pool(&config, &name)? {
                crate::clusterpools::remove_on(&config, &name)
            } else {
                remove_on(&Machine::here(&config)?, &name)
            };
            if let Some(host) = tauri::Manager::try_state::<crate::sessionhost::SessionHost>(&app) {
                links_end_once_gone(&host.connection(), &config, &name);
            }
            removed
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, name);
        Err(NOT_HERE.to_owned())
    }
}

/// **The pool `name`, as a removal left it**: gone from `config`, the window's links to its runner
/// end. One whose runner could not be removed is still kept, and its chats with it. Where that
/// cannot be read, the links end: a chat lost is better than a link kept to a pool removed.
#[cfg(unix)]
fn links_end_once_gone(
    connection: &crate::hostedsessions::Connection,
    config: &std::path::Path,
    name: &str,
) {
    let read = purlis_core::runners::pools(config)
        .map(|pools| pools.iter().any(|pool| pool.name() == name))
        .and_then(|docker| {
            purlis_core::runners::cluster_pools(config)
                .map(|clusters| docker || clusters.iter().any(|pool| pool.name() == name))
        });
    match read {
        Ok(true) => {}
        Ok(false) => connection.pool_removed(name),
        Err(why) => {
            tracing::warn!("purlis: what this machine keeps of its pools was not read: {why}");
            connection.links_to_pool_end(name);
        }
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
            let config = config_root()?;
            if is_a_cluster_pool(&config, &name)? {
                return crate::clusterpools::link_on(&config, &name);
            }
            let machine = Machine::here(&config)?;
            let kept = purlis_core::runners::runner_for(&machine.config, &name)
                .map_err(|why| why.to_string())?;
            match machine.running(&kept) {
                Ok(false) => return Ok(PoolLink::Suspended),
                Ok(true) => {}
                // Docker has no container of its id: removed outside purlis (#4149).
                Err(_) => match machine.docker.missing(&kept.made, kept.engine.as_deref()) {
                    Ok(Some(Missing::Removed(gone))) => {
                        return Ok(PoolLink::Removed {
                            home_kept: gone.home_kept,
                        });
                    }
                    Ok(Some(Missing::OnAnotherEngine)) => {
                        return Ok(PoolLink::OnAnotherEngine);
                    }
                    Ok(None) | Err(_) => {}
                },
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

/// **The Docker pools whose runner was removed outside purlis** (#4149): Docker has no
/// container of its id. What a refused start's Notice offers to make again; a pool whose Docker
/// does not answer is not among them.
#[tauri::command]
#[specta::specta]
pub async fn docker_pools_removed() -> Result<Vec<PoolRemoved>, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("asking which pools' runners were removed", move || {
            removed_on(&Machine::here(&config_root()?)?)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(NOT_HERE.to_owned())
    }
}

#[cfg(unix)]
/// [`docker_pools_removed`], on `machine`.
pub fn removed_on(machine: &Machine) -> Result<Vec<PoolRemoved>, String> {
    let pools = purlis_core::runners::pools(&machine.config).map_err(|why| why.to_string())?;
    Ok(pools
        .iter()
        .filter_map(|pool| {
            let kept = purlis_core::runners::runner_for(&machine.config, pool.name()).ok()?;
            match machine
                .docker
                .missing(&kept.made, kept.engine.as_deref())
                .ok()??
            {
                Missing::Removed(gone) => Some(PoolRemoved {
                    name: pool.name().to_owned(),
                    home_kept: gone.home_kept,
                }),
                Missing::OnAnotherEngine => None,
            }
        })
        .collect())
}

/// **Make the runner again** (#4149): the pool `name`'s runner, removed outside purlis, made
/// again from the pool's own image and limits, with the sign-ins of its home where that is still
/// there, and paired. The window then has it keep the pool's profiles, as after Add a pool. Refused for
/// a runner Docker still has, which resumes as it is.
#[tauri::command]
#[specta::specta]
pub async fn docker_pool_make_again(name: String) -> Result<MadeAgain, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("making a pool's runner again", move || {
            make_again_on(&Machine::here(&config_root()?)?, &name)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(NOT_HERE.to_owned())
    }
}

#[cfg(unix)]
/// [`docker_pool_make_again`], on `machine`.
pub fn make_again_on(machine: &Machine, name: &str) -> Result<MadeAgain, String> {
    let desktop = machine.desktop()?.pinned();
    let again = purlis_core::pools::make_again(&machine.config, &machine.docker, name, &desktop)
        .map_err(|why| refused_said(&why.to_string()))?;
    Ok(MadeAgain {
        sign_ins_kept: again.sign_ins_kept,
    })
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

/// **The runner image a new pool is offered** (the person's ruling of 2026-10-30): what the Add
/// a pool dialog's Image box shows, and what a pool added with the box empty is made from.
/// Refused, in the updater's words, where it could not be read or was not signed as a release
/// signs it.
#[tauri::command]
#[specta::specta]
pub async fn runner_image_of_this_version(
    app: tauri::AppHandle,
) -> Result<RunnerImageOffered, String> {
    #[cfg(unix)]
    {
        crate::poolfollow::image_offered(&app).await
    }
    #[cfg(not(unix))]
    {
        let _ = app;
        Err(NOT_HERE.to_owned())
    }
}

#[cfg(unix)]
/// Whether `name` is a cluster pool on `config` (#3292): the pool commands answer for either
/// kind, as the Pools group lists both.
fn is_a_cluster_pool(config: &std::path::Path, name: &str) -> Result<bool, String> {
    Ok(purlis_core::runners::cluster_pools(config)
        .map_err(|why| why.to_string())?
        .iter()
        .any(|pool| pool.name() == name))
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
        suspends: machine
            .docker
            .made_suspending(&kept.made)
            .ok()
            .flatten()
            .map(suspends_shown),
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
    // A cluster pool's name too (#3292).
    let pools = purlis_core::runners::pool_names(config).map_err(|why| why.to_string())?;
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
        purlis_core::runners::runner_of(config, pool).map_err(|why| format!("{why}."))?;
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
