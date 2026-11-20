// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! Each check on its own, from facts, and then the whole read through a fake forge that answers
//! by path. The rule under all of them: what could not be read is never ready.

use std::collections::HashMap;
use std::sync::Arc;

use super::*;
use crate::forge::transport::{Call, Endpoint, NoAnswer, Reply};

fn github(host: &str) -> Forge {
    Forge {
        kind: Kind::GitHub,
        host: host.to_owned(),
    }
}

fn gitlab(host: &str) -> Forge {
    Forge {
        kind: Kind::GitLab,
        host: host.to_owned(),
    }
}

fn facts(role: Role, access: OrgAccess) -> RepoFacts {
    RepoFacts {
        owner: "acme".to_owned(),
        owner_is_group: true,
        default_branch: "main".to_owned(),
        role,
        org_owner: false,
        instance_admin: false,
        org_access: access,
    }
}

fn all_known(repo: RepoRead) -> Facts {
    Facts {
        repo,
        protected: Fact::Known(true),
        user_apps: Fact::Known(true),
        ci_token: Fact::Known(true),
        registered: Fact::Known(false),
    }
}

fn verdict(report: &Report, id: CheckId) -> Option<Verdict> {
    report
        .checks
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.verdict.clone())
}

fn check_of(report: &Report, id: CheckId) -> &Check {
    report
        .checks
        .iter()
        .find(|c| c.id == id)
        .expect("the check applies")
}

#[test]
fn a_github_com_org_repo_that_everything_allows_is_ready_for_all() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    let report = evaluate(&github("github.com"), "acme/app", Connection::SignIn, &f);
    assert!(report.ready_for_all(), "{report:#?}");
    assert_eq!(report.checks.len(), 3, "role, approval, protection");
}

#[test]
fn an_org_that_refused_purlis_blocks_with_the_owner_and_a_request_text() {
    let f = all_known(RepoRead::Restricted(
        "OAuth App access restrictions".to_owned(),
    ));
    let report = evaluate(&github("github.com"), "acme/app", Connection::SignIn, &f);
    assert_eq!(
        verdict(&report, CheckId::OauthAppAccess),
        Some(Verdict::Blocked(Who::Owner))
    );
    let fix = check_of(&report, CheckId::OauthAppAccess)
        .fix
        .as_ref()
        .unwrap();
    assert_eq!(
        fix.link,
        "https://github.com/organizations/acme/settings/oauth_application_policy"
    );
    assert!(
        fix.steps
            .iter()
            .any(|s| s.contains("/settings/connections/applications/Ov23litEn94I5HXrpdsX"))
    );
    let request = fix.request.as_ref().expect("an owner is asked");
    assert!(
        request.contains("acme/app") && request.contains(&fix.link),
        "{request}"
    );
    assert!(!report.ready_for_all());
}

#[test]
fn an_org_owner_who_sees_the_restriction_fixes_it_themselves_and_asks_nobody() {
    let mut f = facts(Role::Admin, OrgAccess::Restricted);
    f.org_owner = true;
    let report = evaluate(
        &github("github.com"),
        "acme/app",
        Connection::SignIn,
        &all_known(RepoRead::Read(f)),
    );
    assert_eq!(
        verdict(&report, CheckId::OauthAppAccess),
        Some(Verdict::Blocked(Who::You))
    );
    assert_eq!(
        check_of(&report, CheckId::OauthAppAccess)
            .fix
            .as_ref()
            .unwrap()
            .request,
        None
    );
}

#[test]
fn a_public_org_repo_cannot_prove_the_approval_so_it_is_unknown_and_not_ready() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Unproven)));
    let report = evaluate(&github("github.com"), "acme/app", Connection::SignIn, &f);
    assert_eq!(
        verdict(&report, CheckId::OauthAppAccess),
        Some(Verdict::Unknown)
    );
    assert!(!report.ready_for_all());
}

