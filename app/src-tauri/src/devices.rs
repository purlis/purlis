// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Devices view** (#2052, spec #2041 story 54): this machine and every runner it keeps, each
//! with its chats and browsers, its version, how it is and when it last answered. It replaces the
//! Runners view ADR 0089 §12 described (V50), which was never drawn.
//!
//! - **Where each chat and browser is** is where the window started it and keeps it
//!   ([`placed`]): a chat's pool as the router kept it (`pool_of`), so a chat moved since is
//!   placed where it runs now; a browser's as its chat runs it (#2050). A pool reaches its
//!   runner that takes new chats; `<pool>/<runner>` names that runner, a draining one too
//!   (#2865). A chat on a pool this machine no longer keeps is said apart, never put on another
//!   device.
//! - **Each runner is asked how it is on a link of its own** ([`device_runner`]), as the Pools
//!   settings ask theirs: whether its container runs, and only then whether it answers with the
//!   key pinned for it, which version it runs and what it holds. **A suspended runner is never
//!   woken to be asked** (#2051): its container is read, and nothing links to it.
//! - **What a runner holds that the window has not open** (a project open in no window, a chat
//!   closed here that runs on) is counted apart ([`not_open_here`]), from the runner's own list,
//!   so nothing it runs goes unsaid.
//! - **When a runner last answered** is what this window heard ([`Contacts`]): kept in memory for
//!   as long as the app runs, so a runner not reached since it started says so rather than
//!   showing a time nobody saw.
//!
//! The window's alone (`ui::WINDOW_ONLY`): the view links to runners on the person's machine.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Mutex, PoisonError};

use crate::dockerpools::{PoolLimitsShown, PoolLink};
use crate::planes::PlaneId;

/// One runner this machine keeps, as the view lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptRunner {
    pub pool: String,
    pub runner: String,
    pub draining: bool,
}

/// One chat the window has open, and the pool it runs on, as the router keeps it: `None` for
/// this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub plane: PlaneId,
    pub session: u32,
    pub name: String,
    pub on: Option<String>,
}

/// One browser of a chat the window has open, and the pool its chat runs it on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserPlaced {
    pub plane: PlaneId,
    /// The chat it is of.
    pub session: u32,
    /// What the chat calls it: `b1`.
    pub name: String,
    /// What its chat is called, or was where it outlived its chat.
    pub chat: String,
    pub on: Option<String>,
}

/// A chat on a device, as its row names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatOnADevice {
    pub plane: PlaneId,
    pub session: u32,
    pub name: String,
}

/// A browser on a device, as its row names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BrowserOnADevice {
    pub plane: PlaneId,
    /// The chat it is of.
    pub session: u32,
    /// What the chat calls it: `b1`.
    pub name: String,
    /// What its chat is called.
    pub chat: String,
}

/// What runs on one device, of what the window has open.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OnADevice {
    pub chats: Vec<ChatOnADevice>,
    pub browsers: Vec<BrowserOnADevice>,
}

/// One runner, as its row lists it before it is asked how it is.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerOnTheList {
    pub pool: String,
    pub runner: String,
    /// It was replaced by a runner of this machine's version: it keeps its chats until they
    /// end, then purlis removes it (#2749).
    pub draining: bool,
    pub on: OnADevice,
}

/// A chat on a pool this machine no longer keeps.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatOnAPoolNotKept {
    pub plane: PlaneId,
    pub session: u32,
    pub name: String,
    /// The pool its chat was started on, as the router kept it.
    pub pool: String,
}

/// Where everything the window has open runs: this machine, each runner, and what is on a pool
/// no longer kept.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Placement {
    pub here: OnADevice,
    pub runners: Vec<RunnerOnTheList>,
    pub unkept: Vec<ChatOnAPoolNotKept>,
}

/// One pool's limits, which each of its runners that runs requests (#2052, story 55).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct PoolRequests {
    pub pool: String,
    /// What one runner of it is held to.
    pub limits: PoolLimitsShown,
    /// Its CPUs in thousandths, to be added up.
    pub millicpus: u32,
}

/// **What the Devices view draws first** (#2052): this machine's version, where everything the
/// window has open runs, and each pool's limits.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct DevicesShown {
    /// This machine's version.
    pub version: String,
    /// Where everything the window has open runs.
    pub placement: Placement,
    pub pools: Vec<PoolRequests>,
}

