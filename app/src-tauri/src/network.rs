// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The network, as the window shows it** (#1662, spec #1661): Settings' Network page (Open
//! hosts, Allowed, the Blocked log) and each chat's Network view. The Blocked log is read from
//! this machine's event log (T1-32, `purlis_core::eventlog::blockedlog`), and a chat's view from
//! its network record (`purlis_core::sandboxblock::record`), and what writes that record from the
//! app: every block a chat's sandbox met, and every Allow and removal a person made.
//!
//! **Nothing here changes what a chat may reach.** Allow on the Blocked log is the same judged,
//! audited Allow a block's Notice makes (`sandboxing::judged`), kept for this project on this
//! machine or for everyone in it; a chat takes it from its next start, as every Allow made
//! outside its own Notice does.
//!
//! **What the record holds is data.** A chat's name is what the person called it, and a host a
//! block names came on a line the chat's hook sent: both are shown as text and never followed.

use std::path::Path;

use purlis_core::eventlog::blockedlog::{self, Listed};
use purlis_core::eventlog::stops::{Source, Target};
use purlis_core::sandbox::{self, browser_reach::Site, grant, hosts::Host};
use purlis_core::sandboxblock::record::{self, Entry, Record};
use purlis_core::sandboxblock::{Kind, Operation};

use crate::planes::{PlaneId, Planes};
use crate::sandboxing::{Allowed, Audit, GrantLevel, now_secs};

/// The most rows the Blocked log and a chat's refusals list: the newest.
const AT_MOST_LISTED: usize = 50;

/// The chat chat `session` is, as the record names it, and its persona.
fn chat_of(chats: &crate::chats::Chats, session: u32) -> (record::Chat, Option<String>) {
    let recorded = chats.recorded_chat(session);
    let chat = record::Chat {
        id: recorded.as_ref().and_then(|one| one.identity.id.clone()),
        name: chats.shown_name(session),
        session: Some(session),
    };
    (chat, recorded.and_then(|one| one.persona))
}

/// **Records a block the app took** for chat `heard.line.chat` in the project at `root`, heard
/// at `at`: only the host a refused connection was to, of everything the line named. A brokered
/// run's vault is said on its Notice and never kept here.
pub(crate) fn record_block(
    record: &Record,
    root: &Path,
    chats: &crate::chats::Chats,
    heard: &crate::hooks::HeardBlock,
    at: u64,
) {
    let blocked = &heard.line;
    let (chat, persona) = chat_of(chats, blocked.chat);
    let entry = Entry::blocked(
        &blocked.sandbox_blocked,
        blocked.target.as_deref(),
        chat,
        persona.as_deref(),
        at,
    );
    if let Err(why) = record.write(root, &entry) {
        tracing::warn!(
            "purlis: a sandbox block of chat {} was not kept in the network record ({why})",
            blocked.chat
        );
    }
}

/// **Records the repeats the throttle held back** of a block of chat `repeated.chat`, in the
/// project at `root`, on a line of their own (#1681), the last of them `ago` before `now`.
pub(crate) fn record_repeated(
    record: &Record,
    root: &Path,
    chats: &crate::chats::Chats,
    repeated: &purlis_core::sandboxblock::Repeated,
    now: u64,
    ago: std::time::Duration,
) {
    let (chat, persona) = chat_of(chats, repeated.chat);
    let entry = Entry::repeated(
        &repeated.block,
        repeated.target.as_deref(),
        chat,
        persona.as_deref(),
        now.saturating_sub(ago.as_secs()),
        repeated.times,
    );
    if let Err(why) = record.write(root, &entry) {
        tracing::warn!(
            "purlis: repeats of a sandbox block of chat {} were not kept in the network record \
             ({why})",
            repeated.chat
        );
    }
}

/// **Records a person's Keep blocked** of `target` for chat `session` in the project at `root`
/// (#1700): their no to the ask, which grants nothing and so has no grant to list it. Only a
/// host a grant could name is kept; a record that cannot be written is said in the log and
/// changes nothing, since the chat was kept blocked either way.
pub(crate) fn record_kept_blocked(
    record: Option<&Record>,
    root: &Path,
    chats: &crate::chats::Chats,
    session: u32,
    target: &str,
) {
    let Some(record) = record else { return };
    if purlis_core::sandboxblock::named_host(target).is_none() {
        return;
    }
    let (chat, persona) = chat_of(chats, session);
    let entry = Entry::kept_blocked(target, chat, persona.as_deref(), now_secs());
    if let Err(why) = record.write(root, &entry) {
        tracing::warn!(
            "purlis: a Keep blocked of chat {session} was not kept in the network record ({why})"
        );
    }
}

/// Connections purlis's own proxy carried for one chat, as its tally told them (#1664).
pub(crate) struct Connections<'a> {
    /// The chat's number: the one whose proxy it came in on.
    pub session: u32,
    pub who: &'a crate::chats::ReachedAs,
    /// The host and port, where the tally told one apart.
    pub target: Option<&'a str>,
    /// The layer that let them through.
    pub by: &'static str,
    pub times: u64,
}

/// **Records connections purlis's own proxy carried** for a chat in the project at `root`, told
/// at `at`. The chat is who its proxy started for; its name, while it is still open.
pub(crate) fn record_connections(
    record: &Record,
    root: &Path,
    chats: &crate::chats::Chats,
    connections: &Connections<'_>,
    at: u64,
) {
    let chat = record::Chat {
        id: connections.who.id.clone(),
        name: chats.shown_name(connections.session),
        session: Some(connections.session),
    };
    let entry = Entry::connected(
        connections.target,
        connections.by,
        connections.times,
        chat,
        connections.who.persona.as_deref(),
        at,
    );
    if let Err(why) = record.write(root, &entry) {
        tracing::warn!(
            "purlis: connections of chat {} were not kept in the network record ({why})",
            connections.session
        );
    }
}