#[test]
fn a_personal_repo_has_no_org_approval_to_check() {
    let mut f = facts(Role::Admin, OrgAccess::NotApplicable);
    f.owner_is_group = false;
    let report = evaluate(
        &github("github.com"),
        "me/app",
        Connection::SignIn,
        &all_known(RepoRead::Read(f)),
    );
    assert_eq!(verdict(&report, CheckId::OauthAppAccess), None);
    assert!(report.ready_for_all());
}

#[test]
fn a_headless_token_connection_is_not_held_to_an_organizations_oauth_approval() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Restricted)));
    let report = evaluate(&github("github.com"), "acme/app", Connection::Token, &f);
    assert_eq!(verdict(&report, CheckId::OauthAppAccess), None);
    assert_eq!(verdict(&report, CheckId::CiToken), Some(Verdict::Ready));
}

#[test]
fn a_pasted_token_in_the_window_cannot_show_the_approval_so_a_restricted_org_is_not_ready() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Restricted)));
    let report = evaluate(&github("github.com"), "acme/app", Connection::Pasted, &f);
    let approval = check_of(&report, CheckId::OauthAppAccess);
    assert_eq!(approval.verdict, Verdict::Unknown);
    assert!(approval.detail.contains("pasted token cannot show"));
    assert_eq!(verdict(&report, CheckId::CiToken), None);
    assert!(!report.ready_for_all());
}

#[test]
fn a_pasted_token_is_still_held_to_the_server_knowing_purlis() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    let report = evaluate(
        &github("ghe.example.com"),
        "acme/app",
        Connection::Pasted,
        &f,
    );
    assert!(matches!(
        verdict(&report, CheckId::ServerRegistration),
        Some(Verdict::Blocked(_))
    ));
    assert!(!report.ready_for_all());
}

#[test]
fn a_read_only_role_blocks_and_the_owner_is_asked_for_write() {
    let f = all_known(RepoRead::Read(facts(Role::Read, OrgAccess::Approved)));
    let report = evaluate(&github("github.com"), "acme/app", Connection::SignIn, &f);
    assert_eq!(
        verdict(&report, CheckId::Role),
        Some(Verdict::Blocked(Who::Owner))
    );
    let request = check_of(&report, CheckId::Role)
        .fix
        .as_ref()
        .unwrap()
        .request
        .clone()
        .unwrap();
    assert!(request.contains("Write"), "{request}");
}

#[test]
fn a_gitlab_role_names_developer_and_the_members_page() {
    let f = all_known(RepoRead::Read(facts(Role::Read, OrgAccess::NotApplicable)));
    let report = evaluate(&gitlab("gitlab.com"), "acme/app", Connection::SignIn, &f);
    let fix = check_of(&report, CheckId::Role).fix.as_ref().unwrap();
    assert_eq!(fix.link, "https://gitlab.com/acme/app/-/project_members");
    assert!(fix.request.as_ref().unwrap().contains("Developer"));
}

#[test]
fn an_unprotected_default_branch_blocks_and_a_repo_admin_fixes_it_themselves() {
    let mut f = all_known(RepoRead::Read(facts(Role::Admin, OrgAccess::Approved)));
    f.protected = Fact::Known(false);
    let report = evaluate(&github("github.com"), "acme/app", Connection::SignIn, &f);
    assert_eq!(
        verdict(&report, CheckId::BranchProtection),
        Some(Verdict::Blocked(Who::You))
    );
    let fix = check_of(&report, CheckId::BranchProtection)
        .fix
        .as_ref()
        .unwrap();
    assert_eq!(
        fix.link,
        "https://github.com/acme/app/settings/rules/new?target=branch"
    );
    assert_eq!(fix.request, None);
}

#[test]
fn an_unprotected_gitlab_branch_asks_the_owner_when_the_person_is_a_developer() {
    let mut f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::NotApplicable)));
    f.protected = Fact::Known(false);
    let report = evaluate(&gitlab("gitlab.com"), "acme/app", Connection::SignIn, &f);
    assert_eq!(
        verdict(&report, CheckId::BranchProtection),
        Some(Verdict::Blocked(Who::Owner))
    );
    let fix = check_of(&report, CheckId::BranchProtection)
        .fix
        .as_ref()
        .unwrap();
    assert_eq!(
        fix.link,
        "https://gitlab.com/acme/app/-/settings/repository"
    );
}

