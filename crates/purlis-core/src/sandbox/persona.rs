// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A persona's own sandbox grants** (#1362; ADR 0067 §1 as amended): the hosts a persona's
//! chats reach, beside the project's and yours ([`super::hosts`]).
//!
//! **Where they are written.** In the project's committed `charter.toml`, under the persona's
//! name:
//!
//! ```toml
//! [sandbox.personas.devops]
//! hosts = ["10.100.39.145:6443", "*.internal.example"]
//! ```
//!
//! Not in the persona's own `persona.md`, and that is the point (D-1362-1): a chat may edit its
//! own persona's charter (spec #1330, amended 2026-10-06), so a grant written there would be a
//! grant a chat could write itself. `charter.toml` at the project root is a later-code name no
//! chat writes ([`super::PLANTED`]), a brokered write refuses any change under `[sandbox]`
//! ([`super::changes_a_sandbox_key`]). Each entry goes through [`Host::parse`], so it is refused
//! for everything a project's host is: a private address is one exact address with an optional
//! exact port, and no wildcard or range ever names addresses. A table for a persona the project
//! does not define is refused where the project's personas are known, as a Settings save knows
//! them (#1407): it would grant nothing.
//!
//! **Allowed on each machine, bound to what was shown** (D-1362-7). A persona's hosts are
//! committed, so a teammate can change them; and they are the hosts that reach past the
//! presets into private networks. So, unlike the project's own hosts, which apply and are told
//! once, a persona's reach nothing on a machine until the person there allows them on the
//! arrival Notice ([`shown`]). The Allow is kept with the [`digest`] of the list the Notice
//! showed ([`super::local::allow_persona_hosts`]), and checked against the list as it is when
//! it is pressed ([`shown_now`]). A list that changes in any way after it was allowed grants
//! nothing until it is allowed again ([`in_force_here`]): never a part of it, never the old
//! one.
//!
//! **A chat running as that persona gets them** ([`super::Compiled::of`], at the
//! [`super::hosts::Level::Persona`] level); a chat on another persona does not, and a chat that
//! names none gets the default persona's, the one its briefing adopts
//! ([`crate::start::grants_persona`]). **Fixed at the start**: a chat's sandbox is compiled as it
//! starts, so a persona switched mid-chat changes its grants only at its next start.
//!
//! **A chat never widens what it reaches on its own say.** A **Resume** of a session record
//! holds the default persona's grants when the record's persona reaches past them (D-1362-6,
//! [`held_unless_within`]), since a record's `persona:` is something a chat can write, until
//! the person allows its own on its tab. A chat the person starts from the window holds its
//! own. A handoff used to be held the same way on the asking chat's grants (D-1362-5); since a
//! handoff became a dispatch (#1444) it is not, and a chat still held from before stays held,
//! and is refused a dispatch of either kind, until the person allows it.
//!
//! **A dispatch is not held** (#1437, spec #1434). A persona chat started by a dispatch that a
//! dispatch grant covers holds its own persona's grants from its first command
//! ([`crate::dispatchgrant::grants_for_a_dispatched_chat`]): the grant is the person's consent
//! to the pair, given before anything started, so nothing is left to allow on the new chat's
//! tab. Without a grant the dispatch does not start at all. **A handoff is a dispatch**
//! (#1444), decided and started the same way, so this holds for it too.
//!
//! **Policy** (#1343) can forbid persona grants: [`super::hosts::Locks`] is asked of every
//! persona host, and of every persona tool ([`super::policy::Locks::refuses_tool`]).
//!
//! **Its tools** (`tools = { kubectl = { vault, as, key } }`, #1407) are read here beside its
//! hosts and run as [`super::tools`] says: a shim on the chat's `PATH` that calls the brokered
//! `purlis secret exec`. A brokered `secret exec` from a sandboxed chat runs only a tool its
//! persona declares (D-PE-ruling-2), so a tool is what lets a vault's value reach a program on
//! a chat's say: **they wait for the same per-machine Allow as its hosts** (D-PE-ruling-1, the
//! operator's ruling of 2026-10-12). One Allow is of the persona's hosts and tools together, as
//! the Notice shows them ([`digest_of`]), kept in the same local file and listed on the same
//! Granted row; until it is pressed, and after any change to either list, a chat as the persona
//! gets no tool ([`tools_in_force_here`]) and its `secret exec` runs nothing.

use std::collections::BTreeMap;
use std::path::Path;

use super::hosts::{self, Host};
use super::policy::Locks;
use crate::settings::Refusal;

