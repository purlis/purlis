// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The `sandbox` row: where the project turned the sandbox on (ADR 0067), how many new chats
//! on this machine started without it — the opt-out rate SD-2's outcome bar is measured by
//! (V12). A local count, shown here and in Settings and never sent (ruling V78 d).

use super::{Config, Doctor, Row, SettingsGroup};

/// The row, only where the project turned the sandbox on, or an administrator's policy
/// requires it on this machine: any other project prints the rows it always printed.
pub(super) fn sandbox(d: &Doctor) -> Option<Row> {
    const NAME: &str = "sandbox";
    let cfg = match &d.config {
        Config::Read(cfg) => cfg,
        Config::Malformed(_) | Config::Refused(_) => return None,
    };
    // The project's own, or the one an administrator's policy requires here (D-1423-1): one
    // row either way, which says when it is policy's.
    let locks = crate::sandbox::policy::Locks::of(&d.root);
    let said = crate::sandbox::Said::of(Some(cfg));
    let its_own = said.policy.is_some();
    said.in_force(&locks)?;
    Some(Row::ok(
        NAME,
        format!(
            "on{} — {}",
            if its_own { "" } else { ", required by policy" },
            crate::sandbox::local::tally(&d.root).said()
        ),
    ))
}

/// The `sandbox kept files` row: what purlis says of the list of what it keeps from a chat
/// ([`crate::sandbox::kept_said`]).
///
/// **Whenever the list is not whole**, whether or not the project runs its chats sandboxed:
/// why, and what does not happen until it is put right — no sandboxed chat starts
/// ([`crate::sandbox::NotStarted::KeptUntold`]), and the project's own files are not listed in
/// the window. **And where it is whole and has something to say**: an entry left over on this
/// machine, a vault of a kind this purlis does not know, a vault not set up here yet, a folder
/// purlis could not look in. Any other project prints the rows it always printed.
pub(super) fn kept(d: &Doctor) -> Option<Row> {
    if !d.has_plane {
        return None;
    }
    // A file that does not read may turn the sandbox on: said as if it does.
    let in_force = match &d.config {
        Config::Read(cfg) => crate::sandbox::Said::of(Some(cfg))
            .in_force(&crate::sandbox::policy::Locks::of(&d.root))
            .is_some(),
        Config::Malformed(_) | Config::Refused(_) => true,
    };
    kept_for(&d.root, &crate::sandbox::Machine::this(), in_force)
}

/// [`kept`]'s row for the project at `root` on `machine`, where `in_force` is whether its
/// chats run sandboxed.
fn kept_for(
    root: &std::path::Path,
    machine: &crate::sandbox::Machine,
    in_force: bool,
) -> Option<Row> {
    let (untold, notes) = crate::sandbox::kept_said(root, machine);
    kept_row(untold.as_ref(), &notes, in_force)
}

/// [`kept`]'s row for a list that is not whole because of `untold`, or that is whole and
/// noted `notes`.
fn kept_row(
    untold: Option<&crate::sandbox::Untold>,
    notes: &[crate::sandbox::KeptNote],
    in_force: bool,
) -> Option<Row> {
    use crate::sandbox::KeptNote;
    const NAME: &str = "sandbox kept files";
    let said = |text: String| super::one_line(&text, super::PATH_DISPLAY_LIMIT);
    let noted: Vec<String> = notes.iter().map(|note| said(note.to_string())).collect();
    if let Some(untold) = untold {
        let held = if in_force {
            "No sandboxed chat starts in this project, and its own files are not listed in the \
             window, until this is put right."
        } else {
            "The project's own files are not listed in the window until this is put right, and \
             no chat would start sandboxed."
        };
        let repair = if untold.registry.is_some() {
            "Fix the file named above, or remove the vault in its view or with `purlis vault \
             remove <name>`."
        } else {
            "Leave the file named above one name: remove its other names, or move the vault's \
             secrets to a new file."
        };
        return Some(Row::warn(
            NAME,
            format!(
                "purlis cannot tell what to keep from a chat: {}",
                said(untold.to_string())
            ),
            format!("{held} {repair}"),
        ));
    }
    if noted.is_empty() {
        return None;
    }
    let detail = noted.join("; ");
    if notes
        .iter()
        .all(|note| matches!(note, KeptNote::NotSetUp(_)))
    {
        return Some(Row::ok(NAME, detail));
    }
    Some(Row::warn(
        NAME,
        detail,
        "Chats start. An entry left over is removed in the vault's view or with `purlis vault \
         remove <name>`. A vault of a kind this purlis does not know asks for a newer purlis. A \
         folder purlis could not look in is one to make readable.",
    ))
}

