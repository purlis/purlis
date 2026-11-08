// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A miss in the plane's skill log** (#5738, spec #5714): a row of its own beside the runs,
//! with the skill, harness and its version, persona and signal, and never a prompt; read back as
//! a miss, never as a run, and totalled per harness beside a skill's runs.

use super::*;

fn missing<'a>(skill: &'a str, harness: Option<&'a str>) -> Missing<'a> {
    Missing {
        skill,
        harness,
        harness_version: None,
        persona: Some("ops"),
        signal: Signal::Correction,
    }
}

/// Logs a start of built-in `skill`, by the name a harness started it by, on `harness`.
fn ran(plane: &Path, skill: &str, harness: Option<&str>) {
    let level = Level {
        origin: crate::skills::Origin::BuiltIn,
        local: false,
    };
    let starting = Starting {
        id: RUN,
        skill,
        started_by: StartedBy::SkillTool,
        workspace: None,
        level: Some(level),
        harness,
        ran: None,
    };
    started(plane, &starting, Some("ops"), may(), "box").unwrap();
}

#[test]
fn a_miss_is_one_row_with_its_skill_harness_version_persona_and_signal() {
    let dir = a_plane();
    let p = missed(
        dir.path(),
        &Missing {
            harness_version: Some("2.1.304"),
            ..missing("purlis-secrets", Some("claude"))
        },
        may(),
        "box",
    )
    .expect("logged");
    assert_eq!(p, path_for(dir.path(), may(), "box"));
    let row: serde_json::Value =
        serde_json::from_str(std::fs::read_to_string(&p).unwrap().trim()).unwrap();
    assert_eq!(
        row,
        serde_json::json!({
            "missed": "purlis-secrets",
            "signal": "correction",
            "harness": "claude",
            "harness_version": "2.1.304",
            "persona": "ops",
            "ts": crate::dispatch::stamp(may()),
        })
    );
}

#[test]
fn a_miss_is_never_read_as_a_run_and_reads_back_as_a_miss() {
    let dir = a_plane();
    missed(
        dir.path(),
        &missing("purlis-secrets", Some("codex")),
        may(),
        "box",
    )
    .unwrap();
    assert!(runs(dir.path()).is_empty(), "a miss is no run");
    let [miss] = &misses(dir.path())[..] else {
        panic!("one miss");
    };
    assert_eq!(miss.skill, "purlis-secrets");
    assert_eq!(miss.harness.as_deref(), Some("codex"));
    assert_eq!(miss.harness_version, None);
    assert_eq!(miss.persona.as_deref(), Some("ops"));
    assert_eq!(miss.signal, Signal::Correction);
    assert_eq!(miss.device, "box");
}

#[test]
fn a_miss_by_a_signal_a_newer_purlis_wrote_still_reads_as_a_miss() {
    let dir = a_plane();
    let p = path_for(dir.path(), may(), "box");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(
        &p,
        "{\"missed\":\"purlis-secrets\",\"signal\":\"hunch\",\"ts\":\"2026-05-04T11:32:17Z\"}\n\
         {\"missed\":\"purlis-handoff\",\"signal\":\"report\",\"ts\":\"2026-05-04T11:32:18Z\"}\n",
    )
    .unwrap();
    let signals: Vec<Signal> = misses(dir.path()).iter().map(|m| m.signal).collect();
    assert_eq!(signals, [Signal::Report, Signal::Unknown], "newest first");
}

#[test]
fn no_miss_is_logged_in_a_project_this_purlis_may_not_write() {
    let dir = tempfile::tempdir().unwrap();
    assert!(missed(dir.path(), &missing("purlis-secrets", None), may(), "box").is_none());
}

#[test]
fn a_skills_misses_are_totalled_per_harness_beside_its_runs() {
    let dir = a_plane();
    ran(dir.path(), "purlis:purlis-secrets", Some("claude"));
    for harness in [Some("claude"), Some("opencode"), None] {
        missed(
            dir.path(),
            &missing("purlis-secrets", harness),
            may(),
            "box",
        )
        .unwrap();
    }
    missed(
        dir.path(),
        &missing("purlis-handoff", Some("claude")),
        may(),
        "box",
    )
    .unwrap();
    let skill = "purlis-secrets";
    let runs: Vec<Run> = runs(dir.path())
        .into_iter()
        .filter(|r| r.is_of(skill))
        .collect();
    let misses: Vec<Miss> = misses(dir.path())
        .into_iter()
        .filter(|m| m.is_of(skill))
        .collect();
    let rows: Vec<(Group, u64, u64)> = stats_with_misses(&runs, &misses, By::Harness)
        .into_iter()
        .map(|row| (row.group, row.totals.runs, row.totals.misses))
        .collect();
    assert_eq!(
        rows,
        [
            (Group::Named("claude".to_owned()), 1, 1),
            (Group::Named("opencode".to_owned()), 0, 1),
            (Group::NotRecorded, 0, 1),
        ]
    );
    assert!(
        stats(&runs, By::Harness)
            .iter()
            .all(|row| row.totals.misses == 0),
        "totals of runs alone count none"
    );
}