/// **`audit`, then the network record**: every Allow and removal a person makes is audited
/// first (an audit that cannot be written changes nothing), then written to `record` with the
/// chat it came from. A record that cannot be written is said in the log and changes nothing:
/// the Allow stands, and is audited. The tests' seam, with an audit of their own; the app
/// audits in the project's event log ([`crate::planes::Held::audit_then_record`]).
#[cfg(test)]
pub(crate) fn recorded<'a>(
    audit: Audit<'a>,
    record: Option<&'a Record>,
    root: &'a Path,
    chats: &'a crate::chats::Chats,
) -> impl Fn(Option<u32>, &grant::Audited<'_>) -> Result<(), String> + 'a {
    move |number, audited| {
        audit(number, audited)?;
        keep_audited(record, root, chats, number, audited);
        Ok(())
    }
}

impl crate::planes::Held {
    /// **Audit, then record** (#1681), for this project: each Allow and removal is audited in
    /// its event log first, then written to `record` with the chat it came from; a record that
    /// cannot be written changes nothing. What every command that grants or takes back answers
    /// its audit with.
    pub(crate) fn audit_then_record<'a>(
        &'a self,
        record: Option<&'a Record>,
    ) -> impl Fn(Option<u32>, &grant::Audited<'_>) -> Result<(), String> + 'a {
        move |number, audited| {
            self.hooks().record_grant(self.root(), number, audited)?;
            keep_audited(record, self.root(), self.chats(), number, audited);
            Ok(())
        }
    }
}

/// Writes an Allow or removal, audited already, to `record` with the chat it came from.
fn keep_audited(
    record: Option<&Record>,
    root: &Path,
    chats: &crate::chats::Chats,
    number: Option<u32>,
    audited: &grant::Audited<'_>,
) {
    let Some(record) = record else { return };
    let Some(what) = record::What::of_word(audited.what) else {
        tracing::warn!(
            "purlis: an Allow of a kind the network record has no word for was not kept in it"
        );
        return;
    };
    let at = now_secs();
    let scope = record::Scope::from(audited.level);
    let entry = if audited.granted {
        let (chat, persona) = number.map(|n| chat_of(chats, n)).unwrap_or_default();
        Entry::allowed(what, audited.target, scope, chat, persona.as_deref(), at)
    } else {
        Entry::removed(what, audited.target, scope, at)
    };
    if let Err(why) = record.write(root, &entry) {
        tracing::warn!("purlis: an Allow was not kept in the network record ({why})");
    }
}

/// **Records a host added, confirmed or removed in Settings** (#1341's Your hosts and the
/// project's hosts), once `written` says it was: an Allow or a removal at this project on this
/// machine (`local`) or everyone in it (`shared`), decided by the person at this machine.
pub(crate) fn record_settings_host(
    record: Option<&Record>,
    root: &Path,
    written: &crate::settings::EntryWritten,
    allowed: bool,
    which: crate::settings::SettingsWhich,
) {
    let Some(record) = record else { return };
    let crate::settings::EntryWritten::Saved { added, removed, .. } = written else {
        return;
    };
    let host = if allowed {
        added.clone()
    } else {
        removed
            .as_ref()
            .and_then(|values| values.iter().find(|one| one.field == "host"))
            .map(|one| one.value.clone())
    };
    let Some(host) = host else { return };
    let scope = match which {
        crate::settings::SettingsWhich::Shared => record::Scope::Project,
        crate::settings::SettingsWhich::Local => record::Scope::You,
    };
    let at = now_secs();
    let what = record::What::Host;
    let entry = if allowed {
        Entry::allowed(what, &host, scope, record::Chat::default(), None, at)
    } else {
        Entry::removed(what, &host, scope, at)
    };
    if let Err(why) = record.write(root, &entry) {
        tracing::warn!("purlis: a host change was not kept in the network record ({why})");
    }
}

/// One group of **Open hosts**: a preset's, or the project's own.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct OpenHosts {
    /// The preset's name in the window ("AI providers"), or "This project's hosts".
    pub title: String,
    /// Each host, spelled out.
    pub hosts: Vec<String>,
}

/// One row of the **Blocked log**, or of a chat's refusals: what was refused, the newest time,
/// and how often.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct BlockedLately {
    /// The host and port, where the block named one: a refused connection's, the one Allow
    /// may name.
    pub target: Option<String>,
    /// For a refused lookup, the host the program said it looked up (#1663): shown as text,
    /// **never offered to allow**, since a program's own printed words named it.
    pub looked_up: Option<String>,
    /// What was blocked, as the block's Notice says it.
    pub said: String,
    /// The chat it was, by the name it was shown under, where the record has it.
    pub chat: Option<String>,
    /// Its chat's number, where that chat is open here: what its Chat log opens by (R-226).
    pub session: Option<u32>,
    /// When it was last refused, in seconds since 1970.
    pub at: u32,
    /// How many times it was refused in the record.
    pub times: u32,
    /// Whether a chat reaches it now: it is an Open host, or allowed since.
    pub reached: bool,
    /// The scopes Allow may keep it at here: none where there is nothing to allow (no host
    /// named, reached already, not a host a proxy refuses) or policy forbids it.
    pub levels: Vec<GrantLevel>,
}

/// **Settings' Network page** (#1662): Open hosts and the Blocked log. Allowed is the list
/// `sandbox_grants` answers.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct SandboxNetwork {
    /// Whether chats here run sandboxed. Where they do not, nothing is open or blocked.
    pub on: bool,
    pub open: Vec<OpenHosts>,
    pub blocked: Vec<BlockedLately>,
}