/// The `sandbox forge hosts` row (#1405): a `[[forge]]` host the sandbox does not let chats
/// reach while it and the `forge` preset are on ([`crate::sandbox::Refusal::ForgeHost`]), said
/// as Settings says it. Only where one is dropped: any other project prints the rows it always
/// printed.
pub(super) fn forge_hosts(d: &Doctor) -> Option<Row> {
    const NAME: &str = "sandbox forge hosts";
    let Config::Read(cfg) = &d.config else {
        return None;
    };
    let dropped: Vec<String> = crate::sandbox::Said::of(Some(cfg))
        .refused
        .iter()
        .filter(|refusal| matches!(refusal, crate::sandbox::Refusal::ForgeHost(..)))
        .map(|refusal| super::one_line(&refusal.to_string(), super::PATH_DISPLAY_LIMIT))
        .collect();
    if dropped.is_empty() {
        return None;
    }
    Some(
        Row::warn(
            NAME,
            dropped.join("; "),
            "Sandboxed chats cannot reach that forge; outside the sandbox it works as before. \
             Its host is set in Settings, under the project's forges.",
        )
        .in_settings(SettingsGroup::Forges),
    )
}

/// The most entries [`cache_home`] counts before it says "at least": a walk bounded as the
/// sweep of gone projects' homes is, so a doctor never stalls on a huge cache.
const MOST_ENTRIES: usize = 200_000;

/// The `sandbox cache home` row (#1364): how much disk the package caches of this project's
/// sandboxed chats take on this machine ([`crate::sandbox::caches`]). Only where that home was
/// made, and never in the preflight, which walks no cache: any other run prints the rows it
/// always printed.
pub(super) fn cache_home(d: &Doctor) -> Option<Row> {
    const NAME: &str = "sandbox cache home";
    if d.preflight {
        return None;
    }
    let home = d.cache_home.as_ref()?;
    if !std::fs::symlink_metadata(home).is_ok_and(|meta| meta.is_dir()) {
        return None;
    }
    let (bytes, whole) = size_of(home, MOST_ENTRIES);
    Some(Row::ok(
        NAME,
        format!(
            "{}{} in {}, the package caches this project's sandboxed chats download into; \
             purlis removes it once the project is gone and it has gone unused for a month",
            if whole { "" } else { "at least " },
            bytes_said(bytes),
            super::short_path(home)
        ),
    ))
}

/// The bytes the files under `dir` hold, links never followed, and whether every entry was
/// counted: the walk stops after `most` entries.
pub(super) fn size_of(dir: &std::path::Path, most: usize) -> (u64, bool) {
    let mut bytes = 0u64;
    let mut seen = 0usize;
    let mut todo = vec![dir.to_path_buf()];
    while let Some(next) = todo.pop() {
        let Ok(listing) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in listing.flatten() {
            if seen >= most {
                return (bytes, false);
            }
            seen += 1;
            let Ok(meta) = std::fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if meta.is_dir() {
                todo.push(entry.path());
            } else if meta.is_file() {
                bytes = bytes.saturating_add(meta.len());
            }
        }
    }
    (bytes, true)
}

/// `bytes` as a reader expects a size: decimal units, one place after the point.
pub(super) fn bytes_said(bytes: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(1_000_000_000, "GB"), (1_000_000, "MB"), (1_000, "KB")];
    for (size, unit) in UNITS {
        if bytes >= size {
            let value = bytes as f64 / size as f64;
            return format!("{value:.1} {unit}");
        }
    }
    format!("{bytes} bytes")
}

/// The `sandbox local ports` row (#1699): where the sandbox is in force here and an
/// administrator's managed Claude Code settings turn local binding on, which purlis's own
/// setting does not outrank, so a Claude Code chat may connect to every local port. Only where
/// one does: any other project prints the rows it always printed.
pub(super) fn local_ports(d: &Doctor) -> Option<Row> {
    let Config::Read(cfg) = &d.config else {
        return None;
    };
    let locks = crate::sandbox::policy::Locks::of(&d.root);
    crate::sandbox::Said::of(Some(cfg)).in_force(&locks)?;
    local_ports_row(&crate::sandbox::claude::administrators_local_binding())
}

/// The row [`local_ports`] prints for `binding`.
fn local_ports_row(binding: &crate::sandbox::claude::LocalBinding) -> Option<Row> {
    use crate::sandbox::claude::LocalBinding;
    const NAME: &str = "sandbox local ports";
    match binding {
        LocalBinding::Off => None,
        LocalBinding::On(file) => Some(Row::ok(
            NAME,
            format!(
                "an administrator's Claude Code settings turn local binding on ({}), which \
                 outranks purlis's: a sandboxed Claude Code chat can connect to every port on \
                 this machine, every local service among them. Codex and opencode chats cannot",
                file.display()
            ),
        )),
        LocalBinding::Unread { file, why } => Some(Row::warn(
            NAME,
            format!(
                "purlis could not read {} ({why}), so it cannot tell whether an \
                 administrator's Claude Code settings let a sandboxed chat connect to every \
                 local port",
                file.display()
            ),
            "Those are your administrator's managed settings: ask them, or make the file \
             readable.",
        )),
    }
}

