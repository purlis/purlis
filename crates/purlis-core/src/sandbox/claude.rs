// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The policy compiled for Claude Code: a `sandbox` object and `permissions.deny` rules, in
//! the `--settings` charter already hands each chat ([`crate::harness`]).
//!
//! Read out of Claude Code 2.1.285's settings schema, and measured live against it on macOS:
//!
//! - `sandbox` confines the commands its Bash tool runs. Its Read and Edit tools run in its
//!   own process, and a `permissions.deny` rule is what holds them, so every denied path is
//!   written twice: once for the sandbox and once for the tools.
//! - Its web tools run in its own process too, and are not held to the sandbox's allowed hosts
//!   (measured: a fetch of an unlisted host succeeded under `strictAllowlist`). No setting
//!   limits them to a list of hosts, and an allow cannot outrank a deny, so both are denied.
//! - `--settings` sits above project settings on each key it names. Whether a sandboxed chat
//!   loads its project's own settings files at all is decided at its start
//!   ([`crate::harness::claude::project`], ADR 0067 as amended 2026-10-15): only where they hold
//!   nothing beyond the narrowing and cosmetic keys it keeps. Otherwise the chat runs on the
//!   person's settings and these alone, and the person's own are never written by the chat.
//! - Where a class is held by a service no key in its settings can deny, and the service does not
//!   hold it either, the chat is refused ([`super::NotStarted::Uncompilable`]) rather than
//!   started without it. Its sandbox lets a command ask the keychain's service (its profile
//!   allows the lookup), so on macOS the vaults class rests on the keyring items' own access
//!   rule (ruling V90a): a command that reads one is refused or the person is asked.
//!
//! Measured live on 2.1.285 on macOS, a headless chat started with what [`settings`] writes: a
//! command could neither read nor write a denied directory, could write its own directory,
//! reached a listed host, and was refused an unlisted one; and with the web tools denied they
//! were not offered to the model at all, even when named in `--allowedTools`.
//!
//! **The later-code class** (ruling V73b). Measured on 2.1.288 on macOS, a headless turn against
//! a stand-in model server: Claude Code's own sandbox already kept a command from writing git's
//! config and hooks (in a nested clone too), `.mcp.json`, `.vscode`, `.idea`, `.claude`'s
//! settings, commands and agents, and shell startup files, and let it write `opencode.json`,
//! `.opencode`, `.codex`, `.envrc` and `charter.toml`. With each name added to `denyWrite` as
//! `**/<name>`, every one was refused at any depth, and an ordinary file was still written.
//! `.claude/skills`, `tui.json`, `tui.jsonc` and `.agents`, added later (#1057), are held by
//! the same `**/<name>` rule. The project's manifests are not (#1336): they are denied by path,
//! at the project root and in each folder from the chat's up to it ([`Settings::denying`]).
//! No glob can deny a `.git` itself without denying what git writes below it (#1065). The
//! `config` of each git folder was held by name already; now a repository's git folder that is
//! there when the chat starts is held as a whole too ([`super::Denied::git_folder_pins`]): its
//! `config` is denied by path as well, and Claude Code pins each folder above a denied path (as
//! below), so a `.git` directory, and every folder above it, stays where it is; a `.git` file is
//! denied itself. Measured on macOS (`tools/seatbelt-case.sh`).
//!
//! **The hook socket** (ADR 0067 §2, #1328). A `purlis` command a chat runs is part of the chat
//! and asks the app over the socket in `$PURLIS_HOOK_SOCKET`; Claude Code's sandbox refuses
//! every unix socket it is not told to allow. The socket is known only where the chat opens, so
//! [`Settings::reporting_on`] adds it then, as the one path in `network.allowUnixSockets`.
//! Read out of Claude Code 2.1.291's own settings schema: `allowUnixSockets` is a list of
//! paths, *"macOS only … Ignored on Linux (seccomp cannot filter by path)"*, merged across
//! settings sources, and its macOS profile allows each as a `subpath` for connect **and bind**.
//! The bind is harmless only while the chat cannot replace the socket, so the socket's folder is
//! denied to writes beside it: under the project's state folder that is denied already, and the
//! fallback folder in the temp directory is not. `allowAllUnixSockets` is set `false`, never
//! left out, so a user's `true` cannot merge in. On Linux a sandboxed chat reaches no socket.
//!
//! **Through purlis's proxy** (#1665). Read out of Claude Code 2.1.296's own sandbox, and in its
//! docs: from 2.1.285, `network.httpProxyPort` and `network.socksProxyPort` replace its own
//! proxy with the one listening there, and a port left out is still its own, so purlis names
//! both ([`Settings::through`]) or neither. Once they are set, the proxy decides every
//! connection, and Claude Code's allowed domains, its approval prompts and its check of local
//! addresses stop applying; purlis's proxy runs that check itself ([`super::reach`]). The
//! allowed domains are still written: Claude Code restricts a command's network only while
//! they are set. Its macOS profile then lets a command connect to those two loopback ports and
//! no other, **unless a settings source turns `allowLocalBinding` on**: that adds every loopback
//! port, every local service among them. So purlis names it `false`, never leaves it out,
//! which outranks a user's or a project's `true` (read in 2.1.296's settings merge: a `false`
//! in force wins; an administrator's settings still outrank purlis's). A Claude Code older
//! than 2.1.285 keeps its own proxy and the allowed domains, as before
//! ([`super::Applied::answered`]).
//!
//! **What a project's sandbox widens** (spec #1330, #1337). Read out of Claude Code 2.1.291's
//! own settings schema and its profile builder:
//! - `filesystem.allowWrite` is *"Additional paths to allow writing within the sandbox"*,
//!   absolute or `~`-expanded, a glob compiled to a regular expression. It carries the project's
//!   package caches ([`super::Widened::caches`]): each tool's folder and cargo's few files by
//!   path, and inside each of cargo's bare repositories by `<db>/*/**`, which never matches an
//!   entry itself. Each path by both names where a link stands in between; left out when there
//!   are none. Every `denyWrite` still wins inside them.
//! - Each folder purlis made is pinned: Claude Code denies `file-write-unlink` and
//!   `file-write-create` on every folder above a `denyWrite` entry, so a `.git` denied at each
//!   cache folder's top pins that folder, and a bare repository's `config` and `hooks` denied
//!   pin cargo's git database.
//! - `enableWeakerNetworkIsolation` is *"macOS only: Allow access to com.apple.trustd.agent in
//!   the sandbox. Needed for Go-based CLI tools (gh, gcloud, terraform, etc.) to verify TLS
//!   certificates"*; its profile adds exactly `(allow mach-lookup (global-name
//!   "com.apple.trustd.agent"))`. It is `true` only where the project's `certificate-checks` is
//!   (D-1337-7), and `false` otherwise, never left out, so a user's `true` does not merge in.
//!
//! **On a runner** this sandbox is switched off, and purlis wraps the whole chat instead
//! ([`super::claude_wrap`], #4083): its tools keep the deny rules compiled here.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Access, Compiled, Level, Os, Uncompilable};
use crate::harness::Harness;

/// `path` as written, and as the kernel names it where that differs.
fn both_names(path: &Path) -> Vec<std::path::PathBuf> {
    let resolved = super::real(path);
    if resolved == path {
        vec![path.to_path_buf()]
    } else {
        vec![path.to_path_buf(), resolved]
    }
}

