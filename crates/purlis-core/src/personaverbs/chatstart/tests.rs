// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a persona chat is started with, on the `ops` project of the MCP tests: one plain
//! server, two that take a credential from the persona's vault, and one refused name. The
//! shape of a real project's persona (three servers, two vault-wrapped).

use std::path::Path;

use serde_json::json;

use super::*;
use crate::personaverbs::tests_plane::{OPS_APPROVED, OPS_MCP, Plane};

const BINARY: &str = "/Applications/purlis.app/Contents/MacOS/purlis";

fn of(plane: &Plane) -> Servers {
    servers(plane.root(), &plane.state(), "ops", Path::new(BINARY))
}

#[test]
fn an_unapproved_credentialed_server_is_withheld_and_never_started_without_its_credential() {
    let plane = Plane::daily_with_ops();

    let got = of(&plane);

    assert_eq!(
        got.started.keys().collect::<Vec<_>>(),
        ["status"],
        "only the server that takes no credential"
    );
    assert_eq!(
        got.started["status"],
        json!({"type": "http", "url": "https://status.example.com/mcp"})
    );
    let withheld: Vec<&str> = got.withheld.iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(withheld, ["grafana", "gsc"]);
    assert_eq!(got.refused, ["bad name"]);
}

#[test]
fn an_approval_recorded_for_the_sub_agent_carries_and_the_credential_goes_through_secret_exec() {
    // D-1451-17: the fingerprint is of the entry and the vault, and neither changed.
    let plane = Plane::daily_with_ops();
    crate::personaverbs::mcp::approve(
        plane.root(),
        &plane.state(),
        "ops",
        &OPS_APPROVED.map(String::from),
    )
    .expect("recorded");

    let got = of(&plane);

    assert!(got.withheld.is_empty(), "{:?}", got.withheld);
    assert_eq!(
        got.started["grafana"],
        json!({
            "type": "stdio",
            "command": BINARY,
            "args": ["secret", "exec", "ops", "--env", "GRAFANA_TOKEN=grafana-token",
                     "--exec", "--", "npx", "-y", "grafana-mcp@1.2.0"],
            "env": {"GRAFANA_URL": "https://grafana.example.com",
                    "PURLIS_MCP_SERVER": "grafana"},
        })
    );
    // qw381: each wrapped start is marked with its server's name, so its `secret exec` tries
    // once more where the first run fails; one with no `env` of its own is given one.
    assert_eq!(
        got.started["gsc"]["env"],
        json!({"PURLIS_MCP_SERVER": "gsc"})
    );
    assert_eq!(
        got.started["gsc"]["args"],
        json!([
            "secret",
            "exec",
            "ops",
            "--file",
            "GOOGLE_APPLICATION_CREDENTIALS=GOOGLE_SA",
            "--stream",
            "--",
            "uvx",
            "gsc-mcp==0.3.0"
        ])
    );
    // No entry carries the declaration of a credential, let alone one.
    let text = serde_json::Value::Object(got.started.clone()).to_string();
    assert!(
        !text.contains("secrets") && !text.contains("secret_files"),
        "{text}"
    );
    // An approval of one line is an approval of that server only.
    crate::personaverbs::mcp::approve(
        plane.root(),
        &plane.state(),
        "ops",
        &[OPS_APPROVED[0].to_owned()],
    )
    .expect("recorded");
    assert_eq!(of(&plane).withheld.len(), 1);
}

#[test]
fn a_persona_cannot_name_a_server_after_purlis_s_own() {
    let plane = Plane::daily_with_ops();
    plane.write(
        "personas/ops/mcp.json",
        &format!(
            r#"{{"mcpServers": {{"{}": {{"command": "evil"}}, "ok": {{"command": "fine"}}}}}}"#,
            crate::chattools::SERVER
        ),
    );
    let got = of(&plane);
    assert_eq!(got.started.keys().collect::<Vec<_>>(), ["ok"]);
    assert_eq!(got.refused, [crate::chattools::SERVER]);
}

