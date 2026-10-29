// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A runner installs the harness version its profile pins** (#3446, spec #2041's *Profiles on
//! runners*).
//!
//! A runner image ships no harness (`docs/runner-image.md`, "No harness"). A desktop has its
//! runner keep each of its profiles with the version the desktop pinned ([`super::onrunner`]),
//! and a chat that starts on such a profile there runs exactly that version, installed on the
//! runner ([`for_a_start`]):
//!
//! - **From the harness's own release**, as its vendor publishes it: Claude Code's release
//!   bucket, Codex's and opencode's GitHub releases ([`release_of`]). purlis never ships,
//!   patches or wraps a harness binary (ADR 0073, ADR 0080); the vendor's own build runs.
//! - **Checked against the digest its vendor publishes** for that version and platform before a
//!   byte of it is unpacked or kept: Claude Code's release manifest, and the SHA-256 GitHub
//!   publishes for each release asset. A version whose digest cannot be read, or whose download
//!   does not match it, is not installed, and the chat says why.
//! - **Into the runner's data home, on its own volume** (`<data>/harnesses/<kind>/<version>/`),
//!   never under `/tmp`, which a pool mounts `noexec`. Each version is unpacked beside its final
//!   place and moved there whole, so a version is there entirely or not at all.
//! - **Only when a chat starts on it**: nothing is downloaded for a profile no runner chat starts
//!   on, and a version installed once is run again without a download.
//!
//! Only a profile a runner keeps for its desktop (`kept_for`) with a pin is installed so. Every
//! other profile, and every profile on the desktop, runs the harness on `PATH`, as before.

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::MachineProfile;

/// The harnesses' folder under the data home.
pub const DIR: &str = "harnesses";

/// The largest download read: 1 GiB, several times any harness's.
pub const MOST_DOWNLOAD_BYTES: u64 = 1 << 30;

/// The largest release description read: a manifest or a release's list of assets.
const MOST_DESCRIPTION_BYTES: u64 = 4 << 20;

/// Claude Code's release bucket, as its own installer reads it
/// (<https://code.claude.com/docs/en/setup>): `<bucket>/<version>/manifest.json` holds each
/// platform's SHA-256, and `<bucket>/<version>/<platform>/claude` the program.
const CLAUDE_RELEASES: &str = "https://storage.googleapis.com/\
                               claude-code-dist-86c565f3-f756-42ad-8dfa-d59b1c096819/\
                               claude-code-releases";

/// **What fetches a release**: its description, and its download. Over HTTPS in a build
/// ([`OverHttps`]); a test hands its own.
pub trait Fetch {
    /// Writes what `url` answers into `into`, at most `most` bytes, or why not.
    fn fetch(&self, url: &str, most: u64, into: &mut dyn Write) -> Result<(), String>;
}

/// **Fetches over HTTPS only**, with the operating system's trust store, following a redirect
/// only to another HTTPS address (a GitHub release asset is served from its storage host).
/// Nothing is sent but the address: no token, no cookie.
pub struct OverHttps;

impl Fetch for OverHttps {
    fn fetch(&self, url: &str, most: u64, into: &mut dyn Write) -> Result<(), String> {
        if !url.starts_with("https://") {
            return Err(format!("{url} is not an HTTPS address"));
        }
        let agent = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(5)
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_global(Some(Duration::from_secs(30 * 60)))
            .user_agent(concat!("purlis/", env!("CARGO_PKG_VERSION")))
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .new_agent();
        let answer = agent.get(url).call().map_err(|why| why.to_string())?;
        let mut body = answer.into_body().into_reader().take(most + 1);
        let copied = io::copy(&mut body, into).map_err(|why| why.to_string())?;
        if copied > most {
            return Err(format!("it is larger than {} MiB", most >> 20));
        }
        Ok(())
    }
}

/// A platform a runner runs on, by the processor it has: purlis builds runners for Linux on
/// these two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    X64,
    Arm64,
}

impl Platform {
    /// This machine's, where it is a Linux one purlis builds runners for.
    pub fn here() -> Option<Self> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        match std::env::consts::ARCH {
            "x86_64" => Some(Self::X64),
            "aarch64" => Some(Self::Arm64),
            _ => None,
        }
    }
}

