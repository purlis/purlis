// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The `queries` capability through the real registry and executor** (ADR 0120 §9, #5971):
//! the probe installed as `query-probe`, in protocol 7, is declared, fingerprinted, approved,
//! answered — from its program or its facts file — and refused when it is asked wrongly.

#![cfg(unix)]

use std::path::PathBuf;
use std::time::Duration;

use purlis_core::catalogue::{Answer, ErrorKind};
use purlis_core::executor::Executor;
use purlis_core::extension::{self, queries};
use serde_json::{Value, json};

/// As `through_the_executor.rs`'s: macOS assesses a freshly copied program the first time it
/// runs, which can queue past the real deadline on a loaded machine.
const PATIENT: Duration = Duration::from_secs(30);

struct Probe {
    dir: tempfile::TempDir,
}

impl Probe {
    fn assembled() -> Self {
        let dir = tempfile::tempdir().expect("a directory");
        let plane = dir.path().join("plane");
        std::fs::create_dir_all(plane.join("workspaces/alpha")).expect("a workspace");
        std::fs::write(plane.join("charter.toml"), "").expect("a manifest");
        let init = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&plane)
            .status()
            .expect("git runs");
        assert!(init.success(), "git init failed: {init}");
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_extension-probe"))
            .arg("assemble-queries")
            .arg(dir.path().join("ext"))
            .status()
            .expect("assemble runs");
        assert!(status.success(), "assemble failed: {status}");
        Self { dir }
    }

    fn approved() -> Self {
        let probe = Self::assembled();
        let found = extension::install(&probe.config(), &extension::BuiltIn::none(), &probe.ext())
            .expect("installed");
        extension::approve(&probe.config(), found.id(), &found.path, &found.fingerprint)
            .expect("approved");
        probe
    }

    fn config(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    fn ext(&self) -> PathBuf {
        self.dir.path().join("ext")
    }

    fn plane(&self) -> PathBuf {
        self.dir.path().join("plane")
    }

    fn state(&self) -> PathBuf {
        self.ext().join("state")
    }

    fn behaves(&self, how: Value) {
        std::fs::create_dir_all(self.state()).expect("state");
        std::fs::write(self.state().join(extension_probe::BEHAVE), how.to_string())
            .expect("written");
    }

    /// How many questions the program answered.
    fn queried(&self) -> usize {
        std::fs::read_to_string(self.state().join(extension_probe::QUERIED))
            .map_or(0, |text| text.lines().count())
    }

    fn ask_with(&self, executor: &Executor, id: &str, params: Value, plane: bool) -> Answer {
        queries::ask(
            executor,
            &self.config(),
            &extension::project::Choices::read_in(&self.plane(), Some("alpha")),
            &queries::Asked {
                id,
                workspace: "alpha",
                params: &params,
                plane,
            },
            chrono::Utc::now(),
        )
    }

    fn ask(&self, id: &str, params: Value) -> Answer {
        self.ask_with(
            &Executor::default().with_deadline(PATIENT),
            id,
            params,
            false,
        )
    }
}

fn data(answer: &Answer) -> &Value {
    answer
        .outcome
        .as_ref()
        .unwrap_or_else(|kind| panic!("no data: {}", kind.as_str()))
}

#[test]
fn the_probe_declares_its_queries_and_the_approval_prompt_names_each() {
    let probe = Probe::assembled();
    let found = extension::read_at(&probe.ext()).expect("read");
    assert_eq!(
        found.manifest.capabilities,
        [extension::Capability::Queries]
    );
    assert_eq!(found.manifest.protocol, 7);
    let prompt = extension::prompt(&found, extension::Standing::New);
    let said = prompt.declares.join("\n");
    assert!(said.contains("the capability “queries”"), "{said}");
    assert!(
        said.contains(
            "a query, “query-probe.asked” — How many questions the probe answered, and what it \
             was handed.; reads one workspace, and an answer is good for 60s; “who” widens it to \
             anyone's questions, not only the workspace's"
        ),
        "{said}"
    );
    assert!(
        said.contains("“query-probe.everywhere”")
            && said.contains("reads the whole project, a grant of its own on each page"),
        "{said}"
    );
    assert!(
        said.contains("when an artifact's page reads one of its queries"),
        "{said}"
    );
}

