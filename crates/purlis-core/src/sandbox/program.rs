// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Whether the program a sandboxed chat starts is the harness its sandbox was compiled for, and
//! is not a file a sandboxed chat could have written (ruling V87g, and the V73d analog for a
//! profile's program).
//!
//! A harness's own sandbox, like Claude Code's `--settings` sandbox object, binds only the
//! harness that reads it: a profile of Claude Code's kind whose program is something else runs
//! with no sandbox at all, and still shows as sandboxed. And a program inside the plane, such as a
//! wrapper script, is code charter runs later outside any sandbox, which a sandboxed chat could
//! have changed. So, before a sandboxed start:
//!
//! - **Never a program where the chat can write**, for every harness: the plane, the chat's own
//!   folder, the temp directories and whatever else the compiled sandbox lets it write. Asked of
//!   the path as written and of its real path, so a link in a writable place, to a program
//!   outside it, is refused too: the chat could rewrite the link. A relative path is refused,
//!   since it would be found against a folder the chat writes. The real path is what then runs,
//!   for the check and for the chat alike. Refused, not denied writing: a wrapper is the
//!   operator's to keep where they like, and the refusal names where it is.
//! - **Nor a command that names a file there** (D-88g). Every other word of the command is held
//!   to the same places, wherever in the word a path begins: an interpreter outside, handed a
//!   script a chat wrote, runs the chat's code. A word that names no file is left alone. This
//!   is a lint that catches mistakes (D-88k). Where charter wraps the whole harness (opencode
//!   and Codex, on macOS), the wrap is the boundary; Claude Code is not wrapped, because
//!   Seatbelt cannot apply its own sandbox inside charter's (#1150).
//! - **Claude Code answers as Claude Code.** Its program and command, asked `--version`, must
//!   answer a line `X.Y.Z (Claude Code)`. A legitimate wrapper outside the plane passes its words
//!   through and answers the same. It is asked with the chat's own environment, in the chat's
//!   folder.
//!
//! **What the answer proves** is that the profile is Claude Code by intent, not by mistake. A
//! program written to answer like Claude Code passes; the profile's program is one a person
//! approved (ADR 0022), and that approval is what this rests on. So does inline code, inline config, and a
//! program outside that reads and runs a file a chat can write without being handed it.

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use super::NotStarted;
use crate::harness::Harness;

/// How long a program has to answer `--version`. Generous: a program the system has not seen
/// before is assessed on its first run, and a cold start under `sandbox-exec` was measured past
/// five seconds; a program that does not answer in time is told apart from one that answers
/// wrong ([`NotStarted::ProbeTimedOut`]).
pub const PATIENCE: Duration = Duration::from_secs(10);

/// Why a program gave no answer to `--version` charter could read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unanswered {
    /// It did not finish answering within [`PATIENCE`].
    TimedOut,
    /// It could not be run, or it failed.
    Failed,
}

/// The longest command word a sandboxed start checks; a longer one is refused (D-88k).
pub const WORD_MAX: usize = 4096;

/// The most of an answer that is read.
const ANSWER_MAX: u64 = 4096;

/// What answers `--version` for a program and its command, in place of running it: a test's.
pub type Probe<'a> = dyn Fn(&[String]) -> Option<String> + Sync + 'a;

/// Where a sandboxed chat starts, as the check asks it: the folder it runs in, the folders
/// its sandbox lets it write besides (the temp directories among them, [`temp_roots`]), and
/// its own environment.
#[derive(Debug, Clone, Copy)]
pub struct Chat<'a> {
    pub cwd: &'a Path,
    pub writable: &'a [PathBuf],
    pub env: &'a [(String, String)],
}

/// The temp directories every harness's sandbox may let a chat write, `env` being the chat's.
/// A caller adds these to [`Chat::writable`].
pub fn temp_roots(env: &[(String, String)]) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = ["/tmp", "/private/tmp", "/var/tmp", "/private/var/tmp"]
        .into_iter()
        .map(PathBuf::from)
        .collect();
    roots.push(std::env::temp_dir());
    roots.extend(
        env.iter()
            .filter(|(key, _)| key == "TMPDIR")
            .map(|(_, value)| PathBuf::from(value)),
    );
    roots
}

