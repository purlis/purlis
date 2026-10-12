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
//! ([`refused_name`]); a variable that steers how programs are found or loaded
//! ([`refused_var`]); a tool policy forbids ([`super::policy::Locks::refuses_tool`]); and one
//! whose program is not found where no chat writes.
//!
//! **A browser login** is a tool of its own kind ([`login`], D-PE-browser-1): `browser = "<login
//! page>"` in place of `as`, and purlis, not the chat, opens that page and fills the login on
//! its origin alone.

use std::path::{Path, PathBuf};

use crate::settings::Refusal;

pub mod login;

/// The key in a persona's table that holds its tools.
pub const KEY: &str = "tools";

/// The keys one tool's table holds: `as` for a program's, `browser` and `user` for a browser
/// login's ([`login`]).
const TOOL_KEYS: [&str; 5] = ["vault", "as", "key", login::KEY, login::USER_KEY];

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

/// **One tool of a persona's**: the program's name, the vault and key its value comes from,
/// and the variable it is handed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    pub name: String,
    pub vault: String,
    pub var: String,
    pub handed: Handed,
    pub key: String,
    /// Where the tool is a browser login ([`login`]): the page, its origin and the user name's
    /// key. `var` is then [`login::PASS_VAR`], handed as a file.
    pub login: Option<login::Login>,
}

impl Tool {
    /// The tool whole, as an Allow of it is bound to and a person reads it: its name, then the
    /// vault, key and variable it is handed; for a browser login, the page and both keys.
    pub fn whole(&self) -> String {
        if let Some(login) = &self.login {
            let user = login
                .user
                .as_ref()
                .map_or_else(String::new, |user| format!("{user} and "));
            return format!(
                "{} logs in to {} with {user}{} from vault {} in a browser",
                self.name, login.url, self.key, self.vault
            );
        }
        let handed = match self.handed {
            Handed::File => "file",
            Handed::Env => "env",
        };
        format!(
            "{} with {} from vault {} as {} {handed}",
            self.name, self.key, self.vault, self.var
        )
    }

