// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What leaves this machine when a skill is exported or published** (SK-18, #2781; spec #2762
//! decision 44), and the export.
//!
//! [`gather`] reads a skill and the project's skills it refers to (decision 57), through no link,
//! and answers every file that would go, every file that stays behind and why, and every
//! [`Stop`]: what must be fixed before anything goes. A review shows that answer; [`Outgoing::digest`]
//! ties what was reviewed to what goes.
//!
//! **Left behind, and said**: a link, a file by a credential's name ([`secret_name`]), a file the
//! plane's git ignores, and a name purlis would not install from a source.
//! **A stop, for both**: anything credential-shaped in any file, binary ones read as text.
//! **A stop for a publish alone**: personal data, a home folder's path, and a skill that is this
//! machine's alone. An export stays with the person, who may want those, so it goes on saying them
//! nowhere. No stop has an override: the fix is in the skill.
//!
//! Grades never go here: results never sit in a skill's folder (decision 21).
//!
//! [`secret_name`]: crate::reposave::secret_name

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::Path;

use purlis_skills::tree;
use sha2::{Digest as _, Sha256};

use super::fetch::MOST_SKILL_BYTES;
use super::install::level_parts;
use super::write::{self, HeldFile};
use crate::secretshape;
use crate::skills::grades;
use crate::skills::graph;
use crate::skills::verbs::Level;
use crate::worktree::git;

/// The most files one skill sends out.
const MOST_FILES: usize = 2_000;

/// What the files are going out for: a stop's rules differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Export,
    Publish,
}

/// One file that goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutFile {
    pub parts: Vec<String>,
    pub executable: bool,
    pub bytes: Vec<u8>,
}

impl OutFile {
    /// Its path inside the skill's folder.
    pub fn path(&self) -> String {
        self.parts.join("/")
    }
}

/// One file that stays behind, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Behind {
    pub path: String,
    pub why: &'static str,
}

/// One skill that goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutSkill {
    pub name: String,
    pub level: Level,
    pub files: Vec<OutFile>,
    pub left: Vec<Behind>,
}

/// **What must be fixed before anything goes.** It never holds the value it found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    Found {
        skill: String,
        path: String,
        line: usize,
        kind: String,
        masked: String,
    },
    Local {
        skill: String,
        level: String,
    },
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stop::Found {
                skill,
                path,
                line,
                kind,
                masked,
            } => write!(f, "{skill}/{path}:{line} holds {kind} ({masked})"),
            Stop::Local { skill, level } => write!(
                f,
                "{skill} is at {level}, this machine's alone. Move it to a shared level first"
            ),
        }
    }
}

/// **Everything a review shows.** See the module's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    /// The skill named first, then each it refers to.
    pub skills: Vec<OutSkill>,
    pub stops: Vec<Stop>,
    /// The references to a skill that is not there, by name.
    pub broken: Vec<String>,
}

impl Outgoing {
    /// **A digest of exactly what goes**: each skill's name and each file's path, mode and bytes,
    /// each length-prefixed so no two different sets read the same.
    pub fn digest(&self) -> String {
        let mut hash = Sha256::new();
        let mut field = |bytes: &[u8]| {
            hash.update((bytes.len() as u64).to_be_bytes());
            hash.update(bytes);
        };
        for skill in &self.skills {
            field(skill.name.as_bytes());
            for file in &skill.files {
                field(file.path().as_bytes());
                field(&[u8::from(file.executable)]);
                field(&file.bytes);
            }
        }
        crate::runnerlink::hex_of(&hash.finalize())
    }

    fn clear(&self) -> Result<(), String> {
        match self.stops.first() {
            None => Ok(()),
            Some(first) => Err(format!(
                "Nothing was written, as {} of the files must be fixed first: {first}",
                self.stops.len()
            )),
        }
    }
}

