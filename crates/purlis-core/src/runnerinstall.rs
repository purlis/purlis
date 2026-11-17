// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Installing purlis on a runner over SSH, side by side** (RR-14, #723; ADR 0078 §5, V22a).
//!
//! A runner reached over SSH runs the same `purlis` binary the desktop ships, built for the
//! runner's platform, from `<config>/server/<version>/purlis`, `<config>` being the machine store
//! as the runner resolves it. This module installs it there, over the pinned route of the SSH
//! module ([`crate::sshroute`], #6266), and nothing else:
//!
//! 1. **The probe** ([`probe_script`]) asks the runner, through the route, its platform, its
//!    resolved `<config>`, which of sha256sum and shasum it has, whether it has curl (or only
//!    wget, which purlis does not download with), and the versions already installed. Its one
//!    write is sweeping away the temporary files of deliveries cut off more than a day ago.
//! 2. **The desktop has already verified the asset** ([`verify`]): its minisign signature by ADR
//!    0042's key, with the version and the asset's own name in the signature's trusted comment,
//!    as a bundle's and the runner image's are. Only then is its SHA-256 trusted.
//! 3. **The file lands under a temporary name, not executable**: `<config>/server/.incoming-*`,
//!    `0600`, made by a run of its own ([`incoming_command`]) before a byte is sent, so purlis
//!    knows its path and deletes it however the delivery ends. It is then filled, either uploaded
//!    through the route ([`Delivery::Upload`]) or downloaded by the runner from GitHub Releases
//!    by curl over https alone, redirects included ([`Delivery::Download`]).
//! 4. **One command there checks it and places it** ([`settle_command`]): its SHA-256 computed
//!    on the runner and compared with the verified one there, and only on a match made
//!    executable and moved, by one rename, to `<config>/server/<version>/purlis`. A mismatch
//!    deletes the file in the same command and refuses, in plain words ([`Refused::NotTheSame`]).
//!    A version already there is hashed again the same way before it is trusted
//!    ([`installed_command`]), and replaced where it is not the file verified.
//!
//! **What the check protects is the transfer** (ADR 0078 §5 as amended 2026-11-15): that the
//! bytes placed are the bytes the release signed, whatever the network or a mirror did to them.
//! It does not protect against the runner's account itself: whoever can run commands as that
//! user can replace the binary after it is placed, or answer the hash for it, and no check run
//! as that user can tell.
//!
//! **Every command is `exec sh -c '…'`** ([`for_login_shell`]), as sshroute's
//! [`crate::sshroute::HELD_BY`] is (ADR 0123 as amended 2026-11-12): ssh hands the command to the
//! person's login shell, which may be `bash`, `zsh` or `fish`, and each reads that line alike,
//! so the script inside is read by POSIX `sh` alone.
//!
//! **Versions sit side by side** (ADR 0068 §8): placing a version never touches another
//! version's folder, and never stops a host. A binary run from `<config>/server/<version>/` knows its
//! version from where it is ([`version_of_binary`]), and its host takes a place of its own
//! (`crate::hostsocket::here`, `crate::machine::host_folder`), so a new version's host starts
//! beside the old one, takes the new chats, and the old one keeps its chats until each ends.
//!
//! **A drained version goes on the next install** ([`prune`], #6688): once it is in place, the
//! running version's binary there removes the folders of every version older than the one
//! before it whose host has drained, no host serving it and no chat left in its record
//! ([`prune`](mod@prune) says how). The running version and the one before it always stay.
//!
//! **Nothing ssh prints decides anything but what purlis's own commands print and how they
//! exit**: the probe's lines, the temporary file's path, and the exit of the command that checks
//! and places it. What ssh says on standard error is shown, never read.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::sshroute::{self, Reach};

/// The folder under `<config>` the versions live in.
pub const SERVER_DIR: &str = "server";

/// The binary's name in its version's folder.
pub const BINARY: &str = "purlis";

/// The start of a file's temporary name in `<config>/server/`, before what `mktemp` adds.
const INCOMING: &str = ".incoming-";

/// How many characters `mktemp` adds to [`INCOMING`].
const INCOMING_RANDOM: usize = 10;

/// Where a release's runner assets are downloaded from (ADR 0042's trust root is the key, not
/// this address: whatever arrives is checked against the verified digest).
pub const RELEASES: &str = "https://github.com/purlis/purlis/releases/download";

/// How long an upload or a download may take as a whole: a runner's binary is tens of megabytes
/// and a link can be slow (ADR 0078, *What this costs*).
pub const DELIVER_WITHIN: Duration = Duration::from_secs(30 * 60);