/// Why a chat reaches a host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum ReachedBy {
    /// An Open host: a preset's, or the project's own.
    Open,
    /// One of its persona's hosts, allowed on this machine.
    Persona,
    /// An Allowed host: for this chat, this project on this machine, or everyone in it.
    Allowed,
}

/// One host a chat reaches, and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct Reached {
    pub host: String,
    pub by: ReachedBy,
}

/// **A chat's Network view** (#1662): what it can reach now and what it was refused.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ChatNetwork {
    /// Whether the chat is open in this app. A closed one has nothing to say.
    pub open: bool,
    /// Whether this run of the chat is sandboxed. One that is not can reach anything, and
    /// purlis records nothing of it.
    pub sandboxed: bool,
    /// What its sandbox lets it reach, as it was started.
    pub reach: Vec<Reached>,
    /// **The sites only its browser reaches** (#2364): its browser sites, those allowed for this
    /// chat alone first, then those kept for every chat of this project on this machine that
    /// policy leaves in force, each once as `host:port`. Each was allowed for the browser
    /// alone, so the view lists them apart from `reach`.
    pub browser_sites: Vec<String>,
    /// What it was refused, newest first.
    pub refused: Vec<BlockedLately>,
    /// **Where an administrator's managed Claude Code settings let it reach every local port**
    /// (#1699), for a Claude Code chat in the sandbox: they turn local binding on, which
    /// outranks purlis's setting, or they could not be read. A sentence naming the file; none
    /// where nothing says so.
    #[specta(optional)]
    pub local_ports: Option<String>,
}

/// What a sandboxed Claude Code chat's Network view says of `binding` (#1699), as the doctor's
/// `sandbox local ports` row says it.
pub fn local_ports_of(binding: &sandbox::claude::LocalBinding) -> Option<String> {
    use sandbox::claude::LocalBinding;
    match binding {
        LocalBinding::Off => None,
        LocalBinding::On(file) => Some(format!(
            "Your administrator's Claude Code settings ({}) turn local binding on, which \
             outranks purlis's: this chat's commands can connect to every port on this \
             machine, every local service among them.",
            file.display()
        )),
        LocalBinding::Unread { file, why } => Some(format!(
            "purlis could not read {} ({why}), so it cannot tell whether your administrator's \
             Claude Code settings let this chat's commands connect to every port on this \
             machine.",
            file.display()
        )),
    }
}

/// `plane`'s Open hosts under `locks`: each preset in force, then its own hosts. None where
/// its chats are not sandboxed.
fn open_of(plane: &sandbox::Plane, locks: &sandbox::policy::Locks) -> Option<Vec<OpenHosts>> {
    let policy = plane.in_force(locks)?;
    let mut out: Vec<OpenHosts> = locks
        .presets(&policy.egress)
        .into_iter()
        .map(|preset| OpenHosts {
            title: preset.title().to_owned(),
            hosts: sandbox::hosts(&[preset], plane, locks),
        })
        .collect();
    let own: Vec<String> = plane.granted_hosts(locks);
    let presets: Vec<String> = out.iter().flat_map(|one| one.hosts.clone()).collect();
    let own: Vec<String> = own
        .into_iter()
        .filter(|host| !presets.contains(host))
        .collect();
    if !own.is_empty() {
        out.push(OpenHosts {
            title: "This project's hosts".to_owned(),
            hosts: own,
        });
    }
    Some(out)
}

/// Whether one of `listed` (as a sandbox lists hosts) lets a chat reach `target`.
fn covered(listed: &[String], target: &Host) -> bool {
    listed
        .iter()
        .filter_map(|one| Host::parse(one).ok())
        .any(|one| one.covers(target))
}

/// Every host every chat here reaches on this machine: the Open hosts and yours.
fn reached_here(root: &Path, open: &[OpenHosts]) -> Vec<String> {
    open.iter()
        .flat_map(|one| one.hosts.clone())
        .chain(
            sandbox::hosts::personal(root)
                .iter()
                .map(ToString::to_string),
        )
        .collect()
}

/// The scopes Allow on a row for `target` may keep it at, under `locks`.
fn levels_for(target: &Host, locks: &sandbox::policy::Locks) -> Vec<GrantLevel> {
    [grant::Level::You, grant::Level::Project]
        .into_iter()
        .filter(|level| {
            locks
                .refuses_grant(&grant::What::Host(target.clone()), *level)
                .is_none()
        })
        .map(GrantLevel::from)
        .collect()
}

/// The rows `entries` make, newest first: one per host refused (or per kind of refusal where
/// none was named), with how often, judged against `reached` and `locks`. Only a refused
/// connection's host is ever offered to allow; a refused lookup's is shown and offered nothing
/// (#1663).
fn rows_of(
    entries: &[Entry],
    reached: &[String],
    locks: &sandbox::policy::Locks,
) -> Vec<BlockedLately> {
    let mut rows: Vec<BlockedLately> = Vec::new();
    for entry in entries.iter().rev().filter(|entry| entry.is_host_block()) {
        let Some(block) = entry.block else { continue };
        let said = block.said();
        let at = u32::try_from(entry.at).unwrap_or(u32::MAX);
        if let Some(row) = rows.iter_mut().find(|row| {
            row.target == entry.target && row.looked_up == entry.looked_up && row.said == said
        }) {
            row.times = row
                .times
                .saturating_add(u32::try_from(entry.blocks()).unwrap_or(u32::MAX));
            continue;
        }
        let host = entry
            .target
            .as_deref()
            .filter(|_| (block.operation, block.kind) == (Operation::Connect, Kind::Host))
            .and_then(|target| Host::parse(target).ok());
        let is_reached = host.as_ref().is_some_and(|host| covered(reached, host));
        rows.push(BlockedLately {
            target: entry.target.clone(),
            looked_up: entry.looked_up.clone(),
            said,
            chat: entry.chat.name.clone(),
            session: None,
            at,
            times: u32::try_from(entry.blocks()).unwrap_or(u32::MAX),
            reached: is_reached,
            levels: match &host {
                Some(host) if !is_reached => levels_for(host, locks),
                _ => Vec::new(),
            },
        });
    }
    rows.truncate(AT_MOST_LISTED);
    rows
}

