// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The Organisation's folder** (M75 slice 06, #7014; spec §4.5, §6 and §13.6).
//!
//! The Organisation level is an admin-owned folder per OS, delivered by MDM, Intune or Ansible
//! and never fetched (Q310.6): `/etc/purlis`, and on Windows `%ProgramData%\purlis` found with
//! the known-folder API (`FOLDERID_ProgramData`), never from the environment variable, which a
//! chat could set ([`windows_folder`]). It holds `policy.cedar` (rules), `policy.toml` (the full
//! project syntax) and today's `policy.json`, which is translated in memory and adds to the two
//! files' rules and locks ([`translate`]).
//!
//! **Its owner and ACL are verified at load, on every OS** (review round 1, S-B7). A folder or
//! file that fails the check reads as `strict` plus every lock, with a [`Notice`] that names the
//! check and offers the admin a way out: fix the owner and ACL, or remove the folder, so a folder
//! a chat created can't force a permanent lockdown (review round 2, S2 nit). Unix is
//! [`sandbox::policy::judged_as`](crate::sandbox::policy::judged_as), which reads the macOS ACL;
//! Windows is [`judge_windows`], a pure judgement over a described owner and ACL, so its tests
//! gate on every OS. **Anything that cannot be read or judged fails the same way.**
//!
//! In a test build the Organisation level comes from the existing
//! [`crate::sandbox::policy::set_for_this_test`] seam alone, empty by default: never from an
//! environment variable and never from `/etc/purlis` (spec §14.1).
//!
//! [`Notice`]: super::levels::Notice

use std::path::{Path, PathBuf};

use crate::dispatchlimits::Limit;
use crate::sandbox::Preset;
use crate::sandbox::grant;
use crate::sandbox::hosts::Host;
use crate::sandbox::policy::{Check, Locks, Refused};

use super::levels::{self, Organisation};

/// The Organisation's folder on Unix.
pub const UNIX_FOLDER: &str = "/etc/purlis";
/// The Organisation's rules.
pub const RULES_FILE: &str = "policy.cedar";
/// The Organisation's settings, in the project file's syntax.
pub const SETTINGS_FILE: &str = "policy.toml";
/// Today's policy file, translated.
pub const JSON_FILE: &str = "policy.json";

/// The Windows folder, from the answer of the known-folder API, never the environment: what
/// `lookup` returns for `FOLDERID_ProgramData`, then `purlis`. `None` where the API does not
/// answer, which is a failed check, since whether a folder is there cannot be told.
pub fn windows_folder(lookup: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    lookup().map(|program_data| program_data.join("purlis"))
}

/// **The Organisation's folder on this machine**, or `None` where it cannot be found.
#[cfg(unix)]
pub fn folder() -> Option<PathBuf> {
    Some(PathBuf::from(UNIX_FOLDER))
}

/// **The Organisation's folder on this machine** (`FOLDERID_ProgramData`).
#[cfg(windows)]
pub fn folder() -> Option<PathBuf> {
    windows_folder(|| dirs_sys::known_folder(windows_sys::Win32::UI::Shell::FOLDERID_ProgramData))
}

/// What the Organisation's folder gave, as owned text: borrowed as [`Organisation`] by
/// [`Found::as_sources`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The folder, as a Notice names its files.
    pub folder: String,
    /// Who it is: `policy.json`'s `owner`, else "the Organisation".
    pub owner: String,
    /// `policy.cedar`'s text, if the folder holds one.
    pub rules: Option<String>,
    /// `policy.toml`'s text, if the folder holds one.
    pub settings: Option<String>,
    /// `policy.json` translated, or why it is broken.
    pub json: Option<Result<Translation, String>>,
    /// Why the folder reads as strict plus every lock, where a check failed.
    pub failed: Option<String>,
}

impl Found {
    /// The Organisation's files as the loader reads them.
    pub fn as_sources(&self) -> Organisation<'_> {
        Organisation {
            folder: &self.folder,
            owner: &self.owner,
            rules: self.rules.as_deref(),
            settings: self.settings.as_deref(),
            json: self.json.as_ref().map(|json| match json {
                Ok(t) => Ok(levels::Translated {
                    settings: &t.settings,
                    rules: &t.rules,
                }),
                Err(why) => Err(why.as_str()),
            }),
            failed: self.failed.as_deref(),
        }
    }

    /// A folder whose check `refused` failed: strict plus every lock, said with the way out.
    fn failed(folder: &Path, refused: &Refused) -> Self {
        Self {
            folder: folder.display().to_string(),
            owner: "the Organisation".to_owned(),
            rules: None,
            settings: None,
            json: None,
            failed: Some(failed_notice(folder, refused)),
        }
    }
}