/// **The skill `name` at `level`, and the project's skills it refers to, as they would go.**
pub fn gather(
    plane: &Path,
    built_in: Option<&Path>,
    level: &Level,
    name: &str,
    purpose: Purpose,
) -> Result<Outgoing, String> {
    let mut out = Outgoing {
        skills: Vec::new(),
        stops: Vec::new(),
        broken: Vec::new(),
    };
    let mut seen = BTreeSet::from([name.to_owned()]);
    let mut waiting = vec![(level.clone(), name.to_owned())];
    while let Some((level, name)) = waiting.pop() {
        let skill = one_skill(plane, &level, &name)?;
        if purpose == Purpose::Publish && grades::is_local(plane, &level) {
            out.stops.push(Stop::Local {
                skill: name.clone(),
                level: level.to_string(),
            });
        }
        for file in &skill.files {
            stops_in(&name, file, purpose, &mut out.stops);
        }
        let text = skill_text(&skill);
        out.skills.push(skill);
        for used in graph::uses_of_copy(plane, built_in, &name, &text).uses {
            if used.broken {
                if !out.broken.contains(&used.name) {
                    out.broken.push(used.name);
                }
            } else if used.project && seen.insert(used.name.clone()) {
                // The nearest copy, as the skill's own level sees it first.
                let found = Level::copies_of(plane, &used.name)
                    .map(|(at, _)| at)
                    .find(|at| *at == level)
                    .or_else(|| Level::copies_of(plane, &used.name).map(|(at, _)| at).next());
                match found {
                    Some(at) => waiting.push((at, used.name)),
                    None => out.broken.push(used.name),
                }
            }
        }
        waiting.sort_by(|a, b| b.1.cmp(&a.1));
    }
    out.broken.sort();
    Ok(out)
}

/// The `SKILL.md` of the copy that goes.
fn skill_text(skill: &OutSkill) -> String {
    skill
        .files
        .iter()
        .find(|file| file.path() == "SKILL.md")
        .map(|file| String::from_utf8_lossy(&file.bytes).into_owned())
        .unwrap_or_default()
}

/// One skill's files, read through no link, and what stays behind.
fn one_skill(plane: &Path, level: &Level, name: &str) -> Result<OutSkill, String> {
    let dir = level
        .named(plane, name)
        .ok_or_else(|| format!("{level} holds no skill named {name}"))?;
    let mut folder = level_parts(level)?;
    folder.push(name.to_owned());
    let held = write::read_all(plane, &folder, MOST_SKILL_BYTES, MOST_FILES).ok_or_else(|| {
        format!("{name} could not be read whole: a file of it is too large or unreadable")
    })?;
    let ignored = ignored_of(plane, &folder, &held)?;
    let mut left = links_in(&dir);
    let mut files = Vec::new();
    for HeldFile {
        parts,
        executable,
        bytes,
    } in held
    {
        let path = parts.join("/");
        let last = parts.last().map_or("", String::as_str);
        let why = if crate::reposave::secret_name(last).is_some() {
            Some("its name is a credential file's")
        } else if ignored.contains(&path) {
            Some("the project's git ignores it, so it is this machine's")
        } else if tree::folder_parts(&path).is_none() {
            Some("purlis would not install a file of this name from a source")
        } else {
            None
        };
        match why {
            Some(why) => left.push(Behind { path, why }),
            None => files.push(OutFile {
                parts,
                executable,
                bytes,
            }),
        }
    }
    left.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(OutSkill {
        name: name.to_owned(),
        level: level.clone(),
        files,
        left,
    })
}

/// Each link under `dir`: a read follows none, so each is said here.
fn links_in(dir: &Path) -> Vec<Behind> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
            } else if !kind.is_file()
                && let Ok(path) = entry.path().strip_prefix(dir)
            {
                out.push(Behind {
                    path: path.to_string_lossy().into_owned(),
                    why: "it is a link or not a plain file",
                });
            }
        }
    }
    out
}

/// The paths of `held` the plane's git ignores: none where the plane is no repository. Any other
/// failure of git is an error, since a file it would have named must not go out.
fn ignored_of(
    plane: &Path,
    folder: &[String],
    held: &[HeldFile],
) -> Result<BTreeSet<String>, String> {
    let prefix = folder.join("/");
    let mut args = vec![
        "check-ignore".to_owned(),
        "-z".to_owned(),
        "--no-index".to_owned(),
        "--".to_owned(),
    ];
    args.extend(
        held.iter()
            .map(|file| format!("{prefix}/{}", file.parts.join("/"))),
    );
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let run = git::run(plane, &args, git::READ).map_err(|why| why.to_string())?;
    ignored_from(&run, &prefix)
}

/// What `git check-ignore -z` answered: 0 some are ignored, 1 none are.
fn ignored_from(run: &git::Run, prefix: &str) -> Result<BTreeSet<String>, String> {
    match run.code {
        Some(0 | 1) => Ok(run
            .out
            .split('\0')
            .filter_map(|line| line.strip_prefix(prefix)?.strip_prefix('/'))
            .map(str::to_owned)
            .collect()),
        _ if run.err.contains("not a git repository") => Ok(BTreeSet::new()),
        _ => Err(format!(
            "purlis could not ask git which files of the skill it ignores, so nothing goes: {}",
            run.err.trim()
        )),
    }
}