#[test]
fn an_approved_probe_answers_its_query_in_protocol_7_with_what_it_was_handed() {
    let probe = Probe::approved();
    let answer = probe.ask("query-probe.asked", json!({ "who": "ana" }));
    assert_eq!(
        data(&answer),
        &json!({
            "asked": 1,
            "workspace": "alpha",
            "params": { "who": "ana" },
            "protocol": 7,
        })
    );
    assert_eq!(answer.cursor, None);
}

#[test]
fn an_answer_in_the_facts_file_is_used_while_fresh_and_the_program_is_not_started() {
    let probe = Probe::approved();
    let first = probe.ask("query-probe.asked", json!({}));
    assert_eq!(data(&first)["asked"], 1);
    assert_eq!(probe.queried(), 1);
    // Fresh in its facts file, under the workspace's key: no program starts.
    let second = probe.ask("query-probe.asked", Value::Null);
    assert_eq!(data(&second)["asked"], 1);
    assert_eq!(probe.queried(), 1, "the program was started again");
    // With a parameter, the facts file cannot hold the answer: the program is asked.
    probe.ask("query-probe.asked", json!({ "who": "ana" }));
    assert_eq!(probe.queried(), 2);
    // Past its freshness, the program is asked again.
    let facts = probe.state().join(extension_probe::FACTS);
    let mut doc: Value =
        serde_json::from_str(&std::fs::read_to_string(&facts).expect("facts")).expect("JSON");
    doc["workspaces"]["alpha"]["queries"]["asked"]["at"] =
        json!(chrono::Utc::now().timestamp() - 61);
    std::fs::write(&facts, doc.to_string()).expect("written");
    let stale = probe.ask("query-probe.asked", json!({}));
    assert_eq!(data(&stale)["asked"], 3);
}

#[test]
fn another_workspace_s_answer_in_the_facts_file_is_not_this_one_s() {
    let probe = Probe::approved();
    probe.ask("query-probe.asked", json!({}));
    let facts = probe.state().join(extension_probe::FACTS);
    let doc: Value =
        serde_json::from_str(&std::fs::read_to_string(&facts).expect("facts")).expect("JSON");
    let moved = json!({ "workspaces": { "beta": doc["workspaces"]["alpha"].clone() } });
    std::fs::write(&facts, moved.to_string()).expect("written");
    probe.ask("query-probe.asked", json!({}));
    assert_eq!(probe.queried(), 2);
}

#[test]
fn a_plane_scoped_query_answers_only_with_the_plane_grant() {
    let probe = Probe::approved();
    let executor = Executor::default().with_deadline(PATIENT);
    let refused = probe.ask_with(&executor, "query-probe.everywhere", json!({}), false);
    assert_eq!(refused.outcome, Err(ErrorKind::NotApproved));
    assert_eq!(probe.queried(), 0);
    let granted = probe.ask_with(&executor, "query-probe.everywhere", json!({}), true);
    assert_eq!(data(&granted), &json!({ "plane": true, "asked": 1 }));
}

