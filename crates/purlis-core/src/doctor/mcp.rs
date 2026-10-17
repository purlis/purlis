// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `mcp`: whether each MCP server a chat is given can start (#994).
//!
//! A chat purlis starts is given purlis's own server, which is this binary, and, when it runs as
//! a persona, that persona's servers from `personas/<name>/mcp.json` (#1451). A server whose
//! launcher is not there never starts, and the harness does not say so: its tools are simply
//! missing from the chat. Python's `check_mcp_launchers` was written after exactly that, a
//! rename that removed a launcher every registered server still named.
//!
//! **Absolute launchers only.** A bare `npx` is found on the `PATH` of whoever starts the
//! server, which is not the `PATH` this doctor runs under; calling it missing would be a guess
//! about an environment purlis cannot see. An absolute path that is not there is a fact.
//!
//! **What `persona lint` already says is not said again.** A refused server name, and a
//! credentialed server this machine has not approved, are the `personas` row's findings,
//! through the same reader ([`crate::personaverbs::mcp::declared`]). This row asks the one
//! question lint does not: will the program start.
//!
//! WARN, never FAIL: a server that cannot start costs a chat its tools, not the operator their
//! work. Reads files only; asks no harness and starts nothing.

use std::path::Path;

use serde_json::Value;

use super::{DISPLAY_LIMIT, Doctor, PATH_DISPLAY_LIMIT, Row, one_line};
use crate::counts::counted;

const NAME: &str = "mcp";

/// One server a persona's chats are given, whose launcher is an absolute path that is not
/// there.
struct Missing {
    persona: String,
    server: String,
    command: String,
}

pub(super) fn mcp(d: &Doctor) -> Row {
    if !d.has_plane {
        return Row::ok(NAME, "no control plane found");
    }
    let mut servers = 0usize;
    let mut checkable = 0usize;
    let mut missing: Vec<Missing> = Vec::new();
    let personas = crate::personaverbs::names(&d.root);
    let mut declaring = 0usize;
    for persona in &personas {
        let (declared, _refused) = crate::personaverbs::mcp::declared(&d.root, persona);
        let mut any = false;
        for (server, entry) in &declared {
            // purlis's own server rides under this name, and a persona's file cannot stand in
            // for it (`chatstart::servers`): an entry by that name is never started.
            if server == crate::chattools::SERVER {
                continue;
            }
            any = true;
            servers += 1;
            let Some(command) = launcher(entry) else {
                continue;
            };
            checkable += 1;
            if absent(Path::new(command)) {
                missing.push(Missing {
                    persona: persona.clone(),
                    server: server.clone(),
                    command: command.to_owned(),
                });
            }
        }
        declaring += usize::from(any);
    }
    if servers == 0 {
        return Row::ok(
            NAME,
            "no persona declares an MCP server, so a chat is given purlis's own alone",
        );
    }
    if missing.is_empty() {
        let across = format!(
            "{} across {}",
            counted(servers, "persona server", "persona servers"),
            counted(declaring, "persona", "personas")
        );
        return Row::ok(
            NAME,
            if checkable == 0 {
                // None was CHECKED, and none is broken: saying "0 resolve" would read as
                // "nothing is registered", which is another claim about another fact.
                format!("{across}, none on an absolute path")
            } else {
                format!(
                    "{across}, {} on an absolute path, all there",
                    counted(checkable, "launcher", "launchers")
                )
            },
        );
    }
    let named: Vec<String> = missing
        .iter()
        .map(|m| one_line(&format!("{}/{}", m.persona, m.server), DISPLAY_LIMIT))
        .collect();
    let hints: Vec<String> = missing
        .iter()
        .map(|m| {
            format!(
                "{}: {} does not exist, so the server cannot start in a chat as {} — repoint its \
                 command in personas/{}/mcp.json, or remove the server",
                one_line(&m.server, DISPLAY_LIMIT),
                one_line(&m.command, PATH_DISPLAY_LIMIT),
                one_line(&m.persona, DISPLAY_LIMIT),
                one_line(&m.persona, DISPLAY_LIMIT),
            )
        })
        .collect();
    Row::warn(
        NAME,
        format!(
            "{} of {} missing: {}",
            missing.len(),
            counted(checkable, "launcher", "launchers"),
            named.join(", ")
        ),
        hints.join("; "),
    )
}

/// The program an entry starts, when it names one on an absolute path. A server reached by
/// `url` starts nothing here, and a bare name is found on a `PATH` this doctor cannot see.
fn launcher(entry: &Value) -> Option<&str> {
    entry
        .get("command")
        .and_then(Value::as_str)
        .filter(|command| Path::new(command).is_absolute())
}

/// Whether nothing is at `path`. A path the filesystem will not answer for is not called
/// missing: that would be a claim about a file purlis could not look at.
fn absent(path: &Path) -> bool {
    matches!(std::fs::metadata(path), Err(ref e) if crate::memstore::is_absent(e))
}

#[cfg(test)]
mod tests;
