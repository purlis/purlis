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
//! ([`super::persona::digest_of`]). Until then, and after any change, no shim that runs anything is
//! written (on a runner, a tool whose program is absent has one that only says so, #4373) and the
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

pub mod args;
pub mod command_flags;
pub mod fill;
pub mod host;
pub mod login;
pub mod mint;
pub mod repo;

/// The key in a persona's table that holds its tools.
pub const KEY: &str = "tools";

/// The key of a tool's table that names several values ([`Value`]).
pub const VALUES_KEY: &str = "values";

/// The keys one tool's table holds: `as` and `key`, or `values`, for a program's; `browser`,
/// `user` and `form` for a browser login's ([`login`]); `fill` and `card` for a secret-fill
/// line's ([`fill`]); `args` for any program's ([`args`]); `mint` and
/// `app` for a GitHub App's token's ([`mint`]); `host` for a forge's ([`host`]).
const TOOL_KEYS: [&str; 13] = [
    "vault",
    "as",
    "key",
    VALUES_KEY,
    login::KEY,
    login::USER_KEY,
    login::FORM_KEY,
    args::KEY,
    mint::KEY,
    mint::APP_KEY,
    host::KEY,
    fill::KEY,
    fill::CARD_KEY,
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
    /// **The arguments its program runs with** ([`args`], #3421): each a pattern for one whole
    /// argument. None runs it with any; the built-in refusals apply either way.
    pub args: Option<Vec<String>>,
    /// Where the tool's token is minted by a GitHub App ([`mint`], #2053): the App. Its key is
    /// the tool's one value, handed in [`mint::VAR`] as the token minted from it.
    pub mint: Option<mint::Mint>,
    /// **The forge host its run reaches** ([`host`], #3869), for a tool whose program is `gh`
    /// or `glab`: none reaches `github.com` or `gitlab.com`.
    pub host: Option<String>,
    /// **Where the line is a secret-fill line** ([`fill`], #5116): its key and the one origin it
    /// goes to. Such a line runs no program and holds no [`Self::values`].
    pub fill: Option<fill::Fill>,
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
            args: None,
            mint: None,
            host: None,
            fill: None,
        }
    }

    /// **Whether the line runs no program**: a browser login, which purlis performs in the
    /// chat's own browser, or a secret-fill line, which purlis fills there.
    pub fn runs_no_program(&self) -> bool {
        self.login.is_some() || self.fill.is_some()
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
        let line = self.declared_line();
        let line = match &self.args {
            Some(patterns) => format!("{line}{}", args::spelled(patterns)),
            None => line,
        };
        match &self.host {
            Some(host) => format!("{line}{}", host::spelled(host)),
            None => line,
        }
    }

    /// [`Self::whole`] without its `args`: the line a tool without them has always had, so an
    /// Allow made before `args` was read still stands.
    fn declared_line(&self) -> String {
        if let Some(fill) = &self.fill {
            let line = format!(
                "{} fills {:?} from vault {} into fields on {}",
                self.name, fill.key, self.vault, fill.origin
            );
            return if fill.card {
                format!("{line}, a card: Allow once each time")
            } else {
                line
            };
        }
        if let Some(login) = &self.login {
            let user = self
                .key_in(login::USER_VAR)
                .map_or_else(String::new, |user| format!("{user:?} and "));
            let line = format!(
                "{} logs in to {} with {user}{:?} from vault {} in a browser",
                self.name,
                login.url,
                self.key_in(login::PASS_VAR).unwrap_or_default(),
                self.vault
            );
            // A login with no form of its own keeps the line, and the Allow, it had (#4995).
            return match &login.form {
                Some(form) => format!("{line}, typed on {form}"),
                None => line,
            };
        }
        if let (Some(mint), [one]) = (&self.mint, self.values.as_slice()) {
            return format!(
                "{} with a token GitHub App {} mints from {:?} in vault {} as {} env",
                self.name, mint.app, one.key, self.vault, one.var
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
    if tool.runs_no_program() {
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
    walked(program, written).map(|(real, _)| real)
}

/// [`held_outside`], with the folder each link on the way sits in, as it is with every link
/// before it resolved, in the order they are met (#6321).
fn walked(program: &Path, written: &[PathBuf]) -> Option<(PathBuf, Vec<PathBuf>)> {
    use std::path::Component;
    let mut done = PathBuf::new();
    let mut folders = Vec::new();
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
        folders.push(done.clone());
        let target = std::fs::read_link(&next).ok()?;
        start(&target, &mut done, &mut rest);
    }
    (!inside_any(&done, written)).then_some((done, folders))
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
    if tool.runs_no_program() {
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
    // fetch and send over the network, the forge command lines (gh, glab) among them, which
    // make whatever request their arguments name
    "curl",
    "wget",
    "wget2",
    "http",
    "https",
    "httpie",
    "xh",
    "xhs",
    "aria2c",
    "nc",
    "ncat",
    "netcat",
    "socat",
    "ssh",
    "scp",
    "sftp",
    "rsync",
    "gh",
    "glab",
    // copy, encode or rewrite what they are given
    "cat",
    "cp",
    "tee",
    "dd",
    "sed",
    "base64",
    "xxd",
    "od",
    "hexdump",
    "openssl",
    "gzip",
    "tar",
    "zip",
    // the bundled openssl MySQL's test package ships under its own name (#6329)
    "my_openssl",
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
    if tool.runs_no_program() {
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
    let mut with_args: Vec<(String, Vec<String>)> = Vec::new();
    let mut with_host: Vec<(String, String)> = Vec::new();
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
        // Its `args` (#3421), set on the tool once it is read whole, below.
        match tool.get(args::KEY) {
            None => {}
            Some(_) if tool.contains_key(login::KEY) => {
                fault(
                    "a browser login runs no program, so it takes no args — purlis fills its page"
                        .to_owned(),
                );
                continue;
            }
            Some(_) if tool.contains_key(fill::KEY) => {
                fault(
                    "a secret-fill line runs no program, so it takes no args — purlis fills its \
                     field"
                        .to_owned(),
                );
                continue;
            }
            Some(value) => match args::of(value) {
                Ok(patterns) => with_args.push((name.clone(), patterns)),
                Err(why) => {
                    fault(why);
                    continue;
                }
            },
        }
        // Its forge `host` (#3869), set on the tool once it is read whole, below.
        match tool.get(host::KEY) {
            None => {}
            Some(_) if tool.contains_key(login::KEY) => {
                fault("a browser login reaches its declared page, so it takes no host".to_owned());
                continue;
            }
            Some(_) if tool.contains_key(fill::KEY) => {
                fault(
                    "a secret-fill line goes to its origin alone, so it takes no host".to_owned(),
                );
                continue;
            }
            Some(value) => match host::of(value) {
                Ok(host) => with_host.push((name.clone(), host)),
                Err(why) => {
                    fault(why);
                    continue;
                }
            },
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
            if let Some(single) = [
                "key",
                "as",
                login::KEY,
                login::USER_KEY,
                login::FORM_KEY,
                mint::KEY,
                mint::APP_KEY,
                fill::KEY,
                fill::CARD_KEY,
            ]
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
                    args: None,
                    mint: None,
                    host: None,
                    fill: None,
                }),
                Err(why) => fault(why),
            }
            continue;
        }
        let Some(key) = text("key").filter(|key| !key.chars().any(char::is_control)) else {
            fault("it names no key of the vault".to_owned());
            continue;
        };
        // A secret-fill line (#5116): its key goes into fields on one origin, and no run is
        // handed it.
        if tool.contains_key(fill::KEY) {
            match fill::of(tool, key, &text) {
                Ok(fill) => tools.push(Tool {
                    name: name.clone(),
                    vault,
                    values: Vec::new(),
                    login: None,
                    args: None,
                    mint: None,
                    host: None,
                    fill: Some(fill),
                }),
                Err(why) => fault(why),
            }
            continue;
        }
        if tool.contains_key(fill::CARD_KEY) {
            fault(format!(
                "{} marks a secret-fill line's key as a card's — a line that fills names its \
                 origin with fill = \"https://…\"",
                fill::CARD_KEY
            ));
            continue;
        }
        if tool.contains_key(mint::KEY) || tool.contains_key(mint::APP_KEY) {
            // The token minted from the key is handed in GH_TOKEN, never the key (#2053).
            match mint::of(tool, &text) {
                Ok(mint) => tools.push(Tool {
                    mint: Some(mint),
                    ..Tool::one(name, &vault, mint::VAR, Handed::Env, &key)
                }),
                Err(why) => fault(why),
            }
            continue;
        }
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
                        args: None,
                        mint: None,
                        host: None,
                        fill: None,
                    });
                }
                Err(why) => fault(why),
            }
            continue;
        }
        if tool.contains_key(login::FORM_KEY) {
            fault(format!(
                "{} is a browser login's key — a tool that logs a browser in names its page with \
                 browser = \"https://…\"",
                login::FORM_KEY
            ));
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
    for tool in &mut tools {
        if let Some((_, patterns)) = with_args.iter().find(|(name, _)| *name == tool.name) {
            tool.args = Some(patterns.clone());
        }
        if let Some((_, host)) = with_host.iter().find(|(name, _)| *name == tool.name) {
            tool.host = Some(host.clone());
        }
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
    // The form's origin, where it is not the page's own (#4995).
    let form = match tool.get(login::FORM_KEY) {
        None => None,
        Some(_) => {
            let form = text(login::FORM_KEY)
                .ok_or_else(|| "form names no origin — form = \"https://…\"".to_owned())?;
            Some(login::form_of(&form)?).filter(|form| *form != origin)
        }
    };
    Ok((login::Login { url, origin, form }, user))
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
    /// The tools whose program is nowhere on this machine, by name.
    pub absent: Vec<String>,
    /// The tools of [`Resolved::absent`] nobody allowed (#4373): each says the Allow is wanted
    /// too, and its shim runs a program of its name that has come since the chat started.
    pub unallowed: Vec<String>,
    /// On a runner, each tool of [`Resolved::absent`] and what its name says in the chat (#4354).
    pub refusing: Vec<(String, String)>,
    /// Nothing is allowed this chat, and it only says what is absent (#4373). Its shims go in a
    /// folder of their own, named by what they say ([`refusals_folder`]): the persona's folder
    /// holds the shims of another chat's Allow, which this chat is never to find first on its
    /// `PATH`.
    pub only_refuses: bool,
}

/// **The variable a pool's runner is made with, naming the image it runs** (#4354):
/// [`crate::dockerpool::Runner::create_args`], and `operator/src/upstream.rs` for a cluster's. It
/// is read by the runner's own host and never inherited by a chat
/// ([`crate::hookwire::NOT_INHERITED`]); a chat sees the name only in [`not_in_the_image`]. A
/// runner made before it is told "this pool's image".
pub const POOL_IMAGE_VAR: &str = "PURLIS_POOL_IMAGE";

/// **What a persona's declared tool says on a runner whose image has no such program** (#4354),
/// naming the program and, where this runner was told it, the pool's image.
pub fn not_in_the_image(persona: &str, name: &str, image: Option<&str>) -> String {
    let image = image.map_or_else(
        || "this pool's image".to_owned(),
        |image| format!("this pool's image, {image},"),
    );
    let or_own = if matches!(name, "gh" | "glab") {
        ", or run the pool on purlis's own runner image, which carries it"
    } else {
        ""
    };
    format!(
        "{name} is {persona}'s tool, and {image} has no {name}, so nothing ran. Add {name} to the \
         pool's image{or_own} (docs/runner-image.md)."
    )
}

/// **Where a persona's Allow is asked, as a refusal names it** (ADR 0121, amended 2026-11-12):
/// its row in Asks, which the ✋ counts, and Settings › Sandbox. A chat refused a tool for the
/// want of it says so, and the person finds it from the ✋ alone.
pub const ALLOW_IS_ASKED: &str = "\
    the persona's row in Asks, under the ✋ in the purlis window, or Settings › Sandbox";

/// **What a persona's tool nobody allowed says on a runner whose image has no such program**
/// (#4373): [`not_in_the_image`], and that it runs only once the persona's hosts and tools are
/// allowed on the desktop that starts the chat, as the window names that Allow.
pub fn neither_allowed_nor_in_the_image(persona: &str, name: &str, image: Option<&str>) -> String {
    format!(
        "{} It also runs only once {persona}'s hosts and tools are allowed on the desktop that \
         starts the chat ({ALLOW_IS_ASKED}).",
        not_in_the_image(persona, name, image)
    )
}

/// **The folder the shims of a chat that only refuses go in** (#4373), beside the persona's
/// `folder`: named by the digest of what they say, so it holds those shims and nothing else, and
/// two such chats that say the same share one.
fn refusals_folder(folder: &Path, refusing: &[(String, String)], unallowed: &[String]) -> PathBuf {
    use sha2::Digest as _;
    let mut digest = sha2::Sha256::new();
    for (name, said) in refusing {
        digest.update(name.as_bytes());
        digest.update([0]);
        digest.update(said.as_bytes());
        digest.update([0, u8::from(unallowed.contains(name)), 0]);
    }
    let digest = digest.finalize();
    folder
        .join("refusing")
        .join(crate::runnerlink::hex_of(&digest[..16]))
}

/// The image this runner was told it runs ([`POOL_IMAGE_VAR`]).
fn pool_image() -> Option<String> {
    crate::envvar::var(POOL_IMAGE_VAR).filter(|image| !image.is_empty())
}

/// **Why `program`, as a runner chat asked for it, is refused for want of it in the image**
/// (#4354): where it is a tool `listed` for `persona`, not armed, with no such program anywhere
/// on this machine. `None` for anything else, which [`declared`] judges.
pub fn absent_from_the_image(
    armed: &[Ready],
    listed: &[Tool],
    persona: &str,
    program: &str,
) -> Option<String> {
    let named = Path::new(program).file_name()?.to_string_lossy();
    let declared = listed
        .iter()
        .find(|tool| tool.name == named && !tool.runs_no_program())?;
    let is_absent = !armed.iter().any(|ready| names(ready, program))
        && found_in(&declared.name, &search_dirs(), &[]).is_none();
    is_absent.then(|| not_in_the_image(persona, &declared.name, pool_image().as_deref()))
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
/// a persona whose tools wait for the Allow gets no shim, and says so in the log. On a runner,
/// `from_desktop` is what the chat's start says its desktop's person allowed
/// ([`super::persona::tools_its_desktop_allowed`], #4071).
pub fn for_chat(
    root: &Path,
    personas: &std::collections::BTreeMap<String, super::persona::Grants>,
    persona: &str,
    locks: &super::policy::Locks,
    search: &[PathBuf],
    writable: &[PathBuf],
    from_desktop: Option<&str>,
) -> Option<Resolved> {
    for_chat_beside(
        root,
        personas,
        persona,
        locks,
        search,
        writable,
        &temp_folders(),
        from_desktop,
    )
}

/// [`for_chat`], with `temps` as the temporary folders.
#[allow(clippy::too_many_arguments)]
pub(crate) fn for_chat_beside(
    root: &Path,
    personas: &std::collections::BTreeMap<String, super::persona::Grants>,
    persona: &str,
    locks: &super::policy::Locks,
    search: &[PathBuf],
    writable: &[PathBuf],
    temps: &[PathBuf],
    from_desktop: Option<&str>,
) -> Option<Resolved> {
    for_chat_on(
        root,
        personas,
        persona,
        locks,
        search,
        writable,
        temps,
        from_desktop,
        pool_image().as_deref(),
    )
}

/// [`for_chat_beside`], with `image` as what this runner was told it runs: only a pool's runner is
/// told, so it says a chat is on one where its start carries no Allow from a desktop (#4373).
#[allow(clippy::too_many_arguments)]
pub(crate) fn for_chat_on(
    root: &Path,
    personas: &std::collections::BTreeMap<String, super::persona::Grants>,
    persona: &str,
    locks: &super::policy::Locks,
    search: &[PathBuf],
    writable: &[PathBuf],
    temps: &[PathBuf],
    from_desktop: Option<&str>,
    image: Option<&str>,
) -> Option<Resolved> {
    let mut allowed = super::persona::tools_in_force_here(root, personas, Some(persona), locks);
    if let (true, Some(digest)) = (allowed.is_empty(), from_desktop) {
        allowed = super::persona::tools_its_desktop_allowed(root, personas, persona, locks, digest);
    }
    let listed = super::persona::of(personas, Some(persona)).map_or(0, |grants| grants.tools.len());
    if allowed.is_empty() && listed > 0 && !locks.forbids_persona_tools() {
        tracing::info!(
            "purlis: {persona}'s tools run nothing in this chat: they wait for this machine's \
             Allow of {persona}'s hosts and tools"
        );
    }
    let mut resolved = resolve_beside(root, persona, &allowed, locks, search, writable, temps);
    // Only on a runner is a program's absence the image's: on the person's own machine the log
    // says why, as it does for any tool left out.
    if from_desktop.is_none() && image.is_none() {
        return resolved;
    }
    // A tool nobody allowed is found for this and no more: it says the same sentence, and is
    // never ready, so no value flows through its shim (#4373).
    let unallowed: Vec<Tool> = super::persona::of(personas, Some(persona))
        .map(|grants| {
            grants
                .tools
                .iter()
                .filter(|tool| !allowed.contains(tool))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if let Some(unallowed) =
        resolve_beside(root, persona, &unallowed, locks, search, writable, temps)
            .filter(|unallowed| !unallowed.absent.is_empty())
    {
        let into = resolved.get_or_insert_with(|| Resolved {
            persona: persona.to_owned(),
            folder: unallowed.folder.clone(),
            ready: Vec::new(),
            left: Vec::new(),
            absent: Vec::new(),
            unallowed: Vec::new(),
            refusing: Vec::new(),
            only_refuses: true,
        });
        into.absent.extend(unallowed.absent.iter().cloned());
        into.unallowed.extend(unallowed.absent);
    }
    if let Some(resolved) = &mut resolved {
        resolved.refuse_what_is_absent(image);
        if resolved.only_refuses {
            resolved.folder =
                refusals_folder(&resolved.folder, &resolved.refusing, &resolved.unallowed);
        }
    }
    resolved
}

/// Every folder [`resolve_beside`] leaves out of the search for a program: the project, what
/// the chat may write, the temporary folders and what any chat in the project may write.
fn all_written(root: &Path, writable: &[PathBuf], temps: &[PathBuf]) -> Vec<PathBuf> {
    let mut written: Vec<PathBuf> = vec![root.to_path_buf()];
    written.extend(writable.iter().cloned());
    written.extend(temps.iter().cloned());
    written.extend(any_chat_writes(root));
    written
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
    let written = all_written(root, writable, temps);
    let mut ready = Vec::new();
    let mut left = Vec::new();
    let mut absent = Vec::new();
    for tool in tools {
        if let Some(why) = locks.refuses_tool(persona, &tool.name) {
            left.push(why);
            continue;
        }
        // A secret-fill line is never armed: it runs nothing and is handed nothing (#5116).
        if tool.fill.is_some() {
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
            None => {
                // Nowhere at all, as distinct from only where a chat could have put it.
                if found_in(wanted, search, &[]).is_none() {
                    absent.push(tool.name.clone());
                }
                left.push(format!(
                    "{persona}'s tool {} runs nothing: no {wanted} was found outside the folders \
                     a chat may write",
                    tool.name
                ));
            }
        }
    }
    Some(Resolved {
        persona: persona.to_owned(),
        folder: folder(root, persona),
        ready,
        left,
        absent,
        unallowed: Vec::new(),
        refusing: Vec::new(),
        only_refuses: false,
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

/// **The digest of what the person on this machine allowed of `persona`**, for a start of it on
/// a runner to carry (#4071, [`super::persona::digest_allowed_here`]), under policy.
pub fn allowed_digest_here(root: &Path, persona: &str) -> Option<String> {
    let said = super::Plane::read(root).said();
    let policy = said.policy.or(said.unset)?;
    super::persona::digest_allowed_here(
        root,
        &policy.personas,
        persona,
        &super::policy::Locks::of(root),
    )
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
                 {persona}'s hosts and tools are allowed on this machine ({ALLOW_IS_ASKED}), \
                 policy allows persona tools, and {named} is found \
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
    // A secret-fill line is never run, however it came to be armed (#5116).
    if ready.tool.fill.is_some() {
        return Err(format!(
            "{} is {persona}'s secret-fill line: it runs nothing, and purlis fills its key in the \
             chat's own browser through browser_fill_secret",
            ready.tool.name
        ));
    }
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

/// Whether `program`, as a chat named it, is a tool of its persona's in any standing: armed for
/// the chat, or listed by the project now though not armed. Such a program is never one a Run
/// grant is asked for (ADR 0112): a declared tool behaves exactly as before, and one a teammate
/// declared runs nothing here until this machine's own Allow.
pub fn is_declared(armed: &[Ready], listed: &[Tool], program: &str) -> bool {
    let named = Path::new(program)
        .file_name()
        .map_or_else(|| program.to_owned(), |n| n.to_string_lossy().into_owned());
    // Whatever its case: macOS finds `CURL` as `curl`, and no spelling escapes a line.
    armed
        .iter()
        .any(|ready| names(ready, program) || ready.tool.name.eq_ignore_ascii_case(&named))
        || listed
            .iter()
            .any(|tool| tool.name.eq_ignore_ascii_case(&named))
}

/// Whether a line written for `program` limits its arguments, which no grant for any arguments may
/// then go past (#5245): a teammate's narrowing holds from the chat's next run.
pub fn limits_its_arguments(listed: &[Tool], program: &str) -> bool {
    let named = Path::new(program)
        .file_name()
        .map_or_else(|| program.to_owned(), |n| n.to_string_lossy().into_owned());
    listed
        .iter()
        .any(|tool| tool.name.eq_ignore_ascii_case(&named) && tool.args.is_some())
}

/// **Programs no Run grant is offered for, though a declared tool may be one** (ADR 0112, #5239):
/// each runs other programs as its arguments, its files or its prompt say, which [`REFUSED_NAMES`]
/// keeps off every tool. A person who declares one chose it in a reviewed line; a person asked in a
/// notice by a chat should not be asked. Not a complete list: every Run grant's notice says a
/// program may run others.
const NOT_FOR_A_RUN_GRANT: &[&str] = &[
    // run what their arguments name
    "find",
    "gfind",
    "fd",
    "apply",
    "lockf",
    "taskset",
    "ionice",
    "unshare",
    "nsenter",
    "pkexec",
    "at",
    "batch",
    "crontab",
    "rlwrap",
    "ssh-agent",
    "xcrun",
    "toybox",
    "git",
    "git-upload-pack",
    "git-receive-pack",
    "screen",
    "tmux",
    "m4",
    "bm4",
    "libtool",
    // build and run what their files say, files a chat writes
    "make",
    "gmake",
    "bmake",
    "cmake",
    "ninja",
    "rake",
    "ant",
    "mvn",
    "gradle",
    "bazel",
    "just",
    "npm",
    "npx",
    "yarn",
    "pnpm",
    "corepack",
    "pip",
    "pip3",
    "pipx",
    "uv",
    "uvx",
    "poetry",
    "gem",
    "bundle",
    "cargo",
    "rustc",
    "rustup",
    "go",
    "gcc",
    "g++",
    "cc",
    "c++",
    "clang",
    "clang++",
    "c89",
    "c99",
    "swiftc",
    "gfortran",
    "ld",
    "docker",
    "podman",
    "nerdctl",
    "swift",
    "java",
    "javac",
    "dotnet",
    // drive a program, or run commands from their own prompt
    "gdb",
    "lldb",
    "strace",
    "ltrace",
    "dtrace",
    "dtruss",
    "vi",
    "vim",
    "nvim",
    "view",
    "ex",
    "emacs",
    "nano",
    "ed",
    "less",
    "more",
    "man",
    "ftp",
    "sqlite3",
    "tcpdump",
    // run a command a flag, an address or a template names (#5619, found by #5595's review):
    // `xcodebuild`'s build phases, `msgexec`'s command, `leaks --atExit --`, `pyenv exec` and its
    // shims, `erb`'s Ruby, `helm`'s `--post-renderer` and plugins, `socat`'s `EXEC:` and
    // `SYSTEM:`, `ncat --exec` and `nc -e`, and `tar --to-command`, `-I` and
    // `--use-compress-program`. Each is refused whole, not by its flag: a flag can be spelled
    // many ways, and a name refused only takes away an Allow.
    "xcodebuild",
    "msgexec",
    "leaks",
    "pyenv",
    "erb",
    "helm",
    "socat",
    "ncat",
    "nc",
    "netcat",
    "tar",
    "bsdtar",
    // the same family (#5619's review): `op run --` and `aws-vault exec --` run the command after
    // them; version managers run their shims and an `exec` or `run`; `msgfilter` runs its filter;
    // `xctrace`, `valgrind` and `hyperfine` run what they launch, trace or time; `helmfile` runs
    // its hooks.
    "op",
    "aws-vault",
    "rbenv",
    "asdf",
    "mise",
    "direnv",
    "pipenv",
    "conda",
    "volta",
    "msgfilter",
    "xctrace",
    "valgrind",
    "hyperfine",
    "helmfile",
    // secret injectors, which run the command after them with what they fetched: the closest
    // kin of `op run` and `aws-vault exec`
    "doppler",
    "infisical",
    "sops",
    "chamber",
    "envconsul",
    "summon",
    "envchain",
    "dotenvx",
    "teller",
    "berglas",
    "aws-sso",
    "granted",
    "assume",
    // environment managers and project runners, which run a command or what a file says
    "mamba",
    "micromamba",
    "rtx",
    "goenv",
    "jenv",
    "fnm",
    "nix",
    "nix-shell",
    "devbox",
    "pixi",
    "hatch",
    "pdm",
    "rye",
    "tox",
    "nox",
    // more secret injectors (#5619's third review)
    "saml2aws",
    "aws-okta",
    "bws",
    "dotenv",
    "phase",
    "awsume",
    // programs that run what a chat wrote: a script, its tests, its hooks or its stack
    "bunx",
    "tsx",
    "pytest",
    "watchexec",
    "entr",
    "pre-commit",
    "pulumi",
    "cdk",
    // programs that run what a file a chat wrote says (#5619's fourth review): `terraform`'s
    // `local-exec` and `external`, a playbook, a Vagrantfile, a template, a test suite
    "terraform",
    "tofu",
    "terragrunt",
    "ansible",
    "ansible-playbook",
    "vagrant",
    "packer",
    "cdktf",
    "sst",
    "serverless",
    "jest",
    "vitest",
    "mocha",
    "rspec",
    // programs whose flags name a command (#5799): `sed`'s script runs one (GNU `e`), as `awk`'s
    // and `m4`'s do; `aria2c` runs its `--on-download-*` hooks, which a `--conf-path` file or its
    // RPC can set too; `kustomize` runs what a kustomization a chat wrote says, with its exec
    // plugins and helm.
    "sed",
    "aria2c",
    "kustomize",
    // more of the family (#5833). Run the command after them, or one they name:
    // `consul-template -exec`, `railway run`, `aws-runas`, `vals exec`, `gomplate -- cmd`,
    // `envdir` and `chpst`, `systemd-run` and `run0` (systemd's own `sudo`), `telepresence … --`,
    // `firejail`, `flatpak run` and `flatpak-spawn --host`, `rr record`, `pkgx` and
    // `flox activate --`.
    "consul-template",
    "railway",
    "aws-runas",
    "vals",
    "gomplate",
    "envdir",
    "chpst",
    "systemd-run",
    "run0",
    "telepresence",
    "firejail",
    "flatpak",
    "flatpak-spawn",
    "rr",
    "pkgx",
    "flox",
    // Run what a file a chat wrote says: a Taskfile, a skaffold.yaml, a Tiltfile, a dagger
    // module, an Earthfile, a workflow `act` runs, `firebase emulators:exec`, `netlify dev` and
    // `build`, `vercel build`, `wrangler`'s build, `chezmoi`'s scripts, a magefile, `invoke`'s
    // and `fab`'s tasks, a cdk8s app.
    "task",
    "skaffold",
    "tilt",
    "dagger",
    "earthly",
    "act",
    "firebase",
    "netlify",
    "vercel",
    "wrangler",
    "chezmoi",
    "mage",
    "invoke",
    "fab",
    "cdk8s",
    // Build tools and test runners, which build and run what a chat wrote, and the CI runners
    // that run a pipeline file locally. (`phpunit` is refused already, by `php`'s prefix.)
    "sbt",
    "mix",
    "playwright",
    "cypress",
    "gitlab-runner",
    "circleci",
    "stack",
    "cabal",
    "dune",
    "zig",
    "lein",
    // the closest kin of ssh (#5876's review): `sshpass` runs the command after it, `autossh`
    // hands ssh whatever it is given, `mosh --ssh=` runs the command it names, and `lftp` runs
    // commands from its own prompt and scripts, as `ftp` does.
    "sshpass",
    "autossh",
    "mosh",
    "lftp",
    // mysql's and mariadb's server-side and shell-like tools (#5876's sixth review): a server
    // start runs the server program and scripts its flags name, an install runs what its folders
    // hold, `mysqlslap --pre-system` and a test file run a shell command, and `mysqlsh` runs code.
    "mysql.server",
    "mariadb.server",
    "mysqld_safe",
    "mariadbd-safe",
    "mysqld_multi",
    "mysql_install_db",
    "mariadb-install-db",
    "mysqlslap",
    "mariadb-slap",
    "mysqltest",
    "mariadb-test",
    "mysqlsh",
    "mariadbd-multi",
    "mysqltest_embedded",
    "mariadb-test-embedded",
    // the rest of the family (#5891, #5876's seventh review): `mysqlhotcopy --method` names the
    // copy program it has a shell run, the secure installation is a script that runs the client
    // its `--basedir` finds, and mariabackup runs `qpress`, `rsync` and the plugins its flags
    // name. Server-side all three, none a usual reason to hand a chat a key.
    "mysqlhotcopy",
    "mariadb-hotcopy",
    "mysql_secure_installation",
    "mariadb-secure-installation",
    "mariabackup",
    "mariadb-backup",
    // and more of them (#5931, #5929's second review): `mysql_plugin` starts the server its
    // `-m` or `-b` finds with the plugins its `-p` names, and runs the program its
    // `--my-print-defaults` names; `mysql_upgrade` runs the client and `mysqlcheck` with the files
    // of settings and plugins it is handed. Percona's backup tools and MariaDB's `mbstream` are
    // mariabackup's kin, and the server's state transfer scripts (`wsrep_sst_*`, refused by their
    // prefix, below) run the transfer program their arguments name. Server-side all, none a
    // usual reason to hand a chat a key.
    "mysql_plugin",
    "mariadb-plugin",
    "mysql_upgrade",
    "mariadb-upgrade",
    "xtrabackup",
    "innobackupex",
    "xbstream",
    "xbcloud",
    "xbcloud_osenv",
    "xbcrypt",
    "mbstream",
    // The server itself, whose `-b` names the folder its plugins load from and whose
    // `--early-plugin-load` names one, the router, whose configuration names its plugins' folder,
    // and the keyring migration, which loads components from a folder it is told (#5990's
    // second review). The tools' row reads none of those forms. (`mysqld-debug` is `mysqld`'s
    // flavour, by its first part.)
    "mysqld",
    "mariadbd",
    // runs the command its arguments name, as the user they name (#5990's third review)
    "mysqld_safe_helper",
    "mariadbd-safe-helper",
    "mysqlrouter",
    "mysqlrouter_plugin_info",
    "mysqlrouter_passwd",
    "mysqlrouter_keyring",
    "mysql_migrate_keyring",
    // The client-side kin (#5994, from #5990's reviews), refused whole as the rest of the family:
    // a row would have to read every flag and every defaults file that names what they run.
    // mydumper runs a command on each file it writes (`--exec`, `--exec-per-thread`), myloader
    // one on each it reads, and either may be named in a defaults file the command line never
    // shows. Percona Toolkit's `pt-*` (refused by their prefix, below) run code their flags name
    // (`pt-online-schema-change --plugin`, `pt-stalk --plugin`, `pt-kill --execute-command`,
    // `pt-query-digest --filter`), and any flag may come from a configuration file. gh-ost runs
    // the `gh-ost-on-*` programs of the folder its `--hooks-path` names.
    "mydumper",
    "myloader",
    "gh-ost",
    // Galera's helpers: `galera_recovery` has a shell evaluate its arguments into the server's
    // command line, `galera_new_cluster` has the service manager restart whatever unit it names,
    // Percona XtraDB Cluster's `garbd` runs the scripts its `--recv-script` and
    // `--post-recv-script` name (every `garbd` reads a `--cfg` file), and `garb-systemd` runs
    // `garbd`.
    "galera_recovery",
    "galera_new_cluster",
    "garbd",
    "garb-systemd",
    // The server's systemd helpers (#6109): `mysqld_bootstrap` has the service manager start the
    // server's unit with `--wsrep-new-cluster`, as `galera_new_cluster` does; `mysqld_pre_systemd`
    // runs the server's `--initialize` with what its settings name; and Percona XtraDB Cluster's
    // packaged `mysql-systemd` does the second, and has a shell evaluate the server's command
    // line for its `galera-recovery`. Their `mysql` prefix took them to the tools' row, which
    // reads nothing they do.
    "mysqld_bootstrap",
    "mysqld_pre_systemd",
    "mysql-systemd",
    // MariaDB's client on Debian (#6109): innotop's configuration file (its `-c`) names a
    // `plugin_dir`, whose Perl files it loads as the file's `[plugins]` lists them, and compiles
    // its filters and columns as Perl; `mariadb-report` (`mysqlreport` by its compat link) hands
    // its `--outfile` unquoted to a shell.
    "innotop",
    "mariadb-report",
    "mysqlreport",
    // Percona XtraDB Cluster's health checks (#6109): `clustercheck` hands the client its sixth
    // argument as `--defaults-extra-file`, and `pyclustercheck`'s `--conf` names the file of
    // settings its client library reads.
    "clustercheck",
    "pyclustercheck",
    // The slow log's summary and the row finder (#6109): both read the files they are named with
    // Perl's `<>`, whose open runs a name ending in `|` as a shell command.
    "mysqldumpslow",
    "mariadb-dumpslow",
    "mysql_find_rows",
    "mariadb-find-rows",
    // A host value that reaches the client's flags or a DBI address (#6185): `mysqlaccess` splits
    // its command line, `--host=` and the host among it, on white space into the client's
    // arguments; `mytop`, `mysql_convert_table_format` and `mysql_setpermission` join the host
    // into a `DBI:MariaDB:` address its driver splits at each `:` or `;` into attributes, among them
    // a file of settings the client library reads. Percona Server's `ps-admin` `source`s its
    // `--config-file` as a shell file and expands its user and host unquoted into the client's
    // command line.
    "mysqlaccess",
    "mariadb-access",
    "mytop",
    "mysql_convert_table_format",
    "mariadb-convert-table-format",
    "mysql_setpermission",
    "mariadb-setpermission",
    "ps-admin",
    // The test suite's runners and RocksDB's hot backup (#6187): `mysqlxtest`'s script, from its
    // `-f` or its input, runs a shell command at each `-->system`, as `mysqltest`'s does, and
    // `mysqltest_safe_process` runs the program it is given after `--`; `myrocks_hotbackup` hands
    // `os.system` a command its `--extra_wdt_sender_options` is formatted into, and ssh its
    // `--destination` first. `rcmysql`, a link to the server's init script, starts the server
    // (and is judged by its target's folder too, [`an_init_script`]). MySQL's own SLES packages
    // link `rcmysql` to `service` instead, which runs the init script it is named, or has
    // `systemctl` act on the unit, so every `rc*` link to it is refused by its target's name.
    "mysqlxtest",
    "mysqltest_safe_process",
    "myrocks_hotbackup",
    "rcmysql",
    "service",
    // Shipped with the test runners in Percona Server's packages: `mysql_keyring_encryption_test`
    // loads `file://<--component_dir>/<--keyring>` as a component, as `mysql_migrate_keyring` does.
    "mysql_keyring_encryption_test",
    // The client and its test built with the server inside them: what `--server-arg` hands the
    // server (`--plugin-load`, `--plugin-dir`) loads code from a file it names.
    "mysql_embedded",
    "mariadb-embedded",
    "mysql_client_test_embedded",
    "mariadb-client-test-embedded",
    // MySQL Cluster's reporters (#6324), which MySQL's packages ship built with the cluster:
    // `ndb_error_reporter` joins its config file's name into the `ndb_config` commands a shell
    // runs, and its scp user into the `scp` command one runs; `ndb_size.pl` joins its
    // `--hostname` and `--socket` into a `DBI:mysql:` address, as `mytop` does. Neither name
    // starts with `mysql` or `mariadb`, so no row read them.
    "ndb_error_reporter",
    "ndb_size.pl",
    // The service managers beside `service` (#6320): `systemctl link <path>` makes a unit of the
    // file it is named, which `start` runs, and `import-environment` copies a value the run was
    // handed into the manager's environment, for every unit after it (`launchctl` is refused
    // for the same); dpkg's `start-stop-daemon` runs what its `--startas` or `--exec` names;
    // runit's `sv` runs the `check` script of a folder it is named, and `runsv` its `run`;
    // `supervisorctl`'s settings file names a plugin it imports; `openrc --service` runs the path
    // it is given and `openrc-run` sources the script it is named; and `chkconfig` and
    // `deb-systemd-helper` hand `systemctl` the unit they are named.
    "systemctl",
    "start-stop-daemon",
    "sv",
    "runsv",
    "supervisorctl",
    "openrc",
    "openrc-run",
    "chkconfig",
    "deb-systemd-helper",
    // Homebrew (#6325): `brew ruby -e` runs the Ruby it is handed, `brew sh --cmd` a shell
    // command, and `brew <name>` the `brew-<name>` its search path finds.
    "brew",
    // Kept offered, Allow once on the exact command alone (#5799, D-5619-7): `ssh`, `scp`,
    // `sftp`, `rsync`, `kubectl`, `psql`, `zip` and `wget` are the usual reason a chat is handed a
    // key at all. Only their forms that name a command, or a file that can, are refused
    // (`ssh -o ProxyCommand`, `rsync -e`, `kubectl --kubeconfig`, `psql -f`, `zip -TT`,
    // `wget --use-askpass` and the rest: [`command_flags`]). A granted run has no stdin and a
    // home of its own, so what else it reads is in the command the person reads.
];

/// **The programs a Run grant may be wider than one exact command for** (the person's ruling on
/// ADR 0112): Allow for this chat and Always allow, which take any arguments, so the arguments are
/// no longer read by anyone. Every other program [`for_run_grant`] takes gets Allow once alone.
/// **Each addition is its own review of whether that program can run others.** `wget` is out as it
/// stands: its `--use-askpass` runs a command. A program here can still print or send what it is
/// handed, as its notice's caution says.
const REVIEWED_FOR_ANY_ARGUMENTS: &[&str] = &["curl"];

/// Whether `name` is a program a grant of any arguments may be given for, spelled exactly so.
pub fn reviewed_for_any_arguments(name: &str) -> bool {
    REVIEWED_FOR_ANY_ARGUMENTS.contains(&name)
}

/// Whether `program`'s spelling is the one its folder holds: macOS finds `CURL` for `curl`, and a
/// grant bound to a spelling the person never sees elsewhere would escape every check by name.
fn spelled_as_it_is_held(program: &Path) -> bool {
    let (Some(parent), Some(name)) = (program.parent(), program.file_name()) else {
        return false;
    };
    std::fs::read_dir(parent)
        .is_ok_and(|mut entries| entries.any(|one| one.is_ok_and(|one| one.file_name() == name)))
}

/// **The program and values a chat's command names, as a Run grant would hand them** (ADR 0112,
/// #5239): read as a declared tool is, by the same refusals, so nothing a tool may not be is
/// offered to the person. `written` is every folder the asking chat may write. Refused, with a
/// sentence and with nothing asked:
///
/// - a program named by anything but its plain name or its absolute path as the app finds it, so
///   the path the person is shown is never one the chat picked ([`names`]);
/// - a program that runs what it is told, or purlis itself, by name, as found and as its links
///   resolve ([`refused_name`], [`refused_at`]);
/// - one found only where a chat writes, or not at all ([`resolve_beside`]);
/// - a `--dotenv`, which names no variable; a variable that steers what runs ([`refused_var`]);
///   a key no Secret ask takes;
/// - a key a `mint` line names ([`mint::raw`]), or one that `listed` hands only to a browser login
///   or a secret-fill line: purlis fills those on one origin itself, and no program is handed them.
///   The gate holds every persona's such key too, through a link and in any case
///   ([`crate::secretfill::a_browsers`]).
pub fn for_run_grant(
    root: &Path,
    persona: &str,
    asked: &Asked<'_>,
    listed: &[Tool],
    locks: &super::policy::Locks,
    written: &[PathBuf],
) -> Result<Ready, String> {
    let name = Path::new(asked.program)
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    if let Some(why) = refused_name(&name) {
        return Err(why);
    }
    let runs_others = |name: &str| {
        format!(
            "{name} runs other programs as it is told, so purlis does not offer to allow it: \
             declare what it's for as a tool, with the arguments it takes"
        )
    };
    if not_for_a_run_grant(&name) {
        return Err(runs_others(&name));
    }
    if !asked.dotenv.is_empty() {
        return Err(format!(
            "--dotenv names no variable, so {name} can't be allowed with it: hand each key with              --file VARIABLE=key or --env VARIABLE=key"
        ));
    }
    let mut values = Vec::new();
    for (specs, handed) in [(asked.file, Handed::File), (asked.env, Handed::Env)] {
        for spec in specs {
            let Some((var, key)) = spec.split_once('=') else {
                return Err(format!("{} expects VARIABLE=key", handed.flag()));
            };
            if let Some(why) = refused_var(var) {
                return Err(why);
            }
            if var.len() > MOST_VARIABLE_SHOWN {
                return Err(format!(
                    "a variable's name can be at most {MOST_VARIABLE_SHOWN} characters for the \
                     person to read it, so purlis did not ask: use a shorter one"
                ));
            }
            if loads_code(var) {
                return Err(format!(
                    "{var} steers what a program loads, so purlis does not offer to hand it a value"
                ));
            }
            if !crate::secrets::needs::is_key(key) {
                return Err(format!(
                    "{} is not a key name a run grant takes: use letters, digits, _, - and ., \
                     starting with a letter or _",
                    crate::shown::one_line(key, 40)
                ));
            }
            if values.iter().any(|value: &Value| value.var == var) {
                return Err(format!("{var} is handed two values"));
            }
            if let mint::Raw::Refused(why) = mint::raw(root, asked.vault, key) {
                return Err(why);
            }
            let only_a_browser_fills_it = |tool: &Tool| {
                tool.vault == asked.vault
                    && (tool.fill.as_ref().is_some_and(|fill| fill.key == key)
                        || (tool.login.is_some()
                            && tool.values.iter().any(|value| value.key == key)))
            };
            if listed.iter().any(only_a_browser_fills_it) {
                return Err(format!(
                    "{key} in vault {} is one purlis fills in a browser itself, on the one site \
                     {persona} declares, so no program is handed it",
                    asked.vault
                ));
            }
            values.push(Value {
                var: var.to_owned(),
                handed,
                key: key.to_owned(),
            });
        }
    }
    if values.is_empty() {
        return Err(format!(
            "the command hands {name} no key, so there is nothing to allow"
        ));
    }
    if values.len() > MOST_VALUES {
        return Err(format!(
            "a run is handed at most {MOST_VALUES} values, so {name} can't be allowed with more"
        ));
    }
    let tool = Tool {
        name: name.clone(),
        vault: asked.vault.to_owned(),
        values,
        login: None,
        args: None,
        mint: None,
        host: None,
        fill: None,
    };
    let resolved = resolve_beside(
        root,
        persona,
        std::slice::from_ref(&tool),
        locks,
        &search_dirs(),
        written,
        &temp_folders(),
    );
    let Some(mut resolved) = resolved else {
        return Err(format!(
            "no {name} was found outside the folders a chat may write"
        ));
    };
    let Some(ready) = resolved.ready.pop() else {
        return Err(resolved.left.pop().unwrap_or_else(|| {
            format!("no {name} was found outside the folders a chat may write")
        }));
    };
    // A link may lead to one: `vimdiff` to `vim`, `editor` to `vim.basic`, `rcmysql` to the
    // server's init script, whose file name is the client's, or through one (#6321).
    // As `resolve_beside` found it, so the links judged are the ones on the way to what runs.
    let found = found_in(
        &name,
        &search_dirs(),
        &all_written(root, written, &temp_folders()),
    )
    .map(|(found, _)| found);
    if not_for_a_run_grant(&file_name_of(&ready.program))
        || an_init_script(found.as_deref().unwrap_or(&ready.program), &ready.program)
    {
        return Err(runs_others(&name));
    }
    if !names(&ready, asked.program) {
        return Err(format!(
            "{} is not the {name} purlis finds outside the folders a chat may write: name it {name}",
            crate::shown::one_line(asked.program, 120)
        ));
    }
    if !spelled_as_it_is_held(&ready.program) {
        return Err(format!(
            "{name} is not how its folder spells it, so purlis did not ask: name the program as \
             it is spelled"
        ));
    }
    Ok(ready)
}

/// **Whether `found`, which runs as `real`, is a service's init script** (#6187): a file in an
/// `init.d` folder (`/etc/init.d`, `/etc/rc.d/init.d`), whatever its own name, as found, as it
/// runs, or as any link on the way sits (#6321: Upstart's `/etc/init.d/<job>` linked on out to
/// `/lib/init/upstart-job`). A script there starts and stops the service it is named for, and
/// is named as the service is, so the server's is `mysql`, the client's name: judged by its
/// name, `rcmysql start` would reach the client's row.
fn an_init_script(found: &Path, real: &Path) -> bool {
    let in_init_d = |file: &Path| {
        file.parent()
            .and_then(Path::file_name)
            .is_some_and(|folder| folder.eq_ignore_ascii_case("init.d"))
    };
    let links_in_init_d = walked(found, &[]).is_some_and(|(_, folders)| {
        folders.iter().any(|folder| {
            folder
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("init.d"))
        })
    });
    in_init_d(found) || in_init_d(real) || links_in_init_d
}

/// The longest variable name a person is asked to allow: it is drawn among the app's own facts.
const MOST_VARIABLE_SHOWN: usize = 64;

/// **Whether `name` is on [`NOT_FOR_A_RUN_GRANT`], or runs what it is told** ([`REFUSED_NAMES`]),
/// as itself or by a form its builds are named by (#5424): `vim.basic`, a version (`gcc-13`,
/// `clang-17`, `gcc13`), a GNU build (`gm4`, `genv`, `gnumake`), a target's
/// (`x86_64-linux-gnu-gcc`), a flavour (`emacs-nox`, `sudo-rs`), and by its prefix one of git's
/// own programs (`git-shell`), a state transfer script (`wsrep_sst_rsync`) or a Percona Toolkit
/// tool (`pt-kill`). Each form only takes away an Allow, so a name it catches by chance loses a
/// notice and nothing more.
fn not_for_a_run_grant(name: &str) -> bool {
    let refused = |one: &str| {
        !one.is_empty() && (NOT_FOR_A_RUN_GRANT.contains(&one) || runs_what_it_is_told(one))
    };
    // git's own programs are git's: `git-shell` runs what it is told, `git-lfs` runs git. The
    // server's state transfer scripts, each release adding some, run what their arguments name
    // (#5931), and so do Percona Toolkit's tools, each release adding some (#5994).
    let lower = name.to_ascii_lowercase();
    let stem = lower.split('.').next().unwrap_or_default();
    if stem.starts_with("git-") || stem.starts_with("wsrep_sst_") || stem.starts_with("pt-") {
        return true;
    }
    name_forms(name).iter().any(|one| refused(one))
}

/// **The names `name`'s program may be, as its builds are named** (#5424, #5876): itself
/// lowercased, its stem before a `.` (`vim.basic`, `kubectl.docker`), without a version (`gcc-13`,
/// `wget2`), a target's last part (`x86_64-linux-gnu-gcc`), a flavour's first (`emacs-nox`), and
/// each of those without a GNU `g` or `gnu` (`gm4`, `gnumake`). A form caught by chance takes
/// away an Allow and nothing more.
pub(crate) fn name_forms(name: &str) -> Vec<String> {
    let name = name.to_ascii_lowercase();
    let stem = name.split('.').next().unwrap_or_default();
    let mut forms = vec![name.as_str(), stem];
    for one in [name.as_str(), stem] {
        let unversioned = without_version(one);
        forms.push(unversioned);
        // The program a target's build is, its last part (`aarch64-linux-gnu-gcc`), and the
        // program a build is a flavour of, its first (`emacs-nox`, `gdb-multiarch`, `sudo-rs`).
        if let Some((_, last)) = unversioned.rsplit_once('-') {
            forms.push(last);
            forms.push(without_version(last));
        }
        if let Some((first, _)) = unversioned.split_once('-') {
            forms.push(first);
        }
    }
    let gnu = forms
        .iter()
        .filter_map(|one| one.strip_prefix("gnu").or_else(|| one.strip_prefix('g')))
        .collect::<Vec<_>>();
    forms
        .iter()
        .chain(&gnu)
        .filter(|one| !one.is_empty())
        .map(|one| (*one).to_owned())
        .collect()
}

/// `name` without a version at its end: `gcc-13`, `clang-17.0`, `gcc13` and `ld64` are `gcc`,
/// `clang`, `gcc` and `ld`.
pub(crate) fn without_version(name: &str) -> &str {
    let bare = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    let bare = bare.strip_suffix(['-', '_']).unwrap_or(bare);
    if bare.is_empty() { name } else { bare }
}

/// **The caution beside a Run grant's Allow** ([`caution`]), on the app's own search path.
pub fn caution_here(tool: &Tool) -> Option<String> {
    caution(tool, &search_dirs())
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
    allows(ready, args)?;
    Ok(std::iter::once(ready.program.display().to_string())
        .chain(args.iter().cloned())
        .collect())
}

/// **Whether `ready`'s tool runs with `args`** (#3421): never a forge command line's credential
/// or settings command ([`args::refused_forge`], by the tool's name and its program's as found
/// and as its links resolve), and, where it declares `args`, only with arguments that match
/// them ([`args::permits`]). Judged before any value is read for the run.
pub fn allows(ready: &Ready, args: &[String]) -> Result<(), String> {
    let real = ready.program.canonicalize().ok();
    let names: Vec<&Path> = [Path::new(&ready.tool.name), ready.program.as_path()]
        .into_iter()
        .chain(real.as_deref())
        .collect();
    if let Some(why) = args::refused_forge(&names, args) {
        return Err(why);
    }
    // The forge host its run reaches (#3869).
    match (forge_of(&names), &ready.tool.host) {
        (None, Some(_)) => {
            return Err(format!(
                "{} names a forge host, and only a tool whose program is gh or glab takes one, \
                 so nothing ran: take host out of the project's tools.{}",
                ready.tool.name, ready.tool.name
            ));
        }
        (Some(glab), host) => {
            let host = host.as_deref().unwrap_or(host::default_of(glab));
            let forge = if glab { "glab" } else { "gh" };
            if let Some(why) = host::named_elsewhere(forge, glab, host, args) {
                return Err(why);
            }
        }
        (None, None) => {}
    }
    args::permits(&ready.tool.name, ready.tool.args.as_deref(), args)
}

/// **Whether `ready`'s run with `args` in `folder` reads only repository configuration the
/// person controls** (#3868, [`repo::held`]), for a project at `root` whose chat may write
/// `writable`: the variables that hold its git to the repository judged ([`repo::Pin::env`]),
/// or why it never runs. glab's own settings are judged where the tool's name or program is
/// glab, and git's options that name another config, folder or program where it is git
/// ([`repo::refused_git`]). Judged before any value is read for the run.
pub fn reads_held_repository(
    ready: &Ready,
    args: &[String],
    folder: &Path,
    root: &Path,
    writable: &[PathBuf],
) -> Result<Vec<(String, String)>, String> {
    let real = ready.program.canonicalize().ok();
    let names: Vec<String> = [Path::new(&ready.tool.name), ready.program.as_path()]
        .into_iter()
        .chain(real.as_deref())
        .map(file_name_of)
        .collect();
    if names.iter().any(|name| name == "git") {
        // git's own options run programs in more ways than a list can hold, so a declared git
        // runs only with the arguments its `args` let in.
        if ready.tool.args.is_none() {
            return Err(format!(
                "{} is git, which runs with a vault's value only with the arguments its tool \
                 allows, and it allows any, so nothing ran: list them in the project's \
                 tools.{}.args and allow the persona's tools again on this machine.",
                ready.tool.name, ready.tool.name
            ));
        }
        if let Some(why) = repo::refused_git(args) {
            return Err(why);
        }
    }
    let glab = names.iter().any(|name| name == "glab");
    let pin = repo::held(folder, root, writable, glab)?;
    // A command that makes a new repository: `repo clone`, `gist clone`, `--clone` (and
    // `gh repo create -c`).
    let clones = args.get(1).is_some_and(|word| word == "clone")
        || args
            .iter()
            .any(|arg| arg == "--clone" || arg.starts_with("--clone="))
        || (args.first().is_some_and(|word| word == "repo")
            && args.get(1).is_some_and(|word| word == "create")
            && args.iter().any(|arg| arg == "-c"));
    Ok(pin.env(clones))
}

/// Whether a tool with these `names` (its own name, then its program's) is a forge's, and
/// glab's (`true`) or gh's (`false`): by its program, as its links resolve and as found,
/// before the tool's own name.
fn forge_of(names: &[&Path]) -> Option<bool> {
    names
        .iter()
        .rev()
        .find_map(|name| match file_name_of(name).as_str() {
            "gh" => Some(false),
            "glab" => Some(true),
            _ => None,
        })
}

/// **The variables a forge tool's run starts with** (#3869): its host, from its declaration
/// or the forge's own, as the program reads it ([`host::env`]); none for any other program.
pub fn forge_env(ready: &Ready) -> Vec<(String, String)> {
    let real = ready.program.canonicalize().ok();
    let names: Vec<&Path> = [Path::new(&ready.tool.name), ready.program.as_path()]
        .into_iter()
        .chain(real.as_deref())
        .collect();
    forge_of(&names)
        .map(|glab| {
            let host = ready.tool.host.as_deref().unwrap_or(host::default_of(glab));
            vec![host::env(glab, host)]
        })
        .unwrap_or_default()
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

/// **What a run under a Run grant leaves of the chat's variables** (ADR 0112, amended
/// 2026-11-05): where it connects and whom it trusts. The person read a command and a program,
/// and neither shows these, so a chat that wants a host, a port, a region or a certificate
/// authority says it in the command, where the person reads it.
const A_GRANTED_RUN_LEAVES: &[&str] = &[
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NODE_EXTRA_CA_CERTS",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    "AWS_CA_BUNDLE",
    "PGHOST",
    "PGHOSTADDR",
    "PGPORT",
    "PGSSLMODE",
    "MYSQL_HOST",
    "MYSQL_TCP_PORT",
    "REDIS_HOST",
    "REDIS_PORT",
    "AWS_REGION",
    "AWS_DEFAULT_REGION",
];

/// **Whether a run under a Run grant starts with the chat's `var`**: [`a_run_starts_from`], less
/// [`A_GRANTED_RUN_LEAVES`].
pub fn a_granted_run_starts_from(var: &str) -> bool {
    a_run_starts_from(var) && !A_GRANTED_RUN_LEAVES.contains(&var)
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

/// The words every refusing shim's second line holds ([`refusing_shim`]), by which one finds
/// another and passes it by.
const REFUSING_MARK: &str = "starts on a runner (#4354)";

/// **The shim of a tool whose program is not in the runner's image** (#4354): its name in the chat
/// says why, and fails as a missing program does.
///
/// **A tool nobody allowed runs as its bare program anyway** (#4373), so where `passes_on` names
/// the shim's own folder, the shim first runs the first program of its name on `PATH` outside
/// that folder, one that has come since the chat started, and refuses only where there is none.
/// Only an absolute folder is looked in, and another refusing shim is passed by. An allowed
/// tool's shim refuses still: the program it would run is one its start never judged, and it
/// would run without the vault's value.
pub fn refusing_shim(persona: &str, name: &str, said: &str, passes_on: Option<&Path>) -> String {
    let program = quoted(name);
    let passes_on = passes_on.map_or_else(String::new, |folder| {
        format!(
            "# Nobody allowed it, so a `{name}` that came after the chat started runs as itself: \
             the first\n# on PATH outside this folder.\n\
             here=$(cd -P -- {folder} 2>/dev/null && pwd)\n\
             rest=$PATH\n\
             while [ -n \"$rest\" ]; do\n\
             dir=${{rest%%:*}}\n\
             case $rest in *:*) rest=${{rest#*:}} ;; *) rest= ;; esac\n\
             case $dir in /*) ;; *) continue ;; esac\n\
             [ \"$(cd -P -- \"$dir\" 2>/dev/null && pwd)\" = \"$here\" ] && continue\n\
             if [ -f \"$dir\"/{program} ] && [ -x \"$dir\"/{program} ] \
             && ! grep -q {mark} \"$dir\"/{program} 2>/dev/null; then\n\
             exec \"$dir\"/{program} \"$@\"\nfi\ndone\n",
            folder = quoted(&folder.display().to_string()),
            mark = quoted(REFUSING_MARK),
        )
    });
    format!(
        "#!/bin/sh\n# Written by purlis as a chat of {persona} {REFUSING_MARK}. `{name}` \
         is not in\n# this pool's image. Edits here are overwritten.\n\
         {passes_on}echo {said} >&2\nexit 127\n",
        said = quoted(&format!("purlis: {said}")),
    )
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
    /// On a runner of `image`: each tool whose program is absent says so under its name (#4354).
    fn refuse_what_is_absent(&mut self, image: Option<&str>) {
        self.refusing = self
            .absent
            .iter()
            .map(|name| {
                let said = if self.unallowed.contains(name) {
                    neither_allowed_nor_in_the_image(&self.persona, name, image)
                } else {
                    not_in_the_image(&self.persona, name, image)
                };
                (name.clone(), said)
            })
            .collect();
    }

    /// **Writes the shims**, each running `purlis`, and Claude Code's env file, replacing every
    /// shim of an earlier start: none where no tool is ready and none refuses.
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
                .map(|one| &one.tool.name)
                .chain(self.refusing.iter().map(|(tool, _)| tool))
                .any(|tool| std::ffi::OsStr::new(tool) == name);
            if !current {
                std::fs::remove_file(entry.path()).map_err(refused(&entry.path()))?;
            }
        }
        if self.ready.is_empty() && self.refusing.is_empty() {
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
        for (name, said) in &self.refusing {
            put(
                &bin.join(name),
                &refusing_shim(
                    &self.persona,
                    name,
                    said,
                    self.unallowed.contains(name).then_some(bin.as_path()),
                ),
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

#[cfg(test)]
mod args_tests;

#[cfg(test)]
mod command_flags_tests;

#[cfg(test)]
mod fill_tests;
#[cfg(test)]
mod host_tests;
#[cfg(test)]
mod mint_tests;
#[cfg(test)]
mod repo_tests;
