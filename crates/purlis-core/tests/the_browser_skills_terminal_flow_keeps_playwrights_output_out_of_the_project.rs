// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The browser skill's terminal flow keeps Playwright's output out of the project** (#2686).
//!
//! `playwright-cli` writes its snapshots, logs and traces under `.playwright-cli/` in the folder
//! it runs from, and a trace of a logged-in run holds that run's cookies. A project's autosave
//! commits files it does not ignore. So the flow the `browser` skill shows points Playwright's
//! output folder (`PLAYWRIGHT_MCP_OUTPUT_DIR`, read when a session opens, as the secrets file
//! is) at a private folder of its own outside the project before the session opens, refuses to
//! go on where that folder could not be made, and removes it with the session when the flow
//! exits, however it exits.

use std::path::Path;

fn browser_skill() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../app/src-tauri/plugin/skills/browser/SKILL.md");
    std::fs::read_to_string(&path).unwrap_or_else(|why| panic!("{}: {why}", path.display()))
}

/// The skill's one `bash` block: the terminal flow.
fn terminal_flow() -> String {
    let skill = browser_skill();
    let blocks: Vec<&str> = skill
        .split("```bash\n")
        .skip(1)
        .filter_map(|rest| rest.split("\n```").next())
        .collect();
    assert_eq!(blocks.len(), 1, "the browser skill shows one bash flow");
    blocks[0].to_owned()
}

/// The first line of `flow` holding `what`, by number.
fn line_of(flow: &str, what: &str) -> usize {
    flow.lines()
        .position(|line| line.contains(what))
        .unwrap_or_else(|| panic!("the flow has no line with {what}:\n{flow}"))
}

#[test]
fn the_terminal_flow_points_playwrights_output_outside_the_project_before_it_opens() {
    purlis_core::unsteered!();
    let flow = terminal_flow();
    let made = line_of(&flow, "OUT=$(mktemp -d)");
    assert!(
        flow.lines().nth(made).unwrap().contains("|| exit 1"),
        "the flow goes on where its output folder could not be made:\n{flow}"
    );
    let pointed = line_of(&flow, "export PLAYWRIGHT_MCP_OUTPUT_DIR=\"$OUT\"");
    let opened = line_of(&flow, "$P open");
    assert!(
        made < pointed && pointed < opened,
        "the output folder is made and pointed at before the session opens:\n{flow}"
    );
}

#[test]
fn the_terminal_flow_removes_its_output_with_its_session_however_it_ends() {
    purlis_core::unsteered!();
    let flow = terminal_flow();
    let made = line_of(&flow, "OUT=$(mktemp -d)");
    let cleanup = line_of(&flow, "cleanup() {");
    let line = flow.lines().nth(cleanup).unwrap();
    let (closes, removes) = (
        line.find("$P close").unwrap_or(usize::MAX),
        line.find("rm -rf -- \"$OUT\"").unwrap_or(usize::MAX),
    );
    assert!(
        closes < removes && removes != usize::MAX,
        "the cleanup closes the session, then removes its output folder:\n{flow}"
    );
    let trapped = line_of(&flow, "trap cleanup EXIT");
    assert!(
        cleanup < trapped,
        "the cleanup runs when the flow exits, however it exits:\n{flow}"
    );
    assert!(
        made < trapped && trapped < line_of(&flow, "$P open"),
        "the cleanup is set once the folder is made and before the session opens:\n{flow}"
    );
    assert!(
        browser_skill().contains("`.playwright-cli/`"),
        "the skill says where Playwright writes by default"
    );
}