/// The key in `[sandbox]` that holds each persona's grants.
pub const KEY: &str = "personas";

/// The key in a persona's table that declares its browser (#2241).
pub const BROWSER: &str = "browser";

/// The keys one persona's table holds.
const KEYS: [&str; 3] = [hosts::KEY, super::tools::KEY, BROWSER];

/// What one persona is granted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grants {
    /// The hosts its chats reach, beside the project's and yours.
    pub hosts: Vec<Host>,
    /// The programs its chats run through a vault (#1407).
    pub tools: Vec<super::tools::Tool>,
    /// **The browser it declares** (#2241): the answer its chats' browser Ask marks.
    /// **It grants nothing** (ADR 0073): the person still answers the Ask.
    pub browser: Option<Browser>,
}

/// **What a persona declares its chats' browser usually reaches** (#2241, spec #2008 story 3):
/// `browser = "project"` or `"open-web"` in its sandbox section. Only marks an answer of the
/// browser Ask, in its place; never an answer itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    /// This project's sites and its dev servers.
    Project,
    /// This project's sites, its dev servers and the open web.
    OpenWeb,
}

impl Browser {
    /// The word the file writes it with.
    pub fn word(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::OpenWeb => "open-web",
        }
    }

    /// The declaration `word` writes, if it is one.
    pub fn of_word(word: &str) -> Option<Self> {
        [Self::Project, Self::OpenWeb]
            .into_iter()
            .find(|one| one.word() == word)
    }

    /// Whether it reaches the open web.
    pub fn open_web(self) -> bool {
        self == Self::OpenWeb
    }
}

/// **The browser `persona` declares** in the project read as `plane` (#2241): from the
/// project's root `charter.toml`, which no sandboxed chat writes, whether or not the project sandboxes its chats, since a chat's browser is
/// asked about either way. `None` where it declares none, or what it wrote was refused.
pub fn declared_browser(plane: &super::Plane, persona: &str) -> Option<Browser> {
    let said = plane.said();
    said.policy.or(said.unset)?.personas.get(persona)?.browser
}

/// What `[sandbox] personas` holds: each persona's grants, and each thing in it that is not
/// taken, as one sentence.
///
/// `known` is the personas the project defines, where the reader knows them: a table for any
/// other is refused and grants nothing (#1407). A persona made later is read at the next read.
/// `None` checks no name against the project, as where only the file's text is at hand.
pub fn read(
    value: Option<&toml::Value>,
    file: &str,
    known: Option<&[String]>,
) -> (BTreeMap<String, Grants>, Vec<String>) {
    let (grants, refused) = keyed(value, file, known);
    (grants, refused.into_iter().map(|one| one.why).collect())
}