/// The most of what the probe prints that is read.
const MOST_PROBED: u64 = 16 * 1024;

/// How [`settle_command`] and [`installed_command`] exit when the file there is not the one
/// verified (ssh's own failures are 255).
pub const NOT_THE_SAME: i32 = 3;

/// **`script` as a command for the runner's login shell**: `exec sh -c '<script>'`, so POSIX
/// `sh` reads the script whatever the person's login shell is (sshroute's
/// [`crate::sshroute::HELD_BY`], ADR 0123 as amended 2026-11-12). The script holds no `'`, which
/// would end the quoting, no `\`, which `fish` reads inside single quotes, and no `!`, which
/// `csh` would: every part of it is purlis's own text or a path, version or digest that is
/// plain ([`a_plain_path`], [`a_version`]).
///
/// # Panics
/// On a script with one of those three, which no input that passed those checks can make.
#[must_use]
pub fn for_login_shell(script: &str) -> String {
    assert!(
        !script.contains(['\'', '\\', '!']),
        "a script for the runner's login shell holds a quote, backslash or bang: {script}"
    );
    format!("exec sh -c '{script}'")
}

/// **A platform purlis runs on as a runner** (ADR 0078 §5): one release asset each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Target {
    LinuxX86_64,
    LinuxAarch64,
    MacosArm64,
}

impl Target {
    pub const ALL: [Target; 3] = [
        Target::LinuxX86_64,
        Target::LinuxAarch64,
        Target::MacosArm64,
    ];

    /// The target's name, as the release's assets are named by it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Target::LinuxX86_64 => "linux-x86_64",
            Target::LinuxAarch64 => "linux-aarch64",
            Target::MacosArm64 => "macos-arm64",
        }
    }

    /// The target in words a person reads.
    #[must_use]
    pub fn said(self) -> &'static str {
        match self {
            Target::LinuxX86_64 => "Linux on Intel or AMD",
            Target::LinuxAarch64 => "Linux on ARM",
            Target::MacosArm64 => "macOS on Apple silicon",
        }
    }

    /// **The target `uname -s` and `uname -m` name**, or why purlis can't run there, in words.
    pub fn of_uname(system: &str, machine: &str) -> Result<Target, String> {
        match (system.trim(), machine.trim()) {
            ("Linux", "x86_64" | "amd64") => Ok(Target::LinuxX86_64),
            ("Linux", "aarch64" | "arm64") => Ok(Target::LinuxAarch64),
            ("Darwin", "arm64") => Ok(Target::MacosArm64),
            (system, machine) => Err(format!(
                "that computer runs {} on {}, and purlis runs there only on Linux (Intel, AMD or \
                 ARM) or on a Mac with Apple silicon",
                crate::shown::short(system),
                crate::shown::short(machine)
            )),
        }
    }

    /// The target this binary was built for, where it is one.
    #[must_use]
    pub fn here() -> Option<Target> {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Target::LinuxX86_64)
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Some(Target::LinuxAarch64)
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Target::MacosArm64)
        } else {
            None
        }
    }
}

/// **Whether `version` can name a folder and an asset**: a release's version (`1.2.3`,
/// `1.2.3-dev.45`, `1.2.3+abc`), starting with a digit, of at most 64 characters, with nothing a
/// shell or a path would read as its own.
#[must_use]
pub fn a_version(version: &str) -> bool {
    version.len() <= 64
        && version.starts_with(|c: char| c.is_ascii_digit())
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
        && !version.contains("..")
}

/// **The release asset's name for `version` on `target`**: `purlis-<version>-<target>`.
#[must_use]
pub fn asset_name(version: &str, target: Target) -> String {
    format!("{BINARY}-{version}-{}", target.name())
}

/// **Where the release publishes that asset**: on the `v<version>` release, or, for a dev
/// build (`<version>-dev.<run>`), on the one `dev` release whose tag moves with every build
/// (`crate::updates::DEV_TAG`). Its signature is beside it, with `.sig` (`release-manifest`'s
/// `sign-runner-binary`). The asset's name carries its version, so the address of one dev
/// build's asset never names another's, and the `dev` release keeps the last five dev builds'
/// runner assets (`tools/dev-runner-assets.mjs`), not only the newest: a desktop a few dev
/// builds behind still finds its own version's there.
#[must_use]
pub fn asset_url(version: &str, target: Target) -> String {
    let tag = if version.contains("-dev.") {
        crate::updates::DEV_TAG.to_owned()
    } else {
        format!("v{version}")
    };
    format!("{RELEASES}/{tag}/{}", asset_name(version, target))
}