/// Who blocked a call, as a row of the Blocked log says it (the Chat log's words, R-187).
fn blocked_by(source: Source) -> &'static str {
    match source {
        Source::Guard => "blocked by purlis's guards",
        Source::HarnessCheck => "blocked by its harness's own check",
        Source::Sandbox => "blocked by the sandbox",
        Source::Policy => "blocked by your organisation's policy",
        Source::ProjectSetting => "blocked by the project's settings",
    }
}

/// A block's target summary, as its row names it: a host, a folder in the project, a service or
/// a guard's arm (R-220, Q206).
fn summary_of(target: &Target) -> &str {
    match target {
        Target::Host(host) => host.as_str(),
        Target::Folder(folder) => folder.as_str(),
        Target::Service(service) => service.as_str(),
        Target::GuardArm(arm) => arm.as_str(),
    }
}

/// **The Blocked log's rows** of `listed` (newest first), as Settings draws them (T1-32, Q264):
/// one per target and who blocked it, with how often, the newest's chat by the name `name_of`
/// finds and by the number `session_of` finds it open under (R-226), judged against `reached`
/// and `locks`. Only a host purlis's own proxy saw refused is
/// offered to allow, and in a group the newest such block decides it (Q265): a host a chat's
/// own hook reported is unverified (Q254) and may be a program's own printed words, a refused
/// lookup's among them (#1663).
fn blocked_log_rows(
    listed: &[Listed],
    name_of: &dyn Fn(&str) -> Option<String>,
    session_of: &dyn Fn(&str) -> Option<u32>,
    reached: &[String],
    locks: &sandbox::policy::Locks,
) -> Vec<BlockedLately> {
    let mut rows: Vec<BlockedLately> = Vec::new();
    for one in listed {
        let block = &one.block;
        let target = block
            .target
            .as_ref()
            .map(|target| summary_of(target).to_owned());
        let said = blocked_by(block.source).to_owned();
        let host = match &block.target {
            Some(Target::Host(host)) => Host::parse(host.as_str()).ok(),
            _ => None,
        };
        let is_reached = host.as_ref().is_some_and(|host| covered(reached, host));
        let offered = seen_by_the_sandbox(block);
        let levels = || match &host {
            Some(host) if offered && !is_reached => levels_for(host, locks),
            _ => Vec::new(),
        };
        if let Some(row) = rows
            .iter_mut()
            .find(|row| row.target == target && row.said == said)
        {
            row.times = row.times.saturating_add(1);
            // The newest verified block decides the Allow (Q265): a later line a hook reported
            // never hides it.
            if offered && row.levels.is_empty() {
                row.levels = levels();
            }
            continue;
        }
        rows.push(BlockedLately {
            target,
            looked_up: None,
            said,
            chat: name_of(&one.chat),
            session: session_of(&one.chat),
            at: u32::try_from(one.at_ms / 1000).unwrap_or(u32::MAX),
            times: 1,
            reached: is_reached,
            levels: levels(),
        });
    }
    rows.truncate(AT_MOST_LISTED);
    rows
}

/// Whether `block` is one purlis's own sandbox saw (Q254, Q265): the sandbox's, with no
/// qualifier. Only such a block of a host is offered an Allow, and only it lets one through.
fn seen_by_the_sandbox(block: &purlis_core::eventlog::stops::Block) -> bool {
    block.source == Source::Sandbox && block.qualifier.is_none()
}

/// [`SandboxNetwork`] for the project at `root`, whose committed file reads as `plane`, with
/// the Blocked log `listed`, each chat by the name `name_of` finds and the number `session_of`
/// finds it open under.
fn network_of(
    root: &Path,
    plane: &sandbox::Plane,
    listed: &[Listed],
    name_of: &dyn Fn(&str) -> Option<String>,
    session_of: &dyn Fn(&str) -> Option<u32>,
) -> SandboxNetwork {
    let locks = sandbox::policy::Locks::of(root);
    let Some(open) = open_of(plane, &locks) else {
        return SandboxNetwork {
            on: false,
            open: Vec::new(),
            blocked: Vec::new(),
        };
    };
    let blocked = blocked_log_rows(
        listed,
        name_of,
        session_of,
        &reached_here(root, &open),
        &locks,
    );
    SandboxNetwork {
        on: true,
        open,
        blocked,
    }
}

/// The ids of the chats of the project at `root` the app knows, each with the name it is shown
/// under (Q264): those its reopen record holds, then those `chats` has open or closed since
/// launch, whose names win.
fn known_chats_of(
    root: &Path,
    chats: &[(String, String)],
) -> std::collections::BTreeMap<String, String> {
    let mut known: std::collections::BTreeMap<String, String> =
        purlis_core::reopen::read_or_refusal(root)
            .map(|record| record.chats)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|chat| {
                let id = chat.identity.id.clone()?;
                Some((id, purlis_core::reopen::shown_name(&chat, None)))
            })
            .collect();
    known.extend(chats.iter().cloned());
    known
}