/// [`read`], each refusal with the key it is about (#1292).
pub fn keyed(
    value: Option<&toml::Value>,
    file: &str,
    known: Option<&[String]>,
) -> (BTreeMap<String, Grants>, Vec<Refusal>) {
    let mut out = BTreeMap::new();
    let mut refused = Vec::new();
    let Some(value) = value else {
        return (out, refused);
    };
    let table_key = [super::TABLE, KEY];
    let at = format!("{}.{KEY}", super::TABLE);
    let Some(table) = value.as_table() else {
        refused.push(Refusal::at(
            format!(
                "{at} in {file} is not a table, so no persona is granted anything — write \
                 [{at}.<persona>] with hosts"
            ),
            &table_key,
        ));
        return (out, refused);
    };
    for (persona, grants) in table {
        let here = format!("{at}.{persona}");
        let here_key = [super::TABLE, KEY, persona.as_str()];
        if !crate::personas::valid_name(persona) {
            refused.push(Refusal::at(
                format!(
                    "{here} in {file} is not a persona's name, so it grants nothing — a \
                     persona's name is lowercase letters, digits and hyphens"
                ),
                &here_key,
            ));
            continue;
        }
        if known.is_some_and(|known| !known.iter().any(|name| name == persona)) {
            refused.push(Refusal::at(
                format!(
                    "{here} in {file} names no persona of this project, so it grants nothing — \
                     make the persona first, or take its table out"
                ),
                &here_key,
            ));
            continue;
        }
        let Some(grants) = grants.as_table() else {
            refused.push(Refusal::at(
                format!(
                    "{here} in {file} is not a table, so it grants nothing — it holds hosts, tools \
                     and browser"
                ),
                &here_key,
            ));
            continue;
        };
        for key in grants.keys() {
            if !KEYS.contains(&key.as_str()) {
                refused.push(Refusal::at(
                    format!(
                        "{here}.{key} in {file} is not a key purlis reads — a persona's sandbox \
                         holds hosts, tools and browser"
                    ),
                    &[super::TABLE, KEY, persona.as_str(), key.as_str()],
                ));
            }
        }
        let hosts_key = [super::TABLE, KEY, persona.as_str(), hosts::KEY];
        let hosts = match hosts::read(grants.get(hosts::KEY)) {
            Ok(listed) => {
                refused.extend(listed.refused.into_iter().map(|(written, why)| {
                    Refusal::at(
                        format!(
                            "{here}.hosts in {file} names {written}, which no chat is let reach: \
                             {why}"
                        ),
                        &hosts_key,
                    )
                }));
                listed.hosts
            }
            Err(hosts::NotAList) => {
                refused.push(Refusal::at(
                    format!(
                        "{here}.hosts in {file} is not a list of hosts, so none of {persona}'s is \
                         allowed — write hosts = [\"api.example.com\", \"10.0.0.5:6443\"]"
                    ),
                    &hosts_key,
                ));
                Vec::new()
            }
        };
        let browser_key = [super::TABLE, KEY, persona.as_str(), BROWSER];
        let browser = match grants.get(BROWSER) {
            None => None,
            Some(value) => {
                let declared = value.as_str().and_then(Browser::of_word);
                if declared.is_none() {
                    refused.push(Refusal::at(
                        format!(
                            "{here}.{BROWSER} in {file} is not \"project\" or \"open-web\", so \
                             {persona} declares no browser — write browser = \"project\" for this \
                             project's sites and its dev servers, or \"open-web\" for the open \
                             web too"
                        ),
                        &browser_key,
                    ));
                }
                declared
            }
        };
        let (tools, tools_refused) = super::tools::keyed(
            grants.get(super::tools::KEY),
            &here,
            file,
            &[super::TABLE, KEY, persona.as_str()],
        );
        refused.extend(tools_refused);
        out.insert(
            persona.clone(),
            Grants {
                hosts,
                tools,
                browser,
            },
        );
    }
    (out, refused)
}

/// **The grants a chat running as `persona` holds**: its hosts. Nothing for a chat on no
/// persona, or one the project grants nothing.
pub fn of<'a>(personas: &'a BTreeMap<String, Grants>, persona: Option<&str>) -> Option<&'a Grants> {
    personas.get(persona?)
}

/// **The policy a hold is judged by** (#1407): the persona grants the committed file writes,
/// whether or not the project sandboxes its chats now. A hold is kept with the chat, and a chat
/// started while the sandbox is off is started again under it once it is on: judged only while
/// on, it would come back holding nothing. `None` where the file has no `[sandbox]`, which
/// grants no persona anything.
pub fn for_holds(plane: &super::Plane, locks: &Locks) -> Option<super::Policy> {
    let said = plane.said();
    let written = said.unset.clone();
    said.in_force(locks).or(written)
}

/// **What a chat started as `target` by someone the person did not pick holds** (#1362,
/// D-1362-5 and D-1362-6): `None`, its own persona's grants, where `target`'s hosts are all among
/// those of `trusted` (it widens nothing), or where `policy` is `None`, a project that grants no
/// persona anything; otherwise the grants of `trusted`, until the person allows its own on its
/// tab. `policy` is [`for_holds`]'s, so a project that does not sandbox its chats today still
/// holds (#1407).
///
/// `trusted` is, for a **Resume** of a session record, whose `persona:` a chat could have
/// written, the default persona's. A chat the person starts from the window holds its own and
/// asks nothing. A handoff is decided as a dispatch is (#1444), and holds what
/// `dispatchgrant` gives a dispatched chat.
pub fn held_unless_within(
    policy: Option<&super::Policy>,
    trusted: Option<&str>,
    target: Option<&str>,
) -> Option<crate::reopen::HeldGrants> {
    let personas = &policy?.personas;
    let wanted = of(personas, target);
    let had = of(personas, trusted);
    let hosts = |grants: Option<&Grants>| {
        grants
            .map(|grants| grants.hosts.clone())
            .unwrap_or_default()
    };
    let tools = |grants: Option<&Grants>| {
        grants
            .map(|grants| grants.tools.clone())
            .unwrap_or_default()
    };
    // A tool widens as a host does (D-PE-ruling-1): one `trusted` does not declare as it is
    // holds the chat too.
    let (had_hosts, had_tools) = (hosts(had), tools(had));
    let within = hosts(wanted).iter().all(|host| had_hosts.contains(host))
        && tools(wanted).iter().all(|tool| had_tools.contains(tool));
    (!within).then(|| crate::reopen::HeldGrants {
        persona: trusted.map(str::to_owned),
    })
}

