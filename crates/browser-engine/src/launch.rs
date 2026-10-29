// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What the engine is started with, as data: its arguments, its environment and the
//! preferences written into its data folder before it starts.
//!
//! Each line here is a protection the spine names (ADR 0067's egress rules, given to the
//! browser by the session host), and each is a test in `the_engine_starts_with_its_protections_on`:
//!
//! - **DevTools is the inherited pipe** (`--remote-debugging-pipe`, descriptors 3 and 4).
//!   Nothing asks for a port, so none listens anywhere.
//! - **The engine's own sandbox stays on.** No argument here turns it off.
//! - **The reach proxy is the only route.** Every http and https request goes to it, loopback
//!   included (`<-loopback>` removes the bypass the engine adds by itself), and the engine
//!   resolves no name of its own, so a lookup never leaves beside the proxy.
//! - **No direct UDP**: QUIC is off and WebRTC sends no UDP the proxy does not carry.
//! - **Nothing secret is written by the engine**: password saving and autofill are off in the
//!   data folder's preferences, and crash reports are off.
//! - **No window**: the engine is headless; a browser is watched only as a browser view.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::install::Ready;

/// The address of the browser's reach proxy, its only route out. The proxy itself is the
/// session host's (`purlis_core::sandbox::browser_reach::ReachProxy`, #2011); the engine is
/// only told where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReachProxy(pub SocketAddr);

/// The size of a browser's page, in CSS pixels: what its frames are painted at (#2012).
pub const PAGE_SIZE: (u16, u16) = (1280, 800);

/// **What of a browser's data folder is not its state** (#2021): what the engine caches, the
/// home and the log purlis gives it, and the locks of a running engine. Whatever else is in the
/// folder (its cookies, storage and settings) is what a kept login keeps.
pub const NOT_KEPT: &[&str] = &[
    "home",
    "engine.log",
    "Cache",
    "Code Cache",
    "GPUCache",
    "DawnCache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "GrShaderCache",
    "GraphiteDawnCache",
    "ShaderCache",
    "component_crx_cache",
    "Crashpad",
    "SingletonLock",
    "SingletonSocket",
    "SingletonCookie",
    // A browser's downloads and its uploads' copies (#2020): its own while it runs, never
    // part of a kept login.
    "downloads",
    "uploads",
];

/// One engine's start.
#[derive(Debug, Clone)]
pub struct Launch {
    program: PathBuf,
    data_dir: PathBuf,
    reach_proxy: ReachProxy,
    /// Where the engine's standard output and error go, where not its data folder's
    /// `engine.log`.
    log: Option<PathBuf>,
}

/// The variables the engine is given from the host's environment, and no others: where home
/// and temporary files are, how to find programs, and the language and time zone pages see.
/// A token, a socket, a proxy setting or a purlis variable never reaches the engine.
pub const ENVIRONMENT_KEPT: &[&str] = &[
    "PATH", "TMPDIR", "LANG", "LC_ALL", "LC_CTYPE", "TZ", "USER", "LOGNAME",
];

impl Launch {
    /// The start of the engine `ready` proved, with `data_dir` as the browser's own data
    /// folder (one per browser, never shared by two running engines), its only route out the
    /// reach proxy at `reach_proxy`.
    pub fn new(ready: &Ready, data_dir: impl Into<PathBuf>, reach_proxy: ReachProxy) -> Self {
        Self {
            program: ready.program().to_owned(),
            data_dir: data_dir.into(),
            reach_proxy,
            log: None,
        }
    }

    /// A start of `program` that no install has checked: a test's stand-in engine.
    #[cfg(test)]
    pub(crate) fn for_a_test(
        program: &std::path::Path,
        data_dir: impl Into<PathBuf>,
        reach_proxy: ReachProxy,
    ) -> Self {
        Self {
            program: program.to_owned(),
            data_dir: data_dir.into(),
            reach_proxy,
            log: None,
        }
    }

    /// The same start, the engine's standard output and error written to `log` (#2745): a
    /// file outside the data folder outlives the folder's removal.
    pub fn logging_to(self, log: impl Into<PathBuf>) -> Self {
        Self {
            log: Some(log.into()),
            ..self
        }
    }

    /// Where the engine's standard output and error go: the file [`Launch::logging_to`] named,
    /// or `engine.log` in its data folder.
    pub fn log(&self) -> PathBuf {
        self.log
            .clone()
            .unwrap_or_else(|| self.data_dir.join("engine.log"))
    }

    /// The engine's executable.
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// The browser's data folder.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// The browser's own home, inside its data folder.
    fn home(&self) -> PathBuf {
        self.data_dir.join("home")
    }