/// The program `words` start, made the one real file that runs, or why a chat sandboxed for
/// `harness` in the plane at `root` may not start on it. `probe` answers `--version` in place
/// of running it; `None` runs it ([`version_of`]).
pub fn checked(
    harness: Harness,
    words: &[String],
    root: &Path,
    chat: Chat<'_>,
    probe: Option<&Probe<'_>>,
) -> Result<Vec<String>, NotStarted> {
    checked_answering(harness, words, root, chat, probe).map(|(words, _)| words)
}

/// [`checked`], with what the program answered `--version`, for a harness it is asked of
/// (Claude Code): what the start reads its version from ([`super::Applied::answered`], #1665).
pub fn checked_answering(
    harness: Harness,
    words: &[String],
    root: &Path,
    chat: Chat<'_>,
    probe: Option<&Probe<'_>>,
) -> Result<(Vec<String>, Option<String>), NotStarted> {
    let Some(written) = words.first() else {
        return Err(NotStarted::ProgramRelative);
    };
    let written = PathBuf::from(written);
    if !written.is_absolute() {
        return Err(NotStarted::ProgramRelative);
    }
    let real = written
        .canonicalize()
        .map_err(|_| NotStarted::ProgramWritable(written.clone()))?;
    let mut places = vec![root.to_path_buf(), chat.cwd.to_path_buf()];
    places.extend(chat.writable.iter().cloned());
    let places: Vec<(PathBuf, PathBuf)> = places
        .into_iter()
        .map(|dir| {
            let real_dir = dir.canonicalize().unwrap_or_else(|_| dir.clone());
            (dir, real_dir)
        })
        .collect();
    if let Some(path) = [&written, &real]
        .into_iter()
        .find(|path| writable(path, &places))
    {
        return Err(NotStarted::ProgramWritable(path.clone()));
    }
    // Ruling D-88g: an interpreter outside, handed a file the chat wrote, runs the chat's code.
    // So every other word is held to the same places, wherever in the word a path begins.
    // Ruling D-88k: this is a lint that catches a mistake, bounded so it stays one; charter's
    // own wrap around the harness, where charter wraps it, is the boundary.
    if words[1..].iter().any(|word| word.len() > WORD_MAX) {
        return Err(NotStarted::WordTooLong);
    }
    let mut lookups = Lookups::default();
    if let Some(word) = words[1..]
        .iter()
        .find(|word| names_a_writable_file(word, chat.cwd, &places, &mut lookups))
    {
        return Err(NotStarted::WordWritable(word.clone()));
    }
    let mut words = words.to_vec();
    words[0] = real.display().to_string();
    if harness == Harness::ClaudeCode {
        let said = match probe {
            Some(probe) => probe(&words).ok_or(Unanswered::Failed),
            // Asked once more, for twice as long, where it did not answer in time: a busy machine
            // is slow to start a program, and the start should not be refused for that.
            None => match version_of(&words, chat) {
                Err(Unanswered::TimedOut) => version_within(&words, chat, PATIENCE * 2),
                said => said,
            },
        };
        return match said {
            Ok(said) if is_claude_code(&said) => {
                heard(&said);
                Ok((words, Some(said)))
            }
            Err(Unanswered::TimedOut) => Err(NotStarted::ProbeTimedOut(harness)),
            Ok(said) => Err(NotStarted::NotTheHarness(
                harness,
                words[0].clone(),
                Some(crate::shown::one_line(&said, ANSWER_SHOWN)),
            )),
            Err(Unanswered::Failed) => {
                Err(NotStarted::NotTheHarness(harness, words[0].clone(), None))
            }
        };
    }
    Ok((words, None))
}