#[test]
fn a_protection_that_could_not_be_read_is_unknown_and_never_ready() {
    let mut f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    f.protected = Fact::unknown("Not Found");
    let report = evaluate(&github("github.com"), "acme/app", Connection::SignIn, &f);
    assert_eq!(
        verdict(&report, CheckId::BranchProtection),
        Some(Verdict::Unknown)
    );
    assert!(!report.ready_for_all());
    assert_eq!(report.unknowns().count(), 1);
}

#[test]
fn a_repo_that_could_not_be_read_is_never_ready_whatever_else_is_known() {
    let f = all_known(RepoRead::Unknown("Not Found".to_owned()));
    let report = evaluate(&github("github.com"), "acme/app", Connection::SignIn, &f);
    assert_eq!(verdict(&report, CheckId::Role), Some(Verdict::Unknown));
    assert!(!report.ready_for_all());
}

#[test]
fn a_ghes_server_without_purlis_blocks_with_its_admin() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    let report = evaluate(
        &github("ghe.example.com"),
        "acme/app",
        Connection::SignIn,
        &f,
    );
    assert_eq!(
        verdict(&report, CheckId::ServerRegistration),
        Some(Verdict::Blocked(Who::Admin))
    );
    let fix = check_of(&report, CheckId::ServerRegistration)
        .fix
        .as_ref()
        .unwrap();
    assert_eq!(
        fix.link,
        "https://ghe.example.com/settings/applications/new"
    );
    assert!(fix.steps.iter().any(|s| s.contains("Device Flow")));
    assert!(fix.request.as_ref().unwrap().contains("ghe.example.com"));
}

#[test]
fn an_instance_admin_registers_purlis_on_their_own_server_without_asking() {
    let mut f = facts(Role::Admin, OrgAccess::Approved);
    f.instance_admin = true;
    let report = evaluate(
        &github("ghe.example.com"),
        "acme/app",
        Connection::SignIn,
        &all_known(RepoRead::Read(f)),
    );
    assert_eq!(
        verdict(&report, CheckId::ServerRegistration),
        Some(Verdict::Blocked(Who::You))
    );
}

#[test]
fn a_registration_purlis_ships_makes_the_server_check_vanish() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::NotApplicable)));
    let report = evaluate(&gitlab("gitlab.com"), "acme/app", Connection::SignIn, &f);
    assert_eq!(verdict(&report, CheckId::ServerRegistration), None);
    assert_eq!(verdict(&report, CheckId::OauthApplications), None);
}

#[test]
fn a_self_hosted_gitlab_that_forbids_user_apps_blocks_with_its_admin() {
    let mut f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::NotApplicable)));
    f.user_apps = Fact::Known(false);
    let report = evaluate(
        &gitlab("git.example.com"),
        "acme/app",
        Connection::SignIn,
        &f,
    );
    assert_eq!(
        verdict(&report, CheckId::OauthApplications),
        Some(Verdict::Blocked(Who::Admin))
    );
    let fix = check_of(&report, CheckId::OauthApplications)
        .fix
        .as_ref()
        .unwrap();
    assert_eq!(
        fix.link,
        "https://git.example.com/admin/application_settings/general"
    );
    let registration = check_of(&report, CheckId::ServerRegistration)
        .fix
        .as_ref()
        .unwrap();
    assert!(
        registration
            .steps
            .iter()
            .any(|s| s.contains("http://127.0.0.1:47823/purlis/signin"))
    );
}

