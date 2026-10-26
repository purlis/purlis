// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's tools** (#1362, decided in #1407): a program a persona's chats run by its
//! plain name, with a credential from a vault handed to it by `purlis secret exec`, so the
//! chat never reads the vault.
//!
//! **Where they are written.** Beside the persona's hosts, in the project's committed
//! `charter.toml` and nowhere a chat writes ([`super::persona`], D-1362-1):
//!
//! ```toml
//! [sandbox.personas.devops]
//! tools = { kubectl = { vault = "devops", as = "KUBECONFIG file", key = "kubeconfig" } }
//! ```
//!
//! `as` is the variable the program reads and how the value is handed: `"<VAR> file"` puts the
//! value in a private temporary file and the file's path in `VAR` (`secret exec --file`), which
//! is the one to prefer; `"<VAR> env"` puts the value itself in `VAR` (`--env`), which another
//! process of the same user can read while the program runs.
//!
//! **A tool that needs several values** (T-1, the operator's ruling of 2026-10-14) names them
//! under `values`, each with its variable, key and hand-over, all from the tool's one vault:
//!
//! ```toml
//! [sandbox.personas.cloud]
//! tools = { aws = { vault = "cloud", values = {
//!     AWS_ACCESS_KEY_ID = { key = "aws-access-key-id", as = "env" },
//!     AWS_SECRET_ACCESS_KEY = { key = "aws-secret-access-key", as = "env" } } } }
//! ```
//!
//! A tool takes one form or the other: `key` and `as`, or `values` ([`MOST_VALUES`] at most,
//! no variable twice).
//!
//! **How a plain `kubectl` reaches it: a shim** (ADR 0062's kind). As a sandboxed chat running
//! as the persona starts, purlis writes one small script per tool under
//! `.purlis/app/tools/<persona>/bin/`, a folder no chat writes ([`super::Class::Integrity`]),
//! and puts that folder first on the chat's `PATH` ([`first_on_path`]). Each runs
//! `purlis secret exec <vault> --file <VAR>=<key> -- <the real program> "$@"`. A shim works for
//! every harness and inside compound commands and scripts. Claude Code's Bash tool starts each
//! command from the person's shell snapshot, whose start files commonly put their own folders
//! first, so a Claude Code chat is also handed `CLAUDE_ENV_FILE` (read by Claude Code from
//! 2.1.291 before each command), a file beside the shims that puts their folder first again.
//!
//! **The real program is found by the app, as the chat starts**, on the app's own search path,
//! and only where no chat writes: never in the project, a temporary folder or any folder the
//! chat may write, as found and as its links resolve ([`resolve`]). The shim names it by its
//! absolute path, so nothing a chat puts on its `PATH` later is what runs.
//!
//! **Allowed on each machine, as the persona's hosts are** (D-PE-ruling-1, the operator's ruling
//! of 2026-10-12). `purlis secret exec` from a sandboxed chat is brokered by the app, which hands
//! the run only a vault the registry tags for the chat's persona, holds it to the chat's own
//! sandbox ([`crate::secrets::brokered`]), and **runs only a tool the persona declares**
//! (D-PE-ruling-2, [`declared`]). So a committed tool is what lets a vault's value reach a
//! program on a chat's say, and a teammate can change it: a persona's tools reach no chat on a
//! machine until the person there allows them, with its hosts, on the same Notice
//! ([`super::persona::digest_of`]). Until then, and after any change, no shim is written and the
//! broker runs nothing. The person's own terminal, and anything run outside a chat's sandbox, is
//! not held to any of this.
//!
//! **Refused**, with a sentence, and given no shim: a name that is a shell, an interpreter,
//! `env`, `sudo`, `xargs` or another program that runs what it is told, or purlis itself
//! ([`refused_name`]), and one whose program, as found and as its links resolve, is one of
//! those ([`refused_program`]); a variable that steers how programs are found or loaded
//! ([`refused_var`]); a tool policy forbids ([`super::policy::Locks::refuses_tool`]); and one
//! whose program is not found where no chat writes.
//!
//! **A general-purpose program stays a tool, and its Allow says so** (T-4, the operator's ruling
//! of 2026-10-14): curl, wget and the like can show the values they receive in the chat's
//! output, so the person reads one sentence beside such a tool's line ([`caution`]). The
//! sentence is never part of the line an Allow is bound to.
//!
//! **A browser login** is a tool of its own kind ([`login`], D-PE-browser-1): `browser = "<login
//! page>"` in place of `as`, and purlis, not the chat, opens that page in the chat's own browser
//! and fills the login on its origin alone (#2016). It runs no program, and its shim runs
//! nothing: the chat asks for it through the browser tools (`browser_login`).

use std::path::{Path, PathBuf};

use crate::settings::Refusal;

pub mod login;

/// The key in a persona's table that holds its tools.
pub const KEY: &str = "tools";

/// The key of a tool's table that names several values ([`Value`]).
pub const VALUES_KEY: &str = "values";

/// The keys one tool's table holds: `as` and `key`, or `values`, for a program's; `browser` and
/// `user` for a browser login's ([`login`]).
const TOOL_KEYS: [&str; 6] = [
    "vault",
    "as",
    "key",
    VALUES_KEY,
    login::KEY,
    login::USER_KEY,
];

/// The keys one of a tool's `values` holds.
const VALUE_KEYS: [&str; 2] = ["key", "as"];

/// **The most values one tool is handed** (T-1). Each is a file or a variable the run makes and
/// a flag on the shim's line; a program that needs more than a handful of credentials at once is
/// not one tool, and the bound keeps the line a person allows short enough to read whole.
pub const MOST_VALUES: usize = 16;

/// The longest tool name read.
const MOST_NAME: usize = 64;

/// How a tool's value is handed to its program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handed {
    /// A private temporary file, its path in the variable (`secret exec --file`).
    File,
    /// The value itself in the variable (`secret exec --env`).
    Env,
}

impl Handed {
    /// The `secret exec` flag that hands a value this way.
    pub fn flag(self) -> &'static str {
        match self {
            Self::File => "--file",
            Self::Env => "--env",
        }
    }

    /// The word a declaration spells it with.
    pub fn word(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Env => "env",
        }
    }
}

/// **One value a tool is handed**: the variable its program reads, how it is handed, and the key
/// of the tool's vault it comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    pub var: String,
    pub handed: Handed,
    pub key: String,
}

impl Value {
    /// The value as `secret exec` spells it after its flag: `VAR=key`.
    pub fn spec(&self) -> String {
        format!("{}={}", self.var, self.key)
    }
}

/// **One tool of a persona's**: the program's name, the vault its values come from, and each
/// value with the variable it is handed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    pub name: String,
    pub vault: String,
    /// Every value the program is handed, in the order of their variables: one for a tool
    /// written with `key` and `as`, each of its `values` otherwise. For a browser login, the
    /// password's file ([`login::PASS_VAR`]), then the user name's where it has one.
    pub values: Vec<Value>,
    /// Where the tool is a browser login ([`login`]): the page and its origin. Its keys are
    /// among [`Self::values`].
    pub login: Option<login::Login>,
}

impl Tool {
    /// A program's tool handed one value, as `key` and `as` write it.
    pub fn one(name: &str, vault: &str, var: &str, handed: Handed, key: &str) -> Self {
        Self {
            name: name.to_owned(),
            vault: vault.to_owned(),
            values: vec![Value {
                var: var.to_owned(),
                handed,
                key: key.to_owned(),
            }],
            login: None,
        }
    }

    /// The values handed `handed`, as `secret exec` spells each (`VAR=key`), in order.
    pub fn specs(&self, handed: Handed) -> Vec<String> {
        self.values
            .iter()
            .filter(|value| value.handed == handed)
            .map(Value::spec)
            .collect()
    }