/// The SHA-256 of `bytes`, as 64 lower-case hex digits.
#[must_use]
pub fn digest_of(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// **A runner asset the desktop verified**: its version, its platform and the SHA-256 the
/// runner's copy must have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    version: String,
    target: Target,
    digest: String,
}

impl Verified {
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn target(&self) -> Target {
        self.target
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// **The asset `bytes` verified** with the updater's public key `pubkey` (the base64 that
/// `tauri.conf.json` carries, ADR 0042) against `signature` (base64 of a minisign signature
/// file, as the release writes it beside the asset): signed by that key over these exact bytes,
/// for `version` and for this asset's own name in its trusted comment
/// (`version:<version>` and `file:<asset name>`), so an asset of another version or platform
/// is never taken for this one. Refused in a sentence otherwise.
pub fn verify(
    pubkey: &str,
    bytes: &[u8],
    signature: &str,
    version: &str,
    target: Target,
) -> Result<Verified, String> {
    use base64::Engine as _;
    if !a_version(version) {
        return Err(format!(
            "{} isn't a version purlis installs",
            crate::shown::short(version)
        ));
    }
    let text = |b64: &str, what: &str| {
        base64::engine::general_purpose::STANDARD
            .decode(b64.trim())
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .ok_or_else(|| format!("the {what} is not base64 text"))
    };
    let key = minisign_verify::PublicKey::decode(&text(pubkey, "updater's public key")?)
        .map_err(|why| format!("the updater's public key could not be read: {why}"))?;
    let signature = minisign_verify::Signature::decode(&text(signature, "runner's signature")?)
        .map_err(|why| format!("the runner's signature could not be read: {why}"))?;
    key.verify(bytes, &signature, false).map_err(|_| {
        format!(
            "this copy of purlis {version} for {} isn't signed by purlis's release key, so it \
             was not installed",
            target.said()
        )
    })?;
    let comment = signature.trusted_comment();
    let field = |name: &str| {
        comment
            .split('\t')
            .find_map(|field| field.strip_prefix(name))
            .map(str::to_owned)
    };
    let signed_for = field("version:");
    if signed_for.as_deref() != Some(version) {
        return Err(format!(
            "this copy of purlis is signed for version {}, not {version}, so it was not installed",
            signed_for.as_deref().unwrap_or("none")
        ));
    }
    let asset = asset_name(version, target);
    if field("file:").as_deref() != Some(asset.as_str()) {
        return Err(format!(
            "this copy of purlis is signed as {}, not as the one for {}, so it was not installed",
            field("file:").unwrap_or_else(|| "nothing".to_owned()),
            target.said()
        ));
    }
    Ok(Verified {
        version: version.to_owned(),
        target,
        digest: digest_of(bytes),
    })
}

/// The tool that computes a SHA-256 on the runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashTool {
    /// GNU coreutils' and busybox's `sha256sum`.
    Sha256sum,
    /// Perl's `shasum -a 256`, what macOS has.
    Shasum,
}

/// **What the runner can download with.** Only curl is used: it can be told to refuse every
/// protocol but https for the address and for each redirect alike (`--proto =https
/// --proto-redir =https`). wget has no switch that does that for one file (its `--https-only`
/// is for links a recursive download follows), so a runner with only wget is told so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fetch {
    Curl,
    /// wget and no curl: purlis does not download with it.
    OnlyWget,
    Neither,
}

/// **What the probe found on the runner.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probed {
    pub target: Target,
    /// `<config>`, the runner's purlis folder in its machine store, absolute and plain.
    pub config: String,
    pub hash: Option<HashTool>,
    pub fetch: Fetch,
    /// The versions already installed, each a folder with an executable `purlis` in it.
    pub versions: Vec<String>,
}