/// **purlis's own program, which a wrapped chat's line starts with on Linux** (#1040): this
/// running program, held as the harness's is — never started from where the chat may write
/// ([`NotStarted::ProgramWritable`]), and never one replaced while it ran
/// ([`NotStarted::WrapReplaced`]).
pub fn checked_wrap(root: &Path, chat: Chat<'_>) -> Result<PathBuf, NotStarted> {
    let this = std::env::current_exe().map_err(|_| NotStarted::WrapReplaced)?;
    checked_wrap_at(&this, root, chat)
}

/// [`checked_wrap`] for the program at `this`.
pub fn checked_wrap_at(this: &Path, root: &Path, chat: Chat<'_>) -> Result<PathBuf, NotStarted> {
    if this.to_string_lossy().ends_with(" (deleted)") {
        return Err(NotStarted::WrapReplaced);
    }
    let real = this.canonicalize().map_err(|_| NotStarted::WrapReplaced)?;
    let mut places = vec![root.to_path_buf(), chat.cwd.to_path_buf()];
    places.extend(chat.writable.iter().cloned());
    let places: Vec<(PathBuf, PathBuf)> = places
        .into_iter()
        .map(|dir| {
            let real_dir = dir.canonicalize().unwrap_or_else(|_| dir.clone());
            (dir, real_dir)
        })
        .collect();
    if let Some(path) = [this, real.as_path()]
        .into_iter()
        .find(|path| writable(path, &places))
    {
        return Err(NotStarted::ProgramWritable(path.to_path_buf()));
    }
    Ok(real)
}

/// Whether `path` lies in one of `places`, each given as written and as its real path. The one
/// test of "where a chat can write", for a harness here and for a provider's program
/// ([`crate::secrets::program`]).
pub(crate) fn writable(path: &Path, places: &[(PathBuf, PathBuf)]) -> bool {
    places
        .iter()
        .any(|(dir, real_dir)| path.starts_with(dir) || path.starts_with(real_dir))
}

/// Whether a command's word names a file where the chat can write, anywhere in it: after a
/// flag (`--config=/x`, `-I/x`), in a URL (`file:///x`), after `@`, or inside inline code or
/// inline JSON.
///
/// The word is read every way a program might read it: as written, JSON-unescaped, percent-
/// decoded, and each of those two after the other. In every reading:
///
/// - **Every part that begins at a `/`** is asked as an absolute path: as written, with its
///   `.`, `..` and `//` folded by text (as a URL's reader folds them), and resolved through the
///   folders of it that exist (as the system resolves them), so a link, a `..` after a missing
///   folder, or another letter case is seen through.
/// - **Every piece between two characters no file name holds** (whitespace, quotes, brackets,
///   `<>`, `,;:=` and the like), a short flag's attached value (`-Ilib`), and what follows such
///   a character to the word's end, is asked as a relative path against the chat's folder,
///   where it would be found, as written and folded. One that
///   names something there is refused.
///
/// A word that names nothing is no file: a flag, a model's name. This errs toward refusing: a
/// word with a writable place's path inside it, or with a piece that names a file in the chat's
/// folder, is refused even where it means something else (D-88j). It is a lint (D-88k).
fn names_a_writable_file(
    word: &str,
    cwd: &Path,
    places: &[(PathBuf, PathBuf)],
    lookups: &mut Lookups,
) -> bool {
    readings(word).iter().any(|word| {
        word.char_indices()
            .filter(|&(_, c)| c == '/')
            .any(|(at, _)| absolute_writable(Path::new(&word[at..]), places, lookups))
            || relative_parts(word).any(|part| {
                !part.is_empty()
                    && !part.starts_with('/')
                    && (cwd.join(part).symlink_metadata().is_ok()
                        || folded(&cwd.join(part)).symlink_metadata().is_ok())
            })
    })
}

