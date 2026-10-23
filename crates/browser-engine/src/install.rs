// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Which engine runs, and the proof that it is the one meant: a pinned Chrome for Testing,
//! downloaded on first use, kept current apart from purlis's own releases, and checked before
//! it ever runs.
//!
//! **What is trusted.** A [`Manifest`] names a version and, per platform, the archive's address
//! and its SHA-256. Two manifests can be trusted:
//!
//! - the one this build ships ([`Manifest::shipped`], `engine-pin.json` beside this crate),
//!   trusted because it is inside the build the updater verified (ADR 0042);
//! - a newer one received later, trusted only when a [`ManifestSignature`] verifies its
//!   signature: in a build, [`crate::sigstore::Sigstore`], which takes only a Sigstore bundle
//!   made by the manifest workflow on `main` of purlis/purlis (#2073). It is taken only when it
//!   supersedes the one in use ([`Manifest::supersedes`]): a later version, or the same version
//!   issued later, never an earlier one (no step back); and only when it was issued after the
//!   one in use, so the newest manifest a host has read decides.
//!
//! Of the shipped manifest and a kept update, the one that supersedes the other is used. An
//! archive runs only after its digest matched the trusted manifest's, before a byte of it was
//! unpacked; an archive that does not match is deleted and nothing of it is kept. Chrome for
//! Testing publishes no signature or digest of its own, so the pin is the only proof there is.
//!
//! **Fails closed.** A manifest whose bundle does not verify is refused, and the engine in use
//! stays. A build that never receives one runs its own pinned engine. [`NoSigner`] verifies
//! nothing, for what trusts no update at all.
//!
//! **Where updates are published**: [`PUBLISHED_MANIFEST`] and [`PUBLISHED_BUNDLE`], release
//! assets the manifest workflow replaces each day; the session host reads them in the
//! background (`purlis_core::browsers`).
//!
//! **Age.** An engine is behind from the moment the newest trusted manifest was issued: a
//! manifest says *as of this time, this version is current*, so until a newer one is verified,
//! the engine in use ages. At [`EngineSettings::warn_after`] (7 days) it runs with a warning, at
//! [`EngineSettings::refuse_after`] (21 days) it is refused with "Update now", unless the person
//! allowed an outdated engine in Settings ([`EngineSettings::run_outdated`]).
//!
//! **The machine's own Chrome** is the person's choice ([`EngineChoice::Machine`]): purlis did
//! not install it, so there is no manifest to check it against and its age is the vendor's to
//! keep; it runs with the same protections ([`crate::launch`]).

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The largest archive downloaded: 1 GiB, several times Chrome for Testing's.
pub const MAX_ARCHIVE_BYTES: u64 = 1 << 30;

/// Where the newest signed engine manifest is published: an asset of the `engine-manifest`
/// prerelease of purlis/purlis, which `.github/workflows/engine-manifest.yml` replaces.
pub const PUBLISHED_MANIFEST: &str =
    "https://github.com/purlis/purlis/releases/download/engine-manifest/engine-manifest.json";

/// The Sigstore bundle over [`PUBLISHED_MANIFEST`], published beside it.
pub const PUBLISHED_BUNDLE: &str = concat!(
    "https://github.com/purlis/purlis/releases/download/engine-manifest/",
    "engine-manifest.json.sigstore.json"
);

/// The largest manifest or bundle read: either is a few kilobytes.
pub const MAX_MANIFEST_BYTES: u64 = 1 << 20;

/// The manifest this build ships.
const SHIPPED: &str = include_str!("../engine-pin.json");

/// A platform Chrome for Testing is built for, by the name its manifests use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS on Apple silicon.
    MacArm64,
    /// macOS on Intel.
    MacX64,
    /// Linux on x86-64.
    Linux64,
    /// Linux on 64-bit Arm.
    LinuxArm64,
}

