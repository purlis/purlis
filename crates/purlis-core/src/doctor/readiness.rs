// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `project readiness`: whether the project's forge would let everyone else with access sign in
//! to purlis and work (one login, slice D), and, for each blocker, who can fix it.
//!
//! The checks are [`crate::forge::readiness`]'s, read as the person with the forge seam's own
//! reads. **Only a doctor someone asked for asks the forge**: the preflight runs at every session
//! start and asks no network, so it has no row at all, as `project remote` has none.
//!
//! The row is OK only when every check applies and is ready. A blocker or an unreadable setting
//! is a warning (never a failure: the person can work; somebody else may not), and the hint
//! names each blocker, who can fix it, and where.

use super::{Doctor, Row};
use crate::forge::readiness::{self, Connection, Report, Verdict, Who};

const NAME: &str = "project readiness";

pub(super) fn project_readiness(d: &Doctor) -> Option<Row> {
    if d.preflight {
        return None;
    }
    Some(row(d))
}

fn row(d: &Doctor) -> Row {
    let url = match super::remote::origin_url(d, NAME) {
        Ok(url) => url,
        Err(row) => return row,
    };
    let Some((caller, transport)) = d.forges.as_ref() else {
        return Row::not_checked(NAME, "this doctor asks no forge");
    };
    match readiness::check_url(
        &url,
        d.root.as_path(),
        Connection::of_environment(),
        caller,
        transport.clone(),
    ) {
        Ok(report) => said(&report),
        Err(why) => Row::not_checked(NAME, why),
    }
}

/// The row for what the forge answered.
pub(super) fn said(report: &Report) -> Row {
    let path = super::one_line(&report.path, super::DISPLAY_LIMIT);
    if report.ready_for_all() {
        return Row::ok(
            NAME,
            format!("{path} on {} is ready for all users", report.host),
        );
    }
    let blocked = report.blockers().count();
    let unknown = report.unknowns().count();
    let detail = format!(
        "{path} on {} is not ready for all users: {blocked} blocked, {unknown} not readable",
        report.host
    );
    let mut lines: Vec<String> = Vec::new();
    for check in report.checks.iter().filter(|c| c.verdict != Verdict::Ready) {
        let who = match &check.verdict {
            Verdict::Blocked(Who::You) => "you can fix it".to_owned(),
            Verdict::Blocked(who) => format!("the {} can fix it", who.word()),
            _ => "not readable, so not ready".to_owned(),
        };
        let at = check
            .fix
            .as_ref()
            .map(|f| format!(" ({})", f.link))
            .unwrap_or_default();
        lines.push(format!(
            "{}: {} — {who}{at}",
            check.id.title(),
            super::one_line(&check.detail, super::DISPLAY_LIMIT)
        ));
    }
    Row::warn(NAME, detail, lines.join("; "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::Status;
    use crate::forge::Kind;
    use crate::forge::readiness::{Check, CheckId, Fix};

    fn report(checks: Vec<Check>) -> Report {
        Report {
            kind: Kind::GitHub,
            host: "github.com".to_owned(),
            path: "acme/app".to_owned(),
            checks,
        }
    }

    fn check(id: CheckId, verdict: Verdict) -> Check {
        Check {
            id,
            verdict,
            detail: "found".to_owned(),
            fix: Some(Fix {
                link: "https://github.com/x".to_owned(),
                steps: vec![],
                request: None,
            }),
        }
    }

    #[test]
    fn a_report_with_every_check_ready_is_a_green_row() {
        let row = said(&report(vec![check(CheckId::Role, Verdict::Ready)]));
        assert_eq!(row.status, Status::Ok);
        assert!(row.detail.contains("ready for all users"));
    }

    #[test]
    fn a_blocker_is_a_warning_naming_who_fixes_it_and_where() {
        let row = said(&report(vec![
            check(CheckId::Role, Verdict::Ready),
            check(CheckId::OauthAppAccess, Verdict::Blocked(Who::Owner)),
        ]));
        assert_eq!(row.status, Status::Warn);
        assert!(
            row.hint
                .contains("the owner can fix it (https://github.com/x)"),
            "{}",
            row.hint
        );
    }

    #[test]
    fn an_unreadable_check_is_a_warning_and_never_a_green_row() {
        let row = said(&report(vec![check(
            CheckId::BranchProtection,
            Verdict::Unknown,
        )]));
        assert_eq!(row.status, Status::Warn);
        assert!(row.hint.contains("not readable, so not ready"));
    }

    #[test]
    fn an_empty_report_is_not_ready() {
        assert_eq!(said(&report(vec![])).status, Status::Warn);
    }

    #[test]
    fn the_preflight_asks_no_forge_and_has_no_row() {
        let (_dir, root) = crate::doctor::tests::plane("");
        let mut d = crate::doctor::tests::doctor(&root);
        d.preflight = true;
        assert!(project_readiness(&d).is_none());
    }
}