/// **One harness version's release for a platform**: where its download is, the SHA-256 its
/// vendor publishes for it, and, for an archive, the one file in it that is the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub url: String,
    /// Lower-case hex, 64 digits.
    pub sha256: String,
    /// The archive's member that is the program; `None` where the download is the program.
    pub member: Option<String>,
}

/// **Where `kind`'s `version` for `platform` is released, and the digest its vendor publishes
/// for it**, read through `fetch`, or why not.
pub fn release_of(
    kind: &str,
    version: &str,
    platform: Platform,
    fetch: &dyn Fetch,
) -> Result<Release, String> {
    match kind {
        "claude" => {
            let name = match platform {
                Platform::X64 => "linux-x64",
                Platform::Arm64 => "linux-arm64",
            };
            let manifest = described(fetch, &format!("{CLAUDE_RELEASES}/{version}/manifest.json"))?;
            if manifest.get("version").and_then(|v| v.as_str()) != Some(version) {
                return Err(format!("its release manifest is not of {version}"));
            }
            let sha256 = manifest
                .pointer(&format!("/platforms/{name}/checksum"))
                .and_then(|c| c.as_str())
                .ok_or_else(|| format!("its release manifest names no digest for {name}"))?;
            Ok(Release {
                url: format!("{CLAUDE_RELEASES}/{version}/{name}/claude"),
                sha256: hex_digest(sha256)?,
                member: None,
            })
        }
        "codex" => {
            let triple = match platform {
                Platform::X64 => "x86_64-unknown-linux-musl",
                Platform::Arm64 => "aarch64-unknown-linux-musl",
            };
            on_github(
                fetch,
                "openai/codex",
                &format!("rust-v{version}"),
                &format!("codex-{triple}.tar.gz"),
                &format!("codex-{triple}"),
            )
        }
        "opencode" => {
            // The baseline build runs on every x86-64 processor; the other needs AVX2.
            let name = match platform {
                Platform::X64 => "linux-x64-baseline",
                Platform::Arm64 => "linux-arm64",
            };
            on_github(
                fetch,
                "anomalyco/opencode",
                &format!("v{version}"),
                &format!("opencode-{name}.tar.gz"),
                "opencode",
            )
        }
        _ => Err(format!(
            "purlis installs no harness '{}'",
            crate::shown::short(kind)
        )),
    }
}

/// The release `asset` of `repo`'s release `tag`, with the SHA-256 GitHub publishes for it, and
/// the archive's `member` that is the program.
fn on_github(
    fetch: &dyn Fetch,
    repo: &str,
    tag: &str,
    asset: &str,
    member: &str,
) -> Result<Release, String> {
    let release = described(
        fetch,
        &format!("https://api.github.com/repos/{repo}/releases/tags/{tag}"),
    )?;
    let found = release
        .get("assets")
        .and_then(|a| a.as_array())
        .and_then(|assets| {
            assets
                .iter()
                .find(|a| a.get("name").and_then(|n| n.as_str()) == Some(asset))
        })
        .ok_or_else(|| format!("its release {tag} has no {asset}"))?;
    let digest = found
        .get("digest")
        .and_then(|d| d.as_str())
        .and_then(|d| d.strip_prefix("sha256:"))
        .ok_or_else(|| format!("GitHub publishes no SHA-256 for {asset} of {tag}"))?;
    let url = format!("https://github.com/{repo}/releases/download/{tag}/{asset}");
    if found.get("browser_download_url").and_then(|u| u.as_str()) != Some(url.as_str()) {
        return Err(format!("its release {tag} names {asset} somewhere else"));
    }
    Ok(Release {
        url,
        sha256: hex_digest(digest)?,
        member: Some(member.to_owned()),
    })
}

/// The JSON `url` answers, at most [`MOST_DESCRIPTION_BYTES`].
fn described(fetch: &dyn Fetch, url: &str) -> Result<serde_json::Value, String> {
    let mut body = Vec::new();
    fetch
        .fetch(url, MOST_DESCRIPTION_BYTES, &mut body)
        .map_err(|why| format!("its release was not read from {url} ({why})"))?;
    serde_json::from_slice(&body).map_err(|_| format!("{url} did not answer a release"))
}