    /// **A browser login's keys** ([`login`], #2016): the user name's where the page asks for
    /// one, and the password's. None for a tool that is no browser login.
    pub fn login_keys(&self) -> Option<(Option<&str>, &str)> {
        self.login.as_ref()?;
        Some((self.key_in(login::USER_VAR), self.key_in(login::PASS_VAR)?))
    }

    /// The key of the vault the value handed in `var` comes from, where one is.
    fn key_in(&self, var: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|value| value.var == var)
            .map(|value| value.key.as_str())
    }

    /// The tool whole, as an Allow of it is bound to and a person reads it: its name, then the
    /// vault, key and variable it is handed; for a browser login, the page and both keys.
    ///
    /// **One line is one tool** (#1828): a login's keys are quoted, since a key or a user name
    /// may hold the words that join them, and two logins must never share a line, so neither
    /// shares an Allow. A program's tool handed one value keeps its spelling: its key is the only
    /// free text in it, and what follows the key is a vault's and a variable's name, which hold
    /// no space. One handed several names every value on its one line, each key quoted for the
    /// same reason (T-1): `aws with "aws-access-key-id" as AWS_ACCESS_KEY_ID env and
    /// "aws-secret-access-key" as AWS_SECRET_ACCESS_KEY env from vault cloud`.
    pub fn whole(&self) -> String {
        if let Some(login) = &self.login {
            let user = self
                .key_in(login::USER_VAR)
                .map_or_else(String::new, |user| format!("{user:?} and "));
            return format!(
                "{} logs in to {} with {user}{:?} from vault {} in a browser",
                self.name,
                login.url,
                self.key_in(login::PASS_VAR).unwrap_or_default(),
                self.vault
            );
        }
        if let [one] = self.values.as_slice() {
            return format!(
                "{} with {} from vault {} as {} {}",
                self.name,
                one.key,
                self.vault,
                one.var,
                one.handed.word()
            );
        }
        let each: Vec<String> = self
            .values
            .iter()
            .map(|value| format!("{:?} as {} {}", value.key, value.var, value.handed.word()))
            .collect();
        let listed = match each.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => each.join(""),
        };
        format!("{} with {listed} from vault {}", self.name, self.vault)
    }

    /// Each hand-over as `secret exec` writes it, flag then `VAR=key`, in order.
    fn flags(&self) -> Vec<String> {
        self.values
            .iter()
            .flat_map(|value| [value.handed.flag().to_owned(), value.spec()])
            .collect()
    }

    /// The words of `purlis secret exec` that run `program` as this tool, after the binary:
    /// `secret exec <vault> --file <VAR>=<key> [--env <VAR>=<key> …] -- <program>`, one flag per
    /// value. A browser login names its tool by its name instead, after its files.
    pub fn secret_exec(&self, program: &Path) -> Vec<String> {
        // A login runs by its name: the app, not the shim, picks what runs (D-PE-browser-1).
        let run = if self.login.is_some() {
            self.name.clone()
        } else {
            program.display().to_string()
        };
        let named = self
            .argv0_for(program)
            .map(|name| ["--argv0".to_owned(), name.to_owned()]);
        ["secret".to_owned(), "exec".to_owned(), self.vault.clone()]
            .into_iter()
            .chain(self.flags())
            .chain(named.into_iter().flatten())
            .chain(["--".to_owned(), run])
            .collect()
    }

    /// **The name `program` starts under as this tool**, where it is not the program's own
    /// file name: the name the tool was declared as (#1977). A tool found through a link of
    /// another name runs the program the link resolves to, by its path, and a program that does
    /// what its name says (`unxz` linked to `xz`, a `git-*` link) does what the declared one
    /// says. None for a browser login, whose program is purlis's own pick.
    fn argv0_for(&self, program: &Path) -> Option<&str> {
        (self.login.is_none() && program.file_name() != Some(std::ffi::OsStr::new(&self.name)))
            .then_some(self.name.as_str())
    }
}

/// Names no tool may take: each runs whatever it is told, so a tool of one would be a vault's
/// credential handed to anything. Also purlis itself, under each of its names.
const REFUSED_NAMES: &[&str] = &[
    // shells
    "sh",
    "bash",
    "zsh",
    "dash",
    "ksh",
    "mksh",
    "oksh",
    "pdksh",
    "csh",
    "tcsh",
    "fish",
    "nu",
    "elvish",
    "xonsh",
    "ash",
    "busybox",
    "pwsh",
    "powershell",
    "rc",
    "es",
    "yash",
    "osh",
    // what runs another program
    "env",
    "sudo",
    "doas",
    "su",
    "xargs",
    "exec",
    "nohup",
    "nice",
    "timeout",
    "time",
    "command",
    "builtin",
    "eval",
    "script",
    "expect",
    "watch",
    "parallel",
    "chroot",
    "setsid",
    "stdbuf",
    "caffeinate",
    "open",
    "launchctl",
    "osascript",
    "sandbox-exec",
    "arch",
    "flock",
    // interpreters
    "python",
    "pythonw",
    "perl",
    "ruby",
    "node",
    "nodejs",
    "deno",
    "bun",
    "php",
    "lua",
    "luajit",
    "tclsh",
    "wish",
    "awk",
    "gawk",
    "mawk",
    "nawk",
    "jshell",
    "irb",
    "pypy",
    "R",
    "Rscript",
    "julia",
    "guile",
    "racket",
    "sbcl",
    "clisp",
    "ocaml",
    "swift",
    "java",
    "jrunscript",
    // purlis
    "purlis",
    "charter",
    "purlisd",
    "charterd",
];

/// Prefixes no tool name may start with: an interpreter's versioned names (`python3.12`), and
/// purlis's own programs.
const REFUSED_PREFIXES: &[&str] = &[
    "python", "perl", "ruby", "node", "php", "lua", "pypy", "tclsh", "wish", "purlis-", "charter-",
];

/// **Why `name` may not be a tool**, if it may not: not a program's plain name, or one that
/// runs whatever it is told ([`REFUSED_NAMES`]).
pub fn refused_name(name: &str) -> Option<String> {
    let plain = !name.is_empty()
        && name.len() <= MOST_NAME
        && !name.starts_with(['.', '-'])
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'));
    if !plain {
        return Some(
            "a tool is named as a program's plain name: letters, digits, '.', '_', '-' and '+', \
             starting with a letter or digit"
                .to_owned(),
        );
    }
    runs_what_it_is_told(name).then(|| runs_anything(name))
}

/// The sentence a tool named `name` is refused with where its program runs whatever it is told.
fn runs_anything(name: &str) -> String {
    format!(
        "{name} runs whatever it is told, so it would hand the vault's credential to anything — \
         a tool is the one program that needs it"
    )
}

/// `program`'s file name as it is judged: lower case, a `.exe` aside.
fn file_name_of(program: &Path) -> String {
    let lower = program
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_owned()
}

/// **Why `program`, found for `tool`, may not run as it**, if it may not: a tool is refused
/// where the program it runs, as found and as its links resolve, is one no tool may take
/// ([`refused_name`]), with the same sentence. A browser login's program is purlis's own pick,
/// and is not judged here.
pub fn refused_program(tool: &Tool, program: &Path) -> Option<String> {
    let real = program.canonicalize().ok();
    refused_at(tool, program, real.as_deref().unwrap_or(program))
}