/// **Where each chat and browser the window has open is** (#2052): on this machine, on a runner
/// `kept` lists, or on a pool no longer kept. Every kept runner is listed, in `kept`'s order.
pub fn placed(kept: &[KeptRunner], chats: Vec<Placed>, browsers: Vec<BrowserPlaced>) -> Placement {
    let mut runners: Vec<RunnerOnTheList> = kept
        .iter()
        .map(|one| RunnerOnTheList {
            pool: one.pool.clone(),
            runner: one.runner.clone(),
            draining: one.draining,
            on: OnADevice::default(),
        })
        .collect();
    // A pool reaches its runner that takes new chats; `<pool>/<runner>` that runner itself.
    let at = |on: &str| -> Option<usize> {
        match on.split_once('/') {
            Some((pool, runner)) => kept
                .iter()
                .position(|one| one.pool == pool && one.runner == runner),
            None => kept.iter().position(|one| one.pool == on && !one.draining),
        }
    };
    let mut placement = Placement::default();
    for chat in chats {
        let shown = ChatOnADevice {
            plane: chat.plane,
            session: chat.session,
            name: chat.name,
        };
        match chat.on {
            None => placement.here.chats.push(shown),
            Some(pool) => match at(&pool) {
                Some(index) => runners[index].on.chats.push(shown),
                None => placement.unkept.push(ChatOnAPoolNotKept {
                    plane: shown.plane,
                    session: shown.session,
                    name: shown.name,
                    pool,
                }),
            },
        }
    }
    for browser in browsers {
        let shown = BrowserOnADevice {
            plane: browser.plane,
            session: browser.session,
            name: browser.name,
            chat: browser.chat,
        };
        match browser.on.as_deref().map(at) {
            None => placement.here.browsers.push(shown),
            Some(Some(index)) => runners[index].on.browsers.push(shown),
            // Its chat is said on a pool not kept: the browser goes with it, unsaid twice.
            Some(None) => {}
        }
    }
    placement.runners = runners;
    placement
}

/// Whether a runner's container runs (#2051).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RunnerStanding {
    Running,
    /// Its container stopped, its home kept: it starts again as a chat next needs it.
    Suspended,
    /// Its container could not be read, and why.
    Unknown {
        why: String,
    },
}

/// One program a runner holds, as its own list says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedProgram {
    /// Its id in the runner's host.
    pub id: String,
    /// The project it is of, by its stable id.
    pub project: String,
    /// Its number in that project, for a chat.
    pub number: Option<u32>,
    pub ended: bool,
    pub browser: bool,
}

/// What one runner answered, before it is set against what the window has open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub standing: RunnerStanding,
    /// What the link said; none where nothing linked to it.
    pub link: Option<PoolLink>,
    pub version: Option<String>,
    /// When it last answered, in seconds since 1970.
    pub contacted: Option<u64>,
    /// What it holds that has not ended.
    pub holds: Vec<ListedProgram>,
}

/// **What one runner answers** (#2052): its container read first (`running`); only a running one
/// is linked to (`list`), and asked its version (`version`). `last` is when it last answered
/// before, and `now` the time now, in seconds since 1970.
pub fn answered(
    running: Result<bool, String>,
    list: impl FnOnce() -> Result<Vec<ListedProgram>, PoolLink>,
    version: impl FnOnce() -> Option<String>,
    last: Option<u64>,
    now: u64,
) -> Answered {
    let standing = match running {
        Ok(true) => RunnerStanding::Running,
        Ok(false) => RunnerStanding::Suspended,
        Err(why) => RunnerStanding::Unknown { why },
    };
    if standing != RunnerStanding::Running {
        return Answered {
            standing,
            link: None,
            version: None,
            contacted: last,
            holds: Vec::new(),
        };
    }
    match list() {
        Ok(listed) => Answered {
            standing,
            link: Some(PoolLink::Linked),
            version: version(),
            contacted: Some(now),
            holds: listed.into_iter().filter(|one| !one.ended).collect(),
        },
        Err(link) => Answered {
            standing,
            link: Some(link),
            version: None,
            contacted: last,
            holds: Vec::new(),
        },
    }
}