impl Platform {
    /// This machine's platform, when Chrome for Testing is built for it.
    pub fn current() -> Option<Self> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => Some(Self::MacArm64),
            ("macos", "x86_64") => Some(Self::MacX64),
            ("linux", "x86_64") => Some(Self::Linux64),
            ("linux", "aarch64") => Some(Self::LinuxArm64),
            _ => None,
        }
    }

    /// The name manifests use for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::MacArm64 => "mac-arm64",
            Self::MacX64 => "mac-x64",
            Self::Linux64 => "linux64",
            Self::LinuxArm64 => "linux-arm64",
        }
    }

    /// Where the engine's executable is inside the unpacked archive.
    pub fn executable(self) -> PathBuf {
        let top = format!("chrome-{}", self.name());
        match self {
            Self::MacArm64 | Self::MacX64 => Path::new(&top)
                .join("Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"),
            Self::Linux64 | Self::LinuxArm64 => Path::new(&top).join("chrome"),
        }
    }
}

/// One platform's archive in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Download {
    /// The platform's name, as [`Platform::name`].
    pub platform: String,
    /// Where the archive is, always `https://`.
    pub url: String,
    /// The archive's SHA-256, 64 lower-case hexadecimal digits.
    pub sha256: String,
}

/// A version of the engine, as of a time, with each platform's archive pinned by digest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Chrome for Testing's version, such as `155.0.8059.79`.
    pub version: String,
    /// When this version was last known current.
    pub issued: DateTime<Utc>,
    /// Each platform's archive.
    pub downloads: Vec<Download>,
}

/// Why a manifest's text is not a manifest.
#[derive(Debug, thiserror::Error)]
#[error("the engine manifest cannot be read: {0}")]
pub struct BadManifest(String);

impl Manifest {
    /// Reads a manifest, refusing one with a download that is not https or a digest that is not
    /// a SHA-256.
    pub fn parse(text: &[u8]) -> Result<Self, BadManifest> {
        let manifest: Self =
            serde_json::from_slice(text).map_err(|why| BadManifest(why.to_string()))?;
        if manifest.version.is_empty()
            || !manifest
                .version
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.')
        {
            return Err(BadManifest(format!(
                "`{}` is not a version",
                manifest.version
            )));
        }
        for download in &manifest.downloads {
            if !download.url.starts_with("https://") {
                return Err(BadManifest(format!(
                    "the {} download is not an https address",
                    download.platform
                )));
            }
            if download.sha256.len() != 64
                || !download
                    .sha256
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
            {
                return Err(BadManifest(format!(
                    "the {} download carries no SHA-256",
                    download.platform
                )));
            }
        }
        Ok(manifest)
    }

    /// The manifest this build ships.
    pub fn shipped() -> Self {
        Self::parse(SHIPPED.as_bytes()).expect("the shipped engine manifest parses")
    }

    /// Whether this manifest is to be used in place of `other`: it names a later version, or
    /// the same version issued later, which says that version is still current. Never one that
    /// names an earlier version, however late it was issued (no step back).
    pub fn supersedes(&self, other: &Manifest) -> bool {
        (version_parts(&self.version), self.issued) > (version_parts(&other.version), other.issued)
    }

    fn download(&self, platform: Platform) -> Option<&Download> {
        self.downloads
            .iter()
            .find(|d| d.platform == platform.name())
    }
}

/// What verifies the signature of a manifest that did not ship in this build.
pub trait ManifestSignature {
    /// `Ok` only when `signature` is a valid signature of exactly `manifest`.
    fn verify(&self, manifest: &[u8], signature: &str) -> Result<(), String>;
}

/// Verifies nothing: every manifest it is shown is refused, so what uses it runs the engine it
/// pins, and ages towards the refusal.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoSigner;

impl ManifestSignature for NoSigner {
    fn verify(&self, _: &[u8], _: &str) -> Result<(), String> {
        Err("no engine update is trusted here".to_owned())
    }
}

