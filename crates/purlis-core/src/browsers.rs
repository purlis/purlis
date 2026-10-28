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
//!   on the open web nothing asks. A browser the person opens is given the project's reach for
//!   browsers (#2176), and the address they type is judged as the agent's is ([`typed`]). A
//!   browser started with none reaches nothing listed, and every site new to the project is
//!   refused ([`NewSites::refusing`]), so it fails closed.
//! - **Its acts** are the driver's own calls ([`call_of`]), answered in the protocol's words
//!   ([`browsed`]), a screenshot made to fit the control lane ([`MOST_SCREENSHOT_BYTES`]).
//! - **Its data folder** is the one the host gives it, under `<data>/browsers`, fresh: the host
//!   removes it once the browser has ended. **Its engine's log** is written beside it,
//!   `<data>/browsers/<program>.engine.log`, and outlives it: the newest [`ENGINE_LOGS_KEPT`]
//!   are kept, swept as a browser starts ([`engine_log_of`], #2745).
//! - **Its end** is told with who ended it, its engine's exit and why its DevTools connection
//!   closed, and where its engine's log is ([`ended_of`], #2745): the host says it in its log.
//! - **Its logins** (#2016): a declared login is performed by the driver itself
//!   ([`drive::Browser::log_in`]), on the address the window hands it with the vault's values.
//!   The browser's reach proxy and judge are told first: what purlis sends to the login's site
//!   goes, and just before the first value goes in the proxy is bound to that site alone, for
//!   the browser's life ([`crate::sandbox::browser_reach::Bound`]). A browser started with no
//!   reach, or holding a login at another site already, is logged in nowhere.
//! - **Its driver** (#2014): the host's lease is the driver's own ([`drive::Browser::take_over`]),
//!   each act made under the epoch the host checked; the person's input becomes the driver's own
//!   event ([`input_of`]), and each frame names what it shows ([`aim_of`]).

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Once, PoisonError};
use std::time::Duration;

use bytes::Bytes;
use purlis_browser_engine::WebAddress;
use purlis_browser_engine::drive::{self, Browser, CallId, Limits, PageId, Ref};
use purlis_browser_engine::ending::{Ending, EngineExit};
use purlis_browser_engine::install::{
    Age, EngineChoice, EngineSettings, Engines, Fetch, Hop, Https, Manifest, Platform, Ready,
    Refused,
};
use purlis_browser_engine::launch::{self, Launch, PAGE_SIZE};
use purlis_browser_engine::process::{Engine, Keeper};
use purlis_browser_engine::sigstore::Sigstore;
use purlis_host::protocol::browsing::{self as wire, Act, Browsed, BrowserReach, PageSaid};
use purlis_host::protocol::session::Exit;
use purlis_host::{
    BrowserNotStarted, Browsers, Download, FrameSink, LoginEnded, NotActed, NotTaken, Screen,
    ScreenEnded,
};

use crate::sandbox::browser_reach::{Bound, Lane, NewSites, ReachProxy, Site};
use crate::sandbox::reach::{Reach, Written};

pub use purlis_browser_engine::machine::machine_chrome;

pub mod judge;
pub mod sandboxing;
pub mod typed;

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

/// **The folder in a browser's data folder its downloads are held in** (#2020): each under the
/// engine's own name for it. Inside `<data>/browsers`, which no chat reads or writes; removed
/// with the browser's folder as it ends, and never kept with a kept login.
pub const DOWNLOADS: &str = "downloads";

/// The folder in a browser's data folder each upload's copies are made in (#2020).
pub const UPLOADS: &str = "uploads";

/// How many engines' logs are kept under `<data>/browsers` (#2745): the newest, whatever
/// browsers they were of.
pub const ENGINE_LOGS_KEPT: usize = 20;

/// The suffix of an engine's log beside its browser's folder.
const ENGINE_LOG: &str = ".engine.log";

/// **Where the engine of the browser whose folder is `folder` writes its log** (#2745):
/// `<program>.engine.log` beside the folder, so the folder's removal as the browser ends leaves
/// it. A program's id holds no `.`, so no browser's folder is ever named so.
pub fn engine_log_of(folder: &Path) -> PathBuf {
    let mut name = folder.file_name().unwrap_or_default().to_os_string();
    name.push(ENGINE_LOG);
    folder.with_file_name(name)
}

/// **Keeps the newest `kept` engines' logs in `folders`** (#2745), removing older ones whose
/// browser's folder is gone: a running browser's log is never removed.
pub fn sweep_engine_logs(folders: &Path, kept: usize) {
    let Ok(entries) = std::fs::read_dir(folders) else {
        return;
    };
    let mut logs: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(ENGINE_LOG))
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| {
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (modified, entry.path())
        })
        .collect();
    logs.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    for (_, log) in logs.into_iter().skip(kept) {
        let name = log.file_name().unwrap_or_default().to_string_lossy();
        let Some(program) = name.strip_suffix(ENGINE_LOG) else {
            continue;
        };
        let folder = folders.join(program);
        if !folder.exists() {
            let _ = std::fs::remove_file(&log);
        }
    }
}

