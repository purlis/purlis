// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Regenerate: the pin's command writes this person's data files** (ADR 0120 §6, #5962).
//!
//! - **What is approved is the code the command resolves to, never its text** ([`resolve`]).
//!   The command is one program and its words, with no shell between: the first word that
//!   names a file from the pin's working folder is its script, and the script's code is its
//!   skill's folder (the nearest folder above it, inside the plane, holding a `SKILL.md`), or
//!   else the script's own folder. Every path and byte of that folder is fingerprinted
//!   ([`fingerprint`]), so an edit to the script, or to anything beside it that it could load,
//!   asks again even when the command reads the same ([`Standing::Changed`]). A command naming
//!   no script is refused: a program found on `PATH` alone would be approved by its name. The
//!   program word itself (`node`, `python3`) is part of the command, so of the pin's id: a new
//!   one is a new pin ([`Standing::PinChanged`]).
//! - **Each person's, on their own machine** ([`RECORD`], Machine tier), keyed as the page's
//!   approval is ([`super::approval::key`]): a fork of the workspace, or the plane cloned
//!   elsewhere, asks again. Re-read at the press ([`standing`]), never carried from an earlier
//!   read.
//! - **It never runs in the app** (ADR 0120 §6): this module plans and finishes a run and
//!   starts nothing. The command runs in the session host, in the sandbox a chat's commands
//!   run in, and writes into a folder of its own ([`OUT_ENV`]); nothing it writes elsewhere in
//!   the artifact is read.
//! - **It writes only the declared data files, whole or not at all** ([`place`]): only the
//!   names the page declares ([`super::data::declared`]) are taken from the run's folder, each
//!   checked as a page's read would check it (JSON, under [`super::data::MOST_BYTES`], no
//!   link), and only once every one passes is each written beside its place and renamed over
//!   it. A run that failed, or wrote nothing usable, leaves the data as it was, and the bar
//!   says when that data was last good ([`last_good`]).
//! - **Nothing reads the program's output to decide anything**: the run's exit status and the
//!   files in its folder are all that is asked. Its error output is only shown, masked
//!   ([`shown_errors`]).

use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::data::Declared;
use super::pins::Pin;

/// The record of this machine's approvals of what pins' commands resolve to, in purlis's
/// folder of the config home ([`crate::machine::dir`]).
pub const RECORD: &str = "artifact-regenerate-approvals.json";

/// The variable naming the folder a run writes its data files into, by their declared names.
pub const OUT_ENV: &str = "PURLIS_ARTIFACT_OUT";

/// The file whose folder is a skill's.
const SKILL_FILE: &str = "SKILL.md";

/// The most entries a command's code folder may hold to be fingerprinted.
const MOST_ENTRIES: usize = 4096;

/// The most bytes a command's code folder may hold to be fingerprinted.
const MOST_CODE_BYTES: u64 = 64 << 20;

/// The most the record may be.
const MOST_RECORD_BYTES: u64 = 1 << 20;

/// The most of a run's error output the bar shows: its last lines, up to this many bytes.
pub const MOST_ERROR_BYTES: usize = 4096;

/// Words that only a shell reads, which a command run with no shell would pass on as words.
const SHELL_WORDS: [&str; 10] = ["|", "||", "&&", ";", "&", ">", ">>", "<", "<<", "2>"];

/// **Whether Regenerate runs on this platform**: only where a chat's commands run sandboxed.
/// Not on Windows, where they do not, so it is refused rather than run unconfined.
pub const RUNS_HERE: bool = cfg!(not(windows));

/// What a pin's command resolves to: what runs, where, and the code a person approves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The program, as the command names it.
    pub program: String,
    /// Its words after the program.
    pub args: Vec<String>,
    /// Where it starts: the pin's working folder, inside the workspace.
    pub cwd: PathBuf,
    /// The script the command runs.
    pub script: PathBuf,
    /// The folder approved: the script's skill's, or its own.
    pub code: PathBuf,
    /// The fingerprint of that folder ([`fingerprint`]).
    pub fingerprint: String,
}