/// How an archive is downloaded.
pub trait Fetch {
    /// The bytes at `url`.
    fn fetch(&self, url: &str) -> io::Result<Box<dyn Read + '_>>;

    /// The bytes at `url`, and how many there are where the answer says (#2207): what a
    /// download's progress is a share of. Where it is not known, none.
    fn fetch_sized(&self, url: &str) -> io::Result<(Box<dyn Read + '_>, Option<u64>)> {
        Ok((self.fetch(url)?, None))
    }

    /// Whether the install this fetches for is still wanted (#2207): asked once more after the
    /// archive is downloaded and unpacked, just before the engine is put in place, so a download
    /// cancelled while it unpacks puts nothing there. An error stops the install, refused as
    /// [`Refused::Download`] in its words.
    fn still_wanted(&self) -> io::Result<()> {
        Ok(())
    }
}

/// What a download heard, for each request it made: its address, the status that came back
/// (none when nothing did), whether the answer was read to its end, and how long that took.
/// A redirect is a request of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hop<'a> {
    /// The address asked.
    pub url: &'a str,
    /// The status that came back, if one did.
    pub status: Option<u16>,
    /// Whether the answer came back whole: a redirect or an error status read, or a body read
    /// to its end.
    pub answered: bool,
    /// From the request to its last byte read.
    pub took: Duration,
}

/// Told of each request a download makes ([`Https::listening`]).
pub type Listener = Arc<dyn Fn(Hop<'_>) + Send + Sync>;

/// The most redirects one download follows.
pub const MAX_REDIRECTS: usize = 5;

/// Downloads over https, and only https, redirects included, each request told to a
/// [`Listener`] when there is one, and the whole of each request, its body included, bounded
/// by a total time ([`Https::within`]), so a server that stops answering is given up on.
#[derive(Clone)]
pub struct Https {
    total: Duration,
    listener: Option<Listener>,
    /// Plain http too: only this crate's own tests, against a loopback server.
    #[cfg(test)]
    plain: bool,
}

impl Https {
    /// The longest an archive's download may take: an hour.
    pub const ARCHIVE: Duration = Duration::from_secs(60 * 60);

    /// Downloads each taking at most `total`, from the first request to the body's last
    /// byte, redirects included.
    pub fn within(total: Duration) -> Self {
        Self {
            total,
            listener: None,
            #[cfg(test)]
            plain: false,
        }
    }

    fn https_only(&self) -> bool {
        #[cfg(test)]
        if self.plain {
            return false;
        }
        true
    }

    /// The same downloads, each request told to `listener`.
    pub fn listening(self, listener: Listener) -> Self {
        Self {
            listener: Some(listener),
            ..self
        }
    }

    fn hear(&self, hop: Hop<'_>) {
        if let Some(listener) = &self.listener {
            listener(hop);
        }
    }
}

impl Default for Https {
    /// [`Https::ARCHIVE`], told to no one.
    fn default() -> Self {
        Self::within(Self::ARCHIVE)
    }
}

impl Fetch for Https {
    fn fetch(&self, url: &str) -> io::Result<Box<dyn Read + '_>> {
        self.fetch_sized(url).map(|(body, _)| body)
    }

    fn fetch_sized(&self, url: &str) -> io::Result<(Box<dyn Read + '_>, Option<u64>)> {
        let https_only = self.https_only();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .https_only(https_only)
            .timeout_connect(Some(Duration::from_secs(30)))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        // One deadline for the whole download: each request, a redirect's included, gets only
        // what is left of it.
        let deadline = Instant::now() + self.total;
        let mut url = url.to_owned();
        for _ in 0..=MAX_REDIRECTS {
            let started = Instant::now();
            let left = deadline.saturating_duration_since(started);
            let response = match agent
                .get(&url)
                .config()
                .timeout_global(Some(left))
                .build()
                .call()
            {
                Ok(response) => response,
                Err(why) => {
                    self.hear(Hop {
                        url: &url,
                        status: None,
                        answered: false,
                        took: started.elapsed(),
                    });
                    return Err(io::Error::other(why));
                }
            };
            let status = response.status().as_u16();
            if response.status().is_redirection() {
                self.hear(Hop {
                    url: &url,
                    status: Some(status),
                    answered: true,
                    took: started.elapsed(),
                });
                url = response
                    .headers()
                    .get("location")
                    .and_then(|to| to.to_str().ok())
                    .filter(|to| to.starts_with("https://") || !https_only)
                    .ok_or_else(|| {
                        io::Error::other(format!("{url} redirects to no https address"))
                    })?
                    .to_owned();
                continue;
            }
            if !response.status().is_success() {
                self.hear(Hop {
                    url: &url,
                    status: Some(status),
                    answered: true,
                    took: started.elapsed(),
                });
                return Err(io::Error::other(format!("{url} answered {status}")));
            }
            let body = response.into_body();
            let length = body.content_length();
            let body = body.into_with_config().limit(MAX_ARCHIVE_BYTES).reader();
            return Ok((
                Box::new(Heard {
                    body,
                    heard: Some((self.clone(), url, status, started)),
                    whole: false,
                }),
                length,
            ));
        }
        Err(io::Error::other(format!(
            "more than {MAX_REDIRECTS} redirects from {url}"
        )))
    }
}