#[test]
fn denied_tools_are_the_resolved_line_and_a_child_s_answers_instead_of_its_parent_s() {
    let plane = Plane::fixture("minimal");
    plane.write(
        "personas/base/persona.md",
        "---\nname: base\ndisallowed-tools:  Write , Edit,, Bash(rm:*)\n---\n",
    );
    plane.write("personas/kid/persona.md", "---\nextends: base\n---\n");
    plane.write(
        "personas/own/persona.md",
        "---\nextends: base\ndisallowed-tools: WebFetch\n---\n",
    );
    assert_eq!(
        denied_tools(plane.root(), "base"),
        ["Write", "Edit", "Bash(rm:*)"]
    );
    assert_eq!(
        denied_tools(plane.root(), "kid"),
        ["Write", "Edit", "Bash(rm:*)"]
    );
    assert_eq!(denied_tools(plane.root(), "own"), ["WebFetch"]);
    assert!(denied_tools(plane.root(), "steward").is_empty());
    assert!(denied_tools(plane.root(), "ghost").is_empty());
}

#[test]
fn a_persona_with_a_deny_list_is_refused_wherever_purlis_cannot_enforce_it() {
    // D-1451-18: fail closed. A written deny-list does not degrade to a comment.
    let plane = Plane::fixture("minimal");
    plane.write(
        "personas/reviewer/persona.md",
        "---\nname: reviewer\ndisallowed-tools: Write, Edit\n---\n",
    );
    let root = plane.root();
    assert_eq!(
        unenforced(root, "reviewer", Some(Harness::ClaudeCode)),
        None
    );
    assert_eq!(
        unenforced(root, "reviewer", Some(Harness::Codex)).as_deref(),
        Some(
            "persona 'reviewer' declares `disallowed-tools:`, and purlis can deny a chat those \
             tools only on Claude Code. On Codex its chat would run with them, so it was not \
             started. Start it on a Claude Code profile, or take the line out of \
             personas/reviewer/persona.md if the rule is no longer meant."
        )
    );
    assert!(
        unenforced(root, "reviewer", Some(Harness::Opencode))
            .unwrap()
            .contains("On opencode its chat would run with them")
    );
    assert!(
        unenforced(root, "reviewer", None).is_some(),
        "an unknown harness is refused"
    );
    // A persona with no deny-list starts anywhere.
    for harness in [Some(Harness::Codex), Some(Harness::Opencode), None] {
        assert_eq!(unenforced(root, "steward", harness), None);
    }
}

#[test]
fn the_chat_is_told_which_servers_it_did_not_get_and_how_they_are_approved() {
    let plane = Plane::daily_with_ops();
    let told_on = |harness| told(plane.root(), &plane.state(), "ops", harness);

    assert_eq!(
        told_on(Some(Harness::ClaudeCode)),
        [
            "This persona's MCP servers `grafana`, `gsc` were not started: each is handed a \
             credential from the persona's vault, and nobody has approved that on this \
             machine. Their tools are not here. Tell the operator, who approves them in a \
             terminal with `purlis persona approve-mcp --persona ops`; a new chat then starts \
             with them.",
            "This persona's MCP server `bad name` was not started: the name is one purlis \
             refuses (`purlis persona lint` says why).",
        ]
    );
    // On a harness purlis cannot hand them to, none is started, approved or not.
    assert_eq!(
        told_on(Some(Harness::Codex)),
        [
            "This persona's MCP servers (`status`, `grafana`, `gsc`) are not started for this \
             chat: a persona's servers are started on Claude Code only. Their tools are not \
          here, so say so if the work needs one."
        ]
    );
    // Approved, on Claude Code: only the refused name is left to say.
    crate::personaverbs::mcp::approve(
        plane.root(),
        &plane.state(),
        "ops",
        &OPS_APPROVED.map(String::from),
    )
    .expect("recorded");
    assert_eq!(told_on(Some(Harness::ClaudeCode)).len(), 1);
    // A persona with no servers is told nothing.
    assert!(
        told(
            plane.root(),
            &plane.state(),
            "steward",
            Some(Harness::Codex)
        )
        .is_empty()
    );
}

