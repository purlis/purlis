// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

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
        "3 persona servers across 1 persona, 1 launcher on an absolute path, all there"
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
        "1 persona server across 1 persona, none on an absolute path"
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
    assert_eq!(r.detail, "1 of 1 launcher missing: ops/edm");
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