/// A body, told to the download's listener once it is dropped: answered only when it was read
/// to its end.
struct Heard<R> {
    body: R,
    heard: Option<(Https, String, u16, Instant)>,
    whole: bool,
}

impl<R: Read> Read for Heard<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.body.read(buf)?;
        if n == 0 && !buf.is_empty() {
            self.whole = true;
        }
        Ok(n)
    }
}

impl<R> Drop for Heard<R> {
    fn drop(&mut self) {
        if let Some((https, url, status, started)) = self.heard.take() {
            https.hear(Hop {
                url: &url,
                status: Some(status),
                answered: self.whole,
                took: started.elapsed(),
            });
        }
    }
}

/// Which engine a browser runs on: the person's choice in Settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum EngineChoice {
    /// The pinned Chrome for Testing, which purlis installs and keeps current.
    #[default]
    Pinned,
    /// The machine's own Chrome, at this path.
    Machine(PathBuf),
}

/// The engine's settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineSettings {
    /// Which engine.
    pub engine: EngineChoice,
    /// Run the pinned engine even past [`EngineSettings::refuse_after`].
    pub run_outdated: bool,
    /// Behind by this much, the engine runs with a warning.
    pub warn_after: Duration,
    /// Behind by this much, the engine is refused unless `run_outdated`.
    pub refuse_after: Duration,
}

const DAY: u64 = 86_400;

impl Default for EngineSettings {
    fn default() -> Self {
        Self {
            engine: EngineChoice::Pinned,
            run_outdated: false,
            warn_after: Duration::from_secs(7 * DAY),
            refuse_after: Duration::from_secs(21 * DAY),
        }
    }
}

/// How far behind the engine is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Age {
    /// Within the warning's limit.
    Current,
    /// Past it, by this many whole days.
    Behind {
        /// Whole days since the newest trusted manifest was issued.
        days: u64,
    },
    /// The machine's own Chrome: its age is its vendor's to keep.
    Unknown,
}

impl Age {
    /// The words of the warning to show, when there is one.
    pub fn warning(self) -> Option<String> {
        match self {
            Self::Behind { days } => Some(format!(
                "The browser engine is {days} days behind. Update now to bring it up to date."
            )),
            Self::Current | Self::Unknown => None,
        }
    }
}

/// An engine proven to be the one meant, ready to start. Only [`Engines::prepare`] and
/// [`Engines::update`] make one, and a [`crate::launch::Launch`] is made only from one, so no
/// engine starts that was not checked first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    program: PathBuf,
    version: Option<String>,
    age: Age,
}