/// `said`, a SHA-256 in hex, in lower case, or why it is none.
fn hex_digest(said: &str) -> Result<String, String> {
    let said = said.to_ascii_lowercase();
    if said.len() == 64 && said.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(said)
    } else {
        Err("the digest its vendor publishes is not a SHA-256".to_owned())
    }
}

/// Where `kind`'s `version` is installed under the data home `data`: its program.
pub fn program_in(data: &Path, kind: &str, version: &str) -> Option<PathBuf> {
    let program = &crate::harness_declaration::builtin(kind)?.program;
    Some(data.join(DIR).join(kind).join(version).join(program))
}

/// **`kind`'s `version`, installed under the data home `data`**: its program, there already or
/// installed now for `platform` through `fetch`, or why not, in words a chat says.
pub fn install(
    data: &Path,
    kind: &str,
    version: &str,
    platform: Platform,
    fetch: &dyn Fetch,
) -> Result<PathBuf, String> {
    let title = crate::harness_declaration::builtin(kind)
        .map_or_else(|| kind.to_owned(), |d| d.title.clone());
    let not = |why: String| format!("{title} {version} was not installed on this runner: {why}");
    // A version as Semantic Versioning writes it names no other folder.
    let version = super::pin::Version::parse(version)
        .map_err(|_| not("it is not a version".to_owned()))?
        .to_string();
    let program = program_in(data, kind, &version)
        .ok_or_else(|| not("purlis installs no such harness".to_owned()))?;
    if is_installed(&program) {
        return Ok(program);
    }
    let folder = data.join(DIR).join(kind);
    private_dirs(&folder).map_err(|e| not(crate::rewrite::os_words(&e)))?;
    // One install of a harness at a time: a second start waits, and finds it installed.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(folder.join(".lock"))
        .map_err(|e| not(crate::rewrite::os_words(&e)))?;
    let _held = crate::filelock::lock_within(lock, Duration::from_secs(30 * 60), "a harness")
        .map_err(|e| not(crate::rewrite::os_words(&e)))?;
    if is_installed(&program) {
        return Ok(program);
    }
    let release = release_of(kind, &version, platform, fetch).map_err(not)?;
    let download = folder.join(format!(".{version}.download"));
    let staging = folder.join(format!(".{version}.staging"));
    let installed = (|| {
        let _ = std::fs::remove_dir_all(&staging);
        let digest = downloaded(fetch, &release.url, &download)?;
        if digest != release.sha256 {
            return Err(format!(
                "what {} answered does not match the SHA-256 its vendor publishes for it",
                release.url
            ));
        }
        private_dirs(&staging).map_err(|e| crate::rewrite::os_words(&e))?;
        let at = staging.join(program.file_name().unwrap_or_default());
        match &release.member {
            None => std::fs::rename(&download, &at).map_err(|e| crate::rewrite::os_words(&e))?,
            Some(member) => unpacked(&download, member, &at)?,
        }
        executable(&at).map_err(|e| crate::rewrite::os_words(&e))?;
        let into = program.parent().unwrap_or(&folder);
        let _ = std::fs::remove_dir_all(into);
        std::fs::rename(&staging, into).map_err(|e| crate::rewrite::os_words(&e))
    })();
    let _ = std::fs::remove_file(&download);
    let _ = std::fs::remove_dir_all(&staging);
    installed.map_err(not)?;
    Ok(program)
}

/// Whether `program` is there: a regular file, reached through no link.
fn is_installed(program: &Path) -> bool {
    std::fs::symlink_metadata(program).is_ok_and(|m| m.file_type().is_file())
}

/// Fetches `url` into the file `to`, made new, and answers the SHA-256 of what was written.
fn downloaded(fetch: &dyn Fetch, url: &str, to: &Path) -> Result<String, String> {
    let _ = std::fs::remove_file(to);
    let file = private_file(to).map_err(|e| crate::rewrite::os_words(&e))?;
    let mut hashing = Hashing {
        to: io::BufWriter::new(file),
        sha: Sha256::new(),
    };
    fetch
        .fetch(url, MOST_DOWNLOAD_BYTES, &mut hashing)
        .map_err(|why| format!("{url} was not downloaded ({why})"))?;
    hashing
        .to
        .into_inner()
        .map_err(|e| crate::rewrite::os_words(e.error()))?
        .sync_all()
        .map_err(|e| crate::rewrite::os_words(&e))?;
    Ok(crate::extension::hex(&hashing.sha.finalize()))
}