/// **The sentence a failed Organisation folder's Notice says**: the check by name, what was
/// found, what it reads as, and the admin's way out (spec §4.5, review round 2, S2 nit).
pub fn failed_notice(folder: &Path, refused: &Refused) -> String {
    let way_out = if cfg!(windows) {
        "An administrator can fix it so that only Administrators or SYSTEM own it and write it, \
         or remove the folder, and then restart purlis"
    } else {
        "An administrator can fix it (own it as root, take write away from everyone else, and \
         remove any access-control list that lets someone write it), or remove the folder, and \
         then restart purlis"
    };
    // purlis runs as the person, never as root: a file only root can read is a file it cannot.
    let readable = if refused.check == Check::Read && cfg!(unix) {
        " purlis does not run as root, so a file that only root can read cannot be read either: \
         make each file readable by everyone (mode 0644), owned by root."
    } else {
        ""
    };
    format!(
        "The Organisation's folder {} fails the check \"{}\" ({}), so it reads as strict with \
         every lock, under the full sandbox. {way_out}.{readable}",
        folder.display(),
        refused.check.name(),
        refused.why
    )
}

/// **The Organisation's files on this machine**, as a process that decides reads them; `None`
/// where it has none. In a test build it is the [`TEST_LOCKS`](crate::sandbox::policy) seam's
/// locks translated, and the folder is never touched.
#[cfg(not(any(test, feature = "test-support")))]
pub fn in_force() -> Option<Found> {
    match folder() {
        Some(folder) => read_folder(&folder, 0),
        None => Some(Found::failed(
            Path::new("the Organisation's folder"),
            &Refused {
                check: Check::Read,
                why: "the system does not say where it is".to_owned(),
            },
        )),
    }
}

/// **The Organisation's files in a test build**: the seam's locks translated; empty by default.
#[cfg(any(test, feature = "test-support"))]
pub fn in_force() -> Option<Found> {
    from_locks(&Locks::of_this_machine())
}

/// The Organisation a [`Locks`] stands for, as a test build's seam holds it: none where it has no
/// policy, strict plus every lock where it is refused, else its translation.
pub fn from_locks(locks: &Locks) -> Option<Found> {
    if !locks.any() {
        return None;
    }
    let folder = Path::new(UNIX_FOLDER);
    if let Some(why) = locks.refused_because() {
        return Some(Found::failed(
            folder,
            &Refused {
                check: Check::Read,
                why: why.to_owned(),
            },
        ));
    }
    let t = translate_locks(locks);
    Some(Found {
        folder: UNIX_FOLDER.to_owned(),
        owner: t.owner.clone(),
        rules: None,
        settings: None,
        json: Some(Ok(t)),
        failed: None,
    })
}

/// **Reads the Organisation's folder at `folder`**, the system's administrator being user id
/// `admin` (0, except in a test that is its own administrator): `None` where there is no folder
/// and no file, its files where the folder passes every check, and a failed Found where it does
/// not, or where a file cannot be read.
#[cfg(unix)]
pub fn read_folder(folder: &Path, admin: u32) -> Option<Found> {
    use crate::sandbox::policy::read_trusted_as;
    let mut texts = Vec::new();
    for name in [RULES_FILE, SETTINGS_FILE, JSON_FILE] {
        match read_trusted_as(admin, &folder.join(name)) {
            Ok(text) => texts.push(text),
            Err(refused) => return Some(Found::failed(folder, &refused)),
        }
    }
    let json = texts.pop().expect("policy.json's answer");
    let settings = texts.pop().expect("policy.toml's answer");
    let rules = texts.pop().expect("policy.cedar's answer");
    // The folder is judged whenever it is there, file or no file (D-1343-11): a folder nobody
    // could judge was caught above, so no text at all is no folder, or an empty one.
    if rules.is_none() && settings.is_none() && json.is_none() && !folder.exists() {
        return None;
    }
    let json = json.map(|text| translate(&text, &folder.join(JSON_FILE)));
    let owner = match &json {
        Some(Ok(t)) => t.owner.clone(),
        _ => "the Organisation".to_owned(),
    };
    Some(Found {
        folder: folder.display().to_string(),
        owner,
        rules,
        settings,
        json,
        failed: None,
    })
}