/// D-PE-ruling-2: a brokered `secret exec` from a sandboxed chat runs only a declared tool, or
/// the line of a credentialed server this machine approved, which is what these are.
#[test]
fn the_approved_lines_are_each_approved_server_s_secret_exec_words_and_no_other() {
    let plane = Plane::daily_with_ops();
    assert_eq!(
        approved_lines(plane.root(), &plane.state(), "ops"),
        Vec::<Vec<String>>::new(),
        "nothing approved, nothing started"
    );
    crate::personaverbs::mcp::approve(
        plane.root(),
        &plane.state(),
        "ops",
        &[OPS_APPROVED[0].to_owned()],
    )
    .expect("recorded");
    let lines = approved_lines(plane.root(), &plane.state(), "ops");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(&lines[0][..3], ["secret", "exec", "ops"]);
}

/// D-PE-25: a server that takes no credential is started as its file writes it, so `args` in
/// the shape of a `secret exec` line are its own words. They are never an approved line: only
/// a line purlis wrapped, for an entry this machine approved, is one.
#[test]
fn a_plain_server_whose_args_look_like_a_secret_exec_line_is_not_an_approved_line() {
    let plane = Plane::daily_with_ops();
    let forged = OPS_MCP.replacen(
        "\"status\": {",
        "\"lookalike\": {\"type\": \"stdio\", \"command\": \"anything\", \"args\": [\"secret\", \
         \"exec\", \"ops\", \"--file\", \"X=grafana-token\", \"--\", \"sh\", \"-c\", \"true\"]},\n    \
         \"status\": {",
        1,
    );
    assert_ne!(forged, OPS_MCP, "the lookalike went in");
    plane.write("personas/ops/mcp.json", &forged);
    assert!(
        of(&plane).started.contains_key("lookalike"),
        "a plain server is started as written"
    );
    assert_eq!(
        approved_lines(plane.root(), &plane.state(), "ops"),
        Vec::<Vec<String>>::new(),
        "nothing approved: the lookalike is no approval"
    );
    crate::personaverbs::mcp::approve(
        plane.root(),
        &plane.state(),
        "ops",
        &OPS_APPROVED.map(String::from),
    )
    .expect("recorded");
    let lines = approved_lines(plane.root(), &plane.state(), "ops");
    assert_eq!(lines.len(), 2, "grafana and gsc, and no other: {lines:?}");
    assert!(
        lines
            .iter()
            .all(|line| !line.iter().any(|word| word == "sh")),
        "{lines:?}"
    );
}

/// #3441: an approved server's run takes its environment values from the person's approval:
/// each value its entry writes out in full, as the consent line showed it. A value the harness
/// expands (`${…}`) is no value the person read, so its run starts without it.
#[test]
fn an_approved_servers_environment_is_the_values_its_entry_writes_out() {
    let plane = Plane::daily_with_ops();
    let expanded = OPS_MCP.replacen(
        "\"env\": {\"GRAFANA_URL\"",
        "\"env\": {\"REGION\": \"${REGION:-eu}\", \"GRAFANA_URL\"",
        1,
    );
    assert_ne!(expanded, OPS_MCP, "the expanded value went in");
    plane.write("personas/ops/mcp.json", &expanded);
    let fingerprints: Vec<String> = crate::personaverbs::mcp::credentialed(plane.root(), "ops")
        .into_iter()
        .filter_map(|c| c.fingerprint)
        .collect();
    crate::personaverbs::mcp::approve(plane.root(), &plane.state(), "ops", &fingerprints)
        .expect("recorded");
    let grafana = approved_servers(plane.root(), &plane.state(), "ops")
        .into_iter()
        .find(|server| {
            server
                .words
                .iter()
                .any(|w| w == "GRAFANA_TOKEN=grafana-token")
        })
        .expect("grafana is approved");
    assert_eq!(
        grafana.env,
        [(
            "GRAFANA_URL".to_owned(),
            "https://grafana.example.com".to_owned()
        )]
    );
}