/// [`refused_program`], for `found` as found and `real` as its links resolve.
fn refused_at(tool: &Tool, found: &Path, real: &Path) -> Option<String> {
    if tool.login.is_some() {
        return None;
    }
    [found, real]
        .into_iter()
        .map(file_name_of)
        .any(|name| !name.is_empty() && runs_what_it_is_told(&name))
        .then(|| runs_anything(&tool.name))
}

/// The most links followed on the way to one program, as the kernel's own bound on a path.
const MOST_LINKS: usize = 40;

/// **`program` as it runs, where nothing on the way to it is a chat's**: the path with every
/// link resolved, one link at a time, where each link, folders' links too, sits in a folder
/// outside `written`, and so does the program it ends at. `None` otherwise, or where a link
/// cannot be read or there are more than [`MOST_LINKS`].
///
/// A run is of this path, so no link a chat could change later is on the way to what runs.
pub(crate) fn held_outside(program: &Path, written: &[PathBuf]) -> Option<PathBuf> {
    use std::path::Component;
    let mut done = PathBuf::new();
    let mut rest: std::collections::VecDeque<std::ffi::OsString> = Default::default();
    let start = |path: &Path, done: &mut PathBuf, rest: &mut std::collections::VecDeque<_>| {
        let mut parts = Vec::new();
        for part in path.components() {
            match part {
                Component::Prefix(_) | Component::RootDir => {
                    done.clear();
                    done.push(part.as_os_str());
                }
                Component::CurDir => {}
                Component::ParentDir | Component::Normal(_) => {
                    parts.push(part.as_os_str().to_owned());
                }
            }
        }
        for part in parts.into_iter().rev() {
            rest.push_front(part);
        }
    };
    if !program.is_absolute() {
        return None;
    }
    start(program, &mut done, &mut rest);
    let mut links = 0;
    while let Some(part) = rest.pop_front() {
        if part == ".." {
            done.pop();
            continue;
        }
        let next = done.join(&part);
        let meta = std::fs::symlink_metadata(&next).ok()?;
        if !meta.file_type().is_symlink() {
            done = next;
            continue;
        }
        links += 1;
        // The folder that holds the link, as it is with every link before it resolved.
        if links > MOST_LINKS || inside_any(&done, written) {
            return None;
        }
        let target = std::fs::read_link(&next).ok()?;
        start(&target, &mut done, &mut rest);
    }
    (!inside_any(&done, written)).then_some(done)
}

/// **Every folder a chat in the project at `root` may write, whatever the chat**: every folder
/// listed as one chats may be granted, and the project's cache home. Left out of the search
/// for a tool's program as the Notice shows it and as a chat starts, so both find the same one.
fn any_chat_writes(root: &Path) -> Vec<PathBuf> {
    #[cfg(test)]
    if SEARCH_HERE.with(|dirs| dirs.borrow().is_some()) {
        return Vec::new();
    }
    super::local::grantable_folders(root)
        .into_iter()
        .chain(super::caches::root_of(&super::Machine::this(), root))
        .collect()
}

/// The first of `dirs` that holds `wanted`, as found, with its path as it runs
/// ([`held_outside`]): one with anything on the way to it in `written` is passed over.
fn found_in(wanted: &str, dirs: &[PathBuf], written: &[PathBuf]) -> Option<(PathBuf, PathBuf)> {
    dirs.iter()
        .filter(|dir| dir.is_absolute() && !inside_any(dir, written))
        .find_map(|dir| {
            let program = crate::programs::find(wanted, std::slice::from_ref(dir))?;
            let real = held_outside(&program, written)?;
            Some((program, real))
        })
}

#[cfg(test)]
thread_local! {
    /// A test's own search path, in place of the app's, for the calls on its thread.
    pub(crate) static SEARCH_HERE: std::cell::RefCell<Option<Vec<PathBuf>>> =
        const { std::cell::RefCell::new(None) };
}

/// The app's own search path, where a tool's program is found.
fn search_dirs() -> Vec<PathBuf> {
    #[cfg(test)]
    if let Some(dirs) = SEARCH_HERE.with(|dirs| dirs.borrow().clone()) {
        return dirs;
    }
    crate::programs::search_dirs()
}

/// **Why `tool` runs nothing in the project at `root`, judged by its program on this machine**
/// ([`refused_program`]), as the Notice shows it and an Allow is of it: its program found on
/// this process's search path as a chat's start finds it, with the project, the temporary
/// folders and every folder any chat there may write left out ([`any_chat_writes`]). None
/// where it runs or is not found. A chat's start judges again, also leaving out what that chat
/// alone may write.
pub fn refused_here(root: &Path, tool: &Tool) -> Option<String> {
    if tool.login.is_some() {
        return None;
    }
    let mut written = vec![root.to_path_buf()];
    written.extend(temp_folders());
    written.extend(any_chat_writes(root));
    let (found, real) = found_in(&tool.name, &search_dirs(), &written)?;
    refused_at(tool, &found, &real)
}

/// Whether `name` is one of [`REFUSED_NAMES`], or starts with one of [`REFUSED_PREFIXES`],
/// whatever its case.
fn runs_what_it_is_told(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    REFUSED_NAMES
        .iter()
        .any(|one| one.eq_ignore_ascii_case(name))
        || REFUSED_PREFIXES.iter().any(|one| lower.starts_with(one))
}

/// **General-purpose programs** (T-4, the operator's ruling of 2026-10-14): each fetches,
/// sends, copies, encodes, writes or rewrites content as its arguments say, so what it does with the values it
/// is handed is the chat's to decide, not the declaration's. Such a program stays a tool; its
/// Allow says so in one sentence ([`caution`]). A short list, reviewed with each addition, and
/// not a complete one: a program off it is not thereby one that keeps what it is handed.
const GENERAL_PURPOSE: &[&str] = &[
    // fetch and send over the network
    "curl", "wget", "wget2", "http", "https", "httpie", "xh", "xhs", "aria2c", "nc", "ncat",
    "netcat", "socat", "ssh", "scp", "sftp", "rsync",
    // copy, encode or rewrite what they are given
    "cat", "cp", "tee", "dd", "sed", "base64", "xxd", "od", "hexdump", "openssl", "gzip", "tar",
    "zip",
];

/// **Whether the program at `program` is a general-purpose one**, by its file name, whatever
/// its folder and case (a `.exe` aside): one of [`GENERAL_PURPOSE`], or of the names no tool may
/// take ([`refused_name`]).
pub fn general_purpose(program: &Path) -> bool {
    let name = file_name_of(program);
    !name.is_empty() && (GENERAL_PURPOSE.contains(&name.as_str()) || runs_what_it_is_told(&name))
}

/// **The sentence a person reads beside `tool`'s Allow** where its program is a general-purpose
/// one (T-4): by the tool's name, and by each program that name finds on `search`, as found and
/// as its links resolve. None for a program off the list, and none for a browser login, whose program
/// purlis picks. The sentence is said beside the tool's line, never part of it, so an Allow
/// stays bound to the line alone ([`super::persona::digest_of`]).
pub fn caution(tool: &Tool, search: &[PathBuf]) -> Option<String> {
    if tool.login.is_some() {
        return None;
    }
    let found = search.iter().filter_map(|dir| {
        crate::programs::find(&tool.name, std::slice::from_ref(dir))
            .map(|program| (program.canonicalize().ok(), program))
    });
    let general = general_purpose(Path::new(&tool.name))
        || found
            .flat_map(|(real, program)| real.into_iter().chain([program]))
            .any(|program| general_purpose(&program));
    general.then(|| {
        format!(
            "{} can show the values it is handed in the chat's output; allow it only for a \
             persona you trust with them.",
            tool.name
        )
    })
}