// ---- allowed on this machine, bound to what was shown (#1362, D-1362-7) ----------------------

/// **The digest an Allow of a persona's hosts is bound to**, for a persona with no tools:
/// [`digest_of`] with none.
pub fn digest(hosts: &[Host], default: bool) -> String {
    digest_of(hosts, &[], default)
}

/// **The digest an Allow of a persona's hosts and tools is bound to**: SHA-256 over whether the
/// persona is the project's default (whose hosts and tools then reach every chat that names no
/// persona, D-1362-12), the hosts as the sandbox spells them, and each tool whole (its name,
/// vault, variable, how it is handed and key, D-PE-ruling-1), each sorted and once each. Order
/// is no change; any other change is, and so is the persona becoming, or ceasing to be, the
/// default. A persona with no tools has the digest it had before tools were read, so an Allow
/// kept then still stands.
pub fn digest_of(hosts: &[Host], tools: &[super::tools::Tool], default: bool) -> String {
    use sha2::Digest;
    let mut spelled: Vec<String> = hosts.iter().map(ToString::to_string).collect();
    spelled.sort();
    spelled.dedup();
    let reach = if default {
        "default: every chat that names no persona too"
    } else {
        "named: chats as this persona only"
    };
    let mut text = format!("purlis persona hosts 2\n{reach}\n{}", spelled.join("\n"));
    if !tools.is_empty() {
        let mut tools: Vec<String> = tools.iter().map(super::tools::Tool::whole).collect();
        tools.sort();
        tools.dedup();
        text.push_str("\ntools\n");
        text.push_str(&tools.join("\n"));
    }
    crate::extension::hex(&sha2::Sha256::digest(text.as_bytes()))
}

/// Whether `persona` is the project's default at `root` (`[persona] default` in the committed
/// file), the one a chat that names no persona takes ([`crate::start::grants_persona`]).
pub fn is_default(root: &Path, persona: &str) -> bool {
    crate::workspaces::Plane::open(root.to_path_buf())
        .default_persona()
        .is_some_and(|default| default == persona)
}

/// `grants`' hosts as a chat would reach them under `locks`: none a policy locks out (#1343),
/// so the Notice never asks for a host no chat would reach.
fn reached(grants: &Grants, locks: &Locks) -> Vec<Host> {
    grants
        .hosts
        .iter()
        .filter(|host| {
            locks
                .refuses(&hosts::Granted {
                    host: (*host).clone(),
                    level: hosts::Level::Persona,
                })
                .is_none()
        })
        .cloned()
        .collect()
}

/// `persona`'s tools in `grants` as a chat would get them under `locks` in the project at
/// `root`, so the Notice never asks for a tool no chat would get: none a policy forbids
/// (`persona-tools`), and none refused for the program it runs here
/// ([`super::tools::refused_here`]); the sentence for each one so refused.
fn reached_tools(
    root: &Path,
    persona: &str,
    grants: &Grants,
    locks: &Locks,
) -> (Vec<super::tools::Tool>, Vec<String>) {
    let mut refused = Vec::new();
    let tools = grants
        .tools
        .iter()
        .filter(|tool| locks.refuses_tool(persona, &tool.name).is_none())
        .filter(|tool| match super::tools::refused_here(root, tool) {
            Some(why) => {
                refused.push(format!(
                    "{persona}'s tool {} runs nothing: {why}",
                    tool.name
                ));
                false
            }
            None => true,
        })
        .cloned()
        .collect();
    (tools, refused)
}

/// Where the person's Allow of one persona's hosts stands on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// No Allow is kept for it here.
    NotAllowed,
    /// The Allow is of the list as it is now: in force.
    Allowed,
    /// An Allow was kept, and the list (or whether it is the default's) has changed since. It
    /// grants nothing, ever again, and the Notice asks anew (D-1362-13).
    Waiting,
}