    /// The engine's arguments, the program itself not included.
    pub fn arguments(&self) -> Vec<OsString> {
        let proxy = self.reach_proxy.0;
        let proxy_host = match proxy {
            SocketAddr::V4(v4) => v4.ip().to_string(),
            SocketAddr::V6(v6) => format!("[{}]", v6.ip()),
        };
        let mut data_dir = OsString::from("--user-data-dir=");
        data_dir.push(&self.data_dir);
        let mut args: Vec<OsString> = vec![
            "--remote-debugging-pipe".into(),
            data_dir,
            "--headless".into(),
            format!("--window-size={},{}", PAGE_SIZE.0, PAGE_SIZE.1).into(),
            format!("--proxy-server=http://{proxy}").into(),
            "--proxy-bypass-list=<-loopback>".into(),
            format!("--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE {proxy_host}").into(),
            "--disable-quic".into(),
            "--force-webrtc-ip-handling-policy=disable_non_proxied_udp".into(),
            "--disable-breakpad".into(),
            "--disable-crash-reporter".into(),
            // The person's keychain is never touched, and nothing is stored in the OS's.
            "--use-mock-keychain".into(),
            "--password-store=basic".into(),
            // The engine's own calls home (updates, sync, components, casting's discovery) are
            // off; what is left would meet the reach proxy anyway.
            "--disable-background-networking".into(),
            "--disable-component-update".into(),
            "--disable-sync".into(),
            "--disable-extensions".into(),
            "--disable-default-apps".into(),
            "--disable-features=MediaRouter,DialMediaRouteProvider,Translate,AutofillServerCommunication".into(),
            "--no-first-run".into(),
            "--no-default-browser-check".into(),
            "--noerrdialogs".into(),
        ];
        args.push("about:blank".into());
        args
    }

    /// The environment the engine runs with: the host's, kept to [`ENVIRONMENT_KEPT`].
    pub fn environment(
        &self,
        host: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Vec<(OsString, OsString)> {
        let mut env: Vec<(OsString, OsString)> = host
            .into_iter()
            .filter(|(name, _)| ENVIRONMENT_KEPT.iter().any(|kept| name == kept))
            .collect();
        // A home of the browser's own, inside its data folder: on Linux the engine keeps its
        // certificate database under $HOME whatever its data folder, and the person's home is
        // not the browser's to write.
        let home = self.home();
        for (name, under) in [
            ("XDG_CONFIG_HOME", ".config"),
            ("XDG_CACHE_HOME", ".cache"),
            ("XDG_DATA_HOME", ".local/share"),
        ] {
            env.push((name.into(), home.join(under).into_os_string()));
        }
        env.push(("HOME".into(), home.into_os_string()));
        // The person's desktop bus is none of the engine's business, and on a machine with no
        // session bus the engine waits for one at every start.
        env.push(("DBUS_SESSION_BUS_ADDRESS".into(), "disabled:".into()));
        env
    }

    /// Makes the data folder (this user's alone) and writes the preferences that keep password
    /// saving and autofill off, keeping every other preference a kept folder already has.
    pub fn prepare_data_dir(&self) -> io::Result<()> {
        private_dir(&self.data_dir)?;
        private_dir(&self.home())?;
        let default = self.data_dir.join("Default");
        private_dir(&default)?;
        let path = default.join("Preferences");
        let mut prefs = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(Value::Object(prefs)) => prefs,
                // A file the engine could not read either: it starts again from the protections.
                _ => Map::new(),
            },
            Err(why) if why.kind() == io::ErrorKind::NotFound => Map::new(),
            Err(why) => return Err(why),
        };
        merge(
            &mut prefs,
            json!({
                "credentials_enable_service": false,
                "profile": {
                    "password_manager_enabled": false,
                    "password_manager_leak_detection": false,
                },
                "autofill": {
                    "enabled": false,
                    "profile_enabled": false,
                    "credit_card_enabled": false,
                },
            }),
        );
        let bytes = serde_json::to_vec(&Value::Object(prefs)).expect("a JSON value serialises");
        fs::write(path, bytes)
    }
}

/// Sets every leaf of `protections` in `prefs`, keeping whatever else `prefs` holds.
fn merge(prefs: &mut Map<String, Value>, protections: Value) {
    let Value::Object(protections) = protections else {
        return;
    };
    for (key, value) in protections {
        match (prefs.get_mut(&key), value) {
            (Some(Value::Object(inner)), Value::Object(more)) => merge(inner, Value::Object(more)),
            (_, value) => {
                prefs.insert(key, value);
            }
        }
    }
}

fn private_dir(dir: &Path) -> io::Result<()> {
    // Made 0700, or, when there, refused unless a directory (not a link) of this user's, and
    // then set to 0700.
    #[cfg(unix)]
    {
        purlis_same_user::private_directory(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}