/// [`caution`] for each of `tools`, in order, on the app's own search path.
pub fn cautions(tools: &[Tool]) -> Vec<String> {
    let search = search_dirs();
    tools
        .iter()
        .filter_map(|tool| caution(tool, &search))
        .collect()
}

/// Variables no tool's value is handed in: those that steer which program runs or what it
/// loads, and purlis's own.
const REFUSED_VARS: &[&str] = &["PATH", "HOME", "SHELL", "IFS", "ENV", "BASH_ENV", "CDPATH"];

/// Prefixes no tool's variable may start with, besides purlis's own under each of its
/// spellings ([`crate::names::ENV_PREFIX`]).
const REFUSED_VAR_PREFIXES: &[&str] = &["LD_", "DYLD_"];

/// **Why `var` may not carry a tool's value**, if it may not.
pub fn refused_var(var: &str) -> Option<String> {
    let mut bytes = var.bytes();
    let named = bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if !named {
        return Some(format!(
            "{var:?} is not a variable's name — letters, digits and '_', not starting with a \
             digit"
        ));
    }
    let upper = var.to_ascii_uppercase();
    if REFUSED_VARS.contains(&upper.as_str())
        || REFUSED_VAR_PREFIXES
            .iter()
            .copied()
            .chain(crate::names::ENV_PREFIX.spellings())
            .any(|prefix| upper.starts_with(prefix))
    {
        return Some(format!(
            "{var} decides which programs run or what they load, so no credential is put in it"
        ));
    }
    None
}

/// **One persona's `tools`**, as `here` (`sandbox.personas.<p>`) in `file` writes them: each
/// tool read, and each thing that is not taken, as one sentence with its key. A tool with any
/// fault is left out whole.
pub fn keyed(
    value: Option<&toml::Value>,
    here: &str,
    file: &str,
    at: &[&str],
) -> (Vec<Tool>, Vec<Refusal>) {
    let mut tools = Vec::new();
    let mut refused = Vec::new();
    let Some(value) = value else {
        return (tools, refused);
    };
    let key_of = |more: &[&str]| -> Vec<String> {
        at.iter()
            .copied()
            .chain(std::iter::once(KEY))
            .chain(more.iter().copied())
            .map(str::to_owned)
            .collect()
    };
    let refuse = |why: String, more: &[&str]| {
        let key = key_of(more);
        let steps: Vec<&str> = key.iter().map(String::as_str).collect();
        Refusal::at(why, &steps)
    };
    let Some(table) = value.as_table() else {
        refused.push(refuse(
            format!(
                "{here}.tools in {file} is not a table, so no tool is run through a vault — \
                 write tools = {{ kubectl = {{ vault = \"…\", as = \"KUBECONFIG file\", key = \
                 \"…\" }} }}"
            ),
            &[],
        ));
        return (tools, refused);
    };
    for (name, tool) in table {
        let named = format!("{here}.tools.{name} in {file}");
        let mut fault =
            |why: String| refused.push(refuse(format!("{named} runs nothing: {why}"), &[name]));
        if let Some(why) = refused_name(name) {
            fault(why);
            continue;
        }
        let Some(tool) = tool.as_table() else {
            fault("it is not a table of vault, as and key".to_owned());
            continue;
        };
        if let Some(other) = tool.keys().find(|key| !TOOL_KEYS.contains(&key.as_str())) {
            fault(format!(
                "{other} is not a key purlis reads — a tool holds vault, then as and key, or \
                 values"
            ));
            continue;
        }
        let text = |key: &str| {
            tool.get(key)
                .and_then(toml::Value::as_str)
                .map(str::trim)
                .filter(|it| !it.is_empty())
                .map(str::to_owned)
        };
        let Some(vault) = text("vault") else {
            fault("it names no vault".to_owned());
            continue;
        };
        if !crate::secrets::registry::name_ok(&vault) {
            fault(format!("{vault:?} is not a vault's name"));
            continue;
        }
        if let Some(values) = tool.get(VALUES_KEY) {
            if let Some(single) = ["key", "as", login::KEY, login::USER_KEY]
                .into_iter()
                .find(|key| tool.contains_key(*key))
            {
                fault(format!(
                    "it has both values and {single} — a tool names its values either with key \
                     and as, for one, or under values, for several, not both"
                ));
                continue;
            }
            match values_of(values) {
                Ok(values) => tools.push(Tool {
                    name: name.clone(),
                    vault,
                    values,
                    login: None,
                }),
                Err(why) => fault(why),
            }
            continue;
        }
        let Some(key) = text("key").filter(|key| !key.chars().any(char::is_control)) else {
            fault("it names no key of the vault".to_owned());
            continue;
        };
        if tool.contains_key(login::KEY) {
            match login_of(tool, &text) {
                // Its keys are handed as any tool's values are: the password's file, then the
                // user name's (D-PE-browser-1).
                Ok((login, user)) => {
                    let values = std::iter::once(Value {
                        var: login::PASS_VAR.to_owned(),
                        handed: Handed::File,
                        key,
                    })
                    .chain(user.map(|user| Value {
                        var: login::USER_VAR.to_owned(),
                        handed: Handed::File,
                        key: user,
                    }))
                    .collect();
                    tools.push(Tool {
                        name: name.clone(),
                        vault,
                        values,
                        login: Some(login),
                    });
                }
                Err(why) => fault(why),
            }
            continue;
        }
        if tool.contains_key(login::USER_KEY) {
            fault(format!(
                "{} is a browser login's key — a tool that logs a browser in names its page with \
                 browser = \"https://…\"",
                login::USER_KEY
            ));
            continue;
        }
        let Some(handed_as) = text("as") else {
            fault(
                "it does not say as what the value is handed — as = \"KUBECONFIG file\"".to_owned(),
            );
            continue;
        };
        let (var, handed) = match handed_as.rsplit_once(' ') {
            Some((var, "file")) => (var.trim(), Handed::File),
            Some((var, "env")) => (var.trim(), Handed::Env),
            _ => {
                fault(format!(
                    "as = {handed_as:?} is neither \"<VARIABLE> file\" nor \"<VARIABLE> env\""
                ));
                continue;
            }
        };
        if let Some(why) = refused_var(var) {
            fault(why);
            continue;
        }
        tools.push(Tool::one(name, &vault, var, handed, &key));
    }
    (tools, refused)
}

/// **A tool's `values`** (T-1): each variable with its key and hand-over, in the order of their
/// variables, or why they are not taken. Refused: not a table, none or more than
/// [`MOST_VALUES`], a variable [`refused_var`] refuses or one written twice in any case, a
/// value that is not a table of key and as, an `as` that is not `env` or `file`, and a key
/// that is empty or holds a control character.
fn values_of(values: &toml::Value) -> Result<Vec<Value>, String> {
    let example = "values = { USER = { key = \"…\", as = \"env\" }, PASS = { key = \"…\", as = \
                   \"file\" } }";
    let Some(table) = values.as_table() else {
        return Err(format!("values is not a table — write {example}"));
    };
    if table.is_empty() {
        return Err(format!("values names no value — write {example}"));
    }
    if table.len() > MOST_VALUES {
        return Err(format!(
            "values names {} values, and a tool is handed at most {MOST_VALUES}",
            table.len()
        ));
    }
    let mut out: Vec<Value> = Vec::with_capacity(table.len());
    for (var, value) in table {
        if let Some(why) = refused_var(var) {
            return Err(why);
        }
        if let Some(twice) = out.iter().find(|one| one.var.eq_ignore_ascii_case(var)) {
            return Err(format!(
                "values names {} and {var}, one variable twice — each value goes in a variable \
                 of its own",
                twice.var
            ));
        }
        let Some(value) = value.as_table() else {
            return Err(format!(
                "values.{var} is not a table of key and as — {var} = {{ key = \"…\", as = \
                 \"file\" }}"
            ));
        };
        if let Some(other) = value.keys().find(|key| !VALUE_KEYS.contains(&key.as_str())) {
            return Err(format!(
                "values.{var}.{other} is not a key purlis reads — a value holds key and as"
            ));
        }
        let text = |key: &str| {
            value
                .get(key)
                .and_then(toml::Value::as_str)
                .map(str::trim)
                .filter(|it| !it.is_empty())
        };
        let Some(key) = text("key").filter(|key| !key.chars().any(char::is_control)) else {
            return Err(format!("values.{var} names no key of the vault"));
        };
        let handed = match text("as") {
            Some("file") => Handed::File,
            Some("env") => Handed::Env,
            Some(other) => {
                return Err(format!(
                    "values.{var} has as = {other:?}, which is neither \"file\" nor \"env\""
                ));
            }
            None => {
                return Err(format!(
                    "values.{var} does not say as what it is handed — as = \"file\" or \"env\""
                ));
            }
        };
        out.push(Value {
            var: var.clone(),
            handed,
            key: key.to_owned(),
        });
    }
    out.sort_by(|a, b| a.var.cmp(&b.var));
    Ok(out)
}

