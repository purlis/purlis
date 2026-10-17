// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter persona use` against the recorded scenarios' `daily` plane
//! (`persona-use-…` in `tests/fixtures/recorded/behaviour.jsonl`): the line said and the
//! pointer written for each set of ids.

use super::*;
use crate::personaverbs::tests_plane::{Heard, OPS, OPS_MCP, Plane};

fn ids(session: Option<&str>, terminal: Option<&str>) -> Ids {
    Ids {
        session: session.map(String::from),
        terminal: terminal.map(String::from),
    }
}

fn now() -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2026, 5, 4)
        .unwrap()
        .and_hms_opt(11, 32, 17)
        .unwrap()
}

fn run(plane: &Plane, name: &str, ids: &Ids, env: Option<&str>) -> (u8, Heard) {
    run_where(plane, name, ids, env, false)
}

fn run_where(
    plane: &Plane,
    name: &str,
    ids: &Ids,
    env: Option<&str>,
    in_chat: bool,
) -> (u8, Heard) {
    let bucket = ids.session.clone().unwrap_or_else(|| "nosession".into());
    let asking = Asking {
        ids,
        env_persona: env,
        in_chat,
        bucket: &bucket,
        now: now(),
    };
    let mut heard = Heard::default();
    let rc = use_persona(plane.root(), name, &asking, &mut heard.sink());
    (rc, heard)
}

#[test]
fn inside_a_chat_it_is_refused_writes_nothing_and_names_both_ways_forward() {
    // persona-use-inside-a-chat-says-its-persona-is-fixed (#1435)
    let plane = Plane::fixture("daily");
    let ids = ids(Some("s-1"), Some("pane-1"));
    let (rc, heard) = run_where(&plane, "devops", &ids, Some("steward"), true);
    assert_eq!((rc, heard.out.as_str()), (1, ""));
    assert_eq!(
        heard.err,
        concat!(
            "✗ This chat runs as 'steward', and a chat's persona is fixed for its life: ",
            "nothing was changed, and commands here still use 'steward', with the vaults of ",
            "'steward'.\n",
            "• To use a vault of 'devops' here, run the command that needs it and ask the ",
            "operator to press Allow in the notice on this chat's tab. To have 'devops' do ",
            "the work, dispatch to it: `purlis dispatch --to devops`.\n",
        )
    );
    assert!(!plane.path(".charter/sessions/s-1.persona").exists());
    assert!(!plane.path(".charter/terminals").exists());
    assert!(!plane.path(".charter/active-persona").exists());
    assert!(
        !plane
            .path(".charter/persona-state/trace/s-1.jsonl")
            .exists()
    );
    // A chat with no persona is refused one too, in its own words.
    let (rc, heard) = run_where(&plane, "devops", &ids, None, true);
    assert_eq!(rc, 1);
    assert!(
        heard.err.starts_with(
            "✗ This chat runs with no persona, and a chat's persona is fixed for its life: \
             nothing was changed, and commands here still have none.\n"
        ),
        "{}",
        heard.err
    );
    assert!(!plane.path(".charter/sessions").exists());
    // A name the project does not define is refused as it is anywhere.
    let (rc, heard) = run_where(&plane, "ghost", &ids, Some("steward"), true);
    assert_eq!(rc, 1);
    assert_eq!(
        heard.err,
        "✗ no persona 'ghost' (create it: purlis persona create ghost)\n"
    );
}

#[test]
fn a_chat_naming_the_persona_it_runs_as_is_told_so_and_nothing_is_written() {
    let plane = Plane::fixture("daily");
    let ids = ids(Some("s-1"), None);
    let (rc, heard) = run_where(&plane, "devops", &ids, Some(" devops "), true);
    assert_eq!(rc, 0);
    assert_eq!(
        heard.err,
        "• This chat already runs as 'devops', and a chat's persona is fixed for its life: \
         there is nothing to select.\n"
    );
    assert!(!plane.path(".charter/sessions").exists());
}

const TRACED: &str = "{\"ts\": \"2026-05-04T11:32:17\", \"event\": \"persona-use\", \
                      \"persona\": \"devops\"}\n";

#[test]
fn with_a_session_and_no_pane_the_selection_is_this_sessions_and_names_the_next_sessions_start() {
    // persona-use-selects-for-this-session-when-there-is-no-pane-id
    let plane = Plane::fixture("daily");
    let (rc, heard) = run(&plane, "devops", &ids(Some("s-1"), None), None);
    assert_eq!((rc, heard.out.as_str()), (0, ""));
    assert_eq!(
        heard.err,
        "✓ Active persona set to 'devops' for this session only — this terminal reports no \
         pane id, so a new session starts as 'steward'.\n"
    );
    assert_eq!(plane.read(".charter/sessions/s-1.persona"), "devops\n");
    assert!(!plane.path(".charter/active-persona").exists());
    assert!(!plane.path(".charter/terminals").exists());
    assert_eq!(plane.read(".charter/persona-state/trace/s-1.jsonl"), TRACED);
}

#[test]
fn a_session_selection_on_a_plane_with_no_default_says_the_next_starts_with_none() {
    let plane = Plane::fixture("daily");
    plane.write("charter.toml", "schema = 1\n");
    let (_, heard) = run(&plane, "devops", &ids(Some("s-1"), None), None);
    assert_eq!(
        heard.err,
        "✓ Active persona set to 'devops' for this session only — this terminal reports no \
         pane id, so a new session starts with no persona.\n"
    );
}

