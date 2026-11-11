// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Regenerate: the pin's command writes this person's data files** (ADR 0122 §6, #5962).
//!
//! - **What is approved is the code the command resolves to** ([`resolve`]), not only its text.
//!   The command is one program and its words, with no shell between: the first word that
//!   names a file inside the plane, from the pin's working folder, is its script, and the
//!   script's code is its skill's folder (the nearest folder above it, inside the plane,
//!   holding a `SKILL.md`), or else the script's own folder. That folder is accepted only
//!   strictly below a workspace's root or as a skill's own folder ([`allowed`], #6120), and
//!   every path and byte of it is fingerprinted ([`fingerprint`]), so an edit to the script, or to anything beside it that it could load,
//!   asks again even when the command reads the same ([`Standing::Changed`]). A command naming
//!   no script is refused: a program found on `PATH` alone would be approved by its name.
//! - **And every word of the command** ([`of_run`]): the program (`node`, `python3`) and each
//!   word after it, those passed over to find the script (`-e '<code>'`) among them, go into
//!   what is approved with the code's fingerprint. The command is also part of the pin's id, so
//!   a new one is a new pin ([`Standing::PinChanged`]); covering the words as well means a
//!   command changed under an approved id still asks again ([`Standing::Changed`]).
//! - **Each person's, on their own machine** ([`RECORD`], Machine tier), keyed as the page's
//!   approval is ([`super::approval::key`]): a fork of the workspace, or the plane cloned
//!   elsewhere, asks again. Re-read at the press ([`standing`]), never carried from an earlier
//!   read.
//! - **It never runs in the app** (ADR 0122 §6): this module plans and finishes a run and
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

/// What a press of Regenerate is told where it does not run ([`RUNS_HERE`]).
pub const NOT_HERE: &str = "Regenerate runs in the sandbox a chat's commands run in, and this \
     platform has none, so nothing was run";

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
    /// The artifact's own folder: with `code`, all a run reads of the plane ([`launch`]).
    pub artifact: PathBuf,
    /// What is approved: every word of the command and the fingerprint of that folder
    /// ([`of_run`], [`fingerprint`]).
    pub fingerprint: String,
}