/// **Open hosts and the Blocked log** for Settings' Network page (#1662, T1-32).
///
/// Off the window's thread (#1949): it reads the project's manifest, the administrator's policy
/// and this machine's event log.
#[tauri::command]
#[specta::specta]
pub async fn sandbox_network(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<SandboxNetwork, String> {
    let held = planes.held(&plane)?;
    let root = held.root().to_path_buf();
    let chats = held.chats().logged_names();
    // Each chat open here, by its id: a Blocked log row opens its Chat log by its number (R-226).
    let open: std::collections::BTreeMap<String, u32> = chats
        .iter()
        .filter_map(|(id, _)| Some((id.clone(), held.chats().session_of_id(id)?)))
        .collect();
    crate::off_the_window(READING_NETWORK, move || {
        let plane = sandbox::Plane::read(&root);
        let (listed, known) = blocked_log_of(&root, &plane, &chats)?;
        Ok(network_of(
            &root,
            &plane,
            &listed,
            &|chat| known.get(chat).cloned(),
            &|chat| open.get(chat).copied(),
        ))
    })
    .await
}

/// **The Blocked log of the project at `root`**, whose committed file reads as `plane`, from
/// this machine's event log, with the chats `chats` names known as the project's (Q264), and
/// those chats each by the name it is shown under. Nothing is read, and nothing listed, while
/// the project's sandbox is off (Q265) or with no log on this machine.
fn blocked_log_of(
    root: &Path,
    plane: &sandbox::Plane,
    chats: &[(String, String)],
) -> Result<(Vec<Listed>, std::collections::BTreeMap<String, String>), String> {
    let none = || Ok((Vec::new(), std::collections::BTreeMap::new()));
    if plane.in_force(&sandbox::policy::Locks::of(root)).is_none() {
        return none();
    }
    let Some(Ok(log)) = crate::EVENT_LOG.get() else {
        return none();
    };
    let known = known_chats_of(root, chats);
    let now_ms = u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(u64::MAX);
    let listed = blockedlog::listed_in(log, now_ms, &|chat| known.contains_key(chat))
        .map_err(|why| format!("purlis could not read this machine's event log: {why}"))?;
    Ok((listed, known))
}

/// What the commands that read this machine's network record name their work, off the
/// window's thread.
const READING_NETWORK: &str = "reading the network record";

/// [`ChatNetwork`] for chat `session` of the project at `root`, where `chat_sites` are the
/// browser sites the app holds for one chat alone, each with its chat.
fn chat_network_of(
    root: &Path,
    chats: &crate::chats::Chats,
    session: u32,
    chat_sites: &[(u32, Site)],
    record: Option<&Record>,
    now: u64,
) -> ChatNetwork {
    let Some(recorded) = chats.recorded_chat(session) else {
        return ChatNetwork {
            open: false,
            sandboxed: false,
            reach: Vec::new(),
            browser_sites: Vec::new(),
            refused: Vec::new(),
            local_ports: None,
        };
    };
    // Sandboxed by the record of how its run started and the compile this window holds
    // (#2692): one with no record is shown as unsandboxed, with nothing open or blocked.
    let confines = chats
        .confines_of(session)
        .filter(|_| chats.runs_sandboxed(session));
    let Some(confines) = confines else {
        return ChatNetwork {
            open: true,
            sandboxed: false,
            reach: Vec::new(),
            browser_sites: Vec::new(),
            refused: Vec::new(),
            local_ports: None,
        };
    };
    let locks = sandbox::policy::Locks::of(root);
    let plane = sandbox::Plane::read(root);
    let open: Vec<String> = open_of(&plane, &locks)
        .unwrap_or_default()
        .into_iter()
        .flat_map(|one| one.hosts)
        .collect();
    let persona: Vec<String> = recorded
        .persona
        .as_deref()
        .map(|persona| {
            sandbox::persona::shown_in(root, &plane, &locks)
                .into_iter()
                .filter(|one| one.persona == persona && one.allowed())
                .flat_map(|one| one.hosts)
                .map(|host| host.to_string())
                .collect()
        })
        .unwrap_or_default();
    let entries = record.map(|one| one.read(root, now)).unwrap_or_default();
    ChatNetwork {
        open: true,
        sandboxed: true,
        reach: reach_of(&confines.hosts, &open, &persona),
        browser_sites: browser_sites_of(
            &chat_sites_in_force(chat_sites, &locks),
            session,
            &sandbox::local::browser_sites_in_force(root, &locks),
        ),
        refused: refused_of(
            &entries,
            (recorded.identity.id.as_deref(), session),
            &confines.hosts,
            &locks,
        ),
        // Only Claude Code takes an administrator's managed settings (#1699).
        local_ports: (chats.harness(session) == Some(purlis_core::harness::Harness::ClaudeCode))
            .then(|| local_ports_of(&sandbox::claude::administrators_local_binding()))
            .flatten(),
    }
}

/// Each of `hosts` a chat's sandbox reaches, by why: one of the `open` hosts, one of its
/// `persona`'s, or else one allowed.
fn reach_of(hosts: &[String], open: &[String], persona: &[String]) -> Vec<Reached> {
    hosts
        .iter()
        .map(|host| Reached {
            host: host.clone(),
            by: if open.contains(host) {
                ReachedBy::Open
            } else if persona.contains(host) {
                ReachedBy::Persona
            } else {
                ReachedBy::Allowed
            },
        })
        .collect()
}

/// **What every chat's browser-site count is read from** in one project (#2364): this machine's
/// policy as it stands, and the browser sites kept for every chat of the project on this machine
/// that it leaves in force. Read once ([`KeptSites`]) and asked of each chat.
#[derive(Debug)]
pub struct SitesInForce {
    locks: sandbox::policy::Locks,
    kept_here: Vec<Site>,
}

impl SitesInForce {
    /// Read from the policy files and the project at `root`'s kept browser sites.
    pub fn read(root: &Path) -> Self {
        let locks = sandbox::policy::Locks::of(root);
        let kept_here = sandbox::local::browser_sites_in_force(root, &locks);
        Self { locks, kept_here }
    }

    /// **How many sites only chat `session`'s browser reaches**, of `chat_sites`: what its
    /// Network view lists, counted for its tab's sandbox mark.
    pub fn of_chat(&self, chat_sites: &[(u32, Site)], session: u32) -> usize {
        browser_sites_of(
            &chat_sites_in_force(chat_sites, &self.locks),
            session,
            &self.kept_here,
        )
        .len()
    }
}

/// **[`SitesInForce`], kept** (#4739): `opened_chats` is asked at each chat a launch brings
/// back, and counted every chat's sites from the policy files each time, so a restore of fifty
/// chats read them fifty times fifty. They change only when this machine's policy does (the
/// policy watch, `sandboxing::watch_policy`) or when this app keeps or removes a site for every
/// chat here, and each of those forgets what is kept.
#[derive(Debug, Default)]
pub struct KeptSites {
    kept: std::sync::Mutex<Kept>,
}

#[derive(Debug, Default)]
struct Kept {
    /// How many times it was forgotten: a read that began before a forget does not keep what it
    /// read, which may be from before the change.
    forgotten: u64,
    now: Option<std::sync::Arc<SitesInForce>>,
}

impl KeptSites {
    /// What is kept, or what `read` reads where nothing is. `read` runs with no lock held.
    pub fn now(&self, read: impl FnOnce() -> SitesInForce) -> std::sync::Arc<SitesInForce> {
        let forgotten = {
            let kept = self.lock();
            if let Some(now) = &kept.now {
                return std::sync::Arc::clone(now);
            }
            kept.forgotten
        };
        let now = std::sync::Arc::new(read());
        let mut kept = self.lock();
        if kept.forgotten == forgotten {
            kept.now = Some(std::sync::Arc::clone(&now));
        }
        now
    }

    /// The policy or the sites kept here changed: the next ask reads them again.
    pub fn forget(&self) {
        let mut kept = self.lock();
        kept.forgotten += 1;
        kept.now = None;
    }

    /// Whether anything is kept.
    #[cfg(test)]
    pub fn holds(&self) -> bool {
        self.lock().now.is_some()
    }

    /// Through a poisoned lock too: a panic elsewhere costs a read, never a stale answer.
    fn lock(&self) -> std::sync::MutexGuard<'_, Kept> {
        self.kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// `chat_sites`, less each one policy holds at its chat's scope (#2364).
fn chat_sites_in_force(
    chat_sites: &[(u32, Site)],
    locks: &sandbox::policy::Locks,
) -> Vec<(u32, Site)> {
    chat_sites
        .iter()
        .filter(|(_, site)| {
            sandbox::local::browser_site_held_at(site, grant::Level::Chat, locks).is_none()
        })
        .cloned()
        .collect()
}

/// **The sites only chat `session`'s browser reaches** (#2364): of `chat_sites`, the ones
/// allowed for it alone, then each of `kept_here`, the browser sites kept for every chat of
/// this project on this machine, each once.
fn browser_sites_of(chat_sites: &[(u32, Site)], session: u32, kept_here: &[Site]) -> Vec<String> {
    let mine = chat_sites
        .iter()
        .filter(|(chat, _)| *chat == session)
        .map(|(_, site)| site);
    let mut shown: Vec<String> = Vec::new();
    for site in mine.chain(kept_here) {
        let written = site.to_string();
        if !shown.contains(&written) {
            shown.push(written);
        }
    }
    shown
}

/// What the chat whose id is `id` (or, with none, whose number is `session`) was refused, as
/// rows judged against what it `reached` and `locks`: by its id, which outlives a restart and a
/// relaunch, so another chat that has its number since is never shown as it.
fn refused_of(
    entries: &[Entry],
    (id, session): (Option<&str>, u32),
    reached: &[String],
    locks: &sandbox::policy::Locks,
) -> Vec<BlockedLately> {
    let mine: Vec<Entry> = entries
        .iter()
        .filter(|entry| match (id, entry.chat.id.as_deref()) {
            (Some(id), Some(theirs)) => id == theirs,
            (None, None) => entry.chat.session == Some(session),
            _ => false,
        })
        .cloned()
        .collect();
    rows_of(&mine, reached, locks)
        .into_iter()
        .map(|row| BlockedLately { chat: None, ..row })
        .collect()
}

/// **A chat's Network view** (#1662): what chat `session` reaches now, and what it was refused.
#[tauri::command]
#[specta::specta]
pub async fn chat_network(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
) -> Result<ChatNetwork, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    // Off the window's thread (#1949): the chat's part of the network record is read whole.
    crate::off_the_window(READING_NETWORK, move || {
        Ok(chat_network_of(
            held.root(),
            held.chats(),
            session,
            &held.browsers().chat_sites(),
            network.as_ref(),
            now_secs(),
        ))
    })
    .await
}

/// One **Keep blocked** a person answered here (#1700): what the Inbox's look back lists beside
/// what was allowed, since a no leaves no grant behind.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct KeptBlocked {
    /// The host and port kept blocked, as a grant names it.
    pub target: String,
    /// The chat it was kept blocked for, by the name it was shown under, where the record has it.
    pub chat: Option<String>,
    /// When, in seconds since 1970.
    pub at: u32,
}