/// The ways a program might read `word`: as written, JSON-unescaped, percent-decoded, and each
/// of those two after the other.
fn readings(word: &str) -> Vec<String> {
    let unescaped = json_unescaped(word);
    let decoded = percent_decoded(word);
    let mut all = vec![
        word.to_owned(),
        percent_decoded(&unescaped),
        json_unescaped(&decoded),
        unescaped,
        decoded,
    ];
    all.sort();
    all.dedup();
    all
}

/// Whether the absolute `path` lies in one of `places`: as written, folded by text, or resolved
/// by the system, before or after the folding.
fn absolute_writable(path: &Path, places: &[(PathBuf, PathBuf)], lookups: &mut Lookups) -> bool {
    let plain = folded(path);
    if writable(path, places) || writable(&plain, places) {
        return true;
    }
    let real = lookups.resolved(path);
    writable(&real, places)
        || writable(&folded(&real), places)
        || writable(&lookups.resolved(&plain), places)
}

/// `path` with `.` and `//` dropped and each `..` taking off the name before it, by text alone.
fn folded(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// What the system finds for a path, one name at a time, each answer kept: a word's parts share
/// their folders, so each folder is looked up once. The word's parts are still scanned one by
/// one, so the check grows with the square of the word's length, which `WORD_MAX` bounds.
#[derive(Default)]
struct Lookups {
    found: std::collections::HashMap<PathBuf, Option<PathBuf>>,
}

impl Lookups {
    /// The absolute `path` as the system resolves it: each name in turn, a link followed and a
    /// `..` the parent of the real folder before it, until a name that is not there. The rest is
    /// put back after it, as written: the file `path` names, or would name once made.
    fn resolved(&mut self, path: &Path) -> PathBuf {
        let mut real = PathBuf::from("/");
        let mut parts = path.components();
        while let Some(part) = parts.next() {
            match part {
                Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
                Component::ParentDir => {
                    real.pop();
                }
                Component::Normal(name) => {
                    let next = real.join(name);
                    let found = self
                        .found
                        .entry(next.clone())
                        .or_insert_with(|| {
                            next.symlink_metadata().ok()?;
                            next.canonicalize().ok()
                        })
                        .clone();
                    match found {
                        Some(found) => real = found,
                        None => {
                            real.push(name);
                            real.extend(parts);
                            return real;
                        }
                    }
                }
            }
        }
        real
    }
}

/// The parts of `word` a program could take for a relative path.
fn relative_parts(word: &str) -> impl Iterator<Item = &str> {
    let pieces = word.split(|c: char| !in_a_name(c));
    let attached = word
        .split(|c: char| !in_a_name(c))
        .filter(|piece| piece.starts_with('-') && !piece.starts_with("--"))
        .filter_map(|piece| piece.get(2..));
    let after_a_mark = word
        .char_indices()
        .filter(|&(_, c)| !in_a_name(c))
        .map(move |(at, c)| &word[at + c.len_utf8()..]);
    std::iter::once(word)
        .chain(pieces)
        .chain(attached)
        .chain(after_a_mark)
}

/// Whether `c` can be part of a file's name as programs read names off a command line.
fn in_a_name(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '_' | '~' | '+' | '-' | '/')
}

/// `word` with JSON's string escapes read: `\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t` and
/// `\uXXXX`, a surrogate pair included. Any other `\` stays as written.
fn json_unescaped(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut chars = word.chars().peekable();
    let hex4 = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>| -> Option<u32> {
        let digits: String = chars.clone().take(4).collect();
        let code = (digits.len() == 4)
            .then(|| u32::from_str_radix(&digits, 16).ok())
            .flatten()?;
        for _ in 0..4 {
            chars.next();
        }
        Some(code)
    };
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some(simple @ ('"' | '\\' | '/')) => {
                chars.next();
                out.push(simple);
            }
            Some(letter @ ('b' | 'f' | 'n' | 'r' | 't')) => {
                chars.next();
                out.push(match letter {
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    'n' => '\n',
                    'r' => '\r',
                    _ => '\t',
                });
            }
            Some('u') => {
                let mut ahead = chars.clone();
                ahead.next();
                let Some(high) = hex4(&mut ahead) else {
                    out.push(c);
                    continue;
                };
                chars = ahead;
                let code = if (0xD800..0xDC00).contains(&high) {
                    let mut pair = chars.clone();
                    let low = (pair.next() == Some('\\') && pair.next() == Some('u'))
                        .then(|| hex4(&mut pair))
                        .flatten()
                        .filter(|low| (0xDC00..0xE000).contains(low));
                    match low {
                        Some(low) => {
                            chars = pair;
                            0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
                        }
                        None => high,
                    }
                } else {
                    high
                };
                out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
            }
            _ => out.push(c),
        }
    }
    out
}