/// **What pin `pin` of the workspace at `workspace`, in the plane at `root`, runs**, or the
/// sentence saying why it runs nothing.
pub fn resolve(root: &Path, workspace: &Path, pin: &Pin) -> Result<Resolved, String> {
    if !RUNS_HERE {
        return Err(NOT_HERE.into());
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
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let script = words
        .iter()
        .filter(|w| !w.starts_with('-'))
        .find_map(|w| script_at(&root, &cwd, w))
        .ok_or_else(|| {
            format!(
                "Regenerate approves the script a command runs, and `{}` names none from its \
                 working folder; name it by its path, as in `node scripts/report.mjs`",
                one_line(command)
            )
        })?;
    let code = code_folder(&root, &script);
    if !allowed(&root, &code) {
        return Err(format!(
            "its script's code folder would be '{}', and Regenerate approves only code below a \
             workspace's root or in a skill's folder, so nothing was run; move the script into \
             a folder of the workspace, such as `scripts/`",
            one_line(&code.display().to_string())
        ));
    }
    let fingerprint = of_run(&words, &fingerprint(&code)?);
    let artifact = workspace.join(super::DIR).join(&pin.folder);
    let artifact = artifact.canonicalize().unwrap_or(artifact);
    Ok(Resolved {
        program: program.clone(),
        args: args.to_vec(),
        cwd,
        script,
        code,
        artifact,
        fingerprint,
    })
}

/// **What a person approves of a run**: SHA-256 over every word of the command, the program
/// first, each framed by its length, and then the fingerprint of the code it runs. A word
/// [`resolve`] passes over to find the script (`-e '<code>'`, `--require=<file>`) is still
/// covered, so a command that gained one asks again even under an approved pin's id.
fn of_run(words: &[String], code: &str) -> String {
    let mut hash = Sha256::new();
    let mut frame = |name: &str, body: &[u8]| {
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update((body.len() as u64).to_be_bytes());
        hash.update(body);
    };
    frame("purlis-regenerate-run", b"1");
    for word in words {
        frame("word", word.as_bytes());
    }
    frame("code", code.as_bytes());
    hash.finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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

/// The file `word` names from `cwd`, where it names one inside the plane at `root`, once every
/// link is followed. A file outside the plane (an interpreter such as `/bin/sh`, or a system
/// file) is never the script.
fn script_at(root: &Path, cwd: &Path, word: &str) -> Option<PathBuf> {
    if word.is_empty() || word.contains('\0') {
        return None;
    }
    let real = cwd.join(word).canonicalize().ok()?;
    (real.is_file() && real.starts_with(root)).then_some(real)
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

/// The names a skill level's folder has (`docs/plane-format.md`, "workspace and persona
/// skills"): its committed skills and this machine's own.
const SKILL_LEVELS: [&str; 2] = ["skills", "skills.local"];

/// **Whether `code` may be a code folder** (#6120, the operator's ruling of 2026-11-11): an
/// allowlist, so a shape nobody thought of is refused. Only a folder strictly below a
/// workspace's root (`workspaces/<ws>/…`, never `<ws>` itself), or a skill's own folder where a
/// level keeps its skills (`skills[.local]/<skill>`, `personas/<p>/skills[.local]/<skill>`,
/// holding a `SKILL.md`). The plane's root, `workspaces/`, `personas/`, `skills/` and every
/// other folder of the plane are refused, as their approval would let a run read all of them.
fn allowed(root: &Path, code: &Path) -> bool {
    let Ok(rel) = code.strip_prefix(root) else {
        return false;
    };
    let parts: Vec<&str> = rel
        .components()
        .map(|part| part.as_os_str().to_str().unwrap_or(""))
        .collect();
    let a_skill = |level: &str| SKILL_LEVELS.contains(&level) && code.join(SKILL_FILE).is_file();
    match parts.as_slice() {
        [first, ws, _, ..] if *first == crate::sandbox::WORKSPACES => {
            crate::workspaces::is_workspace_folder(&root.join(first).join(ws))
        }
        [level, skill] => !skill.starts_with('.') && a_skill(level),
        ["personas", persona, level, skill] => {
            !persona.starts_with('.') && !skill.starts_with('.') && a_skill(level)
        }
        _ => false,
    }
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
        // An id that is not 128 bits (one recorded before ids were) never stands approved.
        (Some(pin), _) if pin != pin_id || !super::approval::is_pin_id(pin_id) => {
            Standing::PinChanged
        }
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

/// The variable naming the file a run's output, both streams, is written to: only shown.
pub const LOG_ENV: &str = "PURLIS_ARTIFACT_LOG";

/// How long a run may take before purlis stops it.
pub const MOST_SECONDS: u64 = 300;

/// The folders and file of one run, all inside its scratch folder: the only place it writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scratch {
    /// Where it writes its data files, by their declared names ([`OUT_ENV`]).
    pub out: PathBuf,
    /// Its own temporary folder.
    pub tmp: PathBuf,
    /// Its output ([`LOG_ENV`]).
    pub log: PathBuf,
}

impl Scratch {
    /// The scratch layout under `scratch`, made: `out/`, `tmp/` and the log's place.
    pub fn made(scratch: &Path) -> Result<Self, String> {
        let real = scratch
            .canonicalize()
            .map_err(|e| format!("purlis could not make the run's folder ({})", os(&e)))?;
        let made = Self {
            out: real.join("out"),
            tmp: real.join("tmp"),
            log: real.join("run.log"),
        };
        for dir in [&made.out, &made.tmp] {
            std::fs::create_dir(dir)
                .map_err(|e| format!("purlis could not make the run's folder ({})", os(&e)))?;
        }
        Ok(made)
    }
}

/// What the window hands the session host to start: the command wrapped in its sandbox.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    /// The pin's working folder: read, never written.
    pub cwd: PathBuf,
    /// Variables set for it.
    pub set: Vec<(String, String)>,
}

/// The fixed script the wrapped line runs: the command's own words, passed as its arguments and
/// never read by a shell as text, with both its streams written to [`LOG_ENV`].
const RUNS: &str = "exec \"$@\" >\"$PURLIS_ARTIFACT_LOG\" 2>&1";

/// **The run of `resolved`, wrapped in its sandbox** (ADR 0122 §6, amended 2026-11-09): the
/// profile a chat's commands run in (Seatbelt on macOS, purlis's own wrap on Linux, from
/// [`crate::sandbox::seatbelt::profile`]) with this plane's denials, **no network** (no proxy
/// port and no socket is named in it), **writes to `scratch` alone**: its out folder, its
/// temporary folder and its log, and **reads in the plane only its approved code and its
/// artifact's folder** ([`plane_reads`], amended 2026-11-10). On Linux the wrap is purlis's own program, checked as a chat's
/// is ([`crate::sandbox::program::checked_wrap`]).
pub fn launch(root: &Path, resolved: &Resolved, scratch: &Scratch) -> Result<Launch, String> {
    let machine = crate::sandbox::Machine::this();
    let purlis = match machine.os {
        crate::sandbox::Os::Linux => {
            let writes = scratch.out.parent().unwrap_or(&scratch.out).to_path_buf();
            let wrap = crate::sandbox::program::checked_wrap(
                root,
                crate::sandbox::program::Chat {
                    cwd: &writes,
                    writable: std::slice::from_ref(&scratch.tmp),
                    env: &[],
                },
            )
            .map_err(|why| format!("purlis could not start its sandbox ({why})"))?;
            Some(wrap)
        }
        _ => None,
    };
    launch_on(root, resolved, scratch, purlis.as_deref(), &machine)
}

fn launch_on(
    root: &Path,
    resolved: &Resolved,
    scratch: &Scratch,
    purlis: Option<&Path>,
    machine: &crate::sandbox::Machine,
) -> Result<Launch, String> {
    if !RUNS_HERE {
        return Err("Regenerate runs in a sandbox this platform does not have".into());
    }
    let denied = crate::sandbox::Denied::of(root, machine);
    if denied.unread.is_some() || denied.untold.is_some() {
        return Err(
            "purlis could not tell everything a run must be kept from reading, so nothing was \
             run"
            .into(),
        );
    }
    let writes = scratch
        .out
        .parent()
        .ok_or("the run's folder has no place")?
        .to_path_buf();
    let own = crate::sandbox::seatbelt::Own {
        deny: plane_reads(root, resolved)
            .map_err(|why| format!("purlis could not write the run's sandbox ({why})"))?,
        ..crate::sandbox::seatbelt::Own::default()
    };
    let profile =
        crate::sandbox::seatbelt::profile(&denied.paths, &own, &writes, &scratch.tmp, &[], None)
            .map_err(|why| format!("purlis could not write the run's sandbox ({why})"))?;
    let rest = [
        vec![
            "-c".to_owned(),
            RUNS.to_owned(),
            "purlis-regenerate".to_owned(),
            resolved.program.clone(),
        ],
        resolved.args.clone(),
    ]
    .concat();
    let (program, args) =
        crate::sandbox::wrapper::line(machine.os, purlis, profile, "/bin/sh".to_owned(), rest)
            .map_err(str::to_owned)?;
    let path = |p: &Path| p.display().to_string();
    Ok(Launch {
        program,
        args,
        cwd: resolved.cwd.clone(),
        set: vec![
            (OUT_ENV.to_owned(), path(&scratch.out)),
            (LOG_ENV.to_owned(), path(&scratch.log)),
            ("TMPDIR".to_owned(), path(&scratch.tmp)),
        ],
    })
}

/// **A run reads nothing of the plane but its approved code and its artifact's folder** (ADR
/// 0122, amended 2026-11-10): one rule per name of the plane's root denying a file's contents,
/// and a folder's entries, everywhere below it but [`Resolved::code`] and
/// [`Resolved::artifact`]. So a script that imports `../lib/x.mjs`, a `node_modules` above its
/// folder, or a second file its command names elsewhere in the plane, loads none of them: only
/// code the approval fingerprinted runs, and no other file of the plane is copied into the data.
/// What a file is (its metadata) stays readable, so resolving a path through the plane's
/// folders works; the working folder's own entries are listed, never its files read. Nothing
/// is given back: a denial class inside the code still holds, as it is never named here.
///
/// **Nor does a run start a program there, link a file of it, or read a file's extended
/// attributes** (#6120): Seatbelt judges a start and a link by rules of their own, so a native
/// program elsewhere in the plane would otherwise run unapproved. Linux's supervisor judges
/// every open by this same rule ([`purlis_linux_sandbox::profile`]); a start there is the
/// kernel's own rule, which takes nothing back, and an attribute's read is set aside, so those
/// two hold on macOS alone.
fn plane_reads(root: &Path, resolved: &Resolved) -> Result<Vec<String>, &'static str> {
    use crate::sandbox::seatbelt::kernel_strings;
    let mut but = Vec::new();
    for open in [&resolved.code, &resolved.artifact] {
        for name in kernel_strings(open)? {
            but.push(format!("(require-not (subpath {name}))"));
        }
    }
    for name in kernel_strings(&resolved.cwd)? {
        but.push(format!("(require-not (literal {name}))"));
    }
    let but = but.join(" ");
    Ok(kernel_strings(root)?
        .into_iter()
        .flat_map(|name| {
            [
                format!(
                    "(deny file-read-data process-exec file-link (require-all (subpath {name}) {but}))"
                ),
                format!("(deny file-read-xattr (require-all (subpath {name}) {but}))"),
            ]
        })
        .collect())
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