/// The Keep blocked answers among `entries`, newest first, at most [`AT_MOST_LISTED`].
fn kept_blocked_of(entries: &[Entry]) -> Vec<KeptBlocked> {
    entries
        .iter()
        .rev()
        .filter(|entry| entry.is_kept_blocked())
        .filter_map(|entry| {
            Some(KeptBlocked {
                target: entry.target.clone()?,
                chat: entry.chat.name.clone(),
                at: u32::try_from(entry.at).unwrap_or(u32::MAX),
            })
        })
        .take(AT_MOST_LISTED)
        .collect()
}

/// **What the person kept blocked here** (#1700), from this machine's network record, newest
/// first: the window's alone (`WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn sandbox_kept_blocked(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<Vec<KeptBlocked>, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    let network = planes.network().cloned();
    // Off the window's thread (#1949): the record is read whole.
    crate::off_the_window(READING_NETWORK, move || {
        let entries = network
            .map(|record| record.read(&root, now_secs()))
            .unwrap_or_default();
        Ok(kept_blocked_of(&entries))
    })
    .await
}

/// **Allow on the Blocked log** in the project at `root` (#1662): `host`, judged by the core as a
/// block's Allow is (never the window's word, never past a policy lock), then audited and kept
/// by the same path a block's Notice keeps it by (`sandboxing::kept_for`), for this project on
/// this machine or for everyone in it. No chat is named, so none is owed a restart: each takes
/// it from its next start.
///
/// **Only a host the row offered to allow**, never any host the window names: one the network
/// record `lately` has a refused connection to (#1681), and that `blocked_log` has a verified
/// sandbox block of (Q265: the sandbox's, with no qualifier). A host the record never refused,
/// one a program only said it looked up, the same host on another port, and one only a chat's
/// own hook or a guard reported are refused, and nothing is audited.
#[allow(clippy::too_many_arguments)]
fn allow_blocked(
    root: &Path,
    chats: &crate::chats::Chats,
    lately: &[Entry],
    blocked_log: &[Listed],
    host: &str,
    level: GrantLevel,
    audit: Audit<'_>,
    at: u64,
) -> Result<Allowed, String> {
    let (what, level) = crate::sandboxing::judged(
        root,
        &sandbox::Machine::this(),
        None,
        0,
        (crate::sandboxing::GrantWhat::Host, host, level),
    )?;
    let named = what.target();
    let refused_here = lately.iter().any(|entry| {
        entry.event == record::Event::Block
            && entry.block.is_some_and(|block| {
                (block.operation, block.kind) == (Operation::Connect, Kind::Host)
            })
            && entry.target.as_deref() == Some(named.as_str())
    });
    if !refused_here {
        return Err(format!(
            "purlis allowed nothing: {named} is not a connection refused here lately, so it \
             cannot be allowed from the Blocked log."
        ));
    }
    let verified_here = blocked_log.iter().any(|one| {
        let blocked = match &one.block.target {
            Some(Target::Host(blocked)) => Host::parse(blocked.as_str()).ok(),
            _ => None,
        };
        seen_by_the_sandbox(&one.block)
            && blocked.is_some_and(|blocked| blocked.to_string() == named)
    });
    if !verified_here {
        return Err(format!(
            "purlis allowed nothing: {named} is not a connection the sandbox refused here, so it \
             cannot be allowed from the Blocked log."
        ));
    }
    let kept = crate::sandboxing::kept_for(root, chats, None, (&what, level), audit, at)?;
    Ok(Allowed {
        // Whether any running chat took it at once (#1666); none is owed a restart from here.
        live: !kept.live.is_empty(),
        said: match (kept.already, kept.live.is_empty()) {
            (true, _) => format!("{} was allowed already, {}.", what.target(), level.said()),
            // A chat whose proxy asks live takes it now (#1666); any other at its next start.
            (false, false) => format!(
                "Allowed {} {}. Running chats reach it now, or from their next start.",
                what.target(),
                level.said()
            ),
            (false, true) => format!(
                "Allowed {} {}. A chat that is running reaches it from its next start.",
                what.target(),
                level.said()
            ),
        },
    })
}

