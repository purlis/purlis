// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host's browsers, as the binary starts them (#2012, spec #2008): the engine
//! (`purlis_browser_engine`) and its reach proxy ([`crate::sandbox::browser_reach`]) put
//! together behind the host's seam ([`purlis_host::Browsers`]), so the host crate links
//! neither. The window's binary and `purlis serve` both hand the host [`EngineBrowsers`].
//!
//! - **Which engine**: the pinned Chrome for Testing, downloaded into
//!   `<data>/engines/chrome-for-testing` on first use and checked against the pin this build
//!   ships before it runs, or against a newer manifest the manifest workflow signed
//!   ([`Sigstore`], #2073).
//! - **Kept current apart from purlis's releases** ([`keep_current`]): once the pinned engine
//!   has been used on this machine, the host reads the newest published manifest and its
//!   Sigstore bundle [`FIRST_CHECK`] after it starts and every [`CHECK_EVERY`] after, and a
//!   browser that would start on an engine behind past its warning checks first. A manifest
//!   is taken only when its bundle verifies; anything else leaves the engine in use as it is,
//!   and a check that failed or was refused is written to the host's log. The manifest and
//!   bundle are read within [`MANIFEST_READ`], before anything waits on another install. Every
//!   request is listed in the network log, redirects included: the manifest's as the
//!   updater's, an archive's as the browser engine's ([`manifest_reads`], [`archive_reads`],
//!   ADR 0083 §9).
//! - **Its keeper is this binary**, started again with [`KEEPER`]: the first thing each binary's
//!   `main` does is [`keep_if_asked`], so the keeper runs before anything else of the program.
//! - **This runs in the session host.** The window's host is the window's own binary, so with
//!   browsers it runs core code: the engine's download and the data home are read there, as a
//!   browser starts and never before (D-2012-4).
//! - **Its reach** is what the window gives it as it starts ([`reach_of`], #2013), and again
//!   with each act (#2018): the hosts its chat's sandbox lists, with the person's Allows since.
//!   A site new to the project is judged by [`judge::SiteJudge`]: the page may read it, and the
//!   agent's address, a form or a body sent there, or its typing there, asks the person first;
//!   on the open web nothing asks. A browser started with none (the person's own, #2012)
//!   reaches nothing listed, and every site new to the project is refused
//!   ([`NewSites::refusing`]), so it fails closed.
//! - **Its acts** are the driver's own calls ([`call_of`]), answered in the protocol's words
//!   ([`browsed`]), a screenshot made to fit the control lane ([`MOST_SCREENSHOT_BYTES`]).
//! - **Its data folder** is the one the host gives it, under `<data>/browsers`, fresh: the host
//!   removes it once the browser has ended.
//! - **Its driver** (#2014): the host's lease is the driver's own ([`drive::Browser::take_over`]),
//!   each act made under the epoch the host checked; the person's input becomes the driver's own
//!   event ([`input_of`]), and each frame names what it shows ([`aim_of`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, PoisonError};
use std::time::Duration;

use bytes::Bytes;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{self, Browser, CallId, Limits, PageId, Ref};
use purlis_browser_engine::install::{
    Age, EngineSettings, Engines, Fetch, Hop, Https, Manifest, Platform, Ready, Refused,
};
use purlis_browser_engine::launch::{self, Launch, PAGE_SIZE};
use purlis_browser_engine::process::{Engine, Keeper};
use purlis_browser_engine::sigstore::Sigstore;
use purlis_host::protocol::browsing::{self as wire, Act, Browsed, BrowserReach, PageSaid};
use purlis_host::protocol::session::Exit;
use purlis_host::{Browsers, FrameSink, NotActed, NotTaken, Screen};

use crate::sandbox::browser_reach::{Lane, NewSites, ReachProxy};
use crate::sandbox::reach::{Reach, Written};

pub mod judge;

/// The argument that starts this binary as an engine's keeper.
pub const KEEPER: &str = "purlis-engine-keeper";

/// How long a stopping engine has to close by itself before its group is killed.
pub const STOP_GRACE: Duration = Duration::from_secs(2);

