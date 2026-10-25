// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The checks at one exact commit, as one of five closed values (ADR 0060 §6; the Phase 4
//! spec §3.5).
//!
//! | value | means |
//! |---|---|
//! | `PASSED` | at least one check at this head, each concluded success, neutral or skipped |
//! | `FAILED` | one concluded failure, cancelled, timed out, startup failure or `action_required` |
//! | `RUNNING` | one is queued or in progress |
//! | `NOT RUN` | **zero** checks at this head; a stale run does not count |
//! | `UNKNOWN` | charter could not ask, or got an answer it does not recognise |
//!
//! Precedence is `UNKNOWN` > `FAILED` > `RUNNING` > `NOT RUN` > `PASSED`: the one value that
//! means "charter did not look" is never outranked by one that means "it looked and it was
//! fine". Nothing here reads the pull request's own roll-up (`gh pr checks`,
//! `mergeStateStatus`) or the status line's cached CI word: each turns "nothing ran" into
//! green (charter-plane #561).
//!
//! On GitHub the read is two: the check runs **and** the commit statuses at the sha, summed,
//! because CI that reports through statuses (Jenkins, Buildkite) has no check runs at all. On
//! GitLab it is the merge request's own pipelines at that sha, every one of them: of one
//! source on one ref the newest is the verdict (a pipeline run again replaces its earlier
//! attempt), and a branch pipeline and a merge request pipeline at the same commit are both
//! judged, so one failed among them is `FAILED`. Both forges' lists are read whole: every
//! page, up to [`CHECKS_READ`], and a page that does not say how many entries there are is
//! `UNKNOWN`. A merged-results pipeline runs
//! at a sha that is not the branch head, and charter cannot tie it to this head, so a merge
//! request whose only pipelines are at another sha is `UNKNOWN`, never `NOT RUN`.

use serde_json::Value;

use super::ForgeError;

/// The five values, closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Ci {
    // Declared in ascending precedence, so `max` is the value that decides.
    Passed,
    NotRun,
    Running,
    Failed,
    Unknown,
}

impl Ci {
    /// The word a row shows.
    pub fn word(self) -> &'static str {
        match self {
            Ci::Passed => "PASSED",
            Ci::Failed => "FAILED",
            Ci::Running => "RUNNING",
            Ci::NotRun => "NOT RUN",
            Ci::Unknown => "UNKNOWN",
        }
    }
}

/// The checks at one commit: how many were counted (`None` when charter could not ask), the
/// value, why it is `UNKNOWN` when it is, and which ones failed when it is `FAILED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checks {
    pub total: Option<u64>,
    pub ci: Ci,
    pub why: Option<String>,
    /// The checks that failed, at most [`FAILED_NAMED`] of them: what a red save's pull
    /// request is named by (ADR 0051, amended 2026-10-13).
    pub failed: Vec<FailedCheck>,
}

impl Checks {
    fn unknown(why: impl Into<String>) -> Checks {
        Checks {
            total: None,
            ci: Ci::Unknown,
            why: Some(why.into()),
            failed: Vec::new(),
        }
    }
}

/// How many failed checks are named. A commit can carry hundreds; a person reads a handful.
pub const FAILED_NAMED: usize = 20;
/// The longest name kept of a failed check, in characters.
const NAME_LIMIT: usize = 120;
/// The longest link kept of a failed check, in bytes.
const URL_LIMIT: usize = 400;

/// One check that failed: its name as the forge shows it, and its page.
///
/// **Both are whatever wrote the check**: a workflow's job name, a status context, a target
/// link. So the name is one line with every invisible character escaped, and cut
/// ([`crate::shown::one_line`]: a name may be in any script and carry an emoji, and may not
/// carry a control or a direction override), and the link is kept only when it is an
/// `https://` address of a sane length; anything else is no link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedCheck {
    pub name: String,
    pub url: Option<String>,
    /// What the forge reads this check's own findings by ([`super::backend::Requests::annotations`]):
    /// a GitHub check run's id, a GitLab pipeline's. `None` for a commit status, which has
    /// none, and for a check read back from a file: it is the forge's answer, asked for when
    /// it is needed, and never kept.
    pub run: Option<u64>,
}