#[test]
fn with_a_pane_the_selection_is_this_terminals_and_this_sessions_too() {
    // persona-use-with-a-pane-id-selects-for-this-terminal
    let plane = Plane::fixture("daily");
    let (rc, heard) = run(&plane, "devops", &ids(Some("s-1"), Some("pane-1")), None);
    assert_eq!(rc, 0);
    assert_eq!(
        heard.err,
        "✓ Active persona set to 'devops' for this terminal (kept across closing/reopening \
         Claude).\n"
    );
    assert_eq!(plane.read(".charter/terminals/pane-1.persona"), "devops\n");
    assert_eq!(plane.read(".charter/sessions/s-1.persona"), "devops\n");
    assert!(!plane.path(".charter/active-persona").exists());
    assert_eq!(
        set_active(plane.root(), "steward", &ids(None, Some("pane-1"))),
        Scope::Terminal
    );
    assert_eq!(plane.read(".charter/terminals/pane-1.persona"), "steward\n");
    assert!(!plane.path(".charter/active-persona").exists());
}

#[test]
fn with_neither_id_the_selection_is_the_plane_wide_file() {
    // persona-use-with-no-ids-writes-the-plane-wide-file
    let plane = Plane::fixture("daily");
    let (rc, heard) = run(&plane, "devops", &ids(None, None), None);
    assert_eq!(rc, 0);
    assert_eq!(
        heard.err,
        "✓ Active persona set to 'devops' for this control plane (no session or pane id to \
         scope it to).\n"
    );
    assert_eq!(plane.read(".charter/active-persona"), "devops\n");
    assert_eq!(
        plane.read(".charter/persona-state/trace/nosession.jsonl"),
        TRACED
    );
}

#[test]
fn an_id_that_is_empty_or_not_one_path_segment_is_no_id_at_all() {
    assert_eq!(usable(Some("s-1")), Some("s-1"));
    for bad in ["", "a/b", "..", "."] {
        assert_eq!(usable(Some(bad)), None, "{bad:?}");
    }
    assert_eq!(usable(None), None);
    let plane = Plane::fixture("daily");
    assert_eq!(
        set_active(plane.root(), "devops", &ids(Some("../x"), Some(""))),
        Scope::Plane
    );
    assert_eq!(plane.read(".charter/active-persona"), "devops\n");
    assert!(!plane.path(".charter/x.persona").exists());
}

#[test]
fn charter_persona_set_to_another_name_is_said_to_outrank_the_selection() {
    // persona-use-says-the-environment-still-outranks-it
    let plane = Plane::fixture("daily");
    let (rc, heard) = run(&plane, "devops", &ids(Some("s-1"), None), Some("steward"));
    assert_eq!(rc, 0);
    assert!(
        heard.err.ends_with(
            "! $PURLIS_PERSONA or $CHARTER_PERSONA is set to 'steward' and takes \
             precedence — commands use \
             'steward', not 'devops'.\n"
        ),
        "{}",
        heard.err
    );
    for same in [Some("devops"), Some(" devops "), Some("  "), None] {
        let (_, heard) = run(&plane, "devops", &ids(Some("s-1"), None), same);
        assert!(!heard.err.contains("_PERSONA"), "{same:?}");
    }
}

#[test]
fn a_name_outside_the_alphabet_or_not_defined_is_refused_and_nothing_is_written() {
    // persona-use-refuses-a-name-outside-the-alphabet, …-a-persona-the-plane-does-not-define
    let plane = Plane::fixture("daily");
    let (rc, heard) = run(&plane, "Bad", &ids(Some("s-1"), None), None);
    assert_eq!(rc, 1);
    assert_eq!(
        heard.err,
        "✗ invalid persona name 'Bad' (lowercase letters, digits, '.', '_', '-')\n"
    );
    let (rc, heard) = run(&plane, "ghost", &ids(Some("s-1"), None), None);
    assert_eq!(rc, 1);
    assert_eq!(
        heard.err,
        "✗ no persona 'ghost' (create it: purlis persona create ghost)\n"
    );
    assert!(!plane.path(".charter/sessions/s-1.persona").exists());
}

#[test]
fn tools_declared_since_the_ceiling_froze_and_dispatch_scoped_servers_are_named() {
    // persona-use-names-new-tools-and-dispatch-scoped-mcp-servers
    let plane = Plane::fixture("daily");
    plane.write("personas/ops/persona.md", OPS);
    plane.write("personas/ops/mcp.json", OPS_MCP);
    plane.write(".charter/sessions/s-1.gate", "");
    plane.write(".charter/sessions/s-1.tools", "{\"ops\": [\"gh\"]}");
    let (rc, heard) = run(&plane, "ops", &ids(Some("s-1"), None), None);
    assert_eq!(rc, 0);
    assert_eq!(
        heard.err,
        "✓ Active persona set to 'ops' for this session only — this terminal reports no \
         pane id, so a new session starts as 'steward'.\n\
         •   1 tool declared since this session started: kubectl.\n\
         •   Those still prompt HERE. The tool-gate answers within the set that existed at \
         session start — `tools:` is read from a file this session can write, and freezing \
         it is what stops an edit from becoming an unprompted command. A new session picks \
         them up.\n\
         •   3 MCP servers declared: grafana, gsc, status.\n\
         •   They are NOT started for the session you are in, and servers already live here \
         (from .mcp.json or an enabled plugin) stay live. A chat started as this persona \
         is started with them, on Claude Code, once a person has approved the ones that \
         take a credential (`purlis persona approve-mcp`).\n"
    );
    // Nothing new since the freeze, and no servers: neither note.
    plane.write(
        ".charter/sessions/s-1.tools",
        "{\"ops\": [\"gh\", \"kubectl\"]}",
    );
    std::fs::remove_file(plane.path("personas/ops/mcp.json")).unwrap();
    let (_, heard) = run(&plane, "ops", &ids(Some("s-1"), None), None);
    assert_eq!(heard.err.lines().count(), 1, "{}", heard.err);
}