/// **Claude Code's shared temp folder**, by both of its names on macOS: what its sandbox lets
/// every command write besides the folders its settings name, on every system (read in
/// 2.1.300's sandbox runtime, whose list of paths always written holds exactly these two). A
/// chat whose commands write only the temp folder it is handed is denied it
/// ([`super::Compiled::own_temp_only`], #4476). On Linux a path that is not there is left out
/// of what a command writes, so a chat there was refused it only while no Claude Code had made
/// it.
pub const SHARED_TEMP: [&str; 2] = ["/tmp/claude", "/private/tmp/claude"];

/// Claude Code's own tools that reach the network from its process rather than through its
/// sandbox, so the sandbox's allowed hosts do not hold them.
pub const WEB_TOOLS: [&str; 2] = ["WebFetch", "WebSearch"];

/// Claude Code's own browser tools, by the permission rule that names every tool of their
/// server: its Chrome integration, which drives the person's own Chrome. A chat purlis
/// sandboxes is offered none of them, at either level (#2013, spec #2008): the browser a chat
/// drives is purlis's, the one the person watches and whose reach purlis holds.
pub const BROWSER_TOOLS: [&str; 1] = ["mcp__claude-in-chrome"];

/// What a Claude Code chat's `--settings` carries for the sandbox.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The `sandbox` object.
    pub sandbox: Value,
    /// `permissions.deny` rules for its own tools.
    pub deny: Vec<String>,
    /// Folders made, empty, where there is none, before the chat starts ([`Self::make_empty`]):
    /// at the floor, each folder in the home above a name a person may lift
    /// ([`super::MAY_LIFT`]). Claude Code pins every folder above a held path, so neither a
    /// tool's first run nor a lift could make them in the chat.
    pub made_folders: Vec<PathBuf>,
    /// The entries of `denyWrite` that are a repository's git folder pin and nothing else
    /// ([`super::Denied::git_folder_pins`]): what [`Self::written_in`] leaves out where no command
    /// writes (the person's ruling, 2026-11-16, #6961).
    pub pins: Vec<String>,
    /// The folders the person's own and the administrators' Claude Code settings let a command
    /// write, read as the chat starts ([`persons_write_roots`]): places a pin is kept in too.
    pub persons_roots: Vec<String>,
}

/// The folders above each name of [`super::MAY_LIFT`] in `home`, outermost first, `home` and the
/// names themselves left out ([`Settings::made_folders`]).
pub(super) fn above_what_may_be_lifted(home: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for name in super::MAY_LIFT {
        let mut at = home.to_path_buf();
        let parts: Vec<_> = Path::new(name).components().collect();
        for part in &parts[..parts.len().saturating_sub(1)] {
            at.push(part);
            if !out.contains(&at) {
                out.push(at.clone());
            }
        }
    }
    out
}

/// macOS's terminals, by a glob. Claude Code turns it into the regex `^/dev/ttys[^/]*(/.*)?$` in
/// its profile.
pub const TERMINALS: &str = "/dev/ttys*";

/// `compiled`, for Claude Code — or the class it cannot hold on this machine.
///
/// The credential store is held by the store on macOS (ruling V90a) and refused elsewhere:
/// Claude Code's settings have no key that denies it (`Compiled::holds_every_service`).
///
/// No denial reaching here covers `/`, the home directory, the project or a folder above it:
/// [`super::for_start`] refuses that chat, and [`super::Applied::line`] refuses one whose own
/// folder a denial covers ([`super::covering`], #1327).
pub fn settings(compiled: &Compiled) -> Result<Settings, Uncompilable> {
    compiled.holds_every_service(Harness::ClaudeCode)?;
    Ok(rules(compiled))
}

