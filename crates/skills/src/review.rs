// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a skill does beyond its instructions**, as the install review leads with it (SK-10,
//! #2773; ADR 0100 §3; spec #2762 decision 39): the scripts it carries and what runs each, the
//! network hosts its files name, its `` !`cmd` `` lines, and the writes it would make outside the
//! workspace, each risky one marked. Worked out from the files alone, so it lists what they name,
//! not what a script could compute when it runs: the review is a reading aid, and what bounds a
//! skill is the chat's sandbox (ADR 0067).
//!
//! **No file escapes it.** A file that is not text is listed as not shown, and is a script where
//! anything says it runs; a file the instructions run, or with a script's name or `#!` line, is a
//! script whatever its mode. **And what a reviewer reads is what the model reads**: every
//! invisible character (a Unicode tag, a zero-width character, a bidi override, any other format
//! character) is marked, and [`escaped`] shows it.
//!
//! The same reading answers an update ([`changes`]): what the summary gained and lost between the
//! pinned commit and the newer one.

use serde::Serialize;

/// One file of a skill, as the summary reads it: its path inside the skill's folder (`/` between
/// the parts), whether git marks it executable, and its bytes.
#[derive(Debug, Clone, Copy)]
pub struct FileIn<'a> {
    pub path: &'a str,
    pub executable: bool,
    pub bytes: &'a [u8],
}

impl<'a> FileIn<'a> {
    /// Its text, `None` where it is not UTF-8 or holds a NUL: a file the review does not show.
    pub fn text(&self) -> Option<&'a str> {
        std::str::from_utf8(self.bytes)
            .ok()
            .filter(|text| !text.contains('\0'))
    }
}

/// Why an item of the summary is marked risky.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Risk {
    /// It reaches the network: `curl`, `wget`, a URL.
    Network,
    /// It runs what it downloads or builds: `| sh`, `eval`.
    RunsWhatItFetches,
    /// It asks for more rights: `sudo`, `doas`.
    Elevates,
    /// It removes files: `rm -r`.
    Removes,
    /// It hides what it runs: `base64 -d`.
    Hidden,
    /// It is not text, so the review cannot show what it does.
    NotText,
}

impl std::fmt::Display for Risk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Network => "reaches the network",
            Self::RunsWhatItFetches => "runs what it downloads or builds",
            Self::Elevates => "asks for more rights",
            Self::Removes => "removes files",
            Self::Hidden => "hides what it runs",
            Self::NotText => "is not text, so it is not shown",
        })
    }
}

/// A script the skill carries, and what runs it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Script {
    pub path: String,
    /// What runs it: its `#!` line's program (`python3`), its kind by its name (`sh`), or "an
    /// executable file".
    pub runs: String,
    /// Why it is marked; empty for one that is not.
    pub risks: Vec<Risk>,
}

/// A network host the skill's files name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Host {
    pub host: String,
    /// The files that name it.
    pub files: Vec<String>,
    /// Named by a script or a `!` line, which reach it when they run; a link in the
    /// instructions is not.
    pub risky: bool,
}

/// A `` !`cmd` `` line: a command some harnesses run before the model reads the skill. Every one
/// is marked.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Command {
    pub path: String,
    /// Its line, from 1.
    pub line: usize,
    pub command: String,
}

/// A write a script or a `!` line names outside the workspace. Every one is marked.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Outside {
    pub path: String,
    /// Its line, from 1.
    pub line: usize,
    /// Where it writes, as written (`~/.zshrc`, `/etc/hosts`).
    pub target: String,
}

/// The kind of a character a reader does not see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Invisible {
    /// A Unicode tag character (U+E0000 to U+E007F), which can spell words no one sees.
    Tag,
    /// A zero-width or blank-looking character: a joiner, a variation selector, a Hangul
    /// filler.
    ZeroWidth,
    /// A bidi override or isolate, which reorders what is shown.
    Bidi,
    /// Another format or control character.
    Format,
}

impl Invisible {
    /// The kind of `c`, or `None` for a character a reader sees (and a line end or tab).
    pub fn of(c: char) -> Option<Invisible> {
        match c {
            '\u{E0000}'..='\u{E007F}' => Some(Self::Tag),
            '\u{200B}'..='\u{200D}'
            | '\u{2060}'
            | '\u{FEFF}'
            | '\u{180E}'
            | '\u{034F}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{E0100}'..='\u{E01EF}'
            | '\u{115F}'
            | '\u{1160}'
            | '\u{3164}'
            | '\u{FFA0}' => Some(Self::ZeroWidth),
            '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{061C}' => Some(Self::Bidi),
            '\n' | '\t' => None,
            '\u{0}'..='\u{1F}'
            | '\u{7F}'..='\u{9F}'
            | '\u{AD}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{2061}'..='\u{2064}'
            | '\u{206A}'..='\u{206F}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{1D173}'..='\u{1D17A}' => Some(Self::Format),
            _ => None,
        }
    }
}