#[test]
fn a_setting_only_an_admin_reads_is_unknown_to_everyone_else() {
    let mut f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::NotApplicable)));
    f.user_apps = Fact::unknown("403");
    let report = evaluate(
        &gitlab("git.example.com"),
        "acme/app",
        Connection::SignIn,
        &f,
    );
    assert_eq!(
        verdict(&report, CheckId::OauthApplications),
        Some(Verdict::Unknown)
    );
}

#[test]
fn a_token_connection_checks_what_ci_may_do() {
    for (known, expected) in [
        (Fact::Known(true), Verdict::Ready),
        (Fact::Known(false), Verdict::Blocked(Who::Owner)),
        (Fact::unknown("403"), Verdict::Unknown),
    ] {
        let mut f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
        f.ci_token = known;
        let report = evaluate(&github("github.com"), "acme/app", Connection::Token, &f);
        assert_eq!(verdict(&report, CheckId::CiToken), Some(expected));
    }
}

#[test]
fn a_sign_in_connection_does_not_check_the_ci_token() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    let report = evaluate(&github("github.com"), "acme/app", Connection::SignIn, &f);
    assert_eq!(verdict(&report, CheckId::CiToken), None);
}

#[test]
fn a_host_is_ready_only_when_every_project_checked_is_and_none_checked_is_not_ready() {
    let ready = Report {
        kind: Kind::GitHub,
        host: "github.com".to_owned(),
        path: "a/b".to_owned(),
        checks: vec![Check {
            id: CheckId::Role,
            verdict: Verdict::Ready,
            detail: String::new(),
            fix: None,
        }],
    };
    let mut unknown = ready.clone();
    unknown.checks[0].verdict = Verdict::Unknown;
    let empty = Report {
        checks: vec![],
        ..ready.clone()
    };
    assert_eq!(host_state(&[]), HostState::NotChecked);
    assert_eq!(host_state(std::slice::from_ref(&ready)), HostState::Ready);
    assert_eq!(
        host_state(&[ready.clone(), unknown]),
        HostState::NotReady { projects: 1 }
    );
    assert_eq!(host_state(&[empty]), HostState::NotReady { projects: 1 });
}

/// A forge that answers by REST path, and fails what nobody scripted.
struct Fake(HashMap<String, Reply>);

impl Fake {
    fn new(answers: &[(&str, u16, &str)]) -> Arc<Fake> {
        Arc::new(Fake(
            answers
                .iter()
                .map(|(path, status, body)| {
                    let reply = if *status < 400 {
                        Reply {
                            code: 0,
                            out: (*body).to_owned(),
                            err: String::new(),
                            status: Some(*status),
                            headers: vec![],
                        }
                    } else {
                        Reply {
                            code: 1,
                            out: (*body).to_owned(),
                            err: format!("HTTP {status}"),
                            status: Some(*status),
                            headers: vec![],
                        }
                    };
                    ((*path).to_owned(), reply)
                })
                .collect(),
        ))
    }
}

impl Transport for Fake {
    fn send(&self, _: &Forge, call: &Call) -> Result<Reply, NoAnswer> {
        let Endpoint::Rest { path, .. } = &call.endpoint else {
            return Err(NoAnswer::Missing("graphql".to_owned()));
        };
        self.0
            .get(path)
            .cloned()
            .ok_or_else(|| NoAnswer::Missing(format!("not scripted: {path}")))
    }

    fn check_auth(&self, _: &Forge) -> Result<(), ForgeError> {
        Ok(())
    }
}

const GITHUB_REPO: &str = r#"{"private":true,"owner":{"login":"acme","type":"Organization"},
    "default_branch":"main","permissions":{"admin":false,"push":true,"pull":true}}"#;