/// [`settings`], whatever services hold here: a chat purlis wraps whole holds those itself
/// ([`super::claude_wrap`]), and answers to these rules with its Read, Edit and web tools.
pub(super) fn rules(compiled: &Compiled) -> Settings {
    let mut deny_read = Vec::new();
    let mut deny_write = Vec::new();
    let mut read_rules = Vec::new();
    let mut edit_rules = Vec::new();
    for (denial, path) in names_of(&compiled.denied.paths) {
        for path in spelled(denial, &path) {
            let rooted = format!("/{path}");
            if denial.access == Access::ReadWrite {
                deny_read.push(path.clone());
                read_rules.push(format!("Read({rooted})"));
                read_rules.push(format!("Read({rooted}/**)"));
            }
            deny_write.push(path);
            edit_rules.push(format!("Edit({rooted})"));
            edit_rules.push(format!("Edit({rooted}/**)"));
        }
    }
    // No project's vaults are read, at either level (#2071): under every spelling of a state
    // folder, anywhere. The chat's own project's are denied by path above.
    for state in crate::names::STATE_DIR.spellings() {
        for kept in [
            "vaults",
            crate::secrets::fingerprint::KEY_FILE,
            crate::secrets::links::FILE,
        ] {
            let name = format!("{state}/{kept}");
            for (plain, cased) in [
                (format!("/**/{name}"), format!("/**/{}", either_case(&name))),
                (
                    format!("/**/{name}/**"),
                    format!("/**/{}/**", either_case(&name)),
                ),
            ] {
                for rule in [plain, cased] {
                    read_rules.push(format!("Read(/{rule})"));
                    edit_rules.push(format!("Edit(/{rule})"));
                    deny_read.push(rule.clone());
                    deny_write.push(rule);
                }
            }
        }
    }
    // On macOS, a chat's commands keep to its own terminal, at either level. They are given none
    // of their own, and Claude Code and its hooks run outside the profile, so this holds only
    // what a command opens by name. The floor denies the writes too ([`TERMINALS`]). Claude Code
    // writes its pty rules after these, so its `allowPty`, were it ever set, would open them again.
    if compiled.os == Os::MacOs {
        deny_read.push(TERMINALS.to_owned());
        read_rules.push(format!("Read(/{TERMINALS})"));
        edit_rules.push(format!("Edit(/{TERMINALS})"));
    }
    // The later-code class (ruling V73b), by name at any depth: added to the names Claude
    // Code's own sandbox already keeps commands from writing, never in their place. What these
    // hold is a command's write of the name; one that arrives another way is told, not refused
    // ([`super::arrived`]). A name
    // that holds only itself (`.git`) is left out: a glob here denies what is below it too,
    // and git writes there. A glob can't be written past one path, so a name a person lifted for
    // this chat goes whole ([`super::grant::lift`]; its Notice says so).
    let held = |planted: &&super::Planted| {
        planted.reach == super::Reach::AndBelow
            && !compiled
                .widened
                .lifted
                .iter()
                .any(|folder| folder.ends_with(planted.path))
    };
    for planted in super::PLANTED.iter().filter(held) {
        let glob = format!("**/{}", planted.path);
        deny_write.extend(plain_and_cased("**/", planted.path));
        edit_rules.push(format!("Edit({glob})"));
        edit_rules.push(format!("Edit({glob}/**)"));
    }
    // Under each granted folder (#1342), every later-code name again, by absolute path: a
    // relative `**/<name>` is read against the chat's folder, so it is not known to hold in a
    // folder outside it (D-1342-3). The folder is the kernel's spelling already (D-1342-12).
    for folder in &compiled.writable {
        for planted in super::PLANTED.iter().filter(held) {
            let glob = format!("{}/**/{}", folder.display(), planted.path);
            edit_rules.push(format!("Edit(/{glob})"));
            edit_rules.push(format!("Edit(/{glob}/**)"));
            deny_write.extend(plain_and_cased(
                &format!("{}/**/", folder.display()),
                planted.path,
            ));
        }
        // And every project manifest under it (D-T55-4): #1336 holds them by path only in the
        // chat's own folders, which a granted folder is not.
        for name in super::MANIFESTS {
            let glob = format!("{}/**/{name}", folder.display());
            edit_rules.push(format!("Edit(/{glob})"));
            deny_write.extend(plain_and_cased(&format!("{}/**/", folder.display()), name));
        }
    }
    // Each repository's git folder that was there as the chat started, held as a whole: a path
    // in or at it is denied ([`super::Denied::git_folder_pins`]), and Claude Code pins every
    // folder above a denied path (see the caches below), so a `.git` directory stays where it is
    // while git writes the rest of it, and a `.git` file is held itself. Each as a plain path, by
    // every name a rule on it is written by: a glob gets no folder above it pinned. The Edit
    // tool pins nothing, so for it every branch folder's git in a plane is one rule (#6961): a
    // plane holds hundreds, and each by path weighs the settings past auto mode's check.
    let pinned_from = deny_write.len();
    for pin in &compiled.denied.git_folder_pins {
        for name in names_for(pin) {
            let path = name.display().to_string();
            match every_branch_folders_git(&path) {
                Some(glob) => edit_rules.push(format!("Edit(/{glob})")),
                None => {
                    edit_rules.push(format!("Edit(/{path})"));
                    edit_rules.push(format!("Edit(/{path}/**)"));
                }
            }
            deny_write.push(path);
        }
    }
    let pinned: Vec<String> = deny_write[pinned_from..].to_vec();
    // The project's package caches (D-1337-6), each path as written and as the kernel names it.
    let mut allow_write: Vec<String> = Vec::new();
    if let Some(caches) = &compiled.widened.caches {
        let mut allow = |path: String| {
            if !allow_write.contains(&path) {
                allow_write.push(path);
            }
        };
        for path in caches.trees.iter().chain(&caches.files) {
            for name in both_names(path) {
                allow(name.display().to_string());
            }
        }
        // Inside each bare repository, never an entry itself: a glob is a regular expression
        // in Claude Code's profile, and `*/**` matches nothing at the first level.
        for bare in &caches.bare {
            for name in both_names(bare) {
                allow(format!("{}/*/**", name.display()));
            }
        }
        // Claude Code pins every folder above a denied path, so it is not removed, renamed or
        // made (measured in 2.1.291's profile builder: each ancestor of a `denyWrite` entry is
        // denied `file-write-unlink` and `file-write-create` as a literal). A repository's
        // `.git` at a tree's top is denied for that pin, and what a later git runs in each
        // bare repository (ADR 0067 §5) pins the database.
        let mut deny = |path: String| {
            deny_write.push(path.clone());
            edit_rules.push(format!("Edit(/{path})"));
            edit_rules.push(format!("Edit(/{path}/**)"));
        };
        for tree in &caches.trees {
            for name in both_names(tree) {
                deny(format!("{}/.git", name.display()));
            }
        }
        for bare in &caches.bare {
            for name in both_names(bare) {
                for run in super::caches::BARE_RUN {
                    deny(format!("{}/*/{run}", name.display()));
                }
            }
        }
    }
    // A person's grants (#1342), each exactly as judged: already the kernel's spelling
    // (D-1342-12), never resolved again here, so a folder swapped for a link since is not
    // followed to where it now points. Claude Code holds `denyWrite` over `allowWrite`, so a
    // class inside one still wins.
    for path in &compiled.writable {
        let path = path.display().to_string();
        if !allow_write.contains(&path) {
            allow_write.push(path);
        }
    }
    // The person's own Claude Code settings (ADR 0067 as amended 2026-10-15): a sandboxed chat
    // runs on them, so neither its commands nor its Edit tool write them. The floor holds them
    // below, with every later-code name in the home.
    if compiled.level != Level::Floor
        && let Some(home) = &compiled.homes.home
    {
        for file in [".claude/settings.json", ".claude/settings.local.json"] {
            for name in names_for(home) {
                for rule in plain_and_cased(&format!("{}/", name.display()), file) {
                    edit_rules.push(format!("Edit(/{rule})"));
                    deny_write.push(rule);
                }
            }
        }
    }
    // The floor (#2071): no folder limits, so every folder is written but what it holds.
    if compiled.level == Level::Floor {
        allow_write = vec!["/".to_owned()];
        // The terminals, which the full sandbox already leaves out of what a command writes.
        if compiled.os == Os::MacOs {
            deny_write.push(TERMINALS.to_owned());
        }
        // Every later-code name wherever it is, the home's among them: a relative `**/<name>`
        // is read against the chat's folder, and at the floor every folder is written.
        for planted in super::PLANTED.iter().filter(held) {
            let mut globs = vec![(
                format!("/**/{}", planted.path),
                format!("/**/{}", either_case(planted.path)),
            )];
            if let Some(home) = &compiled.homes.home
                && !planted.path.contains("**")
            {
                for name in names_for(home) {
                    let name = name.display();
                    globs.push((
                        format!("{name}/{}", planted.path),
                        format!("{name}/{}", either_case(planted.path)),
                    ));
                }
            }
            for (plain, cased) in globs {
                for rule in [plain, cased] {
                    edit_rules.push(format!("Edit(/{rule})"));
                    edit_rules.push(format!("Edit(/{rule}/**)"));
                    deny_write.push(rule);
                }
            }
        }
        // A name lifted for this chat leaves no entry under its folder above, which a held path's
        // entry pins: held by [`super::PIN`] beside it instead, so that folder is never moved
        // aside, or swapped for a link, while the chat writes the name. On macOS alone, where a
        // path that is not there is only held; Claude Code's sandbox on Linux puts something at it.
        if compiled.os == Os::MacOs
            && let Some(home) = &compiled.homes.home
        {
            for planted in super::PLANTED.iter().filter(|planted| {
                planted.reach == super::Reach::AndBelow
                    && !planted.path.contains("**")
                    && !held(planted)
            }) {
                let Some(above) = Path::new(planted.path)
                    .parent()
                    .filter(|above| !above.as_os_str().is_empty())
                else {
                    continue;
                };
                for name in names_for(home) {
                    let pin = name.join(above).join(super::PIN).display().to_string();
                    edit_rules.push(format!("Edit(/{pin})"));
                    deny_write.push(pin);
                }
            }
        }
        // Every project's state folder and manifests, wherever they are (#2071): the full sandbox
        // writes no project but the chat's own, and none of these in it. Claude Code's
        // `denyWrite` cannot be lifted inside, so its own project's nested manifests are held
        // too (D-391-8).
        let every: Vec<(String, String)> = crate::names::STATE_DIR
            .spellings()
            .flat_map(|state| {
                [
                    (
                        format!("/**/{state}"),
                        format!("/**/{}", either_case(state)),
                    ),
                    (
                        format!("/**/{state}/**"),
                        format!("/**/{}/**", either_case(state)),
                    ),
                ]
            })
            .chain(
                super::MANIFESTS
                    .iter()
                    .map(|name| (format!("/**/{name}"), format!("/**/{}", either_case(name)))),
            )
            // And every workspace's manifest (#2046).
            .chain([(
                format!("/**/{}/*/{}", super::WORKSPACES, super::WORKSPACE_MANIFEST),
                format!(
                    "/**/{}/*/{}",
                    either_case(super::WORKSPACES),
                    either_case(super::WORKSPACE_MANIFEST)
                ),
            )])
            .collect();
        for (plain, cased) in every {
            for rule in [plain, cased] {
                edit_rules.push(format!("Edit(/{rule})"));
                deny_write.push(rule);
            }
        }
        for path in &compiled.held {
            for name in names_for(path) {
                let name = name.display().to_string();
                edit_rules.push(format!("Edit(/{name})"));
                edit_rules.push(format!("Edit(/{name}/**)"));
                deny_write.push(name);
            }
        }
    }
    // An outcome eval's shell writes no temp folder but its own (#4476): Claude Code lets every
    // command write its shared one, whatever the settings say, so it is denied by name.
    if compiled.own_temp_only {
        for path in SHARED_TEMP {
            edit_rules.push(format!("Edit(/{path})"));
            edit_rules.push(format!("Edit(/{path}/**)"));
            deny_write.push(path.to_owned());
        }
    }
    // A pin is a pin alone where nothing else wrote the same entry.
    let pins: Vec<String> = pinned
        .iter()
        .filter(|pin| {
            deny_write.iter().filter(|it| it == pin).count()
                == pinned.iter().filter(|it| it == pin).count()
        })
        .cloned()
        .collect();
    let mut filesystem = json!({
        "denyRead": deny_read,
        "denyWrite": deny_write,
    });
    if !allow_write.is_empty() {
        filesystem["allowWrite"] = json!(allow_write);
    }
    if compiled.level == Level::Floor {
        return floor(filesystem, read_rules, edit_rules, compiled, pins);
    }
    let deny = WEB_TOOLS
        .iter()
        .chain(BROWSER_TOOLS.iter())
        .map(|tool| (*tool).to_owned())
        .chain(read_rules)
        .chain(edit_rules)
        .collect();
    let sandbox = json!({
            "enabled": true,
            "failIfUnavailable": true,
            "allowUnsandboxedCommands": false,
            "enableWeakerNetworkIsolation": compiled.widened.trust,
            "network": {
                "allowedDomains": compiled.hosts,
                "strictAllowlist": true,
                "allowAllUnixSockets": false,
                "allowLocalBinding": false,
            },
            "filesystem": filesystem,
    });
    let mut sandbox = sandbox;
    // The sockets a person allowed this chat (D-sock-1), as the kernel names each: written only
    // where there is one, so a chat allowed none is compiled as it always was.
    if !compiled.widened.sockets.is_empty() {
        sandbox["network"]["allowUnixSockets"] = json!(
            compiled
                .widened
                .sockets
                .iter()
                .map(|socket| socket.display().to_string())
                .collect::<Vec<_>>()
        );
    }
    Settings {
        sandbox,
        deny,
        made_folders: Vec::new(),
        pins,
        persons_roots: persons_write_roots(compiled),
    }
    .tidied()
}

