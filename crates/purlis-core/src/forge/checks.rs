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
//! GitLab it is the merge request's own pipelines at that sha. A merged-results pipeline runs
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
        }
    }
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
) -> impl Iterator<Item = FailedCheck> + 'a {
    list.iter()
        .filter(move |entry| judge(entry) == One::Counted(Ci::Failed))
        .map(move |entry| FailedCheck::of(entry[name].as_str(), entry[url].as_str()))
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

/// The list under `key`, read whole or refused: a page that does not hold every entry the
/// forge counted is a list charter could not enumerate, and that is `UNKNOWN`.
fn whole<'a>(answer: &'a Value, key: &str, what: &str) -> Result<&'a Vec<Value>, String> {
    let list = answer[key]
        .as_array()
        .ok_or_else(|| format!("the forge's {what} answer holds no '{key}' list"))?;
    let counted = answer["total_count"].as_u64().unwrap_or(list.len() as u64);
    if counted as usize != list.len() {
        return Err(format!(
            "the forge counted {counted} {what} and sent {}; purlis reads them all or says it \
             could not",
            list.len()
        ));
    }
    Ok(list)
}

/// GitHub's check runs and commit statuses at one sha, summed.
pub(super) fn github(runs: &Value, statuses: &Value) -> Result<Checks, String> {
    let runs = whole(runs, "check_runs", "check runs")?;
    let statuses = whole(statuses, "statuses", "commit statuses")?;
    let mut checks = reduce(
        runs.iter()
            .map(check_run)
            .chain(statuses.iter().map(commit_status)),
    );
    checks.failed = failed_among(runs, check_run, "name", "html_url")
        .chain(failed_among(
            statuses,
            commit_status,
            "context",
            "target_url",
        ))
        .take(FAILED_NAMED)
        .collect();
    Ok(checks)
}

/// A GitLab merge request's pipelines, judged at one sha.
pub(super) fn gitlab(answer: &Value, sha: &str, request: u64) -> Result<Checks, String> {
    let pipelines = answer
        .as_array()
        .ok_or("the forge's pipelines answer is not a list")?;
    let here: Vec<&Value> = pipelines
        .iter()
        .filter(|p| p["sha"].as_str() == Some(sha))
        .collect();
    if here.is_empty() && !pipelines.is_empty() {
        return Err(format!(
            "merge request !{request} has pipelines, none of them at {sha} (a merged-results \
             pipeline runs at a commit that is not the branch head), so purlis cannot say \
             what ran at this head"
        ));
    }
    // Newest first, as GitLab lists them: the latest pipeline at this head is its verdict.
    // `total` is then what was judged: that one pipeline, or none.
    let mut checks = reduce(here.first().map(|p| pipeline(p)));
    if checks.ci == Ci::Failed
        && let Some(p) = here.first()
    {
        let name = match (p["id"].as_u64(), p["status"].as_str()) {
            (Some(id), Some(status)) => format!("pipeline #{id} ({status})"),
            (Some(id), None) => format!("pipeline #{id}"),
            _ => "its pipeline".to_string(),
        };
        checks.failed = vec![FailedCheck::of(Some(&name), p["web_url"].as_str())];
    }
    Ok(checks)
}

/// Whether the merge request's newest pipeline at `sha` is one GitLab `skipped`: CI was asked
/// and ran nothing for this commit (`[ci skip]`, or rules that left every job out), which is
/// GitLab's own word that no check will run on it.
pub(super) fn gitlab_skipped_at(answer: &Value, sha: &str) -> bool {
    answer
        .as_array()
        .and_then(|all| all.iter().find(|p| p["sha"].as_str() == Some(sha)))
        .is_some_and(|p| p["status"].as_str() == Some("skipped"))
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
        let checks = github(&runs, &statuses).unwrap();
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
        let runs = json!({"total_count": many.len(), "check_runs": many});
        let checks = github(&runs, &json!({"total_count": 0, "statuses": []})).unwrap();
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