/// **Reads the Organisation's folder at `folder`** where no owner and ACL reader exists yet: a
/// folder that is there cannot be judged, so it fails closed (see [`judge_windows`] for the
/// judgement it will be handed).
#[cfg(not(unix))]
pub fn read_folder(folder: &Path, _admin: u32) -> Option<Found> {
    match std::fs::symlink_metadata(folder) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => Some(Found::failed(
            folder,
            &Refused {
                check: Check::Read,
                why: format!("it cannot be read: {err}"),
            },
        )),
        Ok(_) => Some(Found::failed(
            folder,
            &Refused {
                check: Check::Read,
                why: "this build cannot read a Windows folder's owner and access-control list"
                    .to_owned(),
            },
        )),
    }
}

// ── Windows: a pure judgement over a described owner and ACL ─────────────────────────────

/// A Windows security principal, as far as the judgement needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Principal {
    /// The Administrators group (`S-1-5-32-544`).
    Administrators,
    /// The local system account (`S-1-5-18`).
    System,
    /// Anyone else, by name or SID: a standard user, `Users`, `Everyone`, `Authenticated Users`.
    Other(String),
}

impl Principal {
    /// Whether this principal may own the folder and write it.
    fn is_admin(&self) -> bool {
        matches!(self, Principal::Administrators | Principal::System)
    }
}

/// One access-control entry of a Windows folder or file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ace {
    /// Who it is for.
    pub who: Principal,
    /// Whether it allows (`true`) or denies.
    pub allow: bool,
    /// Whether it grants any way to change the object: write data, append, delete, delete child,
    /// write attributes or extended attributes, change permissions, take ownership, or generic
    /// write or all.
    pub writes: bool,
}

/// What a reader says of one Windows file or folder, without following a link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Described {
    /// Whether it is a link or a reparse point.
    pub link: bool,
    /// Whether it is a regular file (for a file) or a directory (for its folder).
    pub expected_type: bool,
    /// Its owner.
    pub owner: Principal,
    /// Its access-control list: `None` for a **null DACL**, which grants everyone full access
    /// (not an empty list, which grants no one anything). It fails the judgement.
    pub aces: Option<Vec<Ace>>,
}

/// **Whether a Windows Organisation folder, and each file in it, is one purlis trusts** (spec
/// §4.5): not a link, the right kind of thing, owned by the Administrators group or SYSTEM, and
/// written by no one else. A standard user can create `%ProgramData%\purlis`, so the owner is
/// judged first; an entry that allows write to anyone but Administrators and SYSTEM fails the
/// ACL check, whoever owns it. A denying entry takes nothing away from another entry's allow,
/// so it never excuses one.
pub fn judge_windows(folder: &Described, file: Option<&Described>) -> Result<(), Refused> {
    let refuse = |check, why: &str| {
        Err(Refused {
            check,
            why: why.to_owned(),
        })
    };
    for (described, of_folder) in std::iter::once((folder, true)).chain(file.map(|f| (f, false))) {
        let (it, its) = if of_folder {
            ("its folder", "its folder's")
        } else {
            ("it", "its")
        };
        if described.link {
            return refuse(Check::Link, &format!("{it} is a link"));
        }
        if !described.expected_type {
            return refuse(
                Check::Kind,
                &if of_folder {
                    "its folder is not a folder".to_owned()
                } else {
                    "it is not a regular file".to_owned()
                },
            );
        }
        if !described.owner.is_admin() {
            return refuse(
                Check::Owner,
                &format!("{its} owner is not the Administrators group or SYSTEM"),
            );
        }
        let Some(aces) = &described.aces else {
            return refuse(
                Check::Acl,
                &format!("{its} access-control list is null, which lets everyone write it"),
            );
        };
        if aces
            .iter()
            .any(|ace| ace.allow && ace.writes && !ace.who.is_admin())
        {
            return refuse(
                Check::Acl,
                &format!(
                    "{its} access-control list lets others than Administrators and SYSTEM write it"
                ),
            );
        }
    }
    Ok(())
}

