// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The session host's browsers, as the binary starts them (#2012, spec #2008): the engine
//! (`purlis_browser_engine`) and its reach proxy ([`crate::sandbox::browser_reach`]) put
//! together behind the host's seam ([`purlis_host::Browsers`]), so the host crate links
//! neither. The window's binary and `purlis serve` both hand the host [`EngineBrowsers`].
//!
//! - **Which engine**: the pinned Chrome for Testing, downloaded into
//!   `<data>/engines/chrome-for-testing` on first use and checked against the pin this build
//!   ships before it runs.
//! - **Its keeper is this binary**, started again with [`KEEPER`]: the first thing each binary's
//!   `main` does is [`keep_if_asked`], so the keeper runs before anything else of the program.
//! - **This runs in the session host.** The window's host is the window's own binary, so with
//!   browsers it runs core code: the engine's download and the data home are read there, as a
//!   browser starts and never before (D-2012-4).
//! - **Its reach**, until a browser has a chat and a posture (#2013, #2018): nothing listed, and
//!   every site new to the project refused ([`NewSites::refusing`]), so it fails closed.
//! - **Its data folder** is the one the host gives it, under `<data>/browsers`, fresh: the host
//!   removes it once the browser has ended.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bytes::Bytes;
use purlis_browser_engine::drive::{Browser, Limits};
use purlis_browser_engine::install::{
    EngineSettings, Engines, Https, Manifest, NoSigner, Platform,
};
use purlis_browser_engine::launch::{self, Launch, PAGE_SIZE};
use purlis_browser_engine::process::{Engine, Keeper};
use purlis_host::protocol::session::Exit;
use purlis_host::{Browsers, FrameSink, Screen};

use crate::sandbox::browser_reach::{Lane, NewSites, ReachProxy};
use crate::sandbox::reach::Reach;

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
    hosting.with_browsers(EngineBrowsers::here())
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

    fn start(&self, data: &Path) -> Result<Arc<dyn Screen>, String> {
        let (home, program) = self.places()?;
        let platform = Platform::current()
            .ok_or_else(|| "no browser engine is made for this platform".to_owned())?;
        let engines = Engines::new(
            engines_in(&home),
            platform,
            Manifest::shipped(),
            Box::new(NoSigner),
        );
        let ready = engines
            .prepare(&EngineSettings::default(), &Https, chrono::Utc::now())
            .map_err(|why| why.to_string())?;
        let proxy = ReachProxy::start(Lane::of(Reach::of(Vec::new())), NewSites::refusing())
            .map_err(|why| format!("the browser's reach proxy did not start: {why}"))?;
        let launch = Launch::new(&ready, data, launch::ReachProxy(proxy.address()));
        let engine =
            Engine::start(&launch, &Self::keeper(program)).map_err(|why| why.to_string())?;
        let browser = Browser::new(engine, Limits::default()).map_err(|why| why.to_string())?;
        Ok(Arc::new(EngineScreen {
            process: browser.process(),
            held: Mutex::new(Some(Held { browser, proxy })),
        }))
    }
}

/// A running browser and its reach proxy, which lives exactly as long as it.
struct Held {
    browser: Browser,
    proxy: ReachProxy,
}

/// One browser on its engine, as the host drives it.
struct EngineScreen {
    process: Option<u32>,
    held: Mutex<Option<Held>>,
}

impl EngineScreen {
    fn with<T>(&self, act: impl FnOnce(&Browser) -> T) -> Option<T> {
        self.held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .map(|held| act(&held.browser))
    }
}

impl Screen for EngineScreen {
    fn show(&self, frames: FrameSink) {
        self.with(|browser| browser.show(Arc::new(move |image| frames(Bytes::from(image)))));
    }

    fn pace(&self, every: Duration) {
        self.with(|browser| browser.pace(every));
    }

    fn hide(&self) {
        self.with(Browser::hide);
    }

    fn size(&self) -> (u16, u16) {
        PAGE_SIZE
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
