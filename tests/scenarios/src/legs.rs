// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Reading a run's leg reports back** (T1.1-54): the JSON lines the legs wrote under
//! `target/sandbox-scenarios/` (and the fixture and e2e covers' runner wrote beside them), the
//! legs a run says it ran, and the trigger that started it, for the run-result gate step
//! (`judge_runs`). Nothing here parses harness output: a record is ticket 5's schema, written by
//! the host that ran the entry.

use std::fs;
use std::path::Path;

use crate::coverage::Ran;
use crate::model::{LegRecord, Os, ReportLeg, TimingsMode};
use crate::plan::Trigger;

/// Every record of every `*.jsonl` file under `path`, in file-name order. A line that is not a
/// record of ticket 5's schema is an error, never skipped: a report that cannot be read must not
/// pass silently.
pub fn read_reports(path: &Path) -> Result<Vec<LegRecord>, String> {
    let mut records = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            records.extend(read_reports(&entry)?);
        } else if entry.extension().is_some_and(|e| e == "jsonl") && !is_timings(&entry) {
            let text =
                fs::read_to_string(&entry).map_err(|e| format!("{}: {e}", entry.display()))?;
            for (n, line) in text
                .lines()
                .enumerate()
                .filter(|(_, l)| !l.trim().is_empty())
            {
                let record = serde_json::from_str(line)
                    .map_err(|e| format!("{}:{}: {e}", entry.display(), n + 1))?;
                records.push(record);
            }
        }
    }
    Ok(records)
}

/// `<os>-timings.jsonl` holds each scenario's run time, not leg records.
fn is_timings(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().ends_with("-timings.jsonl"))
}

/// The trigger a workflow event and labels make: `pull_request`, `label` (a pull request with the
/// `production-timings` label), `merge_group` or `nightly`.
pub fn trigger_from(word: &str) -> Result<Trigger, String> {
    match word {
        "pull_request" => Ok(Trigger::PullRequest),
        "label" => Ok(Trigger::Label),
        "merge_group" => Ok(Trigger::MergeQueue),
        "nightly" => Ok(Trigger::Nightly),
        other => Err(format!(
            "{other:?} is no trigger: pull_request, label, merge_group or nightly"
        )),
    }
}

/// The legs a run ran, from `leg:os:timings,leg:os:timings` (`fake:linux:scaled,fixture:macos:scaled`).
pub fn ran_from(spec: &str) -> Result<Vec<Ran>, String> {
    spec.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|part| {
            let words: Vec<&str> = part.split(':').collect();
            let [leg, os, timings] = words[..] else {
                return Err(format!("{part:?} is not leg:os:timings"));
            };
            let word = |w: &str| serde_json::Value::String(w.to_owned());
            Ok(Ran {
                leg: serde_json::from_value::<ReportLeg>(word(leg))
                    .map_err(|_| format!("{leg:?} is no leg"))?,
                os: serde_json::from_value::<Os>(word(os))
                    .map_err(|_| format!("{os:?} is no OS"))?,
                timings: serde_json::from_value::<TimingsMode>(word(timings))
                    .map_err(|_| format!("{timings:?} is no timings"))?,
            })
        })
        .collect()
}
