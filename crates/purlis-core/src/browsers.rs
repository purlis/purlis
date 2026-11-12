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
//! - **Its secret fills** (#5115, #5117): the driver fills a vault value into one field
//!   ([`drive::Browser::fill_secret`]), and just before the value goes in the browser's reach
//!   proxy is held to the field's site ([`FillHolder`]) until the page holding it loads another
//!   document, or an error page, in its main frame or is closed. Meanwhile an address the agent
//!   writes for another site is refused before the driver hears of it, in words that say why
//!   ([`held_to_said`]), and a click or Back that leads elsewhere answers with the same reason
//!   ([`load_held_said`], #5150). A redirect from that site's own server, after a form sent there,
//!   may take the page to another (#5230, SF-14, [`judge::SiteJudge::may_leave_for`]), and a form
//!   may be sent to a site its key is also approved for (SF-15).
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

use crate::sandbox::browser_reach::{Bound, Lane, LoginSites, NewSites, ReachProxy, Site};
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
        // And its form's, where that is another (#5167): reached from its start, as a browser
        // logged in afresh does.
        let kept_form = kept
            .and_then(|kept| kept.form.as_deref())
            .map(site_of_login)
            .transpose()?
            .filter(|form| Some(form) != kept_site.as_ref());
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
        // Its first download is told as it goes, and stops where the person cancels it (#2207).
        let https = archive_reads();
        let archives = Progressing::new(&https, download, DOWNLOAD_TOLD_EVERY);
        let ready = prepared(
            &engines,
            &settings,
            &manifest_reads(),
            &archives,
            engines_clock(
                cfg!(debug_assertions),
                crate::envvar::var(ENGINES_CLOCK_VAR).as_deref(),
                chrono::Utc::now(),
            ),
        )
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
            // Its page's site, and its form's where that is another (#5167), with no sign-in open:
            // the agent is kept off the form's pages until a login performed in it signs in there
            // (#4995, #5152).
            let at = LoginSites {
                page: site.clone(),
                form: kept_form.clone(),
            };
            opened_from_kept(&proxy, sites, &at)?;
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
        // The agent is kept off the site its login signs in on, outside a sign-in (#5152).
        if let Some(sites) = &sites {
            let bound = sites.bound().clone();
            browser.keep_off(Arc::new(move |address: &str| kept_off_at(&bound, address)));
            // During a sign-in, on every page but its own and the windows related to it (the
            // person's ruling of 2026-11-06).
            let bound = sites.bound().clone();
            browser.keep_sign_in_to_its_page(Arc::new(move |address: &str| {
                let (host, port) = judge::site_of(address)?;
                bound
                    .signing_in_at(&host, port)
                    .map(|site| site.to_string())
            }));
        }
        // A click or Back the proxy refused under a fill's hold answers why (#5150).
        let bound = proxy.bound().clone();
        browser.say_why_refused(Arc::new(move |address: &str| {
            load_held_said(&bound, address)
        }));
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