/// **A browser's end as the host is told it** (#2745): its engine's exit in the protocol's words,
/// and why, with where its engine's log is. An exit nobody saw is said as none.
pub fn ended_of(ending: &Ending, log: &Path) -> ScreenEnded {
    let exit = match &ending.exit {
        Some(EngineExit::Code(code)) => Exit::Code(code.unsigned_abs()),
        Some(EngineExit::Signal(signal)) => Exit::Signal(signal.clone()),
        None => Exit::Code(0),
    };
    ScreenEnded {
        exit,
        why: format!("{ending}; its engine's log is {}", log.display()),
    }
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

/// **The longest a browser's start may take** on a host: its first start reads the engines'
/// manifest and downloads the engine inside it, each within its own limit, and a minute more to
/// start the engine. A window waits this long for a browser's start, never the shorter wait it
/// gives any other answer; the person sees the download as it goes and may cancel it (#2207).
pub const A_START_TAKES_AT_MOST: Duration =
    Duration::from_secs(MANIFEST_READ.as_secs() + Https::ARCHIVE.as_secs() + 60);

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

    /// This device's budget (#2611): what its memory gives, or what the person set in
    /// Settings › This machine › Browsers, read at each start. The window's host and a runner's
    /// each read their own machine's.
    fn budget(&self) -> Option<usize> {
        Some(crate::browserlimits::Limits::here().on_this_device)
    }

    fn start(
        &self,
        data: &Path,
        reach: Option<&BrowserReach>,
        kept: Option<&wire::KeptLogin>,
        download: &Download,
    ) -> Result<Arc<dyn Screen>, BrowserNotStarted> {
        let (home, program) = self.places()?;
        // Its folder's own, made before anything of a browser is written into it.
        made_private_and_unbacked(&browsers_in(&home))?;
        // A browser opened from kept logins holds a login from its first moment (#2021): it is
        // a chat's, judged, and bound to the login's site before its engine starts.
        let kept_site = kept
            .map(|kept| {
                if reach.is_none() {
                    return Err("a browser given no reach opens no kept logins".to_owned());
                }
                site_of_login(&kept.address)
            })
            .transpose()?;
        let platform = Platform::current()
            .ok_or_else(|| "no browser engine is made for this platform".to_owned())?;
        // Which engine (#2835): this machine's own Chrome where the person chose it, else the
        // pinned one where it can make its sandbox, else none.
        let choice = engine_for(&Chosen::here(&engines_in(&home)))
            .map_err(BrowserNotStarted::NoEngineSandbox)?;
        let settings = EngineSettings {
            engine: choice,
            ..EngineSettings::default()
        };
        let engines = engines_here(&home, platform);
        let ready = prepared(&engines, &settings, download)
            .map_err(|refused| download_failed_said(&refused))?;
        // A chat's browser is judged (#2018); the person's own, given no reach, refuses every
        // site new to the project at its proxy.
        let sites = reach.map(judge::SiteJudge::of).transpose()?.map(Arc::new);
        let (listed, new_sites) = reach_of(reach)?;
        let new_sites = sites
            .as_ref()
            .map_or(new_sites, judge::SiteJudge::new_sites);
        // A chat's browser may hold a login (#2016): its proxy is bound with its judge.
        let bound = sites
            .as_ref()
            .map_or_else(Bound::default, |sites| sites.bound().clone());
        let proxy = ReachProxy::bound_by(Lane::of(listed), new_sites, bound)
            .map_err(|why| format!("the browser's reach proxy did not start: {why}"))?;
        let mut keeping = Keeping::new(&home, data);
        if let (Some(kept), Some(site), Some(sites)) = (kept, &kept_site, &sites) {
            proxy.bind(site)?;
            sites.bound_to(site);
            keeping.open(kept)?;
        }
        // Its engine's log beside its folder, kept past its end (#2745).
        sweep_engine_logs(&browsers_in(&home), ENGINE_LOGS_KEPT);
        let log = engine_log_of(data);
        let launch =
            Launch::new(&ready, data, launch::ReachProxy(proxy.address())).logging_to(&log);
        // An engine that ended for want of a sandbox is said as one (#2835), never its crash.
        let no_sandbox_or = |why: String| {
            let said = std::fs::read_to_string(&log).unwrap_or_default();
            if sandboxing::said_no_usable_sandbox(&said) {
                BrowserNotStarted::NoEngineSandbox(sandboxing::NO_ENGINE_SANDBOX.to_owned())
            } else {
                BrowserNotStarted::Failed(why)
            }
        };
        let engine = Engine::start(&launch, &Self::keeper(program))
            .map_err(|why| no_sandbox_or(why.to_string()))?;
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
        .map_err(|why| no_sandbox_or(why.to_string()))?;
        // Its downloads are held in its own folder, which no chat reads (#2020), before any
        // page of it loads.
        let held_in = data.join(DOWNLOADS);
        purlis_same_user::private_directory(&held_in)
            .map_err(|why| format!("the browser's downloads folder could not be made ({why})"))?;
        browser
            .hold_downloads_in(&held_in)
            .map_err(|why| format!("the browser did not take its downloads folder: {why}"))?;
        Ok(Arc::new(EngineScreen {
            data: data.to_path_buf(),
            uploads: std::sync::atomic::AtomicU64::new(0),
            process: browser.process(),
            log,
            fresh: Arc::clone(&keeping.fresh),
            keeping: Arc::new(Mutex::new(keeping)),
            sites,
            held: Mutex::new(Some(Held {
                browser: Arc::new(browser),
                proxy: Arc::new(proxy),
            })),
        }))
    }
}