/// A browser login's `browser`, and the key its `user` names, or why the tool is not one
/// ([`login`]).
fn login_of(
    tool: &toml::map::Map<String, toml::Value>,
    text: &dyn Fn(&str) -> Option<String>,
) -> Result<(login::Login, Option<String>), String> {
    if tool.contains_key("as") {
        return Err(
            "a browser login is handed its login by purlis, so it takes no as — write browser, \
             user and key"
                .to_owned(),
        );
    }
    let Some(url) = text(login::KEY) else {
        return Err("browser names no login page — browser = \"https://…\"".to_owned());
    };
    let origin = login::origin_of(&url)?;
    let user = match tool.get(login::USER_KEY) {
        None => None,
        Some(_) => Some(
            text(login::USER_KEY)
                .filter(|user| !user.chars().any(char::is_control))
                .ok_or_else(|| "user names no key of the vault".to_owned())?,
        ),
    };
    Ok((login::Login { url, origin }, user))
}

/// The folder a persona's tools are written in, in the project at `root`: under the app's
/// state, which every chat is denied writing ([`super::Denied::of`]).
pub fn folder(root: &Path, persona: &str) -> PathBuf {
    crate::names::state(root)
        .join("app")
        .join("tools")
        .join(persona)
}

#[cfg(test)]
thread_local! {
    /// A test's own temporary folders, in place of the machine's, for the calls on its thread:
    /// a test's files live in the machine's, so a run that judges by them would refuse them.
    pub(crate) static TEMPS_HERE: std::cell::RefCell<Option<Vec<PathBuf>>> =
        const { std::cell::RefCell::new(None) };
}

/// The folders a sandboxed chat may write wherever it runs: the temporary folders, as written
/// and as the kernel names them.
fn temp_folders() -> Vec<PathBuf> {
    #[cfg(test)]
    if let Some(temps) = TEMPS_HERE.with(|temps| temps.borrow().clone()) {
        return temps;
    }
    let mut out: Vec<PathBuf> = [
        std::env::temp_dir(),
        PathBuf::from("/tmp"),
        PathBuf::from("/var/tmp"),
        PathBuf::from("/var/folders"),
    ]
    .into_iter()
    .collect();
    let real: Vec<PathBuf> = out.iter().map(|dir| super::real(dir)).collect();
    out.extend(real);
    out
}

/// Whether `path` is inside one of `writable`, as written or as the kernel names either.
fn inside_any(path: &Path, writable: &[PathBuf]) -> bool {
    let real = super::real(path);
    writable.iter().any(|dir| {
        let dir_real = super::real(dir);
        [path, real.as_path()]
            .iter()
            .any(|p| p.starts_with(dir) || p.starts_with(&dir_real))
    })
}

/// A tool and the absolute program its shim runs: none (an empty path) for a browser login,
/// which runs no program (#2016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub tool: Tool,
    pub program: PathBuf,
}

impl Ready {
    /// **The name its program starts under**, where that is not the program's own file name:
    /// the name the tool was declared as (#1977). The program that runs is [`Ready::program`]
    /// either way.
    pub fn argv0(&self) -> Option<&str> {
        self.tool.argv0_for(&self.program)
    }
}

/// **A persona's tools, resolved for one chat**: the folder their shims go in, each tool with
/// its program, and why each other one runs nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub persona: String,
    pub folder: PathBuf,
    pub ready: Vec<Ready>,
    pub left: Vec<String>,
}

/// **`persona`'s tools for a chat in the project at `root`**: each found on `search` where no
/// chat writes (the project, `writable` and the temporary folders, as found and resolved), and
/// none that `locks` forbids. `None` where the persona has no tools.
pub fn resolve(
    root: &Path,
    persona: &str,
    tools: &[Tool],
    locks: &super::policy::Locks,
    search: &[PathBuf],
    writable: &[PathBuf],
) -> Option<Resolved> {
    resolve_beside(
        root,
        persona,
        tools,
        locks,
        search,
        writable,
        &temp_folders(),
    )
}

/// **`persona`'s tools for a chat starting in the project at `root`**, as [`resolve`] finds them,
/// of only those the person on this machine allowed with the persona's hosts
/// ([`super::persona::tools_in_force_here`], D-PE-ruling-1). None where none is allowed here:
/// a persona whose tools wait for the Allow gets no shim, and says so in the log.
pub fn for_chat(
    root: &Path,
    personas: &std::collections::BTreeMap<String, super::persona::Grants>,
    persona: &str,
    locks: &super::policy::Locks,
    search: &[PathBuf],
    writable: &[PathBuf],
) -> Option<Resolved> {
    for_chat_beside(
        root,
        personas,
        persona,
        locks,
        search,
        writable,
        &temp_folders(),
    )
}

/// [`for_chat`], with `temps` as the temporary folders.
pub(crate) fn for_chat_beside(
    root: &Path,
    personas: &std::collections::BTreeMap<String, super::persona::Grants>,
    persona: &str,
    locks: &super::policy::Locks,
    search: &[PathBuf],
    writable: &[PathBuf],
    temps: &[PathBuf],
) -> Option<Resolved> {
    let allowed = super::persona::tools_in_force_here(root, personas, Some(persona), locks);
    let listed = super::persona::of(personas, Some(persona)).map_or(0, |grants| grants.tools.len());
    if allowed.is_empty() && listed > 0 && !locks.forbids_persona_tools() {
        tracing::info!(
            "purlis: {persona}'s tools run nothing in this chat: they wait for this machine's \
             Allow of {persona}'s hosts and tools"
        );
    }
    resolve_beside(root, persona, &allowed, locks, search, writable, temps)
}