#[test]
fn reading_a_github_org_repo_through_the_forge_is_ready_when_all_is_open() {
    let fake = Fake::new(&[
        ("repos/acme/app", 200, GITHUB_REPO),
        ("user/memberships/orgs/acme", 404, "{}"),
        ("user", 200, r#"{"site_admin":false}"#),
        (
            "repos/acme/app/rules/branches/main",
            200,
            r#"[{"type":"pull_request"}]"#,
        ),
    ]);
    let report = check(
        &github("github.com"),
        "acme/app",
        Connection::SignIn,
        &Caller::window(),
        None,
        fake,
    );
    assert!(report.ready_for_all(), "{report:#?}");
}

#[test]
fn reading_a_restricted_org_names_githubs_refusal_as_the_blocker() {
    let said = "Although you appear to have the correct authorization credentials, the `acme` organization has enabled OAuth App access restrictions";
    let body = format!(r#"{{"message":"{said}"}}"#);
    let fake = Fake::new(&[("repos/acme/app", 403, &body)]);
    let report = check(
        &github("github.com"),
        "acme/app",
        Connection::SignIn,
        &Caller::window(),
        None,
        fake,
    );
    assert_eq!(
        verdict(&report, CheckId::OauthAppAccess),
        Some(Verdict::Blocked(Who::Owner))
    );
    assert_eq!(verdict(&report, CheckId::Role), Some(Verdict::Unknown));
    assert!(!report.ready_for_all());
}

#[test]
fn a_404_on_a_repo_is_unknown_never_blocked_or_ready() {
    let fake = Fake::new(&[("repos/acme/app", 404, r#"{"message":"Not Found"}"#)]);
    let report = check(
        &github("github.com"),
        "acme/app",
        Connection::SignIn,
        &Caller::window(),
        None,
        fake,
    );
    assert_eq!(verdict(&report, CheckId::Role), Some(Verdict::Unknown));
    assert!(!report.ready_for_all());
}

#[test]
fn a_forge_that_answers_nothing_is_not_ready() {
    let report = check(
        &github("github.com"),
        "acme/app",
        Connection::SignIn,
        &Caller::window(),
        None,
        Fake::new(&[]),
    );
    assert!(!report.ready_for_all());
    assert!(report.unknowns().count() >= 2, "{report:#?}");
}

#[test]
fn reading_the_workflow_permissions_of_a_token_connection() {
    let fake = Fake::new(&[
        ("repos/acme/app", 200, GITHUB_REPO),
        ("user/memberships/orgs/acme", 404, "{}"),
        ("user", 200, r#"{"site_admin":false}"#),
        (
            "repos/acme/app/rules/branches/main",
            200,
            r#"[{"type":"pull_request"}]"#,
        ),
        (
            "repos/acme/app/actions/permissions/workflow",
            200,
            r#"{"default_workflow_permissions":"read"}"#,
        ),
    ]);
    let report = check(
        &github("github.com"),
        "acme/app",
        Connection::Token,
        &Caller::window(),
        None,
        fake,
    );
    assert_eq!(
        verdict(&report, CheckId::CiToken),
        Some(Verdict::Blocked(Who::Owner))
    );
}

#[test]
fn a_workflow_permission_only_admins_read_is_unknown() {
    let fake = Fake::new(&[
        ("repos/acme/app", 200, GITHUB_REPO),
        ("user/memberships/orgs/acme", 404, "{}"),
        ("user", 200, r#"{"site_admin":false}"#),
        (
            "repos/acme/app/rules/branches/main",
            200,
            r#"[{"type":"pull_request"}]"#,
        ),
        (
            "repos/acme/app/actions/permissions/workflow",
            403,
            r#"{"message":"Must have admin rights"}"#,
        ),
    ]);
    let report = check(
        &github("github.com"),
        "acme/app",
        Connection::Token,
        &Caller::window(),
        None,
        fake,
    );
    assert_eq!(verdict(&report, CheckId::CiToken), Some(Verdict::Unknown));
}

#[test]
fn reading_a_self_hosted_gitlab_asks_its_application_settings() {
    let project = r#"{"default_branch":"main","namespace":{"kind":"group","full_path":"acme"},
        "permissions":{"project_access":{"access_level":30},"group_access":null},
        "ci_push_repository_for_job_token_allowed":true}"#;
    let fake = Fake::new(&[
        ("projects/acme%2Fapp", 200, project),
        ("user", 200, r#"{"is_admin":false}"#),
        (
            "application/settings",
            403,
            r#"{"message":"403 Forbidden"}"#,
        ),
        (
            "projects/acme%2Fapp/repository/branches/main",
            200,
            r#"{"protected":true}"#,
        ),
    ]);
    let report = check(
        &gitlab("git.example.com"),
        "acme/app",
        Connection::SignIn,
        &Caller::window(),
        None,
        fake,
    );
    assert_eq!(verdict(&report, CheckId::Role), Some(Verdict::Ready));
    assert_eq!(
        verdict(&report, CheckId::ServerRegistration),
        Some(Verdict::Blocked(Who::Admin))
    );
    assert_eq!(
        verdict(&report, CheckId::OauthApplications),
        Some(Verdict::Unknown)
    );
    assert_eq!(
        verdict(&report, CheckId::BranchProtection),
        Some(Verdict::Ready)
    );
}

#[test]
fn a_gitlab_job_token_that_may_not_push_blocks_a_token_connection() {
    let project = r#"{"default_branch":"main","namespace":{"kind":"group","full_path":"acme"},
        "permissions":{"project_access":{"access_level":40},"group_access":null},
        "ci_push_repository_for_job_token_allowed":false}"#;
    let fake = Fake::new(&[
        ("projects/acme%2Fapp", 200, project),
        ("user", 200, r#"{"is_admin":false}"#),
        (
            "projects/acme%2Fapp/repository/branches/main",
            200,
            r#"{"protected":true}"#,
        ),
    ]);
    let report = check(
        &gitlab("gitlab.com"),
        "acme/app",
        Connection::Token,
        &Caller::window(),
        None,
        fake,
    );
    assert_eq!(
        verdict(&report, CheckId::CiToken),
        Some(Verdict::Blocked(Who::You))
    );
}

fn refusing(status: u16, headers: &[(&str, &str)], body: &str) -> Arc<Fake> {
    let reply = Reply {
        code: 1,
        out: body.to_owned(),
        err: format!("HTTP {status}"),
        status: Some(status),
        headers: headers
            .iter()
            .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
            .collect(),
    };
    Arc::new(Fake(HashMap::from([("repos/acme/app".to_owned(), reply)])))
}

fn read_by(connection: Connection, fake: Arc<Fake>) -> Report {
    check(
        &github("github.com"),
        "acme/app",
        connection,
        &Caller::window(),
        None,
        fake,
    )
}

#[test]
fn a_private_org_repo_read_over_the_cli_login_proves_nothing_about_purlis() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    let report = evaluate(&github("github.com"), "acme/app", Connection::Cli, &f);
    let approval = check_of(&report, CheckId::OauthAppAccess);
    assert_eq!(approval.verdict, Verdict::Unknown);
    assert!(
        approval.fix.as_ref().unwrap().steps[0].contains("Sign in to purlis"),
        "{approval:#?}"
    );
    assert!(!report.ready_for_all());
}

#[test]
fn a_restriction_seen_over_the_cli_login_is_not_purlis_being_refused() {
    let f = all_known(RepoRead::Restricted("the CLI's app was refused".to_owned()));
    let report = evaluate(&github("github.com"), "acme/app", Connection::Cli, &f);
    assert_eq!(
        verdict(&report, CheckId::OauthAppAccess),
        Some(Verdict::Unknown)
    );
}

#[test]
fn a_personal_repo_read_over_the_cli_login_has_still_no_approval_to_check() {
    let mut f = facts(Role::Admin, OrgAccess::NotApplicable);
    f.owner_is_group = false;
    let report = evaluate(
        &github("github.com"),
        "me/app",
        Connection::Cli,
        &all_known(RepoRead::Read(f)),
    );
    assert_eq!(verdict(&report, CheckId::OauthAppAccess), None);
}

#[test]
fn the_cli_login_is_still_held_to_the_server_knowing_purlis() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    let report = evaluate(&github("ghe.example.com"), "acme/app", Connection::Cli, &f);
    assert!(matches!(
        verdict(&report, CheckId::ServerRegistration),
        Some(Verdict::Blocked(_))
    ));
}

#[test]
fn a_plain_403_names_both_causes_and_is_unknown_never_ready() {
    let report = read_by(
        Connection::SignIn,
        refusing(403, &[], r#"{"message":"Forbidden"}"#),
    );
    let approval = check_of(&report, CheckId::OauthAppAccess);
    assert_eq!(approval.verdict, Verdict::Unknown);
    let fix = approval.fix.as_ref().unwrap();
    let text = fix.steps.join("\n");
    assert!(text.contains("OAuth app restriction"), "{text}");
    assert!(text.contains("IP allow list"), "{text}");
    assert!(text.contains("Enterprise Managed Users"), "{text}");
    assert!(fix.request.is_some());
    assert!(!report.ready_for_all());
}

#[test]
fn a_saml_403_is_unknown_with_its_own_way_out_and_no_request() {
    let report = read_by(
        Connection::SignIn,
        refusing(
            403,
            &[(
                "x-github-sso",
                "required; url=https://github.com/orgs/acme/sso?authorization_request=x",
            )],
            r#"{"message":"Resource protected by organization SAML enforcement."}"#,
        ),
    );
    let approval = check_of(&report, CheckId::OauthAppAccess);
    assert_eq!(approval.verdict, Verdict::Unknown);
    assert!(approval.detail.contains("SAML"), "{}", approval.detail);
    let fix = approval.fix.as_ref().unwrap();
    // A header's address is the forge's to name: it is never copied into a link or a request.
    assert!(!fix.link.contains("authorization_request"), "{fix:#?}");
    assert!(fix.request.is_none());
}

#[test]
fn a_rate_limited_403_says_nothing_about_the_organization() {
    let report = read_by(
        Connection::SignIn,
        refusing(
            403,
            &[("x-ratelimit-remaining", "0")],
            r#"{"message":"API rate limit exceeded"}"#,
        ),
    );
    let approval = check_of(&report, CheckId::OauthAppAccess);
    assert_eq!(approval.verdict, Verdict::Unknown);
    assert!(
        approval.detail.starts_with("not read"),
        "{}",
        approval.detail
    );
    assert!(!report.ready_for_all());
}

#[test]
fn the_restriction_words_on_a_404_are_not_the_restriction() {
    let report = read_by(
        Connection::SignIn,
        refusing(404, &[], r#"{"message":"OAuth App access restrictions"}"#),
    );
    assert_eq!(
        verdict(&report, CheckId::OauthAppAccess),
        Some(Verdict::Unknown)
    );
}

#[test]
fn a_group_owner_registers_purlis_on_their_servers_group_without_asking() {
    let mut member = facts(Role::Admin, OrgAccess::Approved);
    member.org_owner = true;
    let f = all_known(RepoRead::Read(member));
    let report = evaluate(
        &github("ghe.example.com"),
        "acme/app",
        Connection::SignIn,
        &f,
    );
    let check = check_of(&report, CheckId::ServerRegistration);
    assert_eq!(check.verdict, Verdict::Blocked(Who::You));
    assert!(
        check
            .fix
            .as_ref()
            .unwrap()
            .steps
            .iter()
            .any(|s| s.contains("organizations/acme/settings/applications/new"))
    );
}

#[test]
fn a_member_asks_the_administrator_or_the_groups_owner() {
    let f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    let report = evaluate(
        &gitlab("git.example.com"),
        "acme/app",
        Connection::SignIn,
        &f,
    );
    let check = check_of(&report, CheckId::ServerRegistration);
    assert_eq!(check.verdict, Verdict::Blocked(Who::Admin));
    let request = check.fix.as_ref().unwrap().request.as_ref().unwrap();
    assert!(request.contains("as an owner of acme"), "{request}");
}

#[test]
fn a_host_with_a_pinned_application_is_not_told_it_has_no_registration() {
    let mut f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    f.registered = Fact::Known(true);
    let report = evaluate(
        &gitlab("git.example.com"),
        "acme/app",
        Connection::SignIn,
        &f,
    );
    assert_eq!(verdict(&report, CheckId::ServerRegistration), None);
    assert_eq!(verdict(&report, CheckId::OauthApplications), None);
}

#[test]
fn pins_that_could_not_be_read_leave_the_registration_unknown_and_not_ready() {
    let mut f = all_known(RepoRead::Read(facts(Role::Write, OrgAccess::Approved)));
    f.registered = Fact::unknown("the file of pinned applications is not readable");
    let report = evaluate(
        &gitlab("git.example.com"),
        "acme/app",
        Connection::SignIn,
        &f,
    );
    assert_eq!(
        verdict(&report, CheckId::ServerRegistration),
        Some(Verdict::Unknown)
    );
    assert!(!report.ready_for_all());
}

/// The pins the person pressed, through slice C's store: a real file in a temporary config home.
mod the_pins_the_store_keeps {
    use super::*;
    use crate::forge::registered;
    use crate::forge::route::HostScope;

    fn pinned_on(host: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        registered::pin(
            &HostScope::for_a_test(),
            dir.path(),
            Kind::GitLab,
            host,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        dir
    }

    #[test]
    fn a_pinned_host_is_registered_and_another_is_not() {
        let dir = pinned_on("git.example.com");
        let at = |kind, host: &str| {
            registered_in(
                Some(dir.path()),
                &Forge {
                    kind,
                    host: host.to_owned(),
                },
            )
        };
        assert_eq!(at(Kind::GitLab, "git.example.com"), Fact::Known(true));
        assert_eq!(at(Kind::GitLab, "other.example.com"), Fact::Known(false));
        // A pin is of one forge: the other forge on the same host has none.
        assert_eq!(at(Kind::GitHub, "git.example.com"), Fact::Known(false));
    }

    #[test]
    fn the_whole_check_reads_the_pins_of_the_config_home_it_is_given() {
        let dir = pinned_on("git.example.com");
        let read = |pins: Option<&std::path::Path>| {
            check(
                &gitlab("git.example.com"),
                "acme/app",
                Connection::SignIn,
                &Caller::window(),
                pins,
                Fake::new(&[]),
            )
        };
        assert_eq!(
            verdict(&read(Some(dir.path())), CheckId::ServerRegistration),
            None
        );
        assert!(matches!(
            verdict(&read(None), CheckId::ServerRegistration),
            Some(Verdict::Blocked(_))
        ));
    }

    #[test]
    fn a_host_purlis_ships_a_registration_on_needs_no_pin() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            registered_in(Some(dir.path()), &github("github.com")),
            Fact::Known(true)
        );
    }

    #[test]
    fn no_config_home_holds_no_pin() {
        assert_eq!(
            registered_in(None, &gitlab("git.example.com")),
            Fact::Known(false)
        );
    }

    #[test]
    fn a_pin_file_that_cannot_be_read_is_unknown_and_never_none() {
        let dir = pinned_on("git.example.com");
        std::fs::write(registered::path(dir.path()), "not json").unwrap();
        assert!(matches!(
            registered_in(Some(dir.path()), &gitlab("git.example.com")),
            Fact::Unknown(_)
        ));
    }
}

#[test]
fn only_purlis_own_oauth_flows_are_a_sign_in() {
    use crate::forge::accounts::Method;
    assert_eq!(
        Connection::of_method(Some(Method::Device)),
        Connection::SignIn
    );
    assert_eq!(
        Connection::of_method(Some(Method::Pkce)),
        Connection::SignIn
    );
    assert_eq!(Connection::of_method(Some(Method::Import)), Connection::Cli);
    assert_eq!(Connection::of_method(Some(Method::Pat)), Connection::Pasted);
    assert_ne!(Connection::of_method(Some(Method::Pat)), Connection::Token);
    assert_eq!(Connection::of_method(None), Connection::Cli);
}