/// **The floor's settings** (#2071): Claude Code's sandbox on, holding `filesystem` (every
/// denial class, and the floor's held folders, with every other folder writable), and its
/// network through the chat's own proxy, whose reach carries every host
/// ([`super::reach::Reach::anything`]): the proxy's ports are added where the chat opens
/// ([`Settings::through`]), and no domain is listed, so Claude Code's own list and its prompts
/// decide nothing. Local binding stays off, so a command reaches no port of this machine but the
/// proxy's. Its web tools are allowed. Unix sockets stay as at the full sandbox: none but the ones
/// a person allowed (the operator's ruling Q1), and the hook socket the chat opens with.
fn floor(
    filesystem: Value,
    read_rules: Vec<String>,
    edit_rules: Vec<String>,
    compiled: &Compiled,
    pins: Vec<String>,
) -> Settings {
    let mut sandbox = json!({
        "enabled": true,
        "failIfUnavailable": true,
        "allowUnsandboxedCommands": false,
        "enableWeakerNetworkIsolation": compiled.widened.trust,
        "network": {
            "allowAllUnixSockets": false,
            "allowLocalBinding": false,
        },
        "filesystem": filesystem,
    });
    if !compiled.widened.sockets.is_empty() {
        sandbox["network"]["allowUnixSockets"] = json!(
            compiled
                .widened
                .sockets
                .iter()
                .map(|socket| socket.display().to_string())
                .collect::<Vec<_>>()
        );
    }
    Settings {
        sandbox,
        deny: BROWSER_TOOLS
            .iter()
            .map(|tool| (*tool).to_owned())
            .chain(read_rules)
            .chain(edit_rules)
            .collect(),
        made_folders: match &compiled.homes.home {
            Some(home) if compiled.level == Level::Floor => above_what_may_be_lifted(home),
            _ => Vec::new(),
        },
        pins,
        persons_roots: persons_write_roots(compiled),
    }
    .tidied()
}

impl Settings {
    /// These settings with each rule and each denied path written once ([`fewest`], [`once`]).
    fn tidied(mut self) -> Settings {
        self.deny = fewest(std::mem::take(&mut self.deny));
        for list in ["denyRead", "denyWrite"] {
            if let Some(paths) = strings_of(&self.sandbox["filesystem"][list]) {
                self.sandbox["filesystem"][list] = json!(once(paths));
            }
        }
        self
    }

    /// These settings, `before` with more added, with each rule and path added written only
    /// where what is there does not hold it already: what `before` had is kept as it was, and
    /// leads each list, so a caller that reads the added rules after `before`'s
    /// ([`crate::harness::claude::reporting_on`]) reads exactly them.
    fn tidied_after(mut self, before: &Settings) -> Settings {
        self.deny = added_once(&before.deny, &self.deny, fewest);
        for list in ["denyRead", "denyWrite"] {
            let (Some(had), Some(now)) = (
                strings_of(&before.sandbox["filesystem"][list]),
                strings_of(&self.sandbox["filesystem"][list]),
            ) else {
                continue;
            };
            self.sandbox["filesystem"][list] = json!(added_once(&had, &now, once));
        }
        self
    }