/// [`resolve`], with `temps` as the temporary folders.
pub(crate) fn resolve_beside(
    root: &Path,
    persona: &str,
    tools: &[Tool],
    locks: &super::policy::Locks,
    search: &[PathBuf],
    writable: &[PathBuf],
    temps: &[PathBuf],
) -> Option<Resolved> {
    if tools.is_empty() {
        return None;
    }
    let mut written: Vec<PathBuf> = vec![root.to_path_buf()];
    written.extend(writable.iter().cloned());
    written.extend(temps.iter().cloned());
    written.extend(any_chat_writes(root));
    let mut ready = Vec::new();
    let mut left = Vec::new();
    for tool in tools {
        if let Some(why) = locks.refuses_tool(persona, &tool.name) {
            left.push(why);
            continue;
        }
        // A browser login runs no program: purlis fills it in the chat's own browser (#2016).
        if tool.login.is_some() {
            ready.push(Ready {
                tool: tool.clone(),
                program: PathBuf::new(),
            });
            continue;
        }
        let wanted = tool.name.as_str();
        // What runs is the program as its links resolve, judged here: nothing a chat could
        // change later is on the way to it.
        let found = found_in(wanted, search, &written);
        if let Some(why) = found
            .as_ref()
            .and_then(|(found, real)| refused_at(tool, found, real))
        {
            left.push(format!(
                "{persona}'s tool {} runs nothing: {why}",
                tool.name
            ));
            continue;
        }
        match found {
            Some((_, program)) => ready.push(Ready {
                tool: tool.clone(),
                program,
            }),
            None => left.push(format!(
                "{persona}'s tool {} runs nothing: no {wanted} was found outside the folders a \
                 chat may write",
                tool.name
            )),
        }
    }
    Some(Resolved {
        persona: persona.to_owned(),
        folder: folder(root, persona),
        ready,
        left,
    })
}

/// **`persona`'s tools as the project at `root` declares them, and those this machine allowed
/// now** (#2560): what a chat started without the sandbox, which no start armed, is held to at
/// the call itself, as a sandboxed chat's start arms it. The declarations are read whether or
/// not the project turned the sandbox on, as a persona's declared browser is
/// ([`super::persona::declared_browser`]); what is allowed is exactly what this machine's Allow
/// of the persona's hosts and tools covers, under policy ([`super::persona::tools_in_force_here`]).
pub fn declared_and_allowed_here(root: &Path, persona: &str) -> (Vec<Tool>, Vec<Tool>) {
    let said = super::Plane::read(root).said();
    let Some(policy) = said.policy.or(said.unset) else {
        return (Vec::new(), Vec::new());
    };
    let locks = super::policy::Locks::of(root);
    let declared = super::persona::of(&policy.personas, Some(persona))
        .map(|grants| grants.tools.clone())
        .unwrap_or_default();
    let allowed =
        super::persona::tools_in_force_here(root, &policy.personas, Some(persona), &locks);
    (declared, allowed)
}

/// **What a brokered `secret exec` from a sandboxed chat asks to run**, as [`declared`] judges
/// it: the vault, each hand-over as `secret exec` spells it (`VAR=key`), and the program as the
/// chat named it.
#[derive(Debug, Clone, Copy)]
pub struct Asked<'a> {
    pub vault: &'a str,
    pub env: &'a [String],
    pub file: &'a [String],
    pub dotenv: &'a [String],
    pub program: &'a str,
}

/// Whether `named`, as a chat named a program, is `ready`'s: its plain name, or its program's
/// absolute path as found or as the kernel names it.
fn names(ready: &Ready, named: &str) -> bool {
    if named == ready.tool.name {
        return true;
    }
    // A browser login is asked for by its name alone: its program is purlis's to run.
    if ready.tool.login.is_some() {
        return false;
    }
    let path = Path::new(named);
    path.is_absolute()
        && (path == ready.program
            || path
                .canonicalize()
                .is_ok_and(|real| ready.program.canonicalize().is_ok_and(|its| its == real)))
}

/// Whether `asked`, the hand-overs a chat asked for one way, are exactly `want`, the ones the
/// tool declares that way: the same ones, each once, in any order.
fn same_hand_overs(asked: &[String], want: &[String]) -> bool {
    let mut asked: Vec<&str> = asked.iter().map(String::as_str).collect();
    let mut want: Vec<&str> = want.iter().map(String::as_str).collect();
    asked.sort_unstable();
    want.sort_unstable();
    asked == want
}

/// **Which of the chat's tools a brokered `secret exec` runs** (D-PE-ruling-2, the operator's
/// ruling of 2026-10-12): `armed` is what the chat's start armed ([`super::Confines::tools`]),
/// each allowed on this machine with its program found where no chat writes. The command runs
/// only where its program is one of them and it asks for exactly that tool's vault and
/// hand-over, nothing more; the run is then of the tool's own program, by its absolute path.
///
/// Anything else is refused with a sentence naming the tool to declare, or why the one declared
/// is not armed here (`listed`, the persona's tools as the project's file lists them now): a
/// chat no longer runs an arbitrary program with its persona's vault. The person's own terminal
/// and a chat the app did not start sandboxed never come here.
pub fn declared<'a>(
    armed: &'a [Ready],
    listed: &[Tool],
    persona: &str,
    asked: &Asked<'_>,
) -> Result<&'a Ready, String> {
    let named = Path::new(asked.program).file_name().map_or_else(
        || asked.program.to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let terminal = "or run the command in your own terminal, where purlis limits nothing";
    let Some(ready) = armed.iter().find(|ready| names(ready, asked.program)) else {
        if let Some(tool) = listed.iter().find(|tool| tool.name == named) {
            return Err(format!(
                "{named} is {persona}'s tool, and it is not armed in this chat: it runs only once \
                 {persona}'s hosts and tools are allowed on this machine (the project's Notice, \
                 or Settings › Sandbox), policy allows persona tools, and {named} is found \
                 outside every folder a chat may write. Start the chat again once it is ({}), {terminal}.",
                tool.whole()
            ));
        }
        if let Some(why) = refused_name(&named) {
            return Err(format!(
                "a chat runs only {persona}'s declared tools with a vault, and {named} cannot be \
                 one: {why}. Run the program a tool names instead, {terminal}."
            ));
        }
        let vault = asked.vault;
        return Err(format!(
            "a chat runs only {persona}'s declared tools with a vault, and {named} is not one, so \
             nothing ran. Declare it in the project's settings, [sandbox.personas.{persona}] \
             tools = {{ {named} = {{ vault = \"{vault}\", as = \"<VARIABLE> file\", key = \
             \"<key>\" }} }} (for several values, values = {{ <VARIABLE> = {{ key = \"<key>\", \
             as = \"file\" }}, … }} in place of as and key), and allow it on this machine; \
             {terminal}."
        ));
    };
    // Exactly the tool's vault and every one of its hand-overs, each the way it is declared,
    // and nothing else: not a subset, not one more, not another pairing of variable and key,
    // not a file where an env was declared, and never a `--dotenv` (T-1, D-PE-ruling-2).
    let exact = asked.vault == ready.tool.vault
        && same_hand_overs(asked.file, &ready.tool.specs(Handed::File))
        && same_hand_overs(asked.env, &ready.tool.specs(Handed::Env))
        && asked.dotenv.is_empty();
    if !exact {
        return Err(format!(
            "{} is {persona}'s tool {}, and a chat runs it with exactly that: run `{}`, or \
             `purlis secret exec {} {} -- {}`; {terminal}.",
            ready.tool.name,
            ready.tool.whole(),
            ready.tool.name,
            ready.tool.vault,
            ready.tool.flags().join(" "),
            ready.tool.name,
        ));
    }
    Ok(ready)
}

/// **What a declared tool's run is**, once [`declared`] took it: the tool's own program by its
/// absolute path, then the chat's arguments. Refused for a browser login, which runs no program:
/// purlis performs it in the chat's own browser (#2016).
pub fn run_words(ready: &Ready, args: &[String]) -> Result<Vec<String>, String> {
    if ready.tool.login.is_some() {
        return Err(crate::secrets::brokered::login_by_the_browser_tool(
            &ready.tool.name,
        ));
    }
    Ok(std::iter::once(ready.program.display().to_string())
        .chain(args.iter().cloned())
        .collect())
}