impl FailedCheck {
    /// A failed check, from what the forge (or a file purlis wrote earlier) names it by.
    pub fn of(name: Option<&str>, url: Option<&str>) -> FailedCheck {
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        FailedCheck {
            name: crate::shown::one_line(name.unwrap_or("a check"), NAME_LIMIT),
            url: url
                .filter(|u| {
                    u.starts_with("https://")
                        && u.len() <= URL_LIMIT
                        && !u.chars().any(|c| c.is_control() || c.is_whitespace())
                })
                .map(str::to_string),
            run: None,
        }
    }

    /// The same check, read by `run` on its forge.
    pub fn read_by(self, run: Option<u64>) -> FailedCheck {
        FailedCheck { run, ..self }
    }
}

/// How many findings of one failed check are read: a run can carry thousands, and what a
/// person or a chat needs first is the handful that say what broke.
pub const ANNOTATIONS_READ: usize = 10;
/// The longest title kept of a finding, in characters.
const TITLE_LIMIT: usize = 120;
/// The longest place kept of a finding (`path:line`), in characters.
const PLACE_LIMIT: usize = 200;
/// The longest message kept of a finding, in characters.
pub const MESSAGE_LIMIT: usize = 600;

/// One finding of a failed check, as its forge holds it: a GitHub check run's annotation, or
/// a failed job of a GitLab pipeline.
///
/// **Every field is whatever wrote the check**, as a [`FailedCheck`]'s name is: each is cut,
/// and made printable ([`crate::shown::one_line`] for the title and the place; the message
/// keeps its line breaks and loses every other control and invisible formatting character).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// What the finding is called: an annotation's title, a job's name.
    pub title: Option<String>,
    /// Where it points: `path:line`, or the path alone.
    pub at: Option<String>,
    /// What it says.
    pub message: String,
}

impl Annotation {
    /// A finding, from what the forge names it by. One with nothing to say is `None`.
    pub fn of(
        title: Option<&str>,
        path: Option<&str>,
        line: Option<u64>,
        message: Option<&str>,
    ) -> Option<Annotation> {
        let kept = |text: Option<&str>, most: usize| {
            text.map(str::trim)
                .filter(|t| !t.is_empty())
                .map(|t| crate::shown::one_line(t, most))
        };
        let message: String = message
            .unwrap_or_default()
            .replace("\r\n", "\n")
            .chars()
            .map(|c| if c == '\r' { '\n' } else { c })
            .filter(|c| *c == '\n' || *c == '\t' || !crate::panel::undrawable(*c))
            .collect();
        let message = message.trim();
        let title = kept(title, TITLE_LIMIT);
        if message.is_empty() && title.is_none() {
            return None;
        }
        let mut cut: String = message.chars().take(MESSAGE_LIMIT).collect();
        if message.chars().count() > MESSAGE_LIMIT {
            cut.push('…');
        }
        let at = kept(path, PLACE_LIMIT).map(|path| match line {
            Some(line) => format!("{path}:{line}"),
            None => path,
        });
        Some(Annotation {
            title,
            at,
            message: cut,
        })
    }
}

/// A GitHub check run's annotations, the first [`ANNOTATIONS_READ`]: failures first, then the
/// rest in the forge's order, because a run lists its notices and warnings beside what broke.
pub(super) fn github_annotations(answer: &Value) -> Result<Vec<Annotation>, String> {
    let list = answer
        .as_array()
        .ok_or("the forge's annotations answer is not a list")?;
    let failed = |entry: &&Value| entry["annotation_level"].as_str() == Some("failure");
    Ok(list
        .iter()
        .filter(failed)
        .chain(list.iter().filter(|entry| !failed(entry)))
        .filter_map(|entry| {
            Annotation::of(
                entry["title"].as_str(),
                entry["path"].as_str(),
                entry["start_line"].as_u64(),
                entry["message"].as_str(),
            )
        })
        .take(ANNOTATIONS_READ)
        .collect())
}