/// **What pin `pin` of the workspace at `workspace`, in the plane at `root`, runs**, or the
/// sentence saying why it runs nothing.
pub fn resolve(root: &Path, workspace: &Path, pin: &Pin) -> Result<Resolved, String> {
    if !RUNS_HERE {
        return Err(
            "Regenerate runs in the sandbox a chat's commands run in, and this platform has \
             none, so nothing was run"
                .into(),
        );
    }
    let Some(command) = pin.command.as_deref().filter(|c| !c.trim().is_empty()) else {
        return Err(format!(
            "'{}' names no command that writes its data; pin it again with --command",
            crate::personas::one_line(&pin.folder)
        ));
    };
    let words = shell_words::split(command)
        .map_err(|_| format!("its command `{}` has an unclosed quote", one_line(command)))?;
    if let Some(word) = words
        .iter()
        .find(|w| SHELL_WORDS.contains(&w.as_str()) || w.starts_with('>'))
    {
        return Err(format!(
            "its command runs one program with no shell, so `{word}` would reach it as a word; \
             put pipes and redirections in the script it runs"
        ));
    }
    let Some((program, args)) = words.split_first() else {
        return Err("its command is empty".into());
    };
    let workspace = workspace
        .canonicalize()
        .map_err(|e| format!("the workspace is not there ({})", os(&e)))?;
    let cwd = working_folder(&workspace, pin.cwd.as_deref())?;
    let script = words
        .iter()
        .filter(|w| !w.starts_with('-'))
        .find_map(|w| script_at(&cwd, w))
        .ok_or_else(|| {
            format!(
                "Regenerate approves the script a command runs, and `{}` names none from its \
                 working folder; name it by its path, as in `node scripts/report.mjs`",
                one_line(command)
            )
        })?;
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let code = code_folder(&root, &script);
    let fingerprint = fingerprint(&code)?;
    Ok(Resolved {
        program: program.clone(),
        args: args.to_vec(),
        cwd,
        script,
        code,
        fingerprint,
    })
}

/// The pin's working folder, inside the workspace once every link is followed.
fn working_folder(workspace: &Path, cwd: Option<&str>) -> Result<PathBuf, String> {
    let at = workspace.join(cwd.unwrap_or("."));
    let real = at.canonicalize().map_err(|e| {
        format!(
            "its working folder '{}' is not there ({})",
            one_line(cwd.unwrap_or(".")),
            os(&e)
        )
    })?;
    if !real.starts_with(workspace) || !real.is_dir() {
        return Err(format!(
            "its working folder '{}' is not a folder inside the workspace",
            one_line(cwd.unwrap_or("."))
        ));
    }
    Ok(real)
}

/// The file `word` names from `cwd`, where it names one, once every link is followed.
fn script_at(cwd: &Path, word: &str) -> Option<PathBuf> {
    if word.is_empty() || word.contains('\0') {
        return None;
    }
    let real = cwd.join(word).canonicalize().ok()?;
    real.is_file().then_some(real)
}

/// The folder a script's approval covers: the nearest one above it, inside the plane, holding
/// a `SKILL.md`; else its own.
fn code_folder(root: &Path, script: &Path) -> PathBuf {
    let own = script.parent().unwrap_or(script).to_path_buf();
    if script.starts_with(root) {
        for above in own.ancestors() {
            if !above.starts_with(root) {
                break;
            }
            if above.join(SKILL_FILE).is_file() {
                return above.to_path_buf();
            }
        }
    }
    own
}

/// **The fingerprint of code folder `code`**: SHA-256 over its own path and every entry below
/// it, each a length-framed part named by its kind, its executable bit and its path, holding
/// its bytes (a link: where it points, never followed). The parts go in sorted by path, so the
/// order the file system lists them in changes nothing; a file added, taken away, renamed or
/// edited changes it. A folder over [`MOST_ENTRIES`] or [`MOST_CODE_BYTES`] is refused.
pub fn fingerprint(code: &Path) -> Result<String, String> {
    let mut parts: Vec<(String, PathBuf)> = Vec::new();
    let mut bytes = 0u64;
    walk(code, code, &mut parts, &mut bytes)?;
    parts.sort();
    let mut hash = Sha256::new();
    let mut frame = |name: &str, body: &[u8]| {
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update((body.len() as u64).to_be_bytes());
        hash.update(body);
    };
    frame("purlis-regenerate-code", b"1");
    frame("at", code.to_string_lossy().as_bytes());
    for (name, path) in &parts {
        let body = match name.as_bytes().first() {
            Some(b'f') => std::fs::read(path).map_err(|e| unreadable(code, &e))?,
            Some(b'l') => std::fs::read_link(path)
                .map_err(|e| unreadable(code, &e))?
                .to_string_lossy()
                .into_owned()
                .into_bytes(),
            _ => Vec::new(),
        };
        frame(name, &body);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn walk(
    code: &Path,
    dir: &Path,
    parts: &mut Vec<(String, PathBuf)>,
    bytes: &mut u64,
) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| unreadable(code, &e))? {
        let entry = entry.map_err(|e| unreadable(code, &e))?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|e| unreadable(code, &e))?;
        let rel = path
            .strip_prefix(code)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let kind = if meta.file_type().is_symlink() {
            'l'
        } else if meta.is_dir() {
            'd'
        } else {
            'f'
        };
        parts.push((format!("{kind}{} {rel}", executable(&meta)), path.clone()));
        if parts.len() > MOST_ENTRIES {
            return Err(too_big(code));
        }
        match kind {
            'd' => walk(code, &path, parts, bytes)?,
            'f' => {
                *bytes = bytes.saturating_add(meta.len());
                if *bytes > MOST_CODE_BYTES {
                    return Err(too_big(code));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(unix)]
fn executable(meta: &std::fs::Metadata) -> char {
    use std::os::unix::fs::PermissionsExt as _;
    if meta.permissions().mode() & 0o111 != 0 {
        'x'
    } else {
        '-'
    }
}

#[cfg(not(unix))]
fn executable(_meta: &std::fs::Metadata) -> char {
    '-'
}

fn too_big(code: &Path) -> String {
    format!(
        "the code its command runs, {}, is over the {} entries or {} Regenerate fingerprints",
        code.display(),
        MOST_ENTRIES,
        crate::browserfiles::bytes_said(MOST_CODE_BYTES)
    )
}

fn unreadable(code: &Path, e: &std::io::Error) -> String {
    format!(
        "could not read the code its command runs, {} ({})",
        code.display(),
        os(e)
    )
}

/// Where a pin's command stands with this machine's person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    /// Approved here, this pin and this code: it runs.
    Approved,
    /// Never approved here: asks.
    New,
    /// Approved here, and the code it resolves to changed since: asks again.
    Changed,
    /// Approved here under another pin, before its folder, command or working folder changed:
    /// asks again.
    PinChanged,
}

impl Standing {
    /// Whether a press runs it without asking.
    pub fn runs(self) -> bool {
        self == Self::Approved
    }

    /// Its name, as the command line and the window spell it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::New => "new",
            Self::Changed => "changed",
            Self::PinChanged => "pin-changed",
        }
    }
}

