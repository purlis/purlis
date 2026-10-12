// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Get a project by its URL** (#1815, spec #1810): the welcome's "Get a project from your
//! forge" door. The person pastes a project's URL and picks a folder; purlis clones the project
//! there, and the window then opens it through the same open-and-trust question as every other
//! open (ADR 0035). Nothing here opens or trusts anything.
//!
//! - **The URL** is HTTPS or SSH, in either SSH spelling ([`ProjectUrl::parse`]). Anything else
//!   is refused before git runs, with a sentence that says what to paste instead.
//! - **The folder** defaults to `~/Projects/<name>` ([`folder_for`]); the window remembers the
//!   last place the person picked.
//! - **The clone** shows its progress ([`Progress`]) and can be called off at any moment.
//! - **Nothing is left behind** when it is called off or fails: every folder this run made is
//!   removed, and a folder that was there and empty is emptied again ([`get`]).
//! - **Each failure has its own sentence** ([`Why`]): a bad URL, a missing project, no access,
//!   no network, a folder already in use. The window offers Retry beside each.
//!
//! **Whose credentials.** The clone is the person's own git, run for them: their credential
//! helpers and their SSH, until purlis has a credential of its own for the window's git
//! (#752). It runs through the runner's [`git::spawn_persons_clone`], which keeps everything
//! else the runner holds, and never prompts: a sign-in git would have asked for is a "no access"
//! sentence instead. No chat reaches this module; its only caller is a command of the window's
//! alone.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crate::worktree::git;

/// Where a project goes when the person has not picked a place yet: `~/Projects` (ruling Q7).
pub const DEFAULT_PARENT: &str = "Projects";

/// How a project's URL reaches its forge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// `https://host/owner/name`.
    Https,
    /// `ssh://user@host/owner/name`, or the short `user@host:owner/name`.
    Ssh,
    /// A local stand-in for a forge, in a test or a scenario build only (`file://…`).
    #[cfg(any(test, feature = "fenced"))]
    StandIn,
}

/// A project's URL, checked: one git can clone over HTTPS or SSH, with a name to give its folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectUrl {
    text: String,
    host: String,
    name: String,
    transport: Transport,
}

impl ProjectUrl {
    /// `text` as a project URL, or the sentence that says what to paste instead.
    ///
    /// Accepted: `https://host[:port]/path`, `ssh://[user@]host[:port]/path` and
    /// `[user@]host:path`. Refused: every other scheme (`http://` sends everything in the clear,
    /// `git://` is unauthenticated, `file://` and `ext::` are not a forge), a URL that holds a
    /// password or a token, or any user name over HTTPS (it would be written into the clone's
    /// config), and anything git or ssh could read as an option.
    pub fn parse(text: &str) -> Result<Self, Refused> {
        let text = text.trim();
        if text.is_empty() {
            return Err(Refused::bad_url("Paste the project's URL first."));
        }
        if text.chars().any(|c| c.is_whitespace() || c.is_control()) || text.starts_with('-') {
            return Err(Refused::bad_url(NOT_A_PROJECT_URL));
        }
        let (transport, rest) = if let Some(rest) = strip_scheme(text, "https://") {
            (Transport::Https, rest)
        } else if let Some(rest) = strip_scheme(text, "ssh://") {
            (Transport::Ssh, rest)
        } else if text.contains("://") || text.contains("::") {
            return Err(Refused::bad_url(ONLY_HTTPS_OR_SSH));
        } else {
            return Self::scp_like(text);
        };
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let (user, host_port) = match authority.rsplit_once('@') {
            Some((user, host)) => (Some(user), host),
            None => (None, authority),
        };
        if user.is_some_and(|user| user.contains(':')) {
            return Err(Refused::bad_url(
                "That URL holds a password or a token. Paste it without one: the clone uses your \
                 own git sign-in.",
            ));
        }
        // An HTTPS user name is where a token goes when it is pasted as the whole sign-in
        // (`https://<token>@host/…`), and git would keep it in the clone's config, where a chat
        // in that project reads it. A forge's HTTPS URL never needs one, so none is taken.
        if transport == Transport::Https && user.is_some() {
            return Err(Refused::bad_url(
                "That URL holds a user name or a token. Paste it without one, like \
                 https://github.com/owner/project: the clone uses your own git sign-in.",
            ));
        }
        let host = host_port.split(':').next().unwrap_or_default();
        Self::checked(text, host, path, transport)
    }