/// When this process was started as an engine's keeper ([`KEEPER`]), keep the engine's group
/// and never return; otherwise return at once, having read nothing but the arguments.
pub fn keep_if_asked() {
    if std::env::args_os().nth(1).is_some_and(|arg| arg == KEEPER) {
        purlis_browser_engine::keeper::run();
    }
}

/// Where the pinned engines are installed: `<data>/engines/chrome-for-testing`, as
/// `docs/plane-format.md` records it (#2009).
pub fn engines_in(data: &Path) -> PathBuf {
    data.join("engines").join("chrome-for-testing")
}

/// Where each browser's data folder is made: `<data>/browsers`.
pub fn browsers_in(data: &Path) -> PathBuf {
    data.join(crate::datahome::BROWSERS)
}

/// `hosting`, starting its browsers on the pinned engine with this binary as their keeper, and
/// making their data folders under `<data>/browsers`. Nothing is read until a browser starts.
pub fn given_to(hosting: purlis_host::Hosting) -> purlis_host::Hosting {
    keep_engines_current_in_the_background();
    hosting.with_browsers(EngineBrowsers::here())
}

/// How long after the session host starts it first checks for a newer engine.
pub const FIRST_CHECK: Duration = Duration::from_secs(5 * 60);

/// How often it checks again. The manifest workflow signs one a day.
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// The longest the published manifest and its bundle may take to read, each.
pub const MANIFEST_READ: Duration = Duration::from_secs(60);

/// **The background check** (#2073): when the pinned engine has been used on this machine,
/// read the newest published manifest and its bundle through `manifests`, and take it as
/// [`Engines::update`] does, its archive downloaded through `archives`. A machine that never
/// used it reads nothing.
pub fn keep_current(
    engines: &Engines,
    settings: &EngineSettings,
    manifests: &dyn Fetch,
    archives: &dyn Fetch,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<Result<Ready, Refused>> {
    if !engines.in_use() {
        return None;
    }
    Some(update(engines, settings, manifests, archives, now))
}

/// The published manifest read through `manifests`, then taken as [`Engines::update`] does.
fn update(
    engines: &Engines,
    settings: &EngineSettings,
    manifests: &dyn Fetch,
    archives: &dyn Fetch,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Ready, Refused> {
    let (manifest, bundle) = Engines::published(manifests)?;
    engines.update(&manifest, &bundle, settings, archives, now)
}

/// The published manifest and its bundle are read within [`MANIFEST_READ`], each request
/// listed in the network log as the updater's.
pub fn manifest_reads() -> Https {
    Https::within(MANIFEST_READ).listening(listed_as(crate::netlog::Feature::Updater))
}

/// An engine's archive is read within [`Https::ARCHIVE`], each request listed in the network
/// log as the browser engine's.
pub fn archive_reads() -> Https {
    Https::default().listening(listed_as(crate::netlog::Feature::BrowserEngine))
}

/// Each request a download makes, listed in the network log under `feature`.
fn listed_as(feature: crate::netlog::Feature) -> Arc<dyn Fn(Hop<'_>) + Send + Sync> {
    Arc::new(move |hop: Hop<'_>| {
        crate::netlog::fetched(feature, hop.url, hop.status, hop.answered, hop.took);
    })
}

/// A check of the engine that failed or was refused, in the host's log. One that found
/// nothing newer says nothing.
fn report(checked: &Result<Ready, Refused>) {
    match checked {
        Ok(ready) => tracing::info!(
            version = ready.version().unwrap_or_default(),
            "the browser engine was updated"
        ),
        Err(Refused::Older { .. }) => {}
        Err(why) => tracing::warn!("the browser engine's update check did not update it: {why}"),
    }
}

/// The engines under `<data>` on `platform`, trusting the pin this build ships and the
/// manifests the manifest workflow signed.
fn engines_here(data: &Path, platform: Platform) -> Engines {
    Engines::new(
        engines_in(data),
        platform,
        Manifest::shipped(),
        Box::new(Sigstore::engine_manifests()),
    )
}

/// Starts the background check once in this process: [`FIRST_CHECK`] after now, then every
/// [`CHECK_EVERY`]. It reads the data home at each check, and never fails the host.
fn keep_engines_current_in_the_background() {
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        let _ = std::thread::Builder::new()
            .name("engine-updates".into())
            .spawn(|| {
                std::thread::sleep(FIRST_CHECK);
                loop {
                    check_here();
                    std::thread::sleep(CHECK_EVERY);
                }
            });
    });
}