/// The engine a browser starts on: the trusted one, after reading the published manifest first
/// when that engine is behind past its warning or refused for its age, so a machine whose
/// background check has not run yet is not refused for an engine already superseded.
fn prepared(
    engines: &Engines,
    settings: &EngineSettings,
    download: &Download,
) -> Result<Ready, Refused> {
    let https = archive_reads();
    // Its first download is told as it goes, and stops where the person cancels it (#2207).
    let archives = Progressing::new(&https, download, DOWNLOAD_TOLD_EVERY);
    let now = chrono::Utc::now();
    let first = engines.prepare(settings, &archives, now);
    let behind = match &first {
        Ok(ready) => matches!(ready.age(), Age::Behind { .. }),
        Err(refused) => matches!(refused, Refused::Outdated { .. }),
    };
    if !behind {
        return first;
    }
    let checked = update(engines, settings, &manifest_reads(), &archives, now);
    report(&checked);
    checked.or(first)
}

/// A fresh id for a browser a window starts (#2207): the window names it, so it knows which
/// start an engine download it is told of is for.
pub fn a_browsers_id() -> String {
    ulid::Ulid::generate().to_string()
}

/// How often at most an engine's download is told how far it has come (#2207): a few times a
/// second, and once more as it ends.
pub const DOWNLOAD_TOLD_EVERY: std::time::Duration = std::time::Duration::from_millis(250);

/// **An engine's download, told as it goes and cancelled by the person** (#2207): each read of
/// the archive counts toward `download`, which is told no more often than `every`, and once
/// more as the archive ends; a read after the person cancelled fails in
/// [`purlis_host::ENGINE_DOWNLOAD_CANCELLED`]'s words, so the install keeps nothing of it.
pub struct Progressing<'a> {
    inner: &'a dyn Fetch,
    download: &'a Download,
    every: std::time::Duration,
}

impl<'a> Progressing<'a> {
    /// Reads through `inner`, told to `download` at most every `every`.
    pub fn new(inner: &'a dyn Fetch, download: &'a Download, every: std::time::Duration) -> Self {
        Progressing {
            inner,
            download,
            every,
        }
    }
}

impl Fetch for Progressing<'_> {
    fn still_wanted(&self) -> std::io::Result<()> {
        if self.download.is_cancelled() {
            return Err(std::io::Error::other(
                purlis_host::ENGINE_DOWNLOAD_CANCELLED,
            ));
        }
        self.inner.still_wanted()
    }

    fn fetch(&self, url: &str) -> std::io::Result<Box<dyn std::io::Read + '_>> {
        let (body, total) = self.inner.fetch_sized(url)?;
        Ok(Box::new(Counted {
            body,
            total,
            done: 0,
            told_at: None,
            ended: false,
            download: self.download,
            every: self.every,
        }))
    }
}

/// An archive's body, counted as it is read ([`Progressing`]).
struct Counted<'a> {
    body: Box<dyn std::io::Read + 'a>,
    total: Option<u64>,
    done: u64,
    told_at: Option<std::time::Instant>,
    ended: bool,
    download: &'a Download,
    every: std::time::Duration,
}

impl std::io::Read for Counted<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.download.is_cancelled() {
            return Err(std::io::Error::other(
                purlis_host::ENGINE_DOWNLOAD_CANCELLED,
            ));
        }
        let read = self.body.read(buf)?;
        if read == 0 {
            if !self.ended && !buf.is_empty() {
                self.ended = true;
                self.download.tell(self.done, self.total);
            }
            return Ok(0);
        }
        self.done = self
            .done
            .saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        let now = std::time::Instant::now();
        if self
            .told_at
            .is_none_or(|at| now.duration_since(at) >= self.every)
        {
            self.told_at = Some(now);
            self.download.tell(self.done, self.total);
        }
        Ok(read)
    }
}

/// **Why the engine is not ready, in words that say what to do** (#2207): a download the person
/// cancelled, one that failed, and one that did not match the pin, each said apart.
pub fn download_failed_said(refused: &Refused) -> String {
    match refused {
        Refused::Download(why) if why.to_string() == purlis_host::ENGINE_DOWNLOAD_CANCELLED => {
            download_cancelled_said()
        }
        Refused::Download(why) => format!(
            "the browser engine could not be downloaded ({why}): check this machine's \
             connection, then open the browser again"
        ),
        Refused::Digest { .. } => "the browser engine that was downloaded is not the one \
             purlis pins, so it was not kept: open the browser again to download it again"
            .to_owned(),
        other => other.to_string(),
    }
}