    /// **These settings for a chat that opens in `cwd`, each git folder pin written only where a
    /// command may write** (the person's ruling, 2026-11-16, #6961). A pin is a plain path in
    /// `denyWrite`, so that Claude Code pins every folder above it too; a plane holds hundreds,
    /// one a branch folder, and each by both names weighs the settings past what auto mode's
    /// check takes. Where no command may write, a pin holds and pins nothing more: Claude Code
    /// refuses every write outside the places its sandbox writes. So a pin is kept where it
    /// meets one, below or above it: `cwd`, each `allowWrite` entry (a person's grants, the
    /// caches; a glob by its plain part), each write root of the person's own and the
    /// administrators' settings as the chat started ([`persons_write_roots`]; one added while
    /// the chat runs counts from its next start), and Claude Code's shared temp folder ([`SHARED_TEMP`],
    /// the only paths its sandbox always writes), each by both its names; and `/`, at the floor,
    /// meets every pin. The chat's own temp folder is made new for it as it starts
    /// ([`super::Confinement::proxy_only`]), so no pin is in or above it. The Edit tool's pins are kept whole ([`every_branch_folders_git`]).
    pub fn written_in(&self, cwd: &Path) -> Settings {
        let mut out = self.clone();
        let mut places: Vec<String> = Vec::new();
        let mut place = |path: &Path| {
            for name in both_names(path) {
                let name = name.display().to_string();
                if !places.contains(&name) {
                    places.push(name);
                }
            }
        };
        place(cwd);
        for root in &self.persons_roots {
            place(Path::new(root));
        }
        for shared in SHARED_TEMP {
            place(Path::new(shared));
        }
        for allowed in strings_of(&self.sandbox["filesystem"]["allowWrite"]).unwrap_or_default() {
            let plain = allowed
                .split(['*', '?', '['])
                .next()
                .unwrap_or_default()
                .trim_end_matches('/');
            place(Path::new(if plain.is_empty() { "/" } else { plain }));
        }
        let meets = |pin: &str| {
            places.iter().any(|place| {
                let under = |inner: &str, outer: &str| {
                    outer == "/" || inner == outer || inner.starts_with(&format!("{outer}/"))
                };
                under(pin, place) || under(place, pin)
            })
        };
        // DO NOT MERGE: mutation M1, every pin left out.
        let away: Vec<&String> = self.pins.iter().filter(|pin| !meets(pin) || true).collect();
        // DO NOT MERGE: mutation M3, the folder of every workspace written.
        if let (Some(workspaces), Some(filesystem)) = (
            cwd.ancestors().nth(4),
            out.sandbox["filesystem"].as_object_mut(),
        ) {
            let list = filesystem.entry("allowWrite").or_insert_with(|| json!([]));
            if let Some(list) = list.as_array_mut() {
                list.push(json!(workspaces.display().to_string()));
            }
        }
        if let Some(list) = out.sandbox["filesystem"]["denyWrite"].as_array_mut() {
            list.retain(|it| {
                !it.as_str()
                    .is_some_and(|it| away.iter().any(|pin| *pin == it))
            });
        }
        out
    }

    /// Whether these switch Claude Code's own sandbox off: a chat purlis wraps whole
    /// ([`super::claude_wrap`]).
    pub fn is_off(&self) -> bool {
        self.sandbox.get("enabled") == Some(&Value::Bool(false))
    }

    /// Makes each of [`Self::made_folders`] outside the sandbox, empty, where nothing is there:
    /// what is already there keeps what it holds, and a link, even one to nothing, is neither
    /// followed nor replaced. One not made is left to the chat, whose tool then says why.
    pub fn make_empty(&self) {
        for folder in &self.made_folders {
            if std::fs::symlink_metadata(folder).is_ok() {
                continue;
            }
            // Never through a link: each folder above it that is made here must be a folder.
            let through_a_link = folder.parent().is_some_and(|parent| {
                self.made_folders.iter().any(|made| made == parent)
                    && !std::fs::symlink_metadata(parent).is_ok_and(|it| it.file_type().is_dir())
            });
            if through_a_link {
                tracing::warn!(
                    "purlis: {} was not made for the chat: a folder above it is not a folder",
                    folder.display()
                );
                continue;
            }
            if let Err(err) = std::fs::create_dir(folder) {
                tracing::warn!(
                    "purlis: {} was not made for the chat ({err})",
                    folder.display()
                );
            }
        }
    }

    /// These settings with `held` folders never written, at the floor (#2071): each under every
    /// name a rule on it is written by, to the sandbox and to the Edit tool.
    pub fn holding(&self, held: &[PathBuf]) -> Settings {
        let mut out = self.clone();
        for path in held {
            for name in names_for(path) {
                let name = name.display().to_string();
                if let Some(list) = out.sandbox["filesystem"]["denyWrite"].as_array_mut() {
                    list.push(json!(name));
                }
                out.deny.push(format!("Edit(/{name})"));
                out.deny.push(format!("Edit(/{name}/**)"));
            }
        }
        out.tidied_after(self)
    }

    /// These settings for a chat whose commands reach the network through purlis's proxy
    /// (#1665): `ports`, its HTTP and then its SOCKS5 port, as `network.httpProxyPort` and
    /// `network.socksProxyPort`, and nothing else changed.
    ///
    /// **Both, always**, which a pair of ports holds by its type: with one left out, Claude Code
    /// starts its own proxy for that one, and a command reaches whatever it allows there. The
    /// allowed domains stay: Claude Code restricts a command's network only while they are set,
    /// and with the ports set it hands every connection to the proxy, which decides by the same
    /// hosts ([`super::reach`]).
    pub fn through(&self, ports: [u16; 2]) -> Settings {
        let mut out = self.clone();
        let [http, socks] = ports;
        out.sandbox["network"]["httpProxyPort"] = json!(http);
        out.sandbox["network"]["socksProxyPort"] = json!(socks);
        out
    }

    /// These settings for a chat whose hooks and `purlis` commands report on `socket`: as
    /// compiled, with that one socket, as the kernel names it, as the only path in
    /// `network.allowUnixSockets` (ADR 0067 §2: the chat may connect to that socket and
    /// nothing more), and its folder denied to writes, to the sandbox and to the Edit tool, so
    /// the bind Claude Code grants with it can never take the app's place. Without a socket it
    /// is the compiled settings, which allow none.
    pub fn reporting_on(&self, socket: Option<&Path>) -> Settings {
        let mut out = self.clone();
        let Some(socket) = socket else {
            return out;
        };
        // The hook socket first, then each a person allowed (D-sock-1).
        let hook = json!(super::real(socket).display().to_string());
        let mut sockets = vec![hook.clone()];
        if let Some(granted) = out.sandbox["network"]["allowUnixSockets"].as_array() {
            sockets.extend(granted.iter().filter(|one| **one != hook).cloned());
        }
        out.sandbox["network"]["allowUnixSockets"] = json!(sockets);
        // The folder under every name a rule on it is written by, as a denied path is
        // ([`names_for`]): through a link and across a firmlink too (#1418).
        if let Some(folder) = socket.parent() {
            for name in names_for(folder) {
                let folder = name.display().to_string();
                if let Some(denied) = out.sandbox["filesystem"]["denyWrite"].as_array_mut() {
                    denied.push(json!(folder));
                }
                out.deny.push(format!("Edit(/{folder})"));
                out.deny.push(format!("Edit(/{folder}/**)"));
            }
        }
        out.tidied_after(self)
    }