/// A home folder's path, which works on one machine alone.
fn home_path(line: &str) -> Option<std::ops::Range<usize>> {
    ["/Users/", "/home/", "C:\\Users\\"]
        .iter()
        .find_map(|root| {
            let start = line.find(root)?;
            let rest = &line[start + root.len()..];
            let name = rest
                .find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == '.'))
                .unwrap_or(rest.len());
            (name > 0).then(|| start..start + root.len() + name)
        })
}

/// Every stop `file` holds. Bytes that are not text are read as text too, since a token in a
/// binary file is still a token.
fn stops_in(skill: &str, file: &OutFile, purpose: Purpose, stops: &mut Vec<Stop>) {
    let text = String::from_utf8_lossy(&file.bytes);
    let mut stop = |line: usize, kind: &str, value: &str| {
        stops.push(Stop::Found {
            skill: skill.to_owned(),
            path: file.path(),
            line,
            kind: kind.to_owned(),
            masked: secretshape::masked(value),
        });
    };
    // A file's own name goes out too.
    let path = file.path();
    for leak in secretshape::leaks(&path) {
        if purpose == Purpose::Publish || !secretshape::is_personal(leak.rule) {
            stop(0, leak.kind, &path[leak.span.clone()]);
        }
    }
    let mut any = false;
    for (index, line) in text.lines().enumerate() {
        for leak in secretshape::leaks(line) {
            if purpose == Purpose::Publish || !secretshape::is_personal(leak.rule) {
                any = true;
                stop(index + 1, leak.kind, &line[leak.span.clone()]);
            }
        }
        if purpose == Purpose::Publish
            && let Some(span) = home_path(line)
        {
            any = true;
            stop(index + 1, "a home folder's path", &line[span]);
        }
    }
    // What runs over lines, a private key's block among them, is read whole.
    if !any && let Some(found) = secretshape::found(&text) {
        stop(1, found.kind, "");
    }
}

/// **The files, as a folder of skill folders at `to`**, which must not exist yet. Each file is
/// made new, so nothing is written through a link or over anything. What was made is taken back
/// where a later file fails.
pub fn export_folder(out: &Outgoing, to: &Path) -> Result<(), String> {
    out.clear()?;
    std::fs::create_dir(to).map_err(|why| match why.kind() {
        std::io::ErrorKind::AlreadyExists => format!("{} is already there", to.display()),
        _ => format!("{} could not be made: {why}", to.display()),
    })?;
    let wrote = || -> std::io::Result<()> {
        for skill in &out.skills {
            for file in &skill.files {
                let mut path = to.join(&skill.name);
                path.extend(&file.parts);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                new_file(&path, file.executable)?.write_all(&file.bytes)?;
            }
        }
        Ok(())
    }();
    wrote.map_err(|why| {
        let _ = std::fs::remove_dir_all(to);
        format!("{} could not be written: {why}", to.display())
    })
}

/// **The files, as a zip at `to`**, which must not exist yet, with a fixed time on each entry, so
/// the same files are always the same bytes.
pub fn export_zip(out: &Outgoing, to: &Path) -> Result<(), String> {
    out.clear()?;
    let failed = |why: &dyn std::fmt::Display| format!("the zip could not be written: {why}");
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for skill in &out.skills {
        for file in &skill.files {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .last_modified_time(zip::DateTime::default())
                .unix_permissions(if file.executable { 0o755 } else { 0o644 });
            writer
                .start_file(format!("{}/{}", skill.name, file.path()), options)
                .map_err(|why| failed(&why))?;
            writer.write_all(&file.bytes).map_err(|why| failed(&why))?;
        }
    }
    let bytes = writer.finish().map_err(|why| failed(&why))?.into_inner();
    let mut made = new_file(to, false).map_err(|why| match why.kind() {
        std::io::ErrorKind::AlreadyExists => format!("{} is already there", to.display()),
        _ => format!("{} could not be made: {why}", to.display()),
    })?;
    made.write_all(&bytes).map_err(|why| {
        let _ = std::fs::remove_file(to);
        failed(&why)
    })
}

/// A file made new, never one that is there, a link included.
fn new_file(path: &Path, executable: bool) -> std::io::Result<std::fs::File> {
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        open.mode(if executable { 0o755 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = executable;
    open.open(path)
}

#[cfg(test)]
mod tests;