    /// The words of `purlis secret exec` that run `program` as this tool, after the binary:
    /// `secret exec <vault> --file <VAR>=<key> -- <program>`. A browser login names its tool by
    /// its name instead, after its files.
    pub fn secret_exec(&self, program: &Path) -> Vec<String> {
        if let Some(login) = &self.login {
            // A login runs by its name: the app, not the shim, picks what runs (D-PE-browser-1).
            let mut words = vec!["secret".to_owned(), "exec".to_owned(), self.vault.clone()];
            for spec in login::hand_over(&self.key, login) {
                words.extend(["--file".to_owned(), spec]);
            }
            words.extend(["--".to_owned(), self.name.clone()]);
            return words;
        }
        let flag = match self.handed {
            Handed::File => "--file",
            Handed::Env => "--env",
        };
        vec![
            "secret".to_owned(),
            "exec".to_owned(),
            self.vault.clone(),
            flag.to_owned(),
            format!("{}={}", self.var, self.key),
            "--".to_owned(),
            program.display().to_string(),
        ]
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
    let lower = name.to_ascii_lowercase();
    if REFUSED_NAMES
        .iter()
        .any(|one| one.eq_ignore_ascii_case(name))
        || REFUSED_PREFIXES.iter().any(|one| lower.starts_with(one))
    {
        return Some(format!(
            "{name} runs whatever it is told, so it would hand the vault's credential to anything \
             — a tool is the one program that needs it"
        ));
    }
    None
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
                "{other} is not a key purlis reads — a tool holds vault, as and key"
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
        let Some(key) = text("key").filter(|key| !key.chars().any(char::is_control)) else {
            fault("it names no key of the vault".to_owned());
            continue;
        };
        if tool.contains_key(login::KEY) {
            match login_of(tool, &text) {
                Ok(login) => tools.push(Tool {
                    name: name.clone(),
                    vault,
                    var: login::PASS_VAR.to_owned(),
                    handed: Handed::File,
                    key,
                    login: Some(login),
                }),
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
        tools.push(Tool {
            name: name.clone(),
            vault,
            var: var.to_owned(),
            handed,
            key,
            login: None,
        });
    }
    (tools, refused)
}

/// A browser login's `browser` and `user`, or why the tool is not one ([`login`]).
fn login_of(
    tool: &toml::map::Map<String, toml::Value>,
    text: &dyn Fn(&str) -> Option<String>,
) -> Result<login::Login, String> {
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
    Ok(login::Login { url, origin, user })
}

/// The folder a persona's tools are written in, in the project at `root`: under the app's
/// state, which every chat is denied writing ([`super::Denied::of`]).
pub fn folder(root: &Path, persona: &str) -> PathBuf {
    crate::names::state(root)
        .join("app")
        .join("tools")
        .join(persona)
}

/// The folders a sandboxed chat may write wherever it runs: the temporary folders, as written
/// and as the kernel names them.
fn temp_folders() -> Vec<PathBuf> {
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

/// A tool and the absolute program its shim runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub tool: Tool,
    pub program: PathBuf,
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
    let dirs: Vec<PathBuf> = search
        .iter()
        .filter(|dir| dir.is_absolute() && !inside_any(dir, &written))
        .cloned()
        .collect();
    let mut ready = Vec::new();
    let mut left = Vec::new();
    for tool in tools {
        if let Some(why) = locks.refuses_tool(persona, &tool.name) {
            left.push(why);
            continue;
        }
        // A browser login runs Playwright's program, never one the tool's name finds.
        let wanted = if tool.login.is_some() {
            login::PROGRAM
        } else {
            tool.name.as_str()
        };
        let found = dirs.iter().find_map(|dir| {
            let program = crate::programs::find(wanted, std::slice::from_ref(dir))?;
            let real = program.canonicalize().ok()?;
            (!inside_any(&real, &written)).then_some(program)
        });
        match found {
            Some(program) => ready.push(Ready {
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

/// The one hand-over `ready` declares, as `secret exec` spells it: `VAR=key`.
fn handed(ready: &Ready) -> String {
    format!("{}={}", ready.tool.var, ready.tool.key)
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
             \"<key>\" }} }}, and allow it on this machine; {terminal}."
        ));
    };
    if let Some(login) = &ready.tool.login {
        let want = login::hand_over(&ready.tool.key, login);
        if asked.vault == ready.tool.vault
            && asked.file == want.as_slice()
            && asked.env.is_empty()
            && asked.dotenv.is_empty()
        {
            return Ok(ready);
        }
        let files: String = want.iter().map(|spec| format!(" --file {spec}")).collect();
        return Err(format!(
            "{} is {persona}'s tool {}, and a chat runs it with exactly that: run `{}`, or \
             `purlis secret exec {}{files} -- {}`; {terminal}.",
            ready.tool.name,
            ready.tool.whole(),
            ready.tool.name,
            ready.tool.vault,
            ready.tool.name,
        ));
    }
    let want = handed(ready);
    let (asked_for, others) = match ready.tool.handed {
        Handed::File => (asked.file, [asked.env, asked.dotenv]),
        Handed::Env => (asked.env, [asked.file, asked.dotenv]),
    };
    let exact = asked.vault == ready.tool.vault
        && asked_for.len() == 1
        && asked_for[0] == want
        && others.iter().all(|other| other.is_empty());
    if !exact {
        let flag = match ready.tool.handed {
            Handed::File => "--file",
            Handed::Env => "--env",
        };
        return Err(format!(
            "{} is {persona}'s tool {}, and a chat runs it with exactly that: run `{}`, or \
             `purlis secret exec {} {flag} {want} -- {}`; {terminal}.",
            ready.tool.name,
            ready.tool.whole(),
            ready.tool.name,
            ready.tool.vault,
            ready.tool.name,
        ));
    }
    Ok(ready)
}

/// **What a declared tool's run is**, once [`declared`] took it: the tool's own program by its
/// absolute path, then the chat's arguments; for a browser login, `purlis browser _login` by
/// `purlis`'s absolute path, on the declared page, for the session the chat's arguments name and
/// nothing else ([`login::session_of`]). Refused where the arguments are anything more, or no
/// `purlis` is there to run it.
pub fn run_words(
    ready: &Ready,
    args: &[String],
    purlis: Option<&Path>,
) -> Result<Vec<String>, String> {
    let Some(login) = &ready.tool.login else {
        return Ok(std::iter::once(ready.program.display().to_string())
            .chain(args.iter().cloned())
            .collect());
    };
    let session = login::session_of(args, &ready.tool.name, &login.url)?;
    let Some(purlis) = purlis else {
        return Err(format!(
            "{} logs in through purlis's own command line, and none was found beside the app, so \
             nothing ran",
            ready.tool.name
        ));
    };
    Ok(login::words(purlis, &ready.program, login, &session))
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
/// dynamic loader's, and an interpreter's or runtime's start-up options. A brokered run from a
/// chat starts without them (D-PE-ruling-2), so the chat cannot have a declared tool, or an
/// approved server's exact line, run its own code beside the credential.
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
    if ready.tool.login.is_some() {
        return format!(
            "#!/bin/sh\n# Written by purlis as a chat of {persona} starts (D-PE-browser-1). A \
             plain `{name}` in that chat\n# asks the app to log a browser in to {url} with a \
             login from the {vault} vault:\n# the chat never reads the vault. Edits here are \
             overwritten.\n\
             if [ -z \"{channel}\" ]; then\n\
             echo 'purlis: {name} logs a browser in only in a chat the app started' >&2\n\
             exit 1\nfi\nexec {words} \"$@\"\n",
            name = ready.tool.name,
            url = ready
                .tool
                .login
                .as_ref()
                .map_or("", |login| login.url.as_str()),
            vault = ready.tool.vault,
            words = words.join(" "),
        );
    }
    format!(
        "#!/bin/sh\n# Written by purlis as a chat of {persona} starts (#1407). A plain `{name}` in \
         that chat runs\n# through `purlis secret exec`, which hands it {var} from the {vault} \
         vault: the chat\n# never reads the vault. Edits here are overwritten.\n\
         if [ -z \"{channel}\" ]; then\n\
         # No app to ask from here: inside a `secret exec` run already, which handed the value \
         on, or\n# outside the chat. The program itself, with nothing added.\n\
         exec {program} \"$@\"\nfi\nexec {words} \"$@\"\n",
        name = ready.tool.name,
        var = ready.tool.var,
        vault = ready.tool.vault,
        program = quoted(&ready.program.display().to_string()),
        words = words.join(" "),
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
#[path = "tools_tests.rs"]
mod tests;

#[cfg(all(test, unix))]
#[path = "tools_login_tests.rs"]
mod login_tests;