    /// These settings with `denied` denied too, after the compiled rules: each path as written
    /// and as the kernel names it where that differs, to the sandbox and to the Read and Edit
    /// tools, as [`settings`] writes the compiled ones. The manifests between the project root
    /// and the chat's folder, which only the place the chat opens knows (#1336).
    pub fn denying(&self, denied: &[super::Denial]) -> Settings {
        let mut out = self.clone();
        for (denial, path) in names_of(denied) {
            for path in spelled(denial, &path) {
                let filesystem = &mut out.sandbox["filesystem"];
                if denial.access == Access::ReadWrite {
                    if let Some(list) = filesystem["denyRead"].as_array_mut() {
                        list.push(json!(path));
                    }
                    out.deny.push(format!("Read(/{path})"));
                    out.deny.push(format!("Read(/{path}/**)"));
                }
                if let Some(list) = filesystem["denyWrite"].as_array_mut() {
                    list.push(json!(path));
                }
                out.deny.push(format!("Edit(/{path})"));
                out.deny.push(format!("Edit(/{path}/**)"));
            }
        }
        out.tidied_after(self)
    }
}

/// Each of `denied` under every name a rule on it is written by: as written, and as the kernel
/// names it where that differs, so a path through a link (a linked `~/.config`, `/tmp`, `/var`)
/// is denied by both names and a rule matches whichever name the sandbox or a tool compares
/// (FD-27); and, where its last part is a link, that link in its folder as the kernel names
/// it, so the link itself is held as well as its target ([`super::seatbelt::held_names`], #1336).
/// Each of those is also denied on the other side of a macOS firmlink (`/Users` and
/// `/System/Volumes/Data/Users`, #1356), which resolving leaves as it is spelled.
///
/// A denial of a folder alone is left out: a rule here holds what is below a path too, and
/// what is below such an entry is the chat's to write (#2752), as a later-code name that holds
/// only itself is left out. One of a file alone is kept, for nothing is below a file: a program
/// held by its own name (a cluster pool's plugin, #3292) is held here as the profile holds it.
fn names_of(denied: &[super::Denial]) -> Vec<(&super::Denial, std::path::PathBuf)> {
    denied
        .iter()
        .filter(|denial| denial.reach == super::Reach::AndBelow || not_a_folder(&denial.path))
        .flat_map(|denial| {
            names_for(&denial.path)
                .into_iter()
                .map(move |name| (denial, name))
        })
        .collect()
}

/// Whether `path`, followed through links, is there and is not a folder.
fn not_a_folder(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| !meta.is_dir())
}

/// The spellings a rule on `path` for `denial` is written under: as it is, and, for one held
/// under every spelling, with each letter in either case too (#2752).
fn spelled(denial: &super::Denial, path: &Path) -> Vec<String> {
    let plain = path.display().to_string();
    if denial.any_case {
        let cased = either_case(&plain);
        vec![plain, cased]
    } else {
        vec![plain]
    }
}

/// **`name`, a glob's tail, with each ASCII letter as a class of both its cases** (`a` as
/// `[aA]`, the letter as written first). A disk that compares names case-folded opens the same
/// file under any spelling, so a held name is written for every spelling of it. Each glob is
/// held as written too ([`plain_and_cased`]), so a rule never rests on the class alone. Claude
/// Code 2.1.300 keeps a `[...]` class when it turns a glob into its profile's regex (it escapes
/// `.^$+{}()|\` and no bracket), and reads a path holding one as a glob.
pub fn either_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() * 4);
    for c in name.chars() {
        if c.is_ascii_alphabetic() {
            // The letter as written first, so the glob still reads as the name.
            let other = if c.is_ascii_lowercase() {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            };
            out.push('[');
            out.push(c);
            out.push(other);
            out.push(']');
        } else {
            out.push(c);
        }
    }
    out
}

/// `prefix` and the glob tail `name` as two globs: as written, and in either case
/// ([`either_case`]).
fn plain_and_cased(prefix: &str, name: &str) -> [String; 2] {
    [
        format!("{prefix}{name}"),
        format!("{prefix}{}", either_case(name)),
    ]
}

/// **`rules` for Claude Code's own tools, each path written once** (#6961, Sentry
/// PURLIS-APP-J): what decides as they did, in fewer words. Settings near a megabyte switch auto
/// mode's check off for the chat, and every action that needs the check is then refused unasked.
///
/// Claude Code matches a Read or Edit rule's path the way gitignore matches a line (the `ignore`
/// library at its defaults, read in its bundled CLI): **a path is held when it or a folder above
/// it matches, and letters match in either case.** So, of the rules for one tool, this drops:
///
/// - a rule written twice;
/// - `T(<p>/**)` beside `T(<p>)`, which holds what is below `<p>` already;
/// - `T(<glob>)` that [`either_case`] made, beside `T(<the name as written>)`, which matches
///   every case already;
/// - `T(<q>)`, a plain path, below `T(<p>)`, a plain path that is a folder above it.
///
/// A rule on another tool, or one that is not a path rule, is kept as it is, in its place. That
/// every spelling the rules held before is held still, and by the pinned Claude Code itself, is
/// `the_smaller_rules_deny_every_spelling_the_old_rules_denied`.
pub(crate) fn fewest(rules: Vec<String>) -> Vec<String> {
    let parts = |rule: &str| -> Option<(String, String)> {
        let (tool, rest) = rule.split_once('(')?;
        let path = rest.strip_suffix(')')?;
        matches!(tool, "Read" | "Edit").then(|| (tool.to_owned(), path.to_owned()))
    };
    let written: std::collections::HashSet<&str> = rules.iter().map(String::as_str).collect();
    let plain: std::collections::HashSet<(String, String)> = rules
        .iter()
        .filter_map(|rule| parts(rule))
        .filter(|(_, path)| literal(path))
        .collect();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for rule in &rules {
        if !seen.insert(rule.as_str()) {
            continue;
        }
        let Some((tool, path)) = parts(rule) else {
            out.push(rule.clone());
            continue;
        };
        // DO NOT MERGE: mutation M2, no rule on the /System/Volumes/Data name.
        if path.starts_with("//System/Volumes/Data") {
            continue;
        }
        if let Some(above) = path.strip_suffix("/**")
            && written.contains(format!("{tool}({above})").as_str())
        {
            continue;
        }
        let as_written = uncased(&path);
        if as_written != path && written.contains(format!("{tool}({as_written})").as_str()) {
            continue;
        }
        if literal(&path)
            && folders_above(&path).any(|above| plain.contains(&(tool.clone(), above.to_owned())))
        {
            continue;
        }
        out.push(rule.clone());
    }
    out
}

/// `paths` for Claude Code's sandbox, each written once, and none below a plain path in the list:
/// Claude Code holds a listed path and all that is below it, and pins each folder above it, so
/// a path below another listed one holds and pins nothing more (#6961). A glob is kept as it is.
fn once(paths: Vec<String>) -> Vec<String> {
    let plain: std::collections::HashSet<String> =
        paths.iter().filter(|path| literal(path)).cloned().collect();
    let mut seen = std::collections::HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .filter(|path| !literal(path) || !folders_above(path).any(|above| plain.contains(above)))
        .collect()
}