/// What a browser's start is refused with where the person cancelled its engine's download
/// (#2207): [`purlis_host::ENGINE_DOWNLOAD_CANCELLED`]'s words, which a window tells apart.
pub fn download_cancelled_said() -> String {
    format!(
        "{}: open the browser again to download it",
        purlis_host::ENGINE_DOWNLOAD_CANCELLED
    )
}

/// **What decides which engine a browser starts on** (#2835).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    /// The person chose this machine's own Chrome (`machine.json` `browsers.engine`).
    pub machine_chrome: bool,
    /// This machine's own Chrome, where one is installed.
    pub found: Option<PathBuf>,
    /// The pinned engine can make its sandbox here ([`sandboxing::sandboxes`]).
    pub sandboxes: bool,
}

impl Chosen {
    /// This machine now, its pinned engines in `engines`. A fenced (test) build reads no
    /// person's choice outside its fence: there, nothing is chosen.
    pub fn here(engines: &Path) -> Chosen {
        Chosen {
            machine_chrome: chosen_here(),
            found: purlis_browser_engine::machine::machine_chrome(),
            sandboxes: sandboxing::sandboxes(&sandboxing::Machine::here(), engines),
        }
    }
}

/// Whether the person chose this machine's own Chrome, in this machine's store.
fn chosen_here() -> bool {
    let Some(config) = crate::machine::config_root_unheld() else {
        return false;
    };
    #[cfg(feature = "fenced")]
    if !crate::fence::inside(&config, &crate::fence::fence()) {
        return false;
    }
    crate::machine::read(&config).store.browsers.machine_chrome
}