/// `path`, a `PATH` a brokered tool run starts from, without each folder in the project at
/// `root`, in `writable`, or in a temporary folder: a program the tool runs by its bare name is
/// never one a chat could have put there (D-PE-ruling-2).
pub fn path_outside(path: &str, root: &Path, writable: &[PathBuf]) -> String {
    let mut written: Vec<PathBuf> = vec![root.to_path_buf()];
    written.extend(writable.iter().cloned());
    written.extend(temp_folders());
    path.split(':')
        .filter(|dir| {
            let dir = Path::new(dir);
            dir.is_absolute() && !inside_any(dir, &written)
        })
        .collect::<Vec<_>>()
        .join(":")
}

/// Variables of a chat's environment that make a program load or run code it names: the
/// dynamic loader's, the C library's and the GLib and GTK libraries' module paths, and an
/// interpreter's or runtime's start-up options. A brokered run from a chat starts without them
/// (D-PE-ruling-2), so the chat cannot have a declared tool, or an approved server's exact line,
/// run its own code beside the credential; nor can it have purlis's own program, which applies
/// the run's sandbox on Linux and links those libraries (#2342).
const LOADS_CODE: &[&str] = &[
    "BASH_ENV",
    "ENV",
    "NODE_OPTIONS",
    "NODE_PATH",
    "PYTHONPATH",
    "PYTHONHOME",
    "PYTHONSTARTUP",
    "PERL5OPT",
    "PERL5LIB",
    "PERLLIB",
    "RUBYOPT",
    "RUBYLIB",
    "JAVA_TOOL_OPTIONS",
    "_JAVA_OPTIONS",
    "JDK_JAVA_OPTIONS",
    "GCONV_PATH",
    "LOCPATH",
    "GIO_EXTRA_MODULES",
    "GIO_MODULE_DIR",
    "GTK_MODULES",
    "GTK3_MODULES",
    "GTK_PATH",
    "GTK_EXE_PREFIX",
    "GST_PLUGIN_PATH",
    "GST_PLUGIN_SYSTEM_PATH",
];

/// Prefixes of [`LOADS_CODE`]'s kind: the dynamic loaders' own.
const LOADS_CODE_PREFIXES: &[&str] = &["LD_", "DYLD_"];

/// **Whether `var`, in a chat's environment, makes a program load or run code it names**
/// ([`LOADS_CODE`]): taken out of a brokered run's environment.
pub fn loads_code(var: &str) -> bool {
    LOADS_CODE.contains(&var)
        || LOADS_CODE_PREFIXES
            .iter()
            .any(|prefix| var.starts_with(prefix))
}

/// **The chat's variables a brokered run starts from** (#2047), a declared tool's or an
/// approved server's: where the program is
/// found, the language, time zone and terminal it prints for, the chat's ssh agent (#1350), the
/// certificate authorities it trusts, and the plain values a client reads for where it connects
/// (a database's host, port and name, as a run's tunnel reads them, #1708, and a cloud's
/// region), which an argument could say as well. Nothing else of the chat's environment reaches
/// the run: no `HOME` or `XDG_*` folder, no `GIT_CONFIG_*`, `AWS_CONFIG_FILE` or any other
/// variable that names a file of settings a chat could have written, and no program to run
/// (`PAGER`, `EDITOR`, `GIT_SSH_COMMAND`). What the tool is handed comes from its declaration,
/// and the run sets its own proxy, temp folder and home ([`home_of_its_own`]). An approved
/// server's run starts with the variables its approved entry sets too.
const A_RUN_STARTS_FROM: &[&str] = &[
    "PATH",
    "LANG",
    "TZ",
    "TERM",
    "COLORTERM",
    "NO_COLOR",
    "COLUMNS",
    "LINES",
    "USER",
    "LOGNAME",
    "SSH_AUTH_SOCK",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NODE_EXTRA_CA_CERTS",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "AWS_CA_BUNDLE",
    "PGHOST",
    "PGHOSTADDR",
    "PGPORT",
    "PGDATABASE",
    "PGUSER",
    "PGSSLMODE",
    "MYSQL_HOST",
    "MYSQL_TCP_PORT",
    "REDIS_HOST",
    "REDIS_PORT",
    "AWS_REGION",
    "AWS_DEFAULT_REGION",
];

/// **Whether a brokered run starts with the chat's `var`** ([`A_RUN_STARTS_FROM`], and each
/// `LC_*`).
pub fn a_run_starts_from(var: &str) -> bool {
    A_RUN_STARTS_FROM.contains(&var) || var.starts_with("LC_")
}

/// **A brokered run's home** (#2047): `HOME` and the `XDG_*` folders, each in the run's own
/// temp folder `tmp`, so whatever a program reads or writes as its settings is the run's alone.
pub fn home_of_its_own(tmp: &Path) -> Vec<(String, String)> {
    [
        ("HOME", ""),
        ("XDG_CONFIG_HOME", ".config"),
        ("XDG_CACHE_HOME", ".cache"),
        ("XDG_DATA_HOME", ".local/share"),
        ("XDG_STATE_HOME", ".local/state"),
    ]
    .iter()
    .map(|(var, below)| {
        let at = if below.is_empty() {
            tmp.to_path_buf()
        } else {
            tmp.join(below)
        };
        ((*var).to_owned(), at.display().to_string())
    })
    .collect()
}

/// Where a chat's shims and Claude Code's env file were written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// The folder that goes first on the chat's `PATH`.
    pub bin: PathBuf,
    /// The file Claude Code reads before each command (`CLAUDE_ENV_FILE`).
    pub env_file: PathBuf,
}

/// The variable that hands [`crate::start::with_chat_path`] the folder to put first on the
/// chat's `PATH`. Taken out of the chat's environment there: the chat never sees it.
pub const FIRST_ON_PATH: &str = "PURLIS_TOOLS_FIRST_ON_PATH";

/// The variable Claude Code reads a file of shell lines from before each command it runs.
pub const CLAUDE_ENV_FILE: &str = "CLAUDE_ENV_FILE";

impl Written {
    /// What a chat of `harness` gains: the shims' folder first on its `PATH`, and for Claude
    /// Code the file that puts it first again before each command.
    pub fn env(&self, harness: crate::harness::Harness) -> Vec<(String, String)> {
        let mut env = vec![(FIRST_ON_PATH.to_owned(), self.bin.display().to_string())];
        if harness == crate::harness::Harness::ClaudeCode {
            env.push((
                CLAUDE_ENV_FILE.to_owned(),
                self.env_file.display().to_string(),
            ));
        }
        env
    }
}

/// `env`, a chat's environment, with the folder [`FIRST_ON_PATH`] names first on its `PATH`
/// (`path` where `env` names none), and that variable taken out.
pub fn first_on_path(
    mut env: Vec<(String, String)>,
    path: Option<String>,
) -> Vec<(String, String)> {
    let Some(at) = env.iter().position(|(name, _)| name == FIRST_ON_PATH) else {
        return env;
    };
    let (_, first) = env.remove(at);
    env.retain(|(name, _)| name != FIRST_ON_PATH);
    // No `PATH` to put it in front of: the chat keeps the one it inherits, and runs the
    // programs themselves, as it would with no tools.
    let Some(had) = env
        .iter()
        .position(|(name, _)| name == "PATH")
        .map(|at| env.remove(at).1)
        .or(path)
    else {
        return env;
    };
    let rest = had
        .split(':')
        .filter(|dir| !dir.is_empty() && *dir != first);
    let joined = std::iter::once(first.as_str())
        .chain(rest)
        .collect::<Vec<_>>()
        .join(":");
    env.push(("PATH".to_owned(), joined));
    env.sort();
    env
}