/// `word` with each `%XX` read as the byte it stands for, as a URL's reader would.
fn percent_decoded(word: &str) -> String {
    let bytes = word.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[at] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(at + 1).copied().and_then(hex),
                bytes.get(at + 2).copied().and_then(hex),
            )
        {
            out.push((high * 16 + low) as u8);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The Claude Code version this process last heard a program answer `--version` with, as
/// `X.Y.Z` ([`claude_code_version`]).
static HEARD: std::sync::Mutex<Option<(u32, u32, u32)>> = std::sync::Mutex::new(None);

/// Keep the version in `said`, which [`checked_answering`] heard Claude Code answer.
fn heard(said: &str) {
    if let Some(version) = claude_code_version(said)
        && let Ok(mut kept) = HEARD.lock()
    {
        *kept = Some(version);
    }
}

/// **The version of `harness` this process last heard it answer** (#1353): a sandboxed
/// Claude Code start, and the picker's check, ask its program `--version`; what it answered is
/// kept, in its `X.Y.Z` shape and nothing else, for a Report's draft to name. `None` for a
/// harness never asked (Codex, opencode) and before any answer.
pub fn version_heard(harness: Harness) -> Option<String> {
    if harness != Harness::ClaudeCode {
        return None;
    }
    let (major, minor, patch) = (*HEARD.lock().ok()?)?;
    Some(format!("{major}.{minor}.{patch}"))
}

/// Whether `said` holds a line Claude Code answers `--version` with: `X.Y.Z (Claude Code)`.
pub fn is_claude_code(said: &str) -> bool {
    claude_code_version(said).is_some()
}

/// The version in the first line of `said` Claude Code answers `--version` with,
/// `X.Y.Z (Claude Code)`, or `None` where no line is one.
pub fn claude_code_version(said: &str) -> Option<(u32, u32, u32)> {
    said.lines().find_map(|line| {
        let version = line.trim().strip_suffix(" (Claude Code)")?;
        let parts: Vec<&str> = version.split('.').collect();
        let [major, minor, patch] = parts[..] else {
            return None;
        };
        let number = |part: &str| {
            (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse::<u32>().ok())
                .flatten()
        };
        Some((number(major)?, number(minor)?, number(patch)?))
    })
}

/// What `words` answer to `--version`, run in `chat`'s folder with `chat`'s environment (and
/// the app's `HOME` and `PATH`, which every chat also gets), within [`PATIENCE`] and at most
/// [`ANSWER_MAX`] bytes, or why there is none.
///
/// **Inside charter's wrap** (D-88k), on macOS: the probe is the first time charter runs the
/// program, so it runs under a profile that lets it write nothing but a temp directory of its
/// own (its `TMPDIR`, removed after) and reach no network ([`super::seatbelt::probe_profile`]).
/// A command that loads a file a chat wrote gets nothing out of the probe.
///
/// The program runs in a process group of its own, and the whole group is killed when it does
/// not answer in time. A child that left the group and holds the answer's pipe open does not
/// hold the start up: the answer is read on a thread of its own, and given up on at the
/// deadline.
pub fn version_of(words: &[String], chat: Chat<'_>) -> Result<String, Unanswered> {
    version_within(words, chat, PATIENCE)
}

/// How much of a program's answer a refusal says.
const ANSWER_SHOWN: usize = 80;

/// [`version_of`], waiting `patience` for the answer.
pub fn version_within(
    words: &[String],
    chat: Chat<'_>,
    patience: Duration,
) -> Result<String, Unanswered> {
    answer_within(words, chat, patience, &mut |_| {})
}

/// [`version_of`], telling `seen` the id of the probe's process, which leads its process
/// group: a test's way to see that the whole group is gone after.
#[cfg(test)]
pub(crate) fn answer_of(
    words: &[String],
    chat: Chat<'_>,
    seen: &mut dyn FnMut(u32),
) -> Result<String, Unanswered> {
    answer_within(words, chat, PATIENCE, seen)
}

/// [`answer_of`], waiting `patience` for the answer.
fn answer_within(
    words: &[String],
    chat: Chat<'_>,
    patience: Duration,
    seen: &mut dyn FnMut(u32),
) -> Result<String, Unanswered> {
    let failed = Unanswered::Failed;
    let (program, rest) = words.split_first().ok_or(failed)?;
    // D-88k: inside charter's own wrap, which writes nothing but its own temp directory and
    // reaches no network. On macOS; on Linux it runs as before until #2343.
    let tmp = tempfile::Builder::new()
        .prefix("charter-probe-")
        .tempdir()
        .map_err(|_| failed)?;
    let mut command = if cfg!(target_os = "macos") {
        let profile = super::seatbelt::probe_profile(tmp.path()).map_err(|_| failed)?;
        let mut wrapped = std::process::Command::new(super::backend::SANDBOX_EXEC);
        wrapped.args(["-p", &profile]).arg(program);
        wrapped
    } else {
        std::process::Command::new(program)
    };
    command
        .args(rest)
        .arg("--version")
        .current_dir(chat.cwd)
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    for key in ["HOME", "PATH"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    command.envs(chat.env.iter().map(|(key, value)| (key, value)));
    command.env("TMPDIR", tmp.path());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = crate::forklock::spawn(&mut command).map_err(|_| failed)?;
    seen(child.id());
    let stdout = child.stdout.take().ok_or(failed)?;
    let (sender, answer) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.take(ANSWER_MAX).read_to_end(&mut out);
        let _ = sender.send(out);
    });
    let deadline = Instant::now() + patience;
    // Wait without reaping: while the probe is an unreaped zombie its id stays taken, so
    // the group kill below cannot reach a stranger that was handed the id afterwards.
    while !exited(&mut child) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    let in_time = exited(&mut child);
    // Its own children may still hold the answer's pipe open: the group goes too.
    #[cfg(unix)]
    {
        use rustix::process::{Pid, Signal, kill_process_group};
        let _ = kill_process_group(Pid::from_child(&child), Signal::KILL);
    }
    let _ = child.kill();
    let status = child.wait().map_err(|_| failed)?;
    if !in_time {
        return Err(Unanswered::TimedOut);
    }
    // A child that left the group and kept the pipe cannot hold the start past the deadline.
    let left = deadline.saturating_duration_since(Instant::now());
    let out = answer
        .recv_timeout(left.max(Duration::from_millis(100)))
        .map_err(|_| Unanswered::TimedOut)?;
    status
        .success()
        .then(|| String::from_utf8_lossy(&out).trim().to_owned())
        .ok_or(failed)
}

/// Whether `child` has exited, leaving it unreaped.
#[cfg(unix)]
fn exited(child: &mut std::process::Child) -> bool {
    use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
    // An error (the child is gone already) counts as exited, so the loop never spins on it.
    !matches!(
        waitid(WaitId::Pid(Pid::from_child(child)), options),
        Ok(None)
    )
}

/// Whether `child` has exited.
#[cfg(not(unix))]
fn exited(child: &mut std::process::Child) -> bool {
    !matches!(child.try_wait(), Ok(None))
}

#[cfg(all(test, unix))]
mod tests;