// ── policy.json, translated (spec §13.6) ─────────────────────────────────────────────────

/// `policy.json` as the loader reads it: the Organisation's locks and rules in the two files'
/// syntax, named by [`levels::Translated`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    /// `owner`: who the Organisation is.
    pub owner: String,
    /// `[policy.locks]`, and the `[policy.limits]` of `dispatch`, in `policy.toml`'s syntax.
    pub settings: String,
    /// Its permits and forbids on `connect` and `dispatch-to`, in `policy.cedar`'s syntax.
    pub rules: String,
}

/// **Translates the policy file's `text`** (read from `file`) **as spec §13.6 says**, or says why
/// it is broken: it is not the policy file purlis reads, or it says something this version does
/// not know, which makes the file broken (D-1343-3), never read in part.
pub fn translate(text: &str, file: &Path) -> Result<Translation, String> {
    let locks = Locks::parse(text, file);
    match locks.refused_because() {
        Some(why) => Err(format!("{} is refused: {why}", file.display())),
        None => Ok(translate_locks(&locks)),
    }
}

/// The widest grant scope the policy's `allow-scopes` names, as `[policy.locks] keep` spells it.
fn keep_of(scopes: &[grant::Level]) -> &'static str {
    let rank = |scope: &grant::Level| match scope {
        grant::Level::Once => 0,
        grant::Level::Chat => 1,
        grant::Level::Project => 2,
        grant::Level::You | grant::Level::Machine => 3,
    };
    match scopes.iter().map(rank).max() {
        None | Some(0) => "once",
        Some(1) => "this chat",
        Some(2) => "this workspace",
        Some(_) => "everywhere",
    }
}

/// **The Cedar entity a listed `host`'s name is, and its port**, as `Host::covers` reads it: a
/// name is the `Host` of that id, `*.domain` is the `HostSet` every name under it is put in
/// ([`crate::policy::wildcard_set`]), and a port is the request's `context.port`: a request's
/// resource is the bare host, never the host's own spelling.
fn host_entity(host: &Host) -> (String, Option<u16>) {
    let spelt = host.to_string();
    let bare = match host.port() {
        Some(port) => spelt.strip_suffix(&format!(":{port}")).unwrap_or(&spelt),
        None => &spelt,
    };
    let entity = match bare.strip_prefix("*.") {
        Some(suffix) => format!("HostSet::{}", quoted(&super::wildcard_set(suffix))),
        None => format!("Host::{}", quoted(bare)),
    };
    (entity, host.port())
}

/// **The condition that is `host`**: the request is that name, at that port where it has one.
fn host_matches(host: &Host) -> String {
    any_host_matches(std::slice::from_ref(host))
}

/// **The condition that is any of `hosts`**, `false` where there is none. The hosts of one port
/// share one flat set, so a long list (a preset's) never nests: Cedar's evaluator refuses a
/// condition nested deeper than it can follow, and that would deny every request.
fn any_host_matches(hosts: &[Host]) -> String {
    // One set holds one entity type (the schema's validator refuses a mixed one): names, then
    // wildcards' sets, each by port.
    let mut by_port: std::collections::BTreeMap<(Option<u16>, bool), Vec<String>> =
        std::collections::BTreeMap::new();
    for host in hosts {
        let (entity, port) = host_entity(host);
        by_port
            .entry((port, entity.starts_with("HostSet::")))
            .or_default()
            .push(entity);
    }
    let groups: Vec<String> = by_port
        .into_iter()
        .map(|((port, _), entities)| {
            let set = format!("resource in [{}]", entities.join(", "));
            match port {
                Some(port) => format!("({set} && context.port == {port})"),
                None => format!("({set})"),
            }
        })
        .collect();
    if groups.is_empty() {
        "false".to_owned()
    } else {
        groups.join(" || ")
    }
}

