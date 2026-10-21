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
//! What it does not hold: a command moved a directory holding a `config` into a nested
//! clone's `.git` (measured), and no glob can deny the `.git` itself without denying what git
//! writes below it (#1065).
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

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Access, Compiled, Level, Uncompilable};
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
}

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
        for kept in ["vaults", crate::secrets::fingerprint::KEY_FILE] {
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
    // The later-code class (ruling V73b), by name at any depth: added to the names Claude
    // Code's own sandbox already keeps commands from writing, never in their place. A name
    // that holds only itself (`.git`) is left out: a glob here denies what is below it too,
    // and git writes there.
    for planted in super::PLANTED
        .iter()
        .filter(|planted| planted.reach == super::Reach::AndBelow)
    {
        let glob = format!("**/{}", planted.path);
        deny_write.extend(plain_and_cased("**/", planted.path));
        edit_rules.push(format!("Edit({glob})"));
        edit_rules.push(format!("Edit({glob}/**)"));
    }
    // Under each granted folder (#1342), every later-code name again, by absolute path: a
    // relative `**/<name>` is read against the chat's folder, so it is not known to hold in a
    // folder outside it (D-1342-3). The folder is the kernel's spelling already (D-1342-12).
    for folder in &compiled.writable {
        for planted in super::PLANTED
            .iter()
            .filter(|planted| planted.reach == super::Reach::AndBelow)
        {
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
        // Every later-code name wherever it is, the home's among them: a relative `**/<name>`
        // is read against the chat's folder, and at the floor every folder is written.
        for planted in super::PLANTED
            .iter()
            .filter(|planted| planted.reach == super::Reach::AndBelow)
        {
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
    let mut filesystem = json!({
        "denyRead": deny_read,
        "denyWrite": deny_write,
    });
    if !allow_write.is_empty() {
        filesystem["allowWrite"] = json!(allow_write);
    }
    if compiled.level == Level::Floor {
        return Ok(floor(filesystem, read_rules, edit_rules, compiled));
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
    Ok(Settings { sandbox, deny })
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
    }
}

impl Settings {
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
        out
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
        out
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
        out
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
/// A denial of an entry alone is left out: a rule here holds what is below a path too, and
/// what is below such an entry is the chat's to write (#2752), as a later-code name that holds
/// only itself is left out.
fn names_of(denied: &[super::Denial]) -> Vec<(&super::Denial, std::path::PathBuf)> {
    denied
        .iter()
        .filter(|denial| denial.reach == super::Reach::AndBelow)
        .flat_map(|denial| {
            names_for(&denial.path)
                .into_iter()
                .map(move |name| (denial, name))
        })
        .collect()
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

/// `glob` with each class [`either_case`] made written back as the letter it was made from:
/// what a test compares a held name with.
#[cfg(test)]
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

/// The names [`names_of`] writes a rule on `path` under.
fn names_for(path: &Path) -> Vec<std::path::PathBuf> {
    let mut names = vec![path.to_path_buf()];
    for name in super::seatbelt::held_names(path) {
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