/// A GitLab pipeline's failed jobs, the first [`ANNOTATIONS_READ`], each as a finding: its
/// name and stage, and why GitLab says it failed.
pub(super) fn gitlab_failed_jobs(answer: &Value) -> Result<Vec<Annotation>, String> {
    let list = answer
        .as_array()
        .ok_or("the forge's jobs answer is not a list")?;
    Ok(list
        .iter()
        .filter_map(|job| {
            let name = match (job["name"].as_str(), job["stage"].as_str()) {
                (Some(name), Some(stage)) => Some(format!("{name} (stage {stage})")),
                (Some(name), None) => Some(name.to_string()),
                _ => None,
            };
            let why = job["failure_reason"]
                .as_str()
                .map(|reason| format!("the job failed: {reason}"))
                .unwrap_or_else(|| "the job failed".to_string());
            Annotation::of(name.as_deref(), None, None, Some(&why))
        })
        .take(ANNOTATIONS_READ)
        .collect())
}

/// What one run, status or pipeline says, before they are summed. `Uncounted` is a run the
/// forge itself disowned (`stale`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum One {
    Counted(Ci),
    Uncounted,
}

/// A GitHub check run: its `status` and `conclusion`.
fn check_run(run: &Value) -> One {
    let status = run["status"].as_str().unwrap_or("");
    if status != "completed" {
        return One::Counted(match status {
            "queued" | "in_progress" | "waiting" | "requested" | "pending" => Ci::Running,
            _ => Ci::Unknown,
        });
    }
    match run["conclusion"].as_str().unwrap_or("") {
        "success" | "neutral" | "skipped" => One::Counted(Ci::Passed),
        "failure" | "cancelled" | "timed_out" | "startup_failure" | "action_required" => {
            One::Counted(Ci::Failed)
        }
        "stale" => One::Uncounted,
        _ => One::Counted(Ci::Unknown),
    }
}

/// A GitHub commit status: its `state`.
fn commit_status(status: &Value) -> One {
    One::Counted(match status["state"].as_str().unwrap_or("") {
        "success" => Ci::Passed,
        "failure" | "error" => Ci::Failed,
        "pending" => Ci::Running,
        _ => Ci::Unknown,
    })
}

/// A GitLab pipeline: its `status`, one of the thirteen GitLab 19.4 lists
/// (`doc/api/pipelines.md`). `manual` waits on a person, so it did not pass, as GitHub's
/// `action_required` does not. `canceling` is on its way to `canceled`, and
/// `waiting_for_callback` is still going. A `skipped` pipeline ran nothing (`[ci skip]`, or
/// every job skipped), so it is not counted, and alone it is `NOT RUN` — unlike a GitHub check
/// run concluded `skipped`, which is one check among others saying it had nothing to do.
fn pipeline(p: &Value) -> One {
    if p["status"].as_str() == Some("skipped") {
        return One::Uncounted;
    }
    One::Counted(match p["status"].as_str().unwrap_or("") {
        "success" => Ci::Passed,
        "failed" | "canceled" | "canceling" | "manual" => Ci::Failed,
        "created"
        | "waiting_for_resource"
        | "preparing"
        | "waiting_for_callback"
        | "pending"
        | "running"
        | "scheduled" => Ci::Running,
        _ => Ci::Unknown,
    })
}

/// Sum what was read into one value. Zero counted is `NOT RUN`: silence is never a pass.
fn reduce(each: impl IntoIterator<Item = One>) -> Checks {
    let mut total = 0u64;
    let mut worst = Ci::Passed;
    for one in each {
        if let One::Counted(ci) = one {
            total += 1;
            worst = worst.max(ci);
        }
    }
    Checks {
        total: Some(total),
        ci: if total == 0 { Ci::NotRun } else { worst },
        why: None,
        failed: Vec::new(),
    }
}

