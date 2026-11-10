// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The week's measures of asking** (ADR 0121 decision 18, #6021): for one project on this
//! machine, how many asks the person answered in Asks in the last seven days, the median time
//! from an ask being raised to its answer, and how many asks its rules saved.
//!
//! Each is read from what is kept already, and nothing is kept for them: the answers in
//! [`super::Record`], and for the asks saved, the connections purlis's own proxy carried for the
//! project's chats (`sandboxblock::record`, kept 30 days).
//!
//! **An ask saved** is a need a rule met that would otherwise have asked: a chat reaching a
//! rule's host, let through at the rule's scope, after the rule was made. Each chat counts once a
//! day, as an ask for a host is one per chat until its answer runs out, and the chat the rule was
//! made in does not count on the day it was made, where its own answer still covered it. Only a
//! host's rule is counted: a folder's, a service's and a run's grant is met with no record of it,
//! so their saves are not known and are not guessed.

use std::collections::BTreeSet;

use super::{Record, Rule, Scope};
use crate::sandboxblock::record::{Entry, Event, Outcome, Scope as Decided};

/// **What "this week" is**: the seven days before now, as long as answers are kept.
pub const WEEK_SECS: u64 = super::OFFER_WINDOW_SECS;

/// A day, in seconds: a chat's need of a host is counted once over it.
const DAY_SECS: u64 = 24 * 60 * 60;

/// The source a host's ask is named by, as `asking::request_of` names it.
const SANDBOX_HOST: &str = "sandbox-host";

/// **One project's week**, as All chats says it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Measures {
    /// The asks answered in Asks in the last seven days.
    pub asks: u32,
    /// The median time from raised to answered, in seconds, over the asks whose raising is known;
    /// none where none is.
    pub median_secs: Option<u64>,
    /// The asks its rules saved in the last seven days.
    pub saved: u32,
}

/// **Project `project`'s week** at `now`: its answers in `record`, and the asks its rules saved
/// among `connections`, its proxy's lines.
pub fn measures(record: &Record, project: &str, connections: &[Entry], now: u64) -> Measures {
    let from = now.saturating_sub(WEEK_SECS);
    let week: Vec<_> = record
        .answered
        .iter()
        .filter(|one| one.project == project && one.at >= from && one.at <= now)
        .collect();
    let waits: Vec<u64> = week
        .iter()
        .filter_map(|one| Some(one.at.saturating_sub(one.raised?)))
        .collect();
    let saved = record
        .rules
        .iter()
        .filter(|rule| rule.project == project)
        .map(|rule| saved_by(rule, connections, from))
        .fold(0_u32, u32::saturating_add);
    Measures {
        asks: u32::try_from(week.len()).unwrap_or(u32::MAX),
        median_secs: median(waits),
        saved,
    }
}

/// **The asks `rule` saved** since `from` among `connections`, its project's proxy lines: each
/// chat a day it reached the rule's host, let through at the rule's scope, after the rule was
/// made, but for the rule's own chat on the day it was made. None for a rule of anything but a
/// host.
pub fn saved_by(rule: &Rule, connections: &[Entry], from: u64) -> u32 {
    let Some(host) = host_of(rule) else {
        return 0;
    };
    let made_on = rule.at / DAY_SECS;
    let needs: BTreeSet<(String, u64)> = connections
        .iter()
        .filter(|one| {
            one.event == Event::Connect
                && one.outcome == Outcome::Allowed
                && one.at > rule.at
                && one.at >= from
                && one.scope.is_some_and(|by| by_the_rule(rule.scope, by))
                && one.target.as_deref().map(without_port) == Some(host.as_str())
        })
        .filter(|one| !(one.chat.session == Some(rule.session) && one.at / DAY_SECS == made_on))
        .map(|one| (chat_of(one), one.at / DAY_SECS))
        .collect();
    u32::try_from(needs.len()).unwrap_or(u32::MAX)
}

/// The host a rule allows, where it is a host's: its request names the source and the target
/// (`["sandbox-host", "api.example.com"]`).
fn host_of(rule: &Rule) -> Option<String> {
    let (source, target): (String, String) = serde_json::from_str(&rule.request).ok()?;
    (source == SANDBOX_HOST).then(|| without_port(&target).to_ascii_lowercase())
}

/// Whether a connection let through `by` that decision is what a rule of `scope` wrote: a chat's
/// grant for this chat, this project's on this machine or everyone's in it for this project, and
/// this machine's for every project.
fn by_the_rule(scope: Scope, by: Decided) -> bool {
    match scope {
        Scope::Chat => by == Decided::Chat,
        Scope::Project => matches!(by, Decided::You | Decided::Project),
        Scope::Machine => by == Decided::You,
    }
}

/// `host:port` as its host; a host as it is.
fn without_port(target: &str) -> &str {
    match target.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => target,
    }
}

/// Which chat a line is of: its id, else its number, else its name.
fn chat_of(line: &Entry) -> String {
    match (&line.chat.id, line.chat.session, &line.chat.name) {
        (Some(id), ..) => format!("id:{id}"),
        (None, Some(session), _) => format!("session:{session}"),
        (None, None, Some(name)) => format!("name:{name}"),
        (None, None, None) => String::new(),
    }
}

/// The median of `values`: the mean of the middle two where there are an even number.
pub fn median(mut values: Vec<u64>) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let middle = values.len() / 2;
    Some(match values.len() % 2 {
        1 => values[middle],
        _ => values[middle - 1].midpoint(values[middle]),
    })
}

#[cfg(test)]
mod tests;