impl Ready {
    /// Its executable.
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Its version, for the pinned engine.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// How far behind it is.
    pub fn age(&self) -> Age {
        self.age
    }
}

/// Why no engine is ready.
#[derive(Debug, thiserror::Error)]
pub enum Refused {
    /// The trusted manifest pins no archive for this platform.
    #[error("no browser engine is pinned for {platform}")]
    NotPinned {
        /// The platform's name.
        platform: &'static str,
    },
    /// The archive could not be downloaded.
    #[error("the browser engine could not be downloaded: {0}")]
    Download(#[source] io::Error),
    /// The archive downloaded is not the one pinned.
    #[error(
        "the browser engine downloaded is not the one this build pins (SHA-256 {got}, \
         pinned {expected}), so it was deleted"
    )]
    Digest {
        /// The pinned digest.
        expected: String,
        /// The downloaded archive's.
        got: String,
    },
    /// The archive could not be unpacked, or has no engine where one is expected.
    #[error("the browser engine's archive could not be unpacked: {0}")]
    Archive(String),
    /// A manifest's signature did not verify.
    #[error("the engine update was refused because its signature does not verify: {0}")]
    Signature(String),
    /// A signed manifest that does not parse.
    #[error(transparent)]
    Manifest(#[from] BadManifest),
    /// A signed manifest that does not supersede the one in use ([`Manifest::supersedes`]), or
    /// was issued no later than it.
    #[error("the engine update names {version} and does not supersede the engine manifest in use")]
    Older {
        /// The update's version.
        version: String,
    },
    /// The engine is behind past the refusal's limit.
    #[error(
        "The browser engine is {days} days behind, so it was not started. Update now, or \
         allow an outdated engine in Settings."
    )]
    Outdated {
        /// Whole days behind.
        days: u64,
    },
    /// The machine's own Chrome is not where Settings says.
    #[error("the Chrome chosen in Settings is not at {}", .0.display())]
    MachineChrome(PathBuf),
    /// The warning and refusal ages in Settings do not make sense together.
    #[error("{0}")]
    Settings(String),
    /// The engines' folder could not be read or written.
    #[error("the browser engines' folder could not be used: {0}")]
    Store(#[source] io::Error),
}

/// The engines installed on this machine, in one folder, and what is trusted to fill it.
pub struct Engines {
    root: PathBuf,
    platform: Platform,
    shipped: Manifest,
    signer: Box<dyn ManifestSignature>,
}

/// Held only while an engine is moved into place or an update is kept, never across a
/// download, so a stalled download never holds up another browser's start.
static SWAPPING: Mutex<()> = Mutex::new(());

const UPDATE: &str = "update.json";
const UPDATE_SIGNATURE: &str = "update.json.sigstore.json";
const INSTALLED: &str = "installed.json";

impl Engines {
    /// The engines under `root` for `platform`, trusting `shipped` and what `signer` verifies.
    pub fn new(
        root: impl Into<PathBuf>,
        platform: Platform,
        shipped: Manifest,
        signer: Box<dyn ManifestSignature>,
    ) -> Self {
        Self {
            root: root.into(),
            platform,
            shipped,
            signer,
        }
    }

    /// The engine to start under `settings` at `now`, downloading the pinned one first when it
    /// is not installed yet.
    pub fn prepare(
        &self,
        settings: &EngineSettings,
        fetch: &dyn Fetch,
        now: DateTime<Utc>,
    ) -> Result<Ready, Refused> {
        if let EngineChoice::Machine(path) = &settings.engine {
            if !path.is_file() {
                return Err(Refused::MachineChrome(path.clone()));
            }
            return Ok(Ready {
                program: path.clone(),
                version: None,
                age: Age::Unknown,
            });
        }
        if settings.refuse_after <= settings.warn_after {
            return Err(Refused::Settings(
                "The browser engine's refusal age in Settings must be later than its warning age."
                    .to_owned(),
            ));
        }
        let manifest = self.trusted();
        let age = age(&manifest, settings, now);
        if let Age::Behind { days } = age
            && !settings.run_outdated
            && behind(&manifest, now) >= settings.refuse_after
        {
            return Err(Refused::Outdated { days });
        }
        let program = self.install(&manifest, fetch)?;
        Ok(Ready {
            program,
            version: Some(manifest.version.clone()),
            age,
        })
    }