/// The entries of `list` that `judge` counts as failed, each named by `name` and linked by
/// `url`, its keys in the forge's record.
fn failed_among<'a>(
    list: &'a [Value],
    judge: fn(&Value) -> One,
    name: &'a str,
    url: &'a str,
    run: Option<&'a str>,
) -> impl Iterator<Item = FailedCheck> + 'a {
    list.iter()
        .filter(move |entry| judge(entry) == One::Counted(Ci::Failed))
        .map(move |entry| {
            FailedCheck::of(entry[name].as_str(), entry[url].as_str())
                .read_by(run.and_then(|key| entry[key].as_u64()))
        })
}

/// Whether `sha` is a commit id charter will put in an API path: hex, 7 to 64.
pub fn sha_ok(sha: &str) -> bool {
    (7..=64).contains(&sha.len()) && sha.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The checks at `sha`, read by `read` only when `sha` is a commit id: anything else, and any
/// failure `read` reports, is `UNKNOWN` with its reason. Each backend's `checks_at` is this
/// around its own reads.
pub(super) fn guarded(sha: &str, read: impl FnOnce() -> Result<Checks, ForgeError>) -> Checks {
    if !sha_ok(sha) {
        return Checks::unknown(format!(
            "the forge named the head {}, which is not a commit id",
            crate::shown::short(sha)
        ));
    }
    read().unwrap_or_else(|why| Checks::unknown(why.to_string()))
}

/// The most checks of one kind purlis reads at a commit: ten pages of a hundred.
pub const CHECKS_READ: u64 = 1000;

/// One page of the list under `key`, and how many entries the forge counts in all. **A page
/// that does not say how many there are is refused**: without the count a first page would
/// stand for the whole list.
pub(super) fn page(answer: &Value, key: &str, what: &str) -> Result<(Vec<Value>, u64), String> {
    let list = answer[key]
        .as_array()
        .ok_or_else(|| format!("the forge's {what} answer holds no '{key}' list"))?;
    let counted = answer["total_count"]
        .as_u64()
        .ok_or_else(|| format!("the forge's {what} answer does not say how many there are"))?;
    Ok((list.clone(), counted))
}

/// Every entry of a paged list, read with `ask` (page 1, 2, …) until the forge's own count is
/// reached. More than [`CHECKS_READ`] is refused in words a person can act on, and so is a
/// forge that counts more than it sends.
pub(super) fn every_page(
    key: &str,
    what: &str,
    mut ask: impl FnMut(u64) -> Result<Value, ForgeError>,
) -> Result<Vec<Value>, ForgeError> {
    let mut all: Vec<Value> = Vec::new();
    for n in 1..=CHECKS_READ / 100 {
        let (list, counted) = page(&ask(n)?, key, what)?;
        if counted > CHECKS_READ {
            return Err(ForgeError::new(format!(
                "there are too many checks to read: the forge counts {counted} {what} at this \
                 commit, and purlis reads up to {CHECKS_READ}"
            )));
        }
        let got = list.len();
        all.extend(list);
        if all.len() as u64 >= counted {
            return Ok(all);
        }
        if got == 0 {
            break;
        }
    }
    Err(ForgeError::new(format!(
        "the forge counted more {what} than it sent ({} read); purlis reads them all or says \
         it could not",
        all.len()
    )))
}

/// GitHub's check runs and commit statuses at one sha, each list whole, summed.
pub(super) fn github(runs: &[Value], statuses: &[Value]) -> Checks {
    let mut checks = reduce(
        runs.iter()
            .map(check_run)
            .chain(statuses.iter().map(commit_status)),
    );
    checks.failed = failed_among(runs, check_run, "name", "html_url", Some("id"))
        .chain(failed_among(
            statuses,
            commit_status,
            "context",
            "target_url",
            None,
        ))
        .take(FAILED_NAMED)
        .collect();
    checks
}

/// The pipelines at `sha` that are each the verdict of what they ran for: of one `source` on
/// one `ref`, the newest (GitLab lists newest first), so a pipeline run again replaces its
/// earlier attempt, and a branch pipeline and a merge request pipeline at the same commit
/// are both judged.
fn judged_at<'a>(pipelines: &'a [Value], sha: &str) -> Vec<&'a Value> {
    let mut seen: Vec<(Option<&str>, Option<&str>)> = Vec::new();
    pipelines
        .iter()
        .filter(|p| p["sha"].as_str() == Some(sha))
        .filter(|p| {
            let of = (p["source"].as_str(), p["ref"].as_str());
            let first = !seen.contains(&of);
            seen.push(of);
            first
        })
        .collect()
}