/// **Whether the rules `deny` hold `tool` from `path`**, as [`fewest`] leaves them: a rule on
/// the path itself, on `<path>/**`, or on a plain folder above it, in any case. `path` is a
/// rule's path, as a test asks for it (`//a/b`, a glob, `<p>/**`, or an [`either_case`] glob).
/// What a test asks where it once asked for the exact rule.
#[cfg(test)]
pub(crate) fn tool_holds(deny: &[String], tool: &str, path: &str) -> bool {
    let has = |path: &str| deny.iter().any(|rule| *rule == format!("{tool}({path})"));
    if has(path) {
        return true;
    }
    let path = path.strip_suffix("/**").unwrap_or(path);
    if has(path) {
        return true;
    }
    let path = uncased(path).to_lowercase();
    if !literal(&path) {
        return false;
    }
    // A rule's glob as gitignore reads one: `*` within a name, `**` any folders or none, in
    // either case, and what is below what it matches held with it.
    let matches = |glob: &str| {
        let mut pattern = String::from("(?i)^");
        let mut rest = glob;
        while let Some(c) = rest.chars().next() {
            if let Some(after) = rest.strip_prefix("**/") {
                pattern.push_str("(?:.*/)?");
                rest = after;
            } else if let Some(after) = rest.strip_prefix("**") {
                pattern.push_str(".*");
                rest = after;
            } else {
                match c {
                    '*' => pattern.push_str("[^/]*"),
                    '?' => pattern.push_str("[^/]"),
                    _ => pattern.push_str(&regex::escape(&c.to_string())),
                }
                rest = &rest[c.len_utf8()..];
            }
        }
        pattern.push_str("(?:/.*)?$");
        regex::Regex::new(&pattern).is_ok_and(|it| it.is_match(&path))
    };
    deny.iter()
        .filter_map(|rule| rule.strip_prefix(&format!("{tool}(")))
        .filter_map(|rest| rest.strip_suffix(')'))
        .map(|held| uncased(held).to_lowercase())
        .any(|held| {
            if literal(&held) {
                path == held || path.starts_with(&format!("{held}/"))
            } else {
                !held.contains('[') && matches(&held)
            }
        })
}

/// [`tool_holds`] for `rule`, written as a whole rule (`Edit(//a/b)`).
#[cfg(test)]
pub(crate) fn rule_holds(deny: &[String], rule: &str) -> bool {
    match rule
        .split_once('(')
        .and_then(|(tool, rest)| Some((tool, rest.strip_suffix(')')?)))
    {
        Some((tool, path)) => tool_holds(deny, tool, path),
        None => deny.iter().any(|it| it == rule),
    }
}

/// **Whether a sandbox list (`denyRead`, `denyWrite`) holds `path`**, as [`once`] leaves it:
/// listed itself, or below a plain path listed, of the same kind (plain, or [`either_case`]'s).
#[cfg(test)]
pub(crate) fn list_holds<S: AsRef<str>>(list: &[S], path: &str) -> bool {
    let list: Vec<&str> = list.iter().map(AsRef::as_ref).collect();
    if list.contains(&path) {
        return true;
    }
    let cased = uncased(path) != path;
    let path = uncased(path);
    if !literal(&path) {
        return false;
    }
    list.iter()
        .filter(|it| (uncased(it) != **it) == cased)
        .map(|it| uncased(it))
        .filter(|it| literal(it))
        .any(|it| path.starts_with(&format!("{it}/")))
}

/// **Every branch folder's git in the plane `path`'s is in**, as one glob, where `path` is a
/// branch folder's git (`<plane>/workspaces/<ws>/.worktrees/<repo>/<piece>/.git`, the folders
/// [`super::planted::clones_purlis_works_in`] finds): `<plane>/workspaces/*/.worktrees/*/*/.git`.
/// It holds that one and every other branch folder's, one made later among them, and nothing
/// that is not a branch folder's git. `None` for any other path, and for one whose plane's own
/// path a glob would read as more than itself.
fn every_branch_folders_git(path: &str) -> Option<String> {
    let rest = path.strip_suffix("/.git")?;
    let (rest, _piece) = rest.rsplit_once('/')?;
    let (rest, _repo) = rest.rsplit_once('/')?;
    let rest = rest.strip_suffix(&format!("/{}", crate::worktree::DIR_NAME))?;
    let (rest, _workspace) = rest.rsplit_once('/')?;
    let plane = rest.strip_suffix(&format!("/{}", super::WORKSPACES))?;
    let parts = &path[plane.len()..];
    (literal(path) && !plane.is_empty() && parts.split('/').all(|part| part != ".." && part != "."))
        .then(|| {
            format!(
                "{plane}/{}/*/{}/*/*/.git",
                super::WORKSPACES,
                crate::worktree::DIR_NAME
            )
        })
}

/// `now`, which is `had` with more after it, with `had` as it was and each added entry kept only
/// where `tidy`, run over the whole, keeps it: an added entry `tidy` drops is held by one it
/// keeps, which is `had`'s or another added one.
fn added_once(had: &[String], now: &[String], tidy: fn(Vec<String>) -> Vec<String>) -> Vec<String> {
    debug_assert!(now.starts_with(had), "added after what was there");
    let kept: std::collections::HashSet<String> = tidy(now.to_vec()).into_iter().collect();
    let mut out = had.to_vec();
    for added in &now[had.len().min(now.len())..] {
        if kept.contains(added) && !out.contains(added) {
            out.push(added.clone());
        }
    }
    out
}

/// The strings of a JSON list, or `None` where it is not one.
fn strings_of(list: &Value) -> Option<Vec<String>> {
    list.as_array().map(|paths| {
        paths
            .iter()
            .filter_map(|path| path.as_str().map(str::to_owned))
            .collect()
    })
}

/// Whether `path` is a plain path, with nothing a glob reads as more than itself.
fn literal(path: &str) -> bool {
    !path.contains(['*', '?', '[', '\\'])
}

/// Each folder above `path`, as `path` spells it: `/a/b/c` gives `/a` and `/a/b`; `//a/b`, a
/// rule's absolute path, gives `//a`.
fn folders_above(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/')
        .map(|(at, _)| &path[..at])
        .filter(|above| !above.trim_start_matches('/').is_empty())
}

/// `glob` with each class [`either_case`] made written back as the letter it was made from:
/// what a test compares a held name with, and how [`fewest`] knows a rule's plain twin.
pub(crate) fn uncased(glob: &str) -> String {
    let chars: Vec<char> = glob.chars().collect();
    let mut out = String::with_capacity(glob.len());
    let mut at = 0;
    while at < chars.len() {
        if chars[at] == '['
            && at + 3 < chars.len()
            && chars[at + 3] == ']'
            && chars[at + 1].is_ascii_alphabetic()
            && chars[at + 1] != chars[at + 2]
            && chars[at + 2].eq_ignore_ascii_case(&chars[at + 1])
        {
            out.push(chars[at + 1]);
            at += 4;
        } else {
            out.push(chars[at]);
            at += 1;
        }
    }
    out
}