/// One background check of this machine's engines.
fn check_here() {
    let (Some(data), Some(platform)) = (crate::datahome::root(), Platform::current()) else {
        return;
    };
    let engines = engines_here(&data, platform);
    let checked = keep_current(
        &engines,
        &EngineSettings::default(),
        &manifest_reads(),
        &archive_reads(),
        chrono::Utc::now(),
    );
    if let Some(checked) = checked {
        report(&checked);
    }
}

/// Starts each browser on the pinned engine, with this binary as its keeper.
pub struct EngineBrowsers {
    /// `<data>`, and this program, or `None` for this machine's and this process's, found as a
    /// browser starts.
    places: Option<(PathBuf, PathBuf)>,
}

impl EngineBrowsers {
    /// Browsers whose engines and data folders are under `data`, kept by `program` run with
    /// [`KEEPER`].
    pub fn new(data: PathBuf, program: PathBuf) -> Self {
        EngineBrowsers {
            places: Some((data, program)),
        }
    }

    /// Browsers for this binary on this machine: under `<data>`, kept by this very program,
    /// both found as a browser starts.
    pub fn here() -> Self {
        EngineBrowsers { places: None }
    }

    /// `<data>`, and the program that keeps each engine.
    fn places(&self) -> Result<(PathBuf, PathBuf), String> {
        if let Some(places) = &self.places {
            return Ok(places.clone());
        }
        let data = crate::datahome::root()
            .ok_or_else(|| "there is no data home to keep a browser's engine in".to_owned())?;
        let program = std::env::current_exe()
            .map_err(|why| format!("this program's path is unknown: {why}"))?;
        Ok((data, program))
    }

    /// The keeper of each engine: `program`, run with [`KEEPER`].
    fn keeper(program: PathBuf) -> Keeper {
        Keeper {
            program,
            args: vec![KEEPER.into()],
        }
    }
}

impl Browsers for EngineBrowsers {
    fn data_folders(&self) -> Result<PathBuf, String> {
        self.places().map(|(data, _)| browsers_in(&data))
    }

    fn start(&self, data: &Path, reach: Option<&BrowserReach>) -> Result<Arc<dyn Screen>, String> {
        let (home, program) = self.places()?;
        let platform = Platform::current()
            .ok_or_else(|| "no browser engine is made for this platform".to_owned())?;
        let engines = engines_here(&home, platform);
        let ready = prepared(&engines).map_err(|why| why.to_string())?;
        // A chat's browser is judged (#2018); the person's own, given no reach, refuses every
        // site new to the project at its proxy.
        let sites = reach.map(judge::SiteJudge::of).transpose()?.map(Arc::new);
        let (listed, new_sites) = reach_of(reach)?;
        let new_sites = sites
            .as_ref()
            .map_or(new_sites, judge::SiteJudge::new_sites);
        let proxy = ReachProxy::start(Lane::of(listed), new_sites)
            .map_err(|why| format!("the browser's reach proxy did not start: {why}"))?;
        let launch = Launch::new(&ready, data, launch::ReachProxy(proxy.address()));
        let engine =
            Engine::start(&launch, &Self::keeper(program)).map_err(|why| why.to_string())?;
        let browser = match &sites {
            Some(sites) => {
                let judge = Arc::clone(sites);
                Browser::judged(
                    engine,
                    Limits::default(),
                    Arc::new(move |out: &drive::Outbound| judge.judge(out)),
                )
            }
            None => Browser::new(engine, Limits::default()),
        }
        .map_err(|why| why.to_string())?;
        Ok(Arc::new(EngineScreen {
            process: browser.process(),
            sites,
            held: Mutex::new(Some(Held {
                browser: Arc::new(browser),
                proxy,
            })),
        }))
    }
}