/// A GitLab merge request's pipelines, judged at one sha.
pub(super) fn gitlab(answer: &Value, sha: &str, request: u64) -> Result<Checks, String> {
    let pipelines = answer
        .as_array()
        .ok_or("the forge's pipelines answer is not a list")?;
    let here = judged_at(pipelines, sha);
    if here.is_empty() && !pipelines.is_empty() {
        return Err(format!(
            "merge request !{request} has pipelines, none of them at {sha} (a merged-results \
             pipeline runs at a commit that is not the branch head), so purlis cannot say \
             what ran at this head"
        ));
    }
    // Every pipeline that is a verdict at this head is judged: one failed among them is
    // red, whatever a newer one of another kind says.
    let mut checks = reduce(here.iter().map(|p| pipeline(p)));
    checks.failed = here
        .iter()
        .filter(|p| pipeline(p) == One::Counted(Ci::Failed))
        .take(FAILED_NAMED)
        .map(|p| {
            let name = match (p["id"].as_u64(), p["status"].as_str()) {
                (Some(id), Some(status)) => format!("pipeline #{id} ({status})"),
                (Some(id), None) => format!("pipeline #{id}"),
                _ => "its pipeline".to_string(),
            };
            FailedCheck::of(Some(&name), p["web_url"].as_str()).read_by(p["id"].as_u64())
        })
        .collect();
    Ok(checks)
}