#[test]
fn a_viewer_without_the_extension_or_its_query_or_with_it_off_is_answered_needs_extension() {
    let needs = Err(ErrorKind::NeedsExtension);
    // Not installed at all.
    let bare = Probe::assembled();
    assert_eq!(bare.ask("query-probe.asked", json!({})).outcome, needs);
    // Installed and not approved.
    extension::install(&bare.config(), &extension::BuiltIn::none(), &bare.ext())
        .expect("installed");
    assert_eq!(bare.ask("query-probe.asked", json!({})).outcome, needs);
    // Approved, and asked a query it does not declare.
    let probe = Probe::approved();
    assert_eq!(probe.ask("query-probe.mentions", json!({})).outcome, needs);
    assert_eq!(probe.ask("slack.unread", json!({})).outcome, needs);
    // Turned off in the project, and in the workspace.
    let executor = Executor::default().with_deadline(PATIENT);
    for off in [
        extension::project::Choices::from_text(
            Some("[extensions.query-probe]\nenabled = false\n"),
            None,
        ),
        extension::project::Choices::from_text(None, None).in_workspace(
            "alpha",
            Some(r#"{"settings": {"extensions": {"query-probe": {"enabled": false}}}}"#),
        ),
    ] {
        let answer = queries::ask(
            &executor,
            &probe.config(),
            &off,
            &queries::Asked {
                id: "query-probe.asked",
                workspace: "alpha",
                params: &json!({}),
                plane: false,
            },
            chrono::Utc::now(),
        );
        assert_eq!(answer.outcome, needs);
    }
    assert_eq!(probe.queried(), 0, "nothing was started");
}

#[test]
fn a_query_changed_since_its_approval_is_not_answered() {
    let probe = Probe::approved();
    let at = probe.ext().join(extension::MANIFEST);
    let mut doc: Value =
        serde_json::from_str(&std::fs::read_to_string(&at).expect("manifest")).expect("JSON");
    doc["contributes"]["queries"][0]["fresh_seconds"] = json!(1);
    std::fs::write(&at, doc.to_string()).expect("written");
    assert_eq!(
        probe.ask("query-probe.asked", json!({})).outcome,
        Err(ErrorKind::NotApproved)
    );
    assert_eq!(probe.queried(), 0);
}

#[test]
fn a_parameter_the_query_does_not_take_or_of_the_wrong_type_is_refused() {
    let probe = Probe::approved();
    for params in [json!({ "team": "x" }), json!({ "who": 3 }), json!(["who"])] {
        assert_eq!(
            probe.ask("query-probe.asked", params.clone()).outcome,
            Err(ErrorKind::Refused),
            "{params}"
        );
    }
    assert_eq!(probe.queried(), 0);
}

#[test]
fn a_slow_query_is_a_timeout_and_a_failing_one_failed() {
    let probe = Probe::approved();
    probe.behaves(json!({ "sleep_ms": 10_000 }));
    let began = std::time::Instant::now();
    let late = probe.ask_with(
        &Executor::default().with_deadline(Duration::from_millis(500)),
        "query-probe.asked",
        json!({ "who": "ana" }),
        false,
    );
    assert!(
        began.elapsed() < Duration::from_secs(4),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(late.outcome, Err(ErrorKind::Timeout));
    probe.behaves(json!({ "fail": true }));
    assert_eq!(
        probe
            .ask("query-probe.asked", json!({ "who": "ana" }))
            .outcome,
        Err(ErrorKind::Failed)
    );
}

#[test]
fn a_manifest_speaking_protocol_6_cannot_ask_for_queries() {
    let probe = Probe::assembled();
    let at = probe.ext().join(extension::MANIFEST);
    let mut doc: Value =
        serde_json::from_str(&std::fs::read_to_string(&at).expect("manifest")).expect("JSON");
    doc["version"] = json!(6);
    std::fs::write(&at, doc.to_string()).expect("written");
    let why = extension::read_at(&probe.ext())
        .map(|_| ())
        .expect_err("refused");
    assert!(
        why.contains("\"queries\", which needs protocol 7, and is version 6"),
        "{why}"
    );
}

#[test]
fn queries_without_their_capability_are_refused_by_name() {
    let probe = Probe::assembled();
    let at = probe.ext().join(extension::MANIFEST);
    let mut doc: Value =
        serde_json::from_str(&std::fs::read_to_string(&at).expect("manifest")).expect("JSON");
    doc["capabilities"] = json!([]);
    std::fs::write(&at, doc.to_string()).expect("written");
    let why = extension::read_at(&probe.ext())
        .map(|_| ())
        .expect_err("refused");
    assert!(why.contains("'contributes.queries'"), "{why}");
}
