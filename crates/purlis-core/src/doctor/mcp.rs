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
        let across = format!("{servers} persona server(s) across {declaring} persona(s)");
        return Row::ok(
            NAME,
            if checkable == 0 {
                // None was CHECKED, and none is broken: saying "0 resolve" would read as
                // "nothing is registered", which is another claim about another fact.
                format!("{across}, none on an absolute path")
            } else {
                format!("{across}, {checkable} launcher(s) on an absolute path, all there")
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
            "{} of {checkable} launcher(s) missing: {}",
            missing.len(),
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
mod tests {
    use super::*;
    use crate::doctor::Status;

    fn plane() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join("charter.toml"), "schema = 1\n").unwrap();
        for d in ["personas", "inventory", "workspaces"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        (dir, root)
    }

    fn persona(root: &Path, name: &str, mcp: Option<&str>) {
        let dir = root.join("personas").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("persona.md"),
            format!("---\nname: {name}\nrole: R\nvault: none\ndelegate-when: w\n---\n\n# R\n"),
        )
        .unwrap();
        if let Some(mcp) = mcp {
            std::fs::write(dir.join("mcp.json"), mcp).unwrap();
        }
    }

    fn row(root: &Path) -> Row {
        mcp(&Doctor::at(root, root, false, true))
    }

    #[test]
    fn a_project_whose_personas_declare_no_server_gives_a_chat_purlis_s_own_alone() {
        let (_d, root) = plane();
        persona(&root, "ops", None);
        let r = row(&root);
        assert_eq!(r.status, Status::Ok, "{r:?}");
        assert!(r.detail.starts_with("no persona declares"), "{r:?}");
        assert!(!r.deferred(), "{r:?}");
    }

    #[test]
    fn a_launcher_on_an_absolute_path_that_is_there_starts() {
        let (_d, root) = plane();
        let here = std::env::current_exe().unwrap();
        persona(
            &root,
            "ops",
            Some(&format!(
                r#"{{"mcpServers": {{"a": {{"command": "{}"}}, "b": {{"command": "npx"}}, "c": {{"url": "https://x.example.invalid"}}}}}}"#,
                here.display()
            )),
        );
        let r = row(&root);
        assert_eq!(r.status, Status::Ok, "{r:?}");
        assert_eq!(
            r.detail,
            "3 persona server(s) across 1 persona(s), 1 launcher(s) on an absolute path, all there"
        );
    }

    #[test]
    fn bare_commands_are_not_called_checked() {
        let (_d, root) = plane();
        persona(
            &root,
            "ops",
            Some(r#"{"mcpServers": {"b": {"command": "npx"}}}"#),
        );
        assert_eq!(
            row(&root).detail,
            "1 persona server(s) across 1 persona(s), none on an absolute path"
        );
    }

    #[test]
    fn a_launcher_that_is_not_there_is_named_with_its_persona_and_what_clears_it() {
        let (_d, root) = plane();
        let gone = root.join("bin/gone-server");
        persona(
            &root,
            "ops",
            Some(&format!(
                r#"{{"mcpServers": {{"edm": {{"command": "{}"}}}}}}"#,
                gone.display()
            )),
        );
        persona(&root, "dev", None);
        let r = row(&root);
        assert_eq!(r.status, Status::Warn, "{r:?}");
        assert_eq!(r.detail, "1 of 1 launcher(s) missing: ops/edm");
        assert!(r.hint.contains(&gone.display().to_string()), "{r:?}");
        assert!(r.hint.contains("personas/ops/mcp.json"), "{r:?}");
        assert!(!r.deferred(), "{r:?}");
    }

    #[test]
    fn a_persona_entry_under_purlis_s_own_name_is_never_started_so_never_counted() {
        let (_d, root) = plane();
        persona(
            &root,
            "ops",
            Some(&format!(
                r#"{{"mcpServers": {{"{}": {{"command": "/nowhere/at/all"}}}}}}"#,
                crate::chattools::SERVER
            )),
        );
        let r = row(&root);
        assert_eq!(r.status, Status::Ok, "{r:?}");
        assert!(r.detail.starts_with("no persona declares"), "{r:?}");
    }

    #[test]
    fn no_project_is_no_finding() {
        let dir = tempfile::tempdir().unwrap();
        let r = mcp(&Doctor::at(dir.path(), dir.path(), false, true));
        assert_eq!(r, Row::ok(NAME, "no control plane found"));
    }
}