/// The engine a browser starts on: the trusted one, after reading the published manifest first
/// when that engine is behind past its warning or refused for its age, so a machine whose
/// background check has not run yet is not refused for an engine already superseded.
fn prepared(engines: &Engines) -> Result<Ready, Refused> {
    let settings = EngineSettings::default();
    let archives = archive_reads();
    let now = chrono::Utc::now();
    let first = engines.prepare(&settings, &archives, now);
    let behind = match &first {
        Ok(ready) => matches!(ready.age(), Age::Behind { .. }),
        Err(refused) => matches!(refused, Refused::Outdated { .. }),
    };
    if !behind {
        return first;
    }
    let checked = update(engines, &settings, &manifest_reads(), &archives, now);
    report(&checked);
    checked.or(first)
}

/// The most bytes a screenshot's PNG is carried as: with its base64 and the answer around it,
/// well inside one control frame of the session protocol
/// (`purlis_host::protocol::link::MOST_CONTROL_FRAME_BYTES`), so a larger one is halved to fit
/// (`drive::fit`).
pub const MOST_SCREENSHOT_BYTES: usize = 600 * 1024;

/// **What a browser reaches** (#2013): the hosts `reach` lists, each at its layer, and the hosts
/// policy pins as never allowed, read as the chat's own reach reads them; and a public site
/// nothing lists only where `reach` lets it reach the open web. None given, nothing is listed
/// and every new site is refused. A listing that does not read is refused, never dropped.
pub fn reach_of(reach: Option<&BrowserReach>) -> Result<(Reach, NewSites), String> {
    let Some(reach) = reach else {
        return Ok((Reach::of(Vec::new()), NewSites::refusing()));
    };
    let written = Written {
        listed: reach
            .listed
            .iter()
            .map(|listed| {
                (
                    listed.host.clone(),
                    crate::sandbox::reach::by_of(listed.layer),
                )
            })
            .collect(),
        never: reach.never.clone(),
        withdrawn: reach.withdrawn.clone(),
        anything: false,
    };
    let read = Reach::from_written(&written)
        .map_err(|why| format!("the browser's reach does not read: {why}"))?;
    let new_sites = if reach.open_web {
        NewSites::carrying()
    } else {
        NewSites::refusing()
    };
    Ok((read, new_sites))
}

/// **A chat's act as the driver's own call** (#2013), or why it is refused before the driver
/// hears of it: an address that is not an http or https one, in the words that say so.
pub fn call_of(act: Act) -> Result<drive::Call, String> {
    use drive::Call;
    let page = |page: Option<String>| page.map(PageId::new);
    let target = |r: Option<String>| r.map(Ref::new);
    let address = |address: &str| WebAddress::parse(address).map_err(|why| why.to_string());
    Ok(match act {
        Act::Open { address: to } => Call::Open {
            address: address(&to)?,
        },
        Act::Navigate {
            page: on,
            address: to,
        } => Call::Navigate {
            page: page(on),
            address: address(&to)?,
        },
        Act::Snapshot {
            page: on,
            within,
            from,
            lines,
        } => Call::Snapshot {
            page: page(on),
            within: target(within),
            from: from.map_or(1, |from| from.max(1) as usize),
            lines: lines.map(|lines| lines as usize),
        },
        Act::Find {
            page: on,
            text,
            role,
            within,
        } => Call::Find {
            page: page(on),
            text,
            role,
            within: target(within),
        },
        Act::Click { target: t } => Call::Click {
            target: Ref::new(t),
        },
        Act::DoubleClick { target: t } => Call::DoubleClick {
            target: Ref::new(t),
        },
        Act::Hover { target: t } => Call::Hover {
            target: Ref::new(t),
        },
        Act::Type { page: on, fields } => Call::Type {
            page: page(on),
            fields: fields
                .into_iter()
                .map(|field| drive::Field {
                    into: target(field.into),
                    text: field.text,
                    replace: field.replace,
                })
                .collect(),
        },
        Act::Press { page: on, keys } => Call::Press {
            page: page(on),
            keys,
        },
        Act::Select { target: t, options } => Call::Select {
            target: Ref::new(t),
            options,
        },
        Act::Drag { from, to } => Call::Drag {
            from: Ref::new(from),
            to: match to {
                wire::DragTo::Element(to) => drive::DragTo::Element(Ref::new(to)),
                wire::DragTo::Offset { x, y } => drive::DragTo::Offset {
                    x: f64::from(x),
                    y: f64::from(y),
                },
            },
        },
        Act::Scroll { page: on, at, x, y } => Call::Scroll {
            page: page(on),
            at: target(at),
            x: f64::from(x),
            y: f64::from(y),
        },
        Act::Back { page: on } => Call::Back { page: page(on) },
        Act::WaitForText {
            page: on,
            text,
            gone,
            within_ms,
        } => Call::WaitForText {
            page: page(on),
            text,
            gone,
            within: Duration::from_millis(within_ms),
        },
        Act::Pages => Call::Pages,
        Act::SwitchPage { page: to } => Call::SwitchPage {
            page: PageId::new(to),
        },
        Act::Screenshot { page: on } => Call::Screenshot { page: page(on) },
    })
}

