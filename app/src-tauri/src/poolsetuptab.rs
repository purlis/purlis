// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Pool setup tab's commands** (spec #4469, #4480): what `purlis_core::poolsetup` answers, as
//! the window reads it, and the choices the person makes there. Reading changes nothing here;
//! each change happens on its own button.

use crate::dockerpools::PoolAdding;
use crate::planes::{PlaneId, Planes};

/// One step's next action.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupAction {
    pub id: String,
    pub label: String,
    pub does: String,
    pub command: Option<String>,
    pub url: Option<String>,
}

/// A step by hand: the exact commands, and what running them means (#4481).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupSee {
    pub note: String,
    pub text: String,
}

/// One line of what Try it checks (#4483): `ok`, `missing`, `todo` or `skip`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupCheck {
    pub verdict: String,
    pub title: String,
    pub more: Option<String>,
}

/// One step as it stands.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupStep {
    pub id: String,
    pub name: String,
    pub state: String,
    pub line: Option<String>,
    pub findings: Vec<String>,
    pub next: Option<PoolSetupAction>,
    pub skippable: bool,
    pub see: Option<PoolSetupSee>,
    pub checks: Vec<PoolSetupCheck>,
}

/// What the tab draws.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupShown {
    /// The pool this setup made.
    pub pool: Option<String>,
    /// The name typed on the Pool step, for the box to hold.
    pub name: Option<String>,
    /// The step the person has reached.
    pub at: Option<String>,
    pub steps: Vec<PoolSetupStep>,
    /// The pool was made by hand under the name typed, and purlis noticed (#4481).
    pub noticed: bool,
    /// Pools this machine keeps that this setup isn't on.
    pub others: Vec<String>,
}

/// What the Pool step's button did: nothing made, and why, where a team's image breaks the
/// contract; none where the pool was made.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupCreated {
    pub breaks: Vec<String>,
}