/// **The probe** (ADR 0078 §5), a POSIX `sh` script, run as [`for_login_shell`] wraps it. It
/// prints one `key=value` line each: `system` and `machine` (`uname -s`, `uname -m`), `config`
/// (the machine store resolved as purlis resolves it: `$PURLIS_CONFIG_HOME`, its old spelling,
/// then `$XDG_CONFIG_HOME`, then `~/.config`, and in it the folder of purlis's name that is
/// there), `hash` (the tool it has, or `none`), `fetch` (`curl`, `wget` where wget is the only
/// one, or `none`), and a `version` line for each version installed. Its one write: the
/// temporary files in `<config>/server/` more than a day old, which only a delivery cut off
/// leaves, are deleted.
#[must_use]
pub fn probe_script() -> String {
    let env = |rest: &str| -> Vec<String> {
        crate::names::ENV_PREFIX
            .spellings()
            .map(|prefix| format!("{prefix}{rest}"))
            .collect()
    };
    // `$PURLIS_CONFIG_HOME` when it is set at all, else its old spelling, as
    // `envvar::lookup` picks; a blank one is unset (`machine::rooted`).
    let mut pick = String::new();
    for (at, name) in env("CONFIG_HOME").iter().enumerate() {
        pick.push_str(if at == 0 { "if" } else { " elif" });
        pick.push_str(&format!(" [ -n \"${{{name}+x}}\" ]; then h=\"${name}\";"));
    }
    pick.push_str(" else h=; fi");
    // The folder: the first spelling that is a folder there, else the newest old one
    // (`names::Name::folder_at`).
    let folder = crate::names::CONFIG_HOME;
    let mut choose = String::new();
    for (at, name) in folder.spellings().enumerate() {
        choose.push_str(if at == 0 { "if" } else { " elif" });
        choose.push_str(&format!(" [ -d \"$h/{name}\" ]; then d=\"$h/{name}\";"));
    }
    choose.push_str(&format!(" else d=\"$h/{}\"; fi", folder.newest_old()));
    format!(
        "{pick}; [ -n \"$h\" ] || h=\"${{XDG_CONFIG_HOME:-}}\"; [ -n \"$h\" ] || h=\"$HOME/.config\"; \
         {choose}; \
         echo \"system=$(uname -s)\"; echo \"machine=$(uname -m)\"; echo \"config=$d\"; \
         if command -v sha256sum >/dev/null 2>&1; then echo hash=sha256sum; \
         elif command -v shasum >/dev/null 2>&1; then echo hash=shasum; else echo hash=none; fi; \
         if command -v curl >/dev/null 2>&1; then echo fetch=curl; \
         elif command -v wget >/dev/null 2>&1; then echo fetch=wget; else echo fetch=none; fi; \
         [ -d \"$d/{SERVER_DIR}\" ] && find \"$d/{SERVER_DIR}\" -maxdepth 1 -type f \
         -name \"{INCOMING}*\" -mtime +0 -exec rm -f -- {{}} + 2>/dev/null; \
         for v in \"$d/{SERVER_DIR}\"/*/; do v=${{v%/}}; \
         [ -x \"$v/{BINARY}\" ] && [ -f \"$v/{BINARY}\" ] && echo \"version=${{v##*/}}\"; done; true"
    )
}

/// **What the probe printed, read**, or why it can't be used, in words.
pub fn probed_from(printed: &str) -> Result<Probed, String> {
    let mut system = None;
    let mut machine = None;
    let mut config = None;
    let mut hash = None;
    let mut fetch = Fetch::Neither;
    let mut versions = Vec::new();
    for line in printed.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "system" => system = Some(value.to_owned()),
            "machine" => machine = Some(value.to_owned()),
            "config" => config = Some(value.to_owned()),
            "hash" => {
                hash = match value {
                    "sha256sum" => Some(HashTool::Sha256sum),
                    "shasum" => Some(HashTool::Shasum),
                    _ => None,
                }
            }
            "fetch" => {
                fetch = match value {
                    "curl" => Fetch::Curl,
                    "wget" => Fetch::OnlyWget,
                    _ => Fetch::Neither,
                }
            }
            // A folder that is not a version purlis names is no version of its.
            "version" if a_version(value) => versions.push(value.to_owned()),
            _ => {}
        }
    }
    let (Some(system), Some(machine), Some(config)) = (system, machine, config) else {
        return Err("that computer didn't answer purlis's check".to_owned());
    };
    let target = Target::of_uname(&system, &machine)?;
    if !a_plain_path(&config) {
        return Err(format!(
            "purlis's folder on that computer would be {}, which purlis can't install into: it \
             must be a full path with no spaces or quotes",
            crate::shown::short(&config)
        ));
    }
    versions.sort();
    versions.dedup();
    Ok(Probed {
        target,
        config,
        hash,
        fetch,
        versions,
    })
}

/// **Whether `path` is a path purlis writes into a command for the runner's shell as it is**:
/// absolute, with no `..`, and only letters, digits and `/._-+@`. Anything else is refused,
/// never quoted, so no path read from the runner can become shell syntax there.
#[must_use]
pub fn a_plain_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 1024
        && !path.split('/').any(|part| part == "..")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+' | '@'))
}