fn said(page: drive::PageInfo) -> PageSaid {
    PageSaid {
        page: page.id.as_str().to_owned(),
        address: page.address,
        title: page.title,
        current: page.current,
    }
}

/// **The driver's answer in the protocol's words** (#2013), a screenshot made to fit `most`
/// bytes: halved until it does, and said so in the answer's note, or left out and said why.
pub fn browsed(answer: drive::Answer, most: usize) -> Browsed {
    let (settled, waiting_for) = match answer.settled {
        drive::Settled::Yes => (true, Vec::new()),
        drive::Settled::No(waiting) => (false, waiting),
    };
    let mut out = Browsed {
        page: answer.page.map(said),
        settled,
        waiting_for,
        opened: answer.opened.into_iter().map(said).collect(),
        note: answer.note,
        repeated: answer.repeated,
        asked: answer.asked,
        ..Browsed::default()
    };
    let mut also = |note: String| {
        out.note = Some(match out.note.take() {
            Some(before) => format!("{before}; {note}"),
            None => note,
        });
    };
    match answer.content {
        drive::Content::Nothing => {}
        drive::Content::Text(text) => out.text = Some(text),
        drive::Content::Pages(pages) => out.pages = pages.into_iter().map(said).collect(),
        drive::Content::Seen(seen) => out.seen = Some(seen),
        drive::Content::Image(png) => match drive::fit::to_fit(&png, most) {
            Ok(fitted) => {
                if fitted.halved > 0 {
                    also(format!(
                        "the screenshot was made smaller to fit, to {}×{} pixels",
                        fitted.width, fitted.height
                    ));
                }
                out.png = Some(fitted.png);
            }
            Err(why) => also(format!("no screenshot is attached: {why}")),
        },
    }
    out
}

/// **The person's input as the driver's own event** (#2014), aimed where they aimed it.
pub fn input_of(input: &wire::Input) -> drive::PersonInput {
    let aim_at = |aim: &wire::Aim| drive::Aim {
        page: PageId::new(aim.page.clone()),
        document: aim.document,
    };
    match input {
        wire::Input::Mouse {
            aim,
            action,
            x,
            y,
            button,
            clicks,
            modifiers,
            delta_x,
            delta_y,
        } => drive::PersonInput::Mouse {
            aim: aim_at(aim),
            kind: match action {
                wire::MouseAction::Moved => drive::MouseKind::Moved,
                wire::MouseAction::Pressed => drive::MouseKind::Pressed,
                wire::MouseAction::Released => drive::MouseKind::Released,
                wire::MouseAction::Wheel => drive::MouseKind::Wheel,
            },
            x: f64::from(*x),
            y: f64::from(*y),
            button: match button {
                wire::Button::None => drive::MouseButton::None,
                wire::Button::Left => drive::MouseButton::Left,
                wire::Button::Middle => drive::MouseButton::Middle,
                wire::Button::Right => drive::MouseButton::Right,
            },
            clicks: *clicks,
            modifiers: *modifiers,
            delta_x: f64::from(*delta_x),
            delta_y: f64::from(*delta_y),
        },
        wire::Input::Key {
            aim,
            action,
            key,
            physical,
            key_code,
            modifiers,
        } => drive::PersonInput::Key {
            aim: aim_at(aim),
            down: *action == wire::KeyAction::Down,
            key: key.clone(),
            code: physical.clone(),
            key_code: *key_code,
            modifiers: *modifiers,
        },
    }
}

/// **What a frame shows, in the protocol's words** (#2014).
pub fn aim_of(aim: &drive::Aim) -> wire::Aim {
    wire::Aim {
        page: aim.page.as_str().to_owned(),
        document: aim.document,
    }
}