impl std::fmt::Display for Invisible {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Tag => "a hidden tag character",
            Self::ZeroWidth => "a zero-width character",
            Self::Bidi => "a character that reorders the text",
            Self::Format => "an invisible control character",
        })
    }
}

/// A line that holds a character no reader sees. Every one is marked.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Unseen {
    pub path: String,
    /// Its line, from 1.
    pub line: usize,
    pub kinds: Vec<Invisible>,
}

/// **`text` as a reviewer is shown it**: every character [`Invisible::of`] names written as
/// `⟦U+XXXX⟧`, so nothing the model reads is hidden from the person, and no control character
/// reaches a terminal.
pub fn escaped(text: &str) -> String {
    marked(text, |c| Invisible::of(c).is_some())
}

/// **`text` as one line of a terminal shows it**: [`escaped`], and its line ends and tabs
/// marked too, since in a table cell a line end or a tab shears every column below it.
///
/// This is purlis's one rule for untrusted text in terminal output (SK-47): the shared padding,
/// `purlis_core::tui::pad`, applies it to every cell, so no table has to remember to.
pub fn escaped_line(text: &str) -> String {
    marked(text, |c| {
        matches!(c, '\n' | '\t') || Invisible::of(c).is_some()
    })
}

/// `text` with each character `hidden` names written as `⟦U+XXXX⟧`.
fn marked(text: &str, hidden: impl Fn(char) -> bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if hidden(c) {
            out.push_str(&format!("⟦U+{:04X}⟧", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

/// **The summary of one skill**: what it does beyond its instructions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub scripts: Vec<Script>,
    pub hosts: Vec<Host>,
    pub commands: Vec<Command>,
    pub outside: Vec<Outside>,
    pub unseen: Vec<Unseen>,
    /// Each file that is not text, so not shown.
    pub not_text: Vec<String>,
}

impl Summary {
    /// Whether anything in it is marked.
    pub fn risky(&self) -> bool {
        self.marked() > 0
    }

    /// How many of its items are marked.
    pub fn marked(&self) -> usize {
        self.scripts
            .iter()
            .filter(|one| !one.risks.is_empty())
            .count()
            + self.hosts.iter().filter(|one| one.risky).count()
            + self.commands.len()
            + self.outside.len()
            + self.unseen.len()
    }

    /// Whether it lists nothing at all: a skill of instructions alone.
    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
            && self.hosts.is_empty()
            && self.commands.is_empty()
            && self.outside.is_empty()
            && self.unseen.is_empty()
            && self.not_text.is_empty()
    }
}

/// The extensions a script is known by, and what runs it.
const BY_EXTENSION: [(&str, &str); 14] = [
    ("sh", "sh"),
    ("bash", "bash"),
    ("zsh", "zsh"),
    ("fish", "fish"),
    ("py", "python"),
    ("js", "node"),
    ("mjs", "node"),
    ("cjs", "node"),
    ("ts", "a TypeScript runner"),
    ("rb", "ruby"),
    ("pl", "perl"),
    ("ps1", "PowerShell"),
    ("bat", "cmd"),
    ("cmd", "cmd"),
];

/// Words that reach the network when a command runs.
const NETWORK_WORDS: [&str; 9] = [
    "curl", "wget", "nc", "ncat", "ssh", "scp", "rsync", "ftp", "telnet",
];

/// Programs that ask for more rights.
const ELEVATING: [&str; 3] = ["sudo", "doas", "pkexec"];

/// Programs a `| <program>` runs what came down the pipe with.
const RUNNERS: [&str; 10] = [
    "sh", "bash", "zsh", "python", "python3", "node", "ruby", "perl", "iex", "pwsh",
];

/// Programs that write to the path they are given.
const WRITERS: [&str; 10] = [
    "cp", "mv", "install", "tee", "ln", "rm", "mkdir", "touch", "chmod", "rsync",
];

/// The schemes a URL in a skill's files reaches a host with.
const SCHEMES: [&str; 7] = ["https", "http", "wss", "ws", "ftp", "ssh", "git"];

/// Words a line runs the path after: `bash scripts/x.sh`, `run scripts/x.sh`.
const RUN_WORDS: [&str; 7] = ["run", "Run", "execute", "Execute", "exec", "source", "."];

/// Whether `path` is named, in `texts`, as something that runs: after a program that runs files
/// or a word that says to run it, as `./path`, or inside a `!` line.
fn named_to_run(path: &str, texts: &[&str]) -> bool {
    let is_it = |word: &str| {
        let word = word
            .trim_matches(|c: char| "`'\"(),;:".contains(c))
            .trim_end_matches(['.', '!', '?']);
        word == path || word.ends_with(&format!("/{path}"))
    };
    texts.iter().any(|text| {
        let bang = bang_commands(text);
        bang.iter()
            .any(|(_, command)| command.split_whitespace().any(is_it))
            || text.lines().any(|line| {
                let said: Vec<&str> = line.split_whitespace().collect();
                said.iter().enumerate().any(|(at, word)| {
                    is_it(word)
                        && (word.trim_start_matches('`').starts_with("./")
                            || at.checked_sub(1).is_some_and(|before| {
                                let before = said[before].trim_matches('`');
                                let before = before.rsplit('/').next().unwrap_or(before);
                                RUNNERS.contains(&before) || RUN_WORDS.contains(&before)
                            }))
                })
            })
    })
}

/// **The summary of a skill** whose files are `files`.
pub fn summarize(files: &[FileIn<'_>]) -> Summary {
    let mut summary = Summary::default();
    let mut hosts: Vec<Host> = Vec::new();
    let texts: Vec<&str> = files.iter().filter_map(FileIn::text).collect();
    for file in files {
        let named = named_to_run(file.path, &texts);
        let Some(text) = file.text() else {
            summary.not_text.push(file.path.to_owned());
            let shebang = file.bytes.starts_with(b"#!");
            let by_name = script_runs(file.path, file.executable, "");
            if file.executable || shebang || named || by_name.is_some() {
                summary.scripts.push(Script {
                    path: file.path.to_owned(),
                    runs: by_name.unwrap_or_else(|| "a file that is not text".to_owned()),
                    risks: vec![Risk::NotText],
                });
            }
            continue;
        };
        for (at, line) in text.lines().enumerate() {
            let mut kinds: Vec<Invisible> = line.chars().filter_map(Invisible::of).collect();
            if !kinds.is_empty() {
                kinds.sort();
                kinds.dedup();
                summary.unseen.push(Unseen {
                    path: file.path.to_owned(),
                    line: at + 1,
                    kinds,
                });
            }
        }
        let runs = script_runs(file.path, file.executable, text)
            .or_else(|| named.then(|| "a file the instructions run".to_owned()));
        let commands = bang_commands(text);
        // The lines that run: a script's every line, or a `!` line's command.
        let mut running: Vec<(usize, String)> = Vec::new();
        if runs.is_some() {
            running.extend(
                text.lines()
                    .enumerate()
                    .filter(|(_, line)| !line.trim_start().starts_with('#'))
                    .map(|(at, line)| (at + 1, line.to_owned())),
            );
        }
        running.extend(commands.iter().map(|(line, cmd)| (*line, cmd.clone())));
        for (line, command) in &commands {
            summary.commands.push(Command {
                path: file.path.to_owned(),
                line: *line,
                command: command.clone(),
            });
        }
        let mut risks: Vec<Risk> = Vec::new();
        for (line, run) in &running {
            risks.extend(risks_of(run));
            for target in outside_writes(run) {
                summary.outside.push(Outside {
                    path: file.path.to_owned(),
                    line: *line,
                    target,
                });
            }
        }
        for (at, line) in text.lines().enumerate() {
            let runs_here = running.iter().any(|(n, _)| *n == at + 1);
            for host in hosts_in(line) {
                add_host(&mut hosts, &host, file.path, runs_here);
            }
        }
        if let Some(runs) = runs {
            risks.sort();
            risks.dedup();
            summary.scripts.push(Script {
                path: file.path.to_owned(),
                runs,
                risks,
            });
        }
    }
    hosts.sort();
    summary.hosts = hosts;
    summary.scripts.sort();
    summary.commands.sort();
    summary.outside.sort();
    summary.outside.dedup();
    summary.unseen.sort();
    summary.not_text.sort();
    summary
}

/// Adds `file` naming `host` to `hosts`, marked where it names it in a line that runs.
fn add_host(hosts: &mut Vec<Host>, host: &str, file: &str, risky: bool) {
    match hosts.iter_mut().find(|one| one.host == host) {
        Some(one) => {
            if !one.files.iter().any(|f| f == file) {
                one.files.push(file.to_owned());
            }
            one.risky |= risky;
        }
        None => hosts.push(Host {
            host: host.to_owned(),
            files: vec![file.to_owned()],
            risky,
        }),
    }
}

/// What runs the file at `path` whose text is `text`, where it is a script: its `#!` line's
/// program, or its kind by its extension, or "an executable file" where git marks it so.
fn script_runs(path: &str, executable: bool, text: &str) -> Option<String> {
    if let Some(shebang) = text.lines().next().and_then(|line| line.strip_prefix("#!")) {
        let mut words = shebang.split_whitespace();
        let program = words.next().unwrap_or_default();
        let program = program.rsplit('/').next().unwrap_or(program);
        let program = if program == "env" {
            words.find(|word| !word.starts_with('-')).unwrap_or(program)
        } else {
            program
        };
        if !program.is_empty() {
            return Some(program.to_owned());
        }
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    if let Some((_, extension)) = name.rsplit_once('.') {
        let extension = extension.to_ascii_lowercase();
        if let Some((_, runs)) = BY_EXTENSION.iter().find(|(ext, _)| *ext == extension) {
            return Some((*runs).to_owned());
        }
    }
    executable.then(|| "an executable file".to_owned())
}

/// Each `` !`cmd` `` in `text` with its line, and each line of a ```` ```! ```` block: what some
/// harnesses run before the model reads the skill.
fn bang_commands(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut in_block = false;
    for (at, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if in_block {
            if trimmed.starts_with("```") {
                in_block = false;
            } else if !trimmed.is_empty() {
                out.push((at + 1, trimmed.to_owned()));
            }
            continue;
        }
        if trimmed.starts_with("```!") {
            in_block = true;
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find("!`") {
            let after = &rest[start + 2..];
            let Some(end) = after.find('`') else {
                break;
            };
            let command = after[..end].trim();
            if !command.is_empty() {
                out.push((at + 1, command.to_owned()));
            }
            rest = &after[end + 1..];
        }
    }
    out
}

/// The words of a command line, split at blanks and at the shell's own separators.
fn words(line: &str) -> Vec<&str> {
    line.split(|c: char| c.is_whitespace() || ";&|()`$".contains(c))
        .filter(|word| !word.is_empty())
        .collect()
}

/// Why the command line `line` is risky.
fn risks_of(line: &str) -> Vec<Risk> {
    let mut risks = Vec::new();
    let said = words(line);
    if said.iter().any(|word| NETWORK_WORDS.contains(word)) || !hosts_in(line).is_empty() {
        risks.push(Risk::Network);
    }
    if said.iter().any(|word| ELEVATING.contains(word)) {
        risks.push(Risk::Elevates);
    }
    if said.contains(&"eval") || piped_into_a_runner(line) {
        risks.push(Risk::RunsWhatItFetches);
    }
    let removes = said
        .windows(2)
        .any(|pair| pair[0] == "rm" && pair[1].starts_with('-') && pair[1].contains(['r', 'R']));
    if removes {
        risks.push(Risk::Removes);
    }
    let hidden = said
        .windows(2)
        .any(|pair| pair[0] == "base64" && matches!(pair[1], "-d" | "-D" | "--decode"));
    if hidden {
        risks.push(Risk::Hidden);
    }
    risks
}

/// Whether `line` pipes something into a program that runs it: `curl … | sh`.
fn piped_into_a_runner(line: &str) -> bool {
    line.split('|').skip(1).any(|piece| {
        let mut words = piece.split_whitespace();
        let first = words.next().unwrap_or_default();
        let first = if ELEVATING.contains(&first) {
            words.next().unwrap_or_default()
        } else {
            first
        };
        let first = first.rsplit('/').next().unwrap_or(first);
        RUNNERS.contains(&first)
    })
}

/// Whether `target`, a path a command writes, is outside the workspace: absolute, in the home
/// folder, or above where the command runs. A device that discards or prints is not a write.
fn is_outside(target: &str) -> bool {
    let target = target.trim_matches(|c| c == '"' || c == '\'');
    if matches!(target, "/dev/null" | "/dev/stdout" | "/dev/stderr") {
        return false;
    }
    target.starts_with('/')
        || target.starts_with('~')
        || target.starts_with("$HOME")
        || target.starts_with("${HOME}")
        || target == ".."
        || target.starts_with("../")
}

/// Each place outside the workspace the command line `line` writes: after a `>` or `>>`, and each
/// path a program that writes is given.
fn outside_writes(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(at) = rest.find('>') {
        let after = rest[at + 1..].trim_start_matches('>').trim_start();
        let target: String = after
            .chars()
            .take_while(|c| !c.is_whitespace() && !";&|)".contains(*c))
            .collect();
        // `2>&1` and `>&2` move a stream; they write nowhere.
        if !target.starts_with('&') && is_outside(&target) {
            out.push(target);
        }
        rest = &rest[at + 1..];
    }
    let said: Vec<&str> = line.split_whitespace().collect();
    let mut writing = false;
    for word in said {
        if ";&|".contains(word) {
            writing = false;
            continue;
        }
        let program = word.rsplit('/').next().unwrap_or(word);
        if WRITERS.contains(&program) {
            writing = true;
            continue;
        }
        if writing && !word.starts_with('-') && is_outside(word) {
            let word = word.trim_matches(|c| c == '"' || c == '\'' || c == ';');
            if !out.iter().any(|one| one == word) {
                out.push(word.to_owned());
            }
        }
    }
    out
}

/// Each host a URL in `line` names, lowercased: `https://api.example.com/v1` names
/// `api.example.com`, and `git@github.com:o/r` names `github.com`.
fn hosts_in(line: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |host: &str| {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        let plausible = host.contains('.') || host == "localhost";
        if plausible
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-.".contains(c))
            && !out.contains(&host)
        {
            out.push(host);
        }
    };
    let mut rest = line;
    while let Some(at) = rest.find("://") {
        let before = &rest[..at];
        let scheme: String = before
            .chars()
            .rev()
            .take_while(char::is_ascii_alphanumeric)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let after = &rest[at + 3..];
        if SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()) {
            let authority: String = after
                .chars()
                .take_while(|c| !"/?#\"'`)]>< \t,;".contains(*c))
                .collect();
            let host = authority.rsplit('@').next().unwrap_or(&authority);
            let host = host.split(':').next().unwrap_or(host);
            push(host);
        }
        rest = after;
    }
    for word in line.split_whitespace() {
        if let Some((user, rest)) = word.split_once('@')
            && !user.is_empty()
            && !user.contains("://")
            && user
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
            && let Some((host, _)) = rest.split_once(':')
        {
            push(host);
        }
    }
    out
}

/// **What changed in a summary** between two versions of a skill: what the newer one has that
/// the older did not, and what it no longer has. A script whose marks changed is in both.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Changes {
    pub added: Summary,
    pub removed: Summary,
}

impl Changes {
    /// Whether the summary did not change at all.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

/// What `now` has that `was` does not, item by item. A host counts by its name and its mark, a
/// `!` line by its command and an outside write by its target, wherever in the files they moved.
fn gained(now: &Summary, was: &Summary) -> Summary {
    Summary {
        scripts: now
            .scripts
            .iter()
            .filter(|one| !was.scripts.contains(one))
            .cloned()
            .collect(),
        hosts: now
            .hosts
            .iter()
            .filter(|one| {
                !was.hosts
                    .iter()
                    .any(|old| old.host == one.host && old.risky == one.risky)
            })
            .cloned()
            .collect(),
        commands: now
            .commands
            .iter()
            .filter(|one| {
                !was.commands
                    .iter()
                    .any(|old| old.path == one.path && old.command == one.command)
            })
            .cloned()
            .collect(),
        outside: now
            .outside
            .iter()
            .filter(|one| {
                !was.outside
                    .iter()
                    .any(|old| old.path == one.path && old.target == one.target)
            })
            .cloned()
            .collect(),
        unseen: now
            .unseen
            .iter()
            .filter(|one| {
                !was.unseen
                    .iter()
                    .any(|old| old.path == one.path && old.kinds == one.kinds)
            })
            .cloned()
            .collect(),
        not_text: now
            .not_text
            .iter()
            .filter(|one| !was.not_text.contains(one))
            .cloned()
            .collect(),
    }
}

/// **The summary's changes** from `was` to `now`.
pub fn changes(was: &Summary, now: &Summary) -> Changes {
    Changes {
        added: gained(now, was),
        removed: gained(was, now),
    }
}

#[cfg(test)]
mod tests;