/// `<config>/server`.
#[must_use]
pub fn server_dir(config: &str) -> String {
    format!("{config}/{SERVER_DIR}")
}

/// `<config>/server/<version>/purlis`: what a connector runs (`... purlis bridge`).
#[must_use]
pub fn binary_at(config: &str, version: &str) -> String {
    format!("{}/{version}/{BINARY}", server_dir(config))
}

/// **The probe as a command for the runner's login shell.**
#[must_use]
pub fn probe_command() -> String {
    for_login_shell(&probe_script())
}

/// **What makes the temporary file there**, before anything is sent: in `<config>/server/`,
/// `0600`, empty, its path printed, so purlis can delete it however the delivery that follows
/// ends.
#[must_use]
pub fn incoming_command(config: &str) -> String {
    let server = server_dir(config);
    for_login_shell(&format!(
        "umask 077 && mkdir -p -- {server} && mktemp {server}/{INCOMING}XXXXXXXXXX"
    ))
}

/// **What fills `incoming` there with what arrives on standard input**: an upload.
#[must_use]
pub fn receive_command(incoming: &str) -> String {
    for_login_shell(&format!("cat > {incoming}"))
}

/// **What downloads `url` into `incoming` there, with curl over https alone**: the address and
/// every redirect (`--proto =https --proto-redir =https`), the person's `.curlrc` not read
/// (`-q`, which must come first). Refused for an address that is not one of the release's (`url`
/// is written into the command, so it must be plain).
pub fn fetch_command(incoming: &str, url: &str) -> Result<String, String> {
    let plain = url.strip_prefix(RELEASES).is_some_and(|rest| {
        rest.starts_with('/')
            && !rest.contains("..")
            && rest
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+'))
    });
    if !plain {
        return Err(format!(
            "{} isn't where purlis's releases are published",
            crate::shown::short(url)
        ));
    }
    Ok(for_login_shell(&format!(
        "curl -q --proto =https --proto-redir =https --tlsv1.2 --fail --silent --show-error \
         --location -o {incoming} {url}"
    )))
}

/// **Whether `printed` is a temporary file [`incoming_command`] made in `config`'s server
/// folder**: the path, or `None`.
#[must_use]
pub fn incoming_from(config: &str, printed: &str) -> Option<String> {
    let path = printed.trim_end_matches('\n');
    let name = path.strip_prefix(&format!("{}/", server_dir(config)))?;
    let random = name.strip_prefix(INCOMING)?;
    (random.len() == INCOMING_RANDOM && random.chars().all(|c| c.is_ascii_alphanumeric()))
        .then(|| path.to_owned())
}

/// The part of a script that sets `$1` to the SHA-256 of `file` there, as `hash` prints it
/// (its first word; unset where the tool fails).
fn hashed(hash: HashTool, file: &str) -> String {
    match hash {
        HashTool::Sha256sum => format!("set -- $(sha256sum -- {file})"),
        HashTool::Shasum => format!("set -- $(shasum -a 256 -- {file})"),
    }
}

/// **What checks `incoming` and places it, in one command there**: its SHA-256 computed with
/// `hash` and compared with `digest`, the verified one; on a match it is made executable, by
/// this user alone, and moved by one rename to `<config>/server/<version>/purlis`, and on any
/// other answer deleted, printing `found=<what the tool said>` and exiting [`NOT_THE_SAME`].
/// Another version's folder is not touched. `digest` is 64 hex digits, as [`verify`] makes it.
#[must_use]
pub fn settle_command(
    hash: HashTool,
    config: &str,
    version: &str,
    incoming: &str,
    digest: &str,
) -> String {
    let folder = format!("{}/{version}", server_dir(config));
    // No `--` for chmod: macOS's refuses it after a numeric mode. Every path here is absolute
    // ([`a_plain_path`], [`incoming_from`]), so none is read as an option.
    for_login_shell(&format!(
        "{}; if [ \"${{1:-}}\" = {digest} ]; then umask 077 && mkdir -p -- {folder} && \
         chmod 0700 {incoming} && mv -f -- {incoming} {folder}/{BINARY}; else rm -f -- {incoming}; \
         echo \"found=${{1:-}}\"; exit {NOT_THE_SAME}; fi",
        hashed(hash, incoming)
    ))
}