/// The `sandbox grants held` row (#1927): what a person allowed on this machine, for this
/// project or for every project, that an administrator's policy holds now, so it reaches no
/// chat ([`crate::sandbox::held_by_policy`]). Only where the sandbox is in force here and
/// something is held: any other project prints the rows it always printed.
pub(super) fn grants_held(d: &Doctor) -> Option<Row> {
    let Config::Read(cfg) = &d.config else {
        return None;
    };
    let locks = crate::sandbox::policy::Locks::of(&d.root);
    crate::sandbox::Said::of(Some(cfg)).in_force(&locks)?;
    grants_held_row(&crate::sandbox::held_by_policy(&d.root))
}

/// The row [`grants_held`] prints for `held`: how many, at which scope, and the policy's own
/// sentence for each reason, once. It counts and names nothing kept: a path or a name is said
/// in Settings, drawn as data.
fn grants_held_row(held: &[crate::sandbox::HeldGrant]) -> Option<Row> {
    const NAME: &str = "sandbox grants held";
    if held.is_empty() {
        return None;
    }
    let everywhere = held
        .iter()
        .filter(|one| one.level == crate::sandbox::grant::Level::Machine)
        .count();
    let mut why: Vec<&str> = Vec::new();
    for one in held {
        if !why.contains(&one.why.as_str()) {
            why.push(&one.why);
        }
    }
    Some(Row::ok(
        NAME,
        format!(
            "{} allowed on this machine reach no chat while policy holds them ({} for this \
             project, {} for every project). {} They stay listed in Settings › Sandbox › \
             Allowed, with Remove",
            held.len(),
            held.len() - everywhere,
            everywhere,
            why.join(" ")
        ),
    ))
}

/// The `sandbox blocks` row (#1338): what chats' sandboxes blocked in this project on this
/// machine over the last seven days, counted per operation, as the app heard each one, and the
/// hosts refused most (#1662), read from this machine's network record
/// ([`crate::sandboxblock::record`]). Only where something was blocked: a project with none
/// prints the rows it always printed.
///
/// **A block of purlis's own operation is a purlis bug**, so any makes the row a warning that
/// says how to report it. Every other block is the chat's own work meeting the sandbox: counted,
/// not a fault. purlis's own are counted at most once per chat a day (#1353), so one chat's
/// lines, however many, count as one a day.
pub(super) fn blocks(d: &Doctor) -> Option<Row> {
    blocks_at(d, now())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

/// How many refused hosts the `sandbox blocks` row names.
const HOSTS_NAMED: usize = 3;

pub(super) fn blocks_at(d: &Doctor, now: u64) -> Option<Row> {
    const NAME: &str = "sandbox blocks";
    let entries = d
        .network
        .as_ref()
        .map(|record| record.read(&d.root, now))
        .unwrap_or_default();
    let counts = crate::sandboxblock::record::counts(&entries, now);
    if counts.is_empty() {
        return None;
    }
    let said: Vec<String> = counts
        .iter()
        .map(|count| {
            let ours = if count.ours > 0 {
                format!(" ({} purlis's own)", count.ours)
            } else {
                String::new()
            };
            format!("{} {}{ours}", count.operation.word(), count.blocks)
        })
        .collect();
    let hosts = crate::sandboxblock::record::hosts_refused(&entries, now);
    let named: Vec<String> = hosts
        .iter()
        .take(HOSTS_NAMED)
        .map(|(host, times)| format!("{host} ({times})"))
        .collect();
    let more = match hosts.len().saturating_sub(HOSTS_NAMED) {
        0 => String::new(),
        n => format!(" and {n} more"),
    };
    let refused = if named.is_empty() {
        String::new()
    } else {
        format!("; hosts refused most: {}{more}", named.join(", "))
    };
    let detail = format!("{} in the last 7 days{refused}", said.join(", "));
    if counts.iter().any(|count| count.ours > 0) {
        Some(Row::warn(
            NAME,
            detail,
            "A block of purlis's own operation is a purlis bug. The chat's tab offers Report, \
             which shows a draft naming only the operation, the kind of path and the versions, \
             and sends nothing until you press File report.",
        ))
    } else {
        Some(Row::ok(NAME, detail))
    }
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod tests;