/// Why the driver did not take the person's input, as the host says it.
fn not_taken(refusal: drive::Refusal) -> NotTaken {
    match refusal {
        drive::Refusal::NotDriving => NotTaken::NotDriving,
        drive::Refusal::Missed => NotTaken::Missed,
        drive::Refusal::Ended => NotTaken::Ended,
        other => NotTaken::Refused(other.to_string()),
    }
}

/// A running browser and its reach proxy, which lives exactly as long as it.
struct Held {
    browser: Arc<Browser>,
    proxy: ReachProxy,
}

/// One browser on its engine, as the host drives it.
struct EngineScreen {
    process: Option<u32>,
    /// Its judge, shared with its reach proxy (#2018): none for a browser given no reach.
    sites: Option<Arc<judge::SiteJudge>>,
    held: Mutex<Option<Held>>,
}

impl EngineScreen {
    fn with<T>(&self, act: impl FnOnce(&Arc<Browser>) -> T) -> Option<T> {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|held| act(&held.browser))
    }
}

impl Screen for EngineScreen {
    fn show(&self, frames: FrameSink) {
        self.with(|browser| {
            browser.show(Arc::new(move |painted: drive::Painted| {
                frames(aim_of(&painted.aim), Bytes::from(painted.jpeg));
            }));
        });
    }

    fn pace(&self, every: Duration) {
        self.with(|browser| browser.pace(every));
    }

    fn hide(&self) {
        self.with(|browser| browser.hide());
    }

    fn size(&self) -> (u16, u16) {
        self.with(|browser| browser.size())
            .flatten()
            .unwrap_or(PAGE_SIZE)
    }

    fn process(&self) -> Option<u32> {
        self.process
    }

    fn stop(&self) {
        let held = self
            .held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(Held { browser, proxy }) = held {
            browser.stop(STOP_GRACE);
            drop(proxy);
        }
    }

    fn reach_now(&self, reach: &BrowserReach) -> Result<(), String> {
        match &self.sites {
            Some(sites) => sites.reach_now(reach),
            None => Err("this browser was started with no reach, and takes none".to_owned()),
        }
    }

    fn act(&self, call_id: &str, act: Act, epoch: u64) -> Result<Browsed, NotActed> {
        let id = CallId::new(call_id).map_err(|why| NotActed::Refused(why.to_string()))?;
        let call = call_of(act).map_err(NotActed::Refused)?;
        // The call waits on the engine: it is made outside the lock, so frames and a stop are
        // never held up behind it.
        let browser = self.with(Arc::clone);
        browser
            .map_or(Err(drive::Refusal::Ended), |browser| {
                browser.call_at(epoch, &id, call)
            })
            .map(|answer| browsed(answer, MOST_SCREENSHOT_BYTES))
            .map_err(|refusal| match refusal {
                drive::Refusal::Ended => NotActed::Ended,
                other => NotActed::Refused(other.to_string()),
            })
    }

    fn take_over(&self) -> u64 {
        // Outside the lock, as an act is: it returns once no act of the agent's can reach a
        // page.
        self.with(Arc::clone)
            .map_or(0, |browser| browser.take_over())
    }

    fn hand_back(&self) -> u64 {
        self.with(Arc::clone)
            .map_or(0, |browser| browser.hand_back())
    }

    fn input(&self, input: &wire::Input) -> Result<(), NotTaken> {
        let browser = self.with(Arc::clone).ok_or(NotTaken::Ended)?;
        browser.person_input(&input_of(input)).map_err(not_taken)
    }

    fn size_to(&self, width: u16, height: u16) -> Result<(), NotTaken> {
        let browser = self.with(Arc::clone).ok_or(NotTaken::Ended)?;
        browser.size_to(width, height).map_err(not_taken)
    }

    /// Told once the engine has ended, however it ended; at once when it was stopped already.
    fn when_it_ends(&self, tell: Box<dyn FnOnce(Exit) + Send>) {
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        match held.as_ref() {
            Some(Held { browser, .. }) => {
                browser.when_it_ends(Box::new(move || tell(Exit::Code(0))));
            }
            None => {
                drop(held);
                tell(Exit::Code(0));
            }
        }
    }
}

#[cfg(test)]
mod tests;