/// **What checks a version already there**: the SHA-256 of `binary`, computed with `hash`,
/// compared with `digest`; exits 0 on a match and [`NOT_THE_SAME`] otherwise.
#[must_use]
pub fn installed_command(hash: HashTool, binary: &str, digest: &str) -> String {
    for_login_shell(&format!(
        "{}; [ \"${{1:-}}\" = {digest} ] || exit {NOT_THE_SAME}",
        hashed(hash, binary)
    ))
}

/// What [`settle_command`] said it found, where it said.
fn found_in(printed: &str) -> String {
    printed
        .lines()
        .find_map(|line| line.strip_prefix("found="))
        .filter(|word| word.len() <= 64 && word.chars().all(|c| c.is_ascii_hexdigit()))
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// **What deletes `incoming`**.
#[must_use]
pub fn discard_command(incoming: &str) -> String {
    for_login_shell(&format!("rm -f -- {incoming}"))
}

/// **How the asset reaches the runner.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery<'a> {
    /// Sent from this computer through the route: for a runner with no way out to the
    /// internet. These are the bytes [`verify`] checked.
    Upload(&'a [u8]),
    /// Downloaded by the runner from the release, over https: for a slow link from here.
    Download,
}

impl Delivery<'_> {
    /// How the event log says it (`runner.installed`, ADR 0078 §10).
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Delivery::Upload(_) => "upload",
            Delivery::Download => "download",
        }
    }
}

/// **What an install did**, and the drained versions it removed ([`prune`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Installed {
    /// The version was placed, at this path there.
    Placed { binary: String, pruned: Vec<String> },
    /// The version was there already; nothing was sent.
    AlreadyThere { binary: String, pruned: Vec<String> },
}

impl Installed {
    /// The binary's path on the runner.
    #[must_use]
    pub fn binary(&self) -> &str {
        match self {
            Installed::Placed { binary, .. } | Installed::AlreadyThere { binary, .. } => binary,
        }
    }

    /// The drained versions removed from the runner as it was installed ([`prune`]).
    #[must_use]
    pub fn pruned(&self) -> &[String] {
        match self {
            Installed::Placed { pruned, .. } | Installed::AlreadyThere { pruned, .. } => pruned,
        }
    }
}

/// **Why purlis was not installed**, each in words a person reads.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// The runner's copy is not the file purlis verified: deleted, nothing installed.
    #[error(
        "the copy of purlis that reached {host} isn't the one purlis checked, so it was deleted \
         and nothing was installed. Try again. If this happens again, something between this \
         computer and {host} is changing the file"
    )]
    NotTheSame {
        host: String,
        expected: String,
        found: String,
    },
    /// The asset is for another platform than the runner's.
    #[error(
        "this copy of purlis is for {asset}, but {host} runs {runner}, so nothing was installed"
    )]
    WrongPlatform {
        host: String,
        asset: &'static str,
        runner: &'static str,
    },
    /// The runner has no tool to check the copy with.
    #[error(
        "{host} has neither sha256sum nor shasum, so purlis can't check the copy it puts there. \
         Install coreutils on {host}, then try again"
    )]
    NoHashTool { host: String },
    /// The runner can't download: it has no curl, and no wget either.
    #[error(
        "{host} doesn't have curl to download purlis with. Install curl on {host}, or send purlis \
         from this computer instead"
    )]
    NoFetcher { host: String },
    /// The runner has wget and no curl: wget can't be held to https through a redirect.
    #[error(
        "{host} has wget but not curl. purlis downloads only with curl, because wget can't be \
         made to refuse a redirect away from https. Install curl on {host}, or send purlis from \
         this computer instead"
    )]
    OnlyWget { host: String },
    /// The probe's answer can't be used.
    #[error("purlis can't install on {host}: {why}")]
    Unusable { host: String, why: String },
    /// The route failed, or the runner refused a step: what it said, shown as it is.
    #[error("purlis couldn't install on {host}: {said}")]
    Unreached { host: String, said: String },
}

/// **One command run there, through the pinned route**, with `input` on its standard input and
/// the whole run bounded by `within`.
fn run_there(
    reach: &Reach,
    command: String,
    input: Option<&[u8]>,
    most: u64,
    within: Duration,
) -> Result<sshroute::Ran, Refused> {
    let unreached = |said: String| Refused::Unreached {
        host: reach.route.host().to_owned(),
        said,
    };
    let (_folder, known_hosts) = reach.known_hosts_for_a_run().map_err(unreached)?;
    let mut args =
        sshroute::sign_in_args(&reach.route, &reach.pool, &known_hosts).map_err(unreached)?;
    args.pop();
    args.push(command);
    sshroute::run_giving_bytes(&reach.ssh, &args, input, most, within).map_err(unreached)
}

