//! The `sandbox` row: where the project turned the sandbox on (ADR 0067), how many new chats
//! on this machine started without it — the opt-out rate SD-2's outcome bar is measured by
//! (V12). A local count, shown here and in Settings and never sent (ruling V78 d).

use super::{Config, Doctor, Row};

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

/// The `sandbox blocks` row (#1338): what chats' sandboxes blocked in this project on this
/// machine over the last seven days, counted per operation, as the app heard each one, and the
/// hosts refused most (#1662), read from this machine's network record
/// ([`crate::sandboxblock::record`]). Only where something was blocked: a project with none
/// prints the rows it always printed.
///
/// **A block of purlis's own operation is a purlis bug**, so any makes the row a warning that
/// says how to report it. Every other block is the chat's own work meeting the sandbox: counted,
/// not a fault.
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