/// A core value's word, as it serializes.
#[cfg(unix)]
fn word(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|said| said.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// What the core answered, as the window takes it.
#[cfg(unix)]
pub fn shown(found: purlis_core::poolsetup::Resumed) -> PoolSetupShown {
    let purlis_core::poolsetup::Resumed {
        setup,
        name,
        noticed,
    } = found;
    PoolSetupShown {
        pool: setup.pool.clone(),
        name,
        noticed,
        others: setup.others.clone(),
        at: setup.at.as_ref().map(word),
        steps: setup
            .steps
            .iter()
            .map(|step| PoolSetupStep {
                id: word(&step.id),
                name: step.name.to_owned(),
                state: word(&step.state),
                line: step.line.clone(),
                findings: step.findings.clone(),
                next: step.next.as_ref().map(|next| PoolSetupAction {
                    id: word(&next.id),
                    label: next.label.clone(),
                    does: next.does.clone(),
                    command: next.command.clone(),
                    url: next.url.clone(),
                }),
                skippable: step.skippable,
                see: step.see.as_ref().map(|see| PoolSetupSee {
                    note: see.note.clone(),
                    text: see.text.clone(),
                }),
                checks: step
                    .checks
                    .iter()
                    .map(|check| PoolSetupCheck {
                        verdict: word(&check.verdict),
                        title: check.title.clone(),
                        more: check.more.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(unix)]
fn config_root() -> Result<std::path::PathBuf, String> {
    purlis_core::machine::config_root()
        .ok_or_else(|| "There is no config home on this machine to keep a pool in.".to_owned())
}

/// **Each step's state, read now**: the pool this setup made, else a new one's.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<PoolSetupShown, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("reading Pool setup", move || {
            use purlis_core::dockerpool::Docker;
            use purlis_core::poolsetup::{self, Here};
            let config = config_root()?;
            let docker = poolsetup::docker_here();
            let here = Here {
                config: &config,
                plane: Some(&root),
                docker: docker.as_ref(),
                engine: Docker::engine_on_this_machine(),
            };
            Ok(shown(
                poolsetup::resumed(&here).map_err(|why| why.to_string())?,
            ))
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that the person chose Docker on Where.** Nothing else changes.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_choose_docker() -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", || {
            purlis_core::poolsetup::choose_kind(
                &config_root()?,
                purlis_core::poolsetup::Kind::Docker,
            )
            .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps the name typed on the Pool step**, or none, so coming back finds it.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_name(name: Option<String>) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            purlis_core::poolsetup::choose_name(&config_root()?, name.as_deref())
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that this setup goes on with pool `name`**, once it has made it.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_pool(name: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            purlis_core::poolsetup::choose_pool(&config_root()?, &name)
                .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = name;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **The command that does what the Pool step's button would do with `adding`**: See what this
/// does, from the value the button sends (Q30). It reads nothing and changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_command(adding: PoolAdding) -> Result<String, String> {
    #[cfg(unix)]
    {
        Ok(adding.asked().command())
    }
    #[cfg(not(unix))]
    {
        let _ = adding;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Makes the pool `adding` asks for**, on the Pool step's own button: what `purlis pool add`
/// runs, a team's image held to the contract first.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_create(adding: PoolAdding) -> Result<PoolSetupCreated, String> {
    #[cfg(unix)]
    {
        crate::off_the_window("adding a pool", move || {
            use purlis_core::poolsetup::asked::{self, Created};
            let machine = crate::runnerlinks::Machine::here(&config_root()?)?;
            let desktop = machine.desktop_made()?.pinned();
            let made = asked::create(&machine.config, &machine.docker, &adding.asked(), &desktop)
                .map_err(|why| crate::dockerpools::refused_said(&why.to_string()))?;
            Ok(PoolSetupCreated {
                breaks: match made {
                    Created::Added => Vec::new(),
                    Created::Breaks(problems) => problems,
                },
            })
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = adding;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// One of this machine's profiles against a pool's limit (#4482).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupProfileSize {
    pub name: String,
    /// What a send to the runner carries, in MiB rounded up, and in files; null where unread.
    pub mib: Option<u32>,
    pub files: Option<u32>,
    /// Why it could not be measured.
    pub unread: Option<String>,
    /// Each limit it is over, in words, with what to raise it to.
    pub over: Vec<PoolSetupOver>,
    /// The project's default, or a profile an open chat runs on.
    pub in_use: bool,
}

/// A limit a profile is over.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupOver {
    /// The limit's key, as `pool_limit_set` takes it.
    pub key: String,
    pub said: String,
    /// What to raise it to, where a value would do.
    pub raise_to: Option<u32>,
}

/// This machine's profiles against pool `pool`'s limits (#4482).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolSetupSizes {
    pub most_mib: u32,
    pub most_files: u32,
    pub profiles: Vec<PoolSetupProfileSize>,
}

/// A count as the window takes it.
#[cfg(unix)]
fn small(value: u64) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// **This machine's profiles against pool `pool`'s limits**, the ones in use in `plane` marked:
/// measured as a send measures them, with no file read. It changes nothing.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_sizes(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    pool: String,
) -> Result<PoolSetupSizes, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    #[cfg(unix)]
    {
        crate::off_the_window("measuring your profiles", move || {
            let sizes = purlis_core::poolsetup::signins::sizes(&config_root()?, Some(&root), &pool)
                .map_err(|why| why.to_string())?;
            Ok(PoolSetupSizes {
                most_mib: small(sizes.most_mib),
                most_files: small(sizes.most_files),
                profiles: sizes
                    .profiles
                    .into_iter()
                    .map(|one| PoolSetupProfileSize {
                        name: one.name,
                        mib: one.mib.map(small),
                        files: one.files.map(small),
                        unread: one.unread,
                        over: one
                            .hits
                            .iter()
                            .map(|hit| PoolSetupOver {
                                key: hit.limit.limit().key.to_owned(),
                                said: hit.said(),
                                raise_to: hit.suggested.map(small),
                            })
                            .collect(),
                        in_use: one.in_use,
                    })
                    .collect(),
            })
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (root, pool);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Keeps that the person skipped step `step` on pool `pool`, or takes the skip back.** Refused
/// for a step the ones after it need.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_skip(pool: String, step: String, skipped: bool) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("keeping your choice", move || {
            use purlis_core::poolsetup::{self, StepId};
            let step: StepId = serde_json::from_value(serde_json::Value::String(step))
                .map_err(|_| "Pool setup has no step of that name.".to_owned())?;
            let config = config_root()?;
            if skipped {
                poolsetup::skip(&config, &pool, step)
            } else {
                poolsetup::unskip(&config, &pool, step)
            }
            .map_err(|why| why.to_string())
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (pool, step, skipped);
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Starts pool `pool`'s runner again**, on the Sign-ins step's own button: one a day without a
/// link stopped its own container, which is checked as it was made before it starts, as a link to
/// it would start it. It goes back to sleep by itself.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_wake(pool: String) -> Result<(), String> {
    #[cfg(unix)]
    {
        crate::off_the_window("waking the runner", move || {
            let machine = crate::runnerlinks::Machine::here(&config_root()?)?;
            let kept = purlis_core::runners::runner_for(&machine.config, &pool)
                .map_err(|why| why.to_string())?;
            crate::runnerlinks::checked_or_woken(&machine.docker, &machine.config, &kept, true)
                .map_err(|why| why.0)
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = pool;
        Err(crate::dockerpools::NOT_HERE.to_owned())
    }
}

/// **Opens the Docker app**, on the person's press, where this system has one to open. Refused
/// elsewhere, with what to do instead.
#[tauri::command]
#[specta::specta]
pub async fn pool_setup_open_docker() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        crate::off_the_window("opening Docker", || {
            let opened = purlis_core::forklock::output(
                std::process::Command::new("/usr/bin/open").args(["-a", "Docker"]),
            )
            .map_err(|why| why.to_string())?;
            if opened.status.success() {
                Ok(())
            } else {
                Err("purlis couldn't open Docker. Open it the way you usually do.".to_owned())
            }
        })
        .await
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("purlis can't open Docker here. Start it the way you usually do.".to_owned())
    }
}