    /// Takes a manifest received with its signature ("Update now", or the background check):
    /// verifies the signature, refuses one that does not supersede the manifest in use or was
    /// issued no later than it, installs its engine, and keeps it as the manifest in use.
    pub fn update(
        &self,
        manifest: &[u8],
        signature: &str,
        settings: &EngineSettings,
        fetch: &dyn Fetch,
        now: DateTime<Utc>,
    ) -> Result<Ready, Refused> {
        self.signer
            .verify(manifest, signature)
            .map_err(Refused::Signature)?;
        let update = Manifest::parse(manifest)?;
        if !self.takes(&update) {
            return Err(Refused::Older {
                version: update.version,
            });
        }
        let program = self.install(&update, fetch)?;
        let _swapping = SWAPPING.lock().unwrap_or_else(PoisonError::into_inner);
        // Asked again: another update may have been kept while this one downloaded.
        if !self.takes(&update) {
            return Err(Refused::Older {
                version: update.version,
            });
        }
        private_dir(&self.root).map_err(Refused::Store)?;
        write_replacing(&self.root.join(UPDATE_SIGNATURE), signature.as_bytes())?;
        write_replacing(&self.root.join(UPDATE), manifest)?;
        let age = age(&update, settings, now);
        Ok(Ready {
            program,
            version: Some(update.version),
            age,
        })
    }

    /// Reads the newest published manifest and its bundle ([`PUBLISHED_MANIFEST`],
    /// [`PUBLISHED_BUNDLE`]) through `fetch` and takes it as [`Engines::update`] does: the
    /// session host's background check, and "Update now". Either file larger than
    /// [`MAX_MANIFEST_BYTES`] is refused unread.
    pub fn update_from_published(
        &self,
        settings: &EngineSettings,
        fetch: &dyn Fetch,
        now: DateTime<Utc>,
    ) -> Result<Ready, Refused> {
        let (manifest, bundle) = Self::published(fetch)?;
        self.update(&manifest, &bundle, settings, fetch, now)
    }

    /// The newest published manifest and its bundle, read through `fetch`, unverified: what
    /// [`Engines::update`] is given. Either file larger than [`MAX_MANIFEST_BYTES`] is refused
    /// unread.
    pub fn published(fetch: &dyn Fetch) -> Result<(Vec<u8>, String), Refused> {
        let manifest = read_small(fetch, PUBLISHED_MANIFEST)?;
        let bundle = read_small(fetch, PUBLISHED_BUNDLE)?;
        let bundle = String::from_utf8(bundle)
            .map_err(|_| Refused::Signature("the Sigstore bundle is not text".to_owned()))?;
        Ok((manifest, bundle))
    }

    /// Whether an engine is installed here: the pinned engine has been used on this machine.
    pub fn in_use(&self) -> bool {
        fs::read_dir(&self.root).is_ok_and(|entries| {
            entries
                .flatten()
                .any(|entry| installed_digest(&entry.path()).is_some())
        })
    }

    /// Whether `update` is to be taken over the manifest in use: it supersedes it, and was
    /// issued after it.
    fn takes(&self, update: &Manifest) -> bool {
        let current = self.trusted();
        update.supersedes(&current) && update.issued > current.issued
    }