/// `text` as one POSIX shell word, whatever is in it.
fn quoted(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// The shim for `ready`, run by `purlis`.
pub fn shim(purlis: &Path, persona: &str, ready: &Ready) -> String {
    if ready.tool.login.is_some() {
        // A browser login runs nothing here: purlis performs it in the chat's own browser, and
        // the shim only says how to ask for it (#2016).
        return format!(
            "#!/bin/sh\n# Written by purlis as a chat of {persona} starts (#2016). `{name}` is a \
             browser login:\n# purlis fills it from the {vault} vault in the chat's own browser, \
             asked through\n# the browser tools. Edits here are overwritten.\n\
             echo {said} >&2\nexit 2\n",
            name = ready.tool.name,
            vault = ready.tool.vault,
            said = quoted(&format!(
                "purlis: {}",
                crate::secrets::brokered::login_by_the_browser_tool(&ready.tool.name)
            )),
        );
    }
    let words: Vec<String> = std::iter::once(purlis.display().to_string())
        .chain(ready.tool.secret_exec(&ready.program))
        .map(|word| quoted(&word))
        .collect();
    // The chat's hook channel under each spelling the window reads: where neither is set, the
    // app cannot be asked from here.
    let rest = crate::hookwire::SOCKET_ENV
        .strip_prefix(crate::names::ENV_PREFIX.write)
        .unwrap_or(crate::hookwire::SOCKET_ENV);
    let channel: String = crate::names::ENV_PREFIX
        .spellings()
        .map(|prefix| format!("${{{prefix}{rest}:-}}"))
        .collect();
    format!(
        "#!/bin/sh\n# Written by purlis as a chat of {persona} starts (#1407). A plain `{name}` in \
         that chat runs\n# through `purlis secret exec`, which hands it {var} from the {vault} \
         vault: the chat\n# never reads the vault. Edits here are overwritten.\n\
         if [ -z \"{channel}\" ]; then\n\
         # No app to ask from here: inside a `secret exec` run already, which handed the value \
         on, or\n# outside the chat. The program itself, with nothing added.\n\
         {named}exec {program} \"$@\"\nfi\nexec {words} \"$@\"\n",
        name = ready.tool.name,
        var = ready
            .tool
            .values
            .iter()
            .map(|value| value.var.as_str())
            .collect::<Vec<_>>()
            .join(" and "),
        vault = ready.tool.vault,
        program = quoted(&ready.program.display().to_string()),
        words = words.join(" "),
        // Under the declared name (#1977) where the shell can name it: POSIX `exec` has no
        // `-a` (dash's has none), so it is tried in a subshell first.
        named = ready.argv0().map_or_else(String::new, |name| {
            let name = quoted(name);
            format!(
                "if (exec -a {name} true) 2>/dev/null; then\n\
                 exec -a {name} {program} \"$@\"\nfi\n",
                program = quoted(&ready.program.display().to_string()),
            )
        }),
    )
}

/// The file Claude Code reads before each command: the shims' folder `bin` first on `PATH`.
pub fn claude_env(bin: &Path) -> String {
    format!(
        "# Written by purlis as a chat starts (#1407). Claude Code reads this before each command\n\
         # (CLAUDE_ENV_FILE): the persona's tools go first, after the shell's own start files.\n\
         PATH={}:\"$PATH\"\nexport PATH\n",
        quoted(&bin.display().to_string())
    )
}

impl Resolved {
    /// **Writes the shims**, each running `purlis`, and Claude Code's env file, replacing every
    /// shim of an earlier start: none where no tool is ready.
    #[cfg(unix)]
    pub fn write(&self, purlis: &Path) -> std::io::Result<Option<Written>> {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let bin = self.folder.join("bin");
        let env_file = self.folder.join("claude-env.sh");
        let refused = |path: &Path| {
            let path = path.to_path_buf();
            move |err: std::io::Error| {
                std::io::Error::new(err.kind(), format!("{}: {err}", path.display()))
            }
        };
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&bin)
            .map_err(refused(&bin))?;
        // Every shim an earlier start wrote that is not one now.
        for entry in std::fs::read_dir(&bin).map_err(refused(&bin))? {
            let entry = entry.map_err(refused(&bin))?;
            let name = entry.file_name();
            let current = self
                .ready
                .iter()
                .any(|one| std::ffi::OsStr::new(&one.tool.name) == name);
            if !current {
                std::fs::remove_file(entry.path()).map_err(refused(&entry.path()))?;
            }
        }
        if self.ready.is_empty() {
            return Ok(None);
        }
        let put = |path: &Path, text: &str, mode: u32| -> std::io::Result<()> {
            let beside = path.with_extension(format!("purlis-{}", std::process::id()));
            std::fs::write(&beside, text).map_err(refused(path))?;
            std::fs::set_permissions(&beside, std::fs::Permissions::from_mode(mode))
                .map_err(refused(path))?;
            std::fs::rename(&beside, path)
                .inspect_err(|_| {
                    let _ = std::fs::remove_file(&beside);
                })
                .map_err(refused(path))
        };
        for ready in &self.ready {
            put(
                &bin.join(&ready.tool.name),
                &shim(purlis, &self.persona, ready),
                0o700,
            )?;
        }
        put(&env_file, &claude_env(&bin), 0o600)?;
        Ok(Some(Written { bin, env_file }))
    }

    /// Off unix there are no shims: a shim is a POSIX shell script.
    #[cfg(not(unix))]
    pub fn write(&self, _purlis: &Path) -> std::io::Result<Option<Written>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "a persona's tools are POSIX shell scripts, and this platform has no answer yet",
        ))
    }
}

/// **What a sandboxed chat running as the persona gains from `resolved`**, its shims written
/// now ([`Resolved::write`]): the variables [`Written::env`] names for `harness`. Nothing where
/// no tool is ready, no `purlis` is found to run, or the shims cannot be written: the chat then
/// runs each program itself, as it would with no tools, and the reason is logged.
pub fn armed(resolved: &Resolved, harness: crate::harness::Harness) -> Vec<(String, String)> {
    for why in &resolved.left {
        tracing::warn!("purlis: {why}");
    }
    let Some(purlis) = purlis_binary() else {
        tracing::warn!(
            "purlis: {}'s tools run nothing in this chat: no purlis command line was found \
             beside the app",
            resolved.persona
        );
        return Vec::new();
    };
    match resolved.write(&purlis) {
        Ok(Some(written)) => written.env(harness),
        Ok(None) => Vec::new(),
        Err(why) => {
            tracing::warn!(
                "purlis: {}'s tools run nothing in this chat: their shims were not written \
                 ({why})",
                resolved.persona
            );
            Vec::new()
        }
    }
}

/// **The `purlis` a shim runs**: `PURLIS_BINARY` where the app was given one, else the command
/// line beside the running program, under each of its names. None where neither is there.
pub fn purlis_binary() -> Option<PathBuf> {
    purlis_binary_in(|name| crate::envvar::var_os(name).map(|v| v.to_string_lossy().into_owned()))
}

/// [`purlis_binary`], with `PURLIS_BINARY` read from `env`.
pub fn purlis_binary_in(env: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let named = crate::envvar::lookup("PURLIS_BINARY", env).map(PathBuf::from);
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.to_owned()));
    let beside = crate::cliname::INSTALLED
        .iter()
        .filter_map(move |name| Some(dir.as_ref()?.join(name)));
    named
        .into_iter()
        .chain(beside)
        .find(|path| path.is_absolute() && path.is_file())
}

// Shims are POSIX shell scripts, and the tests make runnable files with unix modes.
#[cfg(all(test, unix))]
mod tests;

#[cfg(all(test, unix))]
mod login_tests;