    /// `[user@]host:path`, git's short SSH spelling.
    fn scp_like(text: &str) -> Result<Self, Refused> {
        let Some((authority, path)) = text.split_once(':') else {
            return Err(Refused::bad_url(NOT_A_PROJECT_URL));
        };
        // A colon after a slash is a local path to git, not a host.
        if authority.contains('/') {
            return Err(Refused::bad_url(NOT_A_PROJECT_URL));
        }
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        Self::checked(text, host, path, Transport::Ssh)
    }

    /// The URL, once its host and its last path part are known to be usable.
    fn checked(text: &str, host: &str, path: &str, transport: Transport) -> Result<Self, Refused> {
        if host.is_empty() || host.starts_with('-') {
            return Err(Refused::bad_url(NOT_A_PROJECT_URL));
        }
        let name = name_in(path).ok_or_else(|| {
            Refused::bad_url(
                "That URL names no project. Paste the whole address, with the owner and the \
                 project's name.",
            )
        })?;
        Ok(Self {
            text: text.to_owned(),
            host: host.to_owned(),
            name,
            transport,
        })
    }

    /// A local repository standing in for a forge: a test's, or a scenario build's.
    #[cfg(any(test, feature = "fenced"))]
    pub fn stand_in(path: &Path) -> Self {
        let name = path
            .file_name()
            .and_then(|name| name_in(&name.to_string_lossy()))
            .unwrap_or_else(|| "project".to_owned());
        Self {
            text: format!("file://{}", path.display()),
            host: "this machine".to_owned(),
            name,
            transport: Transport::StandIn,
        }
    }

    /// The URL as pasted, trimmed.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The forge's host, as the sentences name it.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The project's name: the URL's last part, without `.git`. Its folder's default name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How the URL reaches its forge.
    pub fn transport(&self) -> Transport {
        self.transport
    }
}

/// What a URL that is not one says.
const NOT_A_PROJECT_URL: &str = "That is not a project URL. Paste one like \
     https://github.com/owner/project or git@github.com:owner/project.git.";

/// What a URL of another scheme says.
const ONLY_HTTPS_OR_SSH: &str = "purlis gets a project over HTTPS or SSH. Paste a URL like \
     https://github.com/owner/project or git@github.com:owner/project.git.";

/// `text` without `scheme`, matched without regard to case.
fn strip_scheme<'a>(text: &'a str, scheme: &str) -> Option<&'a str> {
    let head = text.get(..scheme.len())?;
    head.eq_ignore_ascii_case(scheme)
        .then(|| &text[scheme.len()..])
}

/// The name a path's last part gives a folder: without a trailing `/` or `.git`, and never one
/// that is no name at all (`.`, `..`) or that holds a separator.
fn name_in(path: &str) -> Option<String> {
    let last = path.trim_end_matches('/').rsplit('/').next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    let usable = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.starts_with('-')
        && !name.contains(['\\', ':', '\0']);
    usable.then(|| name.to_owned())
}

/// The folder a project goes into: `<parent>/<name>`, where `parent` is the place the person
/// picked last, or `~/Projects` when they never picked one.
pub fn folder_for(url: &ProjectUrl, parent: Option<&Path>, home: &Path) -> PathBuf {
    parent
        .map_or_else(|| home.join(DEFAULT_PARENT), Path::to_path_buf)
        .join(url.name())
}

/// The person's home folder, which [`folder_for`]'s default is in; `None` on a machine that
/// does not say.
pub fn home() -> Option<PathBuf> {
    dirs::home_dir()
}

