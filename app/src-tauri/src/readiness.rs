// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **Settings › You › Ready for all users** (one login, slice D): whether the open project's
//! forge would let everyone else with access to it sign in to purlis and work, and, for each
//! blocker, who can fix it, the clicks, and a request text for them.
//!
//! The checks are the core's ([`purlis_core::forge::readiness`]), read as the window, over the
//! person's own sign-in. Nothing is kept: the group asks again each time it is shown and when
//! the person presses Check again. A check that could not be read is `unknown`, which is never
//! read as ready.

use std::path::Path;

use purlis_core::forge::readiness::{Check, Report, Verdict};

use crate::planes::{PlaneId, Planes};

/// One check, as the window draws it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
pub struct ReadinessCheck {
    pub id: String,
    pub title: String,
    /// `ready`, `blocked` or `unknown`.
    pub verdict: String,
    /// Who can fix a blocker: `you`, `owner` or `admin`.
    pub who: Option<String>,
    pub detail: String,
    pub link: Option<String>,
    pub steps: Vec<String>,
    /// A short message for whoever can fix it, when it is not the person.
    pub request: Option<String>,
}

/// What asking about the project's forge came to.
#[derive(Debug, Clone, PartialEq, serde::Serialize, specta::Type)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum ReadinessAnswer {
    /// Nothing was checked, and why on one line: no `origin`, a path on this machine, a host no
    /// forge purlis knows.
    NotChecked { why: String },
    /// The forge `host` was asked about `path`. `ready` is every check ready, and only that.
    Checked {
        host: String,
        path: String,
        ready: bool,
        checks: Vec<ReadinessCheck>,
    },
}

/// Whether the project at the open plane is ready for all users on its forge. Asks the network:
/// off the UI thread.
#[tauri::command]
#[specta::specta]
pub async fn plane_readiness(
    planes: tauri::State<'_, Planes>,
    plane: PlaneId,
) -> Result<ReadinessAnswer, String> {
    let root = planes.held(&plane)?.root().to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        readiness_of(&root, |url| {
            purlis_core::forge::readiness::check_url_in_window(
                url,
                &root,
                &purlis_core::forge::Caller::window(),
            )
        })
    })
    .await
    .map_err(|err| format!("checking the project's forge did not finish: {err}"))
}

/// [`plane_readiness`], without a runtime: `check` is given the project's `origin`.
pub fn readiness_of(
    root: &Path,
    check: impl FnOnce(&str) -> Result<Report, String>,
) -> ReadinessAnswer {
    let Some(url) = crate::live::origin_of(root) else {
        return ReadinessAnswer::NotChecked {
            why: "The project has no origin, so there is no forge to check.".to_owned(),
        };
    };
    match check(&url) {
        Ok(report) => answer(&report),
        Err(why) => ReadinessAnswer::NotChecked { why },
    }
}

fn answer(report: &Report) -> ReadinessAnswer {
    ReadinessAnswer::Checked {
        host: report.host.clone(),
        path: report.path.clone(),
        ready: report.ready_for_all(),
        checks: report.checks.iter().map(row).collect(),
    }
}

fn row(check: &Check) -> ReadinessCheck {
    let (verdict, who) = match &check.verdict {
        Verdict::Ready => ("ready", None),
        Verdict::Blocked(who) => ("blocked", Some(who.word().to_owned())),
        Verdict::Unknown => ("unknown", None),
    };
    ReadinessCheck {
        id: check.id.word().to_owned(),
        title: check.id.title().to_owned(),
        verdict: verdict.to_owned(),
        who,
        detail: check.detail.clone(),
        link: check.fix.as_ref().map(|f| f.link.clone()),
        steps: check
            .fix
            .as_ref()
            .map(|f| f.steps.clone())
            .unwrap_or_default(),
        request: check.fix.as_ref().and_then(|f| f.request.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_core::forge::Kind;
    use purlis_core::forge::readiness::{CheckId, Fix, Who};

    fn report(checks: Vec<Check>) -> Report {
        Report {
            kind: Kind::GitHub,
            host: "github.com".to_owned(),
            path: "acme/app".to_owned(),
            checks,
        }
    }

    fn check(verdict: Verdict) -> Check {
        Check {
            id: CheckId::OauthAppAccess,
            verdict,
            detail: "found".to_owned(),
            fix: Some(Fix {
                link: "https://github.com/x".to_owned(),
                steps: vec!["Open it.".to_owned()],
                request: Some("Please approve.".to_owned()),
            }),
        }
    }

    #[test]
    fn a_blocker_is_told_with_who_fixes_it_and_the_request() {
        let ReadinessAnswer::Checked { ready, checks, .. } =
            answer(&report(vec![check(Verdict::Blocked(Who::Owner))]))
        else {
            panic!("checked");
        };
        assert!(!ready);
        assert_eq!(checks[0].verdict, "blocked");
        assert_eq!(checks[0].who.as_deref(), Some("owner"));
        assert_eq!(checks[0].request.as_deref(), Some("Please approve."));
    }

    #[test]
    fn an_unknown_check_is_never_told_as_ready() {
        let ReadinessAnswer::Checked { ready, checks, .. } =
            answer(&report(vec![check(Verdict::Unknown)]))
        else {
            panic!("checked");
        };
        assert!(!ready);
        assert_eq!(checks[0].verdict, "unknown");
        assert_eq!(checks[0].who, None);
    }

    #[test]
    fn a_project_without_an_origin_checks_nothing_and_asks_no_forge() {
        let dir = tempfile::tempdir().unwrap();
        let said = readiness_of(dir.path(), |_| panic!("no forge is asked"));
        assert!(matches!(said, ReadinessAnswer::NotChecked { .. }));
    }
}