/// A writer that hashes what it writes.
struct Hashing<W> {
    to: W,
    sha: Sha256,
}

impl<W: Write> Write for Hashing<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let n = self.to.write(bytes)?;
        self.sha.update(&bytes[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.to.flush()
    }
}

/// Unpacks the regular file `member` of the gzipped tar `archive` into the file `to`, made new:
/// nothing else of the archive is written, and a link of that name is not followed.
fn unpacked(archive: &Path, member: &str, to: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| crate::rewrite::os_words(&e))?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(io::BufReader::new(file)));
    let entries = tar
        .entries()
        .map_err(|_| "the download is not the archive its release names".to_owned())?;
    for entry in entries {
        let mut entry =
            entry.map_err(|_| "the download is not the archive its release names".to_owned())?;
        let named = entry.path().ok().is_some_and(|p| {
            let p: &Path = p.as_ref();
            p == Path::new(member) || p == Path::new(".").join(member)
        });
        if !named {
            continue;
        }
        if !entry.header().entry_type().is_file() {
            return Err(format!("{member} in the download is not a file"));
        }
        let mut out = private_file(to).map_err(|e| crate::rewrite::os_words(&e))?;
        io::copy(&mut (&mut entry).take(MOST_DOWNLOAD_BYTES), &mut out)
            .map_err(|e| crate::rewrite::os_words(&e))?;
        return out.sync_all().map_err(|e| crate::rewrite::os_words(&e));
    }
    Err(format!("the download holds no {member}"))
}

/// Makes the file `path`, new, readable and writable by this user alone.
fn private_file(path: &Path) -> io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    options.open(path)
}

/// Lets this user alone run `path`.
fn executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Makes `dir` and every folder above it that is missing, each 0700.
fn private_dirs(dir: &Path) -> io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dir)
}

/// **The program a chat on `record`, this runner's, runs**: for a profile kept for a desktop with
/// a pin, the pinned version installed under the data home `data` for `platform` through `fetch`
/// (now, where it is not there yet); `None` for any other profile, which runs the harness on
/// `PATH`, and nothing is fetched for it.
pub fn program_for(
    record: &MachineProfile,
    data: &Path,
    platform: Option<Platform>,
    fetch: &dyn Fetch,
) -> Result<Option<PathBuf>, String> {
    let (Some(_), Some(pinned)) = (&record.kept_for, &record.pinned) else {
        return Ok(None);
    };
    let platform = platform.ok_or_else(|| {
        format!(
            "this runner is no Linux machine purlis installs {} on",
            record.harness
        )
    })?;
    install(data, &record.harness, &pinned.version, platform, fetch).map(Some)
}

/// **The program a chat on `profile` starts with on this machine** (#3446): where this machine
/// is a runner keeping it for its desktop with a pin, the pinned version, installed now where it
/// is not there yet. `None` for every other profile and machine.
pub fn for_a_start(profile: &crate::profiles::Profile) -> Result<Option<PathBuf>, String> {
    let records = super::listed_here();
    let Some(record) = super::pin::record_of(profile, &records) else {
        return Ok(None);
    };
    if record.kept_for.is_none() || record.pinned.is_none() {
        return Ok(None);
    }
    let data = crate::datahome::root()
        .ok_or_else(|| "this runner has no data home to install its harness in".to_owned())?;
    program_for(record, &data, Platform::here(), &OverHttps)
}

/// [`for_a_start`]'s program where it is installed already, with nothing fetched: what a check
/// ahead of a start asks.
pub fn installed_for(profile: &crate::profiles::Profile) -> Option<PathBuf> {
    let records = super::listed_here();
    let record = super::pin::record_of(profile, &records)?;
    record.kept_for.as_ref()?;
    let pinned = record.pinned.as_ref()?;
    let version = super::pin::Version::parse(&pinned.version)
        .ok()?
        .to_string();
    let program = program_in(&crate::datahome::root()?, &record.harness, &version)?;
    is_installed(&program).then_some(program)
}

#[cfg(test)]
mod tests;