fn record(config_root: &Path) -> PathBuf {
    crate::machine::dir(config_root).join(RECORD)
}

/// Every approval the record holds; nothing when it cannot be read.
fn entries(config_root: &Path) -> serde_json::Map<String, Value> {
    let at = record(config_root);
    let Ok(file) = crate::contain::open_no_link(config_root, &at) else {
        return serde_json::Map::new();
    };
    let plain = file
        .metadata()
        .is_ok_and(|meta| meta.is_file() && meta.len() <= MOST_RECORD_BYTES);
    let mut text = String::new();
    if !plain
        || file
            .take(MOST_RECORD_BYTES)
            .read_to_string(&mut text)
            .is_err()
    {
        return serde_json::Map::new();
    }
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(mut doc)) => match doc.remove("approved") {
            Some(Value::Object(approved)) => approved,
            _ => serde_json::Map::new(),
        },
        _ => serde_json::Map::new(),
    }
}

/// **Where pin `pin_id` of folder `folder` stands**, resolved as `resolved`, with the person
/// whose config home is `config_root` (none: no record, so it asks).
pub fn standing(
    config_root: Option<&Path>,
    workspace: &Path,
    folder: &str,
    pin_id: &str,
    resolved: &Resolved,
) -> Standing {
    let Some(config_root) = config_root else {
        return Standing::New;
    };
    let entries = entries(config_root);
    let Some(entry) = entries.get(&super::approval::key(workspace, folder)) else {
        return Standing::New;
    };
    let text = |key: &str| entry.get(key).and_then(Value::as_str);
    match (text("pin"), text("fingerprint")) {
        (Some(pin), _) if pin != pin_id => Standing::PinChanged,
        (Some(_), Some(print)) if print == resolved.fingerprint => Standing::Approved,
        (Some(_), Some(_)) => Standing::Changed,
        _ => Standing::New,
    }
}

/// **Records the person's approval** of what pin `pin_id`'s command resolved to when they were
/// shown it: `fingerprint` is the one shown, never taken again here, so the code approved is
/// the code they saw. Replaces any approval of the folder before it. Only purlis's bar and a
/// person's own terminal call this.
pub fn approve(
    config_root: &Path,
    workspace: &Path,
    folder: &str,
    pin_id: &str,
    resolved: &Resolved,
) -> Result<(), String> {
    let real = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let mut entries = entries(config_root);
    entries.insert(
        super::approval::key(workspace, folder),
        json!({
            "workspace": real.to_string_lossy(),
            "folder": folder,
            "pin": pin_id,
            "code": resolved.code.to_string_lossy(),
            "fingerprint": resolved.fingerprint,
        }),
    );
    let doc = json!({ "version": 1, "approved": Value::Object(entries) });
    let words = |e: std::io::Error| format!("purlis could not keep the approval ({})", os(&e));
    crate::machine::private_dir(config_root).map_err(words)?;
    let text = crate::pyjson::dumps_indent2(&doc) + "\n";
    crate::machine::write_beside(config_root, &record(config_root), text.as_bytes()).map_err(words)
}