/// **The engine a browser starts on** (#2835, ruled 2026-10-20): this machine's own Chrome
/// where the person chose it and it is installed; else the pinned engine where it can make its
/// sandbox; else none, in [`sandboxing::NO_ENGINE_SANDBOX`]'s words. Never an engine without
/// its sandbox.
pub fn engine_for(chosen: &Chosen) -> Result<EngineChoice, String> {
    if chosen.machine_chrome
        && let Some(chrome) = &chosen.found
    {
        return Ok(EngineChoice::Machine(chrome.clone()));
    }
    if chosen.sandboxes {
        Ok(EngineChoice::Pinned)
    } else {
        Err(sandboxing::NO_ENGINE_SANDBOX.to_owned())
    }
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
        Act::Changes { page: on } => Call::Changes { page: page(on) },
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
        Act::Downloads => Call::Downloads,
        // Each needs the browser's own folder: the screen carries them out ([`EngineScreen`]).
        Act::Upload { .. } | Act::SaveDownload { .. } => {
            return Err("purlis's own screen carries this out, not the driver".into());
        }
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
        downloads: answer.downloads.into_iter().map(download_said).collect(),
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

/// A download as the protocol says it (#2020).
fn download_said(download: drive::DownloadInfo) -> wire::DownloadSaid {
    wire::DownloadSaid {
        download: download.id,
        name: download.name,
        origin: download.origin,
        bytes: download.received,
        total: download.total,
        state: match download.state {
            drive::DownloadState::Going => wire::DownloadState::Going,
            drive::DownloadState::Done => wire::DownloadState::Done,
            drive::DownloadState::Cancelled => wire::DownloadState::Cancelled,
            drive::DownloadState::TooLarge => wire::DownloadState::TooLarge,
            drive::DownloadState::TooMany => wire::DownloadState::TooMany,
        },
        file: download.file,
    }
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

/// **The driver's login for what the window handed** (#2016): the address, its origin and the
/// site it is on, and the values, or why it is no login a browser takes.
pub fn login_of(login: &wire::LogIn) -> Result<(drive::Login, Site), String> {
    let address = WebAddress::parse(&login.address)
        .map_err(|why| format!("the login's address is not one a browser opens: {why}"))?;
    // https, or plain http on this machine's loopback alone; a name in its ASCII spelling, as
    // the engine records a page's address.
    let origin = crate::sandbox::tools::login::origin_of(&login.address)?;
    let (host, port) = judge::site_of(&origin)
        .ok_or_else(|| "the login's address names no origin purlis can fill on".to_owned())?;
    Ok((
        drive::Login {
            address,
            origin,
            user: login
                .user
                .as_ref()
                .map(|user| zeroize::Zeroizing::new(user.expose().to_owned())),
            password: zeroize::Zeroizing::new(login.password.expose().to_owned()),
            only_if_asked: false,
        },
        Site::of(&host, port),
    ))
}

/// **A login purlis performs in a browser whose judge is `sites` and whose reach proxy is
/// `proxy`** (#2016), at `site`, on `origin`: `run` performs it, and is handed what binds the
/// browser to `site` alone just before the first value goes in. While it runs, what purlis sends
/// to `site` goes; once bound, the proxy carries `site` alone for the browser's life, every
/// connection it carried before ended. Refused, running nothing, for a browser that holds a login
/// at another site already.
pub fn performed<T>(
    sites: &judge::SiteJudge,
    proxy: &ReachProxy,
    site: &Site,
    origin: &str,
    run: impl FnOnce(&dyn Fn() -> Result<(), String>) -> T,
) -> Result<T, String> {
    if let Some(held) = sites.bound().sites()
        && !held.contains(site)
    {
        return Err(format!(
            "this browser holds a login at {}, and reaches nothing else: open a new browser \
             with browser_new and log in to {origin} there",
            held.iter()
                .map(Site::to_string)
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    sites.logging_in(site);
    let bind = || -> Result<(), String> {
        proxy.bind(site)?;
        sites.bound_to(site);
        Ok(())
    };
    let done = run(&bind);
    sites.done_logging_in();
    Ok(done)
}

/// **`<data>/browsers/`, made this user's alone and left out of this device's backups** (#2021):
/// a running browser's folder holds its cookies and storage as plainly as the engine writes them.
fn made_private_and_unbacked(folders: &Path) -> Result<(), String> {
    purlis_same_user::private_directory(folders)
        .map_err(|e| format!("the browsers' folder could not be made ({e})"))?;
    crate::browserlogins::exclude_from_backups(folders)
}

/// The site a kept login's page is on, as a login's is ([`login_of`]).
fn site_of_login(address: &str) -> Result<Site, String> {
    let origin = crate::sandbox::tools::login::origin_of(address)?;
    let (host, port) = judge::site_of(&origin)
        .ok_or_else(|| "the login's address names no origin purlis can fill on".to_owned())?;
    Ok(Site::of(&host, port))
}

/// **What a browser keeps of a kept login** (#2021): the store it writes back as it ends, if it
/// holds one, and the kept login it opened from. A browser opened from a store another browser
/// holds is a copy: what it changes is discarded.
struct Keeping {
    /// `<data>`.
    home: PathBuf,
    /// The browser's own data folder.
    folder: PathBuf,
    /// The store it writes back, and its hold on it.
    writes: Option<(wire::KeptLogin, crate::browserlogins::Hold)>,
    /// The kept login it opened from, where it did.
    opened_from: Option<wire::KeptLogin>,
    /// The mark beside its folder, held while it runs, which says to a later browser and to a
    /// profile's removal that it is no folder left behind.
    _live: Option<crate::browserlogins::Mark>,
    /// This device's keyring, and its id, where a test names them: this machine's otherwise.
    keyring: Arc<dyn crate::secrets::keyring::Store>,
    device: Option<String>,
    /// When the browser began: a profile removed since keeps nothing of it (#2693).
    began: std::time::SystemTime,
    /// Whether it has done nothing yet: no act and no input of the person's (#2693). Only a
    /// fresh browser, or one opened from that very store, is kept as a login's store. Apart
    /// from the lock its keeping is under, so marking it never waits on a write-back.
    fresh: Arc<std::sync::atomic::AtomicBool>,
    /// The kept login of each login tried in it, `None` for one that keeps none (#2693): it
    /// keeps a login only where every one tried was that one.
    tried: Vec<Option<wire::KeptLogin>>,
}

/// The store `kept` names.
fn store_of(kept: &wire::KeptLogin) -> Result<crate::browserlogins::Store, String> {
    crate::browserlogins::Store::new(&kept.profile, &kept.persona, &kept.login, &kept.identity)
}

impl Keeping {
    fn new(home: &Path, folder: &Path) -> Self {
        Keeping {
            home: home.to_path_buf(),
            folder: folder.to_path_buf(),
            writes: None,
            opened_from: None,
            _live: None,
            keyring: Arc::new(crate::secrets::keyring::OsStore),
            device: None,
            began: std::time::SystemTime::now(),
            fresh: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            tried: Vec::new(),
        }
    }

    /// The browser acted, or the person took it over: what it holds from now may be of other
    /// sites, or another session of the login's own, so it is no longer kept as a login's
    /// store unless it was opened from that store (#2693).
    #[cfg(test)]
    fn browsed(&self) {
        self.fresh.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// A login was tried in it, keeping `keep` (#2693), whatever came of it: it may have signed
    /// the browser in.
    fn tried(&mut self, keep: Option<&wire::KeptLogin>) {
        // A store it holds is another login's: what it holds from now may be this one's session,
        // so nothing of it is written back, and that store stays as last written.
        if self
            .writes
            .as_ref()
            .is_some_and(|(writes, _)| Some(writes) != keep)
        {
            self.writes = None;
        }
        let keep = keep.cloned();
        if !self.tried.contains(&keep) {
            self.tried.push(keep);
        }
    }

    /// This device's key for kept logins.
    fn keys(&self) -> Result<crate::browserlogins::Keys<'_>, String> {
        Ok(crate::browserlogins::Keys {
            store: &*self.keyring,
            device: match &self.device {
                Some(device) => device.clone(),
                None => crate::machine::this_device_id()?,
            },
        })
    }

    /// **Opens `kept` into the browser's folder**, before its engine starts: once every folder
    /// an earlier browser of the same store left behind is swept, the folder marked as `kept`'s,
    /// and holding the store where no other browser does.
    fn open(&mut self, kept: &wire::KeptLogin) -> Result<(), String> {
        let store = store_of(kept)?;
        let place = store.place();
        crate::browserlogins::sweep(&browsers_in(&self.home), Some(&self.folder), &|_, of| {
            of == place
        });
        // Marked, and the mark held, before a byte of the store is written into the folder.
        self.mark(&store)?;
        let hold = crate::browserlogins::hold_since(&self.home, &store, self.began)?;
        let keys = self.keys()?;
        // A store that does not open is never half opened: the browser starts fresh, and the
        // login fills it as any.
        if let Err(why) = crate::browserlogins::open_into(&self.home, &store, &self.folder, &keys) {
            tracing::warn!(
                "purlis: the kept logins of {}'s {} were not opened, so the browser starts \
                 fresh: {why}",
                kept.profile,
                kept.login
            );
        }
        self.opened_from = Some(kept.clone());
        self.writes = hold.map(|hold| (kept.clone(), hold));
        Ok(())
    }

    /// Marks the folder as `store`'s, held while the browser runs.
    fn mark(&mut self, store: &crate::browserlogins::Store) -> Result<(), String> {
        self._live = None;
        self._live = Some(crate::browserlogins::mark(&self.folder, store)?);
        Ok(())
    }

    /// **The browser keeps `keep` from now on**, a login that went through in it: what it holds
    /// is written back as it ends, where no other browser holds that store, and only where it
    /// started fresh or from that very store. What to tell the chat, where anything.
    fn keep(&mut self, keep: &wire::KeptLogin) -> Option<String> {
        if self
            .writes
            .as_ref()
            .is_some_and(|(writes, _)| writes == keep)
        {
            return None;
        }
        if self.writes.is_some()
            || self
                .opened_from
                .as_ref()
                .is_some_and(|opened| opened != keep)
        {
            return Some(
                "this browser keeps another login's logins already, so this one's are not kept"
                    .to_owned(),
            );
        }
        let a_copy = self.opened_from.as_ref() == Some(keep);
        if self.tried.iter().any(|tried| tried.as_ref() != Some(keep)) {
            return Some(
                "another login was tried in this browser, so this one's logins are not kept: to \
                 keep them, log in from a new browser (browser_new, then browser_login)"
                    .to_owned(),
            );
        }
        if self.opened_from.is_none() && !self.fresh.load(std::sync::atomic::Ordering::SeqCst) {
            return Some(
                "this browser went to other pages before the login, so its logins are not \
                 kept: to keep them, log in from a new browser (browser_new, then \
                 browser_login)"
                    .to_owned(),
            );
        }
        let Ok(store) = store_of(keep) else {
            return Some("its logins could not be kept".to_owned());
        };
        if !a_copy && self.mark(&store).is_err() {
            return Some("its logins could not be kept".to_owned());
        }
        match crate::browserlogins::hold_since(&self.home, &store, self.began) {
            Ok(Some(hold)) => {
                self.writes = Some((keep.clone(), hold));
                None
            }
            Ok(None) if a_copy => Some(
                "another browser holds these kept logins, so this one opened a copy of them: \
                 what it changes is not kept"
                    .to_owned(),
            ),
            Ok(None) => Some(
                "another browser keeps these logins, so what this one holds is not kept".to_owned(),
            ),
            Err(_) => Some("its logins could not be kept".to_owned()),
        }
    }

    /// Writes the store back from the browser's folder, where it holds one: once its engine has
    /// ended. A store that cannot be written keeps what it held before; one whose profile was
    /// removed meanwhile is not made again.
    fn write_back(&mut self) {
        let Some((kept, hold)) = self.writes.take() else {
            return;
        };
        let written = self.keys().and_then(|keys| hold.keep(&self.folder, &keys));
        if let Err(why) = written {
            tracing::warn!(
                "purlis: the kept logins of {}'s {} were not written back: {why}",
                kept.profile,
                kept.login
            );
        }
    }
}

/// Who drives a browser after a login, in the protocol's words: never purlis, which hands every
/// browser back; were it to say so, the browser is held from the agent.
fn driver_of(driver: drive::Driver) -> wire::Driver {
    match driver {
        drive::Driver::Agent => wire::Driver::Agent,
        drive::Driver::Person | drive::Driver::Purlis => wire::Driver::Person,
    }
}

/// **Why DevTools closed, in the word their view's last frame says** (#2028): one of
/// `view::frames::CLOSED_WHY`.
fn closed_word(why: drive::InspectEnded) -> &'static str {
    match why {
        drive::InspectEnded::Closed => "handed_back",
        drive::InspectEnded::PageGone => "page_gone",
        drive::InspectEnded::FellBehind => "fell_behind",
        drive::InspectEnded::TooLarge => "too_large",
        drive::InspectEnded::Moved => "moved",
        drive::InspectEnded::Ended => "ended",
    }
}

/// Why the driver did not carry an act or a login out, as the host says it.
fn not_acted(refusal: drive::Refusal) -> NotActed {
    match refusal {
        drive::Refusal::Ended => NotActed::Ended,
        other => NotActed::Refused(other.to_string()),
    }
}

/// A running browser and its reach proxy, which lives exactly as long as it.
struct Held {
    browser: Arc<Browser>,
    proxy: Arc<ReachProxy>,
}

/// One browser on its engine, as the host drives it.
struct EngineScreen {
    /// Its own data folder (#2020): its downloads are held in it, and each upload copied into
    /// it, where no chat reads or writes.
    data: PathBuf,
    /// How many uploads it was given, each copied into a folder of its own.
    uploads: std::sync::atomic::AtomicU64,
    process: Option<u32>,
    /// Where its engine writes its log (#2745).
    log: PathBuf,
    /// What it keeps of a kept login (#2021): written back as its engine ends.
    keeping: Arc<Mutex<Keeping>>,
    /// Whether it has done nothing yet (#2693): its keeping's, marked without its lock.
    fresh: Arc<std::sync::atomic::AtomicBool>,
    /// Its judge, shared with its reach proxy (#2018): none for a browser given no reach.
    sites: Option<Arc<judge::SiteJudge>>,
    held: Mutex<Option<Held>>,
}

impl EngineScreen {
    /// It acted, or the person drove it (#2693).
    fn browsed(&self) {
        self.fresh.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    fn with<T>(&self, act: impl FnOnce(&Arc<Browser>) -> T) -> Option<T> {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|held| act(&held.browser))
    }
}

impl EngineScreen {
    /// **The upload of `files` of the chat's folder `from` into `target`** (#2020), as the
    /// driver's call: each file copied into a folder of the browser's own first
    /// ([`crate::browserfiles::stage`]), and the driver handed the copies.
    fn staged(
        &self,
        target: &str,
        files: &[String],
        from: Option<&str>,
    ) -> Result<drive::Call, NotActed> {
        let from = from.ok_or_else(|| {
            NotActed::Refused("an upload names the chat's folder it reads from".into())
        })?;
        let n = self
            .uploads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let into = self.data.join(UPLOADS).join(n.to_string());
        let copies =
            crate::browserfiles::stage(Path::new(from), files, &into).map_err(NotActed::Refused)?;
        Ok(drive::Call::Upload {
            target: Ref::new(target),
            files: copies,
        })
    }

    /// **Brings download `download` into the chat's folder `into` as `to`** (#2020): only one
    /// that is done, as a new file ([`crate::browserfiles::bring`]). Answered with the browser's
    /// page as it is, and a note of what was brought.
    fn save_download(
        &self,
        epoch: u64,
        download: &str,
        to: &str,
        into: Option<&str>,
    ) -> Result<Browsed, NotActed> {
        let into = into.ok_or_else(|| {
            NotActed::Refused("bringing a download in names the chat's folder".into())
        })?;
        let browser = self.with(Arc::clone).ok_or(NotActed::Ended)?;
        let one = browser
            .downloads()
            .into_iter()
            .find(|one| one.id == download)
            .ok_or_else(|| {
                NotActed::Refused(format!(
                    "this browser has no download {download}: list its downloads to see them"
                ))
            })?;
        if one.state != drive::DownloadState::Done {
            return Err(NotActed::Refused(format!(
                "download {download} is not done ({}), so it is not brought in",
                state_said(one.state)
            )));
        }
        let from = self.data.join(DOWNLOADS).join(&one.file);
        let bytes =
            crate::browserfiles::bring(&from, Path::new(into), to).map_err(NotActed::Refused)?;
        let id = CallId::new(format!("kept:{download}"))
            .map_err(|why| NotActed::Refused(why.to_string()))?;
        let listed = browser
            .call_at(epoch, &id, drive::Call::Downloads)
            .map_err(not_acted)?;
        let mut out = browsed(listed, MOST_SCREENSHOT_BYTES);
        out.note = Some(format!(
            "brought {download} ({}, {}) into the chat's folder as {to}",
            one.name,
            crate::browserfiles::bytes_said(bytes)
        ));
        Ok(out)
    }
}

/// How far a download came, in words.
fn state_said(state: drive::DownloadState) -> &'static str {
    match state {
        drive::DownloadState::Going => "still downloading",
        drive::DownloadState::Done => "done",
        drive::DownloadState::Cancelled => "cancelled",
        drive::DownloadState::TooLarge => "cancelled by purlis: larger than a download may be",
        drive::DownloadState::TooMany => {
            "cancelled by purlis: the browser had as many downloads as it may"
        }
    }
}

impl Screen for EngineScreen {
    fn show(&self, frames: FrameSink) {
        self.with(|browser| {
            browser.show(Arc::new(move |painted: drive::Painted| {
                frames(
                    aim_of(&painted.aim),
                    painted.address,
                    Bytes::from(painted.jpeg),
                );
            }));
        });
    }

    fn pace(&self, every: Duration) {
        self.with(|browser| browser.pace(every));
    }

    fn narrow(&self, most: Option<u16>) {
        self.with(|browser| browser.narrow(most));
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
        self.browsed();
        let id = CallId::new(call_id).map_err(|why| NotActed::Refused(why.to_string()))?;
        let call = match act {
            Act::Upload {
                target,
                files,
                from,
            } => self.staged(&target, &files, from.as_deref())?,
            Act::SaveDownload { download, to, into } => {
                return self.save_download(epoch, &download, &to, into.as_deref());
            }
            act => call_of(act).map_err(NotActed::Refused)?,
        };
        // The call waits on the engine: it is made outside the lock, so frames and a stop are
        // never held up behind it.
        let browser = self.with(Arc::clone);
        browser
            .map_or(Err(drive::Refusal::Ended), |browser| {
                browser.call_at(epoch, &id, call)
            })
            .map(|answer| browsed(answer, MOST_SCREENSHOT_BYTES))
            .map_err(not_acted)
    }

    fn log_in(&self, asked: &wire::LogIn) -> Result<LoginEnded, NotActed> {
        let (mut login, site) = login_of(asked).map_err(NotActed::Refused)?;
        // Opened from this very kept login, a page that asks for nothing is signed in already.
        // Tried from here, whatever comes of it (#2693).
        let opened_from = {
            let mut keeping = self.keeping.lock().unwrap_or_else(PoisonError::into_inner);
            keeping.tried(asked.keep.as_ref());
            keeping.opened_from.clone()
        };
        login.only_if_asked = asked.keep.is_some() && opened_from == asked.keep;
        let Some(sites) = &self.sites else {
            return Err(NotActed::Refused(
                "this browser was started with no reach, so purlis logs nothing in to it"
                    .to_owned(),
            ));
        };
        let (browser, proxy) = {
            let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
            let held = held.as_ref().ok_or(NotActed::Ended)?;
            (Arc::clone(&held.browser), Arc::clone(&held.proxy))
        };
        let ended = performed(sites, &proxy, &site, &login.origin, |bind| {
            browser.log_in(&login, bind)
        })
        .map_err(NotActed::Refused)?
        .map_err(not_acted)?;
        drop(login);
        let mut outcome = ended
            .outcome
            .map(|answer| browsed(answer, MOST_SCREENSHOT_BYTES))
            .map_err(not_acted);
        // A login that went through keeps what its browser holds, where it is to (#2021).
        if let (Ok(browsed), Some(keep)) = (&mut outcome, &asked.keep) {
            let said = self
                .keeping
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .keep(keep);
            if let Some(said) = said {
                browsed.note = Some(match browsed.note.take() {
                    Some(note) => format!("{note}; {said}"),
                    None => said,
                });
            }
        }
        Ok(LoginEnded {
            driver: driver_of(ended.driver),
            epoch: ended.epoch,
            outcome,
        })
    }

    fn take_over(&self) -> u64 {
        // Outside the lock, as an act is: it returns once no act of the agent's can reach a
        // page.
        self.with(Arc::clone).map_or(0, |browser| {
            self.browsed();
            browser.take_over()
        })
    }

    fn hand_back(&self) -> u64 {
        self.with(Arc::clone)
            .map_or(0, |browser| browser.hand_back())
    }

    fn input(&self, input: &wire::Input) -> Result<(), NotTaken> {
        let browser = self.with(Arc::clone).ok_or(NotTaken::Ended)?;
        // Before it is sent: an input refused may still have reached the page.
        self.browsed();
        browser.person_input(&input_of(input)).map_err(not_taken)
    }

    fn size_to(&self, width: u16, height: u16) -> Result<(), NotTaken> {
        let browser = self.with(Arc::clone).ok_or(NotTaken::Ended)?;
        browser.size_to(width, height).map_err(not_taken)
    }

    /// The engine's own DevTools of the page it shows, for the person (#2028).
    fn inspect(
        &self,
        width: u16,
        height: u16,
        ended: Box<dyn FnOnce(&'static str) + Send>,
    ) -> Result<(), NotTaken> {
        let browser = self.with(Arc::clone).ok_or(NotTaken::Ended)?;
        self.browsed();
        browser
            .inspect(width, height, Box::new(move |why| ended(closed_word(why))))
            .map_err(not_taken)
    }

    fn show_inspector(&self, frames: FrameSink) {
        self.with(|browser| {
            browser.show_inspector(Arc::new(move |painted: drive::Painted| {
                frames(
                    aim_of(&painted.aim),
                    painted.address,
                    Bytes::from(painted.jpeg),
                );
            }));
        });
    }

    fn hide_inspector(&self) {
        self.with(|browser| browser.hide_inspector());
    }

    fn inspector_input(&self, input: &wire::Input) -> Result<(), NotTaken> {
        let browser = self.with(Arc::clone).ok_or(NotTaken::Ended)?;
        browser.inspector_input(&input_of(input)).map_err(not_taken)
    }

    /// Told once the engine has ended, however it ended, with how (#2745); at once when it was
    /// stopped already.
    fn when_it_ends(&self, tell: Box<dyn FnOnce(ScreenEnded) + Send>) {
        // A kept login is written back once its engine has ended and before the host removes
        // the browser's folder (#2021).
        let keeping = Arc::clone(&self.keeping);
        let tell: Box<dyn FnOnce(ScreenEnded) + Send> = Box::new(move |ended| {
            keeping
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .write_back();
            tell(ended);
        });
        let log = self.log.clone();
        let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
        match held.as_ref() {
            Some(Held { browser, .. }) => {
                browser.when_it_ends(Box::new(move |ending| tell(ended_of(&ending, &log))));
            }
            None => {
                drop(held);
                tell(ScreenEnded {
                    exit: Exit::Code(0),
                    why: format!(
                        "purlis had stopped its engine already; its engine's log is {}",
                        log.display()
                    ),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests;