/// The chats and browsers the window has open, by what a runner's list names them by: a chat by
/// its project's stable id and its number, a browser by its program's id.
#[derive(Debug, Clone, Default)]
pub struct OpenHere {
    /// Each chat, by its project's stable id and its number, and where the window has it.
    pub chats: BTreeMap<(String, u32), (PlaneId, u32)>,
    pub browsers: BTreeSet<String>,
}

/// A chat the window has open, by its plane and its session.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatHeld {
    pub plane: PlaneId,
    pub session: u32,
}

/// What a runner holds, set against what the window has open: the chats it has open, which the
/// row lists by what the runner says rather than where the window started them, and how many of
/// the chats and browsers it holds the window has not open.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct NotOpenHere {
    pub open: Vec<ChatHeld>,
    pub chats: u32,
    pub browsers: u32,
}

/// **What of `holds` the window has `open`, and how much it has not** (#2052): a chat of a
/// project open in no window, or one closed here that runs on, is counted.
pub fn not_open_here(holds: &[ListedProgram], open: &OpenHere) -> NotOpenHere {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    let held = |one: &ListedProgram| {
        one.number
            .and_then(|number| open.chats.get(&(one.project.clone(), number)))
    };
    NotOpenHere {
        open: holds
            .iter()
            .filter(|one| !one.browser)
            .filter_map(held)
            .map(|(plane, session)| ChatHeld {
                plane: plane.clone(),
                session: *session,
            })
            .collect(),
        chats: count(
            holds
                .iter()
                .filter(|one| !one.browser && held(one).is_none())
                .count(),
        ),
        browsers: count(
            holds
                .iter()
                .filter(|one| one.browser && !open.browsers.contains(&one.id))
                .count(),
        ),
    }
}

/// **When each runner last answered this window**, by `<pool>/<runner>`, for as long as the app
/// runs.
#[derive(Debug, Default)]
pub struct Contacts(Mutex<HashMap<String, u64>>);

impl Contacts {
    /// `runner` answered at `at`, in seconds since 1970.
    pub fn answered(&self, runner: &str, at: u64) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(runner.to_owned(), at);
    }

    /// When `runner` last answered, if it has since the app started.
    pub fn last(&self, runner: &str) -> Option<u64> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(runner)
            .copied()
    }
}

/// The contacts this window keeps.
fn contacts() -> &'static Contacts {
    static CONTACTS: std::sync::OnceLock<Contacts> = std::sync::OnceLock::new();
    CONTACTS.get_or_init(Contacts::default)
}

/// **How one runner is**, as its row says it once asked (#2052).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct RunnerAsked {
    pub standing: RunnerStanding,
    /// What a link to it said; none where nothing linked to it (one suspended, or whose
    /// container could not be read).
    pub link: Option<PoolLink>,
    /// The version it runs, where it answered.
    pub version: Option<String>,
    /// When it last answered this window, in seconds since 1970 (good to 2106); none if it has
    /// not since the app started.
    pub contacted: Option<u32>,
    /// What it holds, set against what the window has open.
    pub holds: NotOpenHere,
}

/// The chats and browsers of every project the window holds, by what a runner names them by.
fn open_here(planes: &crate::planes::Planes) -> OpenHere {
    let mut open = OpenHere::default();
    for held in planes.every_held() {
        if let Some(project) = purlis_core::scaffold::planefile::project_id(held.root()) {
            for one in held.chats().open_now() {
                open.chats.insert(
                    (project.clone(), one.session),
                    (held.plane_id().clone(), one.session),
                );
            }
        }
        for browser in held.browsers().listed() {
            open.browsers.insert(browser.program);
        }
    }
    open
}

/// Everything the window has open, each with the pool it runs on.
fn what_is_open(planes: &crate::planes::Planes) -> (Vec<Placed>, Vec<BrowserPlaced>) {
    let (mut chats, mut browsers) = (Vec::new(), Vec::new());
    for held in planes.every_held() {
        let plane = held.plane_id().clone();
        let open = held.chats().open_now();
        let names: BTreeMap<u32, String> = open
            .iter()
            .filter_map(|one| Some((one.session, held.chats().shown_name(one.session)?)))
            .collect();
        for one in open.iter().filter(|one| one.harness.is_some()) {
            chats.push(Placed {
                plane: plane.clone(),
                session: one.session,
                name: names.get(&one.session).cloned().unwrap_or_default(),
                on: held.chats().pool_of(one.session),
            });
        }
        for browser in held.browsers().listed() {
            browsers.push(BrowserPlaced {
                plane: plane.clone(),
                session: browser.session,
                chat: browser
                    .was
                    .clone()
                    .or_else(|| names.get(&browser.session).cloned())
                    .unwrap_or_default(),
                name: browser.name,
                on: browser.pool,
            });
        }
    }
    chats.sort_by(|a, b| (a.plane.as_str(), a.session).cmp(&(b.plane.as_str(), b.session)));
    (chats, browsers)
}