/// **Allow** on a row of the Blocked log (#1662): the window's alone (`WINDOW_ONLY`).
#[tauri::command]
#[specta::specta]
pub async fn allow_blocked_host(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    host: String,
    level: GrantLevel,
) -> Result<Allowed, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    // Off the window's thread (#1949): the record and the Blocked log are read, the grant
    // judged, audited and kept.
    crate::off_the_window("keeping your Allow", move || {
        let root = held.root();
        let at = now_secs();
        let lately = network
            .as_ref()
            .map(|record| record.read(root, at))
            .unwrap_or_default();
        let (blocked_log, _) = blocked_log_of(
            root,
            &sandbox::Plane::read(root),
            &held.chats().logged_names(),
        )?;
        let audit = held.audit_then_record(network.as_ref());
        allow_blocked(
            root,
            held.chats(),
            &lately,
            &blocked_log,
            &host,
            level,
            &audit,
            at,
        )
    })
    .await
}

/// **Allow next time on a sandbox block** (T1-34; R-291, Q188): the window's alone
/// (`WINDOW_ONLY`), pressed on chat `session`'s block of `target` in its Chat log or on its
/// turn's finish row, with no ask open. A chat never reaches it.
#[tauri::command]
#[specta::specta]
pub async fn allow_next_time(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
    session: u32,
    target: String,
    level: GrantLevel,
) -> Result<Allowed, String> {
    let held = planes.held(&plane)?;
    let network = planes.network().cloned();
    // Off the window's thread (#1949): the Blocked log is read, the grant judged, audited and
    // kept.
    crate::off_the_window("keeping your Allow", move || {
        let root = held.root();
        let (blocked_log, _) = blocked_log_of(
            root,
            &sandbox::Plane::read(root),
            &held.chats().logged_names(),
        )?;
        let audit = held.audit_then_record(network.as_ref());
        allow_next(
            root,
            held.chats(),
            &blocked_log,
            (session, &target, level),
            &audit,
            now_secs(),
        )
    })
    .await
}