/// Which part of a clone is running, in the person's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Reaching the forge, and the forge getting the project ready.
    Connecting,
    /// The project's history coming down.
    Downloading,
    /// That history being unpacked.
    Unpacking,
    /// The project's files written into the folder.
    WritingFiles,
}

impl Stage {
    /// What the window says while this stage runs.
    pub fn words(self) -> &'static str {
        match self {
            Self::Connecting => "Connecting",
            Self::Downloading => "Downloading",
            Self::Unpacking => "Unpacking",
            Self::WritingFiles => "Writing files",
        }
    }
}

/// How far a clone has got: its stage, and the share of that stage done when git says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub stage: Stage,
    pub percent: Option<u8>,
}

/// One line of git's `--progress` output as [`Progress`], or `None` for a line that is not
/// progress (and may say why a clone failed).
pub fn read_progress(line: &str) -> Option<Progress> {
    let line = line.trim();
    // The forge's own preparation comes as `remote: …`, and is part of connecting.
    let said = line.strip_prefix("remote: ").unwrap_or(line);
    let (what, rest) = said.split_once(':')?;
    let stage = match what {
        "Enumerating objects" | "Counting objects" | "Compressing objects" => Stage::Connecting,
        "Receiving objects" => Stage::Downloading,
        "Resolving deltas" => Stage::Unpacking,
        "Updating files" | "Checking out files" => Stage::WritingFiles,
        _ => return None,
    };
    let percent = rest.split_once('%').and_then(|(number, _)| {
        number
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|number| *number <= 100)
    });
    Some(Progress { stage, percent })
}

/// Why a project was not got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// The URL is not one purlis clones.
    BadUrl,
    /// The forge has no project there (or hides a private one from this sign-in).
    Missing,
    /// The forge did not let this sign-in in.
    NoAccess,
    /// The forge could not be reached.
    Unreachable,
    /// The folder is already there and holds something.
    FolderTaken,
    /// Anything else git said.
    Git,
}

/// A project not got: why, and the sentence the window says.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{sentence}")]
pub struct Refused {
    pub why: Why,
    pub sentence: String,
}

impl Refused {
    fn bad_url(sentence: &str) -> Self {
        Self {
            why: Why::BadUrl,
            sentence: sentence.to_owned(),
        }
    }

    fn folder(sentence: String) -> Self {
        Self {
            why: Why::FolderTaken,
            sentence,
        }
    }
}

/// What came of a clone that was not refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Got {
    /// The project is in this folder, ready to open.
    Cloned(PathBuf),
    /// The person called it off, and nothing it made is left.
    CalledOff,
}

/// Why git's own words say a clone failed, as one of the window's sentences.
///
/// git's words are matched lower-cased; `LC_ALL=C` keeps them in English. The no-access words
/// come first: a forge that says "not found" to a sign-in it refuses says so as well.
pub fn read_failure(url: &ProjectUrl, said: &[String]) -> Refused {
    let all = said.join("\n").to_lowercase();
    let any = |words: &[&str]| words.iter().any(|word| all.contains(word));
    let host = url.host();
    let shown = url.as_str();
    let (why, sentence) = if any(&[
        "permission denied",
        "authentication failed",
        "could not read username",
        "could not read password",
        "terminal prompts disabled",
        "host key verification failed",
        "access denied",
        "returned error: 401",
        "returned error: 403",
    ]) {
        let sentence = match url.transport() {
            Transport::Ssh => format!(
                "{host} did not accept your SSH key for {shown}. Check that your key is loaded \
                 and that {host} is a known host (try `ssh -T` to it in a terminal), or paste \
                 the HTTPS URL. Then Retry."
            ),
            _ => format!(
                "{host} did not let you in to {shown}. Sign in to {host} with git on this \
                 machine, or paste the SSH URL, then Retry."
            ),
        };
        (Why::NoAccess, sentence)
    } else if any(&[
        "repository not found",
        "could not be found",
        "does not appear to be a git repository",
        "returned error: 404",
        "does not exist",
    ]) || all.contains("repository '") && all.contains("' not found")
    {
        (
            Why::Missing,
            format!(
                "There is no project at {shown}. Check the owner and the name. A private project \
                 you cannot reach looks missing too."
            ),
        )
    } else if any(&[
        "could not resolve host",
        "could not resolve hostname",
        "connection refused",
        "connection timed out",
        "operation timed out",
        "network is unreachable",
        "failed to connect",
        "no route to host",
    ]) {
        (
            Why::Unreachable,
            format!("purlis could not reach {host}. Check your connection, then Retry."),
        )
    } else {
        let last = said
            .iter()
            .rev()
            .map(|line| line.trim())
            .find(|line| !line.is_empty())
            .map(|line| line.strip_prefix("fatal: ").unwrap_or(line))
            .unwrap_or("it stopped without saying why");
        (Why::Git, format!("git could not get {shown}: {last}"))
    };
    Refused { why, sentence }
}