/// **Judges the Allow kept for `persona`** against `now`, the digest of its list as it stands.
/// Where `judged`, one that does not match is marked as changed, once and for good, so a list
/// that later comes back to the allowed one never returns to force with nobody asked
/// (D-1362-13). A persona none of whose hosts a chat would reach (a policy locks them all out)
/// is not judged: the policy, not the list, changed. A record that cannot be read allows
/// nothing. The same Allow is of the persona's tools ([`digest_of`]).
fn standing(root: &Path, persona: &str, now: &str, judged: bool) -> Standing {
    let Some(kept) = super::local::allowed_persona_hosts(root)
        .into_iter()
        .find(|one| one.persona == persona)
    else {
        return Standing::NotAllowed;
    };
    if kept.changed {
        return Standing::Waiting;
    }
    if kept.digest == now {
        return Standing::Allowed;
    }
    if !judged {
        return Standing::NotAllowed;
    }
    if let Err(why) = super::local::persona_hosts_changed(root, persona) {
        // Not marked: it still grants nothing now, and is judged again at the next read.
        tracing::warn!("purlis: a changed persona hosts Allow was not marked ({why})");
    }
    Standing::Waiting
}

/// **The hosts a chat running as `persona` holds on this machine**, in the project at `root`:
/// its persona's hosts under `locks`, all of them where the person here allowed exactly this
/// list, these tools and this default-ness ([`digest_of`]), and none otherwise. Nothing for a
/// chat on no persona, or on one the project grants nothing.
pub fn in_force_here(
    root: &Path,
    personas: &BTreeMap<String, Grants>,
    persona: Option<&str>,
    locks: &Locks,
) -> Vec<Host> {
    allowed_here(root, personas, persona, locks)
        .map(|(hosts, _)| hosts)
        .unwrap_or_default()
}

/// **The tools a chat running as `persona` gets on this machine** (D-PE-ruling-1): its
/// persona's tools under `locks`, all of them where the person here allowed exactly them with
/// its hosts ([`digest_of`]), and none otherwise. A tool not allowed here has no shim, and a
/// brokered `secret exec` of it runs nothing (D-PE-ruling-2).
pub fn tools_in_force_here(
    root: &Path,
    personas: &BTreeMap<String, Grants>,
    persona: Option<&str>,
    locks: &Locks,
) -> Vec<super::tools::Tool> {
    allowed_here(root, personas, persona, locks)
        .map(|(_, tools)| tools)
        .or_else(|| allowed_by_its_desktop(root, personas, persona, locks))
        .unwrap_or_default()
}

/// **`persona`'s tools on a runner, where its desktop's person allowed exactly this list** (#4071):
/// nobody on a runner can answer an Allow, so a start from its desktop says what was allowed
/// there ([`super::local::desktop_allows_tools`]), held here to the persona as this runner reads
/// it. Its tools alone: its hosts stay [`in_force_here`]'s, which this never widens. A tool armed
/// so is still sent nothing unless the desktop's gate passes and the person allows that run.
fn allowed_by_its_desktop(
    root: &Path,
    personas: &BTreeMap<String, Grants>,
    persona: Option<&str>,
    locks: &Locks,
) -> Option<Vec<super::tools::Tool>> {
    let (Some(name), Some(grants)) = (persona, of(personas, persona)) else {
        return None;
    };
    let (tools, _) = reached_tools(root, name, grants, locks);
    let now = digest_of(&reached(grants, locks), &tools, is_default(root, name));
    (!tools.is_empty()
        && super::local::tools_allowed_by_desktop(root)
            .iter()
            .any(|one| one.persona == name && one.digest == now))
    .then_some(tools)
}

/// **What the person on this machine allowed of `persona`, as its digest** (#4071): what a start
/// of it on a runner carries. None where they allowed nothing, or a list since changed.
pub fn digest_allowed_here(
    root: &Path,
    personas: &BTreeMap<String, Grants>,
    persona: &str,
    locks: &Locks,
) -> Option<String> {
    let (hosts, tools) = allowed_here(root, personas, Some(persona), locks)?;
    Some(digest_of(&hosts, &tools, is_default(root, persona)))
}

/// `persona`'s hosts and tools under `locks`, where the person here allowed exactly them.
fn allowed_here(
    root: &Path,
    personas: &BTreeMap<String, Grants>,
    persona: Option<&str>,
    locks: &Locks,
) -> Option<(Vec<Host>, Vec<super::tools::Tool>)> {
    let (Some(name), Some(grants)) = (persona, of(personas, persona)) else {
        return None;
    };
    let hosts = reached(grants, locks);
    let (tools, _) = reached_tools(root, name, grants, locks);
    let now = digest_of(&hosts, &tools, is_default(root, name));
    let asked = !hosts.is_empty() || !tools.is_empty();
    (asked && standing(root, name, &now, true) == Standing::Allowed).then_some((hosts, tools))
}