/// What [`place`] wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    /// The declared names written, in their declared order.
    pub written: Vec<String>,
}

/// **Puts a finished run's data in place**: from `out`, the folder the run wrote into, each
/// file `declared` names is checked, and only once every one there passes is each written
/// whole into artifact folder `folder` (beside its place, then renamed over it). Anything else
/// the run wrote is never read. A run that wrote none of them, or one that is not JSON, too
/// large or a link, changes nothing, and the sentence says which.
pub fn place(folder: &Path, declared: &Declared, out: &Path) -> Result<Placed, String> {
    if declared.data.is_empty() {
        return Err("its page declares no data files, so Regenerate has nothing to write".into());
    }
    let mut ready: Vec<(&str, Vec<u8>)> = Vec::new();
    for name in &declared.data {
        let staged = path_of(out, name);
        let bytes = match crate::contain::read_no_link(out, &staged) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(format!("the run's {name} is a link or not a plain file")),
        };
        if bytes.len() as u64 > super::data::MOST_BYTES {
            return Err(format!(
                "the run's {name} is {}, over the {} a data file may be",
                crate::browserfiles::bytes_said(bytes.len() as u64),
                crate::browserfiles::bytes_said(super::data::MOST_BYTES)
            ));
        }
        if serde_json::from_slice::<Value>(&bytes).is_err() {
            return Err(format!("the run's {name} is not JSON"));
        }
        ready.push((name, bytes));
    }
    if ready.is_empty() {
        return Err(format!(
            "the command finished but wrote none of the data files its page declares ({}) \
             into ${OUT_ENV}",
            declared.data.join(", ")
        ));
    }
    for (name, bytes) in &ready {
        let target = path_of(folder, name);
        if let Some(parent) = target.parent() {
            crate::contain::no_link_on_the_way(folder, parent)
                .and_then(|()| std::fs::create_dir_all(parent))
                .map_err(|e| format!("could not write {name} ({})", os(&e)))?;
        }
        crate::rewrite::replace(folder, &target, bytes, crate::rewrite::Mode::Kept)
            .map_err(|e| format!("could not write {name} ({})", os(&e)))?;
    }
    Ok(Placed {
        written: ready.into_iter().map(|(name, _)| name.to_owned()).collect(),
    })
}

/// **When the data now in place was last good**: the newest write time of the declared data
/// files there, or none when none is.
pub fn last_good(folder: &Path, declared: &Declared) -> Option<chrono::DateTime<chrono::Utc>> {
    declared
        .data
        .iter()
        .filter_map(|name| std::fs::symlink_metadata(path_of(folder, name)).ok())
        .filter(std::fs::Metadata::is_file)
        .filter_map(|meta| meta.modified().ok())
        .max()
        .map(chrono::DateTime::<chrono::Utc>::from)
}

/// The sentence the bar shows for a run that failed: what happened, and that the data before it
/// stands, with its date.
pub fn failed_said(why: &str, last_good: Option<chrono::DateTime<chrono::Utc>>) -> String {
    match last_good {
        Some(at) => format!(
            "Regenerate failed: {why}. The data shown is unchanged; last good: {}",
            at.format("%Y-%m-%d %H:%M UTC")
        ),
        None => format!("Regenerate failed: {why}. Nothing was generated on this machine yet"),
    }
}

/// **A run's error output as the bar shows it**: its last [`MOST_ERROR_BYTES`], each secret
/// shape in it masked as a tool's output is. Only shown; nothing decides on it.
pub fn shown_errors(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let mut start = text.len().saturating_sub(MOST_ERROR_BYTES);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let tail = &text[start..];
    let mut shown = String::with_capacity(tail.len());
    let mut at = 0;
    for (span, _) in crate::secretshape::leak_spans_whole_and_by_line(tail) {
        if span.start < at {
            continue;
        }
        shown.push_str(&tail[at..span.start]);
        shown.push_str(&crate::secretshape::masked(&tail[span.clone()]));
        at = span.end;
    }
    shown.push_str(&tail[at..]);
    shown
}

/// Where data file `name` is inside `folder`.
fn path_of(folder: &Path, name: &str) -> PathBuf {
    name.split('/')
        .fold(folder.to_path_buf(), |at, s| at.join(s))
}

fn one_line(text: &str) -> String {
    crate::personas::one_line(text)
}

fn os(e: &std::io::Error) -> String {
    crate::rewrite::os_words(e)
}

#[cfg(all(test, unix))]
mod tests;