/// **Allow next time on chat `session`'s sandbox block of `target`, at `level`** (T1-34, R-291),
/// in the project at `root`: a grant made with no ask open, **judged again by the core**, never
/// on the window's word.
///
/// - **Only a block the row offers it on:** a host purlis's own proxy refused for this chat, as
///   `blocked_log` has it (a sandbox block with no qualifier, Q265). A host only the chat's hook
///   named, a guard's block, another chat's, and any host the window names are refused, and
///   nothing is audited.
/// - **Once** is the chat's next attempt at it alone: audited, then held on the chat's board
///   ([`purlis_core::sandbox::Board::allow_next`]), which that attempt uses up. Until then it is
///   listed under Allowed, and it can be removed there. Never past a policy lock, and only for a
///   chat whose proxy asks live, where there is a next attempt to let through.
/// - **A wider label** is the grant the block's Notice makes at that level
///   ([`crate::sandboxing::allow_host_from_a_block`]), which stays until it is removed.
///
/// **A waiting once lives in memory alone** (Q278): a chat, session host or app restart forgets
/// it, as the merge hold is forgotten, and nothing on disk outlives it. It is audited when it is
/// granted (here), when its attempt uses it up, as the person's grant and never the chat's, and
/// when it is put out past the most a board keeps ([`once_ended`], told by the chat's board),
/// and when it is removed (`sandboxing::revoke`).
fn allow_next(
    root: &Path,
    chats: &crate::chats::Chats,
    blocked_log: &[Listed],
    (session, target, level): (u32, &str, GrantLevel),
    audit: Audit<'_>,
    at: u64,
) -> Result<Allowed, String> {
    let host = grant::host(target).map_err(|why| why.to_string())?;
    let named = host.to_string();
    let chat = chats
        .id_of(session)
        .ok_or_else(|| format!("Nothing was allowed: chat {session} is not open."))?;
    let blocked_here = blocked_log.iter().any(|one| {
        one.chat == chat
            && seen_by_the_sandbox(&one.block)
            && matches!(
                &one.block.target,
                Some(Target::Host(blocked))
                    if Host::parse(blocked.as_str()).is_ok_and(|blocked| blocked == host)
            )
    });
    if !blocked_here {
        return Err(format!(
            "Nothing was allowed: the sandbox did not block {named} for this chat, so Allow \
             next time can't allow it."
        ));
    }
    if level != GrantLevel::Once {
        return crate::sandboxing::allow_host_from_a_block(
            root,
            &sandbox::Machine::this(),
            chats,
            session,
            (&named, level),
            audit,
            at,
        );
    }
    let what = grant::What::Host(host.clone());
    if let Some(why) = sandbox::policy::Locks::of(root).refuses_grant(&what, grant::Level::Once) {
        return Err(why);
    }
    let board = chats.board_of(session).ok_or_else(|| {
        "Nothing was allowed: this chat's sandbox doesn't ask about hosts while it runs, so \
         there is no next attempt to let through. Allow it for this chat or longer."
            .to_owned()
    })?;
    let (word, written) = (what.word(), named.as_str());
    let audited = |granted: bool| grant::Audited {
        granted,
        what: word,
        target: written,
        level: grant::Level::Once,
    };
    audit(Some(session), &audited(true))?;
    // Kept only once audited: where the board can't take it, the audit is told it ended.
    if let Err(why) = board.allow_next(&host) {
        if let Err(unsaid) = audit(Some(session), &audited(false)) {
            tracing::warn!("purlis: an Allow next time not kept was not audited ({unsaid})");
        }
        return Err(format!("Nothing was allowed: {why}"));
    }
    Ok(Allowed {
        said: format!(
            "Allowed {named} once: the chat's next attempt at it goes through, and Allowed lists \
             it until then."
        ),
        // Its board takes it: nothing restarts.
        live: true,
    })
}

/// **The audit of a waiting Allow next time · once of `target` that ended** (T1-34, Q278): its
/// attempt used it up, or it was put out without one. The person's grant ending, as a removal
/// is audited.
pub(crate) fn once_ended(target: &str) -> grant::Audited<'_> {
    grant::Audited {
        granted: false,
        what: "host",
        target,
        level: grant::Level::Once,
    }
}

/// **The labels a sandbox block's row may offer for Allow next time** (T1-34, Q278; #1906: no
/// Allow that wouldn't unblock it), in their order, under `locks`, as the Blocked log's
/// [`levels_for`] says its row's: only for a host purlis's own proxy refused (no qualifier), and
/// only those the policy keeps. None for any other block, which offers no Allow next time.
pub(crate) fn next_time_levels(
    locks: &sandbox::policy::Locks,
    source: &str,
    qualifier: Option<&str>,
    target_kind: Option<&str>,
    target: Option<&str>,
) -> Vec<GrantLevel> {
    let host = match (source, qualifier, target_kind, target) {
        ("sandbox", None, Some("host"), Some(target)) => Host::parse(target).ok(),
        _ => None,
    };
    let Some(host) = host else {
        return Vec::new();
    };
    let what = grant::What::Host(host);
    [
        grant::Level::Once,
        grant::Level::You,
        grant::Level::Chat,
        grant::Level::Project,
    ]
    .into_iter()
    .filter(|level| locks.refuses_grant(&what, *level).is_none())
    .map(GrantLevel::from)
    .collect()
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "network_kept_tests.rs"]
mod kept_tests;
