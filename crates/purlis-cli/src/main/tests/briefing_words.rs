// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What every chat is told holds to the settled words** (#5722, #5723; spec #5714's Q28 and
//! Q31): the briefing, every purlis tool's description and arguments, the MCP server's
//! instructions and `ask_operator`'s own answers say no losing side of a settled pair and none
//! of the words the glossary avoids. The shipped skills are held to the same lists by the
//! shipped-skill check (`tests/it/every_shipped_skill_passes_the_built_in_checks.rs`); these
//! texts are here because only the binary reaches the MCP server's.

#[path = "../../../tests/shared/settled_words.rs"]
mod settled_words;

use settled_words::{avoided, losing_sides, prose_paragraphs};

/// The settled pairs hold beyond the skills: in what every chat is told at its start, and in
/// every purlis tool's description and arguments, a chat's other two sources of purlis's words.
#[test]
fn the_briefing_and_every_tool_description_say_no_losing_side() {
    use purlis_core::briefing;
    let mut said: Vec<(String, String)> = [
        ("the sandboxed note", briefing::SANDBOXED_NOTE),
        ("the unsandboxed note", briefing::UNSANDBOXED_NOTE),
        (
            "the branch-folder note",
            briefing::BRANCH_FOLDER_COMMIT_NOTE,
        ),
        ("the one route", purlis_core::handoff::ONE_ROUTE),
        ("the one route's tools", briefing::ONE_ROUTE_TOOLS),
        ("the one route's task", briefing::ONE_ROUTE_IS_A_TASK),
        ("the one route's limit", briefing::ONE_ROUTE_ONLY_THEN),
    ]
    .into_iter()
    .map(|(what, text)| (what.to_owned(), text.to_owned()))
    .collect();
    for tool in crate::mcp::every_tool() {
        said.push((
            format!("the `{}` tool", tool.name),
            format!("{} {}", tool.description, (tool.schema)()),
        ));
    }
    let wrong: Vec<String> = said
        .iter()
        .flat_map(|(what, text)| {
            losing_sides(text)
                .into_iter()
                .map(move |ruling| format!("{what} says the losing side, against {ruling}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// What a chat is told at its start in the cases whose words are purlis's own: the gate, the
/// plane root, a record that cannot be resumed, a persona that is gone.
fn briefings() -> Vec<(String, String)> {
    use purlis_core::{active, briefing, sessionrecord};
    let dir = tempfile::tempdir().expect("a folder");
    let root = std::fs::canonicalize(dir.path()).expect("a real path");
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    for ws in ["alpha", "beta"] {
        std::fs::create_dir_all(root.join("workspaces").join(ws)).expect("a workspace");
        std::fs::write(
            root.join("workspaces").join(ws).join("workspace.md"),
            format!("# {ws}\n\n## Vision\n\nShip {ws}.\n"),
        )
        .expect("workspace.md");
    }
    let outside = root.parent().expect("a parent").to_path_buf();
    /// What is asked, from where, with which variables and in which permission mode.
    type Case<'a> = (
        &'a str,
        &'a std::path::Path,
        &'a [(&'a str, &'a str)],
        &'a str,
    );
    let cases: [Case; 6] = [
        ("the gate", &outside, &[], "default"),
        ("the unattended gate", &outside, &[], "bypassPermissions"),
        (
            "a chat launched at the plane root",
            &outside,
            &[(active::PLANE_ROOT_ENV, "1")],
            "default",
        ),
        ("a chat standing at the plane root", &root, &[], "default"),
        (
            "a record that cannot be resumed",
            &outside,
            &[(sessionrecord::RESUMING_ENV, "nowhere.md")],
            "default",
        ),
        (
            "a persona that is gone",
            &outside,
            &[(active::PERSONA_ENV, "ghost")],
            "default",
        ),
    ];
    cases
        .into_iter()
        .map(|(what, cwd, env, mode)| {
            let lookup = |name: &str| {
                env.iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            };
            let payload = serde_json::json!({ "permission_mode": mode });
            let ask = briefing::Ask {
                root: &root,
                cwd,
                payload: &payload,
                env: &lookup,
                now: chrono::Utc::now(),
                attendance: purlis_core::dispatchunattended::Attendance::Attended,
            };
            (
                format!("the briefing of {what}"),
                briefing::parts(&ask, None).join("\n\n"),
            )
        })
        .collect()
}

/// The glossary's words hold beyond the skills: in what every chat is told at its start, and in
/// every purlis tool's description and arguments.
#[test]
fn the_briefing_and_every_tool_description_use_the_glossary_s_words() {
    use purlis_core::briefing;
    let mut said = briefings();
    assert!(
        said.iter()
            .any(|(_, text)| text.contains("Confirm the workspace")),
        "the gate was not rendered: {said:?}"
    );
    for (what, text) in [
        ("the sandboxed note", briefing::SANDBOXED_NOTE),
        ("the unsandboxed note", briefing::UNSANDBOXED_NOTE),
        (
            "the branch-folder note",
            briefing::BRANCH_FOLDER_COMMIT_NOTE,
        ),
        ("the one route", purlis_core::handoff::ONE_ROUTE),
        ("the one route's tools", briefing::ONE_ROUTE_TOOLS),
        ("the one route's task", briefing::ONE_ROUTE_IS_A_TASK),
        ("the one route's limit", briefing::ONE_ROUTE_ONLY_THEN),
    ] {
        said.push((what.to_owned(), text.to_owned()));
    }
    for tool in crate::mcp::every_tool() {
        said.push((
            format!("the `{}` tool", tool.name),
            format!("{} {}", tool.description, (tool.schema)()),
        ));
    }
    // What the MCP server tells the harness it is for, and what its tools return in purlis's
    // own words.
    said.push((
        "the MCP server's instructions".to_owned(),
        crate::mcp::INSTRUCTIONS.to_owned(),
    ));
    for text in crate::mcp::ASK_RESULTS {
        said.push(("an `ask_operator` result".to_owned(), text.to_owned()));
    }
    let unasked =
        purlis_core::chattools::question(&serde_json::Map::new()).expect_err("no question");
    let blank = purlis_core::chattools::question(
        serde_json::json!({ "question": " " })
            .as_object()
            .expect("an object"),
    )
    .expect_err("a blank question");
    for text in [unasked, blank] {
        said.push(("an `ask_operator` result".to_owned(), text));
    }
    let wrong: Vec<String> = said
        .iter()
        .flat_map(|(what, text)| {
            prose_paragraphs(text)
                .into_iter()
                .flat_map(|(_, prose)| avoided(&prose))
                .map(move |(word, instead)| format!("{what} says “{word}”, not {instead}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