/// What a run that did not succeed said, as a refusal.
fn unreached(reach: &Reach, ran: sshroute::Ran) -> Refused {
    Refused::Unreached {
        host: reach.route.host().to_owned(),
        said: if ran.said.is_empty() {
            format!("it stopped with code {}", ran.code.unwrap_or(-1))
        } else {
            ran.said
        },
    }
}

/// A run that must succeed: what it printed, or what it said.
fn ran_ok(reach: &Reach, ran: sshroute::Ran) -> Result<String, Refused> {
    if ran.code == Some(0) {
        Ok(ran.printed)
    } else {
        Err(unreached(reach, ran))
    }
}

/// **The probe run on the runner through `reach`**, and what it found.
pub fn probe(reach: &Reach) -> Result<Probed, Refused> {
    let ran = run_there(
        reach,
        probe_command(),
        None,
        MOST_PROBED,
        sshroute::RUN_WITHIN,
    )?;
    let printed = ran_ok(reach, ran)?;
    probed_from(&printed).map_err(|why| Refused::Unusable {
        host: reach.route.host().to_owned(),
        why,
    })
}

/// **`verified` installed on the runner `probed` describes, through `reach`**, delivered as
/// `delivery` says: the steps of the module's docs. A version already there is hashed again
/// and left as it is where it is the file verified, and replaced, by the same steps, where it
/// is not. Once the temporary file is made, whatever happens, it is either placed or deleted.
pub fn install(
    reach: &Reach,
    probed: &Probed,
    verified: &Verified,
    delivery: Delivery<'_>,
) -> Result<Installed, Refused> {
    let host = reach.route.host().to_owned();
    let binary = binary_at(&probed.config, verified.version());
    if verified.target() != probed.target {
        return Err(Refused::WrongPlatform {
            host,
            asset: verified.target().said(),
            runner: probed.target.said(),
        });
    }
    let Some(hash) = probed.hash else {
        return Err(Refused::NoHashTool { host });
    };
    if probed.versions.iter().any(|v| v == verified.version()) {
        let ran = run_there(
            reach,
            installed_command(hash, &binary, verified.digest()),
            None,
            4 * 1024,
            sshroute::RUN_WITHIN,
        )?;
        match ran.code {
            Some(0) => {
                return Ok(Installed::AlreadyThere {
                    pruned: pruned_after(reach, probed, verified),
                    binary,
                });
            }
            // Not the file verified: placed again below, by one rename over it, which a host
            // running the old file keeps running.
            Some(NOT_THE_SAME) => {}
            _ => return Err(unreached(reach, ran)),
        }
    }
    let (url, input) = match delivery {
        Delivery::Upload(bytes) => {
            if digest_of(bytes) != verified.digest() {
                return Err(Refused::Unusable {
                    host,
                    why: "the file to send isn't the one purlis checked".to_owned(),
                });
            }
            (None, Some(bytes))
        }
        Delivery::Download => match probed.fetch {
            Fetch::Curl => (Some(asset_url(verified.version(), verified.target())), None),
            Fetch::OnlyWget => return Err(Refused::OnlyWget { host }),
            Fetch::Neither => return Err(Refused::NoFetcher { host }),
        },
    };
    let made = run_there(
        reach,
        incoming_command(&probed.config),
        None,
        4 * 1024,
        sshroute::RUN_WITHIN,
    )?;
    let printed = ran_ok(reach, made)?;
    let Some(incoming) = incoming_from(&probed.config, &printed) else {
        return Err(Refused::Unusable {
            host,
            why: "it didn't say where it put the file".to_owned(),
        });
    };
    let settled = deliver(reach, &incoming, url.as_deref(), input).and_then(|()| {
        run_there(
            reach,
            settle_command(
                hash,
                &probed.config,
                verified.version(),
                &incoming,
                verified.digest(),
            ),
            None,
            4 * 1024,
            sshroute::RUN_WITHIN,
        )
    });
    match settled {
        Ok(ran) if ran.code == Some(0) => Ok(Installed::Placed {
            pruned: pruned_after(reach, probed, verified),
            binary,
        }),
        // Deleted by the command itself.
        Ok(ran) if ran.code == Some(NOT_THE_SAME) => Err(Refused::NotTheSame {
            host,
            expected: verified.digest().to_owned(),
            found: found_in(&ran.printed),
        }),
        Ok(ran) => {
            discard(reach, &incoming);
            Err(unreached(reach, ran))
        }
        Err(refused) => {
            discard(reach, &incoming);
            Err(refused)
        }
    }
}