/// A word as a Cedar or TOML string's contents.
fn quoted(word: &str) -> String {
    format!("\"{}\"", word.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A list of words as a TOML array.
fn array(words: &[&str]) -> String {
    format!(
        "[{}]",
        words
            .iter()
            .map(|w| quoted(w))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// **The translation of an already-valid policy** (spec §13.6; the mapping is decided, round 2 of
/// #7170, and recorded in `docs/plane-format.md`):
///
/// - `owner` becomes the Organisation's owner;
/// - `hosts` (and the hosts of `presets`, where both are set) become Organisation permits on
///   `connect`, and where `hosts` is set, a forbid on every other host. `presets` alone restricts
///   no host, but `presets`, `hosts`, `never-hosts`, `personal-hosts` and `persona-hosts` each
///   lock the `network` family, so no level adds one. A listed host reads as `Host::covers`
///   reads it ([`host_matches`]): a wildcard is a suffix set, a port is `context.port`;
/// - `never-hosts` become forbids on `connect`, and `dispatch` pairs forbids on `dispatch-to`;
/// - a `false` of `dispatch.allow`, `opt-out`, `persona-tools`, `vault-grants` or `live-asks` is a
///   hard forbid with no `@ask`, and its family lock;
/// - `write-grants`, `socket-grants`, `proposed-changes`, `undeclared-secret-asks` and
///   `run-grants` lock the action each one forbids (`write`, `dial`, `propose`, `request`,
///   `run`), and `enforcement` its families;
/// - `allow-scopes` becomes the `keep` lock, at the widest scope it names;
/// - dispatch limits become `[policy.limits]`, which slice 12 reads;
/// - every Organisation forbid, these included, is enforced whatever the project's mode or
///   template: `levels` defaults its mode to `enforce`;
/// - `relayed-internal` is not a permission on a policy action: it stays where it is read today.
pub fn translate_locks(locks: &Locks) -> Translation {
    let mut families: Vec<&'static str> = Vec::new();
    let mut actions: Vec<&'static str> = Vec::new();
    let mut rules = String::from("// purlis-policy 1\n");
    fn lock_family(families: &mut Vec<&'static str>, family: &'static str) {
        if !families.contains(&family) {
            families.push(family);
        }
    }

    // ── hosts ────────────────────────────────────────────────────────────────────────────
    let mut hosts: Vec<Host> = Vec::new();
    let mut fixed = false;
    // The legacy file fixes the hosts only where it sets `hosts` (spec §13.6): `presets` alone
    // says which presets a project may turn on, which is no host restriction, so it is none
    // here (round 3). Where both are set, a preset's hosts are among those that are allowed.
    if let Some(allowed) = locks.allowed_hosts() {
        fixed = true;
        if locks.fixes_presets() {
            for preset in locks.presets(&Preset::ALL) {
                // A preset's host is spelt as a policy spells one, so it is read as one.
                hosts.extend(
                    preset
                        .own_hosts()
                        .iter()
                        .filter_map(|h| Host::parse(h).ok()),
                );
            }
        }
        hosts.extend(allowed.iter().cloned());
    }
    hosts.sort_by_key(ToString::to_string);
    hosts.dedup();
    let listed: Vec<String> = hosts.iter().map(host_matches).collect();
    if fixed {
        lock_family(&mut families, "network");
        for (n, matches) in listed.iter().enumerate() {
            rules.push_str(&format!(
                "@id(\"network-organisation-host-{n}\") @level(\"organisation\")\n\
                 permit (principal, action == Action::\"connect\", resource)\n  \
                 when {{ {matches} }};\n"
            ));
        }
        let set = any_host_matches(&hosts);
        rules.push_str(&format!(
            "@id(\"network-organisation-fixed-hosts\") @level(\"organisation\")\n\
             forbid (principal, action == Action::\"connect\", resource)\n  \
             unless {{ {set} }};\n"
        ));
    }
    for (n, host) in locks.never_hosts().iter().enumerate() {
        rules.push_str(&format!(
            "@id(\"network-organisation-never-{n}\") @level(\"organisation\")\n\
             forbid (principal, action == Action::\"connect\", resource)\n  \
             when {{ {} }};\n",
            host_matches(host)
        ));
    }
    if locks.fixes_presets()
        || !locks.never_hosts().is_empty()
        || locks.forbids_personal_hosts()
        || locks.forbids_persona_hosts()
    {
        lock_family(&mut families, "network");
    }
    if !locks.locked_pairs().is_empty() {
        lock_family(&mut families, "dispatch");
    }

    // ── a `false` is a no: a hard forbid, never an ask (round 2, steward ruling) ─────────
    let unknown_host = if hosts.is_empty() {
        String::new()
    } else {
        format!(" || {}", any_host_matches(&hosts))
    };
    let hard_forbids: [(bool, &[&'static str], &str, String); 5] = [
        (
            locks.forbids_dispatch(),
            &["dispatch"],
            "dispatch-organisation-forbidden",
            "forbid (principal, action == Action::\"dispatch-to\", resource);".to_owned(),
        ),
        (
            locks.forbids_opt_out(),
            &["sandbox", "trust"],
            "trust-organisation-no-opt-out",
            "forbid (principal, action == Action::\"start\", resource)\n  \
             when { context.sandbox == \"off\" };"
                .to_owned(),
        ),
        (
            locks.forbids_persona_tools(),
            &["tools", "vaults"],
            "tools-organisation-no-persona-tools",
            "forbid (principal, action in [Action::\"run\", Action::\"mount\"], resource);"
                .to_owned(),
        ),
        (
            locks.forbids_vault_grants(),
            &["vaults"],
            "vaults-organisation-no-vault-grants",
            "forbid (principal, action == Action::\"use\", resource)\n  \
             unless { resource in principal.persona };"
                .to_owned(),
        ),
        (
            locks.forbids_live_asks(),
            &["asks", "network"],
            "network-organisation-no-live-asks",
            format!(
                "forbid (principal, action == Action::\"connect\", resource)\n  \
                 unless {{ resource in HostSet::\"known-hosts\"{unknown_host} }};"
            ),
        ),
    ];
    for (forbidden, locked, id, rule) in hard_forbids {
        if forbidden {
            for family in locked {
                lock_family(&mut families, family);
            }
            rules.push_str(&format!("@id(\"{id}\") @level(\"organisation\")\n{rule}\n"));
        }
    }

    // ── the other locks ──────────────────────────────────────────────────────────────────
    let actions_forbidden = [
        (locks.forbids_write_grants(), "write"),
        (locks.forbids_socket_grants(), "dial"),
        (locks.proposed_changes_refused().is_some(), "propose"),
        (locks.forbids_undeclared_secret_asks(), "request"),
        (locks.forbids_run_grants(), "run"),
    ];
    for (forbidden, action) in actions_forbidden {
        if forbidden {
            actions.push(action);
        }
    }
    // Every other lock keeps its own family enforced too, as the legacy file's
    // `enforcement_floor` does (round 5): a lock a project's mode could loosen is no lock.
    for (locked, family) in [
        (!locks.dispatch_ceiling().is_unset(), "dispatch"),
        (locks.forbids_undeclared_secret_asks(), "vaults"),
        (locks.forbids_run_grants(), "vaults"),
        (locks.forbids_write_grants(), "sandbox"),
        (locks.forbids_socket_grants(), "ports"),
    ] {
        if locked {
            lock_family(&mut families, family);
        }
    }
    for (n, (from, to)) in locks.locked_pairs().iter().enumerate() {
        rules.push_str(&format!(
            "@id(\"dispatch-organisation-pair-{n}\") @level(\"organisation\")\n\
             forbid (principal in Persona::{}, action == Action::\"dispatch-to\", \
             resource == Persona::{});\n",
            quoted(from),
            quoted(to)
        ));
    }
    let enforced: Vec<&'static str> = locks.enforced_families().map(|f| f.key()).collect();
    let mut settings = String::new();
    let mut table = String::new();
    let mut all_families = families.clone();
    all_families.extend(enforced.iter().copied());
    all_families.sort_unstable();
    all_families.dedup();
    if !all_families.is_empty() {
        table.push_str(&format!("families = {}\n", array(&all_families)));
    }
    if !actions.is_empty() {
        table.push_str(&format!("actions = {}\n", array(&actions)));
    }
    if let Some(scopes) = locks.scope_list() {
        table.push_str(&format!("keep = {}\n", quoted(keep_of(scopes))));
    }
    if !table.is_empty() {
        settings.push_str("[policy.locks]\n");
        settings.push_str(&table);
    }
    let limits: Vec<String> = Limit::ALL
        .into_iter()
        .filter_map(|limit| {
            locks
                .dispatch_ceiling()
                .get(limit)
                .map(|value| format!("\"dispatch.{}\" = {value}\n", limit.word()))
        })
        .collect();
    if !limits.is_empty() {
        settings.push_str("[policy.limits]\n");
        settings.extend(limits);
    }
    Translation {
        owner: locks.owner(),
        settings,
        rules,
    }
}

#[cfg(test)]
mod tests;
