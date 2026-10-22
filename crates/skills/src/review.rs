// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What a skill does beyond its instructions**, as the install review leads with it (SK-10,
//! #2773; ADR 0100 §3; spec #2762 decision 39): the scripts it carries and what runs each, the
//! network hosts its files name, its `` !`cmd` `` lines, and the writes it would make outside the
//! workspace, each risky one marked. Worked out from the files alone, so it lists what they name,
//! not what a script could compute when it runs: the review is a reading aid, and what bounds a
//! skill is the chat's sandbox (ADR 0067).
//!
//! The same reading answers an update ([`changes`]): what the summary gained and lost between the
//! pinned commit and the newer one, then each file's [`line_diff`].

use serde::Serialize;

/// One file of a skill, as the summary reads it: its path inside the skill's folder (`/` between
/// the parts), whether git marks it executable, and its text, `None` for a file that is not text.
#[derive(Debug, Clone, Copy)]
pub struct FileIn<'a> {
    pub path: &'a str,
    pub executable: bool,
    pub text: Option<&'a str>,
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
}

impl std::fmt::Display for Risk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Network => "reaches the network",
            Self::RunsWhatItFetches => "runs what it downloads or builds",
            Self::Elevates => "asks for more rights",
            Self::Removes => "removes files",
            Self::Hidden => "hides what it runs",
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

/// **The summary of one skill**: what it does beyond its instructions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Summary {
    pub scripts: Vec<Script>,
    pub hosts: Vec<Host>,
    pub commands: Vec<Command>,
    pub outside: Vec<Outside>,
}

impl Summary {
    /// Whether anything in it is marked.
    pub fn risky(&self) -> bool {
        self.scripts.iter().any(|one| !one.risks.is_empty())
            || self.hosts.iter().any(|one| one.risky)
            || !self.commands.is_empty()
            || !self.outside.is_empty()
    }

    /// Whether it lists nothing at all: a skill of instructions alone.
    pub fn is_empty(&self) -> bool {
        self.scripts.is_empty()
            && self.hosts.is_empty()
            && self.commands.is_empty()
            && self.outside.is_empty()
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

/// **The summary of a skill** whose files are `files`.
pub fn summarize(files: &[FileIn<'_>]) -> Summary {
    let mut summary = Summary::default();
    let mut hosts: Vec<Host> = Vec::new();
    for file in files {
        let Some(text) = file.text else {
            if file.executable {
                summary.scripts.push(Script {
                    path: file.path.to_owned(),
                    runs: "an executable file".to_owned(),
                    risks: Vec::new(),
                });
            }
            continue;
        };
        let runs = script_runs(file.path, file.executable, text);
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
    let removes = said.windows(2).any(|pair| {
        pair[0] == "rm" && pair[1].starts_with('-') && pair[1].contains(['r', 'R'])
    });
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
            && user.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
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
    }
}

/// **The summary's changes** from `was` to `now`.
pub fn changes(was: &Summary, now: &Summary) -> Changes {
    Changes {
        added: gained(now, was),
        removed: gained(was, now),
    }
}

/// The most lines a [`line_diff`] compares on each side; a longer file is shown whole as
/// replaced.
pub const MOST_DIFF_LINES: usize = 3000;

/// **The lines that changed from `was` to `now`**, as a unified diff with three lines of
/// context: `-` for a line taken out, `+` for one put in, and `@@ line N @@` where a stretch
/// starts. Empty where nothing changed.
pub fn line_diff(was: &str, now: &str) -> String {
    let old: Vec<&str> = was.lines().collect();
    let new: Vec<&str> = now.lines().collect();
    if old == new {
        return String::new();
    }
    let steps = if old.len() > MOST_DIFF_LINES || new.len() > MOST_DIFF_LINES {
        old.iter()
            .map(|line| Step::Out(line))
            .chain(new.iter().map(|line| Step::In(line)))
            .collect()
    } else {
        steps_of(&old, &new)
    };
    unified(&steps)
}

/// One line of a diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step<'a> {
    Same(&'a str),
    Out(&'a str),
    In(&'a str),
}

/// The steps from `old` to `new` by their longest common run of lines.
fn steps_of<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<Step<'a>> {
    let (n, m) = (old.len(), new.len());
    let mut longest = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            longest[i][j] = if old[i] == new[j] {
                longest[i + 1][j + 1] + 1
            } else {
                longest[i + 1][j].max(longest[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut steps = Vec::with_capacity(n + m);
    while i < n && j < m {
        if old[i] == new[j] {
            steps.push(Step::Same(old[i]));
            i += 1;
            j += 1;
        } else if longest[i + 1][j] >= longest[i][j + 1] {
            steps.push(Step::Out(old[i]));
            i += 1;
        } else {
            steps.push(Step::In(new[j]));
            j += 1;
        }
    }
    steps.extend(old[i..].iter().map(|line| Step::Out(line)));
    steps.extend(new[j..].iter().map(|line| Step::In(line)));
    steps
}

/// `steps` as a unified diff, three lines of context around each change.
fn unified(steps: &[Step<'_>]) -> String {
    const CONTEXT: usize = 3;
    let changed: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| !matches!(step, Step::Same(_)))
        .map(|(at, _)| at)
        .collect();
    let mut out = String::new();
    // The step after the last one shown, while a stretch is open.
    let mut open: Option<usize> = None;
    let mut new_line = 1usize;
    for (index, step) in steps.iter().enumerate() {
        let near = changed
            .iter()
            .any(|&c| c + CONTEXT >= index && index + CONTEXT >= c);
        if near {
            if open != Some(index) {
                out.push_str(&format!("@@ line {new_line} @@\n"));
            }
            match step {
                Step::Same(line) => out.push_str(&format!(" {line}\n")),
                Step::Out(line) => out.push_str(&format!("-{line}\n")),
                Step::In(line) => out.push_str(&format!("+{line}\n")),
            }
            open = Some(index + 1);
        }
        if !matches!(step, Step::Out(_)) {
            new_line += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests;