/// **The drained versions removed from the runner `probed` describes, through `reach`**, by
/// the binary of `version` there, the running one ([`prune::prune_command`]): every version
/// older than the one before it whose host has drained (`prune`'s docs). The versions it
/// said it removed; a version with a live chat is never one of them. `probed.config` and
/// `version` are held to the plain checks again ([`a_plain_path`], [`a_version`]), since a
/// `Probed` may be made by hand: one that fails them is refused, and nothing is run.
#[cfg(unix)]
pub fn prune(reach: &Reach, probed: &Probed, version: &str) -> Result<Vec<String>, Refused> {
    let unusable = |why: String| Refused::Unusable {
        host: reach.route.host().to_owned(),
        why,
    };
    if !a_plain_path(&probed.config) {
        return Err(unusable(format!(
            "purlis's folder on that computer would be {}, which purlis can't prune: it must be \
             a full path with no spaces or quotes",
            crate::shown::short(&probed.config)
        )));
    }
    if !a_version(version) {
        return Err(unusable(format!(
            "{} isn't a version purlis installs",
            crate::shown::short(version)
        )));
    }
    let ran = run_there(
        reach,
        prune::prune_command(&probed.config, version),
        None,
        4 * 1024,
        sshroute::RUN_WITHIN,
    )?;
    ran_ok(reach, ran).map(|printed| prune::removed_from(&printed))
}

/// What [`prune`] removed once `verified` is in place: a prune that fails leaves the old
/// versions for the next install, and never fails the install.
fn pruned_after(reach: &Reach, probed: &Probed, verified: &Verified) -> Vec<String> {
    #[cfg(unix)]
    {
        prune(reach, probed, verified.version()).unwrap_or_default()
    }
    #[cfg(not(unix))]
    {
        let _ = (reach, probed, verified);
        Vec::new()
    }
}

/// The temporary file `incoming` filled: downloaded there from `url`, or uploaded from `input`.
fn deliver(
    reach: &Reach,
    incoming: &str,
    url: Option<&str>,
    input: Option<&[u8]>,
) -> Result<(), Refused> {
    let command = match url {
        Some(url) => fetch_command(incoming, url).map_err(|why| Refused::Unusable {
            host: reach.route.host().to_owned(),
            why,
        })?,
        None => receive_command(incoming),
    };
    let ran = run_there(reach, command, input, 4 * 1024, DELIVER_WITHIN)?;
    ran_ok(reach, ran).map(|_| ())
}

/// `incoming` deleted there, as well as can be: a failure leaves a file no one runs, which the
/// next install's temporary name never meets and the next probe deletes once it is a day old.
fn discard(reach: &Reach, incoming: &str) {
    let _ = run_there(
        reach,
        discard_command(incoming),
        None,
        1024,
        sshroute::RUN_WITHIN,
    );
}

/// **The version a binary at `exe` is installed as, under `config_root`'s machine store**: the
/// name of its folder, where `exe` is `<config>/server/<version>/purlis` (both resolved through
/// links). `None` for a binary anywhere else, as the desktop's own and a runner image's are.
#[must_use]
pub fn version_of_binary(config_root: &Path, exe: &Path) -> Option<String> {
    let exe = std::fs::canonicalize(exe).ok()?;
    if exe.file_name()? != BINARY {
        return None;
    }
    let folder = exe.parent()?;
    let version = folder.file_name()?.to_str()?;
    let server = std::fs::canonicalize(crate::machine::dir(config_root).join(SERVER_DIR)).ok()?;
    (a_version(version) && folder.parent()? == server).then(|| version.to_owned())
}

/// **The version this binary is installed as**, under the machine store this process resolves
/// ([`version_of_binary`]); `None` for a binary that is not installed in `<config>/server/`.
#[must_use]
pub fn this_binarys_version() -> Option<String> {
    let config = crate::machine::config_root_unheld()?;
    version_of_binary(&config, &std::env::current_exe().ok()?)
}

/// **The folder `server/<version>` inside the session host's folder**: what a host of an
/// installed version keeps as its own (its credentials, kill switch and host record), beside
/// another version's, so neither ends or reads the other's ([`crate::machine::host_folder`]).
#[must_use]
pub fn host_folder_of(daemon_folder: &Path, version: &str) -> PathBuf {
    daemon_folder.join(SERVER_DIR).join(version)
}

#[cfg(unix)]
pub mod prune;

#[cfg(test)]
mod tests;