/// The names [`names_of`] writes a rule on `path` under: as written, as the kernel names it,
/// the link itself where its last part is one, and as spelled through each link on the way to
/// it. Claude Code pins every folder above a denied path as it is spelled, so the spellings
/// through each link are what keep each link above it, and the folder that link is in, where
/// they are: replaced by a link pointing elsewhere, the path would no longer lead to what is
/// denied.
fn names_for(path: &Path) -> Vec<std::path::PathBuf> {
    let mut names = vec![path.to_path_buf()];
    for name in super::seatbelt::held_names(path)
        .into_iter()
        .chain(super::seatbelt::spelled_through_links(path))
    {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let twins: Vec<_> = names
        .iter()
        .filter_map(|it| super::firmlink_twin(it))
        .collect();
    for twin in twins {
        if !names.contains(&twin) {
            names.push(twin);
        }
    }
    names
}

/// **Where the person's own and the administrators' Claude Code settings let a command write**
/// (#6961, the orchestrator's ruling A'): Claude Code adds to its sandbox's write roots each
/// `sandbox.filesystem.allowWrite` entry, each `permissions.additionalDirectories` folder and
/// each Edit allow rule's path from these tiers ("merged with paths from Edit(...) allow
/// permission rules", read in its bundled CLI). A git folder pin under one is kept. Read from the
/// managed files ([`crate::footerclaim::managed`]) and every user settings file the chat may run
/// on: `~/.claude`'s, `$CLAUDE_CONFIG_DIR`'s and each profile purlis made's.
fn persons_write_roots(compiled: &Compiled) -> Vec<String> {
    #[cfg(test)]
    if let Some(files) = TEST_PERSONS_FILES.with(|it| it.borrow().clone()) {
        return write_roots_in(&files, compiled.homes.home.as_deref());
    }
    let mut files = crate::footerclaim::managed();
    let mut configs: Vec<PathBuf> = Vec::new();
    if let Some(home) = &compiled.homes.home {
        configs.push(home.join(".claude"));
    }
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        configs.push(PathBuf::from(dir));
    }
    if let Some(profiles) = &compiled.homes.profiles {
        for profile in std::fs::read_dir(profiles).into_iter().flatten().flatten() {
            configs.push(profile.path());
            configs.extend(
                std::fs::read_dir(profile.path())
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|entry| entry.path()),
            );
        }
    }
    files.extend(configs.into_iter().map(|dir| dir.join("settings.json")));
    write_roots_in(&files, compiled.homes.home.as_deref())
}

#[cfg(test)]
thread_local! {
    /// The settings files [`persons_write_roots`] reads on this test's thread, in place of this
    /// machine's, where a test sets them: what it measures is then not the machine's.
    pub(crate) static TEST_PERSONS_FILES: std::cell::RefCell<Option<Vec<PathBuf>>> =
        const { std::cell::RefCell::new(None) };
}

/// [`persons_write_roots`] in `files`, each a plain folder: `/` for what cannot be placed exactly
/// (a bare Edit allow, a relative path or pattern, a file there that cannot be read), which keeps
/// every pin. A file that is not there says nothing.
pub(crate) fn write_roots_in(files: &[PathBuf], home: Option<&Path>) -> Vec<String> {
    const EVERYWHERE: &str = "/";
    let placed = |path: &str| -> String {
        let path = match path.strip_prefix("//") {
            Some(rest) => format!("/{rest}"),
            None => path.to_owned(),
        };
        let expanded = match (path.strip_prefix("~/"), home) {
            (Some(rest), Some(home)) => home.join(rest).display().to_string(),
            (Some(_), None) => return EVERYWHERE.to_owned(),
            (None, _) => path,
        };
        if !expanded.starts_with('/') {
            return EVERYWHERE.to_owned();
        }
        let plain = expanded
            .split(['*', '?', '[', '{'])
            .next()
            .unwrap_or_default()
            .trim_end_matches('/');
        if plain.is_empty() {
            EVERYWHERE.to_owned()
        } else {
            plain.to_owned()
        }
    };
    let mut out: Vec<String> = Vec::new();
    let mut add = |root: String| {
        if !out.contains(&root) {
            out.push(root);
        }
    };
    for file in files {
        let doc: Value = match std::fs::metadata(file) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Ok(meta) if meta.is_file() && meta.len() <= MANAGED_AT_MOST => {
                match std::fs::read_to_string(file)
                    .ok()
                    .and_then(|text| serde_json::from_str(&text).ok())
                {
                    Some(doc) => doc,
                    None => {
                        add(EVERYWHERE.to_owned());
                        continue;
                    }
                }
            }
            _ => {
                add(EVERYWHERE.to_owned());
                continue;
            }
        };
        let strings = |pointer: &str| -> Vec<String> {
            doc.pointer(pointer)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|it| it.as_str().map(str::to_owned))
                .collect()
        };
        for path in strings("/sandbox/filesystem/allowWrite")
            .into_iter()
            .chain(strings("/permissions/additionalDirectories"))
        {
            add(placed(&path));
        }
        for rule in strings("/permissions/allow") {
            let rule = rule.trim();
            let (tool, content) = match rule.split_once('(') {
                Some((tool, rest)) => (tool, rest.strip_suffix(')')),
                None => (rule, None),
            };
            if !matches!(tool, "Edit" | "Write" | "MultiEdit" | "NotebookEdit") {
                continue;
            }
            match content {
                // A path from the settings file's own folder, or one to match anywhere: not one
                // folder, so everywhere.
                Some(path) if path.starts_with("//") || path.starts_with("~/") => add(placed(path)),
                _ => add(EVERYWHERE.to_owned()),
            }
        }
    }
    out
}

/// **Whether an administrator's Claude Code settings turn local binding on** (#1699): with
/// `sandbox.network.allowLocalBinding` on, a Claude Code chat's commands may connect to every
/// loopback port, every local service among them, and purlis's `false` does not outrank an
/// administrator's settings. Said in doctor, never changed: the setting is theirs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalBinding {
    /// No managed file turns it on, or one turns it off.
    Off,
    /// This managed file turns it on.
    On(PathBuf),
    /// This managed file is there and could not be read, so purlis cannot tell.
    Unread { file: PathBuf, why: String },
}

/// [`LocalBinding`] in this machine's managed Claude Code settings: the policy file and its
/// drop-ins, read as the status line's claim reads them ([`crate::footerclaim`]).
pub fn administrators_local_binding() -> LocalBinding {
    local_binding_in(&crate::footerclaim::managed())
}

/// How big a managed settings file may be before purlis stops reading it.
const MANAGED_AT_MOST: u64 = 1_048_576;

/// [`LocalBinding`] in `files`, in the order Claude Code combines them. A `false` in force wins,
/// as in Claude Code's merge; a file that is not there says nothing.
pub fn local_binding_in(files: &[PathBuf]) -> LocalBinding {
    let mut on = None;
    for file in files {
        match managed_local_binding(file) {
            Ok(None) => {}
            Ok(Some(false)) => return LocalBinding::Off,
            Ok(Some(true)) => {
                on.get_or_insert_with(|| file.clone());
            }
            Err(why) => {
                return LocalBinding::Unread {
                    file: file.clone(),
                    why,
                };
            }
        }
    }
    on.map_or(LocalBinding::Off, LocalBinding::On)
}

/// What `file` sets `allowLocalBinding` to, if anything, or why it could not be read.
fn managed_local_binding(file: &Path) -> Result<Option<bool>, String> {
    let found = match std::fs::metadata(file) {
        Ok(found) => found,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !found.is_file() {
        return Err("it is not a file".to_owned());
    }
    if found.len() > MANAGED_AT_MOST {
        return Err(format!(
            "it is {} bytes, which purlis will not read",
            found.len()
        ));
    }
    let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let doc: Value =
        serde_json::from_str(&text).map_err(|e| format!("it is not JSON purlis can read: {e}"))?;
    Ok(doc
        .pointer("/sandbox/network/allowLocalBinding")
        .and_then(Value::as_bool))
}