    /// The manifest in use: a stored update that still verifies and supersedes the one this
    /// build ships ([`Manifest::supersedes`]), or else the shipped one.
    fn trusted(&self) -> Manifest {
        let stored = fs::read(self.root.join(UPDATE)).ok().and_then(|text| {
            let signature = fs::read_to_string(self.root.join(UPDATE_SIGNATURE)).ok()?;
            self.signer.verify(&text, &signature).ok()?;
            Manifest::parse(&text).ok()
        });
        match stored {
            Some(update) if update.supersedes(&self.shipped) => update,
            _ => self.shipped.clone(),
        }
    }

    /// The pinned engine's executable, downloaded, checked and unpacked when not installed.
    fn install(&self, manifest: &Manifest, fetch: &dyn Fetch) -> Result<PathBuf, Refused> {
        let platform = self.platform;
        let download = manifest.download(platform).ok_or(Refused::NotPinned {
            platform: platform.name(),
        })?;
        let home = self
            .root
            .join(format!("{}-{}", manifest.version, platform.name()));
        let program = home.join(platform.executable());
        if installed_digest(&home).as_deref() == Some(download.sha256.as_str()) && program.is_file()
        {
            return Ok(program);
        }
        private_dir(&self.root).map_err(Refused::Store)?;
        let archive = Scratch(self.root.join(scratch_name("download")));
        let got = save_hashing(fetch, &download.url, &archive.0)?;
        if got != download.sha256 {
            return Err(Refused::Digest {
                expected: download.sha256.clone(),
                got,
            });
        }
        let unpacked = Scratch(self.root.join(scratch_name("unpack")));
        let file = File::open(&archive.0).map_err(Refused::Store)?;
        zip::ZipArchive::new(file)
            .and_then(|mut zip| zip.extract(&unpacked.0))
            .map_err(|why| Refused::Archive(why.to_string()))?;
        links_stay_inside(&unpacked.0)?;
        if fs::symlink_metadata(unpacked.0.join(INSTALLED)).is_ok() {
            return Err(Refused::Archive(format!(
                "it holds {INSTALLED}, a name purlis keeps for itself"
            )));
        }
        if !unpacked.0.join(platform.executable()).is_file() {
            return Err(Refused::Archive(format!(
                "it has no {}",
                platform.executable().display()
            )));
        }
        // A new file, never one already there, so no link of the archive's is followed.
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(unpacked.0.join(INSTALLED))
            .and_then(|mut record| {
                record.write_all(
                    serde_json::json!({ "sha256": download.sha256 })
                        .to_string()
                        .as_bytes(),
                )
            })
            .map_err(Refused::Store)?;
        // Cancelled while it unpacked: nothing is put in place, and its scratch goes.
        fetch.still_wanted().map_err(Refused::Download)?;
        // The one step that touches the engines in place, one at a time in this process; every
        // download and unpack before it is into a scratch name of its own. An engine another
        // install finished meanwhile is kept, never replaced under a browser running it.
        let _swapping = SWAPPING.lock().unwrap_or_else(PoisonError::into_inner);
        if installed_digest(&home).as_deref() == Some(download.sha256.as_str()) && program.is_file()
        {
            return Ok(program);
        }
        match fs::remove_dir_all(&home) {
            Ok(()) => {}
            Err(why) if why.kind() == io::ErrorKind::NotFound => {}
            Err(why) => return Err(Refused::Store(why)),
        }
        fs::rename(&unpacked.0, &home).map_err(Refused::Store)?;
        Ok(program)
    }
}