/// **Every device and what runs on it** (#2052): this machine and every runner it keeps, each
/// with the chats and browsers the window has open there, and each pool's limits.
#[tauri::command]
#[specta::specta]
pub async fn devices(
    planes: tauri::State<'_, crate::planes::Planes>,
) -> Result<DevicesShown, String> {
    let (chats, browsers) = what_is_open(&planes);
    crate::off_the_window("reading this machine's devices", move || {
        let (kept, pools) = kept_here()?;
        Ok(DevicesShown {
            version: crate::runnerlinks::desktop_version().to_owned(),
            placement: placed(&kept, chats, browsers),
            pools,
        })
    })
    .await
}

#[cfg(unix)]
fn kept_here() -> Result<(Vec<KeptRunner>, Vec<PoolRequests>), String> {
    let Some(config) = purlis_core::machine::config_root() else {
        return Ok((Vec::new(), Vec::new()));
    };
    let kept = purlis_core::runners::read(&config)
        .map_err(|why| why.to_string())?
        .iter()
        .map(|one| KeptRunner {
            pool: one.pool().to_owned(),
            runner: one.name().to_owned(),
            draining: one.draining,
        })
        .collect();
    let pools = purlis_core::runners::pools(&config)
        .map_err(|why| why.to_string())?
        .iter()
        .map(|pool| PoolRequests {
            pool: pool.name().to_owned(),
            limits: crate::dockerpools::shown(pool.limits()),
            millicpus: pool.limits().millicpus(),
        })
        .collect();
    Ok((kept, pools))
}

/// No Docker pool is kept but on macOS and Linux: this machine is the only device.
#[cfg(not(unix))]
fn kept_here() -> Result<(Vec<KeptRunner>, Vec<PoolRequests>), String> {
    Ok((Vec::new(), Vec::new()))
}

/// **How the runner `runner` of pool `pool` is** (#2052): its container read, and only a running
/// one linked to, asked its version and what it holds.
#[tauri::command]
#[specta::specta]
pub async fn device_runner(
    planes: tauri::State<'_, crate::planes::Planes>,
    pool: String,
    runner: String,
) -> Result<RunnerAsked, String> {
    let open = open_here(&planes);
    #[cfg(unix)]
    {
        crate::off_the_window("asking a runner how it is", move || {
            let config = purlis_core::machine::config_root()
                .ok_or_else(|| "There is no config home on this machine.".to_owned())?;
            let machine = crate::runnerlinks::Machine::here(&config)?;
            let kept = purlis_core::runners::read(&config)
                .map_err(|why| why.to_string())?
                .into_iter()
                .find(|one| one.pool() == pool && one.name() == runner)
                .ok_or_else(|| format!("No runner {runner} of pool {pool} is on this machine."))?;
            let name = format!("{pool}/{runner}");
            let asked = answered(
                machine.docker.running(&kept.made).map_err(|why| why.0),
                || {
                    // As it is: a runner is never woken to be asked.
                    let to = crate::runnerlinks::ToARunner::of_kept(&machine, &kept)
                        .map_err(|why| PoolLink::Down { why })?;
                    crate::runnerlinks::listed(&to)
                },
                || crate::runnerlinks::version_of(&machine, &kept).ok(),
                contacts().last(&name),
                crate::sandboxing::now_secs(),
            );
            if let Some(at) = asked.contacted {
                contacts().answered(&name, at);
            }
            Ok(RunnerAsked {
                holds: not_open_here(&asked.holds, &open),
                standing: asked.standing,
                link: asked.link,
                version: asked.version,
                contacted: asked.contacted.and_then(|at| u32::try_from(at).ok()),
            })
        })
        .await
    }
    #[cfg(not(unix))]
    {
        let _ = (open, pool, runner);
        Err("purlis keeps Docker pools on macOS and Linux only.".to_owned())
    }
}

#[cfg(test)]
mod tests;