/// One persona's hosts as the arrival Notice and Settings show them on this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub persona: String,
    /// Its hosts as the project's file lists them.
    pub listed: Vec<Host>,
    /// Those a chat would reach, the ones an Allow allows: none a policy locks out. Empty
    /// where a policy forbids persona hosts.
    pub hosts: Vec<Host>,
    /// Its tools a chat would get, which the same Allow allows (D-PE-ruling-1): none a policy
    /// forbids. Where this and [`Self::hosts`] are both empty, nothing is asked.
    pub tools: Vec<super::tools::Tool>,
    /// Whether it is the project's default persona, so its hosts reach every chat that names
    /// no persona too: part of what an Allow is of.
    pub default: bool,
    /// What an Allow of exactly this is bound to ([`digest_of`]).
    pub digest: String,
    /// Where the Allow kept here stands.
    pub standing: Standing,
    /// Each of its tools left out of [`Self::tools`] because the program it runs here is one no
    /// tool may be, as one sentence.
    pub refused: Vec<String>,
}

impl Shown {
    /// Whether the person allowed exactly this here, so a chat reaches it.
    pub fn allowed(&self) -> bool {
        self.standing == Standing::Allowed && self.asks()
    }

    /// Whether it holds anything an Allow would allow: a host a chat would reach, or a tool.
    pub fn asks(&self) -> bool {
        !self.hosts.is_empty() || !self.tools.is_empty()
    }
}

/// **Each persona's hosts and tools in the project at `root`**, as this machine would grant
/// them, and where the person's Allow stands: the arrival Notice asks for each one not allowed
/// that has a host a chat would reach or a tool (none a policy forbids, #1343). None where the
/// project's chats are not sandboxed, and none for a persona that lists no host and no tool.
pub fn shown(root: &Path) -> Vec<Shown> {
    shown_in(root, &super::Plane::read(root), &Locks::of(root))
}

/// [`shown`], for the project at `root` read as `plane`, under `locks`.
pub fn shown_in(root: &Path, plane: &super::Plane, locks: &Locks) -> Vec<Shown> {
    let Some(policy) = plane.in_force(locks) else {
        return Vec::new();
    };
    policy
        .personas
        .iter()
        .filter(|(_, grants)| !grants.hosts.is_empty() || !grants.tools.is_empty())
        .map(|(persona, grants)| {
            let hosts = reached(grants, locks);
            let (tools, refused) = reached_tools(root, persona, grants, locks);
            let default = is_default(root, persona);
            let digest = digest_of(&hosts, &tools, default);
            let asked = !hosts.is_empty() || !tools.is_empty();
            Shown {
                persona: persona.clone(),
                listed: grants.hosts.clone(),
                standing: standing(root, persona, &digest, asked),
                default,
                digest,
                hosts,
                tools,
                refused,
            }
        })
        .collect()
}

/// **What an Allow of `persona`'s hosts and tools allows**, where `digest` is the digest of them
/// as they stand now: what the window showed is what is kept. Refused, with a sentence, where
/// the persona has no hosts and no tools here, or they changed after they were shown.
pub fn shown_now(root: &Path, persona: &str, digest: &str) -> Result<Shown, String> {
    pick(shown(root), persona, digest)
}

/// [`shown_now`] among `shown`, each persona's hosts as they stand now.
pub fn pick(shown: Vec<Shown>, persona: &str, digest: &str) -> Result<Shown, String> {
    let now = shown
        .into_iter()
        .find(|one| one.persona == persona && one.asks())
        .ok_or_else(|| {
            format!(
                "purlis allowed nothing: chats as {persona} have no hosts or tools of their own \
                 in this project now."
            )
        })?;
    if now.digest != digest {
        return Err(format!(
            "purlis allowed nothing: {persona}'s hosts or tools changed after they were shown. \
             Look at them again."
        ));
    }
    Ok(now)
}

/// Tests' stand-in for a person who allowed every persona's hosts in `policy`, as listed, at
/// `root`.
#[cfg(test)]
pub(crate) fn allow_every_as_listed(root: &Path, policy: &super::Policy) {
    for (persona, grants) in &policy.personas {
        super::local::allow_persona_hosts(
            root,
            persona,
            &digest_of(&grants.hosts, &grants.tools, is_default(root, persona)),
        )
        .expect("kept");
    }
}