/// Dotted version `v`'s parts as numbers, to compare part by part.
fn version_parts(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Refuses an unpacked tree holding a link that is absolute or leads out of the tree.
fn links_stay_inside(root: &Path) -> Result<(), Refused> {
    use std::path::Component;
    let mut folders = vec![PathBuf::new()];
    while let Some(folder) = folders.pop() {
        let entries = fs::read_dir(root.join(&folder)).map_err(Refused::Store)?;
        for entry in entries {
            let entry = entry.map_err(Refused::Store)?;
            let inside = folder.join(entry.file_name());
            let kind = entry.file_type().map_err(Refused::Store)?;
            if kind.is_dir() {
                folders.push(inside);
            } else if kind.is_symlink() {
                let target = fs::read_link(root.join(&inside)).map_err(Refused::Store)?;
                let mut at: Vec<_> = folder.components().collect();
                let mut escapes = false;
                for part in target.components() {
                    match part {
                        Component::Normal(_) => at.push(part),
                        Component::CurDir => {}
                        Component::ParentDir => escapes |= at.pop().is_none(),
                        Component::RootDir | Component::Prefix(_) => escapes = true,
                    }
                }
                if escapes {
                    return Err(Refused::Archive(format!(
                        "it holds a link out of its folder, {}",
                        inside.display()
                    )));
                }
            }
        }
    }
    Ok(())
}

fn private_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        purlis_same_user::private_directory(dir)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(dir)
    }
}

/// How long since `manifest` was issued; nothing when the clock reads earlier.
fn behind(manifest: &Manifest, now: DateTime<Utc>) -> Duration {
    (now - manifest.issued).to_std().unwrap_or_default()
}

fn age(manifest: &Manifest, settings: &EngineSettings, now: DateTime<Utc>) -> Age {
    let behind = behind(manifest, now);
    if behind < settings.warn_after {
        Age::Current
    } else {
        Age::Behind {
            days: behind.as_secs() / DAY,
        }
    }
}

fn installed_digest(home: &Path) -> Option<String> {
    let text = fs::read(home.join(INSTALLED)).ok()?;
    let record: serde_json::Value = serde_json::from_slice(&text).ok()?;
    record.get("sha256")?.as_str().map(str::to_owned)
}

/// The bytes at `url`, refused when there are more than [`MAX_MANIFEST_BYTES`].
fn read_small(fetch: &dyn Fetch, url: &str) -> Result<Vec<u8>, Refused> {
    let mut bytes = Vec::new();
    fetch
        .fetch(url)
        .and_then(|reader| reader.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut bytes))
        .map_err(Refused::Download)?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(Refused::Download(io::Error::other(format!(
            "{url} is larger than any engine manifest"
        ))));
    }
    Ok(bytes)
}

/// Downloads `url` into `to` and returns its SHA-256, refusing more than [`MAX_ARCHIVE_BYTES`].
fn save_hashing(fetch: &dyn Fetch, url: &str, to: &Path) -> Result<String, Refused> {
    let reader = fetch.fetch(url).map_err(Refused::Download)?;
    let mut reader = reader.take(MAX_ARCHIVE_BYTES + 1);
    let mut out = File::create(to).map_err(Refused::Store)?;
    let mut hash = Sha256::new();
    let mut total = 0_u64;
    let mut chunk = vec![0_u8; 256 * 1024];
    loop {
        let n = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(why) if why.kind() == io::ErrorKind::Interrupted => continue,
            Err(why) => return Err(Refused::Download(why)),
        };
        total += n as u64;
        if total > MAX_ARCHIVE_BYTES {
            return Err(Refused::Download(io::Error::other(
                "the archive is larger than any engine",
            )));
        }
        hash.update(&chunk[..n]);
        out.write_all(&chunk[..n]).map_err(Refused::Store)?;
    }
    out.sync_all().map_err(Refused::Store)?;
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn write_replacing(path: &Path, bytes: &[u8]) -> Result<(), Refused> {
    let scratch = path.with_file_name(scratch_name("write"));
    fs::write(&scratch, bytes).map_err(Refused::Store)?;
    fs::rename(&scratch, path).map_err(Refused::Store)
}

/// A name for a scratch file or folder no other call in this process uses.
fn scratch_name(what: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        ".{what}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// A scratch file or folder, removed when dropped unless it was moved away first.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        if self.0.is_dir() {
            let _ = fs::remove_dir_all(&self.0);
        } else {
            let _ = fs::remove_file(&self.0);
        }
    }
}

#[cfg(test)]
mod tests;