/// How often a running clone is checked for the person's stop.
const LISTEN: Duration = Duration::from_millis(50);

/// The most lines of git's own words kept to say why a clone failed.
const KEPT_LINES: usize = 40;

/// Clones `url` into `into`, telling `progress` how far it has got, and stopping as soon as
/// `called_off` is raised.
///
/// `into` is absolute, and is either not there yet or an empty folder. Folders above it that
/// are not there yet are made. When the clone is called off or fails, every folder this call
/// made is removed and an empty folder that was there is emptied again, so a Retry starts clean.
///
/// There is no deadline: the person watches the clone and can call it off, and a large project
/// on a slow line takes as long as it takes.
pub fn get(
    url: &ProjectUrl,
    into: &Path,
    called_off: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Got, Refused> {
    let made = Made::prepare(into)?;
    progress(Progress {
        stage: Stage::Connecting,
        percent: None,
    });
    let answer = cloned(url, into, called_off, progress);
    match answer {
        Ok(Got::Cloned(_)) => {}
        Ok(Got::CalledOff) | Err(_) => made.undo(into),
    }
    answer
}

/// The clone itself, with nothing undone.
fn cloned(
    url: &ProjectUrl,
    into: &Path,
    called_off: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Got, Refused> {
    let parent = into.parent().unwrap_or(into);
    let dest = into.display().to_string();
    let args = [
        "clone",
        "--progress",
        "--no-recurse-submodules",
        "--",
        url.as_str(),
        dest.as_str(),
    ];
    let child =
        match git::spawn_persons_clone(parent, &args, &no_prompt_ssh(parent), &["SSH_AUTH_SOCK"]) {
            Ok(Ok(child)) => child,
            Ok(Err(why)) => return Err(read_failure(url, &[why])),
            Err(unavailable) => {
                return Err(Refused {
                    why: Why::Git,
                    sentence: format!("purlis could not start git: {unavailable}"),
                });
            }
        };
    follow(url, into, child, called_off, progress)
}

/// `core.sshCommand` asking ssh never to prompt, unless the person's own git config (global or
/// system) names an ssh command of its own (a key agent's, a jump host's), which is theirs to
/// keep. A prompt from ssh would go to a terminal nobody watches. Only those two files are read:
/// a repository around the folder the project goes into is not the person's to speak for.
fn no_prompt_ssh(dir: &Path) -> Vec<String> {
    let named = ["--global", "--system"].iter().any(|scope| {
        git::run(
            dir,
            &["config", scope, "--get", "core.sshCommand"],
            git::READ,
        )
        .is_ok_and(|run| run.ok() && !run.out.trim().is_empty())
    });
    if named {
        Vec::new()
    } else {
        vec!["core.sshCommand=ssh -o BatchMode=yes".to_owned()]
    }
}

/// Reads a running clone to its end: its progress to `progress`, its other words kept to say
/// why it failed, and the child stopped the moment `called_off` is raised.
fn follow(
    url: &ProjectUrl,
    into: &Path,
    mut child: std::process::Child,
    called_off: &AtomicBool,
    progress: &mut dyn FnMut(Progress),
) -> Result<Got, Refused> {
    let (lines, heard) = mpsc::channel::<String>();
    if let Some(mut stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let _ = std::io::copy(&mut stdout, &mut std::io::sink());
        });
    }
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            // git rewrites a progress line in place with `\r`, so either ends a line.
            let mut line = Vec::new();
            for byte in std::io::BufReader::new(stderr).bytes() {
                let Ok(byte) = byte else { break };
                if byte != b'\r' && byte != b'\n' {
                    line.push(byte);
                    continue;
                }
                if !line.is_empty() {
                    if lines
                        .send(String::from_utf8_lossy(&line).into_owned())
                        .is_err()
                    {
                        return;
                    }
                    line.clear();
                }
            }
            if !line.is_empty() {
                let _ = lines.send(String::from_utf8_lossy(&line).into_owned());
            }
        });
    }
    let mut said: Vec<String> = Vec::new();
    let mut last: Option<Progress> = None;
    loop {
        if called_off.load(Ordering::SeqCst) {
            git::stop(&mut child);
            return Ok(Got::CalledOff);
        }
        match heard.recv_timeout(LISTEN) {
            Ok(line) => match read_progress(&line) {
                Some(now) if last != Some(now) => {
                    last = Some(now);
                    progress(now);
                }
                Some(_) => {}
                None => {
                    said.push(line);
                    if said.len() > KEPT_LINES {
                        said.remove(0);
                    }
                }
            },
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let status = loop {
        if called_off.load(Ordering::SeqCst) {
            git::stop(&mut child);
            return Ok(Got::CalledOff);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(LISTEN),
            Err(err) => {
                return Err(Refused {
                    why: Why::Git,
                    sentence: format!("purlis lost track of git while it cloned: {err}"),
                });
            }
        }
    };
    if status.success() {
        Ok(Got::Cloned(into.to_path_buf()))
    } else {
        Err(read_failure(url, &said))
    }
}

/// What a clone made before git ran, so a failed or called-off one can take it away again.
struct Made {
    /// The highest folder this run made, which holds everything else it made; `None` when the
    /// destination itself was there already.
    top: Option<PathBuf>,
}

impl Made {
    /// Checks `into` is a place a project can go, and makes the folders above it.
    fn prepare(into: &Path) -> Result<Self, Refused> {
        if !into.is_absolute() {
            return Err(Refused::folder(format!(
                "{} is not a full path. Choose the folder with Choose….",
                into.display()
            )));
        }
        match std::fs::symlink_metadata(into) {
            Ok(meta) => {
                let empty = meta.is_dir()
                    && std::fs::read_dir(into).is_ok_and(|mut entries| entries.next().is_none());
                if !empty {
                    return Err(Refused::folder(format!(
                        "{} is already there and is not an empty folder. Choose another \
                         folder, or move that one away, then Retry.",
                        into.display()
                    )));
                }
                Ok(Self { top: None })
            }
            Err(_) => {
                let top = into
                    .ancestors()
                    .take_while(|folder| std::fs::symlink_metadata(folder).is_err())
                    .last()
                    .unwrap_or(into)
                    .to_path_buf();
                if let Some(parent) = into.parent() {
                    std::fs::create_dir_all(parent).map_err(|err| {
                        Refused::folder(format!(
                            "purlis could not make {}: {err}. Choose another folder, then Retry.",
                            parent.display()
                        ))
                    })?;
                }
                Ok(Self { top: Some(top) })
            }
        }
    }

    /// Takes away what this run made: the folders it made, or what is now inside the empty
    /// folder that was there.
    fn undo(&self, into: &Path) {
        match &self.top {
            Some(top) => {
                let _ = std::fs::remove_dir_all(top);
            }
            None => {
                if let Ok(entries) = std::fs::read_dir(into) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let _ = if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                            std::fs::remove_dir_all(&path)
                        } else {
                            std::fs::remove_file(&path)
                        };
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