/// The engine a browser starts on at `now`: the trusted one, after reading the published
/// manifest through `manifests` first when that engine is behind past its warning or refused
/// for its age, so a machine whose background check has not run yet is not refused for an engine
/// already superseded. Where that read fails, the first answer stands, the refusal included.
fn prepared(
    engines: &Engines,
    settings: &EngineSettings,
    manifests: &dyn Fetch,
    archives: &dyn Fetch,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Ready, Refused> {
    let first = engines.prepare(settings, archives, now);
    let behind = match &first {
        Ok(ready) => matches!(ready.age(), Age::Behind { .. }),
        Err(refused) => matches!(refused, Refused::Outdated { .. }),
    };
    if !behind {
        return first;
    }
    let checked = update(engines, settings, manifests, archives, now);
    report(&checked);
    checked.or(first)
}

/// In a debug build, the time a browser's start judges its engine's age by, as RFC 3339 (#5413):
/// a test's host names the moment its pin was issued, so what it runs does not turn on today's
/// date or on reading the published manifest. A release build reads the real time whatever this
/// says, so it changes nothing a real install refuses.
pub const ENGINES_CLOCK_VAR: &str = "PURLIS_TEST_ENGINE_NOW";

/// When the manifest this build ships was issued: the time a test's host names in
/// [`ENGINES_CLOCK_VAR`], so a refreshed pin needs no test changed.
pub fn when_the_pin_was_issued() -> chrono::DateTime<chrono::Utc> {
    Manifest::shipped().issued
}

/// The time an engine's age is judged by: `real`, unless this is a debug build and `asked` is a
/// time. Handed its facts rather than reading `cfg!` itself, so the rule is testable in both
/// directions from one build.
fn engines_clock(
    debug_build: bool,
    asked: Option<&str>,
    real: chrono::DateTime<chrono::Utc>,
) -> chrono::DateTime<chrono::Utc> {
    asked
        .filter(|_| debug_build)
        .and_then(|asked| chrono::DateTime::parse_from_rfc3339(asked).ok())
        .map_or(real, |asked| asked.to_utc())
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
        Act::FindLoginForm { address: to } => Call::FindLoginForm {
            address: address(&to)?,
        },
        Act::AsksToSignIn => Call::AsksToSignIn,
        Act::DescribeField { target: on } => Call::DescribeField {
            target: Ref::new(on),
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
        drive::Content::Field(field) => out.field = Some(field_said(field)),
        drive::Content::AsksToSignIn(asks) => out.asks_to_sign_in = Some(asks),
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

/// **A field the engine read, in the protocol's words** (#5115).
fn field_said(field: drive::FieldInfo) -> wire::FieldSaid {
    wire::FieldSaid {
        origin: field.origin,
        label: field.label,
        kind: match field.kind {
            drive::FieldKind::OneLine => wire::FieldKind::OneLine,
            drive::FieldKind::MultiLine => wire::FieldKind::MultiLine,
            drive::FieldKind::Editable => wire::FieldKind::Editable,
            drive::FieldKind::Other => wire::FieldKind::Other,
        },
        by_get: Some(field.by_get),
        payment: Some(field.payment),
    }
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

/// **The driver's login for what the window handed** (#2016): the address, the origin it fills
/// on, the sites it is held to, and the values, or why it is no login a browser takes. A login
/// whose form is on another origin (#4995) fills on the form's alone, and is held to its page's
/// site and its form's (#5152), the agent kept off the form's outside a sign-in.
pub fn login_of(login: &wire::LogIn) -> Result<(drive::Login, LoginSites), String> {
    // Where the chat named its sign-in's start (#6291), judged here again, as it opens: the
    // login's page stays what the browser is held to.
    let opens = match &login.start {
        Some(start) => crate::loginstart::start_of(start, &login.address, login.form.as_deref())?,
        None => login.address.clone(),
    };
    let address = WebAddress::parse(&opens)
        .map_err(|why| format!("the login's address is not one a browser opens: {why}"))?;
    // https, or plain http on this machine's loopback alone; a name in its ASCII spelling, as
    // the engine records a page's address.
    let page = crate::sandbox::tools::login::origin_of(&login.address)?;
    let origin = match &login.form {
        Some(form) => crate::sandbox::tools::login::form_of(form)?,
        None => page.clone(),
    };
    let site = |origin: &str| {
        judge::site_of(origin)
            .map(|(host, port)| Site::of(&host, port))
            .ok_or_else(|| "the login's address names no origin purlis can fill on".to_owned())
    };
    let back_to = (origin != page).then(|| page.clone());
    let sites = LoginSites {
        page: site(&page)?,
        form: back_to.as_ref().map(|_| site(&origin)).transpose()?,
    };
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
            suggested: login.suggested,
            clean_already: false,
            back_to,
        },
        sites,
    ))
}

/// **How long a sign-in on a login's form's site lasts at most** (#4995, #5152), from the moment
/// the browser is held to the login's sites: long enough for the login, and for a step the form's
/// site asks for after the password, taken by the agent or by the person. Outside it, the agent
/// is kept off that site's pages, which the browser still reaches.
pub const SIGN_IN_WITHIN: Duration = Duration::from_secs(120);

/// **A login purlis performs in a browser whose judge is `sites` and whose reach proxy is
/// `proxy`** (#2016), at `at`, on `origin`: `run` performs it, and is handed its [`Holder`],
/// which binds the browser to the login's sites just before the first value goes in. While it
/// runs, what purlis sends to the login's sites goes; once bound, the proxy carries the page's
/// site, and its form's where that is another (#4995, #5152), for the browser's life, every
/// connection it carried before ended. The agent acts on the form's site's pages during the
/// login's sign-in alone: until the login's page is back on its page's origin, the login fails,
/// or `within` has passed ([`Holder`]). Refused, running nothing, for a browser that holds
/// another page's login already, or that a page holding a filled secret holds to another site.
pub fn performed<T>(
    sites: &Arc<judge::SiteJudge>,
    proxy: &Arc<ReachProxy>,
    at: &LoginSites,
    origin: &str,
    within: Duration,
    run: impl FnOnce(&Arc<Holder>) -> T,
) -> Result<T, String> {
    // A page holding a secret purlis filled holds the browser to that site alone (#5117), so a
    // login that needs another would only wait on a page that can't load (#5121).
    if let Some(held) = sites.bound().held_for_fill()
        && at.all().iter().any(|site| *site != held)
    {
        return Err(format!(
            "this browser reaches only {held} while a page in it holds a secret purlis filled \
             there, so nothing was filled on {origin}: once that page loads another of its own \
             pages or is closed, call browser_login again, or log in from a new browser with \
             browser_new"
        ));
    }
    if let Some(held) = sites.bound().page()
        && held != at.page
    {
        return Err(format!(
            "this browser holds a login at {held}, and reaches nothing else: open a new browser \
             with browser_new and log in to {origin} there",
        ));
    }
    sites.logging_in(&at.all());
    let holder = Arc::new(Holder {
        sites: Arc::clone(sites),
        proxy: Arc::downgrade(proxy),
        at: at.clone(),
        within,
        sign_in: Mutex::new(None),
        since: Mutex::new(None),
        bound: std::sync::atomic::AtomicBool::new(false),
    });
    let done = run(&holder);
    sites.done_logging_in();
    Ok(done)
}

/// **Whether a login that answered counts as signed in, for a later call of it** (#5121): only
/// where the browser is held to the login's page. A login that answered with the browser held to
/// nothing bound nothing, so its page's asking for nothing says nothing of a session.
fn counts_as_signed_in(answered: bool, held_to: Option<&Site>, page: &Site) -> bool {
    answered && held_to == Some(page)
}

/// **What holds a browser to a login purlis performs in it** (#2016, #4995), handed to its
/// engine. It binds the browser to the login's sites. Where its form is on another site, a
/// sign-in opens there: from the bind, or, in a browser held to the login's page already (opened
/// from kept logins, or logged in before), from the moment the login begins. It ends the sign-in
/// (the agent is kept off the form's site's pages from then on, which the browser still reaches,
/// the person's ruling of 2026-11-04) as soon as the first of these comes: the login's own page
/// is back on its page's origin ([`drive::Hold::back`]), the login answered with nothing to
/// finish there ([`Self::answered`]), or its `within` has passed. Another login asks again.
pub struct Holder {
    sites: Arc<judge::SiteJudge>,
    proxy: std::sync::Weak<ReachProxy>,
    at: LoginSites,
    within: Duration,
    /// The sign-in it opened, if it opened one.
    sign_in: Mutex<Option<u64>>,
    /// When it opened it.
    since: Mutex<Option<std::time::Instant>>,
    /// Whether it bound the browser: a value may have gone in.
    bound: std::sync::atomic::AtomicBool,
}

impl Holder {
    /// **The sign-in this login opened is over**: the agent is kept off the form's site's pages
    /// from now (#5152). Nothing where it opened none, or it is over already.
    pub fn sign_in_over(&self) {
        let number = *self.sign_in.lock().unwrap_or_else(PoisonError::into_inner);
        if let (Some(number), Some(proxy)) = (number, self.proxy.upgrade()) {
            proxy.sign_in_over(number);
        }
    }

    /// **The login answered, as `ended`** (#4995): its sign-in is over unless there is something
    /// left to finish on the form's site, a login that went through (a step the site asks for
    /// after the password) or one the person took over. A login that never bound fills nothing
    /// there, and one that failed with the agent's browser handed back is over.
    pub fn answered(&self, ended: &Result<drive::LoginEnded, drive::Refusal>) {
        let bound = self.bound.load(std::sync::atomic::Ordering::SeqCst);
        let to_finish = matches!(
            ended,
            Ok(ended) if ended.outcome.is_ok() || ended.driver != drive::Driver::Agent
        );
        if !(bound && to_finish) {
            self.sign_in_over();
        }
    }

    /// **Opens a sign-in of its own on `proxy`**, sign-in `number`, ended once `within` has
    /// passed. One it opened already goes on.
    fn opened(&self, proxy: &Arc<ReachProxy>, number: u64) -> Result<(), String> {
        let was = self
            .sign_in
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(number);
        if was == Some(number) {
            return Ok(());
        }
        *self.since.lock().unwrap_or_else(PoisonError::into_inner) =
            Some(std::time::Instant::now());
        let ending = Arc::downgrade(proxy);
        let within = self.within;
        let ends = std::thread::Builder::new()
            .name("browser-sign-in".to_owned())
            .spawn(move || {
                std::thread::sleep(within);
                if let Some(proxy) = ending.upgrade() {
                    proxy.sign_in_over(number);
                }
            });
        // No thread to end it: it ends now, and the login cannot fill.
        if ends.is_err() {
            proxy.sign_in_over(number);
            return Err("purlis could not time the sign-in".to_owned());
        }
        Ok(())
    }

    /// **Its browser's reach proxy**, or why there is none: the browser has ended.
    fn proxy(&self) -> Result<Arc<ReachProxy>, String> {
        self.proxy
            .upgrade()
            .ok_or_else(|| "the browser's reach proxy has stopped".to_owned())
    }

    /// **The form's site the agent may act on while this login's sign-in is open**, and how long
    /// it has left, if it is open.
    pub fn signing_in(&self) -> Option<(Site, Duration)> {
        let number = (*self.sign_in.lock().unwrap_or_else(PoisonError::into_inner))?;
        let since = (*self.since.lock().unwrap_or_else(PoisonError::into_inner))?;
        let form = self.sites.bound().signing_in(number)?;
        Some((form, self.within.saturating_sub(since.elapsed())))
    }
}

impl drive::Hold for Holder {
    fn begins(&self) -> Result<(), String> {
        // A browser held to this login's page already reaches the form's site from now: its
        // page may send it there before anything is filled.
        let Some(form) = &self.at.form else {
            return Ok(());
        };
        if self.sites.bound().page().as_ref() != Some(&self.at.page) {
            return Ok(());
        }
        let proxy = self.proxy()?;
        self.sites.bound_to(form);
        match proxy.sign_in(&self.at)? {
            Some(number) => self.opened(&proxy, number),
            None => Ok(()),
        }
    }

    fn bind(&self) -> Result<(), String> {
        let proxy = self.proxy()?;
        // A sign-in this login opened as it began goes on; the browser is held to its page.
        let open = (*self.sign_in.lock().unwrap_or_else(PoisonError::into_inner))
            .is_some_and(|number| self.sites.bound().signing_in(number).is_some());
        let sign_in = if open {
            proxy.bind(&LoginSites::page(self.at.page.clone()))?;
            None
        } else {
            proxy.bind(&self.at)?
        };
        self.bound.store(true, std::sync::atomic::Ordering::SeqCst);
        for site in self.at.all() {
            self.sites.bound_to(&site);
        }
        match sign_in {
            Some(number) => self.opened(&proxy, number),
            None => Ok(()),
        }
    }

    fn back(&self) {
        self.sign_in_over();
    }
}

/// **What holds a browser to the site a secret was filled on, while the page holding it lives**
/// (#5117; the person's ruling SF-3 and decision SF-9 on #5096), handed to its engine: it holds
/// the browser's reach proxy to that site just before the value goes in, and lets it go once the
/// engine says the page is gone.
pub struct FillHolder {
    proxy: std::sync::Weak<ReachProxy>,
    site: Site,
    /// The other sites the key is approved for (#5230, SF-15): a form may be sent there.
    also: Vec<Site>,
    /// What judges a filled page leaving for another site (#5230): where it has none, it stays.
    sites: Option<std::sync::Weak<judge::SiteJudge>>,
    /// The hold it made, once it made one.
    number: Mutex<Option<u64>>,
}

impl FillHolder {
    /// What holds the browser `proxy` serves to `origin`'s site (`scheme://host[:port]`), or why
    /// that names none.
    pub fn of(proxy: &Arc<ReachProxy>, origin: &str) -> Result<Self, String> {
        let (host, port) = judge::site_of(origin)
            .ok_or_else(|| format!("{origin} is not a site purlis can hold a browser to"))?;
        let site = Site::of(&host, port);
        // A browser bound to a login takes no secret for another site: refused before anything
        // is looked for or filled.
        if let Some(why) = proxy.bound().takes_no_fill_for(&site) {
            return Err(why);
        }
        Ok(Self {
            proxy: Arc::downgrade(proxy),
            site,
            also: Vec::new(),
            sites: None,
            number: Mutex::new(None),
        })
    }

    /// This holder, reaching `origins` too, the other sites the key is approved for (#5230,
    /// SF-15), and asking `sites` where a filled page may leave for. An origin that names no site
    /// is none.
    #[must_use]
    pub fn judged_by(mut self, sites: Option<&Arc<judge::SiteJudge>>, origins: &[String]) -> Self {
        self.also = origins
            .iter()
            .filter_map(|origin| judge::site_of(origin))
            .map(|(host, port)| Site::of(&host, port))
            .collect();
        self.sites = sites.map(Arc::downgrade);
        self
    }
}

impl drive::FillHold for FillHolder {
    fn hold(&self) -> Result<(), String> {
        let proxy = self
            .proxy
            .upgrade()
            .ok_or_else(|| "the browser's reach proxy has stopped".to_owned())?;
        let number = proxy.hold_for_fill_also(&self.site, &self.also)?;
        *self.number.lock().unwrap_or_else(PoisonError::into_inner) = Some(number);
        Ok(())
    }

    fn released(&self) {
        self.released_then(Box::new(|| {}));
    }

    fn released_then(&self, then: Box<dyn FnOnce() + Send>) {
        let number = self
            .number
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(number) = number else {
            return;
        };
        // The page that left may still send as it unloads, once the next document has come:
        // it is let go a while after. Where no thread can time that, it is held on, and whoever
        // waits to go on is never told.
        let proxy = self.proxy.clone();
        let _ = std::thread::Builder::new()
            .name("browser-fill-released".to_owned())
            .spawn(move || {
                std::thread::sleep(FILL_RELEASED_AFTER);
                if let Some(proxy) = proxy.upgrade() {
                    proxy.fill_released(number);
                }
                then();
            });
    }

    fn may_leave_for(&self, address: &str, chain_began: bool) -> drive::Leaving {
        match self.sites.as_ref().and_then(std::sync::Weak::upgrade) {
            Some(sites) => sites.may_leave_from(&self.site, address, chain_began),
            None => drive::Leaving::No {
                why: format!(
                    "purlis stopped this page leaving {}: this browser was started with no \
                     reach, and the page stays where it was",
                    self.site
                ),
            },
        }
    }

    fn approved_for(&self, address: &str) -> bool {
        // A key is approved for another site over https alone, and the host and port say no scheme.
        address
            .get(..6)
            .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https:"))
            && judge::site_of(address)
                .is_some_and(|(host, port)| self.also.contains(&Site::of(&host, port)))
    }
}

/// **How long after the page holding a filled secret is gone its browser stays held** (#5117):
/// the page that left may still send as it unloads, which the engine runs once the next document
/// has come.
pub const FILL_RELEASED_AFTER: Duration = Duration::from_secs(2);

/// **Why the agent's `act` may not go where it would** (#5117): an address it wrote for another
/// site than the one a page holding a secret purlis filled holds the browser to, in words that
/// say why and what ends it. `None` for any other act, and while nothing holds the browser.
pub fn held_to_said(bound: &Bound, act: &Act) -> Option<String> {
    let address = match act {
        Act::Open { address } | Act::FindLoginForm { address } | Act::Navigate { address, .. } => {
            address
        }
        _ => return None,
    };
    let held = bound.held_for_fill()?;
    let (host, port) = judge::site_of(address)?;
    let to = Site::of(&host, port);
    if to == held {
        return None;
    }
    Some(fill_hold_said(&held, &format!("it did not go to {to}")))
}

/// **Why a page shows the engine's own page for `address`** (#5150): a load of another site than
/// the one a page holding a secret purlis filled holds the browser to, in words that say why and
/// what ends it. They hold of a load that failed before the hold began too, so they name no cause
/// of this one. `None` for a load that failed on that site, and while nothing holds the browser.
/// The address may carry what a filled page sent, so neither it nor its site is said.
pub fn load_held_said(bound: &Bound, address: &str) -> Option<String> {
    let held = bound.held_for_fill()?;
    let (host, port) = judge::site_of(address)?;
    (Site::of(&host, port) != held)
        .then(|| fill_hold_said(&held, "pages of other sites do not load"))
}

/// What a chat is told of `what` while a filled page holds its browser to `held` (#5117).
fn fill_hold_said(held: &Site, what: &str) -> String {
    format!(
        "this browser reaches only {held} while a page in it holds a secret purlis filled there, \
         so {what}. Once the page holding the secret loads another of its own pages, or is closed \
         with browser_close or by the person, the browser reaches as before a moment later"
    )
}

/// **`<data>/browsers/`, made this user's alone and left out of this device's backups** (#2021):
/// a running browser's folder holds its cookies and storage as plainly as the engine writes them.
fn made_private_and_unbacked(folders: &Path) -> Result<(), String> {
    purlis_same_user::private_directory(folders)
        .map_err(|e| format!("the browsers' folder could not be made ({e})"))?;
    crate::browserlogins::exclude_from_backups(folders)
}

/// The site a kept login's page is on, as a login's is ([`login_of`]).
/// **A browser opened from a kept login is held to `at`, its sites, from its start** (#2021,
/// #5167): its page's, and its form's where that is another, as a browser logged in afresh is
/// once its sign-in is over, so no sign-in is open and the agent is kept off the form's pages.
fn opened_from_kept(
    proxy: &ReachProxy,
    sites: &judge::SiteJudge,
    at: &LoginSites,
) -> Result<(), String> {
    if let Some(sign_in) = proxy.bind(at)? {
        proxy.sign_in_over(sign_in);
    }
    for one in at.all() {
        sites.bound_to(&one);
    }
    Ok(())
}

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
    /// The identity of each login that went through in it, with the browser held to its page
    /// (#5121): one of these again fills nothing where its page asks for nothing.
    went_through: Vec<String>,
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
            went_through: Vec::new(),
        }
    }

    /// The browser acted, or the person took it over: what it holds from now may be of other
    /// sites, or another session of the login's own, so it is no longer kept as a login's
    /// store unless it was opened from that store (#2693).
    #[cfg(test)]
    fn browsed(&self) {
        self.fresh.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// **Its login's form didn't appear once its site was cleared** (#5272), so the login goes to a
    /// new browser: this one lets go of the store it opened from, for that one to keep, and writes
    /// nothing of what it cleared over it.
    fn no_form_on_clean_load(&mut self) {
        self.writes = None;
    }

    /// **Whether it has held nothing so far** (#5272): no act, no input of the person's, no login
    /// tried, and nothing opened from a kept login.
    fn held_nothing(&self) -> bool {
        self.fresh.load(std::sync::atomic::Ordering::SeqCst)
            && self.opened_from.is_none()
            && self.tried.is_empty()
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

    /// Whether a login of `identity` went through in this browser (#5121).
    fn went_through(&self, identity: Option<&str>) -> bool {
        identity.is_some_and(|identity| self.went_through.iter().any(|one| one == identity))
    }

    /// A login of `identity` runs now: no other counts as having gone through from here (#5121).
    fn only_goes_through(&mut self, identity: Option<&str>) {
        self.went_through
            .retain(|one| Some(one.as_str()) == identity);
    }

    /// A login that went through in it failed since: it counts no more (#5121).
    fn did_not_hold(&mut self, identity: Option<&str>) {
        self.went_through
            .retain(|one| Some(one.as_str()) != identity);
    }

    /// A login of `identity` went through, and left the browser held to its page (#5121).
    fn goes_through(&mut self, identity: Option<&str>) {
        if let Some(identity) = identity
            && !self.went_through(Some(identity))
        {
            self.went_through.push(identity.to_owned());
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
        drive::Refusal::KeptOff {
            site,
            asks_to_sign_in,
        } => NotActed::Refused(kept_off_said(&site, asks_to_sign_in)),
        drive::Refusal::NoFormOnCleanLoad(why) => NotActed::NoFormOnCleanLoad(why),
        other => NotActed::Refused(other.to_string()),
    }
}

/// **The site the agent is kept off at `address`, now** (#5152, the person's ruling of
/// 2026-11-04): the site the browser's login signs in on, where `address` is on it and no
/// sign-in is open ([`Bound::kept_off`]).
pub fn kept_off_at(bound: &Bound, address: &str) -> Option<String> {
    let (host, port) = judge::site_of(address)?;
    bound.kept_off(&host, port).map(|site| site.to_string())
}

/// What a chat is told of a call refused on a page or frame of `site`, the site its browser's
/// login signs in on (#5152): why, and what to do, `browser_login` where the page asks to sign
/// in again.
pub fn kept_off_said(site: &str, asks_to_sign_in: bool) -> String {
    let then = if asks_to_sign_in {
        "It shows its sign-in form again: call browser_login to sign in anew."
    } else {
        "Go back, or navigate to the login's own site; if it asks to sign in again, call \
         browser_login."
    };
    format!(
        "nothing was read or sent: this is a page of {site}, the site this browser signs in on, \
         and you may read or act on its pages only while browser_login signs in, on the page it \
         signs in on and the windows that page opened. {then}"
    )
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
        project: &Path,
    ) -> Result<drive::Call, NotActed> {
        let from = from.ok_or_else(|| {
            NotActed::Refused("an upload names the chat's folder it reads from".into())
        })?;
        let n = self
            .uploads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let into = self.data.join(UPLOADS).join(n.to_string());
        let copies =
            staged_files(project, Path::new(from), files, &into).map_err(NotActed::Refused)?;
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
        project: &Path,
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
        let whose = DownloadsSite::of(
            &one,
            browser.as_ref(),
            self.sites.as_ref().map(|sites| sites.bound()),
        );
        if let Some(why) = whose.refused(download) {
            return Err(NotActed::Refused(why));
        }
        let from = self.data.join(DOWNLOADS).join(&one.file);
        // What purlis filled into the browser is kept out of what the chat reads, a file it
        // brings in among it (#5115), and so is a long text the person typed in it (the
        // person's ruling of 2026-11-06).
        match browser.holds_kept_out(&from) {
            Ok(drive::DownloadHolds::Nothing) => {}
            Ok(drive::DownloadHolds::Filled) => {
                return Err(NotActed::Refused(format!(
                    "download {download} holds a secret purlis filled into this browser, which is \
                     kept out of what you read, so it is not brought in: ask the person to save \
                     it if it is needed"
                )));
            }
            Ok(drive::DownloadHolds::Typed) => {
                return Err(NotActed::Refused(format!(
                    "download {download} holds something the person typed into this browser, \
                     which is kept out of what you read, so it is not brought in: ask the person \
                     to save it if it is needed"
                )));
            }
            Err(why) => {
                return Err(NotActed::Refused(format!(
                    "purlis could not read download {download} to check it holds nothing kept \
                     out of what you read ({why}), so it is not brought in"
                )));
            }
        }
        let bytes = saved_file(project, &from, Path::new(into), to).map_err(NotActed::Refused)?;
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

/// What decides whether a done download is the chat's to bring in, by the site it may be of.
#[derive(Debug, PartialEq, Eq)]
struct DownloadsSite {
    /// The site the agent is kept off it is of, judged on the page it began on.
    kept_off: Option<String>,
    /// It came from no site purlis can tell (a sandboxed frame's, say).
    from_no_origin: bool,
    /// It began on the page the last login was performed on, or a window related to it.
    began_on_the_sign_ins_page: bool,
    /// The site the browser signs in on, which the agent is kept off outside a sign-in.
    keeps_off: Option<String>,
    /// The site a sign-in signs in on now.
    signing_on: Option<String>,
}

/// Where a browser says a download began: what [`DownloadsSite`] is gathered from.
trait DownloadsWhence {
    /// The site the agent is kept off that download `id` is of ([`Browser::download_kept_off`]).
    fn kept_off(&self, id: &str) -> Option<String>;
    /// Whether download `id` began on the sign-in's own page
    /// ([`Browser::download_began_on_the_sign_ins_page`]).
    fn began_on_the_sign_ins_page(&self, id: &str) -> bool;
}

impl DownloadsWhence for Browser {
    fn kept_off(&self, id: &str) -> Option<String> {
        self.download_kept_off(id)
    }

    fn began_on_the_sign_ins_page(&self, id: &str) -> bool {
        self.download_began_on_the_sign_ins_page(id)
    }
}

impl DownloadsSite {
    /// **What decides whether done download `one` is brought in**: where `browser` says it
    /// began, and what the browser's login `bound` holds now (none for a browser given no
    /// reach).
    fn of(
        one: &drive::DownloadInfo,
        browser: &impl DownloadsWhence,
        bound: Option<&Bound>,
    ) -> Self {
        Self {
            kept_off: browser.kept_off(&one.id),
            from_no_origin: one.origin.is_none(),
            began_on_the_sign_ins_page: browser.began_on_the_sign_ins_page(&one.id),
            keeps_off: bound.and_then(|bound| bound.keeps_off().map(|site| site.to_string())),
            signing_on: bound.and_then(|bound| bound.signing_on().map(|site| site.to_string())),
        }
    }

    /// **Why download `download` is not brought in**, or `None` where its site does not keep it.
    fn refused(&self, download: &str) -> Option<String> {
        // One from the site the browser signs in on is that site's to read (#5152), and during
        // a sign-in, only the sign-in's own page's (the person's ruling of 2026-11-06): it is
        // judged on the page it began on.
        if let Some(site) = &self.kept_off {
            return Some(format!(
                "download {download} came from {site}, the site this browser signs in on, so it \
                 is brought in only while browser_login signs in, and only where the page it signs \
                 in on, or a window related to it, began it"
            ));
        }
        if !self.from_no_origin {
            return None;
        }
        // One from no site purlis can tell may be that site's too (the steward's decision on
        // the person's rulings of 2026-11-06).
        if let Some(site) = &self.keeps_off {
            return Some(format!(
                "download {download} came from no site purlis can tell, and this browser signs in \
                 on {site}, so it may be that site's: it is brought in only while browser_login \
                 signs in"
            ));
        }
        // And during a sign-in, one begun anywhere but the sign-in's own page may be its site's,
        // which only that page reads (the person's ruling of 2026-11-06).
        if let Some(site) = &self.signing_on
            && !self.began_on_the_sign_ins_page
        {
            return Some(format!(
                "download {download} came from no site purlis can tell, and browser_login signs in \
                 on {site} now, so it may be that site's: it is brought in only where the page it \
                 signs in on, or a window related to it, began it"
            ));
        }
        None
    }
}

/// **The copies of `files` of the chat's folder `from` an upload hands the engine** (#2020),
/// made in `into`: each judged against what purlis keeps from the chats of the project at
/// `project`, as this host reads it, by what each file is, whatever the window judged by
/// spelling.
pub(crate) fn staged_files(
    project: &Path,
    from: &Path,
    files: &[String],
    into: &Path,
) -> Result<Vec<PathBuf>, String> {
    let kept = crate::browserfiles::Kept::of(project)?;
    crate::browserfiles::stage(from, files, into, &kept)
}

/// **Writes the download at `download` into the chat's folder `into` as `to`** (#2020): never
/// where the brokered-write guard refuses, on the spelling as written and as a folding disk
/// reads it, nor into a folder purlis keeps from the chats of the project at `project`, by what
/// it is. Answers the bytes written.
pub(crate) fn saved_file(
    project: &Path,
    download: &Path,
    into: &Path,
    to: &str,
) -> Result<u64, String> {
    let names = crate::browserfiles::relative(to)?;
    let mut written = into.to_path_buf();
    written.extend(names.iter());
    let mut folded = into.to_path_buf();
    folded.extend(names.iter().map(|name| crate::browserfiles::folded(name)));
    crate::brokered::guard(project, &written, None)?;
    crate::brokered::guard(project, &folded, None)?;
    let kept = crate::browserfiles::Kept::of(project)?;
    crate::browserfiles::bring(download, into, to, &kept)
}

/// How far a download came, in words.
fn state_said(state: drive::DownloadState) -> &'static str {
    match state {
        drive::DownloadState::Going => "still downloading",
        drive::DownloadState::Done => "done",
        drive::DownloadState::Cancelled => "cancelled",
        drive::DownloadState::TooLarge => "cancelled by purlis: larger than a download may be",
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
            Some(sites) => {
                sites.reach_now(reach)?;
                // A filled page that waited on the person to leave goes on, where it now may
                // (#5230).
                self.with(|browser| browser.reach_changed());
                Ok(())
            }
            None => Err("this browser was started with no reach, and takes none".to_owned()),
        }
    }

    fn act(
        &self,
        call_id: &str,
        act: Act,
        epoch: u64,
        project: &Path,
    ) -> Result<Browsed, NotActed> {
        self.browsed();
        let id = CallId::new(call_id).map_err(|why| NotActed::Refused(why.to_string()))?;
        let call = match act {
            Act::Upload {
                target,
                files,
                from,
            } => self.staged(&target, &files, from.as_deref(), project)?,
            Act::SaveDownload { download, to, into } => {
                return self.save_download(epoch, &download, &to, into.as_deref(), project);
            }
            act => {
                // A page holding a secret holds the browser to its site (#5117): an address the
                // agent wrote for another is refused here, saying why.
                let held = self
                    .held
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .as_ref()
                    .and_then(|held| held_to_said(held.proxy.bound(), &act));
                if let Some(why) = held {
                    return Err(NotActed::Refused(why));
                }
                call_of(act).map_err(NotActed::Refused)?
            }
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
        let (mut login, at) = login_of(asked).map_err(NotActed::Refused)?;
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
        let origin = login.origin.clone();
        let (ended, signing_in) =
            performed(sites, &proxy, &at, &origin, SIGN_IN_WITHIN, |holder| {
                // Opened from this very kept login, a page that asks for nothing is signed in
                // already. Tried from here, whatever comes of it (#2693), and only a login that
                // may run: one refused leaves what the browser keeps as it is (#5121).
                let (opened_from, again, held_nothing) = {
                    let mut keeping = self.keeping.lock().unwrap_or_else(PoisonError::into_inner);
                    let held_nothing = keeping.held_nothing();
                    keeping.tried(asked.keep.as_ref());
                    // The site is signed in as whichever login went last.
                    keeping.only_goes_through(asked.identity.as_deref());
                    let again = keeping.went_through(asked.identity.as_deref());
                    (keeping.opened_from.clone(), again, held_nothing)
                };
                // A browser that has held nothing has nothing to clear (#5272).
                login.clean_already = held_nothing;
                // And having gone through in it before, it may be signed in with it already (#5121).
                login.only_if_asked = again || (asked.keep.is_some() && opened_from == asked.keep);
                let ended = browser.log_in(&login, Arc::clone(holder) as Arc<dyn drive::Hold>);
                // A login with nothing left to finish on its form's site signs in no more
                // (#4995); one the person took over is theirs to finish meanwhile.
                holder.answered(&ended);
                (ended, holder.signing_in())
            })
            .map_err(NotActed::LoginNotRun)?;
        // A refusal by its lease is not one before it ran: what the browser keeps was told of
        // this login already.
        let ended = ended.map_err(not_acted)?;
        if matches!(ended.outcome, Err(drive::Refusal::NoFormOnCleanLoad(_))) {
            self.keeping
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .no_form_on_clean_load();
        }
        // Only where the browser is held to its page: nothing else counts as signed in (#5121).
        if counts_as_signed_in(
            ended.outcome.is_ok(),
            sites.bound().page().as_ref(),
            &at.page,
        ) {
            self.keeping
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .goes_through(asked.identity.as_deref());
        } else if ended.outcome.is_err() {
            self.keeping
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .did_not_hold(asked.identity.as_deref());
        }
        let (fills_on, page) = (login.origin.clone(), login.back_to.clone());
        drop(login);
        let mut outcome = ended
            .outcome
            .map(|answer| browsed(answer, MOST_SCREENSHOT_BYTES))
            .map_err(not_acted);
        // Still signing in on the form's site: what the agent may do there, and for how long
        // (#4995, #5152).
        if let (Ok(browsed), Some((_, left)), Some(page)) = (&mut outcome, signing_in, page) {
            let said = format!(
                "you may act on the pages of {fills_on} until this browser is back on {page}, for \
                 {} more seconds at most",
                left.as_secs().max(1),
            );
            browsed.note = Some(match browsed.note.take() {
                Some(note) => format!("{note}; {said}"),
                None => said,
            });
        }
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

    fn fill_secret(&self, epoch: u64, fill: &wire::FillSecret) -> Result<Browsed, NotActed> {
        self.browsed();
        let also = fill.also.clone();
        let fill = drive::SecretFill {
            target: Ref::new(&fill.target),
            origin: fill.origin.clone(),
            key: fill.key.clone(),
            value: zeroize::Zeroizing::new(fill.value.expose().to_owned()),
            multi_line: fill.multi_line,
            under_a_grant: fill.under_a_grant,
        };
        // It waits on the engine: made outside the lock, as an act is.
        let held = {
            let held = self.held.lock().unwrap_or_else(PoisonError::into_inner);
            held.as_ref()
                .map(|held| (Arc::clone(&held.browser), Arc::clone(&held.proxy)))
        };
        let Some((browser, proxy)) = held else {
            return Err(NotActed::Ended);
        };
        // Held to the field's site while its page lives, from before the value goes in (#5117).
        let hold = FillHolder::of(&proxy, &fill.origin)
            .map_err(NotActed::Refused)?
            .judged_by(self.sites.as_ref(), &also);
        browser
            .fill_secret(epoch, &fill, Arc::new(hold))
            .map(|answer| browsed(answer, MOST_SCREENSHOT_BYTES))
            .map_err(not_acted)
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