/// Whether every pipeline GitLab has at `sha` is one it `skipped`, and there is at least
/// one: CI was asked and ran nothing for this commit (`[ci skip]`, or rules that left every
/// job out), which is GitLab's own word that no check will run on it. A skipped pipeline
/// beside any other is not that.
pub(super) fn gitlab_skipped_at(answer: &Value, sha: &str) -> bool {
    let Some(pipelines) = answer.as_array() else {
        return false;
    };
    let here = judged_at(pipelines, sha);
    !here.is_empty() && here.iter().all(|p| p["status"].as_str() == Some("skipped"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn runs(list: &[(&str, Option<&str>)]) -> Checks {
        reduce(list.iter().map(|(status, conclusion)| {
            check_run(&json!({"status": status, "conclusion": conclusion}))
        }))
    }

    #[test]
    fn what_a_failed_check_found_is_read_failures_first_bounded_and_printable() {
        let long = "m".repeat(MESSAGE_LIMIT + 50);
        let mut list = vec![
            json!({"annotation_level": "notice", "title": "a note", "message": "fyi"}),
            json!({"annotation_level": "failure", "title": " unused\u{202e} ", "path": "src/a.rs",
                   "start_line": 3, "message": "one\r\ntwo\u{1b}[31m\u{200b}"}),
            json!({"annotation_level": "failure", "message": long}),
            json!({"annotation_level": "failure", "title": "", "message": "  "}),
        ];
        list.extend(
            (0..30).map(|n| json!({"annotation_level": "warning", "message": n.to_string()})),
        );
        let found = github_annotations(&json!(list)).unwrap();
        assert_eq!(found.len(), ANNOTATIONS_READ);
        assert_eq!(found[0].at.as_deref(), Some("src/a.rs:3"));
        assert_eq!(found[0].title.as_deref(), Some("unused⟦U+202E⟧"));
        assert_eq!(found[0].message, "one\ntwo[31m");
        assert_eq!(found[1].message.chars().count(), MESSAGE_LIMIT + 1);
        assert!(found[1].message.ends_with('…'));
        // The one with nothing to say is left out, and what is no failure comes after.
        assert_eq!(found[2].title.as_deref(), Some("a note"));
        assert!(github_annotations(&json!({"message": "Not Found"})).is_err());

        let jobs = gitlab_failed_jobs(&json!([
            {"name": "lint", "stage": "test", "failure_reason": "script_failure"},
            {"name": "build"},
            {"stage": "deploy"},
        ]))
        .unwrap();
        assert_eq!(jobs[0].title.as_deref(), Some("lint (stage test)"));
        assert_eq!(jobs[0].message, "the job failed: script_failure");
        assert_eq!(
            (jobs[1].title.as_deref(), jobs[1].message.as_str()),
            (Some("build"), "the job failed")
        );
        assert_eq!(jobs[2].title, None);
    }

    #[test]
    fn a_failed_check_carries_what_its_forge_reads_it_by_and_a_status_carries_none() {
        let checks = github(
            &[json!({"id": 4, "name": "lint", "status": "completed", "conclusion": "failure"})],
            &[json!({"context": "ci/ext", "state": "failure"})],
        );
        assert_eq!(
            checks.failed.iter().map(|c| c.run).collect::<Vec<_>>(),
            [Some(4), None]
        );
    }

    #[test]
    fn zero_runs_is_not_run_and_never_passed() {
        let checks = runs(&[]);
        assert_eq!((checks.total, checks.ci), (Some(0), Ci::NotRun));
    }

    #[test]
    fn success_neutral_and_skipped_pass() {
        let checks = runs(&[
            ("completed", Some("success")),
            ("completed", Some("neutral")),
            ("completed", Some("skipped")),
        ]);
        assert_eq!((checks.total, checks.ci), (Some(3), Ci::Passed));
    }

    #[test]
    fn action_required_is_a_failure_because_it_waits_on_a_person() {
        assert_eq!(
            runs(&[("completed", Some("action_required"))]).ci,
            Ci::Failed
        );
        for c in ["failure", "cancelled", "timed_out", "startup_failure"] {
            assert_eq!(runs(&[("completed", Some(c))]).ci, Ci::Failed, "{c}");
        }
    }

    #[test]
    fn a_stale_run_is_not_counted_so_alone_it_is_not_run() {
        let checks = runs(&[("completed", Some("stale"))]);
        assert_eq!((checks.total, checks.ci), (Some(0), Ci::NotRun));
        let checks = runs(&[("completed", Some("stale")), ("completed", Some("success"))]);
        assert_eq!((checks.total, checks.ci), (Some(1), Ci::Passed));
    }

    #[test]
    fn a_queued_or_running_check_is_running() {
        for s in ["queued", "in_progress", "waiting", "requested", "pending"] {
            assert_eq!(
                runs(&[(s, None), ("completed", Some("success"))]).ci,
                Ci::Running
            );
        }
    }

    #[test]
    fn anything_unrecognised_is_unknown_and_never_passed() {
        assert_eq!(runs(&[("completed", Some("brand-new"))]).ci, Ci::Unknown);
        assert_eq!(runs(&[("teleporting", None)]).ci, Ci::Unknown);
    }

    #[test]
    fn precedence_is_unknown_failed_running_not_run_passed() {
        let all = runs(&[
            ("completed", Some("success")),
            ("in_progress", None),
            ("completed", Some("failure")),
            ("completed", Some("mystery")),
        ]);
        assert_eq!(all.ci, Ci::Unknown);
        let no_unknown = runs(&[
            ("completed", Some("success")),
            ("in_progress", None),
            ("completed", Some("failure")),
        ]);
        assert_eq!(no_unknown.ci, Ci::Failed);
        let running = runs(&[("completed", Some("success")), ("in_progress", None)]);
        assert_eq!(running.ci, Ci::Running);
    }

    #[test]
    fn commit_statuses_count_as_checks() {
        let checks = reduce(
            [json!({"state": "success"}), json!({"state": "pending"})]
                .iter()
                .map(commit_status),
        );
        assert_eq!((checks.total, checks.ci), (Some(2), Ci::Running));
        assert_eq!(
            reduce([json!({"state": "error"})].iter().map(commit_status)).ci,
            Ci::Failed
        );
    }

    #[test]
    fn a_gitlab_pipeline_waiting_on_a_person_did_not_pass() {
        assert_eq!(
            reduce([pipeline(&json!({"status": "manual"}))]).ci,
            Ci::Failed
        );
        assert_eq!(
            reduce([pipeline(&json!({"status": "success"}))]).ci,
            Ci::Passed
        );
        assert_eq!(
            reduce([pipeline(&json!({"status": "running"}))]).ci,
            Ci::Running
        );
    }

    #[test]
    fn a_skipped_gitlab_pipeline_ran_nothing_so_it_is_not_run() {
        let checks = reduce([pipeline(&json!({"status": "skipped"}))]);
        assert_eq!((checks.total, checks.ci), (Some(0), Ci::NotRun));
    }

    #[test]
    fn a_failed_check_is_named_with_its_page_and_a_passing_one_is_not() {
        let runs = json!({"total_count": 3, "check_runs": [
            {"name": "build", "status": "completed", "conclusion": "success",
             "html_url": "https://github.com/acme/api/runs/1"},
            {"name": "lint", "status": "completed", "conclusion": "failure",
             "html_url": "https://github.com/acme/api/runs/2"},
            {"name": "deploy", "status": "completed", "conclusion": "cancelled",
             "html_url": "https://github.com/acme/api/runs/3"},
        ]});
        let statuses = json!({"total_count": 1, "statuses": [
            {"context": "ci/jenkins", "state": "error", "target_url": "https://ci.example/7"},
        ]});
        let checks = github(
            runs["check_runs"].as_array().unwrap(),
            statuses["statuses"].as_array().unwrap(),
        );
        assert_eq!(checks.ci, Ci::Failed);
        let named: Vec<(&str, Option<&str>)> = checks
            .failed
            .iter()
            .map(|c| (c.name.as_str(), c.url.as_deref()))
            .collect();
        assert_eq!(
            named,
            [
                ("lint", Some("https://github.com/acme/api/runs/2")),
                ("deploy", Some("https://github.com/acme/api/runs/3")),
                ("ci/jenkins", Some("https://ci.example/7")),
            ]
        );
    }

    #[test]
    fn what_names_a_failed_check_is_whoever_wrote_the_check_so_it_is_bounded() {
        // A name is cut and made readable; a link that is not a plain https address is no link.
        let long = "x".repeat(500);
        let check = FailedCheck::of(Some(&long), Some("javascript:alert(1)"));
        assert_eq!(
            check.name.chars().count(),
            NAME_LIMIT + 1,
            "cut, with its ellipsis"
        );
        // A name in any script keeps its letters; a direction override does not get through.
        let named = FailedCheck::of(Some("тесты ci \u{202e}dne"), None).name;
        assert!(named.starts_with("тесты ci "), "{named}");
        assert!(!named.contains('\u{202e}'), "{named}");
        assert_eq!(check.url, None);
        assert!(
            !FailedCheck::of(Some("a\u{1b}[31mb"), None)
                .name
                .contains('\u{1b}')
        );
        assert_eq!(
            FailedCheck::of(None, Some("http://plain.example/")).url,
            None
        );
        assert_eq!(FailedCheck::of(Some("  "), None).name, "a check");
        let spaced = FailedCheck::of(Some("n"), Some("https://a.example/x y"));
        assert_eq!(spaced.url, None);
        // More failures than a person reads: the first FAILED_NAMED are named.
        let many: Vec<Value> = (0..FAILED_NAMED + 5)
            .map(|n| json!({"name": format!("job {n}"), "status": "completed", "conclusion": "failure"}))
            .collect();
        let checks = github(&many, &[]);
        assert_eq!(checks.failed.len(), FAILED_NAMED);
        assert_eq!(checks.total, Some((FAILED_NAMED + 5) as u64));
    }

    #[test]
    fn a_failed_gitlab_pipeline_is_named_by_its_number_and_a_skipped_one_is_told_apart() {
        let sha = "a".repeat(40);
        let answer = json!([{"id": 9, "sha": sha, "status": "failed",
                             "web_url": "https://gitlab.com/acme/api/-/pipelines/9"}]);
        let checks = gitlab(&answer, &sha, 3).unwrap();
        assert_eq!(checks.ci, Ci::Failed);
        assert_eq!(checks.failed[0].name, "pipeline #9 (failed)");
        assert_eq!(
            checks.failed[0].url.as_deref(),
            Some("https://gitlab.com/acme/api/-/pipelines/9")
        );
        assert!(!gitlab_skipped_at(&answer, &sha));
        assert!(gitlab_skipped_at(
            &json!([{"id": 9, "sha": sha, "status": "skipped"}]),
            &sha
        ));
        assert!(!gitlab_skipped_at(&json!([]), &sha));
    }

    #[test]
    fn every_gitlab_pipeline_at_the_head_is_judged_and_not_only_the_newest() {
        // Review N2. Newest first, as GitLab lists them.
        let sha = "a".repeat(40);
        let p = |id: u64, source: &str, on: &str, status: &str| {
            json!({"id": id, "sha": sha, "source": source, "ref": on, "status": status,
                   "web_url": format!("https://gitlab.com/acme/api/-/pipelines/{id}")})
        };
        // A failed branch pipeline under a newer green merge request pipeline is red.
        let both = json!([
            p(
                9,
                "merge_request_event",
                "refs/merge-requests/3/head",
                "success"
            ),
            p(8, "push", "purlis/save", "failed"),
        ]);
        let checks = gitlab(&both, &sha, 3).unwrap();
        assert_eq!((checks.total, checks.ci), (Some(2), Ci::Failed));
        assert_eq!(checks.failed.len(), 1);
        assert_eq!(checks.failed[0].name, "pipeline #8 (failed)");
        // A newer skipped pipeline over an older failed one is red, and is not "none will run".
        let skipped_over_failed = json!([
            p(
                9,
                "merge_request_event",
                "refs/merge-requests/3/head",
                "skipped"
            ),
            p(8, "push", "purlis/save", "failed"),
        ]);
        assert_eq!(
            gitlab(&skipped_over_failed, &sha, 3).unwrap().ci,
            Ci::Failed
        );
        assert!(!gitlab_skipped_at(&skipped_over_failed, &sha));
        // Run again: of one source on one ref, the newest attempt is the verdict.
        let rerun = json!([
            p(9, "push", "purlis/save", "success"),
            p(8, "push", "purlis/save", "failed"),
        ]);
        let checks = gitlab(&rerun, &sha, 3).unwrap();
        assert_eq!((checks.total, checks.ci), (Some(1), Ci::Passed));
        // Skipped is "none will run" only when nothing else is at the head.
        let only_skipped = json!([p(9, "push", "purlis/save", "skipped")]);
        assert!(gitlab_skipped_at(&only_skipped, &sha));
    }

    #[test]
    fn a_page_that_does_not_say_how_many_there_are_is_not_read_as_all_of_them() {
        // Review N3: a missing count is an answer purlis could not enumerate.
        let (runs, total) = page(
            &json!({"total_count": 1, "check_runs": [{"status": "completed"}]}),
            "check_runs",
            "check runs",
        )
        .unwrap();
        assert_eq!((runs.len(), total), (1, 1));
        let uncounted = page(&json!({"check_runs": []}), "check_runs", "check runs");
        assert!(uncounted.is_err(), "{uncounted:?}");
    }

    #[test]
    fn only_a_commit_id_reaches_an_api_path() {
        assert!(sha_ok("9f3a1c2"));
        assert!(sha_ok(&"a".repeat(40)));
        for bad in ["", "9f3a1c", "-X", "../../x", "9f3a1c2/..", &"a".repeat(65)] {
            assert!(!sha_ok(bad), "{bad}");
        }
    }

    #[test]
    fn no_word_for_silence_reads_as_passing() {
        for ci in [Ci::NotRun, Ci::Unknown] {
            assert!(!ci.word().contains("PASS"), "{}", ci.word());
        }
    }
}
